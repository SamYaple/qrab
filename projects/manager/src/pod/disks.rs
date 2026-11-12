//! Online growth of native iSCSI disks. The operator grows the LUN first,
//! then names the new capacity in a Pod annotation the kubelet projects to
//! `<limits_dir>/disk_capacity` as `{"<disk id>": bytes}`. The manager calls
//! `block_resize` on the disk's raw node only to grow it: libiscsi re-reads the
//! LUN capacity and refuses sizes the target does not have, and virtio-blk
//! signals the guest. Nothing is ever shrunk.
use super::{hotplug::retry_delay_ms, Config};
use crate::Result;
use qapi::commands::{BlockResize, QueryNamedBlockNodes};
use qapi_protocol::Qmp;
use std::collections::BTreeMap;

pub const CAPACITY_FILE: &str = "disk_capacity";

/// The QEMU raw node a native iSCSI disk is attached through.
pub fn node(id: &str) -> String {
    format!("iscsi-{id}")
}

#[derive(Debug, Default)]
pub struct DiskGrowth {
    /// Projected file content last applied successfully.
    applied: Option<String>,
    failures: u32,
    retry_after_ms: Option<u64>,
}

/// Requested capacities for writable configured disks only.
pub fn requested(config: &Config, text: &str) -> BTreeMap<String, u64> {
    let Ok(map) = serde_json::from_str::<BTreeMap<String, u64>>(text) else {
        return BTreeMap::new();
    };
    map.into_iter()
        .filter(|(id, size)| {
            *size > 0
                && size.is_multiple_of(512)
                && i64::try_from(*size).is_ok()
                && config
                    .iscsi_disks
                    .iter()
                    .any(|d| &d.id == id && !d.read_only)
        })
        .collect()
}

/// One growth step per heartbeat. A failure retries with the hot-plug backoff;
/// a changed annotation retries at once.
pub(super) async fn reconcile_at(config: &Config, qmp: &Qmp, growth: &mut DiskGrowth, now: u64) {
    let Some(hotplug) = &config.hotplug else {
        return;
    };
    let Ok(text) = std::fs::read_to_string(hotplug.limits_dir.join(CAPACITY_FILE)) else {
        return;
    };
    let text = text.trim().to_owned();
    let unchanged = growth.applied.as_deref() == Some(text.as_str());
    if unchanged || (growth.failures > 0 && growth.retry_after_ms.is_some_and(|t| now < t)) {
        return;
    }
    let wanted = requested(config, &text);
    let result = grow(qmp, &wanted).await;
    match result {
        Ok(grown) => {
            for (id, size) in grown {
                eprintln!(
                    "{}",
                    serde_json::json!({"event":"disk_grown","disk":id,"capacity_bytes":size})
                );
            }
            growth.applied = Some(text);
            growth.failures = 0;
            growth.retry_after_ms = None;
        }
        Err(error) => {
            growth.failures = growth.failures.saturating_add(1);
            let delay = retry_delay_ms(growth.failures);
            growth.retry_after_ms = Some(now.saturating_add(delay));
            eprintln!(
                "{}",
                serde_json::json!({"event":"disk_grow_failed","failures":growth.failures,"retry_in_ms":delay,"error":error.to_string()})
            );
        }
    }
}

pub(super) async fn reconcile(config: &Config, qmp: &Qmp, growth: &mut DiskGrowth) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    reconcile_at(config, qmp, growth, now).await
}

async fn grow(qmp: &Qmp, wanted: &BTreeMap<String, u64>) -> Result<Vec<(String, u64)>> {
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    let nodes = qmp
        .execute(&QueryNamedBlockNodes { flat: Some(true) })
        .await?;
    let mut grown = Vec::new();
    for (id, size) in wanted {
        let name = node(id);
        let Some(current) = nodes
            .iter()
            .find(|n| n.node_name.as_deref() == Some(name.as_str()))
            .and_then(|n| n.image.as_ref())
            .map(|i| i.virtual_size)
        else {
            continue;
        };
        let size = i64::try_from(*size).unwrap_or(i64::MAX);
        if size > current {
            qmp.execute(&BlockResize {
                device: None,
                node_name: Some(name),
                size,
            })
            .await?;
            grown.push((id.clone(), size as u64));
        }
    }
    Ok(grown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn config(dir: &Path) -> Config {
        std::fs::create_dir_all(dir.join("limits")).unwrap();
        let disk = |id: &str, read_only: bool| {
            serde_json::json!({"id":id,"portal":"10.203.0.1:3260","target":format!("iqn.2026-11.com.yapplesauce:qrab.disk.x.{id}"),
                "initiator_name":"iqn.2026-11.com.yapplesauce:qrab.vm.x","username":"u","password_file":"/run/qrab-auth/x/password","read_only":read_only})
        };
        serde_json::from_value(serde_json::json!({
            "kernel": "/unused", "runtime_dir": dir, "cpus": 1, "memory_mib": 512,
            "iscsi_disks": [disk("root", false), disk("media", true)],
            "hotplug": {"max_cpus": 1, "max_memory_mib": 512, "limits_dir": dir.join("limits"),
                "cgroup_dir": dir.join("cgroup"), "cpu_overhead_millis": 1000, "memory_overhead_mib": 512}
        }))
        .unwrap()
    }

    #[test]
    fn only_configured_writable_disks_with_sane_sizes_are_requested() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(dir.path());
        let wanted = requested(
            &config,
            r#"{"root":134217728,"media":134217728,"other":134217728}"#,
        );
        assert_eq!(wanted, BTreeMap::from([("root".into(), 134217728)]));
        assert!(requested(&config, r#"{"root":1}"#).is_empty());
        assert!(requested(&config, r#"{"root":-1}"#).is_empty());
        assert!(requested(&config, "").is_empty());
        assert!(requested(&config, "not json").is_empty());
    }

    #[tokio::test]
    async fn growth_resizes_only_upward_and_retries_failures_with_backoff() {
        use qapi_protocol::server::QmpSession;
        use std::sync::{Arc, Mutex};
        let dir = tempfile::tempdir().unwrap();
        let config = config(dir.path());
        let socket = dir.path().join("qmp.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let resizes = Arc::new(Mutex::new(Vec::new()));
        let seen = resizes.clone();
        let fail = Arc::new(Mutex::new(true));
        let failing = fail.clone();
        let server = tokio::spawn(async move {
            let mut session = QmpSession::accept(&listener).await.unwrap();
            session.send_greeting(11, 1, 3).await.unwrap();
            session.negotiate().await.unwrap();
            let mut size: i64 = 67108864;
            while let Ok((name, arguments)) = session.recv_command().await {
                match name.as_str() {
                    "query-named-block-nodes" => session
                        .send_return(serde_json::json!([
                            {"node-name":"iscsi-root-lun","drv":"iscsi","encrypted":false,"file":"iscsi","ro":false,
                             "image":{"filename":"iscsi","format":"iscsi","virtual-size":size}},
                            {"node-name":"iscsi-root","drv":"raw","encrypted":false,"file":"iscsi","ro":false,
                             "image":{"filename":"iscsi","format":"raw","virtual-size":size}}
                        ]))
                        .await
                        .unwrap(),
                    "block_resize" => {
                        seen.lock().unwrap().push(arguments.clone());
                        if *failing.lock().unwrap() {
                            session
                                .send_error("GenericError", "Cannot grow iSCSI devices")
                                .await
                                .unwrap();
                        } else {
                            size = arguments["size"].as_i64().unwrap();
                            session.send_return(serde_json::json!({})).await.unwrap();
                        }
                    }
                    _ => session.send_return(serde_json::json!({})).await.unwrap(),
                }
            }
        });
        let (qmp, _events) = Qmp::new(&socket).await.unwrap();
        let file = dir.path().join("limits").join(CAPACITY_FILE);
        let mut growth = DiskGrowth::default();
        // Equal size: nothing to do, and the annotation counts as applied.
        std::fs::write(&file, r#"{"root":67108864}"#).unwrap();
        reconcile_at(&config, &qmp, &mut growth, 0).await;
        assert!(resizes.lock().unwrap().is_empty());
        // A smaller size never shrinks the disk.
        std::fs::write(&file, r#"{"root":33554432}"#).unwrap();
        reconcile_at(&config, &qmp, &mut growth, 1000).await;
        assert!(resizes.lock().unwrap().is_empty());
        // Growth before the LUN grew fails and backs off.
        std::fs::write(&file, r#"{"root":134217728}"#).unwrap();
        reconcile_at(&config, &qmp, &mut growth, 2000).await;
        assert_eq!(growth.failures, 1);
        reconcile_at(&config, &qmp, &mut growth, 2500).await;
        assert_eq!(resizes.lock().unwrap().len(), 1, "waits out the backoff");
        *fail.lock().unwrap() = false;
        reconcile_at(&config, &qmp, &mut growth, 3000).await;
        assert_eq!(growth.failures, 0);
        let calls = resizes.lock().unwrap().clone();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1]["node-name"], "iscsi-root");
        assert_eq!(calls[1]["size"], 134217728);
        // Applied: the same annotation causes no further QMP traffic.
        reconcile_at(&config, &qmp, &mut growth, 4000).await;
        assert_eq!(resizes.lock().unwrap().len(), 2);
        qmp.shutdown().await;
        server.await.unwrap();
    }
}

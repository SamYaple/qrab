//! In-place guest growth. The container's downward-API limits name the
//! desired size; vCPUs and virtio-mem memory are added only once the applied
//! cgroup v2 limits can carry them. Nothing is ever unplugged.
use super::{config::HOTPLUG_BLOCK_MIB, Config, Hotplug};
use crate::Result;
use qapi::{
    commands::{DeviceAdd, QomGet, QomSet, QueryHotpluggableCpus},
    CpuInstanceProperties,
};
use qapi_protocol::Qmp;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// QOM id of the virtio-mem device; its memory backend is `hotplug-memory`.
pub const MEMORY_DEVICE: &str = "hotplug-memory0";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuestSize {
    pub cpus: u32,
    pub memory_mib: u64,
}

/// Where in-place growth stands; non-sensitive, shown to BMC viewers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResizePhase {
    /// The guest has the desired size, or no resize has been requested.
    #[default]
    Idle,
    /// The container limits changed but the kubelet has not applied them.
    WaitingForLimits,
    /// A hot-plug step failed; the next attempt waits `retry_after_unix_ms`.
    Retrying,
    /// The desired size exceeds the boot-time maximum.
    RestartRequired,
    /// Container limit or cgroup files cannot be read.
    LimitsUnavailable,
}

/// First retry delay after a failed hot-plug step; doubles per failure.
pub const RETRY_INITIAL_MS: u64 = 1000;
/// Retry delay ceiling, so a persistent failure costs one attempt a minute.
pub const RETRY_MAX_MS: u64 = 60_000;

/// Delay before attempt `failures + 1`: 1 s, 2 s, 4 s ... capped at 60 s.
pub fn retry_delay_ms(failures: u32) -> u64 {
    RETRY_INITIAL_MS
        .saturating_mul(1 << failures.saturating_sub(1).min(16))
        .min(RETRY_MAX_MS)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Resize progress published with the runtime status.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resize {
    pub boot: GuestSize,
    pub maximum: GuestSize,
    /// Size requested by the container limits (limit minus fixed overhead).
    pub desired: Option<GuestSize>,
    /// Size the applied cgroup limits can carry.
    pub permitted: Option<GuestSize>,
    /// Present vCPUs and boot plus requested virtio-mem memory.
    pub current: GuestSize,
    /// Memory the guest's virtio-mem driver has actually plugged.
    pub plugged_memory_mib: Option<u64>,
    pub detail: Option<String>,
    #[serde(default)]
    pub phase: ResizePhase,
    /// Consecutive failed hot-plug steps; reset by a successful step.
    #[serde(default)]
    pub failures: u32,
    /// The most recent hot-plug failure, kept until a step succeeds.
    #[serde(default)]
    pub last_error: Option<String>,
    /// No QMP hot-plug attempt is made before this time unless the
    /// container limits change.
    #[serde(default)]
    pub retry_after_unix_ms: Option<u64>,
}

impl Resize {
    pub fn new(config: &Config) -> Option<Self> {
        let hotplug = config.hotplug.as_ref()?;
        let boot = GuestSize {
            cpus: config.cpus,
            memory_mib: config.memory_mib,
        };
        Some(Self {
            boot,
            maximum: GuestSize {
                cpus: config.max_cpus(),
                memory_mib: hotplug.max_memory_mib.max(config.memory_mib),
            },
            current: boot,
            ..Self::default()
        })
    }
}

fn read_trimmed(path: &Path) -> Option<String> {
    Some(std::fs::read_to_string(path).ok()?.trim().to_owned())
}

/// Guest size named by the downward-API limit files.
pub fn desired(hotplug: &Hotplug) -> Option<GuestSize> {
    let cpu: u64 = read_trimmed(&hotplug.limits_dir.join("cpu_limit_millis"))?
        .parse()
        .ok()?;
    let memory: u64 = read_trimmed(&hotplug.limits_dir.join("memory_limit_mib"))?
        .parse()
        .ok()?;
    Some(GuestSize {
        cpus: u32::try_from(cpu.checked_sub(hotplug.cpu_overhead_millis)? / 1000).ok()?,
        memory_mib: memory.checked_sub(hotplug.memory_overhead_mib)?,
    })
}

/// Guest size the container's applied cgroup v2 limits can carry; `max` is unbounded.
pub fn permitted(hotplug: &Hotplug) -> Option<GuestSize> {
    let cpu = read_trimmed(&hotplug.cgroup_dir.join("cpu.max"))?;
    let (quota, period) = cpu.split_once(' ')?;
    let cpus = if quota == "max" {
        u32::MAX
    } else {
        let millis =
            quota.parse::<u64>().ok()?.checked_mul(1000)? / period.parse::<u64>().ok()?.max(1);
        u32::try_from(millis.saturating_sub(hotplug.cpu_overhead_millis) / 1000).unwrap_or(u32::MAX)
    };
    let memory = read_trimmed(&hotplug.cgroup_dir.join("memory.max"))?;
    let memory_mib = if memory == "max" {
        u64::MAX
    } else {
        (memory.parse::<u64>().ok()? >> 20).saturating_sub(hotplug.memory_overhead_mib)
    };
    Some(GuestSize { cpus, memory_mib })
}

/// Grow toward `desired` within the boot maxima and the applied limits; never shrink.
pub fn target(
    current: GuestSize,
    maximum: GuestSize,
    desired: GuestSize,
    permitted: GuestSize,
) -> GuestSize {
    GuestSize {
        cpus: desired
            .cpus
            .min(permitted.cpus)
            .min(maximum.cpus)
            .max(current.cpus),
        memory_mib: desired
            .memory_mib
            .min(permitted.memory_mib)
            .min(maximum.memory_mib)
            .max(current.memory_mib),
    }
}

fn slot_order(props: &CpuInstanceProperties) -> [Option<i64>; 7] {
    [
        props.drawer_id,
        props.book_id,
        props.socket_id,
        props.die_id,
        props.cluster_id,
        props.core_id,
        props.thread_id,
    ]
}

/// Fill free CPU slots in topology order without exceeding `cpus`; returns the present count.
async fn add_cpus(qmp: &Qmp, cpus: u32) -> Result<u32> {
    let slots = qmp.execute(&QueryHotpluggableCpus {}).await?;
    let count = |vcpus: i64| u32::try_from(vcpus).unwrap_or(1).max(1);
    let mut present: u32 = slots
        .iter()
        .filter(|s| s.qom_path.is_some())
        .map(|s| count(s.vcpus_count))
        .sum();
    let mut free: Vec<_> = slots.into_iter().filter(|s| s.qom_path.is_none()).collect();
    free.sort_by_key(|s| slot_order(&s.props));
    for slot in free {
        let vcpus = count(slot.vcpus_count);
        if present + vcpus > cpus {
            break;
        }
        let props = match qapi::to_value(qmp.version(), &slot.props)? {
            serde_json::Value::Object(map) => {
                map.into_iter().filter(|(_, v)| !v.is_null()).collect()
            }
            _ => Default::default(),
        };
        qmp.execute(&DeviceAdd {
            driver: slot.r#type,
            id: Some(format!("qrab-vcpu{present}")),
            bus: None,
            arguments: Some(props),
        })
        .await?;
        present += vcpus;
    }
    Ok(present)
}

/// One growth step. Returns true when the published resize state changed.
pub(super) async fn reconcile(config: &Config, qmp: &Qmp, resize: &mut Resize) -> bool {
    reconcile_at(config, qmp, resize, now_ms()).await
}

/// A failed step is retried with capped exponential backoff instead of on
/// every heartbeat; changed container limits retry immediately.
async fn reconcile_at(config: &Config, qmp: &Qmp, resize: &mut Resize, now: u64) -> bool {
    let Some(hotplug) = &config.hotplug else {
        return false;
    };
    let before = resize.clone();
    resize.desired = desired(hotplug);
    resize.permitted = permitted(hotplug);
    let limits_changed = resize.desired != before.desired || resize.permitted != before.permitted;
    if !limits_changed && resize.retry_after_unix_ms.is_some_and(|t| now < t) {
        return false;
    }
    let result = grow(qmp, resize).await;
    match &result {
        Err(error) => {
            resize.failures = resize.failures.saturating_add(1);
            resize.last_error = Some(error.to_string());
            let delay = retry_delay_ms(resize.failures);
            resize.retry_after_unix_ms = Some(now.saturating_add(delay));
            eprintln!(
                "{}",
                serde_json::json!({"event":"guest_resize_failed","failures":resize.failures,"retry_in_ms":delay,"error":error.to_string()})
            );
        }
        Ok(()) => {
            resize.failures = 0;
            resize.last_error = None;
            resize.retry_after_unix_ms = None;
        }
    }
    let (Some(desired), Some(permitted)) = (resize.desired, resize.permitted) else {
        resize.phase = ResizePhase::LimitsUnavailable;
        resize.detail = Some("container limit or cgroup files are unavailable".into());
        return *resize != before;
    };
    (resize.phase, resize.detail) = match &result {
        Err(error) => (
            ResizePhase::Retrying,
            Some(format!("hot-plug failed: {error}")),
        ),
        Ok(())
            if desired.cpus > resize.maximum.cpus
                || desired.memory_mib > resize.maximum.memory_mib =>
        {
            (
                ResizePhase::RestartRequired,
                Some("desired size exceeds the boot-time maximum; a restart is required".into()),
            )
        }
        Ok(()) if desired.cpus > permitted.cpus || desired.memory_mib > permitted.memory_mib => (
            ResizePhase::WaitingForLimits,
            Some("waiting for the kubelet to raise the container cgroup limits".into()),
        ),
        Ok(()) => (ResizePhase::Idle, None),
    };
    if resize.current != before.current {
        eprintln!(
            "{}",
            serde_json::json!({"event":"guest_resized","cpus":resize.current.cpus,"memory_mib":resize.current.memory_mib})
        );
    }
    *resize != before
}

async fn grow(qmp: &Qmp, resize: &mut Resize) -> Result<()> {
    let (Some(desired), Some(permitted)) = (resize.desired, resize.permitted) else {
        return Ok(());
    };
    let goal = target(resize.current, resize.maximum, desired, permitted);
    if goal.cpus > resize.current.cpus {
        // Record partial progress even if a later device_add fails.
        resize.current.cpus = add_cpus(qmp, goal.cpus).await?.max(resize.current.cpus);
    }
    if goal.memory_mib > resize.current.memory_mib {
        let requested = (goal.memory_mib - resize.boot.memory_mib).div_ceil(HOTPLUG_BLOCK_MIB)
            * HOTPLUG_BLOCK_MIB;
        qmp.execute(&QomSet {
            path: format!("/machine/peripheral/{MEMORY_DEVICE}"),
            property: "requested-size".into(),
            value: (requested << 20).into(),
            arguments: None,
        })
        .await?;
        resize.current.memory_mib = goal.memory_mib;
    }
    if resize.current.memory_mib > resize.boot.memory_mib {
        let plugged = qmp
            .execute(&QomGet {
                path: format!("/machine/peripheral/{MEMORY_DEVICE}"),
                property: "size".into(),
                arguments: None,
            })
            .await?;
        resize.plugged_memory_mib = plugged.as_u64().map(|bytes| bytes >> 20);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: GuestSize = GuestSize {
        cpus: 1,
        memory_mib: 512,
    };

    #[test]
    fn growth_is_bounded_by_limits_and_maxima_and_never_shrinks() {
        let max = GuestSize {
            cpus: 4,
            memory_mib: 2048,
        };
        let want = GuestSize {
            cpus: 8,
            memory_mib: 4096,
        };
        let allowed = GuestSize {
            cpus: 2,
            memory_mib: 1024,
        };
        assert_eq!(
            target(ONE, max, want, allowed),
            GuestSize {
                cpus: 2,
                memory_mib: 1024
            }
        );
        assert_eq!(
            target(
                ONE,
                max,
                want,
                GuestSize {
                    cpus: u32::MAX,
                    memory_mib: u64::MAX
                }
            ),
            max
        );
        let grown = GuestSize {
            cpus: 3,
            memory_mib: 1536,
        };
        assert_eq!(target(grown, max, ONE, ONE), grown);
    }

    fn config(dir: &Path) -> Config {
        for sub in ["limits", "cgroup"] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
        }
        serde_json::from_value(serde_json::json!({
            "kernel": "/unused", "runtime_dir": dir, "cpus": 1, "memory_mib": 512,
            "hotplug": {"max_cpus": 4, "max_memory_mib": 2047, "limits_dir": dir.join("limits"),
                "cgroup_dir": dir.join("cgroup"), "cpu_overhead_millis": 1000, "memory_overhead_mib": 512}
        }))
        .unwrap()
    }

    fn write(
        dir: &Path,
        cpu_limit_millis: u64,
        memory_limit_mib: u64,
        cpu_max: &str,
        memory_max: &str,
    ) {
        for (name, value) in [
            ("limits/cpu_limit_millis", format!("{cpu_limit_millis}\n")),
            ("limits/memory_limit_mib", format!("{memory_limit_mib}\n")),
            ("cgroup/cpu.max", format!("{cpu_max}\n")),
            ("cgroup/memory.max", format!("{memory_max}\n")),
        ] {
            std::fs::write(dir.join(name), value).unwrap();
        }
    }

    #[test]
    fn qemu_boots_with_bounded_cpu_and_virtio_mem_headroom() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config(dir.path());
        let args = config.arguments(Path::new("/run/qrab/qmp.sock"));
        assert!(args.windows(2).any(|a| a == ["-smp", "cpus=1,maxcpus=4"]));
        // 2047 - 512 rounds up to whole 2 MiB virtio-mem blocks.
        assert!(args
            .windows(2)
            .any(|a| a == ["-m", "size=512M,maxmem=2048M"]));
        let backend = args
            .windows(2)
            .find(|a| a[0] == "-object" && a[1].contains("memory-backend-ram"))
            .unwrap();
        let backend: serde_json::Value = serde_json::from_str(&backend[1]).unwrap();
        assert_eq!(backend["size"], 1536u64 << 20);
        assert!(args.iter().any(|a| a == "virtio-mem-pci,id=hotplug-memory0,memdev=hotplug-memory,requested-size=0,block-size=2M"));
        config.hotplug.as_mut().unwrap().max_memory_mib = 511;
        assert!(config.validate().is_err(), "maximum below the boot size");
        config.hotplug = None;
        let args = config.arguments(Path::new("/run/qrab/qmp.sock"));
        assert!(args.windows(2).any(|a| a == ["-smp", "1"]));
        assert!(args.windows(2).any(|a| a == ["-m", "512"]));
        assert!(!args.iter().any(|a| a.contains("virtio-mem")));
    }

    #[test]
    fn container_and_cgroup_limits_translate_to_guest_sizes() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(dir.path());
        let hotplug = config.hotplug.as_ref().unwrap();
        assert_eq!(desired(hotplug), None);
        write(dir.path(), 3000, 1536, "300000 100000", "1610612736");
        let two = GuestSize {
            cpus: 2,
            memory_mib: 1024,
        };
        assert_eq!(desired(hotplug), Some(two));
        assert_eq!(permitted(hotplug), Some(two));
        write(dir.path(), 500, 256, "max 100000", "max");
        assert_eq!(
            desired(hotplug),
            None,
            "limits below the overhead are invalid"
        );
        assert_eq!(
            permitted(hotplug),
            Some(GuestSize {
                cpus: u32::MAX,
                memory_mib: u64::MAX
            })
        );
    }

    #[test]
    fn retry_delay_doubles_to_a_cap() {
        let delays: Vec<_> = (1..=9).map(retry_delay_ms).collect();
        assert_eq!(
            delays,
            [1000, 2000, 4000, 8000, 16000, 32000, 60000, 60000, 60000]
        );
        assert_eq!(retry_delay_ms(u32::MAX), RETRY_MAX_MS);
    }

    #[tokio::test]
    async fn failing_device_add_backs_off_and_reports_the_last_error() {
        use qapi_protocol::server::QmpSession;
        let dir = tempfile::tempdir().unwrap();
        let config = config(dir.path());
        let socket = dir.path().join("qmp.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let server = tokio::spawn(async move {
            let mut session = QmpSession::accept(&listener).await.unwrap();
            session.send_greeting(11, 1, 3).await.unwrap();
            session.negotiate().await.unwrap();
            let mut device_adds = 0;
            while let Ok((name, _)) = session.recv_command().await {
                match name.as_str() {
                    "query-hotpluggable-cpus" => session
                        .send_return(serde_json::json!([
                            {"props":{"core-id":1},"vcpus-count":1,"type":"max-x86_64-cpu"},
                            {"props":{"core-id":0},"vcpus-count":1,"type":"max-x86_64-cpu","qom-path":"/machine/unattached/device[0]"}
                        ]))
                        .await
                        .unwrap(),
                    "device_add" => {
                        device_adds += 1;
                        session
                            .send_error("GenericError", "CPU hot-add refused")
                            .await
                            .unwrap()
                    }
                    _ => session.send_return(serde_json::json!({})).await.unwrap(),
                }
            }
            device_adds
        });
        let (qmp, _events) = Qmp::new(&socket).await.unwrap();
        let mut resize = Resize::new(&config).unwrap();
        write(dir.path(), 3000, 1024, "max 100000", "max");
        // One heartbeat per simulated second.
        let mut attempts = Vec::new();
        for second in 0..=123u64 {
            let failures = resize.failures;
            reconcile_at(&config, &qmp, &mut resize, second * 1000).await;
            if resize.failures != failures {
                attempts.push(second);
            }
        }
        assert_eq!(attempts, [0, 1, 3, 7, 15, 31, 63, 123]);
        assert_eq!(resize.phase, ResizePhase::Retrying);
        assert_eq!(resize.failures, 8);
        assert!(resize
            .last_error
            .as_deref()
            .unwrap()
            .contains("CPU hot-add refused"));
        assert_eq!(resize.retry_after_unix_ms, Some(123_000 + 60_000));
        assert_eq!(resize.current, resize.boot);
        // New container limits retry at once instead of waiting out the delay.
        write(dir.path(), 4000, 1024, "max 100000", "max");
        reconcile_at(&config, &qmp, &mut resize, 124_000).await;
        assert_eq!(resize.failures, 9);
        qmp.shutdown().await;
        assert_eq!(server.await.unwrap(), 9, "one device_add per attempt");
    }

    #[tokio::test]
    async fn hotplug_waits_for_applied_cgroup_limits_then_adds_vcpus_and_memory() {
        use qapi_protocol::server::QmpSession;
        let dir = tempfile::tempdir().unwrap();
        let config = config(dir.path());
        let socket = dir.path().join("qmp.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let server = tokio::spawn(async move {
            let mut session = QmpSession::accept(&listener).await.unwrap();
            session.send_greeting(11, 1, 3).await.unwrap();
            session.negotiate().await.unwrap();
            let mut seen = Vec::new();
            let mut present = vec![0];
            while let Ok((name, arguments)) = session.recv_command().await {
                let reply = match name.as_str() {
                    // QEMU lists slots in descending order; present ones carry a QOM path.
                    "query-hotpluggable-cpus" => (0..4).rev().map(|core: i64| {
                        let mut value = serde_json::json!({"props":{"core-id":core,"thread-id":0,"node-id":0,"socket-id":0},"vcpus-count":1,"type":"max-x86_64-cpu"});
                        if present.contains(&core) {
                            value["qom-path"] = format!("/machine/peripheral/cpu{core}").into();
                        }
                        value
                    }).collect(),
                    "device_add" => {
                        present.push(arguments["core-id"].as_i64().unwrap());
                        serde_json::json!({})
                    }
                    "qom-get" => serde_json::json!(512u64 << 20),
                    _ => serde_json::json!({}),
                };
                seen.push((name, arguments));
                session.send_return(reply).await.unwrap();
            }
            seen
        });
        let (qmp, _events) = Qmp::new(&socket).await.unwrap();
        let mut resize = Resize::new(&config).unwrap();
        assert_eq!(
            resize.maximum,
            GuestSize {
                cpus: 4,
                memory_mib: 2047
            }
        );
        // The downward API already shows the new limits while the kubelet
        // has deferred the resize: nothing may be plugged yet.
        write(dir.path(), 4000, 1536, "200000 100000", "1073741824");
        assert!(reconcile(&config, &qmp, &mut resize).await);
        assert_eq!(resize.current, resize.boot);
        assert!(resize.detail.as_deref().unwrap().contains("cgroup"));
        assert_eq!(resize.phase, ResizePhase::WaitingForLimits);
        write(dir.path(), 4000, 1536, "400000 100000", "1610612736");
        assert!(reconcile(&config, &qmp, &mut resize).await);
        assert_eq!(
            resize.current,
            GuestSize {
                cpus: 3,
                memory_mib: 1024
            },
            "{:?}",
            resize.detail
        );
        assert_eq!(resize.plugged_memory_mib, Some(512));
        assert_eq!(resize.detail, None);
        write(dir.path(), 9000, 1536, "max 100000", "max");
        reconcile(&config, &qmp, &mut resize).await;
        assert_eq!(resize.current.cpus, 4);
        assert!(resize.detail.as_deref().unwrap().contains("restart"));
        assert_eq!(resize.phase, ResizePhase::RestartRequired);
        qmp.shutdown().await;
        let seen = server.await.unwrap();
        let names: Vec<_> = seen.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names,
            [
                "query-hotpluggable-cpus",
                "device_add",
                "device_add",
                "qom-set",
                "qom-get",
                "query-hotpluggable-cpus",
                "device_add",
                "qom-get"
            ]
        );
        assert_eq!(
            seen[1].1,
            serde_json::json!({"driver":"max-x86_64-cpu","id":"qrab-vcpu1","core-id":1,"thread-id":0,"node-id":0,"socket-id":0})
        );
        assert_eq!(seen[2].1["core-id"], 2);
        assert_eq!(
            seen[3].1,
            serde_json::json!({"path":"/machine/peripheral/hotplug-memory0","property":"requested-size","value":512u64 << 20})
        );
        assert_eq!(seen[6].1["id"], "qrab-vcpu3");
    }
}

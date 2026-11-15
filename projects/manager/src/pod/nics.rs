//! Guest NICs while QEMU runs. The manager re-reads its config file (the VM
//! ConfigMap, which the kubelet refreshes in place) and attaches, detaches
//! and re-attaches tap NICs over QMP on their `nicport<slot>` root ports:
//! qrab-netd's descriptors go to QEMU with `getfd`, then `netdev_add` and
//! `device_add`; removal is `device_del`, the guest's unplug, then
//! `netdev_del`. A NIC whose anchor Pod was replaced (qrab-netd's `probe`
//! names another anchor) is re-plugged with fresh queues.
use super::{config::Nic, hotplug::retry_delay_ms, Config};
use crate::{Error, Result};
use qapi_protocol::Qmp;
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
use tokio::time::Instant;

/// How often attached NICs' anchors are probed.
pub const PROBE_EVERY: Duration = Duration::from_secs(5);
/// How long a guest may take to release an unplugged NIC before the
/// manager asks again.
pub const UNPLUG_PATIENCE: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Attached {
    mac: String,
    slot: u8,
    /// UID of the anchor whose queues QEMU holds, when qrab-netd named it.
    anchor: Option<String>,
}

/// The NICs of one QEMU run.
#[derive(Debug, Default)]
pub(super) struct Nics {
    attached: BTreeMap<String, Attached>,
    /// `device_del` sent, waiting for the guest to release the device.
    removing: BTreeMap<String, Instant>,
    /// Detached because their anchor changed; attach again at once.
    reattach: BTreeSet<String>,
    failures: u32,
    retry_at: Option<Instant>,
    probed_at: Option<Instant>,
    /// The last failure, for status and logs.
    pub detail: Option<String>,
}

impl Nics {
    /// The NICs QEMU booted with, and the anchors their queues came from.
    pub fn booted(config: &Config, anchors: &BTreeMap<String, Option<String>>) -> Self {
        let attached = config
            .slotted_nics()
            .into_iter()
            .filter_map(|nic| {
                let slot = nic.slot?;
                Some((
                    nic.id.clone(),
                    Attached {
                        mac: nic.mac,
                        slot,
                        anchor: anchors.get(&nic.id).cloned().flatten(),
                    },
                ))
            })
            .collect();
        Self {
            attached,
            ..Self::default()
        }
    }

    /// Whether NICs are being added or removed; an outgoing migration waits
    /// for a settled device set.
    pub fn changing(&self) -> bool {
        !self.removing.is_empty() || !self.reattach.is_empty()
    }

    /// `config` with the NICs QEMU has now, slots included: what a migration
    /// target must recreate.
    pub fn live(&self, config: &Config) -> Config {
        let mut live = config.clone();
        if config.nic_slots > 0 {
            live.nics = self
                .attached
                .iter()
                .map(|(id, a)| Nic {
                    id: id.clone(),
                    mac: a.mac.clone(),
                    slot: Some(a.slot),
                })
                .collect();
        }
        live
    }

    fn free_slot(&self, slots: u8) -> Option<u8> {
        let used: BTreeSet<u8> = self.attached.values().map(|a| a.slot).collect();
        (0..slots).find(|s| !used.contains(s))
    }

    fn failed(&mut self, now: Instant, error: String) {
        self.failures = self.failures.saturating_add(1);
        self.retry_at = Some(now + Duration::from_millis(retry_delay_ms(self.failures)));
        eprintln!(
            "{}",
            json!({"event":"nic_change_failed","error":error,"failures":self.failures})
        );
        self.detail = Some(error);
    }

    fn settled(&mut self) {
        self.failures = 0;
        self.retry_at = None;
        self.detail = None;
    }
}

/// The NIC list the config file names now, or why it cannot be read.
fn desired(config: &Config) -> Result<Vec<Nic>> {
    let Some(path) = &config.source else {
        return Ok(config.nics.clone());
    };
    let fresh = Config::read(path)?;
    let mut probe = config.clone();
    probe.nics = fresh.nics;
    probe.validate()?;
    Ok(probe.nics)
}

/// One step towards the config file's NIC list. Each call makes at most one
/// change so the heartbeat stays fresh; failures back off.
pub(super) async fn reconcile(config: &Config, qmp: &Qmp, nics: &mut Nics) {
    if config.nic_slots == 0 {
        return;
    }
    let now = Instant::now();
    if nics.retry_at.is_some_and(|at| now < at) {
        return;
    }
    if let Err(error) = step(config, qmp, nics, now).await {
        nics.failed(now, error.to_string());
    }
}

async fn step(config: &Config, qmp: &Qmp, nics: &mut Nics, now: Instant) -> Result<()> {
    // Unplugs in flight: finish those the guest has released.
    if let Some((id, since)) = nics.removing.iter().next().map(|(i, s)| (i.clone(), *s)) {
        let qemu_id = format!("nic-{id}");
        if device_present(qmp, &qemu_id).await? {
            if now.duration_since(since) > UNPLUG_PATIENCE {
                // The guest may have missed the attention button; ask again.
                device_del(qmp, &qemu_id).await?;
                nics.removing.insert(id, now);
            }
            return Ok(());
        }
        netdev_del(qmp, &qemu_id).await?;
        nics.removing.remove(&id);
        nics.attached.remove(&id);
        eprintln!("{}", json!({"event":"nic_detached","nic":id}));
        nics.settled();
        return Ok(());
    }
    let wanted = desired(config)?;
    let wanted_ids: BTreeSet<&str> = wanted.iter().map(|n| n.id.as_str()).collect();
    // Remove what the config dropped or changed, and what lost its anchor.
    let stale = nics.attached.iter().find_map(|(id, a)| {
        let gone = !wanted_ids.contains(id.as_str());
        let changed = wanted.iter().any(|n| &n.id == id && n.mac != a.mac);
        (gone || changed || nics.reattach.contains(id)).then(|| id.clone())
    });
    if let Some(id) = stale {
        device_del(qmp, &format!("nic-{id}")).await?;
        eprintln!("{}", json!({"event":"nic_detaching","nic":id}));
        nics.reattach.remove(&id);
        nics.removing.insert(id, now);
        return Ok(());
    }
    // Add the first NIC the config names that QEMU lacks.
    if let Some(nic) = wanted.iter().find(|n| !nics.attached.contains_key(&n.id)) {
        let slot = nics
            .free_slot(config.nic_slots)
            .ok_or_else(|| Error::Config(format!("no free NIC root port for {}", nic.id)))?;
        let anchor = attach(config, qmp, nic, slot).await?;
        eprintln!(
            "{}",
            json!({"event":"nic_attached","nic":nic.id,"slot":slot,"anchor":anchor})
        );
        nics.attached.insert(
            nic.id.clone(),
            Attached {
                mac: nic.mac.clone(),
                slot,
                anchor,
            },
        );
        nics.settled();
        return Ok(());
    }
    // Everything matches: check now and then that each anchor is the one
    // QEMU's queues came from.
    if nics
        .probed_at
        .is_some_and(|at| now.duration_since(at) < PROBE_EVERY)
    {
        return Ok(());
    }
    nics.probed_at = Some(now);
    for (id, attached) in nics.attached.iter_mut() {
        let socket = config.netd_socket.clone();
        let nic = id.clone();
        let Ok(Ok(serving)) =
            tokio::task::spawn_blocking(move || qrab_netd::wire::probe(&socket, &nic)).await
        else {
            // Not plumbed yet (a replacement anchor) or an older qrab-netd.
            continue;
        };
        match &attached.anchor {
            None => attached.anchor = Some(serving),
            Some(held) if *held != serving => {
                eprintln!(
                    "{}",
                    json!({"event":"nic_anchor_replaced","nic":id,"held":held,"serving":serving})
                );
                nics.reattach.insert(id.clone());
            }
            Some(_) => {}
        }
    }
    nics.settled();
    Ok(())
}

/// Hand `nic`'s queues to QEMU and plug it into `nicport<slot>`. Returns the
/// anchor UID the queues came from.
async fn attach(config: &Config, qmp: &Qmp, nic: &Nic, slot: u8) -> Result<Option<String>> {
    let socket = config.netd_socket.clone();
    let id = nic.id.clone();
    let queues = config.tap_queues();
    let grant = tokio::task::spawn_blocking(move || qrab_netd::wire::request(&socket, &id, queues))
        .await
        .map_err(|e| Error::Spawn(e.to_string()))?
        .map_err(|e| Error::Spawn(format!("tap NIC {}: {e}", nic.id)))?;
    let qemu_id = nic.qemu_id();
    let anchor = grant.anchor.clone();
    let mut names = Vec::new();
    let result = async {
        let mut taps = Vec::new();
        let mut vhosts = Vec::new();
        for (kind, list, fds) in [("t", &mut taps, grant.taps), ("v", &mut vhosts, grant.vhosts)] {
            for (q, fd) in fds.into_iter().enumerate() {
                let name = format!("{qemu_id}-{kind}{q}");
                qmp.getfd(&name, fd).await.map_err(Error::Protocol)?;
                names.push(name.clone());
                list.push(name);
            }
        }
        qmp.execute_raw(json!({"execute":"netdev_add","arguments":{
            "type":"tap","id":qemu_id,"fds":taps.join(":"),"vhost":true,"vhostfds":vhosts.join(":")}}))
            .await
            .map_err(Error::Protocol)?;
        let mut device = json!({"driver":"virtio-net-pci","id":qemu_id,"netdev":qemu_id,
            "mac":nic.mac,"romfile":"","bus":Nic::port(slot)});
        if taps.len() > 1 {
            device["mq"] = json!(true);
            device["vectors"] = json!(2 * taps.len() + 2);
        }
        if let Err(error) = qmp
            .execute_raw(json!({"execute":"device_add","arguments":device}))
            .await
        {
            let _ = netdev_del(qmp, &qemu_id).await;
            return Err(Error::Protocol(error));
        }
        Ok(())
    }
    .await;
    // netdev_add took its own references; drop the named copies either way.
    for name in names {
        let _ = qmp
            .execute_raw(json!({"execute":"closefd","arguments":{"fdname":name}}))
            .await;
    }
    result.map(|()| anchor)
}

async fn device_present(qmp: &Qmp, id: &str) -> Result<bool> {
    let list = qmp
        .execute_raw(json!({"execute":"qom-list","arguments":{"path":"/machine/peripheral"}}))
        .await
        .map_err(Error::Protocol)?;
    Ok(list
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item["name"] == id)))
}

async fn device_del(qmp: &Qmp, id: &str) -> Result<()> {
    match qmp
        .execute_raw(json!({"execute":"device_del","arguments":{"id":id}}))
        .await
    {
        Ok(_) => Ok(()),
        // A second request while the unplug is pending, or already gone.
        Err(qapi_protocol::Error::Qmp(error))
            if error.class == "DeviceNotFound" || error.desc.contains("process of unplug") =>
        {
            Ok(())
        }
        Err(error) => Err(Error::Protocol(error)),
    }
}

async fn netdev_del(qmp: &Qmp, id: &str) -> Result<()> {
    match qmp
        .execute_raw(json!({"execute":"netdev_del","arguments":{"id":id}}))
        .await
    {
        Ok(_) => Ok(()),
        Err(qapi_protocol::Error::Qmp(error))
            if error.class == "DeviceNotFound" || error.desc.contains("not found") =>
        {
            Ok(())
        }
        Err(error) => Err(Error::Protocol(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qapi_protocol::server::QmpSession;
    use serde_json::Value;
    use std::{
        os::unix::net::UnixListener,
        path::Path,
        sync::{Arc, Mutex},
    };

    fn write_config(path: &Path, dir: &Path, nics: Value) {
        std::fs::write(
            path,
            serde_json::to_vec(&json!({
                "name":"nic-test","qemu_binary":"/bin/sh","kernel":dir.join("kernel"),
                "runtime_dir":dir.join("run"),"cpus":1,"nic_slots":4,"nics":nics,
                "netd_socket":dir.join("netd.sock")}))
            .unwrap(),
        )
        .unwrap();
    }

    /// qrab-netd's side: grants and probes name the anchor `serving` holds.
    fn fake_netd(socket: &Path, serving: Arc<Mutex<String>>) {
        let listener = UnixListener::bind(socket).unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                let request = qrab_netd::wire::read_request(&stream).unwrap();
                let anchor = serving.lock().unwrap().clone();
                match request.op {
                    qrab_netd::wire::Op::Probe => {
                        qrab_netd::wire::send_probe(&stream, &anchor).unwrap()
                    }
                    _ => {
                        let file =
                            || -> std::os::fd::OwnedFd { tempfile::tempfile().unwrap().into() };
                        let taps: Vec<_> = (0..request.queues).map(|_| file()).collect();
                        let vhosts: Vec<_> = (0..request.queues).map(|_| file()).collect();
                        qrab_netd::wire::send_grant(
                            &stream,
                            "qt000000000000",
                            &anchor,
                            &taps,
                            &vhosts,
                        )
                        .unwrap();
                    }
                }
            }
        });
    }

    /// QEMU's side: records every command, keeps `/machine/peripheral`, and
    /// lets the guest release an unplugged device at its next `qom-list`.
    fn fake_qmp(socket: &Path, log: Arc<Mutex<Vec<(String, Value)>>>) {
        let listener = tokio::net::UnixListener::bind(socket).unwrap();
        tokio::spawn(async move {
            let mut session = QmpSession::accept(&listener).await.unwrap();
            session.send_greeting(11, 1, 3).await.unwrap();
            session.negotiate().await.unwrap();
            let mut devices: BTreeSet<String> = ["nic-lan".to_owned()].into();
            let mut unplugging = BTreeSet::new();
            while let Ok((name, args)) = session.recv_command().await {
                log.lock().unwrap().push((name.clone(), args.clone()));
                let reply = match name.as_str() {
                    "device_add" => {
                        devices.insert(args["id"].as_str().unwrap().to_owned());
                        json!({})
                    }
                    "device_del" => {
                        unplugging.insert(args["id"].as_str().unwrap().to_owned());
                        json!({})
                    }
                    "qom-list" => {
                        for id in std::mem::take(&mut unplugging) {
                            devices.remove(&id);
                        }
                        json!(devices
                            .iter()
                            .map(|d| json!({"name":d,"type":"child<virtio-net-pci>"}))
                            .collect::<Vec<_>>())
                    }
                    _ => json!({}),
                };
                session.send_return(reply).await.unwrap();
            }
        });
    }

    fn names(log: &Arc<Mutex<Vec<(String, Value)>>>) -> Vec<String> {
        std::mem::take(&mut *log.lock().unwrap())
            .into_iter()
            .map(|(name, args)| match name.as_str() {
                "getfd" | "closefd" => format!("{name} {}", args["fdname"].as_str().unwrap()),
                "netdev_add" => format!(
                    "netdev_add {} fds={} vhostfds={}",
                    args["id"].as_str().unwrap(),
                    args["fds"].as_str().unwrap(),
                    args["vhostfds"].as_str().unwrap()
                ),
                "device_add" => format!(
                    "device_add {} bus={} mac={}",
                    args["id"].as_str().unwrap(),
                    args["bus"].as_str().unwrap(),
                    args["mac"].as_str().unwrap()
                ),
                "device_del" | "netdev_del" => format!("{name} {}", args["id"].as_str().unwrap()),
                _ => name,
            })
            .collect()
    }

    #[test]
    fn nics_sit_on_their_slots_root_ports() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("kernel"), []).unwrap();
        let path = dir.path().join("vm.json");
        write_config(
            &path,
            dir.path(),
            json!([{"id":"lan","mac":"02:00:00:00:00:01"},{"id":"wan","mac":"02:00:00:00:00:03","slot":0}]),
        );
        let config = Config::read(&path).unwrap();
        config.validate().unwrap();
        // An explicit slot is kept; the rest take the lowest free ones.
        let slots: Vec<_> = config
            .slotted_nics()
            .iter()
            .map(|n| (n.id.clone(), n.slot))
            .collect();
        assert_eq!(
            slots,
            [("lan".to_owned(), Some(1)), ("wan".to_owned(), Some(0))]
        );
        let args = config.arguments(Path::new("/run/qrab/qmp.sock"));
        for slot in 0..4 {
            assert!(args.contains(&format!(
                "pcie-root-port,id=nicport{slot},bus=pcie.0,chassis={}",
                16 + slot
            )));
        }
        assert!(args
            .iter()
            .any(|a| a.starts_with("virtio-net-pci,id=nic-lan,") && a.ends_with(",bus=nicport1")));
        assert!(args
            .iter()
            .any(|a| a.starts_with("virtio-net-pci,id=nic-wan,") && a.ends_with(",bus=nicport0")));
        // A duplicate slot, a slot past the ports, or slots without ports are refused.
        for (slots, nics) in [
            (
                4,
                json!([{"id":"lan","mac":"02:00:00:00:00:01","slot":2},{"id":"wan","mac":"02:00:00:00:00:03","slot":2}]),
            ),
            (4, json!([{"id":"lan","mac":"02:00:00:00:00:01","slot":4}])),
            (0, json!([{"id":"lan","mac":"02:00:00:00:00:01","slot":0}])),
        ] {
            let mut bad = config.clone();
            bad.nic_slots = slots;
            bad.nics = serde_json::from_value(nics).unwrap();
            let error = bad.validate().unwrap_err().to_string();
            assert!(error.contains("slot"), "{:?}: {error}", bad.nics);
        }
        // Without ports the legacy layout is unchanged.
        let mut legacy = config.clone();
        legacy.nic_slots = 0;
        legacy.nics.iter_mut().for_each(|n| n.slot = None);
        let args = legacy.arguments(Path::new("/run/qrab/qmp.sock"));
        assert!(!args
            .iter()
            .any(|a| a.contains("pcie-root-port") || a.contains("bus=nicport")));
    }

    #[tokio::test]
    async fn nics_follow_the_config_file_and_their_anchors() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("kernel"), []).unwrap();
        let path = dir.path().join("vm.json");
        let lan = json!({"id":"lan","mac":"02:00:00:00:00:01"});
        let wan = json!({"id":"wan","mac":"02:00:00:00:00:03"});
        write_config(&path, dir.path(), json!([lan]));
        let config = Config::read(&path).unwrap();
        let serving = Arc::new(Mutex::new("anchor-1".to_owned()));
        fake_netd(&config.netd_socket, serving.clone());
        let log = Arc::new(Mutex::new(Vec::new()));
        let socket = dir.path().join("qmp.sock");
        fake_qmp(&socket, log.clone());
        let (qmp, _events) = Qmp::new(&socket).await.unwrap();
        let mut nics = Nics::booted(
            &config,
            &[("lan".to_owned(), Some("anchor-1".to_owned()))].into(),
        );

        // Unchanged config: only the anchor probe.
        reconcile(&config, &qmp, &mut nics).await;
        assert!(names(&log).is_empty(), "probes go to qrab-netd, not QEMU");

        // A second NIC appears in the ConfigMap: plugged into the next free port.
        write_config(&path, dir.path(), json!([lan, wan]));
        reconcile(&config, &qmp, &mut nics).await;
        assert_eq!(nics.detail, None);
        assert_eq!(
            names(&log),
            [
                "getfd nic-wan-t0",
                "getfd nic-wan-v0",
                "netdev_add nic-wan fds=nic-wan-t0 vhostfds=nic-wan-v0",
                "device_add nic-wan bus=nicport1 mac=02:00:00:00:00:03",
                "closefd nic-wan-t0",
                "closefd nic-wan-v0",
            ]
        );
        // A migration target would recreate both, on the same ports.
        let live: Vec<_> = nics
            .live(&config)
            .nics
            .iter()
            .map(|n| (n.id.clone(), n.slot))
            .collect();
        assert_eq!(
            live,
            [("lan".to_owned(), Some(0)), ("wan".to_owned(), Some(1))]
        );

        // Removed again: unplug, wait for the guest, then drop the backend.
        write_config(&path, dir.path(), json!([lan]));
        reconcile(&config, &qmp, &mut nics).await;
        assert!(nics.changing());
        reconcile(&config, &qmp, &mut nics).await;
        assert!(!nics.changing());
        assert_eq!(
            names(&log),
            ["device_del nic-wan", "qom-list", "netdev_del nic-wan"]
        );

        // lan's anchor is replaced: re-plugged with fresh queues from the new one.
        *serving.lock().unwrap() = "anchor-2".to_owned();
        nics.probed_at = None;
        reconcile(&config, &qmp, &mut nics).await;
        assert!(nics.changing());
        for _ in 0..3 {
            reconcile(&config, &qmp, &mut nics).await;
        }
        assert_eq!(
            names(&log),
            [
                "device_del nic-lan",
                "qom-list",
                "netdev_del nic-lan",
                "getfd nic-lan-t0",
                "getfd nic-lan-v0",
                "netdev_add nic-lan fds=nic-lan-t0 vhostfds=nic-lan-v0",
                "device_add nic-lan bus=nicport0 mac=02:00:00:00:00:01",
                "closefd nic-lan-t0",
                "closefd nic-lan-v0",
            ]
        );
        assert_eq!(nics.attached["lan"].anchor.as_deref(), Some("anchor-2"));
        assert!(!nics.changing());
        qmp.shutdown().await;
    }
}

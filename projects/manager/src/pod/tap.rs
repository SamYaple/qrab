//! Tap datapath launch support. Before each QEMU start the manager asks
//! qrab-netd for every guest NIC's queues and places the received descriptors
//! at the numbers [`Config::tap_fds`] renders into `-netdev tap,fds=…`. The
//! manager never opens `/dev/net/tun` or `/dev/vhost-net` itself.
use super::config::{Config, TapFds};
use crate::{Error, Result};
use command_fds::{CommandFdExt, FdMapping};
use std::{
    os::fd::{OwnedFd, RawFd},
    path::Path,
};

/// Descriptors for one QEMU start, each with the number QEMU expects it at.
#[derive(Debug, Default)]
pub(super) struct Inherited(Vec<(RawFd, OwnedFd)>);

/// One attempt at every guest NIC's descriptors (no-op without NICs).
/// The caller retries a refusal: the VM Pod starts before qrab-netd has
/// plumbed its anchors, which are created once the Pod is scheduled.
pub(super) async fn fetch(config: &Config) -> Result<Inherited> {
    let layout = config.tap_fds();
    if layout.is_empty() {
        return Ok(Inherited::default());
    }
    let socket = config.netd_socket.clone();
    let queues = config.tap_queues();
    tokio::task::spawn_blocking(move || fetch_blocking(&socket, &layout, queues))
        .await
        .map_err(|e| Error::Spawn(e.to_string()))?
}

/// How long a QEMU start waits for qrab-netd, within the VM Pod's 90 s
/// startup probe.
pub(super) const PATIENCE: std::time::Duration = std::time::Duration::from_secs(60);
pub(super) const RETRY: std::time::Duration = std::time::Duration::from_secs(2);

fn fetch_blocking(socket: &Path, layout: &[TapFds], queues: u8) -> Result<Inherited> {
    let mut held = Vec::new();
    for nic in layout {
        let grant = qrab_netd::wire::request(socket, &nic.nic, queues)
            .map_err(|e| Error::Spawn(format!("tap NIC {}: {e}", nic.nic)))?;
        if grant.taps.len() != nic.taps.len() || grant.vhosts.len() != nic.vhosts.len() {
            return Err(Error::Spawn(format!(
                "tap NIC {}: qrab-netd sent {} tap and {} vhost descriptors, {} each were requested",
                nic.nic,
                grant.taps.len(),
                grant.vhosts.len(),
                queues
            )));
        }
        let pairs = nic
            .taps
            .iter()
            .zip(grant.taps)
            .chain(nic.vhosts.iter().zip(grant.vhosts));
        held.extend(pairs.map(|(target, fd)| (*target, fd)));
    }
    Ok(Inherited(held))
}

impl Inherited {
    /// Hand each descriptor to the child at its number. The command owns
    /// them from here; dropping it after the spawn closes the manager's
    /// copies.
    pub(super) fn install(self, command: &mut tokio::process::Command) -> Result<()> {
        if self.0.is_empty() {
            return Ok(());
        }
        let mappings = self
            .0
            .into_iter()
            .map(|(child_fd, parent_fd)| FdMapping {
                parent_fd,
                child_fd,
            })
            .collect();
        command
            .fd_mappings(mappings)
            .map(|_| ())
            .map_err(|e| Error::Spawn(format!("tap descriptor layout: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, os::unix::net::UnixListener};

    fn config(dir: &Path, nics: serde_json::Value, cpus: u32) -> Config {
        serde_json::from_value(serde_json::json!({
            "name": "tap-test", "qemu_binary": "/bin/sh", "kernel": dir.join("kernel"),
            "runtime_dir": dir.join("run"), "cpus": cpus, "nics": nics,
            "netd_socket": dir.join("netd.sock"),
        }))
        .unwrap()
    }

    /// A stand-in for qrab-netd: answers each request with files whose
    /// contents name the NIC, kind and queue.
    fn fake_netd(
        socket: &Path,
        requests: usize,
        dir: &Path,
    ) -> std::thread::JoinHandle<Vec<qrab_netd::wire::Request>> {
        let listener = UnixListener::bind(socket).unwrap();
        let dir = dir.to_owned();
        std::thread::spawn(move || {
            (0..requests)
                .map(|_| {
                    let (stream, _) = listener.accept().unwrap();
                    let request = qrab_netd::wire::read_request(&stream).unwrap();
                    let file = |kind: &str, q: u8| -> OwnedFd {
                        let path = dir.join(format!("{}-{kind}{q}", request.nic));
                        std::fs::File::create(&path)
                            .unwrap()
                            .write_all(path.file_name().unwrap().as_encoded_bytes())
                            .unwrap();
                        std::fs::File::open(path).unwrap().into()
                    };
                    let taps: Vec<_> = (0..request.queues).map(|q| file("tap", q)).collect();
                    let vhosts: Vec<_> = (0..request.queues).map(|q| file("vhost", q)).collect();
                    qrab_netd::wire::send_grant(&stream, "qt000000000000", &taps, &vhosts).unwrap();
                    request
                })
                .collect()
        })
    }

    #[tokio::test]
    async fn qemu_inherits_each_tap_nics_descriptors_at_the_rendered_numbers() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("kernel"), []).unwrap();
        let config = config(
            dir.path(),
            serde_json::json!([
                {"id": "lan", "mac": "02:00:00:00:00:01"},
                {"id": "wan", "mac": "02:00:00:00:00:03"},
            ]),
            2,
        );
        let netd = fake_netd(&config.netd_socket, 2, dir.path());
        let inherited = fetch(&config).await.unwrap();
        let requests = netd.join().unwrap();
        assert_eq!(
            requests,
            [("lan", 2), ("wan", 2)].map(|(nic, queues)| qrab_netd::wire::Request {
                nic: nic.into(),
                queues,
                op: qrab_netd::wire::Op::Fds,
                via: None,
            })
        );
        // The child reads every inherited descriptor by the numbers the
        // QEMU arguments name.
        let layout = config.tap_fds();
        let numbers: Vec<String> = layout
            .iter()
            .flat_map(|l| l.taps.iter().chain(&l.vhosts))
            .map(i32::to_string)
            .collect();
        assert_eq!(numbers, ["3", "4", "5", "6", "7", "8", "9", "10"]);
        let script = numbers
            .iter()
            .map(|n| format!("cat <&{n}; echo"))
            .collect::<Vec<_>>()
            .join("; ");
        let mut command = tokio::process::Command::new("/bin/sh");
        command.args(["-c", &script]);
        inherited.install(&mut command).unwrap();
        let output = command.output().await.unwrap();
        assert_eq!(
            String::from_utf8(output.stdout)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            [
                "lan-tap0",
                "lan-tap1",
                "lan-vhost0",
                "lan-vhost1",
                "wan-tap0",
                "wan-tap1",
                "wan-vhost0",
                "wan-vhost1"
            ]
        );
        let args = config.arguments(Path::new("/run/qrab/qmp.sock"));
        assert!(args.contains(&"tap,id=nic-lan,fds=3:4,vhost=on,vhostfds=5:6".to_owned()));
        assert!(args.contains(&"tap,id=nic-wan,fds=7:8,vhost=on,vhostfds=9:10".to_owned()));
    }

    #[tokio::test]
    async fn a_refusal_fails_the_launch_with_its_reason() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("kernel"), []).unwrap();
        let config = config(
            dir.path(),
            serde_json::json!([{"id": "lan", "mac": "02:00:00:00:00:01"}]),
            1,
        );
        let listener = UnixListener::bind(&config.netd_socket).unwrap();
        let refuse = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            qrab_netd::wire::read_request(&stream).unwrap();
            qrab_netd::wire::send_refusal(&stream, "NIC \"lan\" is not plumbed yet").unwrap();
        });
        let error = fetch(&config).await.unwrap_err().to_string();
        refuse.join().unwrap();
        assert!(
            error.contains("tap NIC lan") && error.contains("not plumbed yet"),
            "{error}"
        );
        // Without NICs nothing is requested.
        let no_nics = super::super::config::Config {
            nics: Vec::new(),
            ..config
        };
        assert!(fetch(&no_nics).await.unwrap().0.is_empty());
    }

    #[tokio::test]
    async fn a_refused_fetch_succeeds_once_qrab_netd_has_plumbed_the_nic() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("kernel"), []).unwrap();
        let config = config(
            dir.path(),
            serde_json::json!([{"id": "lan", "mac": "02:00:00:00:00:01"}]),
            1,
        );
        let listener = UnixListener::bind(&config.netd_socket).unwrap();
        let files = dir.path().to_owned();
        let netd = std::thread::spawn(move || {
            let (first, _) = listener.accept().unwrap();
            qrab_netd::wire::read_request(&first).unwrap();
            qrab_netd::wire::send_refusal(&first, "NIC \"lan\" is not plumbed yet").unwrap();
            let (second, _) = listener.accept().unwrap();
            qrab_netd::wire::read_request(&second).unwrap();
            let file =
                |name: &str| -> OwnedFd { std::fs::File::create(files.join(name)).unwrap().into() };
            qrab_netd::wire::send_grant(
                &second,
                "qt000000000000",
                &[file("tap")],
                &[file("vhost")],
            )
            .unwrap();
        });
        // The supervisor's retry loop, compressed: a refusal, then a grant.
        assert!(fetch(&config)
            .await
            .unwrap_err()
            .to_string()
            .contains("not plumbed yet"));
        let inherited = fetch(&config).await.unwrap();
        netd.join().unwrap();
        assert_eq!(
            inherited.0.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
            [3, 4]
        );
    }
}

//! The guest side of a NIC Pod. passt gives one guest an L2 interface built
//! from this Pod's own sockets; the relay splices one TCP connection at a time
//! from the VM's QEMU (`-netdev stream`) to passt's Unix socket. Both ends use
//! the same framing (a 4-byte big-endian length, then an Ethernet frame), so
//! the relay copies bytes and never parses them.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{
    future::Future,
    net::SocketAddr,
    os::unix::fs::FileTypeExt,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::net::{TcpListener, TcpStream, UnixStream};

/// The relay's TCP port in NIC Pods.
pub const RELAY_PORT: u16 = 7700;
pub const PASST: &str = "/usr/bin/passt";
/// The gateway passt presents to the guest, the same on every node, so a
/// static network-config needs only the NIC Pod IP. passt answers ARP for
/// it and forwards DNS sent to it to the Pod's resolver.
pub const GUEST_GATEWAY: &str = "169.254.1.1";
const STARTUP: Duration = Duration::from_secs(10);
/// passt drops a closed client on its next event; a client that connects
/// before then is discarded, so the relay pauses briefly between sessions.
const SESSION_GAP: Duration = Duration::from_millis(100);

/// `qrab-manager nic` arguments.
#[derive(Debug, Clone)]
pub struct Options {
    pub listen: SocketAddr,
    /// passt's Unix socket. Its directory also holds `passt.pid` and `ready`.
    pub socket: PathBuf,
    pub passt: PathBuf,
}

impl Options {
    pub fn pid_file(&self) -> PathBuf {
        self.socket.with_file_name("passt.pid")
    }

    pub fn ready_file(&self) -> PathBuf {
        ready_file(&self.socket)
    }

    /// passt in the foreground, logging warnings to stderr, without its own
    /// namespaces: the restricted Pod cannot create them, and the Pod is the
    /// sandbox. Every TCP and UDP port except the relay's (and the kernel's
    /// ephemeral range, which passt always skips for exclusions) is forwarded
    /// to the guest, so the Pod IP acts as the guest's address. The guest's
    /// gateway is the fixed GUEST_GATEWAY, which also forwards DNS, and is
    /// never mapped to the Pod's loopback, where the relay listens.
    pub fn passt_arguments(&self) -> Vec<String> {
        let except = format!("~{}", self.listen.port());
        [
            "-f",
            "-q",
            "--no-ns-isolation",
            "--socket",
            &self.socket.to_string_lossy(),
            "-P",
            &self.pid_file().to_string_lossy(),
            "-t",
            &except,
            "-u",
            &except,
            "-g",
            GUEST_GATEWAY,
            "--no-map-gw",
            "--dns-forward",
            GUEST_GATEWAY,
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }
}

fn ready_file(socket: &Path) -> PathBuf {
    socket.with_file_name("ready")
}

/// Written once passt serves its socket and the relay listener is bound.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ready {
    pub relay_pid: u32,
    pub passt_pid: u32,
    pub listen: SocketAddr,
}

fn log(value: serde_json::Value) {
    eprintln!("{value}");
}

fn remove_stale(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

/// Start passt, then relay until shutdown resolves. Returns an error when
/// passt exits, so the container fails and Kubernetes restarts it.
pub async fn serve(options: &Options, shutdown: impl Future<Output = ()>) -> Result<()> {
    let ready = options.ready_file();
    for path in [&ready, &options.pid_file(), &options.socket] {
        remove_stale(path)?;
    }
    let listener = TcpListener::bind(options.listen).await?;
    let mut passt = tokio::process::Command::new(&options.passt)
        .args(options.passt_arguments())
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| Error::Spawn(format!("start {}: {e}", options.passt.display())))?;
    let passt_pid = passt
        .id()
        .ok_or_else(|| Error::Spawn("passt exited at once".into()))?;
    let deadline = tokio::time::Instant::now() + STARTUP;
    loop {
        if let Some(status) = passt.try_wait()? {
            return Err(Error::Spawn(format!(
                "passt exited during startup: {status}"
            )));
        }
        let pid = std::fs::read_to_string(options.pid_file()).unwrap_or_default();
        let socket = std::fs::metadata(&options.socket).is_ok_and(|m| m.file_type().is_socket());
        if socket && pid.trim().parse() == Ok(passt_pid) {
            break;
        }
        if tokio::time::Instant::now() > deadline {
            return Err(Error::Spawn("passt did not open its socket in time".into()));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let record = Ready {
        relay_pid: std::process::id(),
        passt_pid,
        listen: listener.local_addr()?,
    };
    let partial = ready.with_extension("tmp");
    std::fs::write(&partial, serde_json::to_vec(&record)?)?;
    std::fs::rename(&partial, &ready)?;
    log(serde_json::json!({"event":"nic_ready","passt_pid":passt_pid,"listen":record.listen}));
    let outcome = tokio::select! {
        status = passt.wait() => Err(Error::Spawn(format!("passt exited: {}", status?))),
        result = relay(listener, &options.socket) => result,
        () = shutdown => Ok(()),
    };
    let _ = std::fs::remove_file(&ready);
    let _ = passt.start_kill();
    let _ = passt.wait().await;
    outcome
}

/// TCP keepalive bounds how long a vanished VM Pod (no FIN) holds the slot.
fn configure(stream: &TcpStream) -> Result<()> {
    use nix::sys::socket::{setsockopt, sockopt};
    stream.set_nodelay(true)?;
    let io = |e: nix::Error| Error::Io(e.into());
    setsockopt(stream, sockopt::KeepAlive, &true).map_err(io)?;
    setsockopt(stream, sockopt::TcpKeepIdle, &5).map_err(io)?;
    setsockopt(stream, sockopt::TcpKeepInterval, &2).map_err(io)?;
    setsockopt(stream, sockopt::TcpKeepCount, &3).map_err(io)?;
    Ok(())
}

/// Serve one client at a time: splice it to a fresh connection to `socket`
/// until either side closes. Connections arriving meanwhile are closed at
/// once, as passt itself does; QEMU's `reconnect-ms` retries. Only a failure
/// to reach passt ends the relay.
pub async fn relay(listener: TcpListener, socket: &Path) -> Result<()> {
    loop {
        let (mut client, peer) = match listener.accept().await {
            Ok(accepted) => accepted,
            Err(error) => {
                log(serde_json::json!({"event":"nic_accept_failed","error":error.to_string()}));
                tokio::time::sleep(SESSION_GAP).await;
                continue;
            }
        };
        if let Err(error) = configure(&client) {
            log(
                serde_json::json!({"event":"nic_client_rejected","peer":peer,"error":error.to_string()}),
            );
            continue;
        }
        let mut backend = UnixStream::connect(socket).await?;
        log(serde_json::json!({"event":"nic_client_connected","peer":peer}));
        let outcome = {
            let session = tokio::io::copy_bidirectional(&mut client, &mut backend);
            tokio::pin!(session);
            loop {
                tokio::select! {
                    outcome = &mut session => break outcome,
                    extra = listener.accept() => {
                        if let Ok((extra, other)) = extra {
                            drop(extra);
                            log(serde_json::json!({"event":"nic_client_refused","peer":other,"active":peer}));
                        }
                    }
                }
            }
        };
        match outcome {
            Ok((to_passt, from_passt)) => log(
                serde_json::json!({"event":"nic_client_closed","peer":peer,"bytes_to_passt":to_passt,"bytes_from_passt":from_passt}),
            ),
            Err(error) => log(
                serde_json::json!({"event":"nic_client_closed","peer":peer,"error":error.to_string()}),
            ),
        }
        drop((client, backend));
        tokio::time::sleep(SESSION_GAP).await;
    }
}

fn process_name(pid: u32) -> Option<String> {
    Some(
        std::fs::read_to_string(format!("/proc/{pid}/comm"))
            .ok()?
            .trim_end()
            .to_owned(),
    )
}

/// Readiness of a running `serve`: its record exists, the relay and passt
/// processes are alive and passt's socket is present. It never connects to
/// passt or to the relay, which would take the single client slot.
pub fn probe(socket: &Path) -> Result<Ready> {
    let ready: Ready = serde_json::from_slice(&std::fs::read(ready_file(socket))?)?;
    if !process_name(ready.passt_pid).is_some_and(|name| name.starts_with("passt")) {
        return Err(Error::Config("passt is not running".into()));
    }
    if process_name(ready.relay_pid).is_none() {
        return Err(Error::Config("the relay is not running".into()));
    }
    if !std::fs::metadata(socket).is_ok_and(|m| m.file_type().is_socket()) {
        return Err(Error::Config("passt's socket is missing".into()));
    }
    Ok(ready)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixListener;

    /// A passt stand-in that echoes each connection and, like passt, serves
    /// one client at a time.
    async fn echo_backend(path: PathBuf) {
        let listener = UnixListener::bind(path).unwrap();
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let (mut reader, mut writer) = stream.split();
            let _ = tokio::io::copy(&mut reader, &mut writer).await;
        }
    }

    async fn roundtrip(stream: &mut TcpStream, frame: &[u8]) -> Vec<u8> {
        stream.write_all(frame).await.unwrap();
        let mut echoed = vec![0; frame.len()];
        tokio::time::timeout(Duration::from_secs(5), stream.read_exact(&mut echoed))
            .await
            .unwrap()
            .unwrap();
        echoed
    }

    fn frame(payload: &[u8]) -> Vec<u8> {
        let mut frame = (payload.len() as u32).to_be_bytes().to_vec();
        frame.extend_from_slice(payload);
        frame
    }

    #[tokio::test]
    async fn relay_splices_one_client_at_a_time_and_resplices_after_it_closes() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("passt.sock");
        tokio::spawn(echo_backend(socket.clone()));
        while !socket.exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let relay = tokio::spawn({
            let socket = socket.clone();
            async move { relay(listener, &socket).await }
        });

        let mut first = TcpStream::connect(address).await.unwrap();
        let hello = frame(b"\xff\xff\xff\xff\xff\xff first frame");
        assert_eq!(roundtrip(&mut first, &hello).await, hello);

        // A second client is closed while the first is active.
        let mut second = TcpStream::connect(address).await.unwrap();
        let _ = second.write_all(&frame(b"ignored")).await;
        let mut buffer = [0; 16];
        let read = tokio::time::timeout(Duration::from_secs(5), second.read(&mut buffer))
            .await
            .unwrap();
        assert!(
            matches!(read, Ok(0) | Err(_)),
            "second client was served: {read:?}"
        );
        let again = frame(b"still the first");
        assert_eq!(roundtrip(&mut first, &again).await, again);

        // After the first closes, a reconnecting client gets a fresh splice.
        drop(first);
        let mut third = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let mut stream = TcpStream::connect(address).await.unwrap();
                let probe = frame(b"third");
                if stream.write_all(&probe).await.is_ok() {
                    let mut echoed = vec![0; probe.len()];
                    if matches!(
                        tokio::time::timeout(
                            Duration::from_millis(500),
                            stream.read_exact(&mut echoed)
                        )
                        .await,
                        Ok(Ok(_))
                    ) && echoed == probe
                    {
                        return stream;
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap();
        let last = frame(&[0x5a; 1500]);
        assert_eq!(roundtrip(&mut third, &last).await, last);
        assert!(!relay.is_finished());
        relay.abort();
    }

    #[tokio::test]
    async fn relay_fails_when_passt_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let socket = dir.path().join("missing.sock");
        let relay = tokio::spawn(async move { relay(listener, &socket).await });
        let _client = TcpStream::connect(address).await.unwrap();
        let result = tokio::time::timeout(Duration::from_secs(5), relay)
            .await
            .unwrap()
            .unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn passt_runs_in_the_foreground_without_namespaces_and_forwards_all_but_the_relay() {
        let options = Options {
            listen: "0.0.0.0:7700".parse().unwrap(),
            socket: "/run/nic/passt.sock".into(),
            passt: PASST.into(),
        };
        assert_eq!(
            options.passt_arguments(),
            [
                "-f",
                "-q",
                "--no-ns-isolation",
                "--socket",
                "/run/nic/passt.sock",
                "-P",
                "/run/nic/passt.pid",
                "-t",
                "~7700",
                "-u",
                "~7700",
                "-g",
                "169.254.1.1",
                "--no-map-gw",
                "--dns-forward",
                "169.254.1.1"
            ]
        );
        assert_eq!(options.ready_file(), Path::new("/run/nic/ready"));
    }

    #[tokio::test]
    async fn serve_reports_ready_and_exits_with_an_error_when_passt_dies() {
        let dir = tempfile::tempdir().unwrap();
        // A fake passt: write the pidfile, then exit 3 when asked. The test
        // binds the socket for it once the pidfile appears.
        let fake = dir.path().join("passt");
        std::fs::write(
            &fake,
            "#!/bin/sh\nset -e\nwhile [ \"$1\" != -P ]; do shift; done\necho $$ > \"$2\"\n\
             while [ ! -e \"$(dirname \"$2\")/stop\" ]; do sleep 0.02; done\nexit 3\n",
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap();
        let options = Options {
            listen: "127.0.0.1:0".parse().unwrap(),
            socket: dir.path().join("passt.sock"),
            passt: fake,
        };
        std::fs::write(options.ready_file(), b"stale").unwrap();
        assert!(probe(&options.socket).is_err());
        let serving = tokio::spawn({
            let options = options.clone();
            async move { serve(&options, std::future::pending()).await }
        });
        while !options.pid_file().exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let _backend = UnixListener::bind(&options.socket).unwrap();
        let ready = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Ok(ready) = probe(&options.socket) {
                    return ready;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(ready.relay_pid, std::process::id());
        std::fs::write(dir.path().join("stop"), b"").unwrap();
        let result = tokio::time::timeout(Duration::from_secs(10), serving)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(result, Err(Error::Spawn(message)) if message.contains("passt exited")));
        assert!(probe(&options.socket).is_err());
    }
}

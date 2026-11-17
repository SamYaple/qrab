//! Trusted local Cilium provider resources. The configured API socket path
//! is a protected platform endpoint, not a caller-supplied locator. Its
//! original kernel peer, rather than a process name, selects the tool context.
use crate::process::{Process, SocketPeer};
use std::{
    fs::File,
    io,
    os::{
        fd::AsRawFd,
        unix::{net::UnixStream, process::CommandExt},
    },
    path::Path,
    process::{Child, Command, Stdio},
};

/// Fail promptly on a full local listen queue rather than waiting before
/// any read timeout can apply. The connected stream then uses blocking I/O.
pub(crate) fn connect_local(socket: &Path) -> io::Result<UnixStream> {
    use nix::sys::socket::{AddressFamily, SockFlag, SockType, UnixAddr};
    let fd = nix::sys::socket::socket(
        AddressFamily::Unix,
        SockType::Stream,
        SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
        None,
    )
    .map_err(io::Error::from)?;
    nix::sys::socket::connect(
        fd.as_raw_fd(),
        &UnixAddr::new(socket).map_err(io::Error::from)?,
    )
    .map_err(io::Error::from)?;
    let stream = UnixStream::from(fd);
    stream.set_nonblocking(false)?;
    Ok(stream)
}

/// One original provider and its held namespace/root capabilities.
pub struct Provider {
    peer: SocketPeer,
    process: Process,
    mount: File,
    pid: File,
    root: File,
}

impl Provider {
    pub fn connect(socket: &Path, proc_root: &Path) -> io::Result<Self> {
        // The protected Unix path is also the authority used for Cilium's
        // local API. No HTTP action is sent for resource discovery.
        let stream = connect_local(socket)?;
        let peer = SocketPeer::capture(&stream)?;
        let (process, _) = peer.process(proc_root)?;
        let mount = process.mount_namespace()?;
        let pid = process.pid_namespace()?;
        let root = process.root_directory()?;
        peer.ensure_alive()?;
        Ok(Self {
            peer,
            process,
            mount,
            pid,
            root,
        })
    }

    pub fn spawn_bpftool(&self, args: &[&str], input: Stdio) -> io::Result<Child> {
        self.spawn_program("bpftool", args, input)
    }

    fn spawn_program(&self, program: &str, args: &[&str], input: Stdio) -> io::Result<Child> {
        self.peer.ensure_alive()?;
        // Read through the held process directory, never its numeric path.
        self.process.pod_uid()?;
        let descriptors = [
            self.mount.as_raw_fd(),
            self.pid.as_raw_fd(),
            self.root.as_raw_fd(),
        ];
        let mut command = Command::new("/usr/bin/nsenter");
        command
            .args([
                format!("--mount=/proc/self/fd/{}", descriptors[0]),
                format!("--pid=/proc/self/fd/{}", descriptors[1]),
                format!("--root=/proc/self/fd/{}", descriptors[2]),
                format!("--wd=/proc/self/fd/{}", descriptors[2]),
            ])
            .arg("--")
            .arg(program)
            .args(args)
            .env(
                "PATH",
                "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            )
            .stdin(input)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // SAFETY: after fork, only async-signal-safe fcntl syscalls and OS
        // error construction run. Flags change only in the child descriptor
        // table. Self owns every descriptor until spawn has exec'd nsenter.
        unsafe {
            command.pre_exec(move || {
                for fd in descriptors {
                    let flags = libc::fcntl(fd, libc::F_GETFD);
                    if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                        return Err(io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        command.spawn()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{os::unix::net::UnixListener, sync::mpsc};

    fn fixture() -> (
        tempfile::TempDir,
        tempfile::TempDir,
        mpsc::Sender<()>,
        std::thread::JoinHandle<()>,
    ) {
        let socket = tempfile::tempdir().unwrap();
        let proc_root = tempfile::tempdir().unwrap();
        // A lower, unrelated name lookalike must never determine authority.
        std::fs::create_dir(proc_root.path().join("5")).unwrap();
        std::fs::write(proc_root.path().join("5/comm"), "cilium-agent\n").unwrap();
        let process = proc_root.path().join(std::process::id().to_string());
        std::fs::create_dir_all(process.join("ns")).unwrap();
        std::fs::write(
            process.join("cgroup"),
            "0::/kubepods/pod6b9f0c44-0000-4000-8000-000000000001/container\n",
        )
        .unwrap();
        std::fs::write(process.join("comm"), "not-the-provider-name\n").unwrap();
        for name in ["mnt", "pid"] {
            std::os::unix::fs::symlink(
                format!("/proc/self/ns/{name}"),
                process.join("ns").join(name),
            )
            .unwrap();
        }
        std::os::unix::fs::symlink("/", process.join("root")).unwrap();
        let listener = UnixListener::bind(socket.path().join("cilium.sock")).unwrap();
        let (done, wait) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            let _ = wait.recv();
        });
        (socket, proc_root, done, worker)
    }

    #[test]
    #[ignore = "explicit Linux SO_PEERPIDFD fixture; no node or BPF mutation"]
    fn original_socket_provider_pins_resources_despite_name_and_pid_path_replacement() {
        let (socket, proc_root, done, worker) = fixture();
        let provider =
            Provider::connect(&socket.path().join("cilium.sock"), proc_root.path()).unwrap();
        assert_eq!(provider.process.pid(), std::process::id() as i32);
        let path = proc_root.path().join(std::process::id().to_string());
        std::fs::rename(&path, proc_root.path().join("original")).unwrap();
        std::fs::create_dir_all(path.join("ns")).unwrap();
        std::fs::write(path.join("comm"), "cilium-agent\n").unwrap();
        std::fs::write(path.join("cgroup"), "0::/system.slice/unrelated.service\n").unwrap();
        assert!(provider.process.pod_uid().is_ok());
        for file in [&provider.mount, &provider.pid, &provider.root] {
            assert_ne!(
                nix::fcntl::fcntl(file.as_raw_fd(), nix::fcntl::FcntlArg::F_GETFD).unwrap()
                    & libc::FD_CLOEXEC,
                0
            );
        }
        done.send(()).unwrap();
        worker.join().unwrap();
    }

    #[test]
    #[ignore = "explicit isolated user/mount/PID namespace execution fixture"]
    fn held_provider_namespaces_are_used_without_reopening_a_pid() {
        use std::{
            io::{Read, Write},
            time::{Duration, Instant},
        };
        const HELPER: &str = "QRAB_CILIUM_NAMESPACE_HELPER_SOCKET";
        const TEST: &str =
            "cilium::tests::held_provider_namespaces_are_used_without_reopening_a_pid";
        if let Some(path) = std::env::var_os(HELPER) {
            let listener = UnixListener::bind(path).unwrap();
            for connection in listener.incoming() {
                let mut stream = connection.unwrap();
                let mut byte = [0u8; 1];
                if stream.read(&mut byte).unwrap_or(0) == 1 && byte[0] == b'q' {
                    break;
                }
            }
            return;
        }
        struct Helper(Option<std::process::Child>);
        impl Drop for Helper {
            fn drop(&mut self) {
                if let Some(child) = &mut self.0 {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
        let socket = tempfile::tempdir().unwrap();
        let path = socket.path().join("provider.sock");
        let mut helper = Helper(Some(
            Command::new("unshare")
                .args(["--mount", "--pid", "--fork", "--mount-proc", "--kill-child"])
                .arg(std::env::current_exe().unwrap())
                .args(["--ignored", "--exact", TEST, "--test-threads=1"])
                .env(HELPER, &path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        ));
        let start = Instant::now();
        let probe = loop {
            match UnixStream::connect(&path) {
                Ok(stream) => break stream,
                Err(_) if start.elapsed() < Duration::from_secs(5) => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(error) => panic!("private provider did not start: {error}"),
            }
        };
        let credentials =
            nix::sys::socket::getsockopt(&probe, nix::sys::socket::sockopt::PeerCredentials)
                .unwrap();
        let pid = credentials.pid();
        drop(probe);
        let expected_mount = std::fs::read_link(format!("/proc/{pid}/ns/mnt")).unwrap();
        let expected_pid = std::fs::read_link(format!("/proc/{pid}/ns/pid")).unwrap();
        assert_ne!(
            expected_mount,
            std::fs::read_link("/proc/self/ns/mnt").unwrap()
        );
        assert_ne!(
            expected_pid,
            std::fs::read_link("/proc/self/ns/pid").unwrap()
        );
        let proc_root = tempfile::tempdir().unwrap();
        let entry = proc_root.path().join(pid.to_string());
        std::fs::create_dir_all(entry.join("ns")).unwrap();
        std::fs::write(
            entry.join("cgroup"),
            "0::/kubepods/pod6b9f0c44-0000-4000-8000-000000000001/container\n",
        )
        .unwrap();
        for name in ["mnt", "pid"] {
            std::os::unix::fs::symlink(
                format!("/proc/{pid}/ns/{name}"),
                entry.join("ns").join(name),
            )
            .unwrap();
        }
        std::os::unix::fs::symlink(format!("/proc/{pid}/root"), entry.join("root")).unwrap();
        let provider = Provider::connect(&path, proc_root.path()).unwrap();
        std::fs::rename(&entry, proc_root.path().join("original")).unwrap();
        std::fs::create_dir_all(&entry).unwrap();
        std::fs::write(entry.join("comm"), "cilium-agent\n").unwrap();
        let result = provider
            .spawn_program(
                "/bin/sh",
                &[
                    "-c",
                    "readlink /proc/self/ns/mnt; readlink /proc/self/ns/pid",
                ],
                Stdio::null(),
            )
            .unwrap()
            .wait_with_output()
            .unwrap();
        let mut finish = UnixStream::connect(&path).unwrap();
        finish.write_all(b"q").unwrap();
        drop(finish);
        let output = helper.0.take().unwrap().wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let actual = String::from_utf8(result.stdout).unwrap();
        let expected = format!("{}\n{}\n", expected_mount.display(), expected_pid.display());
        assert_eq!(
            actual, expected,
            "tool entered the authenticated provider's distinct namespaces"
        );
    }
}

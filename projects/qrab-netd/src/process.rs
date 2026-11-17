//! Stable process and namespace resources for the privileged node agent.
//!
//! A numeric PID is a lookup name. Hold its proc directory before validating
//! membership, and open resources relative to that directory. Socket callers
//! additionally need the original peer's pidfd, not a later pidfd_open(PID).
use std::{
    fs::{File, OpenOptions},
    io::{self, Read},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{fs::OpenOptionsExt, net::UnixStream},
    },
    path::{Component, Path},
};

/// A held proc directory. Resource lookups do not reopen its numeric PID path.
pub struct Process {
    pid: i32,
    directory: File,
}

impl Process {
    pub fn open(proc_root: &Path, pid: i32) -> io::Result<Self> {
        if pid <= 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid process PID",
            ));
        }
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(proc_root.join(pid.to_string()))?;
        Ok(Self { pid, directory })
    }

    pub fn pid(&self) -> i32 {
        self.pid
    }

    pub fn pod_uid(&self) -> io::Result<String> {
        let record = self.read_text(Path::new("cgroup"), 16 * 1024)?;
        crate::cgroup::pod_uid(&record).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "process is not in a trusted kubelet cgroup",
            )
        })
    }

    pub fn network_namespace(&self) -> io::Result<File> {
        self.open_relative(Path::new("ns/net"))
    }

    /// Fixed relative proc paths only. `root/...` and `ns/net` intentionally
    /// follow kernel magic links within this held process directory.
    fn open_relative(&self, path: &Path) -> io::Result<File> {
        if path.as_os_str().is_empty()
            || path
                .components()
                .any(|p| !matches!(p, Component::Normal(_)))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid relative process path",
            ));
        }
        let fd = nix::fcntl::openat(
            Some(self.directory.as_raw_fd()),
            path,
            nix::fcntl::OFlag::O_RDONLY | nix::fcntl::OFlag::O_CLOEXEC,
            nix::sys::stat::Mode::empty(),
        )
        .map_err(io::Error::from)?;
        // SAFETY: successful openat returned a fresh descriptor owned here.
        Ok(unsafe { File::from_raw_fd(fd) })
    }

    pub fn read_text(&self, path: &Path, limit: usize) -> io::Result<String> {
        let bound = limit.checked_add(1).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "invalid process text limit")
        })?;
        let mut bytes = Vec::new();
        self.open_relative(path)?
            .take(bound as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > limit {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "process file exceeds its bound",
            ));
        }
        String::from_utf8(bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "process file is not UTF-8"))
    }
}

/// The process that established this Unix connection, pinned by the kernel.
pub struct SocketPeer {
    pid: i32,
    pidfd: OwnedFd,
}

impl SocketPeer {
    pub fn capture(stream: &UnixStream) -> io::Result<Self> {
        let credentials =
            nix::sys::socket::getsockopt(stream, nix::sys::socket::sockopt::PeerCredentials)
                .map_err(io::Error::from)?;
        let mut fd: libc::c_int = -1;
        let mut length = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
        // SAFETY: the stream is borrowed throughout; fd/length are valid
        // writable objects of exactly the size supplied to getsockopt.
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERPIDFD,
                std::ptr::from_mut(&mut fd).cast(),
                &mut length,
            )
        };
        if result < 0 {
            let error = io::Error::last_os_error();
            return Err(if error.raw_os_error() == Some(libc::ENOPROTOOPT) {
                io::Error::new(
                    io::ErrorKind::Unsupported,
                    "netd requires SO_PEERPIDFD support (Linux 6.5+)",
                )
            } else {
                error
            });
        }
        if fd < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid socket peer pidfd",
            ));
        }
        // SAFETY: SO_PEERPIDFD installed a fresh caller-owned descriptor.
        let pidfd = unsafe { OwnedFd::from_raw_fd(fd) };
        if length as usize != std::mem::size_of::<libc::c_int>() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid socket peer pidfd length",
            ));
        }
        let flags = nix::fcntl::fcntl(pidfd.as_raw_fd(), nix::fcntl::FcntlArg::F_GETFD)
            .map_err(io::Error::from)?;
        if flags & libc::FD_CLOEXEC == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "socket peer pidfd must be close-on-exec",
            ));
        }
        let peer = Self {
            pid: credentials.pid(),
            pidfd,
        };
        peer.ensure_alive()?;
        Ok(peer)
    }

    pub fn ensure_alive(&self) -> io::Result<()> {
        let mut poll = libc::pollfd {
            fd: self.pidfd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll points to one initialized pollfd; zero timeout never
        // waits. A readable/hung-up pidfd denotes the original peer's exit.
        match unsafe { libc::poll(&mut poll, 1, 0) } {
            0 => Ok(()),
            n if n < 0 => Err(io::Error::last_os_error()),
            _ => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "original socket peer has exited",
            )),
        }
    }

    pub fn process(&self, proc_root: &Path) -> io::Result<(Process, String)> {
        self.ensure_alive()?;
        let process = Process::open(proc_root, self.pid)?;
        let uid = process.pod_uid()?;
        // If exit/reuse occurred before the directory opened, the original
        // pidfd is now ready even if /proc/PID refers to another process.
        self.ensure_alive()?;
        Ok((process, uid))
    }
}

/// Startup probe only; no Pod membership, namespace change or guest is needed.
pub fn check_peer_support() -> io::Result<()> {
    let (stream, _other) = UnixStream::pair()?;
    SocketPeer::capture(&stream)?.ensure_alive()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        os::unix::net::UnixListener,
        process::{Child, Command, Stdio},
    };

    const FIRST: &str = "6b9f0c44-0000-4000-8000-000000000001";
    const SECOND: &str = "6b9f0c44-0000-4000-8000-000000000002";

    fn fixture(root: &Path, pid: i32, uid: &str, marker: &str) {
        let directory = root.join(pid.to_string());
        for child in ["ns", "root/etc", "root/run/qrab-vpn"] {
            std::fs::create_dir_all(directory.join(child)).unwrap();
        }
        for (path, text) in [
            ("cgroup", format!("0::/kubepods/pod{uid}/container\n")),
            ("ns/net", marker.into()),
            ("root/etc/resolv.conf", marker.into()),
            ("root/run/qrab-vpn/wg.conf", marker.into()),
        ] {
            std::fs::write(directory.join(path), text).unwrap();
        }
    }

    #[test]
    fn directory_and_namespace_handles_do_not_follow_a_reused_pid_path() {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path(), 7, FIRST, "original");
        let process = Process::open(root.path(), 7).unwrap();
        assert_eq!(process.pod_uid().unwrap(), FIRST);
        let mut namespace = process.network_namespace().unwrap();
        std::fs::rename(root.path().join("7"), root.path().join("previous-7")).unwrap();
        fixture(root.path(), 7, SECOND, "replacement");
        // The old check-then-reopen design would now reach another resource.
        assert_eq!(
            std::fs::read_to_string(root.path().join("7/ns/net")).unwrap(),
            "replacement"
        );
        assert_eq!(
            Process::open(root.path(), 7).unwrap().pod_uid().unwrap(),
            SECOND
        );
        let mut text = String::new();
        namespace.read_to_string(&mut text).unwrap();
        assert_eq!(text, "original");
        for path in [
            "cgroup",
            "ns/net",
            "root/etc/resolv.conf",
            "root/run/qrab-vpn/wg.conf",
        ] {
            let text = process.read_text(Path::new(path), 4096).unwrap();
            assert!(text.contains(if path == "cgroup" { FIRST } else { "original" }));
        }
    }

    #[test]
    fn process_reads_are_bounded_relative_and_close_on_exec() {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path(), 7, FIRST, "original");
        let process = Process::open(root.path(), 7).unwrap();
        for path in ["", "/etc/passwd", "../7/cgroup", "root/../../cgroup"] {
            assert_eq!(
                process.read_text(Path::new(path), 4096).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
        assert_eq!(
            process
                .read_text(Path::new("ns/net"), 2)
                .unwrap_err()
                .kind(),
            io::ErrorKind::FileTooLarge
        );
        assert_eq!(
            process
                .read_text(Path::new("ns/net"), usize::MAX)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        for file in [&process.directory, &process.network_namespace().unwrap()] {
            assert_ne!(
                nix::fcntl::fcntl(file.as_raw_fd(), nix::fcntl::FcntlArg::F_GETFD).unwrap()
                    & libc::FD_CLOEXEC,
                0
            );
        }
        assert!(Process::open(root.path(), 0).is_err());
    }

    #[test]
    fn a_symbolic_pid_directory_is_not_a_process_reference() {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path(), 7, FIRST, "original");
        std::os::unix::fs::symlink(root.path().join("7"), root.path().join("8")).unwrap();
        assert!(Process::open(root.path(), 8).is_err());
    }

    #[test]
    #[ignore = "explicit Linux 6.5+ SO_PEERPIDFD fixture; private Unix socket/child only"]
    fn original_socket_peer_exit_cannot_become_a_replacement_process() {
        const CHILD_SOCKET: &str = "QRAB_NETD_PEER_FIXTURE_SOCKET";
        if let Some(path) = std::env::var_os(CHILD_SOCKET) {
            let mut stream = UnixStream::connect(path).unwrap();
            stream.write_all(b"ready\n").unwrap();
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).unwrap();
            return;
        }
        check_peer_support().unwrap();
        struct Reap(Child);
        impl Drop for Reap {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let root = tempfile::tempdir().unwrap();
        let socket = root.path().join("peer.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let name = format!(
            "{}::original_socket_peer_exit_cannot_become_a_replacement_process",
            module_path!().split_once("::").unwrap().1
        );
        let mut child = Reap(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", &name, "--ignored", "--nocapture"])
                .env(CHILD_SOCKET, &socket)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock
                        && std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(5))
                }
                other => panic!("fixture child did not connect: {other:?}"),
            }
        };
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        let mut ready = [0; 6];
        stream.read_exact(&mut ready).unwrap();
        assert_eq!(&ready, b"ready\n");
        let peer = SocketPeer::capture(&stream).unwrap();
        assert_eq!(peer.pid, child.0.id() as i32);
        fixture(root.path(), peer.pid, FIRST, "original");
        let (process, uid) = peer.process(root.path()).unwrap();
        assert_eq!(uid, FIRST);
        assert_eq!(process.pid(), peer.pid);
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        fixture(root.path(), peer.pid, SECOND, "replacement");
        assert_eq!(
            Process::open(root.path(), peer.pid)
                .unwrap()
                .pod_uid()
                .unwrap(),
            SECOND
        );
        assert!(peer.ensure_alive().is_err());
        assert!(peer.process(root.path()).is_err());
        // A late capture still refers to the original connector. It never
        // substitutes pidfd_open() of a currently reused numeric PID.
        assert!(SocketPeer::capture(&stream)
            .and_then(|p| p.process(root.path()))
            .is_err());
    }
}

//! User namespace and cgroup utilities for rootless QEMU process management.
//!
//! This module provides:
//! - Spawning a process inside a new user namespace with uid/gid mapping
//! - Cgroup-based process tree discovery
//! - Process lifetime monitoring via /proc polling

use anyhow::{anyhow, Context, Result};
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{getpid, setgid, setsid, setuid, Gid, Pid, Uid};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::os::fd::IntoRawFd;
use std::os::unix::io::FromRawFd;
use std::path::Path;
use std::process::Stdio;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::unix::pipe;
use tokio::process::{Child, Command};
use tokio::time::{timeout, Duration};

/// A tree of processes rooted at a single PID.
#[derive(Debug, Clone)]
pub(crate) struct ProcessTree {
    pid: Pid,
    children: Vec<ProcessTree>,
}

impl ProcessTree {
    pub(crate) fn root_pid(&self) -> Pid {
        self.pid
    }

    pub(crate) fn all_pids(&self) -> Vec<Pid> {
        let mut pids = vec![self.pid];
        for child in &self.children {
            pids.extend(child.all_pids());
        }
        pids
    }
}

/// Read the parent PID of a process from `/proc/{pid}/stat`.
///
/// The stat file format is: `pid (comm) state ppid ...`
/// The comm field is enclosed in parentheses and may contain spaces,
/// so we find the last `)` and parse fields after it.
async fn get_parent_pid(pid: Pid) -> Result<(Pid, Pid)> {
    let stat_path = format!("/proc/{}/stat", pid);
    let content = fs::read_to_string(&stat_path)
        .await
        .with_context(|| format!("reading {stat_path}"))?;

    // Find the closing ')' of the comm field — everything after is space-separated
    let after_comm = content
        .rfind(')')
        .ok_or_else(|| anyhow!("malformed /proc/{pid}/stat: no closing paren"))?;
    let fields: Vec<&str> = content[after_comm + 1..].split_whitespace().collect();

    // fields[0] = state, fields[1] = ppid
    let ppid: i32 = fields
        .get(1)
        .ok_or_else(|| anyhow!("malformed /proc/{pid}/stat: missing ppid"))?
        .parse()
        .with_context(|| format!("parsing ppid from /proc/{pid}/stat"))?;

    Ok((pid, Pid::from_raw(ppid)))
}

/// Build process trees from the PIDs in a cgroup.
///
/// Reads `/sys/fs/cgroup/{cgroup_name}/cgroup.procs` and reconstructs the
/// parent-child relationships by reading each process's ppid from procfs.
pub(crate) async fn build_cgroup_process_trees(cgroup_name: &str) -> Result<Vec<ProcessTree>> {
    let path = Path::new("/sys/fs/cgroup")
        .join(cgroup_name)
        .join("cgroup.procs");

    let file = fs::File::open(&path)
        .await
        .with_context(|| format!("opening {}", path.display()))?;
    let reader = BufReader::new(file);

    let mut pids = Vec::new();
    let mut lines = reader.lines();
    while let Some(line) = lines.next_line().await? {
        if let Ok(pid) = line.parse::<i32>() {
            pids.push(Pid::from_raw(pid));
        }
    }

    let mut child_to_parent: HashMap<Pid, Pid> = HashMap::new();
    let mut parent_to_children: HashMap<Pid, Vec<Pid>> = HashMap::new();
    let mut all_pids: HashSet<Pid> = HashSet::new();

    for pid in pids {
        if let Ok((pid, ppid)) = get_parent_pid(pid).await {
            all_pids.insert(pid);
            child_to_parent.insert(pid, ppid);
            parent_to_children.entry(ppid).or_default().push(pid);
        }
    }

    // Root processes: their parent is not in the cgroup
    let root_pids: Vec<Pid> = all_pids
        .iter()
        .filter(|&&pid| {
            child_to_parent
                .get(&pid)
                .map(|ppid| !all_pids.contains(ppid))
                .unwrap_or(true)
        })
        .copied()
        .collect();

    fn build_tree(root: Pid, children_map: &HashMap<Pid, Vec<Pid>>) -> ProcessTree {
        let children = children_map
            .get(&root)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|child| build_tree(child, children_map))
            .collect();
        ProcessTree {
            pid: root,
            children,
        }
    }

    Ok(root_pids
        .into_iter()
        .map(|root| build_tree(root, &parent_to_children))
        .collect())
}

/// Wait for all given PIDs to exit by polling `/proc/{pid}`.
///
/// Returns `Ok(())` when all processes have exited, or an error if the
/// timeout (60 seconds) is exceeded.
pub(crate) async fn wait_for_pids(pids: Vec<Pid>) -> Result<()> {
    let mut remaining: HashSet<Pid> = pids.into_iter().collect();

    timeout(Duration::from_secs(60), async {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        while !remaining.is_empty() {
            interval.tick().await;
            remaining.retain(|pid| {
                // If /proc/{pid} doesn't exist, the process has exited
                Path::new(&format!("/proc/{}", pid)).exists()
            });
        }
    })
    .await
    .map_err(|_| {
        anyhow!(
            "timed out waiting for processes to exit: {:?}",
            remaining.iter().collect::<Vec<_>>()
        )
    })?;

    Ok(())
}

/// Spawn a process inside a new user namespace.
///
/// The spawned process goes through this sequence before exec:
/// 1. `setsid()` — detach from controlling terminal
/// 2. `unshare(CLONE_NEWUSER)` — create a new user namespace
/// 3. Signal the parent (via pipe) with the child PID
/// 4. Wait for the parent to set up uid_map/gid_map (via pipe)
/// 5. `setuid(0)` / `setgid(0)` — become root in the user namespace
/// 6. `exec()` the target command
///
/// The caller is responsible for the parent side: reading the child PID,
/// writing uid_map/gid_map, and signaling completion. See `VmBuilder`'s
/// `spawn_preexec_nshelper` for the parent-side implementation.
///
/// # Safety
///
/// The `pre_exec` closure runs between `fork()` and `exec()` in the child
/// process. At this point:
/// - Only one thread exists in the child (all other threads are gone)
/// - The pipe FDs are valid (created before fork, passed via closure capture)
/// - `from_raw_fd` is called exactly once per FD, transferring ownership
/// - All operations are synchronous (no async, no allocations beyond stack)
///
/// The `unsafe` block on `pre_exec` is required by the Rust API but the
/// operations performed are safe given the single-threaded post-fork context.
pub(crate) async fn spawn_namespaced_process(
    reader_pipe: pipe::Receiver,
    writer_pipe: pipe::Sender,
    cmd: String,
    args: Vec<String>,
) -> Result<Child> {
    let reader_raw_fd = reader_pipe.into_blocking_fd()?.into_raw_fd();
    let writer_raw_fd = writer_pipe.into_blocking_fd()?.into_raw_fd();

    let mut process = Command::new(&cmd);

    // SAFETY: see function-level documentation
    unsafe {
        process.pre_exec(move || {
            setsid().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

            unshare(CloneFlags::CLONE_NEWUSER)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

            // Signal parent with our PID so it can write our uid_map/gid_map.
            // The FD is consumed (dropped) at the end of this block.
            {
                #[allow(unused_unsafe)]
                let mut writer = unsafe { std::fs::File::from_raw_fd(writer_raw_fd) };
                writer.write_all(b"r")?;
                let pid_bytes = getpid().as_raw().to_le_bytes();
                writer.write_all(&pid_bytes)?;
            }

            // Wait for parent to confirm uid_map/gid_map setup.
            {
                #[allow(unused_unsafe)]
                let mut reader = unsafe { std::fs::File::from_raw_fd(reader_raw_fd) };
                let mut buf = [0u8; 1];
                reader.read_exact(&mut buf)?;
                if buf[0] != b'R' {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("namespace setup failed: parent sent {:?} instead of 'R'", buf[0] as char),
                    ));
                }
            }

            // Elevate to root inside the user namespace
            setuid(Uid::from_raw(0))
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            setgid(Gid::from_raw(0))
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

            Ok(())
        });
    }

    let child = process
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("spawning {cmd}"))?;

    Ok(child)
}

/// Check whether the current process can create user namespaces.
pub(crate) fn _can_create_user_namespace() -> bool {
    // Check the sysctl — if the file doesn't exist, user namespaces are
    // typically available (the sysctl was added to restrict them).
    match std::fs::read_to_string("/proc/sys/kernel/unprivileged_userns_clone") {
        Ok(val) => val.trim() != "0",
        Err(_) => true,
    }
}

/// Check whether a cgroup path exists and is writable.
pub(crate) fn _cgroup_is_writable(cgroup_name: &str) -> bool {
    let procs_path = format!("/sys/fs/cgroup/{cgroup_name}/cgroup.procs");
    std::fs::OpenOptions::new()
        .write(true)
        .open(procs_path)
        .is_ok()
}

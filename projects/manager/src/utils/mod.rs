use anyhow::Result;
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{getpid, setgid, setsid, setuid, Gid, Pid, Uid};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::os::fd::IntoRawFd;
use std::os::unix::io::FromRawFd;
use std::path::Path;
use std::process::Stdio;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::unix::pipe;
use tokio::process::{Child, Command};
use tokio::time::Duration;

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

async fn get_parent_pid(pid: Pid) -> Result<(Pid, Pid)> {
    let stat_path = format!("/proc/{}/stat", pid);
    let file = File::open(stat_path).await?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    let parts: Vec<&str> = line.split_whitespace().collect();
    let pid: u32 = parts[0].parse()?;
    let ppid: u32 = parts[3].parse()?;
    Ok((Pid::from_raw(pid as i32), Pid::from_raw(ppid as i32)))
}

pub(crate) async fn build_cgroup_process_trees(cgroup_name: String) -> Result<Vec<ProcessTree>> {
    let path = Path::new("/sys/fs/cgroup")
        .join(cgroup_name)
        .join("cgroup.procs");
    let pids = {
        let file = File::open(path).await?;
        let reader = BufReader::new(file);
        let mut pids = Vec::new();
        let mut lines = reader.lines();
        while let Some(line) = lines.next_line().await? {
            if let Ok(pid) = line.parse::<u32>() {
                pids.push(Pid::from_raw(pid as i32));
            }
        }
        pids
    };

    let mut child_to_parent_mapping: HashMap<Pid, Pid> = HashMap::new();
    let mut parent_to_children_mapping: HashMap<Pid, Vec<Pid>> = HashMap::new();
    let mut all_pids: HashSet<Pid> = HashSet::new();

    // Read parent PID for each PID and populate the maps
    for pid in pids {
        if let Ok((pid, ppid)) = get_parent_pid(pid).await {
            all_pids.insert(pid);
            child_to_parent_mapping.insert(pid, ppid);
            parent_to_children_mapping
                .entry(ppid)
                .or_default()
                .push(pid);
        }
    }

    // Find root processes (those not found as children)
    let root_pids: Vec<Pid> = all_pids
        .iter()
        .filter(|&&pid| {
            !child_to_parent_mapping.contains_key(&pid)
                || !all_pids.contains(&child_to_parent_mapping[&pid])
        })
        .cloned()
        .collect();

    fn build_tree(
        root_pid: Pid,
        parent_to_children_mapping: &HashMap<Pid, Vec<Pid>>,
    ) -> ProcessTree {
        let children_pids = parent_to_children_mapping
            .get(&root_pid)
            .cloned()
            .unwrap_or_default();
        let children = children_pids
            .into_iter()
            .map(|child_pid| build_tree(child_pid, parent_to_children_mapping))
            .collect();
        ProcessTree {
            pid: root_pid,
            children,
        }
    }

    let mut trees = Vec::new();
    for root_pid in root_pids {
        trees.push(build_tree(root_pid, &parent_to_children_mapping));
    }
    Ok(trees)
}

pub(crate) async fn wait_for_pids(pids: Vec<Pid>) -> Result<()> {
    // I gave up on this function...
    // TODO something other than sleep, reference previous attempt
    eprintln!("TODO: check for pids to die instead of sleep -- {:?}", pids);
    tokio::time::sleep(Duration::from_millis(1_500)).await;

    //let mut known_pids = HashSet::new();
    //for pid in pids {
    //    known_pids.insert(pid);
    //}
    //let mut interval = tokio::time::interval(Duration::from_millis(100));
    //tokio::time::timeout(Duration::from_millis(60_000), async {
    //    loop {
    //        if known_pids.is_empty() {
    //            break;
    //        }
    //        let mut pids_to_remove = HashSet::new();
    //        for &pid in &known_pids {
    //            match waitpid(pid, Some(WaitPidFlag::WNOHANG)) {
    //                Ok(status) => match status {
    //                    WaitStatus::Exited(_, _) | WaitStatus::Signaled(_, _, _) => {}
    //                    _ => {
    //                        dbg!(status);
    //                    }
    //                },
    //                Err(nix::errno::Errno::ECHILD) => {
    //                    pids_to_remove.insert(pid);
    //                }
    //                Err(e) => {
    //                    eprintln!("Untracked error: {:?}", e);
    //                    pids_to_remove.insert(pid);
    //                }
    //            }
    //        }
    //        for pid in pids_to_remove.iter() {
    //            known_pids.remove(pid);
    //        }
    //        interval.tick().await;
    //    }
    //}).await.unwrap();
    Ok(())
}

pub async fn spawn_namespaced_process(
    reader_pipe: pipe::Receiver,
    writer_pipe: pipe::Sender,
    cmd: String,
    args: Vec<String>,
) -> Result<Child> {
    // Convert our async pipes into raw file descriptors for use in the preexec
    // stage which is not async.
    let reader_raw_fd = reader_pipe.into_blocking_fd()?.into_raw_fd();
    let writer_raw_fd = writer_pipe.into_blocking_fd()?.into_raw_fd();

    let mut process = Command::new(cmd);
    unsafe {
        // FIXME Document and verify I am not violating memory safety here
        // TODO Assume this code violates safety until this message is removed
        process.pre_exec(move || {
            // The purpose of this block is to setup the forked pid before the
            // target process is exec'd. The child pid is created, but we retain
            // full control at this point.

            // Detach from the controlling parent
            setsid().expect("setsid failed");

            // Create and enter new user namespace
            unshare(CloneFlags::CLONE_NEWUSER).unwrap();

            // At this point in execution, we are in a new user namespace but do
            // not have the ability to setuid(0) and elevate to root. In fact,
            // as a result of the unshare we have no CAPABILITIES at all and our
            // user is `nobody` with uid `65534`.
            //
            // We need to setup the /proc/self/uid_map file to expose the parent
            // mappings to the user namespace. However, we don't have permission
            // to write to those files... but our parent process still does!
            //
            // Using our previously created `pipe()` we signal our parent with
            // `r` followed by a `u32` in bytes represending our new child pid.
            // We scope the entire block so that the write handle goes out of
            // scope as soon as possible.
            {
                #[allow(unused_unsafe)]
                let mut writer = unsafe { std::fs::File::from_raw_fd(writer_raw_fd) };
                // signal parent we have created our namespace
                writer.write_all(b"r").unwrap();

                // get our forked child pid and send it to the parent
                let pid = getpid();
                let buf: [u8; 4] = pid.as_raw().to_le_bytes();

                // report pid to parent
                // TODO is this really ok? I should look for another method
                writer.write_all(&buf).unwrap();
            }

            // In this next block, we wait for a response from our parent
            // informing us of a success or failure. An `R` indicates success,
            // and anything else is a failure. This will block until it receives
            // one byte, after which the reader will go out of scope and close.
            {
                #[allow(unused_unsafe)]
                let mut reader = unsafe { std::fs::File::from_raw_fd(reader_raw_fd) };
                let mut buf = [0; 1];
                // Wait for the parent to set up the mappings for us
                reader.read_exact(&mut buf).unwrap();
                if buf[0] != b'R' {
                    eprintln!("Expected 'R' signal, but received {:?}", buf);
                    todo!("Cannot recover and this error block sucks");
                }
            }

            // At this point, the /proc/self/uid_map has been properly setup by
            // our parent process. We can now elevate ourselves to root in our
            // new user namespace.
            setuid(Uid::from_raw(0)).unwrap();
            setgid(Gid::from_raw(0)).unwrap();
            // 😎 We are now root.... less! 😎

            Ok(())
        });
    }

    // Spawn our child process. This is where pre_exec gets triggered to run. It
    // happens after the fork, but before control is handed off with the exec
    let child = process
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(child)
}

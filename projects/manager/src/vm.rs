use anyhow::{anyhow, Result};
use nix::sys::signal::{self, Signal};
use nix::unistd::{getgid, getuid, Pid};
use std::io::Write;
use std::path::Path;
use tokio::fs::{write, File};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::pipe;
use tokio::spawn;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};

use crate::qmp::qapi::{Event, QMPCommand};
use crate::qmp::QMP;

use crate::utils::build_cgroup_process_trees;
use crate::utils::spawn_namespaced_process;
use crate::utils::wait_for_pids;
use crate::utils::ProcessTree;

#[derive(Debug)]
pub struct VM {
    id: String,
    running: bool,
    manager: Option<QMP>,
}

impl VM {
    pub async fn new(id: String) -> Self {
        Self {
            id,
            manager: None,
            running: false,
        }
    }

    pub async fn start(&mut self) -> Result<mpsc::Receiver<Event>> {
        // Try to reattach to a running instance if it exists
        if self.running {
            todo! {"called start on a running vm"};
        }

        match self.reattach().await {
            Ok(events) => return Ok(events),
            Err(_) => {}
        }

        // There is no qemu process running. We can now start qemu, but the vcpu
        // threads do not start running until after we exit the preconfig stage.
        self.prestart().await.unwrap();

        // The qemu process should be running and waiting for us to attach to
        // the socket. When we do, we are returned a events receiver object that
        // gets async generated events from qmp.
        let events = self.attach_manager().await?;

        // At this point, the qemu process is running, but no untrusted code has
        // executed in the VM yet. The qemu process is waiting for us to send
        // any additional configuration settings. Limited `QMPCommands` are
        // available in this mode.
        //let cmd = QMPCommand::QOMList{ arguments: QOMListArgs {path: "/machine".to_string()}};
        //let cmd = QMPCommand::QOMListTypes{ arguments: QOMListTypesArgs::default()};
        //let reply = self.qmp(cmd).await?;
        //eprintln!("{reply}");
        //dbg!(reply);

        // Send `x-exit-preconfig` which tells qemu to continue booting. As soon
        // as this command is sent, untrusted user code may be running inside of
        // the vm.
        let _reply = self.qmp(QMPCommand::XExitPreconfig).await?;
        //dbg!(_reply);

        self.running = true;
        Ok(events)
    }

    pub async fn stop(&mut self) -> Result<()> {
        sleep(Duration::from_millis(100)).await; // TODO -- sleeping to drain events
        if let Some(tree) = self.build_pid_tree().await? {
            let status = self.qmp(QMPCommand::QueryStatus).await?;
            eprintln!("unused poweroff status -- {:?}", status);

            if let Some(ref mut manager) = self.manager {
                manager.shutdown().await;
                self.manager = None;
            };

            signal::kill(tree.root_pid(), Signal::SIGTERM).unwrap();
            wait_for_pids(tree.all_pids()).await.unwrap();

            self.running = false;
        }
        Ok(())
    }

    pub async fn restart(&mut self) -> Result<mpsc::Receiver<Event>> {
        self.stop().await.unwrap();
        if let Some(_) = self.build_pid_tree().await? {
            return Err(anyhow! {"VM processes are still running; this is a race condition"});
        }
        let events = self.start().await?;
        Ok(events)
    }

    // TODO I hate this
    fn cgroup_name(&self) -> String {
        let uid = getuid();
        let id = self.id.clone();
        format!("user.slice/user-{uid}.slice/user@{uid}.service/qrab.slice/qrab-{id}.slice")
        //format!("user.slice/user-{uid}.slice/user@{uid}.service/qrab.slice")
    }

    // TODO im so tired what even is this function
    fn qmp_socket(&self) -> String {
        format!("/tmp/{}.sock", self.id.clone())
    }

    async fn build_pid_tree(&self) -> Result<Option<ProcessTree>> {
        let trees = build_cgroup_process_trees(self.cgroup_name()).await?;
        let tree = match trees.len() {
            0 => return Ok(None),
            1 => trees.first().unwrap().clone(),
            _ => {
                dbg!(trees);
                return Err(anyhow! {"Multiple process tree roots were found."});
            }
        };
        Ok(Some(tree))
    }

    async fn attach_manager(&mut self) -> Result<mpsc::Receiver<Event>> {
        sleep(Duration::from_millis(1_000)).await; // TODO WAIT FOR SOCKET TO EXIST in a better way

        // Attempt to reattach to the QMP socket
        let socket_path_str = self.qmp_socket();
        let socket_path = Path::new(&socket_path_str);
        let (manager, events) = QMP::new(socket_path).await?;
        self.manager = Some(manager);
        // We successfully reattached to this socket

        Ok(events)
    }

    async fn reattach(&mut self) -> Result<mpsc::Receiver<Event>> {
        if let Some(_) = self.build_pid_tree().await? {
            // Our internal state says we are not running, but there is a
            // process running in the cgroup.
            if let Some(_) = &self.manager {
                todo! {r#"This should never happen. Our internal"
                        "self.running state is `false`, but the manager"
                        "is saying it is already init"#};
            }
            let events = self.attach_manager().await?;
            self.running = true;
            return Ok(events);
        }
        Err(anyhow! {"no running process found to reattach to"})
    }

    /// Help a forked process elevate to root in its user namespace
    async fn spawn_preexec_nshelper(
        mut reader: pipe::Receiver,
        mut writer: pipe::Sender,
        cgroup: Option<String>,
    ) -> Result<JoinHandle<()>> {
        let handle = spawn(async move {
            // Wait for our child process to signal to us that the user namespace
            // has been created and return our forked child pid
            let child_pid = {
                let mut buf = [0; 1];
                reader.read_exact(&mut buf).await.unwrap();
                if buf[0] != b'r' {
                    eprintln!("Expected 'r' signal, but received {:?}", buf);
                    todo!("Cannot recover and this error block sucks");
                }
                let mut buf = [0; 4];
                reader.read_exact(&mut buf).await.unwrap();
                u32::from_le_bytes(buf)
            };

            // Create a uid_map for our child process
            let uid_map = format!("0 {} 1", getuid());
            let uid_map_path = format!("/proc/{}/uid_map", child_pid);
            write(uid_map_path, uid_map).await.unwrap();

            // Disable setgroups before updating gid_map
            // TODO add more info here about the security around why we do this
            let setgroups_path = format!("/proc/{}/setgroups", child_pid);
            let mut setgroups = File::create(setgroups_path).await.unwrap();
            setgroups.write_all(b"deny").await.unwrap();

            // Create a gid_map for our child process
            let gid_map = format!("0 {} 1", getgid());
            let gid_map_path = format!("/proc/{}/gid_map", child_pid);
            write(gid_map_path, gid_map).await.unwrap();

            // Finally, we need to add the child process to the user cgroup.
            // We do this by writing the pid to the named cgroup file in sysfs.
            if let Some(name) = &cgroup {
                let path = format!("/sys/fs/cgroup/{}/cgroup.procs", name);
                let mut procs = std::fs::OpenOptions::new()
                    .write(true)
                    .open(path.clone())
                    .expect(&format!("Failed to open {} for writing", path));
                let pid_str = format!("{}\n", child_pid);
                procs
                    .write_all(pid_str.as_bytes())
                    .expect("Failed to write to {path}");
            }

            // Scoped so the write handle is dropped as soon as possible
            {
                // Signal the child that every thing is setup and good to go
                writer.write_all(b"R").await.unwrap();
            }
        });
        Ok(handle)
    }

    async fn prestart(&mut self) -> Result<()> {
        // Default args. This disables defaults and config loading across the
        // board. Maybe `-mem-prealloc` doesn't belong here long term?
        let qmp_unix = format!("unix:{},server,wait=on", self.qmp_socket());
        let cmd = "qemu-system-x86_64".to_string();
        let args = vec![
            "--preconfig".to_string(),
            "-run-with".to_string(),
            "async-teardown=on".to_string(),
            "-nographic".to_string(),
            "-no-user-config".to_string(),
            "-nodefaults".to_string(),
            "-no-reboot".to_string(),
            "-no-shutdown".to_string(),
            "-mem-prealloc".to_string(),
            "-machine".to_string(),
            "none".to_string(),
            "-qmp".to_string(),
            qmp_unix,
            "-name".to_string(),
            self.id.clone(),
            //"-machine", "pc-q35-8.2",
            //"-smp",     "1,sockets=1,cores=1,threads=1",
            //"-m",       "1G",
        ];

        let (inside_write_half, outside_read_half) = pipe::pipe()?;
        let (outside_write_half, inside_read_half) = pipe::pipe()?;
        let nshelper_handle = Self::spawn_preexec_nshelper(
            outside_read_half,
            outside_write_half,
            Some(self.cgroup_name()),
        )
        .await?;
        let child_handle = spawn(spawn_namespaced_process(
            inside_read_half,
            inside_write_half,
            cmd,
            args,
        ));
        let (child_res, nshelper_res) = tokio::join!(child_handle, nshelper_handle);
        let child = child_res??;
        nshelper_res.unwrap();

        if let Some(tree) = self.build_pid_tree().await? {
            if Pid::from_raw(child.id().unwrap() as i32) != tree.root_pid() {
                return Err(anyhow! {"child_pid does not match cgroup list"});
            }
        }
        Ok(())
    }

    pub async fn qmp(&mut self, cmd: QMPCommand) -> Result<String> {
        if let Some(ref mut manager) = self.manager {
            Ok(manager.execute(cmd).await?)
        } else {
            Err(anyhow! {"QMP manager not set up"})
        }
    }
}

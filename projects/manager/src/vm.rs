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

use qapi::QapiCommand;
use qapi_protocol::{Qmp, QmpEvent};

use crate::utils::build_cgroup_process_trees;
use crate::utils::spawn_namespaced_process;
use crate::utils::wait_for_pids;
use crate::utils::ProcessTree;

#[derive(Debug)]
pub struct VM {
    id: String,
    running: bool,
    qmp: Option<Qmp>,
}

impl VM {
    pub async fn new(id: String) -> Self {
        Self {
            id,
            qmp: None,
            running: false,
        }
    }

    pub async fn start(&mut self) -> Result<mpsc::Receiver<QmpEvent>> {
        if self.running {
            todo!("called start on a running vm");
        }

        if let Ok(events) = self.reattach().await {
            return Ok(events);
        }

        // There is no qemu process running. Start qemu in preconfig mode —
        // vcpu threads do not run until we exit the preconfig stage.
        self.prestart().await?;

        // The qemu process is now running and waiting for us to connect to the
        // QMP socket. Connecting returns an event receiver for async events.
        let events = self.attach_qmp().await?;

        // The VM is running but no untrusted code has executed yet. QEMU is
        // waiting for additional configuration. Only preconfig-safe commands
        // are available at this point.

        // Exit preconfig — this tells QEMU to continue booting. After this
        // command, untrusted guest code may be running.
        self.execute(&qapi::XExitPreconfig {}).await?;

        self.running = true;
        Ok(events)
    }

    pub async fn stop(&mut self) -> Result<()> {
        sleep(Duration::from_millis(100)).await; // TODO: drain events properly
        if let Some(tree) = self.build_pid_tree().await? {
            let status: qapi::StatusInfo = self.execute(&qapi::QueryStatus {}).await?;
            eprintln!("pre-stop status: running={}", status.running);

            if let Some(ref mut qmp) = self.qmp {
                qmp.shutdown().await;
                self.qmp = None;
            }

            signal::kill(tree.root_pid(), Signal::SIGTERM).unwrap();
            wait_for_pids(tree.all_pids()).await?;

            self.running = false;
        }
        Ok(())
    }

    pub async fn restart(&mut self) -> Result<mpsc::Receiver<QmpEvent>> {
        self.stop().await?;
        if self.build_pid_tree().await?.is_some() {
            return Err(anyhow!("VM processes are still running; this is a race condition"));
        }
        self.start().await
    }

    pub async fn execute<C: QapiCommand>(&mut self, cmd: &C) -> Result<C::Returns> {
        let qmp = self.qmp.as_ref().ok_or_else(|| anyhow!("QMP not connected"))?;
        Ok(qmp.execute(cmd).await?)
    }

    // -- internal helpers --

    fn cgroup_name(&self) -> String {
        let uid = getuid();
        format!(
            "user.slice/user-{uid}.slice/user@{uid}.service/qrab.slice/qrab-{}.slice",
            self.id
        )
    }

    fn qmp_socket(&self) -> String {
        format!("/tmp/{}.sock", self.id)
    }

    async fn build_pid_tree(&self) -> Result<Option<ProcessTree>> {
        let trees = build_cgroup_process_trees(self.cgroup_name()).await?;
        match trees.len() {
            0 => Ok(None),
            1 => Ok(Some(trees.into_iter().next().unwrap())),
            _ => {
                dbg!(&trees);
                Err(anyhow!("Multiple process tree roots were found."))
            }
        }
    }

    async fn attach_qmp(&mut self) -> Result<mpsc::Receiver<QmpEvent>> {
        sleep(Duration::from_millis(1_000)).await; // TODO: wait for socket properly

        let socket_path_str = self.qmp_socket();
        let socket_path = Path::new(&socket_path_str);
        let (qmp, events) = Qmp::new(socket_path).await?;
        self.qmp = Some(qmp);

        Ok(events)
    }

    async fn reattach(&mut self) -> Result<mpsc::Receiver<QmpEvent>> {
        if self.build_pid_tree().await?.is_some() {
            if self.qmp.is_some() {
                todo!(
                    "Internal state inconsistency: self.running is false but QMP is already connected"
                );
            }
            let events = self.attach_qmp().await?;
            self.running = true;
            return Ok(events);
        }
        Err(anyhow!("no running process found to reattach to"))
    }

    /// Help a forked process elevate to root in its user namespace
    async fn spawn_preexec_nshelper(
        mut reader: pipe::Receiver,
        mut writer: pipe::Sender,
        cgroup: Option<String>,
    ) -> Result<JoinHandle<()>> {
        let handle = spawn(async move {
            // Wait for child to signal that the user namespace has been created
            let child_pid = {
                let mut buf = [0; 1];
                reader.read_exact(&mut buf).await.unwrap();
                assert_eq!(buf[0], b'r', "Expected 'r' signal from child");
                let mut buf = [0; 4];
                reader.read_exact(&mut buf).await.unwrap();
                u32::from_le_bytes(buf)
            };

            // Set up uid_map
            let uid_map = format!("0 {} 1", getuid());
            write(format!("/proc/{child_pid}/uid_map"), uid_map)
                .await
                .unwrap();

            // Disable setgroups before updating gid_map
            let mut setgroups = File::create(format!("/proc/{child_pid}/setgroups"))
                .await
                .unwrap();
            setgroups.write_all(b"deny").await.unwrap();

            // Set up gid_map
            let gid_map = format!("0 {} 1", getgid());
            write(format!("/proc/{child_pid}/gid_map"), gid_map)
                .await
                .unwrap();

            // Add child to cgroup
            if let Some(name) = &cgroup {
                let path = format!("/sys/fs/cgroup/{name}/cgroup.procs");
                let mut procs = std::fs::OpenOptions::new()
                    .write(true)
                    .open(&path)
                    .unwrap_or_else(|e| panic!("Failed to open {path}: {e}"));
                procs
                    .write_all(format!("{child_pid}\n").as_bytes())
                    .unwrap();
            }

            // Signal the child that setup is complete
            writer.write_all(b"R").await.unwrap();
        });
        Ok(handle)
    }

    async fn prestart(&mut self) -> Result<()> {
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
        nshelper_res?;

        if let Some(tree) = self.build_pid_tree().await? {
            if Pid::from_raw(child.id().unwrap() as i32) != tree.root_pid() {
                return Err(anyhow!("child_pid does not match cgroup list"));
            }
        }
        Ok(())
    }
}

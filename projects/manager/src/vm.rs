use anyhow::Result;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use qapi::QapiCommand;
use qapi_protocol::{Qmp, QmpEvent};

use crate::arch::{Arch, MachineConfig};

static SOCKET_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Builder for configuring and spawning a QEMU virtual machine.
pub struct VmBuilder {
    arch: Arch,
    config: MachineConfig,
    drives: Vec<(PathBuf, String)>,
    raw_drives: Vec<(PathBuf, String)>,
    kernel: Option<PathBuf>,
    initrd: Option<PathBuf>,
    append: Option<String>,
    serial: bool,
    no_shutdown: bool,
    preconfig: bool,
    firmware: Option<PathBuf>,
    extra_args: Vec<String>,
}

impl VmBuilder {
    pub fn new(arch: Arch, config: MachineConfig) -> Self {
        Self {
            arch,
            config,
            drives: Vec::new(),
            raw_drives: Vec::new(),
            kernel: None,
            initrd: None,
            append: None,
            serial: false,
            no_shutdown: false,
            preconfig: false,
            firmware: None,
            extra_args: Vec::new(),
        }
    }

    /// Add a virtio drive (qcow2, raw, etc).
    pub fn drive(mut self, file: PathBuf, format: &str) -> Self {
        self.drives.push((file, format.to_string()));
        self
    }

    /// Add a read-only raw drive (e.g., CIDATA configdrive).
    pub fn raw_drive_ro(mut self, file: PathBuf) -> Self {
        self.raw_drives.push((file, "raw".to_string()));
        self
    }

    /// Use direct kernel boot (bypasses bootloader).
    pub fn kernel(mut self, kernel: PathBuf, initrd: PathBuf, append: &str) -> Self {
        self.kernel = Some(kernel);
        self.initrd = Some(initrd);
        self.append = Some(append.to_string());
        self
    }

    /// Enable serial output to a log file.
    pub fn serial(mut self) -> Self {
        self.serial = true;
        self
    }

    /// Don't exit QEMU on guest shutdown.
    pub fn no_shutdown(mut self) -> Self {
        self.no_shutdown = true;
        self
    }

    /// Start QEMU in preconfig mode (`--preconfig`).
    /// Requires `x-exit-preconfig` to continue boot.
    pub fn preconfig(mut self) -> Self {
        self.preconfig = true;
        self
    }

    /// Override firmware path for UEFI boot.
    pub fn firmware(mut self, path: PathBuf) -> Self {
        self.firmware = Some(path);
        self
    }

    /// Add an extra QEMU argument.
    pub fn arg(mut self, a: impl Into<String>) -> Self {
        self.extra_args.push(a.into());
        self
    }

    /// Spawn the QEMU process, connect QMP, and return a `Vm` handle.
    pub async fn spawn(self) -> Result<Vm> {
        let id = SOCKET_COUNTER.fetch_add(1, Ordering::SeqCst);
        let tmpdir = std::env::temp_dir();
        let qmp_sock = tmpdir.join(format!("qrab-qmp-{}-{}.sock", std::process::id(), id));
        let serial_log = if self.serial {
            Some(tmpdir.join(format!(
                "qrab-serial-{}-{}.log",
                std::process::id(),
                id
            )))
        } else {
            Option::None
        };

        let _ = tokio::fs::remove_file(&qmp_sock).await;
        if let Some(ref s) = serial_log {
            let _ = tokio::fs::remove_file(s).await;
        }

        let mut args: Vec<String> = Vec::new();

        // Preconfig
        if self.preconfig {
            args.push("--preconfig".into());
        }

        // Machine
        match &self.config {
            MachineConfig::None => {
                args.extend(["-machine".into(), "none".into()]);
            }
            MachineConfig::Bios { memory } => {
                let machine = self
                    .arch
                    .bios_machine()
                    .expect("BIOS boot not supported for this architecture");
                args.extend(["-machine".into(), machine.into()]);
                args.extend(["-m".into(), memory.clone()]);
            }
            MachineConfig::Uefi { memory } => {
                let machine = self.arch.uefi_machine();
                args.extend(["-machine".into(), machine.into()]);
                args.extend(["-m".into(), memory.clone()]);
                if let Some(ref fw) = self.firmware {
                    match self.arch {
                        Arch::X86_64 | Arch::I386 => {
                            args.extend(["-bios".into(), fw.display().to_string()]);
                        }
                        Arch::Aarch64 | Arch::Arm => {
                            args.extend([
                                "-drive".into(),
                                format!(
                                    "if=pflash,format=raw,readonly=on,file={}",
                                    fw.display()
                                ),
                            ]);
                        }
                    }
                }
            }
        }

        // CPU
        if let Some(cpu) = self.arch.cpu() {
            args.extend(["-cpu".into(), cpu.into()]);
        }

        args.extend(["-nographic".into(), "-nodefaults".into()]);

        // QMP
        args.extend([
            "-qmp".into(),
            format!("unix:{},server=on,wait=off", qmp_sock.display()),
        ]);

        // Serial
        if let Some(ref log) = serial_log {
            args.extend(["-serial".into(), format!("file:{}", log.display())]);
        }

        // Direct kernel boot
        if let Some(ref kernel) = self.kernel {
            args.extend(["-kernel".into(), kernel.display().to_string()]);
        }
        if let Some(ref initrd) = self.initrd {
            args.extend(["-initrd".into(), initrd.display().to_string()]);
        }
        if let Some(ref append) = self.append {
            args.extend(["-append".into(), append.clone()]);
        }

        // Drives
        for (file, format) in &self.drives {
            args.extend([
                "-drive".into(),
                format!("if=virtio,format={},file={}", format, file.display()),
            ]);
        }
        for (file, format) in &self.raw_drives {
            args.extend([
                "-drive".into(),
                format!(
                    "if=virtio,format={},file={},readonly=on",
                    format,
                    file.display()
                ),
            ]);
        }

        if self.no_shutdown {
            args.push("-no-shutdown".into());
        }

        args.extend(self.extra_args);

        let child = Command::new(self.arch.binary())
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        // Wait for QMP socket
        for _ in 0..50 {
            if qmp_sock.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        if !qmp_sock.exists() {
            anyhow::bail!("QMP socket did not appear: {}", qmp_sock.display());
        }

        let (qmp, events) = Qmp::new(&qmp_sock).await?;

        Ok(Vm {
            qmp,
            events,
            serial_log,
            child,
            qmp_sock,
        })
    }
}

/// A running QEMU virtual machine with a QMP connection.
pub struct Vm {
    pub qmp: Qmp,
    pub events: mpsc::Receiver<QmpEvent>,
    pub serial_log: Option<PathBuf>,
    child: Child,
    qmp_sock: PathBuf,
}

impl Vm {
    /// Execute a typed QMP command and return the typed response.
    pub async fn execute<C: QapiCommand>(&self, cmd: &C) -> Result<C::Returns> {
        Ok(self.qmp.execute(cmd).await?)
    }

    /// Graceful shutdown: disconnect QMP, then kill the process.
    pub async fn shutdown(mut self) {
        self.qmp.shutdown().await;
        let _ = self.child.start_kill();
        let _ = self.child.wait().await;
        let _ = tokio::fs::remove_file(&self.qmp_sock).await;
        if let Some(ref s) = self.serial_log {
            let _ = tokio::fs::remove_file(s).await;
        }
    }
}

impl Drop for Vm {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        let _ = std::fs::remove_file(&self.qmp_sock);
        if let Some(ref s) = self.serial_log {
            let _ = std::fs::remove_file(s);
        }
    }
}

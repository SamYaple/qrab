pub mod fixtures;
pub mod serial;

use qapi_qmp::{Qmp, QmpEvent};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use serial::SerialConsole;

static SOCKET_COUNTER: AtomicU64 = AtomicU64::new(0);

// ── Architecture ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86_64,
    I386,
    Aarch64,
    Arm,
}

impl Arch {
    pub fn all() -> &'static [Arch] {
        &[Arch::X86_64, Arch::I386, Arch::Aarch64, Arch::Arm]
    }

    pub fn binary(&self) -> PathBuf {
        let env_key = match self {
            Arch::X86_64 => "QEMU_TEST_BIN_X86_64",
            Arch::I386 => "QEMU_TEST_BIN_I386",
            Arch::Aarch64 => "QEMU_TEST_BIN_AARCH64",
            Arch::Arm => "QEMU_TEST_BIN_ARM",
        };
        if let Ok(path) = std::env::var(env_key) {
            return PathBuf::from(path);
        }
        PathBuf::from(self.default_binary_name())
    }

    fn default_binary_name(&self) -> &'static str {
        match self {
            Arch::X86_64 => "qemu-system-x86_64",
            Arch::I386 => "qemu-system-i386",
            Arch::Aarch64 => "qemu-system-aarch64",
            Arch::Arm => "qemu-system-arm",
        }
    }

    pub fn is_available(&self) -> bool {
        let bin = self.binary();
        which::which(&bin).is_ok()
    }

    /// Machine type for SeaBIOS/BIOS boot. None if arch doesn't support it.
    pub fn bios_machine(&self) -> Option<&'static str> {
        match self {
            Arch::X86_64 | Arch::I386 => Some("pc"),
            Arch::Aarch64 | Arch::Arm => None,
        }
    }

    /// Machine type for UEFI boot.
    pub fn uefi_machine(&self) -> &'static str {
        match self {
            Arch::X86_64 | Arch::I386 => "q35",
            Arch::Aarch64 | Arch::Arm => "virt",
        }
    }

    /// CPU model to use for this architecture.
    pub fn cpu(&self) -> Option<&'static str> {
        match self {
            Arch::Aarch64 => Some("max"),
            Arch::Arm => Some("cortex-a15"),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Arch::X86_64 => "x86_64",
            Arch::I386 => "i386",
            Arch::Aarch64 => "aarch64",
            Arch::Arm => "arm",
        }
    }
}

impl std::fmt::Display for Arch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

// ── Machine config ───────────────────────────────────────────────────────────

pub enum MachineConfig {
    /// -machine none
    None,
    /// SeaBIOS boot (x86 only): -machine pc
    Bios { memory: String },
    /// UEFI boot: -machine q35/virt + firmware
    Uefi { memory: String },
}

// ── QemuBuilder ──────────────────────────────────────────────────────────────

pub struct QemuBuilder {
    arch: Arch,
    config: MachineConfig,
    drives: Vec<(PathBuf, String)>, // (file, format)
    serial: bool,
    no_shutdown: bool,
    extra_args: Vec<String>,
}

impl QemuBuilder {
    pub fn new(arch: Arch, config: MachineConfig) -> Self {
        Self {
            arch,
            config,
            drives: Vec::new(),
            serial: false,
            no_shutdown: false,
            extra_args: Vec::new(),
        }
    }

    pub fn drive(mut self, file: PathBuf, format: &str) -> Self {
        self.drives.push((file, format.to_string()));
        self
    }

    pub fn serial(mut self) -> Self {
        self.serial = true;
        self
    }

    pub fn no_shutdown(mut self) -> Self {
        self.no_shutdown = true;
        self
    }

    #[allow(dead_code)]
    pub fn arg(mut self, a: impl Into<String>) -> Self {
        self.extra_args.push(a.into());
        self
    }

    pub async fn spawn(self) -> anyhow::Result<TestQemu> {
        let id = SOCKET_COUNTER.fetch_add(1, Ordering::SeqCst);
        let tmpdir = std::env::temp_dir();
        let qmp_sock = tmpdir.join(format!("qrab-qmp-{}-{}.sock", std::process::id(), id));
        let serial_sock = if self.serial {
            Some(tmpdir.join(format!("qrab-serial-{}-{}.sock", std::process::id(), id)))
        } else {
            Option::None
        };

        let _ = tokio::fs::remove_file(&qmp_sock).await;
        if let Some(ref s) = serial_sock {
            let _ = tokio::fs::remove_file(s).await;
        }

        let mut args: Vec<String> = Vec::new();

        // Machine
        match &self.config {
            MachineConfig::None => {
                args.extend(["-machine".into(), "none".into()]);
            }
            MachineConfig::Bios { memory } => {
                let machine = self.arch.bios_machine()
                    .expect("BIOS boot not supported for this architecture");
                args.extend(["-machine".into(), machine.into()]);
                args.extend(["-m".into(), memory.clone()]);
            }
            MachineConfig::Uefi { memory } => {
                let machine = self.arch.uefi_machine();
                args.extend(["-machine".into(), machine.into()]);
                args.extend(["-m".into(), memory.clone()]);
                let fw = fixtures::firmware_code(self.arch);
                match self.arch {
                    Arch::X86_64 | Arch::I386 => {
                        args.extend(["-bios".into(), fw.display().to_string()]);
                    }
                    Arch::Aarch64 | Arch::Arm => {
                        args.extend([
                            "-drive".into(),
                            format!("if=pflash,format=raw,readonly=on,file={}", fw.display()),
                        ]);
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
        if let Some(ref sock) = serial_sock {
            args.extend([
                "-serial".into(),
                format!("unix:{},server=on,wait=off", sock.display()),
            ]);
        }

        // Drives
        for (file, format) in &self.drives {
            args.extend([
                "-drive".into(),
                format!("if=virtio,format={},file={}", format, file.display()),
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

        let serial = if let Some(ref sock) = serial_sock {
            // Wait for serial socket
            for _ in 0..50 {
                if sock.exists() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Some(SerialConsole::connect(sock).await?)
        } else {
            Option::None
        };

        Ok(TestQemu {
            qmp,
            events,
            serial,
            _process: QemuProcess {
                child,
                qmp_sock,
                serial_sock,
            },
        })
    }
}

// ── TestQemu ─────────────────────────────────────────────────────────────────

pub struct TestQemu {
    pub qmp: Qmp,
    pub events: mpsc::Receiver<QmpEvent>,
    pub serial: Option<SerialConsole>,
    _process: QemuProcess,
}

impl TestQemu {
    pub async fn shutdown(mut self) {
        self.qmp.shutdown().await;
    }
}

struct QemuProcess {
    child: Child,
    qmp_sock: PathBuf,
    serial_sock: Option<PathBuf>,
}

impl Drop for QemuProcess {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        let _ = std::fs::remove_file(&self.qmp_sock);
        if let Some(ref s) = self.serial_sock {
            let _ = std::fs::remove_file(s);
        }
    }
}

// ── Version helpers ──────────────────────────────────────────────────────────

/// Detect the QEMU version for a given architecture by running `--version`.
pub fn detect_version(arch: Arch) -> Option<(u32, u32, u32)> {
    let output = std::process::Command::new(arch.binary())
        .arg("--version")
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse "QEMU emulator version X.Y.Z"
    let version_str = stdout.split_whitespace()
        .find(|s| s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
              && s.contains('.'))?;
    let parts: Vec<&str> = version_str.split('.').collect();
    let major = parts.first()?.parse().ok()?;
    let minor = parts.get(1)?.parse().ok()?;
    let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    Some((major, minor, patch))
}

// ── Event helpers ────────────────────────────────────────────────────────────

/// Wait for a QMP event with a specific name.
pub async fn wait_for_event(
    events: &mut mpsc::Receiver<QmpEvent>,
    event_name: &str,
    timeout: Duration,
) -> Option<QmpEvent> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match tokio::time::timeout_at(deadline, events.recv()).await {
            Ok(Some(evt)) if evt.event == event_name => return Some(evt),
            Ok(Some(_)) => continue,
            _ => return Option::None,
        }
    }
}

/// Wait for a typed QMP event, deserializing the data field.
#[allow(dead_code)]
pub async fn wait_for_typed_event<T: qapi::QapiEvent + serde::de::DeserializeOwned>(
    events: &mut mpsc::Receiver<QmpEvent>,
    timeout: Duration,
) -> Option<T> {
    let evt = wait_for_event(events, T::NAME, timeout).await?;
    // Events with no data field default to Value::Null via serde(default).
    // Treat null as empty object for struct deserialization.
    let data = if evt.data.is_null() {
        serde_json::Value::Object(serde_json::Map::new())
    } else {
        evt.data
    };
    serde_json::from_value(data).ok()
}

/// Event/operation timeout from env or default 30s.
pub fn event_timeout() -> Duration {
    let secs: u64 = std::env::var("QEMU_TEST_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    Duration::from_secs(secs)
}

/// Boot timeout from env or default 30s.
pub fn boot_timeout() -> Duration {
    let secs: u64 = std::env::var("QEMU_TEST_BOOT_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    Duration::from_secs(secs)
}

/// Set QEMU version to 99.0.0 for non-QEMU serde tests.
/// Safe to call multiple times (OnceLock ignores subsequent calls).
pub fn ensure_version_set() {
    qapi::set_qemu_version(99, 0, 0);
}

// ── Test generation macros ───────────────────────────────────────────────────

/// Generate a test function for each specified architecture.
///
/// Each generated function checks if the architecture binary is available
/// and skips (returns early) if not.
///
/// Usage:
/// ```ignore
/// arch_tests!(test_name, [X86_64, Aarch64], |arch| async move {
///     let mut t = QemuBuilder::new(arch, MachineConfig::None).spawn().await.unwrap();
///     // ... test body ...
///     t.shutdown().await;
/// });
/// ```
#[macro_export]
macro_rules! arch_tests {
    ($name:ident, [$($arch:ident),+ $(,)?], $body:expr) => {
        $(
            paste::paste! {
                #[tokio::test]
                async fn [<$name _ $arch:lower>]() {
                    use common::*;
                    let arch = Arch::$arch;
                    if !arch.is_available() {
                        eprintln!("SKIP: {} not available ({})", arch, arch.binary().display());
                        return;
                    }
                    ($body)(arch).await;
                }
            }
        )+
    };
}

/// Like arch_tests! but also checks QEMU version.
/// Usage: `arch_version_tests!(name, [X86_64], >= (5, 2), |arch| async move { ... });`
#[macro_export]
macro_rules! arch_version_tests {
    ($name:ident, [$($arch:ident),+ $(,)?], >= ($maj:expr, $min:expr), $body:expr) => {
        $(
            paste::paste! {
                #[tokio::test]
                async fn [<$name _ $arch:lower>]() {
                    use common::*;
                    let arch = Arch::$arch;
                    if !arch.is_available() {
                        eprintln!("SKIP: {} not available ({})", arch, arch.binary().display());
                        return;
                    }
                    if let Some(v) = detect_version(arch) {
                        if v < ($maj, $min, 0) {
                            eprintln!("SKIP: {} is v{}.{}.{}, need >= {}.{}", arch, v.0, v.1, v.2, $maj, $min);
                            return;
                        }
                    }
                    ($body)(arch).await;
                }
            }
        )+
    };
}

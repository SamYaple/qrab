use std::path::PathBuf;

/// Supported QEMU target architectures.
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

    /// Path to the QEMU binary for this architecture.
    /// Checks env var `QEMU_TEST_BIN_{ARCH}` first, then falls back to the default name.
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

    /// Whether the QEMU binary for this arch is installed and in PATH.
    pub fn is_available(&self) -> bool {
        which::which(self.binary()).is_ok()
    }

    /// Machine type for SeaBIOS/BIOS boot. None if not supported.
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

    /// CPU model for this architecture, if one must be specified.
    pub fn cpu(&self) -> Option<&'static str> {
        match self {
            Arch::Aarch64 => Some("max"),
            Arch::Arm => Some("cortex-a15"),
            _ => None,
        }
    }

    /// Short label for this architecture.
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

/// Machine configuration presets.
pub enum MachineConfig {
    /// Minimal: `-machine none`
    None,
    /// SeaBIOS boot (x86 only): `-machine pc`
    Bios { memory: String },
    /// UEFI boot: `-machine q35/virt` + firmware
    Uefi { memory: String },
}

/// Detect the QEMU version for a given architecture by running `--version`.
pub fn detect_version(arch: Arch) -> Option<(u32, u32, u32)> {
    let output = std::process::Command::new(arch.binary())
        .arg("--version")
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let version_str = stdout
        .split_whitespace()
        .find(|s| {
            s.chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
                && s.contains('.')
        })?;
    let parts: Vec<&str> = version_str.split('.').collect();
    let major = parts.first()?.parse().ok()?;
    let minor = parts.get(1)?.parse().ok()?;
    let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    Some((major, minor, patch))
}

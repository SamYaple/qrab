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

    /// Machine type string for SecureBoot (same as UEFI but with SMM).
    pub fn secure_boot_machine(&self) -> Option<&'static str> {
        match self {
            Arch::X86_64 | Arch::I386 => Some("q35"),
            _ => None, // SecureBoot only on x86
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
    /// UEFI Secure Boot: q35 + SMM + secure pflash (x86 only)
    SecureBoot { memory: String },
}

/// Detect the QEMU version for a given architecture by running `--version`.
pub fn detect_version(arch: Arch) -> Option<(u32, u32, u32)> {
    let output = std::process::Command::new(arch.binary())
        .arg("--version")
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_version_string(&stdout)
}

/// Parse a QEMU `--version` output (or any containing string) into
/// `(major, minor, patch)`. Designed to be resilient against the various
/// forms QEMU prints: release strings (`"9.1.0"`), dev-build strings with
/// suffixes (`"9.1.50-dev"`, `"9.1.50-dirty"`), strings with git info
/// appended in parentheses (`"9.1.50 (v9.1.0-123-gabcdef)"`), and the
/// full preamble (`"QEMU emulator version 9.1.0"`).
///
/// Strategy: locate the first whitespace-separated token that starts with
/// a digit and contains a `.`, then walk it by `.`-separated components,
/// taking only the leading digit run of each component. Missing patch
/// defaults to 0. Returns `None` only when no usable major version can be
/// extracted.
fn parse_version_string(s: &str) -> Option<(u32, u32, u32)> {
    let token = s.split_whitespace().find(|w| {
        w.chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
            && w.contains('.')
    })?;
    let mut parts = token.split('.');
    let major = parse_leading_digits(parts.next()?)?;
    let minor = parts.next().and_then(parse_leading_digits).unwrap_or(0);
    let patch = parts.next().and_then(parse_leading_digits).unwrap_or(0);
    Some((major, minor, patch))
}

/// Parse the leading run of ASCII digits from `s` as a `u32`. Returns
/// `None` if `s` does not start with a digit. Stops at the first
/// non-digit, so inputs like `"50-dev"` become `50` and `"0-rc1"`
/// becomes `0`.
fn parse_leading_digits(s: &str) -> Option<u32> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if end == 0 {
        return None;
    }
    s[..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::parse_version_string;

    #[test]
    fn parse_release_string() {
        assert_eq!(parse_version_string("9.1.0"), Some((9, 1, 0)));
        assert_eq!(parse_version_string("9.1.50"), Some((9, 1, 50)));
    }

    #[test]
    fn parse_rc_suffix() {
        assert_eq!(parse_version_string("9.2.0-rc1"), Some((9, 2, 0)));
    }

    #[test]
    fn parse_with_preamble() {
        assert_eq!(
            parse_version_string("QEMU emulator version 9.1.0"),
            Some((9, 1, 0))
        );
    }

    #[test]
    fn parse_dev_suffix() {
        assert_eq!(parse_version_string("9.1.50-dev"), Some((9, 1, 50)));
        assert_eq!(parse_version_string("9.1.50-dirty"), Some((9, 1, 50)));
    }

    #[test]
    fn parse_with_git_info() {
        assert_eq!(
            parse_version_string("9.1.50 (v9.1.0-123-gabcdef)"),
            Some((9, 1, 50))
        );
        assert_eq!(
            parse_version_string("9.1.0 (v9.1.0-dirty)"),
            Some((9, 1, 0))
        );
    }

    #[test]
    fn parse_full_qemu_preamble() {
        assert_eq!(
            parse_version_string(
                "QEMU emulator version 9.1.50 (v9.1.0-123-gabcdef)\nCopyright ...",
            ),
            Some((9, 1, 50))
        );
    }

    #[test]
    fn parse_missing_patch() {
        assert_eq!(parse_version_string("9.1"), Some((9, 1, 0)));
    }

    #[test]
    fn parse_no_version() {
        assert_eq!(parse_version_string("no version here"), None);
        assert_eq!(parse_version_string(""), None);
    }
}

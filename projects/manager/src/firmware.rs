use anyhow::Result;
use std::path::{Path, PathBuf};

/// Discovered OVMF firmware paths.
#[derive(Debug, Clone)]
pub struct OvmfFirmware {
    /// OVMF code image (read-only pflash)
    pub code: PathBuf,
    /// OVMF vars template (copied per-VM for writable pflash)
    pub vars_template: PathBuf,
}

/// Search system paths for OVMF firmware.
///
/// SecureBoot firmware is preferred when `secure` is true. Falls back
/// to standard OVMF if secure variant not found.
pub fn find_ovmf(secure: bool) -> Option<OvmfFirmware> {
    // Try secure code paths first when requested, then standard.
    let code_search: Vec<&[&str]> = if secure {
        vec![OVMF_SECURE_CODE_PATHS, OVMF_CODE_PATHS]
    } else {
        vec![OVMF_CODE_PATHS]
    };

    for code_paths in code_search {
        for &code_path in code_paths {
            let code = Path::new(code_path);
            if !code.is_file() {
                continue;
            }
            // Found a code image — now find matching vars template.
            for &vars_path in OVMF_VARS_PATHS {
                let vars = Path::new(vars_path);
                if vars.is_file() {
                    return Some(OvmfFirmware {
                        code: code.to_path_buf(),
                        vars_template: vars.to_path_buf(),
                    });
                }
            }
        }
    }

    None
}

/// Search system paths for AMD SEV-SNP OVMF firmware.
///
/// The AmdSev OVMF variant is a single combined firmware image (no
/// separate CODE/VARS split) built from `OvmfPkg/AmdSev/AmdSevX64.dsc`.
/// It contains the SEV metadata GUID table required for SNP launch.
/// Returns the path to the combined OVMF.fd if found.
pub fn find_amdsev_ovmf() -> Option<PathBuf> {
    for &path in AMDSEV_OVMF_PATHS {
        let p = Path::new(path);
        if p.is_file() {
            return Some(p.to_path_buf());
        }
    }
    None
}

/// AMD SEV-SNP OVMF firmware search paths.
const AMDSEV_OVMF_PATHS: &[&str] = &[
    // Gentoo (edk2-amdsev package)
    "/usr/share/edk2/AmdSev/OVMF.fd",
    // Fedora/RHEL
    "/usr/share/edk2/ovmf/OVMF.amdsev.fd",
    "/usr/share/OVMF/OVMF.amdsev.fd",
    // Ubuntu/Debian
    "/usr/share/OVMF/OVMF_CODE.amdsev.fd",
    // Common build output location
    "/usr/share/qemu/OVMF.amdsev.fd",
];

/// Common OVMF code image search paths (non-SecureBoot).
const OVMF_CODE_PATHS: &[&str] = &[
    // Gentoo
    "/usr/share/edk2-ovmf/OVMF_CODE.fd",
    // Ubuntu/Debian
    "/usr/share/OVMF/OVMF_CODE_4M.fd",
    "/usr/share/OVMF/OVMF_CODE.fd",
    // Fedora/RHEL
    "/usr/share/edk2/ovmf/OVMF_CODE.fd",
    // QEMU bundled
    "/usr/share/qemu/edk2-x86_64-code.fd",
];

/// SecureBoot OVMF code image search paths.
const OVMF_SECURE_CODE_PATHS: &[&str] = &[
    // Gentoo
    "/usr/share/edk2-ovmf/OVMF_CODE.secboot.fd",
    // Ubuntu/Debian
    "/usr/share/OVMF/OVMF_CODE_4M.secboot.fd",
    "/usr/share/OVMF/OVMF_CODE.secboot.fd",
    // Fedora/RHEL
    "/usr/share/edk2/ovmf/OVMF_CODE.secboot.fd",
    // QEMU bundled
    "/usr/share/qemu/edk2-x86_64-secure-code.fd",
];

/// OVMF vars template search paths.
const OVMF_VARS_PATHS: &[&str] = &[
    // Gentoo
    "/usr/share/edk2-ovmf/OVMF_VARS.fd",
    // Ubuntu/Debian
    "/usr/share/OVMF/OVMF_VARS_4M.fd",
    "/usr/share/OVMF/OVMF_VARS.fd",
    // Fedora/RHEL
    "/usr/share/edk2/ovmf/OVMF_VARS.fd",
    // QEMU bundled
    "/usr/share/qemu/edk2-i386-vars.fd",
];

/// Prepare per-VM OVMF vars by copying the template.
///
/// Each VM needs its own writable copy of OVMF_VARS.fd because the
/// UEFI variable store is modified at runtime (boot order, SecureBoot
/// keys, etc.). The copy is placed in the VM's tmpdir and cleaned up
/// automatically.
pub fn prepare_vars_copy(template: &Path, tmpdir: &Path) -> Result<PathBuf> {
    let dest = tmpdir.join("OVMF_VARS.fd");
    std::fs::copy(template, &dest)?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn prepare_vars_copy_creates_file() {
        let src_dir = tempfile::tempdir().unwrap();
        let template = src_dir.path().join("OVMF_VARS.fd");
        fs::write(&template, b"fake vars data").unwrap();

        let dst_dir = tempfile::tempdir().unwrap();
        let result = prepare_vars_copy(&template, dst_dir.path()).unwrap();

        assert!(result.is_file());
        assert_eq!(fs::read(&result).unwrap(), b"fake vars data");
    }

    #[test]
    fn prepare_vars_copy_fails_missing_template() {
        let dst_dir = tempfile::tempdir().unwrap();
        let result = prepare_vars_copy(Path::new("/nonexistent/OVMF_VARS.fd"), dst_dir.path());
        assert!(result.is_err());
    }
}

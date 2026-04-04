use super::Arch;
use std::path::PathBuf;

/// Cache directory for test fixtures.
fn fixtures_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap()
        .join("target")
        .join("test-fixtures");
    std::fs::create_dir_all(&dir).expect("Failed to create test-fixtures directory");
    dir
}

/// Download cirros disk image for the given architecture. Cached.
/// Only x86_64 and aarch64 are supported.
pub async fn cirros_image(arch: Arch) -> PathBuf {
    let filename = match arch {
        Arch::X86_64 | Arch::I386 => "cirros-0.6.2-x86_64-disk.img",
        Arch::Aarch64 => "cirros-0.6.2-aarch64-disk.img",
        Arch::Arm => panic!("cirros boot tests not supported for arm 32-bit"),
    };

    let path = fixtures_dir().join(filename);
    if path.exists() {
        return path;
    }

    let url = format!("https://download.cirros-cloud.net/0.6.2/{}", filename);
    eprintln!("Downloading {} ...", url);

    let output = tokio::process::Command::new("curl")
        .args(["-fSL", "-o", &path.display().to_string(), &url])
        .output()
        .await
        .expect("Failed to run curl");

    assert!(
        output.status.success(),
        "Failed to download cirros image: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(path.exists(), "Downloaded file not found: {}", path.display());
    eprintln!("Downloaded to {}", path.display());
    path
}

/// Create a blank qcow2 image. Cached by name.
pub async fn blank_qcow2(name: &str, size_mb: u64) -> PathBuf {
    let path = fixtures_dir().join(format!("{}.qcow2", name));
    if path.exists() {
        return path;
    }

    let output = tokio::process::Command::new("qemu-img")
        .args(["create", "-f", "qcow2", &path.display().to_string(), &format!("{}M", size_mb)])
        .output()
        .await
        .expect("Failed to run qemu-img");

    assert!(
        output.status.success(),
        "Failed to create qcow2 image: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    path
}

/// Resolve UEFI firmware code path for an architecture.
/// Returns None if not found (caller should skip the test).
pub fn find_firmware(arch: Arch) -> Option<PathBuf> {
    // Check arch-specific env var first
    let env_key = match arch {
        Arch::X86_64 => "QEMU_TEST_OVMF_X86_64",
        Arch::I386 => "QEMU_TEST_OVMF_I386",
        Arch::Aarch64 => "QEMU_TEST_OVMF_AARCH64",
        Arch::Arm => "QEMU_TEST_OVMF_ARM",
    };
    if let Ok(path) = std::env::var(env_key) {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Some(p);
        }
    }

    // Check generic env var
    if let Ok(path) = std::env::var("QEMU_TEST_OVMF") {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Some(p);
        }
    }

    // Search common system paths
    let candidates: &[&str] = match arch {
        Arch::X86_64 => &[
            "/usr/share/edk2-ovmf/OVMF_CODE.fd",
            "/usr/share/OVMF/OVMF_CODE.fd",
            "/usr/share/qemu/edk2-x86_64-code.fd",
        ],
        Arch::I386 => &[
            "/usr/share/qemu/edk2-i386-code.fd",
        ],
        Arch::Aarch64 => &[
            "/usr/share/qemu/edk2-aarch64-code.fd",
        ],
        Arch::Arm => &[
            "/usr/share/qemu/edk2-arm-code.fd",
        ],
    };

    for path in candidates {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    None
}

/// Resolve UEFI firmware code path. Panics if not found.
pub fn firmware_code(arch: Arch) -> PathBuf {
    find_firmware(arch).unwrap_or_else(|| {
        panic!(
            "UEFI firmware not found for {}. Set QEMU_TEST_OVMF_{} or install edk2/OVMF.",
            arch,
            arch.label().to_uppercase()
        );
    })
}

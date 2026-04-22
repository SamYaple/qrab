use super::Arch;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Cache directory for test fixtures.
fn fixtures_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target")
        .join("test-fixtures");
    std::fs::create_dir_all(&dir).expect("Failed to create test-fixtures directory");
    dir
}

/// Download cirros disk image. Cached. x86_64 only — the scope-
/// tightening round dropped non-x86_64 `Arch` variants.
pub async fn cirros_image(_arch: Arch) -> PathBuf {
    let filename = "cirros-0.6.2-x86_64-disk.img";

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

    assert!(
        path.exists(),
        "Downloaded file not found: {}",
        path.display()
    );
    eprintln!("Downloaded to {}", path.display());
    path
}

/// Create a temporary copy of the cirros image for a single test.
/// This avoids file locking conflicts when multiple tests run in parallel.
pub async fn cirros_image_copy(arch: Arch) -> PathBuf {
    let source = cirros_image(arch).await;
    let id = FIXTURE_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dest = std::env::temp_dir().join(format!(
        "qrab-cirros-{}-{}-{}.img",
        arch.label(),
        std::process::id(),
        id
    ));
    tokio::fs::copy(&source, &dest)
        .await
        .expect("Failed to copy cirros image");
    dest
}

/// Download cirros kernel for direct boot. Cached. x86_64 only.
pub async fn cirros_kernel(_arch: Arch) -> PathBuf {
    download_cirros_file("cirros-0.6.2-x86_64-kernel").await
}

/// Download cirros initramfs for direct boot. Cached. x86_64 only.
pub async fn cirros_initramfs(_arch: Arch) -> PathBuf {
    download_cirros_file("cirros-0.6.2-x86_64-initramfs").await
}

async fn download_cirros_file(filename: &str) -> PathBuf {
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
        "Failed to download {}: {}",
        filename,
        String::from_utf8_lossy(&output.stderr)
    );
    path
}

/// Create a NoCloud CIDATA FAT image with instance metadata.
/// This tells cirros's cloud-init to skip the EC2 metadata service.
/// Cached — only created once.
pub async fn nocloud_cidata() -> PathBuf {
    let path = fixtures_dir().join("cidata.img");
    if path.exists() {
        return path;
    }

    // Create a 1MB FAT image with mkfs.vfat, then add files
    let output = tokio::process::Command::new("dd")
        .args([
            "if=/dev/zero",
            &format!("of={}", path.display()),
            "bs=1M",
            "count=1",
        ])
        .output()
        .await
        .expect("Failed to run dd");
    assert!(output.status.success(), "dd failed");

    let output = tokio::process::Command::new("mkfs.vfat")
        .args(["-n", "CIDATA", &path.display().to_string()])
        .output()
        .await
        .expect("Failed to run mkfs.vfat");
    assert!(
        output.status.success(),
        "mkfs.vfat failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Add meta-data and user-data files to the FAT image
    let meta_data = "instance-id: qrab-test\nlocal-hostname: qrab-test\n";
    let user_data =
        "#cloud-config\npassword: test\nchpasswd: { expire: False }\nssh_pwauth: False\n";
    add_files_to_fat(&path, &[("META-DAT", meta_data), ("USER-DAT", user_data)]);

    path
}

/// Write files into a FAT12/16 image created by mkfs.vfat.
/// Filenames must be 8.3 format (uppercase, no long filenames).
fn add_files_to_fat(img_path: &std::path::Path, files: &[(&str, &str)]) {
    let mut data = std::fs::read(img_path).expect("Failed to read FAT image");

    let bytes_per_sector = u16::from_le_bytes([data[11], data[12]]) as usize;
    let sectors_per_cluster = data[13] as usize;
    let reserved_sectors = u16::from_le_bytes([data[14], data[15]]) as usize;
    let num_fats = data[16] as usize;
    let root_entries = u16::from_le_bytes([data[17], data[18]]) as usize;
    let sectors_per_fat = u16::from_le_bytes([data[22], data[23]]) as usize;

    let root_dir_offset = (reserved_sectors + num_fats * sectors_per_fat) * bytes_per_sector;
    let root_dir_size = root_entries * 32;
    let data_area_offset = root_dir_offset + root_dir_size;
    let cluster_size = sectors_per_cluster * bytes_per_sector;
    let fat_offset = reserved_sectors * bytes_per_sector;

    // Find first free root directory entry (skip volume label)
    let mut entry_offset = root_dir_offset;
    while entry_offset < root_dir_offset + root_dir_size {
        if data[entry_offset] == 0x00 || data[entry_offset] == 0xE5 {
            break;
        }
        entry_offset += 32;
    }

    let mut next_cluster: usize = 2;
    // Skip used clusters
    loop {
        let fo = fat_offset + (next_cluster * 3) / 2;
        let val = if next_cluster % 2 == 0 {
            (data[fo] as u16 | ((data[fo + 1] as u16) << 8)) & 0xFFF
        } else {
            ((data[fo] as u16 | ((data[fo + 1] as u16) << 8)) >> 4) & 0xFFF
        };
        if val == 0 {
            break;
        }
        next_cluster += 1;
    }

    for (name, content) in files {
        let content_bytes = content.as_bytes();
        // 8.3 filename: "META-DAT" -> "META-DAT   " (8 chars name, 3 chars ext)
        let padded = format!("{:<8}{:<3}", &name[..name.len().min(8)], "");
        let short_name = &padded.as_bytes()[..11];

        // Directory entry
        let mut entry = [0u8; 32];
        entry[..11].copy_from_slice(short_name);
        entry[11] = 0x20; // archive
        entry[26..28].copy_from_slice(&(next_cluster as u16).to_le_bytes());
        entry[28..32].copy_from_slice(&(content_bytes.len() as u32).to_le_bytes());
        data[entry_offset..entry_offset + 32].copy_from_slice(&entry);
        entry_offset += 32;

        // Write file data
        let file_offset = data_area_offset + (next_cluster - 2) * cluster_size;
        data[file_offset..file_offset + content_bytes.len()].copy_from_slice(content_bytes);

        // Mark cluster as end-of-chain in FAT
        let fo = fat_offset + (next_cluster * 3) / 2;
        if next_cluster % 2 == 0 {
            data[fo] = 0xFF;
            data[fo + 1] = (data[fo + 1] & 0xF0) | 0x0F;
        } else {
            data[fo] = (data[fo] & 0x0F) | 0xF0;
            data[fo + 1] = 0xFF;
        }

        next_cluster += (content_bytes.len() + cluster_size - 1) / cluster_size;
        next_cluster = next_cluster.max(next_cluster); // skip to next
    }

    std::fs::write(img_path, &data).expect("Failed to write FAT image");
}

/// Create a blank qcow2 image. Cached by name.
pub async fn blank_qcow2(name: &str, size_mb: u64) -> PathBuf {
    let path = fixtures_dir().join(format!("{}.qcow2", name));
    if path.exists() {
        return path;
    }

    let output = tokio::process::Command::new("qemu-img")
        .args([
            "create",
            "-f",
            "qcow2",
            &path.display().to_string(),
            &format!("{}M", size_mb),
        ])
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

/// Resolve UEFI firmware code path. Returns None if not found
/// (caller should skip the test). x86_64 only post-tightening.
pub fn find_firmware(_arch: Arch) -> Option<PathBuf> {
    if let Ok(path) = std::env::var("QEMU_TEST_OVMF_X86_64") {
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
    let candidates: &[&str] = &[
        "/usr/share/edk2-ovmf/OVMF_CODE.fd",
        "/usr/share/OVMF/OVMF_CODE.fd",
        "/usr/share/qemu/edk2-x86_64-code.fd",
    ];

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

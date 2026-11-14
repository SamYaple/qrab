//! Guest operating-system effects behind a trait so the protocol and freeze
//! state machine can be tested without CAP_SYS_ADMIN.
use std::{
    fs::File,
    io,
    os::fd::AsRawFd,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    pub dir: PathBuf,
    pub fstype: String,
    pub device: (u32, u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownMode {
    Powerdown,
    Halt,
    Reboot,
}

pub trait Platform: Send + Sync + 'static {
    fn secure_boot_update(
        &self,
        _request: crate::secure_boot::QrabSecureBootUpdate,
    ) -> io::Result<crate::secure_boot::SecureBootDatabases> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "authenticated EFI variable updates are unavailable",
        ))
    }

    fn secure_boot_databases(&self) -> io::Result<crate::secure_boot::SecureBootDatabases> {
        Ok(crate::secure_boot::SecureBootDatabases::default())
    }

    fn boot_state(&self) -> io::Result<crate::boot::BootState> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "UEFI boot inventory is unavailable",
        ))
    }
    fn boot_next(&self, _reference: Option<String>) -> io::Result<crate::boot::BootState> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "UEFI BootNext management is unavailable",
        ))
    }

    fn firmware_state(&self) -> io::Result<crate::proto::FirmwareState> {
        Ok(crate::proto::FirmwareState::default())
    }

    /// Interfaces and counters observed inside the guest network namespace.
    fn network_interfaces(&self) -> io::Result<Vec<crate::proto::NetworkInterface>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "network inventory is unavailable",
        ))
    }

    /// Freezable local filesystems in mount order, one entry per superblock.
    fn mounts(&self) -> io::Result<Vec<Mount>>;
    /// `Unsupported` means the filesystem cannot freeze and is skipped.
    fn freeze(&self, mount: &Mount) -> io::Result<()>;
    /// An error means this call did not thaw the filesystem.
    fn thaw(&self, mount: &Mount) -> io::Result<()>;
    /// Returns (bytes trimmed, effective minimum extent).
    fn trim(&self, mount: &Mount, minimum: u64) -> io::Result<(u64, u64)>;
    /// Called after the request has been accepted; it need not return.
    fn shutdown(&self, mode: ShutdownMode) -> io::Result<()>;
}

/// Parse `/proc/self/mountinfo`, keeping only block-backed filesystems.
/// Pseudo filesystems (proc, sysfs, tmpfs, devtmpfs, overlay, network
/// filesystems) have device major 0; btrfs is the one real exception.
pub fn parse_mountinfo(text: &str) -> Vec<Mount> {
    let mut mounts: Vec<Mount> = Vec::new();
    for line in text.lines() {
        let Some((left, right)) = line.split_once(" - ") else {
            continue;
        };
        let left: Vec<_> = left.split(' ').collect();
        let mut right = right.split(' ');
        let (Some(device), Some(dir), Some(fstype)) = (left.get(2), left.get(4), right.next())
        else {
            continue;
        };
        let Some((major, minor)) = device
            .split_once(':')
            .and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)))
        else {
            continue;
        };
        if major == 0 && fstype != "btrfs" {
            continue;
        }
        let mount = Mount {
            dir: PathBuf::from(unescape_mount(dir)),
            fstype: fstype.to_owned(),
            device: (major, minor),
        };
        // Bind mounts share a superblock; freezing it twice would fail EBUSY.
        if !mounts.iter().any(|m| m.device == mount.device) {
            mounts.push(mount);
        }
    }
    mounts
}

fn unescape_mount(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\'
            && i + 3 < bytes.len()
            && bytes[i + 1..i + 4]
                .iter()
                .all(|b| (b'0'..=b'7').contains(b))
        {
            let value = bytes[i + 1..i + 4]
                .iter()
                .fold(0u32, |acc, b| acc * 8 + u32::from(b - b'0'));
            if let Ok(value) = u8::try_from(value) {
                out.push(value);
                i += 4;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

const FIFREEZE: u32 = 0xc004_5877;
const FITHAW: u32 = 0xc004_5878;
const FITRIM: u32 = 0xc018_5879;

#[repr(C)]
struct FstrimRange {
    start: u64,
    len: u64,
    minlen: u64,
}

pub struct Linux {
    pub mountinfo: PathBuf,
}

impl Default for Linux {
    fn default() -> Self {
        Self {
            mountinfo: "/proc/self/mountinfo".into(),
        }
    }
}

fn open_dir(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
}

fn ioctl_int(file: &File, request: u32) -> io::Result<()> {
    let mut unused: libc::c_int = 0;
    // SAFETY: FIFREEZE/FITHAW take an int argument that the kernel ignores;
    // the pointer is valid for the duration of the call.
    let result = unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            request as libc::Ioctl,
            &mut unused as *mut libc::c_int,
        )
    };
    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

impl Platform for Linux {
    fn secure_boot_update(
        &self,
        request: crate::secure_boot::QrabSecureBootUpdate,
    ) -> io::Result<crate::secure_boot::SecureBootDatabases> {
        crate::secure_boot::authenticated_update(Path::new("/sys/firmware/efi"), request)
    }

    fn secure_boot_databases(&self) -> io::Result<crate::secure_boot::SecureBootDatabases> {
        crate::secure_boot::databases(Path::new("/sys/firmware/efi"))
    }

    fn boot_state(&self) -> io::Result<crate::boot::BootState> {
        crate::boot::state()
    }
    fn boot_next(&self, reference: Option<String>) -> io::Result<crate::boot::BootState> {
        crate::boot::set_next(reference)
    }

    fn firmware_state(&self) -> io::Result<crate::proto::FirmwareState> {
        crate::firmware::state(Path::new("/sys/firmware/efi"))
    }

    fn network_interfaces(&self) -> io::Result<Vec<crate::proto::NetworkInterface>> {
        crate::network::interfaces()
    }

    fn mounts(&self) -> io::Result<Vec<Mount>> {
        Ok(parse_mountinfo(&std::fs::read_to_string(&self.mountinfo)?))
    }

    fn freeze(&self, mount: &Mount) -> io::Result<()> {
        match ioctl_int(&open_dir(&mount.dir)?, FIFREEZE) {
            Err(e) if e.raw_os_error() == Some(libc::EOPNOTSUPP) => {
                Err(io::Error::new(io::ErrorKind::Unsupported, e))
            }
            other => other,
        }
    }

    fn thaw(&self, mount: &Mount) -> io::Result<()> {
        ioctl_int(&open_dir(&mount.dir)?, FITHAW)
    }

    fn trim(&self, mount: &Mount, minimum: u64) -> io::Result<(u64, u64)> {
        let file = open_dir(&mount.dir)?;
        let mut range = FstrimRange {
            start: 0,
            len: u64::MAX,
            minlen: minimum,
        };
        // SAFETY: FITRIM reads and updates exactly one fstrim_range.
        let result = unsafe {
            libc::ioctl(
                file.as_raw_fd(),
                FITRIM as libc::Ioctl,
                &mut range as *mut FstrimRange,
            )
        };
        if result == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok((range.len, range.minlen))
    }

    fn shutdown(&self, mode: ShutdownMode) -> io::Result<()> {
        let command = match mode {
            ShutdownMode::Powerdown => libc::LINUX_REBOOT_CMD_POWER_OFF,
            ShutdownMode::Halt => libc::LINUX_REBOOT_CMD_HALT,
            ShutdownMode::Reboot => libc::LINUX_REBOOT_CMD_RESTART,
        };
        // SAFETY: sync has no preconditions. reboot only returns on failure.
        unsafe {
            libc::sync();
            if libc::reboot(command) == -1 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }
}

pub fn uname() -> Option<(String, String, String, String)> {
    // SAFETY: utsname is plain old data and uname fills it completely.
    let mut name: libc::utsname = unsafe { std::mem::zeroed() };
    // SAFETY: the pointer is valid for writing a utsname.
    if unsafe { libc::uname(&mut name) } != 0 {
        return None;
    }
    let field = |raw: &[libc::c_char]| {
        let bytes: Vec<u8> = raw
            .iter()
            .take_while(|c| **c != 0)
            .map(|c| *c as u8)
            .collect();
        String::from_utf8_lossy(&bytes).into_owned()
    };
    Some((
        field(&name.nodename),
        field(&name.release),
        field(&name.version),
        field(&name.machine),
    ))
}

/// Minimal os-release(5) value parsing: optional single or double quotes and
/// backslash escapes inside double quotes. No shell expansion is performed.
pub fn parse_os_release(text: &str) -> std::collections::HashMap<String, String> {
    let mut values = std::collections::HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.is_empty()
            || !key
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        {
            continue;
        }
        let value = if let Some(inner) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
            let mut out = String::new();
            let mut chars = inner.chars();
            while let Some(c) = chars.next() {
                if c == '\\' {
                    if let Some(next) = chars.next() {
                        out.push(next);
                    }
                } else {
                    out.push(c);
                }
            }
            out
        } else if let Some(inner) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
            inner.to_owned()
        } else {
            value.to_owned()
        };
        values.insert(key.to_owned(), value);
    }
    values
}

/// Whether `path` is a hook the agent may execute: a regular, executable,
/// non-symlink file, and (with `require_root`) it and its directory are owned
/// by root and writable by nobody else.
pub fn check_hook(path: &Path, require_root: bool) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(format!("no update hook is installed at {}", path.display()))
        }
        Err(e) => return Err(format!("cannot inspect {}: {e}", path.display())),
    };
    if !meta.file_type().is_file() {
        return Err(format!("{} is not a regular file", path.display()));
    }
    if meta.mode() & 0o100 == 0 {
        return Err(format!("{} is not executable", path.display()));
    }
    if require_root {
        let parent = path.parent().unwrap_or(Path::new("/"));
        let dir = std::fs::symlink_metadata(parent)
            .map_err(|e| format!("cannot inspect {}: {e}", parent.display()))?;
        for (what, meta) in [(path, &meta), (parent, &dir)] {
            if meta.uid() != 0 || meta.mode() & 0o022 != 0 {
                return Err(format!(
                    "{} must be owned by root and not group/world writable",
                    what.display()
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mountinfo_keeps_real_filesystems_once() {
        let text = "\
22 1 0:21 / /proc rw,nosuid shared:5 - proc proc rw
23 1 0:22 / /sys rw shared:6 - sysfs sysfs rw
24 1 253:1 / / rw,relatime shared:1 - ext4 /dev/vda1 rw
25 24 253:2 / /var/lib/my\\040data rw - xfs /dev/vdb rw
26 24 0:30 / /tmp rw - tmpfs tmpfs rw
27 24 253:1 /srv /srv rw - ext4 /dev/vda1 rw
28 24 0:45 / /home rw - btrfs /dev/vdc rw
garbage line
";
        let mounts = parse_mountinfo(text);
        let dirs: Vec<_> = mounts.iter().map(|m| m.dir.to_str().unwrap()).collect();
        assert_eq!(dirs, ["/", "/var/lib/my data", "/home"]);
        assert_eq!(mounts[1].fstype, "xfs");
        assert_eq!(mounts[1].device, (253, 2));
    }

    #[test]
    fn os_release_values_are_unquoted_without_expansion() {
        let values = parse_os_release(
            "# comment\nID=debian\nNAME=\"Debian \\\"GNU\\\"/Linux\"\nPRETTY_NAME='x $y'\nbad key=1\n",
        );
        assert_eq!(values["ID"], "debian");
        assert_eq!(values["NAME"], "Debian \"GNU\"/Linux");
        assert_eq!(values["PRETTY_NAME"], "x $y");
        assert_eq!(values.len(), 3);
    }

    #[test]
    fn hook_must_be_an_executable_regular_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let hook = dir.path().join("hook");
        assert!(check_hook(&hook, false)
            .unwrap_err()
            .contains("no update hook"));
        std::fs::write(&hook, "#!/bin/sh\n").unwrap();
        assert!(check_hook(&hook, false).unwrap_err().contains("executable"));
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(check_hook(&hook, false).is_ok());
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&hook, &link).unwrap();
        assert!(check_hook(&link, false).is_err());
        // The test user is not root, so the production ownership policy refuses.
        // SAFETY: getuid has no preconditions.
        if unsafe { libc::getuid() } != 0 {
            assert!(check_hook(&hook, true)
                .unwrap_err()
                .contains("owned by root"));
        }
    }
}

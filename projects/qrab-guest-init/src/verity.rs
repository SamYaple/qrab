//! Root filesystem must be a dm-verity target (L5 step 8).
//!
//! Two sources cross-checked:
//!
//! 1. `/proc/self/mountinfo` — confirms `/` is backed by a
//!    `/dev/mapper/*` node (otherwise it's a raw block device and
//!    verity cannot be in the I/O path).
//! 2. `/sys/block/dm-<N>/dm/uuid` — confirms the dm target is built
//!    on top of the `verity` subsystem (uuids start with `CRYPT-VERITY-`).
//!
//! The parser is string-only so tests can feed fixtures; the runtime
//! wiring reads the real sysfs paths in `sequencer.rs`.

use crate::tokens::Token;

/// Extract the source device of the `/` mount from a `mountinfo` blob.
///
/// Format per `man 5 proc`:
/// > `mount-id parent-id major:minor root mount-point mount-options \
/// >  optional-fields - fstype source super-options`
///
/// We need field 9 (0-indexed 8), which is the source device, but
/// only for the mount whose mount-point (field 4 / 0-indexed 4) is
/// literally `/`.
pub fn parse_root_source(mountinfo: &str) -> Option<&str> {
    for line in mountinfo.lines() {
        let (left, right) = line.split_once(" - ")?;
        let left_fields: Vec<&str> = left.split_whitespace().collect();
        if left_fields.len() < 5 {
            continue;
        }
        if left_fields[4] != "/" {
            continue;
        }
        // Right half: `fstype source super-options`
        let mut right_iter = right.split_whitespace();
        let _fstype = right_iter.next()?;
        let source = right_iter.next()?;
        return Some(source);
    }
    None
}

/// The source device looks like a dm node if it lives under
/// `/dev/mapper/` or `/dev/dm-<N>`. Both are equivalent at the
/// kernel level; the verity check passes either way.
pub fn is_dm_source(source: &str) -> bool {
    source.starts_with("/dev/mapper/") || source.starts_with("/dev/dm-")
}

/// A sysfs dm uuid starts with a capitalized subsystem prefix
/// separated by hyphens; verity targets always use `CRYPT-VERITY-`.
/// See `Documentation/admin-guide/device-mapper/verity.rst` in the
/// kernel tree.
pub fn is_verity_uuid(uuid: &str) -> bool {
    uuid.trim().starts_with("CRYPT-VERITY-")
}

/// Compose the two checks. Takes already-read strings so the pure
/// parsing logic stays trivially testable.
pub fn check_root_verity(mountinfo: &str, dm_uuid: &str) -> Result<(), Token> {
    let Some(source) = parse_root_source(mountinfo) else {
        return Err(Token::FailVerity);
    };
    if !is_dm_source(source) {
        return Err(Token::FailVerity);
    }
    if !is_verity_uuid(dm_uuid) {
        return Err(Token::FailVerity);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VERITY_MOUNTINFO: &str = "\
22 28 0:21 / /sys rw,nosuid,nodev,noexec,relatime shared:6 - sysfs sysfs rw\n\
23 28 0:4 / /proc rw,nosuid,nodev,noexec,relatime shared:13 - proc proc rw\n\
28 1 254:0 / / ro,relatime shared:1 - ext4 /dev/mapper/root ro\n\
";

    const PLAIN_MOUNTINFO: &str = "\
28 1 253:0 / / rw,relatime shared:1 - ext4 /dev/vda2 rw\n\
";

    #[test]
    fn root_source_extracted_from_mountinfo() {
        assert_eq!(parse_root_source(VERITY_MOUNTINFO), Some("/dev/mapper/root"));
    }

    #[test]
    fn root_source_missing_when_no_root_entry() {
        let no_root = "22 28 0:21 / /sys rw shared:6 - sysfs sysfs rw\n";
        assert_eq!(parse_root_source(no_root), None);
    }

    #[test]
    fn dm_source_recognized() {
        assert!(is_dm_source("/dev/mapper/root"));
        assert!(is_dm_source("/dev/dm-0"));
        assert!(!is_dm_source("/dev/vda2"));
        assert!(!is_dm_source("/dev/nvme0n1p1"));
    }

    #[test]
    fn verity_uuid_recognized() {
        assert!(is_verity_uuid("CRYPT-VERITY-abcdef-root"));
        assert!(is_verity_uuid("CRYPT-VERITY-abcdef-root\n"));
        assert!(!is_verity_uuid("CRYPT-LUKS2-ffff-root"));
        assert!(!is_verity_uuid("LVM-bla"));
        assert!(!is_verity_uuid(""));
    }

    #[test]
    fn happy_path_passes() {
        assert!(check_root_verity(VERITY_MOUNTINFO, "CRYPT-VERITY-abcdef-root\n").is_ok());
    }

    #[test]
    fn raw_block_device_rootfs_fails() {
        assert_eq!(
            check_root_verity(PLAIN_MOUNTINFO, "CRYPT-VERITY-abcdef-root\n"),
            Err(Token::FailVerity),
        );
    }

    #[test]
    fn dm_but_not_verity_fails() {
        assert_eq!(
            check_root_verity(VERITY_MOUNTINFO, "CRYPT-LUKS2-ffff-root\n"),
            Err(Token::FailVerity),
        );
    }

    #[test]
    fn empty_inputs_fail() {
        assert_eq!(check_root_verity("", ""), Err(Token::FailVerity));
    }
}

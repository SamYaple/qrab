//! Read-only guest EFI state. Missing variables are unknown, never disabled.
use crate::proto::FirmwareState;
use std::{
    io::{self, Read},
    path::Path,
};

const EFI_GLOBAL: &str = "8be4df61-93ca-11d2-aa0d-00e098032b8c";

fn flag(variables: &Path, name: &str) -> Option<bool> {
    let mut data = Vec::new();
    std::fs::File::open(variables.join(format!("{name}-{EFI_GLOBAL}")))
        .ok()?
        .take(6)
        .read_to_end(&mut data)
        .ok()?;
    // efivarfs stores the 32-bit attributes before the UEFI UINT8 value.
    // Extra payload bytes are malformed; do not silently interpret them.
    if data.len() != 5 {
        return None;
    }
    match data[4] {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

pub(crate) fn state(efi: &Path) -> io::Result<FirmwareState> {
    match std::fs::metadata(efi) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => return Ok(FirmwareState::default()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(FirmwareState::default())
        }
        Err(error) => return Err(error),
    }
    let variables = efi.join("efivars");
    Ok(FirmwareState {
        efi_runtime_available: true,
        secure_boot: flag(&variables, "SecureBoot"),
        setup_mode: flag(&variables, "SetupMode"),
        audit_mode: flag(&variables, "AuditMode"),
        deployed_mode: flag(&variables, "DeployedMode"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_efivar_flags_distinguish_disabled_missing_and_malformed() {
        let dir = tempfile::tempdir().unwrap();
        let efi = dir.path().join("efi");
        let variables = efi.join("efivars");
        assert_eq!(state(&efi).unwrap(), FirmwareState::default());
        std::fs::create_dir_all(&variables).unwrap();
        std::fs::write(
            variables.join(format!("SecureBoot-{EFI_GLOBAL}")),
            [7, 0, 0, 0, 0],
        )
        .unwrap();
        std::fs::write(
            variables.join(format!("SetupMode-{EFI_GLOBAL}")),
            [7, 0, 0, 0, 1],
        )
        .unwrap();
        std::fs::write(
            variables.join(format!("AuditMode-{EFI_GLOBAL}")),
            [7, 0, 0, 0, 2],
        )
        .unwrap();
        std::fs::write(
            variables.join(format!("DeployedMode-{EFI_GLOBAL}")),
            [7, 0, 0, 0, 1, 0],
        )
        .unwrap();
        let state = state(&efi).unwrap();
        assert!(state.efi_runtime_available);
        assert_eq!(state.secure_boot, Some(false));
        assert_eq!(state.setup_mode, Some(true));
        assert_eq!(state.audit_mode, None);
        assert_eq!(state.deployed_mode, None);
        std::fs::remove_file(variables.join(format!("SecureBoot-{EFI_GLOBAL}"))).unwrap();
        assert_eq!(super::state(&efi).unwrap().secure_boot, None);
    }
}

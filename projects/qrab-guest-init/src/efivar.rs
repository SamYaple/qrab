//! EFI variable reads via `efivarfs`. Each file under
//! `/sys/firmware/efi/efivars/<Name>-<GUID>` starts with a 4-byte
//! little-endian attribute word, followed by the variable payload.
//! L5 needs three variables, all under the EFI global GUID
//! `8be4df61-93ca-11d2-aa0d-00e098032b8c`:
//!
//! | Variable | Meaning | L5 requirement |
//! |---|---|---|
//! | `SecureBoot` | 1 = SB enabled | **must be 1** |
//! | `DeployedMode` | 1 = operational mode | **must be 1** |
//! | `SetupMode` | 0 = keys enrolled | **must be 0** |

use crate::tokens::Token;

/// EFI global namespace GUID as it appears in the efivarfs filename.
pub const EFI_GLOBAL_GUID: &str = "8be4df61-93ca-11d2-aa0d-00e098032b8c";

/// Minimum size of an efivarfs payload that carries a single-byte
/// scalar (4-byte attrs + 1-byte value).
const MIN_SCALAR_LEN: usize = 5;

/// Strip the efivarfs attribute prefix and return the first payload
/// byte. The attribute word is not meaningful to L5 checks — we only
/// care about the payload value, which for the three variables we
/// read is always a single byte.
pub fn parse_scalar_byte(raw: &[u8]) -> Result<u8, EfivarParseError> {
    if raw.len() < MIN_SCALAR_LEN {
        return Err(EfivarParseError::Truncated { len: raw.len() });
    }
    Ok(raw[4])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EfivarParseError {
    /// File shorter than the 4-byte attribute header + 1 payload byte.
    Truncated { len: usize },
}

/// Check [`SecureBoot`] == 1. L5 spec V16.
pub fn check_secure_boot(raw: &[u8]) -> Result<(), Token> {
    match parse_scalar_byte(raw) {
        Ok(1) => Ok(()),
        _ => Err(Token::FailSbOff),
    }
}

/// Check [`DeployedMode`] == 1 AND [`SetupMode`] == 0. L5 spec V16a + V16b.
pub fn check_deployed_and_setup_mode(
    deployed_raw: &[u8],
    setup_raw: &[u8],
) -> Result<(), Token> {
    let dep = parse_scalar_byte(deployed_raw).map_err(|_| Token::FailSbSetup)?;
    let setup = parse_scalar_byte(setup_raw).map_err(|_| Token::FailSbSetup)?;
    if dep == 1 && setup == 0 {
        Ok(())
    } else {
        Err(Token::FailSbSetup)
    }
}

/// Build the efivarfs filename for a given variable under the EFI
/// global namespace. Example:
/// `build_efivar_path("SecureBoot") == "SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c"`.
pub fn build_efivar_filename(name: &str) -> String {
    format!("{name}-{EFI_GLOBAL_GUID}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic efivarfs blob: 4 bytes of attrs + payload.
    fn blob(attrs: u32, value: u8) -> Vec<u8> {
        let mut v = attrs.to_le_bytes().to_vec();
        v.push(value);
        v
    }

    #[test]
    fn parse_scalar_byte_strips_header() {
        assert_eq!(parse_scalar_byte(&blob(0x07, 0x01)), Ok(0x01));
        assert_eq!(parse_scalar_byte(&blob(0x07, 0x00)), Ok(0x00));
    }

    #[test]
    fn parse_scalar_byte_rejects_truncated() {
        for len in 0..=4 {
            let raw = vec![0u8; len];
            assert!(matches!(
                parse_scalar_byte(&raw),
                Err(EfivarParseError::Truncated { .. }),
            ));
        }
    }

    #[test]
    fn secure_boot_on_passes() {
        assert!(check_secure_boot(&blob(0x07, 1)).is_ok());
    }

    #[test]
    fn secure_boot_off_fails() {
        assert_eq!(check_secure_boot(&blob(0x07, 0)), Err(Token::FailSbOff));
    }

    #[test]
    fn secure_boot_other_value_fails() {
        assert_eq!(check_secure_boot(&blob(0x07, 0x80)), Err(Token::FailSbOff));
    }

    #[test]
    fn secure_boot_truncated_fails() {
        assert_eq!(check_secure_boot(&[]), Err(Token::FailSbOff));
        assert_eq!(check_secure_boot(&[0x07, 0, 0, 0]), Err(Token::FailSbOff));
    }

    #[test]
    fn deployed_ok_setup_clear_passes() {
        assert!(check_deployed_and_setup_mode(&blob(0x07, 1), &blob(0x07, 0)).is_ok());
    }

    #[test]
    fn deployed_zero_fails() {
        assert_eq!(
            check_deployed_and_setup_mode(&blob(0x07, 0), &blob(0x07, 0)),
            Err(Token::FailSbSetup),
        );
    }

    #[test]
    fn setup_mode_set_fails() {
        assert_eq!(
            check_deployed_and_setup_mode(&blob(0x07, 1), &blob(0x07, 1)),
            Err(Token::FailSbSetup),
        );
    }

    #[test]
    fn truncated_deployed_or_setup_fails() {
        assert_eq!(
            check_deployed_and_setup_mode(&[], &blob(0x07, 0)),
            Err(Token::FailSbSetup),
        );
        assert_eq!(
            check_deployed_and_setup_mode(&blob(0x07, 1), &[]),
            Err(Token::FailSbSetup),
        );
    }

    #[test]
    fn filename_shape_matches_efivarfs_convention() {
        assert_eq!(
            build_efivar_filename("SecureBoot"),
            "SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c",
        );
    }
}

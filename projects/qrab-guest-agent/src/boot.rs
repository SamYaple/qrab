//! Bounded UEFI boot inventory and the single fixed BootNext runtime variable.
//! No raw NVRAM access and no arbitrary EFI variable read/write interface.
use serde::{Deserialize, Serialize};
use std::{
    fs::OpenOptions,
    io::{self, Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

const EFI_GLOBAL: &str = "8be4df61-93ca-11d2-aa0d-00e098032b8c";
pub const MAX_BOOT_OPTIONS: usize = 64;
const MAX_VARIABLE_BYTES: usize = 16 * 1024;
const MAX_DESCRIPTION_UNITS: usize = 512;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct BootState {
    pub efi_runtime_available: bool,
    pub boot_order: Vec<String>,
    pub boot_next: Option<String>,
    pub options: Vec<BootOption>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct BootOption {
    pub reference: String,
    pub description: String,
    pub active: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QrabBootState {}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QrabBootNext {
    pub reference: Option<String>,
}

macro_rules! command {
    ($ty:ty,$name:literal) => {
        impl qapi::QapiCommand for $ty {
            type Returns = BootState;
            const NAME: &'static str = $name;
            const ALLOW_OOB: bool = false;
            const ALLOW_PRECONFIG: bool = false;
            const SUCCESS_RESPONSE: bool = true;
            const UNSTABLE: bool = false;
            const AVAILABLE_SINCE: (u32, u32, u32) = crate::proto::SCHEMA_VERSION;
            const REMOVED_IN: Option<(u32, u32, u32)> = None;
            const PRECONFIG_SINCE: Option<(u32, u32, u32)> = None;
        }
    };
}
command!(QrabBootState, "qrab-boot-state");
command!(QrabBootNext, "qrab-boot-next");

pub fn valid_reference(reference: &str) -> bool {
    reference.len() == 8
        && reference.starts_with("Boot")
        && reference.as_bytes()[4..]
            .iter()
            .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(b))
}
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn variable(root: &Path, name: &str) -> std::path::PathBuf {
    root.join(format!("{name}-{EFI_GLOBAL}"))
}
fn read_variable(root: &Path, name: &str) -> io::Result<Option<Vec<u8>>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(variable(root, name))
    {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut data = Vec::new();
    file.take((MAX_VARIABLE_BYTES + 1) as u64)
        .read_to_end(&mut data)?;
    if data.len() < 4 || data.len() > MAX_VARIABLE_BYTES {
        return Err(invalid("invalid EFI boot variable size"));
    }
    Ok(Some(data[4..].to_vec()))
}
fn reference(number: u16) -> String {
    format!("Boot{number:04X}")
}
fn numbers(data: &[u8]) -> io::Result<Vec<u16>> {
    if !data.len().is_multiple_of(2) || data.len() / 2 > MAX_BOOT_OPTIONS {
        return Err(invalid("invalid UEFI BootOrder"));
    }
    Ok(data
        .chunks_exact(2)
        .map(|p| u16::from_le_bytes([p[0], p[1]]))
        .collect())
}
fn option(reference: String, data: &[u8]) -> io::Result<BootOption> {
    if data.len() < 8 {
        return Err(invalid("invalid UEFI load option"));
    }
    let attrs = u32::from_le_bytes(data[..4].try_into().unwrap());
    let path_length = usize::from(u16::from_le_bytes(data[4..6].try_into().unwrap()));
    let mut description = Vec::new();
    let mut offset = 6;
    loop {
        if offset + 2 > data.len() || description.len() > MAX_DESCRIPTION_UNITS {
            return Err(invalid("invalid UEFI boot description"));
        }
        let unit = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        if unit == 0 {
            break;
        }
        description.push(unit);
    }
    let description = String::from_utf16(&description)
        .map_err(|_| invalid("invalid UEFI boot description encoding"))?;
    if description.chars().any(char::is_control) {
        return Err(invalid("invalid UEFI boot description characters"));
    }
    let Some(path) = data.get(offset..offset.saturating_add(path_length)) else {
        return Err(invalid("invalid UEFI boot device path"));
    };
    if path.len() < 4 {
        return Err(invalid("invalid UEFI boot device path"));
    }
    let mut cursor = 0;
    while cursor < path.len() {
        if cursor + 4 > path.len() {
            return Err(invalid("invalid UEFI boot device path"));
        }
        let length = usize::from(u16::from_le_bytes([path[cursor + 2], path[cursor + 3]]));
        if length < 4 || cursor + length > path.len() {
            return Err(invalid("invalid UEFI boot device path"));
        }
        cursor += length;
    }
    if path[path.len() - 4..] != [0x7f, 0xff, 4, 0] {
        return Err(invalid("unterminated UEFI boot device path"));
    }
    Ok(BootOption {
        reference,
        description,
        active: attrs & 1 != 0,
    })
}

pub(crate) fn state() -> io::Result<BootState> {
    state_at(Path::new("/sys/firmware/efi"))
}
pub(crate) fn set_next(reference: Option<String>) -> io::Result<BootState> {
    set_next_at(Path::new("/sys/firmware/efi"), reference)
}
fn state_at(efi: &Path) -> io::Result<BootState> {
    let root = efi.join("efivars");
    if !root.is_dir() {
        return Ok(BootState::default());
    }
    let order = read_variable(&root, "BootOrder")?
        .map(|d| numbers(&d))
        .transpose()?
        .unwrap_or_default();
    let mut options = Vec::new();
    let mut boot_order = Vec::new();
    for number in order {
        let name = reference(number);
        // Duplicate order entries carry no additional boot option information.
        if boot_order.contains(&name) {
            continue;
        }
        boot_order.push(name.clone());
        if let Some(data) = read_variable(&root, &name)? {
            // Malformed options are not actionable; preserve the actual order
            // reference but omit its invalid resource rather than invent data.
            if let Ok(item) = option(name, &data) {
                options.push(item);
            }
        }
    }
    let boot_next = read_variable(&root, "BootNext")?
        .map(|data| {
            if data.len() != 2 {
                return Err(invalid("invalid UEFI BootNext"));
            }
            Ok(reference(u16::from_le_bytes(data.try_into().unwrap())))
        })
        .transpose()?;
    Ok(BootState {
        efi_runtime_available: true,
        boot_order,
        boot_next,
        options,
    })
}
fn set_next_at(efi: &Path, requested: Option<String>) -> io::Result<BootState> {
    let current = state_at(efi)?;
    if !current.efi_runtime_available {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "EFI runtime variables are unavailable",
        ));
    }
    let root = efi.join("efivars");
    if let Some(name) = requested.as_deref() {
        if !valid_reference(name)
            || !current
                .options
                .iter()
                .any(|o| o.reference == name && o.active)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "BootNext must name an active existing BootOrder option",
            ));
        }
        let number = u16::from_str_radix(&name[4..], 16)
            .map_err(|_| invalid("invalid BootNext reference"))?;
        // Only EFI_GLOBAL:BootNext can be changed. Firmware consumes this
        // NV|BOOTSERVICE|RUNTIME variable on the next firmware boot.
        let mut data = 7u32.to_le_bytes().to_vec();
        data.extend(number.to_le_bytes());
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(variable(&root, "BootNext"))?;
        file.write_all(&data)?;
        file.flush()?;
    } else {
        match std::fs::remove_file(variable(&root, "BootNext")) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    let observed = state_at(efi)?;
    if observed.boot_next != requested {
        return Err(io::Error::other(
            "UEFI BootNext readback did not match the request",
        ));
    }
    Ok(observed)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(root: &Path, name: &str, payload: &[u8]) {
        let mut bytes = 7u32.to_le_bytes().to_vec();
        bytes.extend(payload);
        std::fs::write(variable(root, name), bytes).unwrap();
    }
    fn load_option(description: &str, active: bool) -> Vec<u8> {
        let mut data = u32::from(active).to_le_bytes().to_vec();
        data.extend(4u16.to_le_bytes());
        for unit in description.encode_utf16().chain([0]) {
            data.extend(unit.to_le_bytes());
        }
        data.extend([0x7f, 0xff, 4, 0]);
        data
    }
    #[test]
    fn actual_boot_variables_are_decoded_and_only_boot_next_is_mutated() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("efivars");
        std::fs::create_dir(&root).unwrap();
        write(&root, "BootOrder", &[0x0a, 0, 2, 0]);
        write(&root, "Boot000A", &load_option("Linux α", true));
        write(&root, "Boot0002", &load_option("Disabled", false));
        write(&root, "SecureBoot", &[1]);
        let original = std::fs::read(variable(&root, "BootOrder")).unwrap();
        let boot = state_at(dir.path()).unwrap();
        assert_eq!(boot.boot_order, ["Boot000A", "Boot0002"]);
        assert_eq!(boot.options[0].description, "Linux α");
        assert!(boot.options[0].active);
        assert!(set_next_at(dir.path(), Some("Boot0002".into())).is_err());
        assert!(set_next_at(dir.path(), Some("../Boot000A".into())).is_err());
        let boot = set_next_at(dir.path(), Some("Boot000A".into())).unwrap();
        assert_eq!(boot.boot_next.as_deref(), Some("Boot000A"));
        assert_eq!(
            std::fs::read(variable(&root, "BootOrder")).unwrap(),
            original
        );
        assert_eq!(read_variable(&root, "SecureBoot").unwrap(), Some(vec![1]));
        assert!(set_next_at(dir.path(), None).unwrap().boot_next.is_none());
    }
    #[test]
    fn malformed_variables_never_become_synthetic_options() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(state_at(dir.path()).unwrap(), BootState::default());
        let root = dir.path().join("efivars");
        std::fs::create_dir(&root).unwrap();
        write(&root, "BootOrder", &[1, 0]);
        write(&root, "Boot0001", &[0, 1]);
        assert!(state_at(dir.path()).unwrap().options.is_empty());
        write(&root, "BootNext", &[1]);
        assert!(state_at(dir.path()).is_err());
        assert!(!valid_reference("Boot00aA") && !valid_reference("Boot12345"));
    }
}

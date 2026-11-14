//! Fixed-name Secure Boot database inventory. No arbitrary EFI variable API.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::OpenOptions,
    io::{self, Read},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

const GLOBAL_GUID: &str = "8be4df61-93ca-11d2-aa0d-00e098032b8c";
const IMAGE_GUID: &str = "d719b2cb-3d3a-4596-a3bc-dad00e67656f";
pub const MAX_DATABASE_BYTES: usize = 256 * 1024;
pub const MAX_TOTAL_BYTES: usize = 256 * 1024;
pub const MAX_SIGNATURE_BYTES: usize = 16 * 1024;
pub const MAX_SIGNATURES: usize = 512;
pub const X509_GUID: &str = "a5c059a1-94e4-4aa7-87b5-ab155c2bf072";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QrabSecureBootDatabases {}
impl qapi::QapiCommand for QrabSecureBootDatabases {
    type Returns = SecureBootDatabases;
    const NAME: &'static str = "qrab-secure-boot-databases";
    const ALLOW_OOB: bool = false;
    const ALLOW_PRECONFIG: bool = false;
    const SUCCESS_RESPONSE: bool = true;
    const UNSTABLE: bool = false;
    const AVAILABLE_SINCE: (u32, u32, u32) = crate::proto::SCHEMA_VERSION;
    const REMOVED_IN: Option<(u32, u32, u32)> = None;
    const PRECONFIG_SINCE: Option<(u32, u32, u32)> = None;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct SecureBootDatabases {
    pub efi_runtime_available: bool,
    pub databases: Vec<SecureBootDatabase>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct SecureBootDatabase {
    pub id: String,
    pub present: Option<bool>,
    pub attributes: Option<u32>,
    pub signatures: Vec<EfiSignature>,
    pub unsupported_types: Vec<String>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct EfiSignature {
    pub signature_type_guid: String,
    pub owner_guid: String,
    pub data_hex: String,
}

fn database_guid(id: &str) -> Option<&'static str> {
    match id {
        "PK" | "KEK" => Some(GLOBAL_GUID),
        "db" | "dbx" => Some(IMAGE_GUID),
        _ => None,
    }
}
fn malformed() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "malformed EFI signature list")
}
fn limit() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "EFI signature inventory exceeds bound",
    )
}
fn u32le(bytes: &[u8]) -> io::Result<u32> {
    Ok(u32::from_le_bytes(
        bytes.try_into().map_err(|_| malformed())?,
    ))
}
fn guid(bytes: &[u8]) -> io::Result<String> {
    if bytes.len() != 16 {
        return Err(malformed());
    }
    Ok(format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        u32le(&bytes[..4])?,
        u16::from_le_bytes(bytes[4..6].try_into().unwrap()),
        u16::from_le_bytes(bytes[6..8].try_into().unwrap()),
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    ))
}
fn signature_size(kind: &str) -> Option<Option<usize>> {
    Some(match kind {
        X509_GUID => None,
        "826ca512-cf10-4ac9-b187-be01496631bd" => Some(20),
        "0b6e5233-a65c-44c9-9407-d9ab83bfc8bd" => Some(28),
        "c1c41626-504c-4092-aca9-41f936934328" => Some(32),
        "ff3e5307-9fd0-48c9-85f1-8ad56c701e01" => Some(48),
        "093e0fae-a6c4-4f50-9f1b-d41e2b89c19a" => Some(64),
        "3bd2a492-96c0-4079-b420-fcf98ef103ed" => Some(48),
        "7076876e-80c2-4ee6-aad2-28b349a6865b" => Some(64),
        "446dbf63-2502-4cda-bcfa-2465d2b0fe9d" => Some(80),
        "3c5766e8-269c-4e34-aa14-ed776e85b3b6" => Some(256),
        "e2b36190-879b-4a3d-ad8d-f2e7bba32784" => Some(528),
        _ => return None,
    })
}

/// Parse complete concatenated EFI_SIGNATURE_LIST values. Unknown types are
/// recorded by GUID, never returned as opaque data that could hide key material.
fn parse(bytes: &[u8]) -> io::Result<(Vec<EfiSignature>, Vec<String>)> {
    if bytes.len() > MAX_DATABASE_BYTES {
        return Err(limit());
    }
    let mut offset = 0;
    let mut entries = 0;
    let mut lists = 0;
    let mut signatures = Vec::new();
    let mut unsupported = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    while offset < bytes.len() {
        lists += 1;
        if lists > MAX_SIGNATURES {
            return Err(limit());
        }
        let header = bytes.get(offset..offset + 28).ok_or_else(malformed)?;
        let kind = guid(&header[..16])?;
        let list_size = usize::try_from(u32le(&header[16..20])?).map_err(|_| malformed())?;
        let header_size = usize::try_from(u32le(&header[20..24])?).map_err(|_| malformed())?;
        let entry_size = usize::try_from(u32le(&header[24..28])?).map_err(|_| malformed())?;
        let data_start = 28usize.checked_add(header_size).ok_or_else(malformed)?;
        if list_size < data_start || entry_size <= 16 || entry_size > MAX_SIGNATURE_BYTES + 16 {
            return Err(malformed());
        }
        let list = bytes
            .get(offset..offset.checked_add(list_size).ok_or_else(malformed)?)
            .ok_or_else(malformed)?;
        let data = list.get(data_start..).ok_or_else(malformed)?;
        if !data.len().is_multiple_of(entry_size) {
            return Err(malformed());
        }
        let expected = signature_size(&kind);
        if expected.is_some() && header_size != 0 {
            return Err(malformed());
        }
        if expected.flatten().is_some_and(|n| n + 16 != entry_size) {
            return Err(malformed());
        }
        for signature in data.chunks_exact(entry_size) {
            entries += 1;
            if entries > MAX_SIGNATURES {
                return Err(limit());
            }
            if expected.is_none() {
                unsupported.insert(kind.clone());
                continue;
            }
            if kind == X509_GUID {
                use x509_cert::der::Decode;
                x509_cert::Certificate::from_der(&signature[16..]).map_err(|_| malformed())?;
            }
            let owner = guid(&signature[..16])?;
            let data_hex = signature[16..]
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            if duplicates.insert((kind.clone(), owner.clone(), data_hex.clone())) {
                signatures.push(EfiSignature {
                    signature_type_guid: kind.clone(),
                    owner_guid: owner,
                    data_hex,
                });
            }
        }
        offset = offset.checked_add(list_size).ok_or_else(malformed)?;
    }
    Ok((signatures, unsupported.into_iter().collect()))
}

pub(crate) fn databases(efi: &Path) -> io::Result<SecureBootDatabases> {
    if !efi.try_exists()? {
        return Ok(SecureBootDatabases::default());
    }
    let variables = efi.join("efivars");
    let mounted = variables.is_dir();
    let mut databases = Vec::new();
    let mut total = 0;
    let mut count = 0;
    for id in ["PK", "KEK", "db", "dbx"] {
        let mut db = SecureBootDatabase {
            id: id.into(),
            ..Default::default()
        };
        if !mounted {
            db.error = Some("Unavailable".into());
            databases.push(db);
            continue;
        }
        let path = variables.join(format!("{id}-{}", database_guid(id).unwrap()));
        match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
        {
            Ok(file) => {
                db.present = Some(true);
                let mut bytes = Vec::new();
                if file
                    .take(MAX_DATABASE_BYTES as u64 + 5)
                    .read_to_end(&mut bytes)
                    .is_err()
                {
                    db.error = Some("Unavailable".into());
                } else if bytes.len() < 4 {
                    db.error = Some("Malformed".into());
                } else if bytes.len() > MAX_DATABASE_BYTES + 4
                    || total + bytes.len() - 4 > MAX_TOTAL_BYTES
                {
                    db.error = Some("TooLarge".into());
                } else {
                    total += bytes.len() - 4;
                    db.attributes = Some(u32le(&bytes[..4])?);
                    match parse(&bytes[4..]) {
                        Ok((signatures, unsupported))
                            if count + signatures.len() <= MAX_SIGNATURES =>
                        {
                            count += signatures.len();
                            db.signatures = signatures;
                            db.unsupported_types = unsupported;
                        }
                        Ok(_) => db.error = Some("TooLarge".into()),
                        Err(_) => db.error = Some("MalformedOrTooLarge".into()),
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => db.present = Some(false),
            Err(_) => db.error = Some("Unavailable".into()),
        }
        databases.push(db);
    }
    Ok(SecureBootDatabases {
        efi_runtime_available: true,
        databases,
    })
}

pub const MAX_AUTH_UPDATE_BYTES: usize = 128 * 1024;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct QrabSecureBootUpdate {
    pub database_id: String,
    pub payload_hex: String,
    #[serde(default)]
    pub append: bool,
}
impl qapi::QapiCommand for QrabSecureBootUpdate {
    type Returns = SecureBootDatabases;
    const NAME: &'static str = "qrab-secure-boot-update";
    const ALLOW_OOB: bool = false;
    const ALLOW_PRECONFIG: bool = false;
    const SUCCESS_RESPONSE: bool = true;
    const UNSTABLE: bool = false;
    const AVAILABLE_SINCE: (u32, u32, u32) = crate::proto::SCHEMA_VERSION;
    const REMOVED_IN: Option<(u32, u32, u32)> = None;
    const PRECONFIG_SINCE: Option<(u32, u32, u32)> = None;
}

fn decode_auth(hex: &str) -> io::Result<Vec<u8>> {
    if hex.len() > MAX_AUTH_UPDATE_BYTES * 2 || !hex.len().is_multiple_of(2) {
        return Err(limit());
    }
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digit = |c| match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                b'A'..=b'F' => Some(c - b'A' + 10),
                _ => None,
            };
            Ok(
                digit(pair[0]).ok_or_else(malformed)? * 16
                    + digit(pair[1]).ok_or_else(malformed)?,
            )
        })
        .collect()
}
fn der_value<'a>(bytes: &mut &'a [u8], tag: u8) -> io::Result<&'a [u8]> {
    if bytes.first() != Some(&tag) {
        return Err(malformed());
    }
    let first = *bytes.get(1).ok_or_else(malformed)?;
    let (header, length) = if first < 128 {
        (2, usize::from(first))
    } else {
        let count = usize::from(first & 127);
        if count == 0 || count > 4 {
            return Err(malformed());
        }
        let size = bytes.get(2..2 + count).ok_or_else(malformed)?;
        if size[0] == 0 {
            return Err(malformed());
        }
        let mut length = 0usize;
        for byte in size {
            length = length
                .checked_mul(256)
                .and_then(|n| n.checked_add(usize::from(*byte)))
                .ok_or_else(malformed)?;
        }
        if length < 128 {
            return Err(malformed());
        }
        (2 + count, length)
    };
    let end = header.checked_add(length).ok_or_else(malformed)?;
    let value = bytes.get(header..end).ok_or_else(malformed)?;
    *bytes = bytes.get(end..).ok_or_else(malformed)?;
    Ok(value)
}
fn validate_pkcs7(bytes: &[u8]) -> io::Result<()> {
    // Validate the ContentInfo/SignedData envelope, never a PKCS#8/RSA/EC
    // private-key container. Firmware remains the signature/time authority.
    let mut outer = bytes;
    let mut sequence = der_value(&mut outer, 0x30)?;
    if !outer.is_empty()
        || der_value(&mut sequence, 0x06)? != [0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x02]
    {
        return Err(malformed());
    }
    let mut explicit = der_value(&mut sequence, 0xa0)?;
    let signed = der_value(&mut explicit, 0x30)?;
    if !sequence.is_empty() || !explicit.is_empty() || signed.is_empty() {
        return Err(malformed());
    }
    Ok(())
}

/// Only authenticated .auth payloads are admitted. This checks framing and
/// fixed policy; the EFI runtime verifies signatures, key authorization and
/// anti-replay timestamps. No raw-variable fallback or key-reset shortcut.
pub fn validate_update(request: &QrabSecureBootUpdate) -> io::Result<Vec<u8>> {
    if database_guid(&request.database_id).is_none()
        || request.append && request.database_id == "PK"
    {
        return Err(malformed());
    }
    let payload = decode_auth(&request.payload_hex)?;
    if payload.len() < 40 {
        return Err(malformed());
    }
    let year = u16::from_le_bytes(payload[0..2].try_into().unwrap());
    let month = payload[2];
    let day = payload[3];
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    if !(1900..=9999).contains(&year)
        || day == 0
        || day > days
        || payload[4] > 23
        || payload[5] > 59
        || payload[6] > 59
        || payload[7..16].iter().any(|b| *b != 0)
    {
        return Err(malformed());
    }
    let length = usize::try_from(u32le(&payload[16..20])?).map_err(|_| malformed())?;
    let end = 16usize.checked_add(length).ok_or_else(malformed)?;
    if length <= 24
        || end > payload.len()
        || payload[20..22] != [0x00, 0x02]
        || payload[22..24] != [0xf1, 0x0e]
        || guid(&payload[24..40])? != "4aafd29d-68df-49ee-8aa9-347d375665a7"
    {
        return Err(malformed());
    }
    validate_pkcs7(&payload[40..end])?;
    let (_, unsupported) = parse(&payload[end..])?;
    if !unsupported.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unsupported EFI signature type",
        ));
    }
    Ok(payload)
}

pub(crate) fn authenticated_update(
    efi: &Path,
    request: QrabSecureBootUpdate,
) -> io::Result<SecureBootDatabases> {
    use std::io::Write;
    let payload = validate_update(&request)?;
    let guid = database_guid(&request.database_id).ok_or_else(malformed)?;
    let variables = efi.join("efivars");
    if !variables.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "EFI runtime variables are unavailable",
        ));
    }
    // NV | BOOTSERVICE_ACCESS | RUNTIME_ACCESS | TIME_BASED_AUTHENTICATED_WRITE_ACCESS.
    let attributes = 0x27u32 | if request.append { 0x40 } else { 0 };
    let mut bytes = attributes.to_le_bytes().to_vec();
    bytes.extend_from_slice(&payload);
    let path = variables.join(format!("{}-{guid}", request.database_id));
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    // efivarfs SetVariable is one transaction. Never retry a short/ambiguous
    // write, clear immutable flags, or bypass firmware authentication failures.
    if file.write(&bytes)? != bytes.len() {
        return Err(io::Error::other(
            "incomplete EFI authenticated update; outcome unknown",
        ));
    }
    databases(efi)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn guid_bytes(guid: &str) -> [u8; 16] {
        let parts = guid.split('-').collect::<Vec<_>>();
        let mut bytes = [0; 16];
        bytes[..4].copy_from_slice(&u32::from_str_radix(parts[0], 16).unwrap().to_le_bytes());
        bytes[4..6].copy_from_slice(&u16::from_str_radix(parts[1], 16).unwrap().to_le_bytes());
        bytes[6..8].copy_from_slice(&u16::from_str_radix(parts[2], 16).unwrap().to_le_bytes());
        let tail = format!("{}{}", parts[3], parts[4]);
        for (index, byte) in bytes[8..].iter_mut().enumerate() {
            *byte = u8::from_str_radix(&tail[index * 2..index * 2 + 2], 16).unwrap();
        }
        bytes
    }
    fn list(kind: &str, data: &[u8], count: usize) -> Vec<u8> {
        let entry_size = 16 + data.len();
        let mut out = guid_bytes(kind).to_vec();
        out.extend_from_slice(
            &u32::try_from(28 + entry_size * count)
                .unwrap()
                .to_le_bytes(),
        );
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(entry_size as u32).to_le_bytes());
        for _ in 0..count {
            out.extend_from_slice(&guid_bytes("12345678-1234-5678-90ab-cdef12345678"));
            out.extend_from_slice(data);
        }
        out
    }
    fn auth(data: &[u8]) -> Vec<u8> {
        let pkcs7 = [
            0x30, 0x12, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x02, 0xa0,
            0x05, 0x30, 0x03, 0x02, 0x01, 0x01,
        ];
        let mut out = vec![0xea, 0x07, 11, 14, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        out.extend_from_slice(&(24u32 + pkcs7.len() as u32).to_le_bytes());
        out.extend_from_slice(&[0x00, 0x02, 0xf1, 0x0e]);
        out.extend_from_slice(&guid_bytes("4aafd29d-68df-49ee-8aa9-347d375665a7"));
        out.extend_from_slice(&pkcs7);
        out.extend_from_slice(data);
        out
    }
    fn request(payload: &[u8]) -> QrabSecureBootUpdate {
        QrabSecureBootUpdate {
            database_id: "db".into(),
            payload_hex: payload.iter().map(|b| format!("{b:02x}")).collect(),
            append: false,
        }
    }
    #[test]
    fn lists_are_bounded_deduplicated_and_decode_little_endian_guids() {
        let bytes = list("c1c41626-504c-4092-aca9-41f936934328", &[7; 32], 2);
        let (items, unknown) = parse(&bytes).unwrap();
        assert_eq!(items.len(), 1);
        assert!(unknown.is_empty());
        assert_eq!(items[0].owner_guid, "12345678-1234-5678-90ab-cdef12345678");
        assert_eq!(items[0].data_hex, "07".repeat(32));
        for end in 1..bytes.len() {
            assert!(parse(&bytes[..end]).is_err(), "truncated at {end}");
        }
        assert!(parse(&vec![0; MAX_DATABASE_BYTES + 1]).is_err());
        assert!(parse(&list(
            "c1c41626-504c-4092-aca9-41f936934328",
            &[7; 32],
            MAX_SIGNATURES + 1
        ))
        .is_err());
        let mut invalid = bytes.clone();
        invalid[16..20].copy_from_slice(&0u32.to_le_bytes());
        assert!(parse(&invalid).is_err());
        invalid = bytes.clone();
        invalid[24..28].copy_from_slice(&16u32.to_le_bytes());
        assert!(parse(&invalid).is_err());
        invalid = bytes;
        invalid[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse(&invalid).is_err());
    }
    #[test]
    fn only_public_x509_and_known_digest_types_leave_the_parser() {
        let certificate = include_bytes!("../tests/fixtures/secureboot-test-cert.der");
        assert_eq!(parse(&list(X509_GUID, certificate, 1)).unwrap().0.len(), 1);
        // Private-key/PKCS#8-style ASN.1 starts with INTEGER, not TBSCertificate.
        assert!(parse(&list(X509_GUID, &[0x30, 0x03, 0x02, 0x01, 0], 1)).is_err());
        let unknown = "11111111-2222-3333-4444-555555555555";
        let (items, types) =
            parse(&list(unknown, b"opaque potentially sensitive bytes", 1)).unwrap();
        assert!(items.is_empty());
        assert_eq!(types, vec![unknown]);
    }
    #[test]
    fn fixed_database_names_distinguish_absent_unmounted_and_malformed() {
        let temp = tempfile::tempdir().unwrap();
        let efi = temp.path().join("efi");
        assert!(!databases(&efi).unwrap().efi_runtime_available);
        std::fs::create_dir_all(&efi).unwrap();
        assert_eq!(databases(&efi).unwrap().databases[0].present, None);
        let vars = efi.join("efivars");
        std::fs::create_dir(&vars).unwrap();
        let mut value = 0x27u32.to_le_bytes().to_vec();
        value.extend(list("c1c41626-504c-4092-aca9-41f936934328", &[9; 32], 1));
        std::fs::write(vars.join(format!("db-{IMAGE_GUID}")), value).unwrap();
        std::fs::write(vars.join(format!("PK-{GLOBAL_GUID}")), [1, 2]).unwrap();
        let snapshot = databases(&efi).unwrap();
        assert_eq!(snapshot.databases[0].error.as_deref(), Some("Malformed"));
        assert_eq!(snapshot.databases[1].present, Some(false));
        assert_eq!(snapshot.databases[2].signatures.len(), 1);
    }
    #[test]
    fn authenticated_updates_validate_bounds_timestamp_envelope_and_fixed_policy() {
        let payload = auth(&list("c1c41626-504c-4092-aca9-41f936934328", &[9; 32], 1));
        assert_eq!(validate_update(&request(&payload)).unwrap(), payload);
        for end in 0..40 {
            assert!(validate_update(&request(&payload[..end])).is_err());
        }
        let mut invalid = payload.clone();
        invalid[7] = 1;
        assert!(validate_update(&request(&invalid)).is_err());
        invalid = payload.clone();
        invalid[3] = 32;
        assert!(validate_update(&request(&invalid)).is_err());
        invalid = payload.clone();
        invalid[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(validate_update(&request(&invalid)).is_err());
        invalid = payload.clone();
        invalid[24] = 0;
        assert!(validate_update(&request(&invalid)).is_err());
        let mut req = request(&payload);
        req.database_id = "../../other".into();
        assert!(validate_update(&req).is_err());
        req.database_id = "PK".into();
        req.append = true;
        assert!(validate_update(&req).is_err());
        req = request(&vec![0; MAX_AUTH_UPDATE_BYTES + 1]);
        assert!(validate_update(&req).is_err());
        assert!(validate_update(&request(&auth(&list(
            X509_GUID,
            &[0x30, 0x03, 0x02, 0x01, 0],
            1
        ))))
        .is_err());
    }
    #[test]
    fn update_writes_only_fixed_variable_and_never_follows_symlinks() {
        let temp = tempfile::tempdir().unwrap();
        let efi = temp.path().join("efi");
        let vars = efi.join("efivars");
        std::fs::create_dir_all(&vars).unwrap();
        let protected = vars.join(format!("SecureBoot-{GLOBAL_GUID}"));
        std::fs::write(&protected, [7, 0, 0, 0, 1]).unwrap();
        let payload = auth(&list("c1c41626-504c-4092-aca9-41f936934328", &[9; 32], 1));
        // This regular-file fixture checks syscall framing only. Actual EFI
        // cryptographic/time authorization is performed by firmware at runtime.
        authenticated_update(&efi, request(&payload)).unwrap();
        let variable = vars.join(format!("db-{IMAGE_GUID}"));
        let written = std::fs::read(&variable).unwrap();
        assert_eq!(&written[..4], &0x27u32.to_le_bytes());
        assert_eq!(&written[4..], payload);
        assert_eq!(std::fs::read(&protected).unwrap(), [7, 0, 0, 0, 1]);
        std::fs::remove_file(&variable).unwrap();
        std::os::unix::fs::symlink(&protected, &variable).unwrap();
        assert!(authenticated_update(&efi, request(&payload)).is_err());
        assert_eq!(std::fs::read(protected).unwrap(), [7, 0, 0, 0, 1]);
    }
}

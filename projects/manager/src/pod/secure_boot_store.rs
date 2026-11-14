//! Bounded cold editor for one EDK2 authenticated-store policy byte.
//!
//! Call only under the VM's persistent-state lease after QEMU is stopped.
//! This module never edits files: callers atomically install the returned
//! candidate with a backup. PK/KEK/db/dbx and every unrelated byte are retained.
use crate::{Error, Result};
use std::collections::BTreeSet;
use x509_cert::der::Decode;

pub const MAX_STORE_BYTES: usize = 4 * 1024 * 1024;
const MAX_RECORDS: usize = 4096;
const MAX_DATABASE_BYTES: usize = 256 * 1024;
const MAX_SIGNATURES: usize = 512;
const NV_DATA: [u8; 16] = [
    0x8d, 0x2b, 0xf1, 0xff, 0x96, 0x76, 0x8b, 0x4c, 0xa9, 0x85, 0x27, 0x47, 0x07, 0x5b, 0x4f, 0x50,
];
const AUTH_STORE: [u8; 16] = [
    0x78, 0x2c, 0xf3, 0xaa, 0x7b, 0x94, 0x9a, 0x43, 0xa1, 0x80, 0x2e, 0x14, 0x4e, 0xc3, 0x77, 0x92,
];
const ENABLE_GUID: [u8; 16] = [
    0xc7, 0x0b, 0xa3, 0xf0, 0x08, 0xaf, 0x56, 0x45, 0x99, 0xc4, 0x00, 0x10, 0x09, 0xc9, 0x3a, 0x44,
];
const GLOBAL: [u8; 16] = [
    0x61, 0xdf, 0xe4, 0x8b, 0xca, 0x93, 0xd2, 0x11, 0xaa, 0x0d, 0x00, 0xe0, 0x98, 0x03, 0x2b, 0x8c,
];
const IMAGE_SECURITY: [u8; 16] = [
    0xcb, 0xb2, 0x19, 0xd7, 0x3a, 0x3d, 0x96, 0x45, 0xa3, 0xbc, 0xda, 0xd0, 0x0e, 0x67, 0x65, 0x6f,
];
const FLAG: &str = "SecureBootEnable";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Info {
    /// The boot-services policy flag, not current guest SecureBoot state.
    pub enabled: Option<bool>,
    pub has_pk: bool,
    pub has_db: bool,
}
struct Parsed {
    info: Info,
    flag: Option<usize>,
    append: usize,
    end: usize,
}
fn invalid(message: &str) -> Error {
    Error::Config(format!(
        "unsupported or malformed EDK2 variable store: {message}"
    ))
}
fn bytes(data: &[u8], offset: usize, size: usize) -> Result<&[u8]> {
    data.get(
        offset
            ..offset
                .checked_add(size)
                .ok_or_else(|| invalid("range overflow"))?,
    )
    .ok_or_else(|| invalid("range outside image"))
}
fn u16le(data: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        bytes(data, offset, 2)?.try_into().unwrap(),
    ))
}
fn u32le(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes(data, offset, 4)?.try_into().unwrap(),
    ))
}
fn guid(data: &[u8]) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{}",
        u32::from_le_bytes(data[..4].try_into().unwrap()),
        u16::from_le_bytes(data[4..6].try_into().unwrap()),
        u16::from_le_bytes(data[6..8].try_into().unwrap()),
        data[8],
        data[9],
        data[10..]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

/// Validate EFI_SIGNATURE_LIST envelopes and recognized entry payloads. No
/// certificate/private-key execution occurs. DER parsing is structural; trust
/// and image acceptance are enforced by the firmware on the next boot.
fn database(data: &[u8], name: &str) -> Result<bool> {
    if data.is_empty() {
        return Ok(false);
    }
    if data.len() > MAX_DATABASE_BYTES {
        return Err(invalid("signature database exceeds bound"));
    }
    let mut offset = 0;
    let mut entries = 0;
    while offset < data.len() {
        let header = bytes(data, offset, 28)?;
        let kind = guid(&header[..16]);
        let list_size = u32le(header, 16)? as usize;
        let header_size = u32le(header, 20)? as usize;
        let signature_size = u32le(header, 24)? as usize;
        if header_size != 0
            || list_size < 28
            || signature_size <= 16
            || signature_size > 16 * 1024 + 16
        {
            return Err(invalid("invalid signature list header"));
        }
        let list = bytes(data, offset, list_size)?;
        let payload = &list[28..];
        if payload.is_empty() || !payload.len().is_multiple_of(signature_size) {
            return Err(invalid("invalid signature list entries"));
        }
        let is_x509 = kind == "a5c059a1-94e4-4aa7-87b5-ab155c2bf072";
        let fixed = match kind.as_str() {
            "a5c059a1-94e4-4aa7-87b5-ab155c2bf072" => None,
            "826ca512-cf10-4ac9-b187-be01496631bd" => Some(20),
            "0b6e5233-a65c-44c9-9407-d9ab83bfc8bd" => Some(28),
            "c1c41626-504c-4092-aca9-41f936934328" => Some(32),
            "ff3e5307-9fd0-48c9-85f1-8ad56c701e01" => Some(48),
            "093e0fae-a6c4-4f50-9f1b-d41e2b89c19a" => Some(64),
            "3bd2a492-96c0-4079-b420-fcf98ef103ed" if name == "dbx" => Some(48),
            "7076876e-80c2-4ee6-aad2-28b349a6865b" if name == "dbx" => Some(64),
            "446dbf63-2502-4cda-bcfa-2465d2b0fe9d" if name == "dbx" => Some(80),
            _ => return Err(invalid("unsupported EFI signature type")),
        };
        if name == "PK" && !is_x509 {
            return Err(invalid("PK must be one X.509 certificate"));
        }
        if name == "KEK" && !is_x509 {
            return Err(invalid("KEK must contain X.509 certificates"));
        }
        for entry in payload.chunks_exact(signature_size) {
            entries += 1;
            if entries > MAX_SIGNATURES {
                return Err(invalid("signature entry count exceeds bound"));
            }
            let value = &entry[16..];
            if fixed.is_some_and(|size| size != value.len()) {
                return Err(invalid("signature hash has wrong length"));
            }
            if is_x509 && x509_cert::Certificate::from_der(value).is_err() {
                return Err(invalid("malformed X.509 certificate"));
            }
        }
        offset = offset
            .checked_add(list_size)
            .ok_or_else(|| invalid("signature size overflow"))?;
    }
    if name == "PK" && entries != 1 {
        return Err(invalid("PK must be one X.509 certificate"));
    }
    Ok(entries > 0)
}

fn parse(data: &[u8]) -> Result<Parsed> {
    if data.len() < 100 || data.len() > MAX_STORE_BYTES {
        return Err(invalid("image size outside bound"));
    }
    let mut volumes = Vec::new();
    for offset in (0..data.len().saturating_sub(64)).step_by(1024) {
        if data.get(offset + 16..offset + 32) == Some(NV_DATA.as_slice()) {
            volumes.push(offset);
        }
    }
    if volumes.len() != 1 {
        return Err(invalid("expected one NV-data firmware volume"));
    }
    let fv = volumes[0];
    let length = u64::from_le_bytes(bytes(data, fv + 32, 8)?.try_into().unwrap());
    let length = usize::try_from(length).map_err(|_| invalid("firmware volume size overflow"))?;
    let header_size = u16le(data, fv + 48)? as usize;
    if bytes(data, fv + 40, 4)? != b"_FVH"
        || data[fv + 55] != 2
        || header_size < 72
        || !header_size.is_multiple_of(4)
        || u16le(data, fv + 52)? != 0
        || u32le(data, fv + 44)? & 0x800 == 0
        || length < header_size + 28
    {
        return Err(invalid("unsupported firmware volume header"));
    }
    let volume = bytes(data, fv, length)?;
    let header = bytes(volume, 0, header_size)?;
    let checksum = header.chunks_exact(2).fold(0u16, |sum, p| {
        sum.wrapping_add(u16::from_le_bytes([p[0], p[1]]))
    });
    if checksum != 0 {
        return Err(invalid("firmware volume header checksum failed"));
    }
    let mut map = 56;
    let mut mapped = 0usize;
    let mut terminated = false;
    while map + 8 <= header_size {
        let count = u32le(header, map)? as usize;
        let size = u32le(header, map + 4)? as usize;
        map += 8;
        if count == 0 && size == 0 {
            terminated = true;
            break;
        }
        if count == 0 || size == 0 {
            return Err(invalid("invalid firmware block map"));
        }
        mapped = mapped
            .checked_add(
                count
                    .checked_mul(size)
                    .ok_or_else(|| invalid("block map overflow"))?,
            )
            .ok_or_else(|| invalid("block map overflow"))?;
    }
    if !terminated || mapped != length {
        return Err(invalid("firmware block map size mismatch"));
    }
    let store = fv + header_size;
    if bytes(data, store, 16)? != AUTH_STORE || data[store + 20] != 0x5a || data[store + 21] != 0xfe
    {
        return Err(invalid(
            "expected formatted healthy authenticated variable store",
        ));
    }
    let size = u32le(data, store + 16)? as usize;
    if size < 28 || !size.is_multiple_of(4) || size > length - header_size {
        return Err(invalid("variable store size outside firmware volume"));
    }
    let end = store + size;
    let mut cursor = store + 28;
    let mut records = 0;
    let mut info = Info::default();
    let mut flag = None;
    let mut active = BTreeSet::new();
    let mut transition = BTreeSet::new();
    while cursor < end {
        if data.get(cursor..cursor.saturating_add(2)) == Some([0xff, 0xff].as_slice()) {
            if data[cursor..end].iter().all(|b| *b == 0xff) {
                break;
            }
            return Err(invalid("non-erased bytes after variable terminator"));
        }
        records += 1;
        if records > MAX_RECORDS {
            return Err(invalid("variable count exceeds bound"));
        }
        let h = bytes(data, cursor, 60)?;
        if cursor + 60 > end || u16le(h, 0)? != 0x55aa || ![0x3f, 0x3e, 0x3d, 0x3c].contains(&h[2])
        {
            return Err(invalid("unsupported or incomplete variable record"));
        }
        let name_size = u32le(h, 36)? as usize;
        let data_size = u32le(h, 40)? as usize;
        if !(2..=4096).contains(&name_size) || !name_size.is_multiple_of(2) {
            return Err(invalid("invalid variable name size"));
        }
        let start = cursor
            .checked_add(60 + name_size)
            .ok_or_else(|| invalid("variable size overflow"))?;
        let next = start
            .checked_add(data_size)
            .and_then(|n| n.checked_add(3))
            .map(|n| n & !3)
            .filter(|n| *n <= end)
            .ok_or_else(|| invalid("variable record exceeds store"))?;
        let encoded = bytes(data, cursor + 60, name_size)?;
        if encoded[name_size - 2..] != [0, 0] {
            return Err(invalid("variable name is not terminated"));
        }
        let units = encoded[..name_size - 2]
            .chunks_exact(2)
            .map(|p| u16::from_le_bytes([p[0], p[1]]))
            .collect::<Vec<_>>();
        let name =
            String::from_utf16(&units).map_err(|_| invalid("variable name is not UTF-16"))?;
        if name.is_empty() || name.contains('\0') {
            return Err(invalid("invalid variable name"));
        }
        let vendor: [u8; 16] = h[44..60].try_into().unwrap();
        let key = (vendor, name.clone());
        if h[2] == 0x3e {
            transition.insert(key.clone());
        }
        if h[2] == 0x3f {
            if !active.insert(key) {
                return Err(invalid("duplicate active variable identity"));
            }
            let value = bytes(data, start, data_size)?;
            let attr = u32le(h, 4)?;
            if vendor == ENABLE_GUID && name == FLAG {
                if attr != 3 || value.len() != 1 || value[0] > 1 {
                    return Err(invalid("invalid SecureBootEnable policy variable"));
                }
                info.enabled = Some(value[0] != 0);
                flag = Some(start);
            }
            if ((vendor == GLOBAL && ["PK", "KEK"].contains(&name.as_str()))
                || (vendor == IMAGE_SECURITY && ["db", "dbx"].contains(&name.as_str())))
                && !value.is_empty()
            {
                if attr != 0x27 {
                    return Err(invalid(
                        "signature database lacks expected authenticated attributes",
                    ));
                }
                let present = database(value, &name)?;
                if name == "PK" {
                    info.has_pk = present;
                }
                if name == "db" {
                    info.has_db = present;
                }
            }
        }
        cursor = next;
    }
    if !transition.is_subset(&active) {
        return Err(invalid(
            "unfinished variable update has no active successor",
        ));
    }
    Ok(Parsed {
        info,
        flag,
        append: cursor,
        end,
    })
}

pub fn inspect(data: &[u8]) -> Result<Info> {
    Ok(parse(data)?.info)
}

/// Only SecureBootEnable is modified. Existing records require a one-byte
/// patch; an absent flag appends one unauthenticated NV|BS record if space is
/// erased. No garbage collection, tombstone rewrite, key mutation or migration.
pub fn set_enabled(data: &[u8], enabled: bool) -> Result<Vec<u8>> {
    let parsed = parse(data)?;
    if enabled && (!parsed.info.has_pk || !parsed.info.has_db) {
        return Err(Error::Config(
            "Secure Boot enable requires an enrolled valid PK and db".into(),
        ));
    }
    let mut output = data.to_vec();
    if let Some(offset) = parsed.flag {
        output[offset] = u8::from(enabled);
        return Ok(output);
    }
    let mut record = vec![0u8; 60];
    record[..2].copy_from_slice(&0x55aau16.to_le_bytes());
    record[2] = 0x3f;
    record[4..8].copy_from_slice(&3u32.to_le_bytes());
    record[44..60].copy_from_slice(&ENABLE_GUID);
    let encoded = FLAG
        .encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    record[36..40].copy_from_slice(&(encoded.len() as u32).to_le_bytes());
    record[40..44].copy_from_slice(&1u32.to_le_bytes());
    record.extend(encoded);
    record.push(u8::from(enabled));
    while !record.len().is_multiple_of(4) {
        record.push(0xff);
    }
    if record.len() > parsed.end - parsed.append {
        return Err(invalid(
            "no erased space for policy variable; refusing compaction",
        ));
    }
    output[parsed.append..parsed.append + record.len()].copy_from_slice(&record);
    // The same strict reader must recognize exactly the requested candidate.
    if inspect(&output)?.enabled != Some(enabled) {
        return Err(invalid("candidate policy verification failed"));
    }
    Ok(output)
}

#[cfg(test)]
pub(super) fn test_store(enabled: bool) -> Vec<u8> {
    tests::fixture(&[
        tests::record(ENABLE_GUID, FLAG, 3, &[u8::from(enabled)], 0x3f),
        tests::record(GLOBAL, "Boot0001", 7, b"opaque boot payload", 0x3f),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    // Public disposable experiment certificate only; no private signing key.
    const CERT_HEX:&str="3082033930820221a003020102021404dd26541b57e6e6c1d38b2001a92968d517db91300d06092a864886f70d01010b0500302c312a302806035504030c217172616220646973706f7361626c652073656375726520626f6f742070726f6265301e170d3236313131343137353833325a170d3236313131353137353833325a302c312a302806035504030c217172616220646973706f7361626c652073656375726520626f6f742070726f626530820122300d06092a864886f70d01010105000382010f003082010a0282010100a4313b093bad8fe81e4ce4da38758d6916ab2bf985c0b43202dce91591890c74b1f3839e19b449db3cbf5933ecc2b0f1c4cf2efd44a118b85ce3a3c271abe2a882620136085abcc5057dd67996fe5ab686a8c664bb0af27f6b5523ecad208696ed9a0faf1b6b438b9501b1ef3d50b4c0f628eda96fed962981feca9578c9b1162d5103ada227317a87b72c6b69c531c14a6629546904f486af36d7a4915d793351692bec6ee12a43db9f42b0886b32149051da68e47fecd2cdeb4c4d564576cf61d23db14466a3a90e4be94c9913a877f05a369ed622a1011c5d3b20fd2bc04091bdc60aedb1d23caf6a3efdb0774e8614570b450d87f428cb3423912a85b7250203010001a3533051301d0603551d0e04160414470a505bc40a7d7218664eadecf30dcfb1c864fd301f0603551d23041830168014470a505bc40a7d7218664eadecf30dcfb1c864fd300f0603551d130101ff040530030101ff300d06092a864886f70d01010b0500038201010010a4282f9ba2f2bd17e726c1176fdf1c06eb3e161e3091e149932d45ff0b42b5f5f888a2c425da1808e0c8e806a4ad4e1cecd4c561a1fe3f99b4675f08110b9d15a585f54b75b843fe3b88584050b859e56cd2524e051842226239a5b28cb7518f7a4fd1dc8f7fed12924d2bb4dec35f5c4cb594949c4d3e071a4bf0623ade80254a1322729a559df75d1a5b77c336fe71eb39f1adf216365818c845e05b97fee3cfdfce2a5047cdd4291697a9c7cab003ee97b71925a399ca1cd01f268c9792bee09a3a6eaa324eac2e1c2763a5d372df71bfea4643fdf035e1eb9953fc9f52f64d61975d54a62e69f6c492acb970eef7b06fe461cc19d15b402052d04c7948";
    fn certificate() -> Vec<u8> {
        CERT_HEX
            .as_bytes()
            .chunks_exact(2)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
            .collect()
    }
    fn list(kind: [u8; 16], payload: &[u8]) -> Vec<u8> {
        let mut data = kind.to_vec();
        data.extend(((28 + 16 + payload.len()) as u32).to_le_bytes());
        data.extend(0u32.to_le_bytes());
        data.extend(((16 + payload.len()) as u32).to_le_bytes());
        data.extend([0u8; 16]);
        data.extend(payload);
        data
    }
    fn key() -> Vec<u8> {
        list(
            [
                0xa1, 0x59, 0xc0, 0xa5, 0xe4, 0x94, 0xa7, 0x4a, 0x87, 0xb5, 0xab, 0x15, 0x5c, 0x2b,
                0xf0, 0x72,
            ],
            &certificate(),
        )
    }
    pub(super) fn record(
        vendor: [u8; 16],
        name: &str,
        attr: u32,
        value: &[u8],
        state: u8,
    ) -> Vec<u8> {
        let mut out = vec![0u8; 60];
        out[..2].copy_from_slice(&0x55aau16.to_le_bytes());
        out[2] = state;
        out[4..8].copy_from_slice(&attr.to_le_bytes());
        out[44..60].copy_from_slice(&vendor);
        let encoded = name
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        out[36..40].copy_from_slice(&(encoded.len() as u32).to_le_bytes());
        out[40..44].copy_from_slice(&(value.len() as u32).to_le_bytes());
        out.extend(encoded);
        out.extend(value);
        while !out.len().is_multiple_of(4) {
            out.push(0xff);
        }
        out
    }
    pub(super) fn fixture(records: &[Vec<u8>]) -> Vec<u8> {
        let mut data = vec![0xff; 0x20000];
        data[..72].fill(0);
        data[16..32].copy_from_slice(&NV_DATA);
        data[32..40].copy_from_slice(&0x20000u64.to_le_bytes());
        data[40..44].copy_from_slice(b"_FVH");
        data[44..48].copy_from_slice(&0x4feffu32.to_le_bytes());
        data[48..50].copy_from_slice(&72u16.to_le_bytes());
        data[55] = 2;
        data[56..60].copy_from_slice(&32u32.to_le_bytes());
        data[60..64].copy_from_slice(&4096u32.to_le_bytes());
        let sum = data[..72].chunks_exact(2).fold(0u16, |s, b| {
            s.wrapping_add(u16::from_le_bytes([b[0], b[1]]))
        });
        data[50..52].copy_from_slice(&sum.wrapping_neg().to_le_bytes());
        data[72..100].fill(0);
        data[72..88].copy_from_slice(&AUTH_STORE);
        data[88..92].copy_from_slice(&(0xe000u32 - 72).to_le_bytes());
        data[92] = 0x5a;
        data[93] = 0xfe;
        let mut pos = 100;
        for record in records {
            data[pos..pos + record.len()].copy_from_slice(record);
            pos += record.len();
        }
        // Model unrelated FTW regions: must remain byte-identical.
        data[0xe000..].fill(0x71);
        data
    }
    fn enrolled(flag: bool) -> Vec<u8> {
        fixture(&[
            record(GLOBAL, "PK", 0x27, &key(), 0x3f),
            record(IMAGE_SECURITY, "db", 0x27, &key(), 0x3f),
            record(ENABLE_GUID, FLAG, 3, &[u8::from(flag)], 0x3f),
            record(GLOBAL, "Boot0001", 7, b"opaque boot payload", 0x3f),
        ])
    }
    #[test]
    fn existing_flag_changes_exactly_one_byte_and_preserves_all_other_regions() {
        let data = enrolled(false);
        let info = inspect(&data).unwrap();
        assert_eq!(
            info,
            Info {
                enabled: Some(false),
                has_pk: true,
                has_db: true
            }
        );
        let enabled = set_enabled(&data, true).unwrap();
        assert_eq!(inspect(&enabled).unwrap().enabled, Some(true));
        assert_eq!(data.iter().zip(&enabled).filter(|(a, b)| a != b).count(), 1);
        assert_eq!(set_enabled(&enabled, false).unwrap(), data);
    }
    #[test]
    fn empty_policy_can_be_added_but_missing_keys_never_enable_secure_boot() {
        assert_eq!(
            inspect(&super::test_store(false)).unwrap().enabled,
            Some(false)
        );
        let data = fixture(&[]);
        assert_eq!(inspect(&data).unwrap(), Info::default());
        assert!(set_enabled(&data, true).is_err());
        let disabled = set_enabled(&data, false).unwrap();
        assert_eq!(inspect(&disabled).unwrap().enabled, Some(false));
        assert_eq!(&disabled[0xe000..], &data[0xe000..]);
    }
    #[test]
    fn append_requires_erased_space_and_valid_existing_enrollment() {
        let no_flag = fixture(&[
            record(GLOBAL, "PK", 0x27, &key(), 0x3f),
            record(IMAGE_SECURITY, "db", 0x27, &key(), 0x3f),
        ]);
        assert_eq!(
            inspect(&set_enabled(&no_flag, true).unwrap())
                .unwrap()
                .enabled,
            Some(true)
        );
        let full = fixture(&[record(
            [9u8; 16],
            "Fill",
            7,
            &vec![0u8; 0xe000 - 100 - 70],
            0x3f,
        )]);
        assert!(set_enabled(&full, false).is_err());
        let mut dirty = fixture(&[]);
        dirty[0xe000 - 1] = 0;
        assert!(inspect(&dirty).is_err());
    }

    #[test]
    fn malformed_or_ambiguous_stores_and_keys_are_rejected() {
        let data = enrolled(true);
        let mut checksum = data.clone();
        checksum[50] ^= 1;
        assert!(inspect(&checksum).is_err());
        let mut format = data.clone();
        format[92] = 0;
        assert!(inspect(&format).is_err());
        let duplicate = record(ENABLE_GUID, FLAG, 3, &[1], 0x3f);
        assert!(inspect(&fixture(&[duplicate.clone(), duplicate])).is_err());
        assert!(inspect(&fixture(&[record(ENABLE_GUID, FLAG, 3, &[1], 0x3e)])).is_err());
        assert!(inspect(&fixture(&[record(
            GLOBAL,
            "PK",
            0x27,
            b"not a signature list",
            0x3f
        )]))
        .is_err());
        assert!(inspect(&fixture(&[record(
            GLOBAL,
            "PK",
            0x27,
            &list(
                [
                    0xa1, 0x59, 0xc0, 0xa5, 0xe4, 0x94, 0xa7, 0x4a, 0x87, 0xb5, 0xab, 0x15, 0x5c,
                    0x2b, 0xf0, 0x72
                ],
                b"not DER"
            ),
            0x3f
        )]))
        .is_err());
        assert!(inspect(&vec![0u8; MAX_STORE_BYTES + 1]).is_err());
    }
    #[test]
    fn old_deleted_records_are_preserved_and_never_selected() {
        let old = record(ENABLE_GUID, FLAG, 3, &[0], 0x3c);
        let active = record(ENABLE_GUID, FLAG, 3, &[1], 0x3f);
        let data = fixture(&[old.clone(), active]);
        let disabled = set_enabled(&data, false).unwrap();
        assert_eq!(&disabled[100..100 + old.len()], old.as_slice());
        assert_eq!(inspect(&disabled).unwrap().enabled, Some(false));
    }
    #[test]
    #[ignore = "requires disposable enrolled OVMF VARS supplied by QRAB_TEST_OVMF_VARS"]
    fn real_firmware_vars_preserve_keys_and_can_export_native_candidates() {
        let path = std::env::var("QRAB_TEST_OVMF_VARS").expect("set disposable VARS path");
        let data = std::fs::read(path).unwrap();
        let info = inspect(&data).unwrap();
        assert!(info.has_pk && info.has_db);
        let desired = !info.enabled.unwrap();
        let start = std::time::Instant::now();
        let candidate = set_enabled(&data, desired).unwrap();
        println!(
            "native inspect+edit elapsed {:?}, bytes {}",
            start.elapsed(),
            data.len()
        );
        assert_eq!(inspect(&candidate).unwrap().enabled, Some(desired));
        assert_eq!(
            data.iter().zip(&candidate).filter(|(a, b)| a != b).count(),
            1
        );
        if let Ok(path) = std::env::var("QRAB_TEST_OVMF_CANDIDATE") {
            std::fs::write(path, candidate).unwrap();
        }
    }
}

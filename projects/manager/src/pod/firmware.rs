//! Firmware state is reported by the guest's EFI variables, never inferred
//! from the configured boot mode or firmware filename.
use qapi_protocol::{qga::Qga, Result};
pub use qrab_guest_agent::proto::FirmwareState;
use qrab_guest_agent::proto::QrabFirmwareState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FirmwareSnapshot {
    pub observed_unix_ms: u64,
    pub state: FirmwareState,
}

impl FirmwareSnapshot {
    pub(super) async fn query(qga: &mut Qga) -> Result<Self> {
        let mut state = qga.execute(&QrabFirmwareState {}).await?;
        // A malformed/untrusted guest must not claim EFI state when it says
        // the EFI runtime interface is unavailable.
        if !state.efi_runtime_available {
            state = FirmwareState::default();
        }
        Ok(Self {
            observed_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            state,
        })
    }
}

// Secure Boot database payloads remain public certificates/hashes only. Full
// X.509 decoding rejects private-key and malformed ASN.1 blobs before Redfish.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecureBootDatabaseSnapshot {
    pub observed_unix_ms: u64,
    pub efi_runtime_available: bool,
    pub databases: Vec<SecureBootDatabaseInventory>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecureBootDatabaseInventory {
    pub id: String,
    pub present: Option<bool>,
    pub attributes: Option<u32>,
    pub signatures: Vec<SecureBootSignatureInventory>,
    pub unsupported_types: Vec<String>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecureBootSignatureInventory {
    pub id: String,
    pub signature_type: String,
    pub owner_guid: String,
    pub data_hex: String,
    pub certificate_pem: Option<String>,
    pub fingerprint_sha256: String,
}

pub use qrab_guest_agent::secure_boot::QrabSecureBootUpdate as SecureBootUpdateRequest;
pub fn validate_secure_boot_update(request: &SecureBootUpdateRequest) -> crate::Result<()> {
    qrab_guest_agent::secure_boot::validate_update(request)
        .map(|_| ())
        .map_err(|_| crate::Error::Config("invalid authenticated EFI update".into()))
}

impl SecureBootDatabaseSnapshot {
    pub(super) async fn update(qga: &mut Qga, request: SecureBootUpdateRequest) -> Result<Self> {
        qrab_guest_agent::secure_boot::validate_update(&request).map_err(|_| {
            qapi_protocol::Error::InvalidRequest("invalid authenticated EFI update".into())
        })?;
        let id = request.database_id.clone();
        let raw = qga.execute(&request).await?;
        let snapshot = database_snapshot(raw)?;
        if snapshot
            .databases
            .iter()
            .find(|database| database.id == id)
            .is_none_or(|database| database.error.is_some())
        {
            return Err(qapi_protocol::Error::InvalidRequest(
                "EFI update submitted but inventory could not confirm the result; outcome unknown"
                    .into(),
            ));
        }
        Ok(snapshot)
    }

    pub(super) async fn query(qga: &mut Qga) -> Result<Self> {
        let raw = qga
            .execute(&qrab_guest_agent::secure_boot::QrabSecureBootDatabases {})
            .await?;
        database_snapshot(raw)
    }
}

fn decode_signature_hex(hex: &str) -> Option<Vec<u8>> {
    if hex.len() > qrab_guest_agent::secure_boot::MAX_SIGNATURE_BYTES * 2
        || !hex.len().is_multiple_of(2)
    {
        return None;
    }
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digit = |byte: u8| match byte {
                b'0'..=b'9' => Some(byte - b'0'),
                b'a'..=b'f' => Some(byte - b'a' + 10),
                b'A'..=b'F' => Some(byte - b'A' + 10),
                _ => None,
            };
            Some(digit(pair[0])? * 16 + digit(pair[1])?)
        })
        .collect()
}
fn valid_guid(text: &str) -> bool {
    text.len() == 36
        && text.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}
fn signature_type(guid: &str) -> Option<(&'static str, Option<usize>)> {
    Some(match guid {
        qrab_guest_agent::secure_boot::X509_GUID => ("EFI_CERT_X509_GUID", None),
        "826ca512-cf10-4ac9-b187-be01496631bd" => ("EFI_CERT_SHA1_GUID", Some(20)),
        "0b6e5233-a65c-44c9-9407-d9ab83bfc8bd" => ("EFI_CERT_SHA224_GUID", Some(28)),
        "c1c41626-504c-4092-aca9-41f936934328" => ("EFI_CERT_SHA256_GUID", Some(32)),
        "ff3e5307-9fd0-48c9-85f1-8ad56c701e01" => ("EFI_CERT_SHA384_GUID", Some(48)),
        "093e0fae-a6c4-4f50-9f1b-d41e2b89c19a" => ("EFI_CERT_SHA512_GUID", Some(64)),
        "3bd2a492-96c0-4079-b420-fcf98ef103ed" => ("EFI_CERT_X509_SHA256_GUID", Some(48)),
        "7076876e-80c2-4ee6-aad2-28b349a6865b" => ("EFI_CERT_X509_SHA384_GUID", Some(64)),
        "446dbf63-2502-4cda-bcfa-2465d2b0fe9d" => ("EFI_CERT_X509_SHA512_GUID", Some(80)),
        "3c5766e8-269c-4e34-aa14-ed776e85b3b6" => ("EFI_CERT_RSA2048_GUID", Some(256)),
        "e2b36190-879b-4a3d-ad8d-f2e7bba32784" => ("EFI_CERT_RSA2048_SHA256_GUID", Some(528)),
        _ => return None,
    })
}
fn signature_inventory(
    raw: qrab_guest_agent::secure_boot::EfiSignature,
) -> Option<SecureBootSignatureInventory> {
    use x509_cert::der::{Decode, EncodePem};
    if !valid_guid(&raw.owner_guid) || !valid_guid(&raw.signature_type_guid) {
        return None;
    }
    let kind = raw.signature_type_guid.to_ascii_lowercase();
    let (name, size) = signature_type(&kind)?;
    let data = decode_signature_hex(&raw.data_hex)?;
    if data.is_empty() || size.is_some_and(|size| data.len() != size) {
        return None;
    }
    let certificate_pem = if kind == qrab_guest_agent::secure_boot::X509_GUID {
        Some(
            x509_cert::Certificate::from_der(&data)
                .ok()?
                .to_pem(x509_cert::der::pem::LineEnding::LF)
                .ok()?,
        )
    } else {
        None
    };
    let owner = raw.owner_guid.to_ascii_lowercase();
    let mut identity = Sha256::new();
    identity.update(kind.as_bytes());
    identity.update([0]);
    identity.update(owner.as_bytes());
    identity.update([0]);
    identity.update(&data);
    Some(SecureBootSignatureInventory {
        id: format!("{:x}", identity.finalize()),
        signature_type: name.into(),
        owner_guid: owner,
        data_hex: data.iter().map(|byte| format!("{byte:02x}")).collect(),
        certificate_pem,
        fingerprint_sha256: format!("{:x}", Sha256::digest(&data)),
    })
}
fn database_snapshot(
    raw: qrab_guest_agent::secure_boot::SecureBootDatabases,
) -> Result<SecureBootDatabaseSnapshot> {
    let mut snapshot = SecureBootDatabaseSnapshot {
        observed_unix_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX),
        efi_runtime_available: raw.efi_runtime_available,
        ..Default::default()
    };
    if !raw.efi_runtime_available {
        return Ok(snapshot);
    }
    if raw.databases.len() > 4 {
        return Err(qapi_protocol::Error::InvalidRequest(
            "too many Secure Boot databases".into(),
        ));
    }
    let mut names = std::collections::BTreeSet::new();
    let mut count = 0;
    let mut bytes = 0;
    for database in raw.databases {
        if !["PK", "KEK", "db", "dbx"].contains(&database.id.as_str())
            || !names.insert(database.id.clone())
        {
            return Err(qapi_protocol::Error::InvalidRequest(
                "invalid Secure Boot database identity".into(),
            ));
        }
        let mut output = SecureBootDatabaseInventory {
            id: database.id,
            present: database.present,
            attributes: database.attributes,
            unsupported_types: database
                .unsupported_types
                .into_iter()
                .filter(|g| valid_guid(g))
                .take(32)
                .collect(),
            error: database.error.map(|e| {
                if [
                    "Unavailable",
                    "Malformed",
                    "TooLarge",
                    "MalformedOrTooLarge",
                ]
                .contains(&e.as_str())
                {
                    e
                } else {
                    "Unavailable".into()
                }
            }),
            ..Default::default()
        };
        let mut ids = std::collections::BTreeSet::new();
        if output.present == Some(true) && output.error.is_none() {
            for signature in database.signatures {
                count += 1;
                bytes += signature.data_hex.len() / 2;
                if count > qrab_guest_agent::secure_boot::MAX_SIGNATURES
                    || bytes > qrab_guest_agent::secure_boot::MAX_TOTAL_BYTES
                {
                    return Err(qapi_protocol::Error::InvalidRequest(
                        "Secure Boot database snapshot exceeds bound".into(),
                    ));
                }
                if let Some(signature) = signature_inventory(signature) {
                    if ids.insert(signature.id.clone()) {
                        output.signatures.push(signature);
                    }
                } else {
                    output.error = Some("Malformed".into());
                }
            }
        }
        snapshot.databases.push(output);
    }
    Ok(snapshot)
}

// Firmware CODE artifacts are opt-in and immutable. This module never writes
// pflash, NVRAM, Secure Boot variables, TPM state or the identity manifest.
use super::Config;
use crate::{Error as ManagerError, Result as ManagerResult};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

const MAX_CATALOG: u64 = 64 * 1024;
const MAX_CODE: u64 = 64 * 1024 * 1024;
const STAGED: &str = "firmware-stage.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FirmwareArtifact {
    /// Approved build metadata, required for secure-capable targets.
    #[serde(default)]
    pub secure_boot_capable: bool,
    pub id: String,
    pub version: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub vars_size_bytes: u64,
    pub vars_template_sha256: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FirmwareCatalogView {
    pub enabled: bool,
    pub artifacts: Vec<FirmwareArtifact>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FirmwareStageStatus {
    pub enabled: bool,
    pub staged: Option<FirmwareArtifact>,
    /// `PendingRestart` until a live QEMU's selected CODE digest matches.
    pub activation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogEntry {
    #[serde(flatten)]
    artifact: FirmwareArtifact,
    file: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogManifest {
    schema: u32,
    artifacts: Vec<CatalogEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StagedFirmware {
    schema: u32,
    state_id: String,
    name: String,
    artifact: FirmwareArtifact,
}

struct Catalog {
    root: PathBuf,
    entries: Vec<CatalogEntry>,
    owner: u32,
}

fn invalid(message: impl Into<String>) -> ManagerError {
    ManagerError::Config(message.into())
}

fn open_regular(path: &Path, maximum: u64) -> ManagerResult<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() || metadata.len() > maximum {
        return Err(invalid(
            "firmware metadata/image is not a bounded regular file",
        ));
    }
    Ok(file)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> ManagerResult<T> {
    let mut bytes = Vec::new();
    open_regular(path, MAX_CATALOG)?
        .take(MAX_CATALOG + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_CATALOG {
        return Err(invalid("firmware metadata exceeds bound"));
    }
    Ok(serde_json::from_slice(&bytes)?)
}

fn trusted(metadata: &fs::Metadata, owner: u32) -> bool {
    metadata.uid() == owner && metadata.permissions().mode() & 0o222 == 0
}

fn identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        && id != "."
        && id != ".."
}

impl Catalog {
    fn from_env() -> ManagerResult<Option<Self>> {
        match std::env::var_os("QRAB_FIRMWARE_ROOT") {
            Some(root) => Self::load(Path::new(&root), 0).map(Some),
            None => Ok(None),
        }
    }
    fn load(root: &Path, owner: u32) -> ManagerResult<Self> {
        if !root.is_absolute() {
            return Err(invalid("QRAB_FIRMWARE_ROOT must be absolute"));
        }
        let root_meta = fs::symlink_metadata(root)?;
        if !root_meta.file_type().is_dir() || !trusted(&root_meta, owner) {
            return Err(invalid(
                "firmware catalog must be a root-owned nonwritable directory",
            ));
        }
        let manifest_file = open_regular(&root.join("manifest.json"), MAX_CATALOG)?;
        if !trusted(&manifest_file.metadata()?, owner) {
            return Err(invalid(
                "firmware catalog manifest must be root-owned and nonwritable",
            ));
        }
        let manifest: CatalogManifest =
            serde_json::from_reader(manifest_file.take(MAX_CATALOG + 1))?;
        if manifest.schema != 1 || manifest.artifacts.len() > 64 {
            return Err(invalid("unsupported or oversized firmware catalog"));
        }
        let mut ids = std::collections::BTreeSet::new();
        for entry in &manifest.artifacts {
            let a = &entry.artifact;
            if !identifier(&a.id)
                || !identifier(&entry.file)
                || !ids.insert(a.id.clone())
                || a.version.is_empty()
                || a.version.len() > 128
                || !a.version.bytes().all(|c| c.is_ascii_graphic() || c == b' ')
                || a.sha256.len() != 64
                || !a
                    .sha256
                    .bytes()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
                || a.vars_template_sha256.len() != 64
                || !a
                    .vars_template_sha256
                    .bytes()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
                || a.size_bytes == 0
                || a.size_bytes > MAX_CODE
                || a.vars_size_bytes == 0
                || a.vars_size_bytes > MAX_CODE
            {
                return Err(invalid(
                    "firmware catalog entry has invalid identity, digest or dimensions",
                ));
            }
        }
        Ok(Self {
            root: root.to_path_buf(),
            entries: manifest.artifacts,
            owner,
        })
    }
    fn view(&self) -> FirmwareCatalogView {
        FirmwareCatalogView {
            enabled: true,
            artifacts: self.entries.iter().map(|e| e.artifact.clone()).collect(),
        }
    }
    fn entry(&self, id: &str) -> ManagerResult<&CatalogEntry> {
        self.entries
            .iter()
            .find(|e| e.artifact.id == id)
            .ok_or_else(|| invalid("firmware artifact is not in the approved catalog"))
    }
    fn validate(&self, config: &Config, entry: &CatalogEntry) -> ManagerResult<PathBuf> {
        let uefi = config
            .uefi
            .as_ref()
            .ok_or_else(|| invalid("firmware staging requires UEFI boot"))?;
        let artifact = &entry.artifact;
        if fs::metadata(&uefi.code)?.len() != artifact.size_bytes
            || fs::metadata(&uefi.vars_template)?.len() != artifact.vars_size_bytes
        {
            return Err(invalid(
                "firmware CODE/VARS dimensions are incompatible with this VM",
            ));
        }
        if digest(open_regular(&uefi.vars_template, MAX_CODE)?)? != artifact.vars_template_sha256 {
            return Err(invalid(
                "firmware artifact does not approve this UEFI VARS template",
            ));
        }
        if uefi.secure_boot_capable && !entry.artifact.secure_boot_capable {
            return Err(invalid(
                "firmware artifact is not approved for a secure-capable target",
            ));
        }
        let vars = config.state_dir.join("identity/OVMF_VARS.fd");
        if open_regular(&vars, MAX_CODE)?.metadata()?.len() != artifact.vars_size_bytes {
            return Err(invalid(
                "firmware artifact is incompatible with the retained NVRAM store",
            ));
        }
        let path = self.root.join(&entry.file);
        let file = open_regular(&path, MAX_CODE)?;
        let metadata = file.metadata()?;
        if !trusted(&metadata, self.owner) || metadata.len() != artifact.size_bytes {
            return Err(invalid(
                "firmware CODE artifact must be root-owned, nonwritable and exact-size",
            ));
        }
        if digest(file)? != artifact.sha256 {
            return Err(invalid("firmware CODE artifact SHA256 mismatch"));
        }
        Ok(path)
    }
}

fn digest(mut file: File) -> ManagerResult<String> {
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn identity(config: &Config) -> ManagerResult<PathBuf> {
    let directory = config.state_dir.join("identity");
    let metadata = fs::symlink_metadata(&directory)?;
    if !metadata.file_type().is_dir() || metadata.permissions().mode() & 0o077 != 0 {
        return Err(invalid(
            "firmware staging requires an existing private VM identity directory",
        ));
    }
    let manifest: serde_json::Value = read_json(&directory.join("manifest.json"))?;
    if manifest["version"] != 1
        || manifest["state_id"] != config.state_id.as_deref().unwrap_or(&config.name)
        || manifest["name"] != config.name
        || manifest["uefi"] != true
    {
        return Err(invalid("firmware staging identity does not match this VM"));
    }
    Ok(directory)
}

fn staged(config: &Config) -> ManagerResult<Option<StagedFirmware>> {
    let path = config.state_dir.join("identity").join(STAGED);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
        Ok(metadata) => {
            if !metadata.file_type().is_file() {
                return Err(invalid("staged firmware metadata is not a regular file"));
            }
        }
    }
    identity(config)?;
    let stage: StagedFirmware = read_json(&path)?;
    if stage.schema != 1
        || stage.state_id != config.state_id.as_deref().unwrap_or(&config.name)
        || stage.name != config.name
    {
        return Err(invalid(
            "staged firmware belongs to a different VM identity",
        ));
    }
    Ok(Some(stage))
}

pub fn firmware_catalog() -> ManagerResult<FirmwareCatalogView> {
    Ok(Catalog::from_env()?.map(|c| c.view()).unwrap_or_default())
}

pub fn stage_firmware(config: &Config, id: &str) -> ManagerResult<FirmwareStageStatus> {
    let catalog = Catalog::from_env()?
        .ok_or_else(|| invalid("firmware artifact catalog is not configured"))?;
    stage_with(config, id, &catalog)
}

fn stage_with(config: &Config, id: &str, catalog: &Catalog) -> ManagerResult<FirmwareStageStatus> {
    let directory = identity(config)?;
    let entry = catalog.entry(id)?;
    catalog.validate(config, entry)?;
    let stage = StagedFirmware {
        schema: 1,
        state_id: config.state_id.as_deref().unwrap_or(&config.name).into(),
        name: config.name.clone(),
        artifact: entry.artifact.clone(),
    };
    let mut candidate = tempfile::NamedTempFile::new_in(&directory)?;
    candidate
        .as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    serde_json::to_writer(&mut candidate, &stage)?;
    candidate.write_all(b"\n")?;
    candidate.as_file().sync_all()?;
    candidate
        .persist(directory.join(STAGED))
        .map_err(|error| ManagerError::Io(error.error))?;
    File::open(&directory)?.sync_all()?;
    Ok(FirmwareStageStatus {
        enabled: true,
        staged: Some(entry.artifact.clone()),
        activation: Some("PendingRestart".into()),
    })
}

/// `running` must be true only after a live QEMU has reached Running using
/// this effective config; selecting an artifact for a future boot is not proof
/// of activation. No next-boot request is ever described as already applied.
pub fn firmware_stage_status(config: &Config, running: bool) -> ManagerResult<FirmwareStageStatus> {
    let catalog = Catalog::from_env()?;
    let Some(stage) = staged(config)? else {
        return Ok(FirmwareStageStatus {
            enabled: catalog.is_some(),
            ..Default::default()
        });
    };
    let catalog = catalog
        .ok_or_else(|| invalid("staged firmware requires its configured immutable catalog"))?;
    status_with(config, &stage, &catalog, running)
}

fn status_with(
    config: &Config,
    stage: &StagedFirmware,
    catalog: &Catalog,
    running: bool,
) -> ManagerResult<FirmwareStageStatus> {
    let entry = catalog.entry(&stage.artifact.id)?;
    if stage.artifact != entry.artifact {
        return Err(invalid(
            "approved firmware catalog entry changed after staging",
        ));
    }
    catalog.validate(config, entry)?;
    let active = running
        && config
            .uefi
            .as_ref()
            .map(|u| open_regular(&u.code, MAX_CODE).and_then(digest))
            .transpose()?
            .as_deref()
            == Some(stage.artifact.sha256.as_str());
    Ok(FirmwareStageStatus {
        enabled: true,
        staged: Some(stage.artifact.clone()),
        activation: Some(if active { "Active" } else { "PendingRestart" }.into()),
    })
}

/// Startup-only: call while holding the VM's persistent-state lease, before
/// launching QEMU, and republish/revalidate the effective Config afterwards.
/// The retained identity, VARS and TPM are never modified by this selection.
pub fn apply_staged_firmware(config: &mut Config) -> ManagerResult<()> {
    let Some(stage) = staged(config)? else {
        return Ok(());
    };
    let catalog = Catalog::from_env()?
        .ok_or_else(|| invalid("staged firmware requires its configured immutable catalog"))?;
    apply_with(config, &stage, &catalog)
}

fn apply_with(config: &mut Config, stage: &StagedFirmware, catalog: &Catalog) -> ManagerResult<()> {
    let entry = catalog.entry(&stage.artifact.id)?;
    if stage.artifact != entry.artifact {
        return Err(invalid(
            "approved firmware catalog entry changed after staging",
        ));
    }
    let path = catalog.validate(config, entry)?;
    config
        .uefi
        .as_mut()
        .ok_or_else(|| invalid("staged firmware requires UEFI"))?
        .code = path;
    Ok(())
}

#[cfg(test)]
mod catalog_tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, Config, Catalog) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("catalog");
        let state_dir = temp.path().join("state");
        let identity = state_dir.join("identity");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&identity).unwrap();
        fs::set_permissions(&identity, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(
            identity.join("manifest.json"),
            br#"{"version":1,"state_id":"vm-one","name":"test","uefi":true}"#,
        )
        .unwrap();
        fs::write(identity.join("OVMF_VARS.fd"), [1; 16]).unwrap();
        let current_code = temp.path().join("current-code.fd");
        let vars_template = temp.path().join("vars.fd");
        fs::write(&current_code, [2; 32]).unwrap();
        fs::write(&vars_template, [3; 16]).unwrap();
        fs::write(root.join("code.fd"), [4; 32]).unwrap();
        let sha256 = digest(File::open(root.join("code.fd")).unwrap()).unwrap();
        let vars_template_sha256 = digest(File::open(&vars_template).unwrap()).unwrap();
        let manifest = serde_json::json!({"schema":1,"artifacts":[{
            "id":"approved-v2","version":"2.0","sha256":sha256,"file":"code.fd","size_bytes":32,"vars_size_bytes":16,"vars_template_sha256":vars_template_sha256
        }]});
        fs::write(root.join("manifest.json"), manifest.to_string()).unwrap();
        for path in [&root, &root.join("manifest.json"), &root.join("code.fd")] {
            fs::set_permissions(
                path,
                fs::Permissions::from_mode(if path == &root { 0o555 } else { 0o444 }),
            )
            .unwrap();
        }
        let owner = fs::metadata(&root).unwrap().uid();
        let catalog = Catalog::load(&root, owner).unwrap();
        let config = serde_json::from_value(
            serde_json::json!({"name":"test","state_id":"vm-one","state_dir":state_dir,
            "uefi":{"code":current_code,"vars_template":vars_template,"boot_disk":"/unused"}}),
        )
        .unwrap();
        (temp, config, catalog)
    }
    #[test]
    fn stage_is_identity_bound_atomic_and_preserves_nvram_and_tpm_material() {
        let (_temp, mut config, catalog) = fixture();
        let vars = config.state_dir.join("identity/OVMF_VARS.fd");
        let before = fs::read(&vars).unwrap();
        let tpm = config.state_dir.join("identity/tpm");
        fs::create_dir(&tpm).unwrap();
        fs::write(tpm.join("identity"), b"unchanged TPM identity").unwrap();
        let status = stage_with(&config, "approved-v2", &catalog).unwrap();
        assert_eq!(status.activation.as_deref(), Some("PendingRestart"));
        assert_eq!(
            fs::metadata(config.state_dir.join("identity").join(STAGED))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let stage = staged(&config).unwrap().unwrap();
        assert_eq!(
            status_with(&config, &stage, &catalog, true)
                .unwrap()
                .activation
                .as_deref(),
            Some("PendingRestart")
        );
        let old_code = config.uefi.as_ref().unwrap().code.clone();
        assert_ne!(old_code, catalog.root.join("code.fd"));
        apply_with(&mut config, &stage, &catalog).unwrap();
        assert_eq!(
            config.uefi.as_ref().unwrap().code,
            catalog.root.join("code.fd")
        );
        assert_eq!(
            status_with(&config, &stage, &catalog, false)
                .unwrap()
                .activation
                .as_deref(),
            Some("PendingRestart")
        );
        assert_eq!(
            status_with(&config, &stage, &catalog, true)
                .unwrap()
                .activation
                .as_deref(),
            Some("Active")
        );
        assert_eq!(fs::read(vars).unwrap(), before);
        assert_eq!(
            fs::read(tpm.join("identity")).unwrap(),
            b"unchanged TPM identity"
        );
        config.state_id = Some("different-vm".into());
        assert!(staged(&config).is_err());
    }
    #[test]
    fn digest_layout_and_catalog_identity_are_revalidated_before_boot() {
        let (_temp, mut config, catalog) = fixture();
        stage_with(&config, "approved-v2", &catalog).unwrap();
        let stage = staged(&config).unwrap().unwrap();
        fs::set_permissions(
            catalog.root.join("code.fd"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        fs::write(catalog.root.join("code.fd"), [5; 32]).unwrap();
        fs::set_permissions(
            catalog.root.join("code.fd"),
            fs::Permissions::from_mode(0o444),
        )
        .unwrap();
        assert!(apply_with(&mut config, &stage, &catalog).is_err());
        fs::set_permissions(
            catalog.root.join("code.fd"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        fs::write(catalog.root.join("code.fd"), [4; 32]).unwrap();
        fs::set_permissions(
            catalog.root.join("code.fd"),
            fs::Permissions::from_mode(0o444),
        )
        .unwrap();
        fs::write(config.uefi.as_ref().unwrap().vars_template.clone(), [9; 16]).unwrap();
        assert!(apply_with(&mut config, &stage, &catalog).is_err());
        fs::write(config.uefi.as_ref().unwrap().vars_template.clone(), [3; 17]).unwrap();
        assert!(apply_with(&mut config, &stage, &catalog).is_err());
    }
    #[test]
    fn writable_catalog_and_symlink_code_are_rejected() {
        let (_temp, config, catalog) = fixture();
        fs::set_permissions(&catalog.root, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(Catalog::load(&catalog.root, catalog.owner).is_err());
        fs::remove_file(catalog.root.join("code.fd")).unwrap();
        std::os::unix::fs::symlink(
            &config.uefi.as_ref().unwrap().code,
            catalog.root.join("code.fd"),
        )
        .unwrap();
        assert!(catalog.validate(&config, &catalog.entries[0]).is_err());
    }
}

#[cfg(test)]
mod database_tests {
    use super::*;
    fn wire(kind: &str, data: &[u8]) -> qrab_guest_agent::secure_boot::EfiSignature {
        qrab_guest_agent::secure_boot::EfiSignature {
            signature_type_guid: kind.into(),
            owner_guid: "12345678-1234-5678-90ab-cdef12345678".into(),
            data_hex: data.iter().map(|b| format!("{b:02x}")).collect(),
        }
    }
    #[test]
    fn valid_certificates_get_content_stable_ids_and_private_key_blobs_are_rejected() {
        let der =
            include_bytes!("../../../qrab-guest-agent/tests/fixtures/secureboot-test-cert.der");
        let raw = wire(qrab_guest_agent::secure_boot::X509_GUID, der);
        let item = signature_inventory(raw.clone()).unwrap();
        assert_eq!(item.id.len(), 64);
        assert_eq!(item.fingerprint_sha256.len(), 64);
        assert!(item
            .certificate_pem
            .as_ref()
            .unwrap()
            .starts_with("-----BEGIN CERTIFICATE-----"));
        let mut uppercase = raw;
        uppercase.data_hex.make_ascii_uppercase();
        uppercase.owner_guid.make_ascii_uppercase();
        assert_eq!(signature_inventory(uppercase).unwrap().id, item.id);
        assert!(signature_inventory(wire(
            qrab_guest_agent::secure_boot::X509_GUID,
            &[0x30, 0x03, 0x02, 0x01, 0]
        ))
        .is_none());
    }
    #[test]
    fn database_identity_duplicates_and_malformed_hashes_are_not_trusted() {
        let signature = wire("c1c41626-504c-4092-aca9-41f936934328", &[3; 32]);
        let database = qrab_guest_agent::secure_boot::SecureBootDatabase {
            id: "dbx".into(),
            present: Some(true),
            signatures: vec![signature.clone(), signature],
            ..Default::default()
        };
        let snapshot = database_snapshot(qrab_guest_agent::secure_boot::SecureBootDatabases {
            efi_runtime_available: true,
            databases: vec![database.clone()],
        })
        .unwrap();
        assert_eq!(snapshot.databases[0].signatures.len(), 1);
        assert!(
            database_snapshot(qrab_guest_agent::secure_boot::SecureBootDatabases {
                efi_runtime_available: true,
                databases: vec![database.clone(), database]
            })
            .is_err()
        );
        assert!(
            signature_inventory(wire("c1c41626-504c-4092-aca9-41f936934328", &[3; 31])).is_none()
        );
        let unknown =
            database_snapshot(qrab_guest_agent::secure_boot::SecureBootDatabases::default())
                .unwrap();
        assert!(!unknown.efi_runtime_available);
        assert!(unknown.databases.is_empty());
    }
}

//! Provisioned VM disks and sealed CoW base images behind the storage daemon.
//!
//! Every operation is keyed by VM UID and disk ID, takes the global journal
//! lock and checks recorded ZFS GUIDs/ownership before mutating anything.
//! Credentials arrive only through [`export`]'s argument (the CLI reads stdin)
//! and never appear in reports, argv or errors.
use crate::{
    fence_record, prepare_pool, valid_password, Backend, Origin, Phase, Record, Refusal, Report,
    Spec, Store, MIN_VOLUME, OWNER,
};
use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::{fs::File, io::Read as _, os::unix::fs::OpenOptionsExt as _, path::Path};

/// Largest raw file `vm-image-import` accepts.
pub const MAX_IMPORT: u64 = 64 << 30;

/// The zvol size of an imported raw file: provisioned disks' whole-MiB
/// alignment with their 16MiB floor, so every clone size stays valid.
pub fn import_capacity(size_bytes: u64) -> u64 {
    size_bytes
        .div_ceil(1 << 20)
        .saturating_mul(1 << 20)
        .max(MIN_VOLUME)
}

/// The verified raw file an imported generation was written from.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ImportSource {
    /// Lowercase hex SHA-256 of the raw bytes written to the zvol.
    pub sha256: String,
    pub size_bytes: u64,
}
impl ImportSource {
    fn validate(&self) -> Result<()> {
        ensure!(valid_sha256(&self.sha256), Refusal("invalid-sha256"));
        ensure!(self.size_bytes > 0, Refusal("source-empty"));
        ensure!(
            self.size_bytes.is_multiple_of(512),
            Refusal("source-misaligned")
        );
        ensure!(self.size_bytes <= MAX_IMPORT, Refusal("source-too-large"));
        Ok(())
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImagePhase {
    Publishing,
    Sealed,
    /// No new clones; data retained for explicit offline disposal.
    Retired,
}

/// Root journal entry for one sealed base-image generation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub schema: u32,
    pub owner: String,
    pub origin: Origin,
    pub capacity_bytes: u64,
    /// Published generations name their fenced source disk. Imports leave
    /// these three empty and record [`Image::import`] instead.
    pub source_vm_uid: String,
    pub source_disk: String,
    pub source_guid: String,
    /// Absent (and not serialized) for published generations, so their
    /// journal stays byte-compatible with earlier releases.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import: Option<ImportSource>,
    pub phase: ImagePhase,
    pub dataset_guid: Option<String>,
    pub snapshot_guid: Option<String>,
}
impl Image {
    pub(crate) fn validate(&self) -> Result<()> {
        self.origin.validate()?;
        match &self.import {
            None => {
                Spec::disk(
                    self.source_vm_uid.clone(),
                    self.source_disk.clone(),
                    self.capacity_bytes,
                    None,
                )?;
                ensure!(
                    !self.source_guid.is_empty()
                        && self.source_guid.bytes().all(|b| b.is_ascii_digit()),
                    "image manifest is malformed"
                );
            }
            Some(import) => {
                import.validate()?;
                ensure!(
                    self.source_vm_uid.is_empty()
                        && self.source_disk.is_empty()
                        && self.source_guid.is_empty()
                        && self.capacity_bytes == import_capacity(import.size_bytes),
                    "image manifest is malformed"
                );
            }
        }
        let digits = |g: &Option<String>| {
            g.as_ref()
                .is_none_or(|g| !g.is_empty() && g.bytes().all(|b| b.is_ascii_digit()))
        };
        ensure!(
            self.schema == 2
                && self.owner == OWNER
                && digits(&self.dataset_guid)
                && digits(&self.snapshot_guid)
                && (self.phase == ImagePhase::Publishing
                    || (self.dataset_guid.is_some() && self.snapshot_guid.is_some())),
            "image manifest is malformed"
        );
        Ok(())
    }
    pub fn report(&self) -> ImageReport {
        ImageReport {
            schema: 2,
            image: self.origin.image.clone(),
            generation: self.origin.generation,
            capacity_bytes: self.capacity_bytes,
            dataset: self.origin.dataset(),
            snapshot: self.origin.snapshot(),
            phase: self.phase,
            source_vm_uid: self.source_vm_uid.clone(),
            source_disk: self.source_disk.clone(),
            import: self.import.clone(),
        }
    }
}

#[derive(Serialize)]
pub struct ImageReport {
    pub schema: u32,
    pub image: String,
    pub generation: u32,
    pub capacity_bytes: u64,
    pub dataset: String,
    pub snapshot: String,
    pub phase: ImagePhase,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source_vm_uid: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source_disk: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import: Option<ImportSource>,
}

/// Total logical zvol size stays within the dedicated parent quota, so thin
/// clones cannot overcommit the pool into guest-visible ENOSPC.
fn budget<B: Backend>(host: &mut B, additional: u64) -> Result<()> {
    let quota = host.parent_quota()?;
    let used = host.logical_bytes()?;
    ensure!(
        used.checked_add(additional)
            .is_some_and(|total| total <= quota),
        Refusal("budget-exhausted")
    );
    Ok(())
}

fn existing(store: &Store, uid: &str, disk: &str) -> Result<Record> {
    store
        .read_disk(uid, disk)?
        .ok_or_else(|| Refusal("disk-not-found").into())
}

/// Finish (or verify) a provisioned record. A record without a GUID is an
/// interrupted creation and resumes against our own ownership markers only.
fn provision<B: Backend>(
    store: &Store,
    host: &mut B,
    spec: Spec,
    image: Option<&Image>,
) -> Result<Report> {
    prepare_pool(store, host)?;
    let mut record = match store.read_disk(&spec.vm_uid, spec.disk.as_deref().unwrap_or(""))? {
        Some(record) => {
            ensure!(record.spec.origin == spec.origin, Refusal("disk-conflict"));
            if record.zvol_guid.is_some() {
                // Retries are idempotent; capacity changes go through expand.
                ensure!(
                    record.spec.capacity_bytes == spec.capacity_bytes,
                    Refusal("disk-conflict")
                );
                host.verify_volume(&record)?;
                return Ok(record.report());
            }
            ensure!(record.spec == spec, Refusal("disk-conflict"));
            record
        }
        None => {
            host.preflight_new_volume(&spec)?;
            budget(host, spec.capacity_bytes)?;
            let record = Record {
                schema: 2,
                owner: OWNER.into(),
                spec,
                generation: 0,
                phase: Phase::Provisioned,
                zvol_guid: None,
                origin_guid: None,
                chap_password: String::new(),
            };
            store.write(&record)?;
            record
        }
    };
    let guid = match image {
        Some(image) => {
            let guid = host.ensure_clone(&record.spec, image, None)?;
            record.origin_guid = image.snapshot_guid.clone();
            guid
        }
        None => host.ensure_volume(&record.spec, None)?,
    };
    record.zvol_guid = Some(guid);
    store.write(&record)?;
    host.verify_volume(&record)?;
    Ok(record.report())
}

/// Blank, thick-reserved zvol. Never exported until [`export`].
pub fn create<B: Backend>(
    store: &Store,
    host: &mut B,
    uid: &str,
    disk: &str,
    capacity_bytes: u64,
) -> Result<Report> {
    let spec = Spec::disk(uid.into(), disk.into(), capacity_bytes, None)?;
    let _lock = store.lock()?;
    provision(store, host, spec, None)
}

/// CoW clone of a sealed image generation. The clone holds the image snapshot
/// so it cannot be destroyed underneath, and its origin GUID is recorded.
pub fn clone<B: Backend>(
    store: &Store,
    host: &mut B,
    uid: &str,
    disk: &str,
    origin: Origin,
    capacity_bytes: u64,
) -> Result<Report> {
    let spec = Spec::disk(
        uid.into(),
        disk.into(),
        capacity_bytes,
        Some(origin.clone()),
    )?;
    let _lock = store.lock()?;
    let image = store
        .read_image(&origin)?
        .filter(|i| i.phase != ImagePhase::Publishing)
        .ok_or(Refusal("image-not-published"))?;
    ensure!(image.phase == ImagePhase::Sealed, Refusal("image-retired"));
    ensure!(
        capacity_bytes >= image.capacity_bytes,
        Refusal("clone-smaller-than-image")
    );
    host.verify_image(&image)?;
    provision(store, host, spec, Some(&image))
}

/// Grow only. The guest sees the new size after its next QEMU start.
pub fn expand<B: Backend>(
    store: &Store,
    host: &mut B,
    uid: &str,
    disk: &str,
    capacity_bytes: u64,
) -> Result<Report> {
    let _lock = store.lock()?;
    let mut record = existing(store, uid, disk)?;
    ensure!(
        !matches!(record.phase, Phase::Fencing | Phase::Retired),
        Refusal("disk-unavailable")
    );
    ensure!(
        record.zvol_guid.is_some(),
        Refusal("disk-provisioning-incomplete")
    );
    ensure!(
        capacity_bytes >= record.spec.capacity_bytes,
        Refusal("shrink-refused")
    );
    let mut grown = record.spec.clone();
    grown.capacity_bytes = capacity_bytes;
    grown.validate()?;
    if capacity_bytes > record.spec.capacity_bytes {
        budget(host, capacity_bytes - record.spec.capacity_bytes)?;
        host.expand(&record, capacity_bytes)?;
        record.spec = grown;
        store.write(&record)?;
    }
    host.verify_volume(&record)?;
    if record.phase == Phase::Active {
        host.verify_active(&record)?;
    }
    Ok(record.report())
}

/// Export with an exact VM-initiator ACL and the caller's CHAP secret.
///
/// Generation 1 is the first export. An active export accepts only its own
/// generation and secret. After a verified fence, re-export requires the next
/// generation and a different secret, so revoked credentials never return.
pub fn export<B: Backend>(
    store: &Store,
    host: &mut B,
    uid: &str,
    disk: &str,
    generation: u64,
    password: &str,
) -> Result<Report> {
    ensure!(valid_password(password), Refusal("invalid-credential"));
    let _lock = store.lock()?;
    let mut record = existing(store, uid, disk)?;
    ensure!(
        record.zvol_guid.is_some(),
        Refusal("disk-provisioning-incomplete")
    );
    match record.phase {
        Phase::Retired => bail!(Refusal("disk-retired")),
        Phase::Fencing => bail!(Refusal("fence-incomplete")),
        Phase::Active => {
            ensure!(
                generation == record.generation && password == record.chap_password,
                Refusal("fence-required")
            );
            host.verify_volume(&record)?;
            host.verify_active(&record)?;
            return Ok(record.report());
        }
        Phase::Preparing => ensure!(
            generation == record.generation && password == record.chap_password,
            Refusal("generation-mismatch")
        ),
        Phase::Provisioned => {
            ensure!(generation == 1, Refusal("generation-mismatch"));
            host.verify_volume(&record)?;
            record.generation = 1;
            record.chap_password = password.into();
            record.phase = Phase::Preparing;
            store.write(&record)?;
        }
        Phase::Fenced => {
            ensure!(
                Some(generation) == record.generation.checked_add(1),
                Refusal("generation-mismatch")
            );
            ensure!(
                password != record.chap_password,
                Refusal("credential-reused")
            );
            host.verify_fenced(&record)?;
            record.generation = generation;
            record.chap_password = password.into();
            record.phase = Phase::Preparing;
            store.write(&record)?;
        }
    }
    host.verify_volume(&record)?;
    host.activate(&record)?;
    host.verify_active(&record)?;
    record.phase = Phase::Active;
    store.write(&record)?;
    Ok(record.report())
}

/// Kernel ACL/LUN drain barrier; see [`crate::fence`].
pub fn fence<B: Backend>(store: &Store, host: &mut B, uid: &str, disk: &str) -> Result<Report> {
    let _lock = store.lock()?;
    let record = existing(store, uid, disk)?;
    fence_record(store, host, record)
}

/// Verified live state. A missing journal is `disk-not-found`.
pub fn status<B: Backend>(store: &Store, host: &mut B, uid: &str, disk: &str) -> Result<Report> {
    let _lock = store.lock()?;
    let record = existing(store, uid, disk)?;
    if record.zvol_guid.is_some() {
        host.verify_volume(&record)?;
    }
    match record.phase {
        Phase::Active => host.verify_active(&record)?,
        Phase::Fenced => host.verify_fenced(&record)?,
        _ => {}
    }
    Ok(record.report())
}

/// Administrative: tear down a fenced export and keep the data as retained.
pub fn retire<B: Backend>(store: &Store, host: &mut B, uid: &str, disk: &str) -> Result<Report> {
    let _lock = store.lock()?;
    let mut record = existing(store, uid, disk)?;
    match record.phase {
        Phase::Retired => {
            host.verify_volume(&record)?;
            return Ok(record.report());
        }
        Phase::Fenced | Phase::Provisioned => {}
        _ => bail!(Refusal("fence-required")),
    }
    ensure!(
        record.zvol_guid.is_some(),
        Refusal("disk-provisioning-incomplete")
    );
    if record.phase == Phase::Fenced {
        host.verify_fenced(&record)?;
    }
    host.retire(&record)?;
    record.phase = Phase::Retired;
    store.write(&record)?;
    Ok(record.report())
}

/// Administrative: copy a fenced (quiesced) disk into a sealed image
/// generation. Generations are immutable once sealed.
pub fn publish<B: Backend>(
    store: &Store,
    host: &mut B,
    origin: Origin,
    source_uid: &str,
    source_disk: &str,
) -> Result<ImageReport> {
    origin.validate()?;
    let _lock = store.lock()?;
    let source = existing(store, source_uid, source_disk)?;
    ensure!(source.phase == Phase::Fenced, Refusal("source-not-fenced"));
    host.verify_volume(&source)?;
    host.verify_fenced(&source)?;
    let source_guid = source
        .zvol_guid
        .clone()
        .ok_or(Refusal("disk-provisioning-incomplete"))?;
    let mut image = match store.read_image(&origin)? {
        Some(image) => {
            ensure!(
                image.source_vm_uid == source_uid
                    && image.source_disk == source_disk
                    && image.source_guid == source_guid
                    && image.capacity_bytes == source.spec.capacity_bytes,
                Refusal("image-generation-exists")
            );
            match image.phase {
                ImagePhase::Sealed => {
                    host.verify_image(&image)?;
                    return Ok(image.report());
                }
                ImagePhase::Retired => bail!(Refusal("image-retired")),
                ImagePhase::Publishing => image,
            }
        }
        None => {
            budget(host, source.spec.capacity_bytes)?;
            let image = Image {
                schema: 2,
                owner: OWNER.into(),
                origin,
                capacity_bytes: source.spec.capacity_bytes,
                source_vm_uid: source_uid.into(),
                source_disk: source_disk.into(),
                source_guid,
                import: None,
                phase: ImagePhase::Publishing,
                dataset_guid: None,
                snapshot_guid: None,
            };
            store.write_image(&image)?;
            image
        }
    };
    host.publish_image(&source, &mut image)?;
    ensure!(
        image.dataset_guid.is_some() && image.snapshot_guid.is_some(),
        "image publication did not record its GUIDs"
    );
    image.phase = ImagePhase::Sealed;
    store.write_image(&image)?;
    host.verify_image(&image)?;
    Ok(image.report())
}

/// Opens the caller-named raw file without following a final symlink or
/// blocking on a FIFO, then checks the opened file itself, not the path.
fn open_import_source(path: &Path, sha256: String) -> Result<(File, ImportSource)> {
    ensure!(path.is_absolute(), Refusal("invalid-source"));
    let flags =
        nix::fcntl::OFlag::O_NOFOLLOW | nix::fcntl::OFlag::O_NONBLOCK | nix::fcntl::OFlag::O_NOCTTY;
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(flags.bits())
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.raw_os_error() == Some(nix::errno::Errno::ELOOP as i32) => {
            bail!(Refusal("source-not-regular"))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            bail!(Refusal("source-not-found"))
        }
        Err(error) => return Err(anyhow::Error::new(error).context("open the import source")),
    };
    let meta = file.metadata()?;
    ensure!(meta.file_type().is_file(), Refusal("source-not-regular"));
    let source = ImportSource {
        sha256,
        size_bytes: meta.len(),
    };
    source.validate()?;
    Ok((file, source))
}

/// SHA-256 and length of exactly the bytes handed to the device writer.
struct Hashing<R> {
    inner: R,
    hasher: Sha256,
    count: u64,
}
impl<R: std::io::Read> std::io::Read for Hashing<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.hasher.update(&buf[..n]);
        self.count += n as u64;
        Ok(n)
    }
}

/// Administrative and root-run only, never a storage-daemon verb: create a
/// sealed generation from a raw image file with a known SHA-256. The result
/// is indistinguishable from a published generation to status, clone and
/// retire; its journal records the file's digest and size instead of a VM.
pub fn import<B: Backend>(
    store: &Store,
    host: &mut B,
    origin: Origin,
    source: &Path,
    sha256: &str,
) -> Result<ImageReport> {
    origin.validate()?;
    let sha256 = sha256.to_ascii_lowercase();
    ensure!(valid_sha256(&sha256), Refusal("invalid-sha256"));
    let (file, source) = open_import_source(source, sha256)?;
    let _lock = store.lock()?;
    prepare_pool(store, host)?;
    if let Some(existing) = store.read_image(&origin)? {
        // Only an interrupted import is ours to discard and redo.
        ensure!(
            existing.import.is_some() && existing.phase == ImagePhase::Publishing,
            Refusal("image-generation-exists")
        );
        discard_import(store, host, &existing)?;
    }
    let mut image = Image {
        schema: 2,
        owner: OWNER.into(),
        origin,
        capacity_bytes: import_capacity(source.size_bytes),
        source_vm_uid: String::new(),
        source_disk: String::new(),
        source_guid: String::new(),
        import: Some(source),
        phase: ImagePhase::Publishing,
        dataset_guid: None,
        snapshot_guid: None,
    };
    image.validate()?;
    host.preflight_new_image(&image)?;
    budget(host, image.capacity_bytes)?;
    store.write_image(&image)?;
    if let Err(error) = write_and_seal(store, host, &mut image, file) {
        // Nothing half-made may remain: destroy our new dataset, then its journal.
        return Err(match discard_import(store, host, &image) {
            Ok(()) => error,
            Err(cleanup) => error.context(format!(
                "cleanup of the new dataset failed ({cleanup:#}); rerun the same import to finish it"
            )),
        });
    }
    Ok(image.report())
}

fn write_and_seal<B: Backend>(
    store: &Store,
    host: &mut B,
    image: &mut Image,
    file: File,
) -> Result<()> {
    let source = image
        .import
        .clone()
        .ok_or_else(|| anyhow::anyhow!("not an import"))?;
    image.dataset_guid = Some(host.create_image_volume(image)?);
    // From here a crash leaves a journal that the next import cleans up.
    store.write_image(image)?;
    let mut reader = Hashing {
        inner: file.take(source.size_bytes),
        hasher: Sha256::new(),
        count: 0,
    };
    let written = host.write_image(image, &mut reader)?;
    ensure!(
        written == source.size_bytes && reader.count == source.size_bytes,
        Refusal("source-changed")
    );
    ensure!(
        format!("{:x}", reader.hasher.finalize()) == source.sha256,
        Refusal("sha256-mismatch")
    );
    host.seal_new_image(image)?;
    ensure!(
        image.dataset_guid.is_some() && image.snapshot_guid.is_some(),
        "image seal did not record its GUIDs"
    );
    host.verify_image(image)?;
    image.phase = ImagePhase::Sealed;
    store.write_image(image)
}

/// Destroys an unsealed import's dataset, then removes its journal.
fn discard_import<B: Backend>(store: &Store, host: &mut B, image: &Image) -> Result<()> {
    if let Some(recorded) = store.read_image(&image.origin)? {
        ensure!(
            recorded.import.is_some() && recorded.phase == ImagePhase::Publishing,
            "the import journal is already sealed; inspect it before retrying"
        );
    }
    host.destroy_new_image(image)?;
    store.remove_unsealed_import(&image.origin)
}

pub fn image_status<B: Backend>(
    store: &Store,
    host: &mut B,
    origin: &Origin,
) -> Result<ImageReport> {
    let _lock = store.lock()?;
    let image = store
        .read_image(origin)?
        .ok_or(Refusal("image-not-found"))?;
    if image.phase == ImagePhase::Sealed {
        host.verify_image(&image)?;
    }
    Ok(image.report())
}

/// Administrative: stop new clones of a sealed generation. Refused while any
/// disk journal (in any phase) or ZFS clone hold still descends from it.
pub fn image_retire<B: Backend>(
    store: &Store,
    host: &mut B,
    origin: &Origin,
) -> Result<ImageReport> {
    let _lock = store.lock()?;
    let mut image = store
        .read_image(origin)?
        .ok_or(Refusal("image-not-found"))?;
    match image.phase {
        ImagePhase::Retired => return Ok(image.report()),
        ImagePhase::Publishing => bail!(Refusal("image-not-published")),
        ImagePhase::Sealed => {}
    }
    ensure!(
        !store
            .records()?
            .iter()
            .any(|r| r.spec.origin.as_ref() == Some(origin)),
        Refusal("image-in-use")
    );
    host.retire_image(&image)?;
    image.phase = ImagePhase::Retired;
    store.write_image(&image)?;
    Ok(image.report())
}

/// Wrapper commands exposed through the storage daemon's fixed argv.
pub const COMMANDS: &[&str] = &[
    "vm-disk-create",
    "vm-disk-clone",
    "vm-disk-expand",
    "vm-disk-export",
    "vm-disk-fence",
    "vm-disk-status",
    "vm-disk-retire",
    "vm-image-publish",
    "vm-image-status",
    "vm-image-retire",
];

/// Administrator-only commands, deliberately absent from [`COMMANDS`] and
/// [`dispatch`]: the storage daemon cannot reach them.
pub const ADMIN_COMMANDS: &[&str] = &["vm-image-import"];

/// Run one administrator-only command with the wrapper's exact-argv rules.
pub fn admin_dispatch<B: Backend>(
    args: &[String],
    store: &Store,
    host: &mut B,
) -> Result<serde_json::Value> {
    let (command, rest) = args.split_first().ok_or(Refusal("invalid-arguments"))?;
    ensure!(command == "vm-image-import", Refusal("invalid-arguments"));
    let v = flags(
        rest,
        &["--image", "--image-generation", "--source", "--sha256"],
    )?;
    let origin = Origin::new(v[0].into(), number(v[1])?)?;
    let report = import(store, host, origin, Path::new(v[2]), v[3])?;
    Ok(serde_json::json!({"ok": true, "image": report}))
}

/// Exact flag set per command: no repeats, extras or credential-bearing flags.
fn flags<'a>(args: &'a [String], expected: &[&str]) -> Result<Vec<&'a str>> {
    ensure!(
        args.len() == expected.len() * 2,
        Refusal("invalid-arguments")
    );
    let mut values = Vec::new();
    for (pair, name) in args.chunks_exact(2).zip(expected) {
        ensure!(pair[0] == *name, Refusal("invalid-arguments"));
        values.push(pair[1].as_str());
    }
    Ok(values)
}

fn number<T: std::str::FromStr>(value: &str) -> Result<T> {
    ensure!(
        !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()),
        Refusal("invalid-arguments")
    );
    value
        .parse()
        .map_err(|_| Refusal("invalid-arguments").into())
}

/// Exactly 32 lowercase hex bytes, optionally newline-terminated.
fn read_password(input: &mut dyn std::io::Read) -> Result<String> {
    let mut bytes = Vec::new();
    std::io::Read::take(input, 65).read_to_end(&mut bytes)?;
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    let password = String::from_utf8(bytes).map_err(|_| Refusal("invalid-credential"))?;
    ensure!(valid_password(&password), Refusal("invalid-credential"));
    Ok(password)
}

/// Run one wrapper command. Only `vm-disk-export` reads `input`.
pub fn dispatch<B: Backend>(
    args: &[String],
    input: &mut dyn std::io::Read,
    store: &Store,
    host: &mut B,
) -> Result<serde_json::Value> {
    let (command, rest) = args.split_first().ok_or(Refusal("invalid-arguments"))?;
    let disk = |report: Report| serde_json::json!({"ok": true, "disk": report});
    let image = |report: ImageReport| serde_json::json!({"ok": true, "image": report});
    Ok(match command.as_str() {
        "vm-disk-create" => {
            let v = flags(rest, &["--vm-uid", "--disk", "--capacity-bytes"])?;
            disk(create(store, host, v[0], v[1], number(v[2])?)?)
        }
        "vm-disk-clone" => {
            let v = flags(
                rest,
                &[
                    "--vm-uid",
                    "--disk",
                    "--image",
                    "--image-generation",
                    "--capacity-bytes",
                ],
            )?;
            let origin = Origin::new(v[2].into(), number(v[3])?)?;
            disk(clone(store, host, v[0], v[1], origin, number(v[4])?)?)
        }
        "vm-disk-expand" => {
            let v = flags(rest, &["--vm-uid", "--disk", "--capacity-bytes"])?;
            disk(expand(store, host, v[0], v[1], number(v[2])?)?)
        }
        "vm-disk-export" => {
            let v = flags(rest, &["--vm-uid", "--disk", "--generation"])?;
            let generation = number(v[2])?;
            let password = read_password(input)?;
            disk(export(store, host, v[0], v[1], generation, &password)?)
        }
        "vm-disk-fence" | "vm-disk-status" | "vm-disk-retire" => {
            let v = flags(rest, &["--vm-uid", "--disk"])?;
            disk(match command.as_str() {
                "vm-disk-fence" => fence(store, host, v[0], v[1])?,
                "vm-disk-status" => status(store, host, v[0], v[1])?,
                _ => retire(store, host, v[0], v[1])?,
            })
        }
        "vm-image-publish" => {
            let v = flags(
                rest,
                &[
                    "--image",
                    "--image-generation",
                    "--source-vm-uid",
                    "--source-disk",
                ],
            )?;
            let origin = Origin::new(v[0].into(), number(v[1])?)?;
            image(publish(store, host, origin, v[2], v[3])?)
        }
        "vm-image-status" | "vm-image-retire" => {
            let v = flags(rest, &["--image", "--image-generation"])?;
            let origin = Origin::new(v[0].into(), number(v[1])?)?;
            image(if command == "vm-image-status" {
                image_status(store, host, &origin)?
            } else {
                image_retire(store, host, &origin)?
            })
        }
        _ => bail!(Refusal("invalid-arguments")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{fixture, Fake, UID};
    use crate::{refusal, restore, MIN_VOLUME};

    fn argv(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).to_owned()).collect()
    }

    #[test]
    fn wrapper_argv_is_exact_and_the_credential_comes_only_from_stdin() {
        let (_dir, store, _) = fixture();
        let mut host = Fake::default();
        let mut empty: &[u8] = b"";
        let create = argv(&[
            "vm-disk-create",
            "--vm-uid",
            UID,
            "--disk",
            "root",
            "--capacity-bytes",
            "67108864",
        ]);
        let out = dispatch(&create, &mut empty, &store, &mut host).unwrap();
        assert_eq!(out["disk"]["phase"], "provisioned");
        for bad in [
            argv(&[
                "vm-disk-create",
                "--disk",
                "root",
                "--vm-uid",
                UID,
                "--capacity-bytes",
                "67108864",
            ]),
            argv(&["vm-disk-create", "--vm-uid", UID, "--disk", "root"]),
            argv(&[
                "vm-disk-create",
                "--vm-uid",
                UID,
                "--disk",
                "root",
                "--capacity-bytes",
                "+67108864",
            ]),
            argv(&[
                "vm-disk-export",
                "--vm-uid",
                UID,
                "--disk",
                "root",
                "--generation",
                "1",
                "--password",
                PW1,
            ]),
            argv(&["vm-disk-shell", "--vm-uid", UID]),
        ] {
            let mut input: &[u8] = b"";
            assert_eq!(
                code(dispatch(&bad, &mut input, &store, &mut host)),
                "invalid-arguments"
            );
        }
        let export = argv(&[
            "vm-disk-export",
            "--vm-uid",
            UID,
            "--disk",
            "root",
            "--generation",
            "1",
        ]);
        let mut short: &[u8] = b"abc\n";
        assert_eq!(
            code(dispatch(&export, &mut short, &store, &mut host)),
            "invalid-credential"
        );
        let mut long: &[u8] = b"111111111111111111111111111111111\n";
        assert_eq!(
            code(dispatch(&export, &mut long, &store, &mut host)),
            "invalid-credential"
        );
        let mut secret = format!("{PW1}\n").into_bytes();
        let out = dispatch(&export, &mut secret.as_slice(), &store, &mut host).unwrap();
        assert_eq!(out["disk"]["phase"], "active");
        assert!(!out.to_string().contains(PW1));
        secret.clear();
        let status = argv(&["vm-disk-status", "--vm-uid", UID, "--disk", "root"]);
        let out = dispatch(&status, &mut empty, &store, &mut host).unwrap();
        assert!(!out.to_string().contains(PW1));
        assert_eq!(
            out["disk"]["target_iqn"],
            format!("iqn.2026-11.com.yapplesauce:qrab.disk.{UID}.root")
        );
    }

    const OTHER: &str = "0b5c1d0e-1111-4222-8333-944455556666";
    const PW1: &str = "11111111111111111111111111111111";
    const PW2: &str = "22222222222222222222222222222222";
    const MIB: u64 = 1024 * 1024;

    fn code(result: Result<impl Sized>) -> &'static str {
        match result {
            Ok(_) => "ok",
            Err(error) => refusal(&error).unwrap_or("other"),
        }
    }

    #[test]
    fn create_is_idempotent_and_never_exports_without_a_credential() {
        let (_dir, store, _) = fixture();
        let mut host = Fake::default();
        let report = create(&store, &mut host, UID, "root", 64 * MIB).unwrap();
        assert_eq!(report.phase, Some(Phase::Provisioned));
        assert_eq!(report.generation, 0);
        assert!(host.exports.is_empty());
        create(&store, &mut host, UID, "root", 64 * MIB).unwrap();
        assert_eq!(
            code(create(&store, &mut host, UID, "root", 128 * MIB)),
            "disk-conflict"
        );
        let record = store.read_disk(UID, "root").unwrap().unwrap();
        assert!(record.chap_password.is_empty());
        assert_eq!(record.schema, 2);
    }

    #[test]
    fn foreign_datasets_and_preexisting_targets_are_refused_before_any_journal() {
        let (_dir, store, _) = fixture();
        let mut host = Fake {
            foreign: true,
            ..Default::default()
        };
        assert!(create(&store, &mut host, UID, "root", 64 * MIB).is_err());
        assert!(store.read_disk(UID, "root").unwrap().is_none());
        assert!(host.volumes.is_empty());
    }

    #[test]
    fn invalid_identifiers_are_rejected_before_locking_or_host_calls() {
        let (_dir, store, _) = fixture();
        let mut host = Fake::default();
        assert_eq!(
            code(create(&store, &mut host, "../x", "root", 64 * MIB)),
            "invalid-vm-uid"
        );
        assert_eq!(
            code(create(&store, &mut host, UID, "Root", 64 * MIB)),
            "invalid-disk-id"
        );
        assert_eq!(
            code(create(&store, &mut host, UID, "root", 64 * MIB + 1)),
            "invalid-capacity"
        );
        assert_eq!(host.pool_calls, 0);
    }

    #[test]
    fn aggregate_logical_budget_bounds_blank_disks_and_clones() {
        let (_dir, store, _) = fixture();
        let mut host = Fake::default();
        create(&store, &mut host, UID, "big", 7 * 1024 * MIB).unwrap();
        assert_eq!(
            code(create(&store, &mut host, UID, "more", 2 * 1024 * MIB)),
            "budget-exhausted"
        );
        assert!(store.read_disk(UID, "more").unwrap().is_none());
        assert_eq!(
            code(expand(&store, &mut host, UID, "big", 9 * 1024 * MIB)),
            "budget-exhausted"
        );
    }

    #[test]
    fn export_rotation_requires_fence_next_generation_and_a_new_secret() {
        let (_dir, store, _) = fixture();
        let mut host = Fake::default();
        create(&store, &mut host, UID, "root", 64 * MIB).unwrap();
        assert_eq!(
            code(export(&store, &mut host, UID, "root", 2, PW1)),
            "generation-mismatch"
        );
        assert_eq!(
            code(export(&store, &mut host, UID, "root", 1, "short")),
            "invalid-credential"
        );
        let report = export(&store, &mut host, UID, "root", 1, PW1).unwrap();
        assert_eq!(report.phase, Some(Phase::Active));
        let key = format!("{UID}.root");
        assert_eq!(host.exports.get(&key).map(String::as_str), Some(PW1));
        // Idempotent with the same credential; any change requires a fence.
        export(&store, &mut host, UID, "root", 1, PW1).unwrap();
        assert_eq!(
            code(export(&store, &mut host, UID, "root", 2, PW2)),
            "fence-required"
        );
        fence(&store, &mut host, UID, "root").unwrap();
        assert!(!host.exports.contains_key(&key));
        assert_eq!(
            code(export(&store, &mut host, UID, "root", 2, PW1)),
            "credential-reused"
        );
        assert_eq!(
            code(export(&store, &mut host, UID, "root", 3, PW2)),
            "generation-mismatch"
        );
        let report = export(&store, &mut host, UID, "root", 2, PW2).unwrap();
        assert_eq!(report.generation, 2);
        assert_eq!(host.exports.get(&key).map(String::as_str), Some(PW2));
    }

    #[test]
    fn interrupted_export_resumes_only_with_the_persisted_credential() {
        let (_dir, store, _) = fixture();
        let mut host = Fake {
            fail_fence: false,
            ..Default::default()
        };
        create(&store, &mut host, UID, "root", 64 * MIB).unwrap();
        let mut record = store.read_disk(UID, "root").unwrap().unwrap();
        record.generation = 1;
        record.chap_password = PW1.into();
        record.phase = Phase::Preparing;
        store.write(&record).unwrap();
        assert_eq!(
            code(export(&store, &mut host, UID, "root", 1, PW2)),
            "generation-mismatch"
        );
        export(&store, &mut host, UID, "root", 1, PW1).unwrap();
    }

    #[test]
    fn failed_fence_stays_fencing_and_blocks_export_and_retire() {
        let (_dir, store, _) = fixture();
        let mut host = Fake::default();
        create(&store, &mut host, UID, "root", 64 * MIB).unwrap();
        export(&store, &mut host, UID, "root", 1, PW1).unwrap();
        host.fail_fence = true;
        assert!(fence(&store, &mut host, UID, "root").is_err());
        assert_eq!(
            store.read_disk(UID, "root").unwrap().unwrap().phase,
            Phase::Fencing
        );
        assert_eq!(
            code(export(&store, &mut host, UID, "root", 2, PW2)),
            "fence-incomplete"
        );
        assert_eq!(
            code(retire(&store, &mut host, UID, "root")),
            "fence-required"
        );
        host.fail_fence = false;
        fence(&store, &mut host, UID, "root").unwrap();
    }

    #[test]
    fn expand_grows_only_and_keeps_the_export() {
        let (_dir, store, _) = fixture();
        let mut host = Fake::default();
        create(&store, &mut host, UID, "root", 64 * MIB).unwrap();
        export(&store, &mut host, UID, "root", 1, PW1).unwrap();
        assert_eq!(
            code(expand(&store, &mut host, UID, "root", 32 * MIB)),
            "shrink-refused"
        );
        let report = expand(&store, &mut host, UID, "root", 96 * MIB).unwrap();
        assert_eq!(report.capacity_bytes, 96 * MIB);
        assert_eq!(report.phase, Some(Phase::Active));
        expand(&store, &mut host, UID, "root", 96 * MIB).unwrap();
        assert_eq!(host.calls.iter().filter(|c| **c == "expand").count(), 1);
        assert_eq!(
            code(expand(&store, &mut host, UID, "absent", 96 * MIB)),
            "disk-not-found"
        );
    }

    #[test]
    fn publish_requires_a_fenced_source_and_seals_an_immutable_generation() {
        let (_dir, store, _) = fixture();
        let mut host = Fake::default();
        create(&store, &mut host, UID, "golden", 64 * MIB).unwrap();
        let origin = Origin::new("debian".into(), 1).unwrap();
        assert_eq!(
            code(publish(&store, &mut host, origin.clone(), UID, "golden")),
            "source-not-fenced"
        );
        export(&store, &mut host, UID, "golden", 1, PW1).unwrap();
        assert_eq!(
            code(publish(&store, &mut host, origin.clone(), UID, "golden")),
            "source-not-fenced"
        );
        fence(&store, &mut host, UID, "golden").unwrap();
        let image = publish(&store, &mut host, origin.clone(), UID, "golden").unwrap();
        assert_eq!(image.phase, ImagePhase::Sealed);
        assert_eq!(image.snapshot, "tank/qrab-iscsi/image-debian-g1@sealed");
        publish(&store, &mut host, origin.clone(), UID, "golden").unwrap();
        create(&store, &mut host, UID, "other", 64 * MIB).unwrap();
        export(&store, &mut host, UID, "other", 1, PW1).unwrap();
        fence(&store, &mut host, UID, "other").unwrap();
        assert_eq!(
            code(publish(&store, &mut host, origin.clone(), UID, "other")),
            "image-generation-exists"
        );
        assert_eq!(host.calls.iter().filter(|c| **c == "publish").count(), 1);
        image_status(&store, &mut host, &origin).unwrap();
    }

    #[test]
    fn clone_holds_and_records_the_origin_and_detects_replacement() {
        let (_dir, store, _) = fixture();
        let mut host = Fake::default();
        let origin = Origin::new("debian".into(), 1).unwrap();
        assert_eq!(
            code(clone(
                &store,
                &mut host,
                OTHER,
                "root",
                origin.clone(),
                64 * MIB
            )),
            "image-not-published"
        );
        create(&store, &mut host, UID, "golden", 64 * MIB).unwrap();
        export(&store, &mut host, UID, "golden", 1, PW1).unwrap();
        fence(&store, &mut host, UID, "golden").unwrap();
        publish(&store, &mut host, origin.clone(), UID, "golden").unwrap();
        assert_eq!(
            code(clone(
                &store,
                &mut host,
                OTHER,
                "root",
                origin.clone(),
                32 * MIB
            )),
            "clone-smaller-than-image"
        );
        let report = clone(&store, &mut host, OTHER, "root", origin.clone(), 128 * MIB).unwrap();
        assert_eq!(report.origin.as_ref(), Some(&origin));
        let record = store.read_disk(OTHER, "root").unwrap().unwrap();
        assert_eq!(record.origin_guid.as_deref(), Some("78"));
        assert!(host.holds[&origin.snapshot()].contains(&format!("qrab-clone-{OTHER}.root")));
        // Same disk with another origin, or a blank create, is a conflict.
        assert_eq!(
            code(create(&store, &mut host, OTHER, "root", 128 * MIB)),
            "disk-conflict"
        );
        // Origin identity is checked on every read, not just at clone time.
        host.replaced_origin = true;
        assert_eq!(
            code(status(&store, &mut host, OTHER, "root")),
            "clone-origin-mismatch"
        );
        assert_eq!(
            code(export(&store, &mut host, OTHER, "root", 1, PW1)),
            "clone-origin-mismatch"
        );
    }

    #[test]
    fn retire_requires_fence_keeps_data_and_refuses_reexport() {
        let (_dir, store, _) = fixture();
        let mut host = Fake::default();
        create(&store, &mut host, UID, "root", 64 * MIB).unwrap();
        export(&store, &mut host, UID, "root", 1, PW1).unwrap();
        assert_eq!(
            code(retire(&store, &mut host, UID, "root")),
            "fence-required"
        );
        fence(&store, &mut host, UID, "root").unwrap();
        let report = retire(&store, &mut host, UID, "root").unwrap();
        assert_eq!(report.phase, Some(Phase::Retired));
        assert!(host
            .volumes
            .contains_key(&format!("tank/qrab-iscsi/disk-{UID}-root")));
        assert_eq!(
            code(export(&store, &mut host, UID, "root", 2, PW2)),
            "disk-retired"
        );
        retire(&store, &mut host, UID, "root").unwrap();
    }

    #[test]
    fn status_reports_never_contain_credentials() {
        let (_dir, store, _) = fixture();
        let mut host = Fake::default();
        assert_eq!(
            code(status(&store, &mut host, UID, "root")),
            "disk-not-found"
        );
        create(&store, &mut host, UID, "root", MIN_VOLUME).unwrap();
        export(&store, &mut host, UID, "root", 1, PW1).unwrap();
        let public =
            serde_json::to_string(&status(&store, &mut host, UID, "root").unwrap()).unwrap();
        assert!(!public.contains(PW1) && !public.contains("password"));
        assert!(public.contains("\"disk\":\"root\""));
    }

    fn published(store: &Store, host: &mut Fake) -> Origin {
        let origin = Origin::new("debian".into(), 1).unwrap();
        create(store, host, UID, "golden", 64 * MIB).unwrap();
        export(store, host, UID, "golden", 1, PW1).unwrap();
        fence(store, host, UID, "golden").unwrap();
        publish(store, host, origin.clone(), UID, "golden").unwrap();
        origin
    }

    #[test]
    fn image_retire_is_refused_while_any_clone_descends_from_it() {
        let (_dir, store, _) = fixture();
        let mut host = Fake::default();
        let origin = published(&store, &mut host);
        clone(&store, &mut host, OTHER, "root", origin.clone(), 64 * MIB).unwrap();
        assert_eq!(
            code(image_retire(&store, &mut host, &origin)),
            "image-in-use"
        );
        // A retired clone still depends on the snapshot.
        fence(&store, &mut host, OTHER, "root").unwrap();
        retire(&store, &mut host, OTHER, "root").unwrap();
        assert_eq!(
            code(image_retire(&store, &mut host, &origin)),
            "image-in-use"
        );
        assert!(host.retired_images.is_empty());
        // A ZFS hold without a journal (e.g. a foreign clone) also blocks it.
        let (_dir, store, _) = fixture();
        let mut host = Fake::default();
        let origin = published(&store, &mut host);
        host.holds
            .get_mut(&origin.snapshot())
            .unwrap()
            .push("qrab-clone-elsewhere".into());
        assert_eq!(
            code(image_retire(&store, &mut host, &origin)),
            "image-in-use"
        );
    }

    #[test]
    fn retired_images_accept_no_new_clones_or_republication() {
        let (_dir, store, _) = fixture();
        let mut host = Fake::default();
        let origin = published(&store, &mut host);
        let report = image_retire(&store, &mut host, &origin).unwrap();
        assert_eq!(report.phase, ImagePhase::Retired);
        assert_eq!(host.retired_images, vec![origin.key()]);
        image_retire(&store, &mut host, &origin).unwrap();
        assert_eq!(host.retired_images.len(), 1);
        assert_eq!(
            code(clone(
                &store,
                &mut host,
                OTHER,
                "root",
                origin.clone(),
                64 * MIB
            )),
            "image-retired"
        );
        assert_eq!(
            code(publish(&store, &mut host, origin.clone(), UID, "golden")),
            "image-retired"
        );
        assert_eq!(
            code(image_retire(
                &store,
                &mut host,
                &Origin::new("absent".into(), 1).unwrap()
            )),
            "image-not-found"
        );
        let mut empty: &[u8] = b"";
        let argv: Vec<String> = [
            "vm-image-retire",
            "--image",
            "debian",
            "--image-generation",
            "1",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let out = dispatch(&argv, &mut empty, &store, &mut host).unwrap();
        assert_eq!(out["image"]["phase"], "retired");
    }

    #[test]
    fn budget_follows_the_verified_parent_quota() {
        let (_dir, store, _) = fixture();
        let mut host = Fake {
            quota: Some(32 << 30),
            ..Default::default()
        };
        create(&store, &mut host, UID, "big", 20 << 30).unwrap();
        create(&store, &mut host, UID, "more", 10 << 30).unwrap();
        assert_eq!(
            code(create(&store, &mut host, UID, "over", 4 << 30)),
            "budget-exhausted"
        );
        host.quota = Some(5 << 40);
        assert!(create(&store, &mut host, UID, "over", 1 << 30).is_err());
        assert!(store.read_disk(UID, "over").unwrap().is_none());
    }

    /// A deterministic raw file and its lowercase SHA-256.
    fn raw(dir: &Path, name: &str, len: usize) -> (std::path::PathBuf, String, Vec<u8>) {
        use sha2::Digest as _;
        let data: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
        let path = dir.join(name);
        std::fs::write(&path, &data).unwrap();
        let sha = format!("{:x}", Sha256::digest(&data));
        (path, sha, data)
    }

    fn cirros(generation: u32) -> Origin {
        Origin::new("cirros".into(), generation).unwrap()
    }

    fn nothing_left(store: &Store, host: &Fake, origin: &Origin) {
        assert!(store.read_image(origin).unwrap().is_none());
        assert!(!host.volumes.contains_key(&origin.dataset()));
        assert!(!host.images.contains_key(&origin.key()));
        assert!(!host.written.contains_key(&origin.key()));
        assert!(host
            .holds
            .get(&origin.snapshot())
            .is_none_or(|h| h.is_empty()));
    }

    #[test]
    fn import_seals_a_generation_that_clones_like_a_published_one() {
        let (dir, store, _) = fixture();
        let mut host = Fake::default();
        let origin = cirros(1);
        let (path, sha, data) = raw(dir.path(), "cirros.raw", 17 * 1024 * 1024 + 512);
        let report = import(&store, &mut host, origin.clone(), &path, &sha).unwrap();
        assert_eq!(report.phase, ImagePhase::Sealed);
        assert_eq!(report.capacity_bytes, 18 * MIB);
        assert_eq!(report.snapshot, "tank/qrab-iscsi/image-cirros-g1@sealed");
        assert_eq!(host.written[&origin.key()], data);
        assert_eq!(host.holds[&origin.snapshot()], vec!["qrab-image"]);
        let public = serde_json::to_value(&report).unwrap();
        assert_eq!(public["import"]["sha256"], sha.as_str());
        assert_eq!(public["import"]["size_bytes"], data.len());
        assert!(public.get("source_vm_uid").is_none());

        // The journal has the published shape plus the import origin.
        let record = store.read_image(&origin).unwrap().unwrap();
        assert_eq!(record.phase, ImagePhase::Sealed);
        assert_eq!(record.capacity_bytes, 18 * MIB);
        assert!(record.source_vm_uid.is_empty() && record.source_guid.is_empty());
        assert_eq!(
            record.import,
            Some(ImportSource {
                sha256: sha.clone(),
                size_bytes: data.len() as u64
            })
        );
        assert_eq!(record.dataset_guid.as_deref(), Some("86"));
        assert_eq!(record.snapshot_guid.as_deref(), Some("88"));
        assert_eq!(
            image_status(&store, &mut host, &origin).unwrap().phase,
            ImagePhase::Sealed
        );

        // Clone, publish and retire treat it like a published generation.
        assert_eq!(
            code(clone(
                &store,
                &mut host,
                OTHER,
                "root",
                origin.clone(),
                16 * MIB
            )),
            "clone-smaller-than-image"
        );
        let disk = clone(&store, &mut host, OTHER, "root", origin.clone(), 64 * MIB).unwrap();
        assert_eq!(disk.origin.as_ref(), Some(&origin));
        let clone_record = store.read_disk(OTHER, "root").unwrap().unwrap();
        assert_eq!(clone_record.origin_guid.as_deref(), Some("88"));
        status(&store, &mut host, OTHER, "root").unwrap();
        export(&store, &mut host, OTHER, "root", 1, PW1).unwrap();
        assert_eq!(
            code(image_retire(&store, &mut host, &origin)),
            "image-in-use"
        );
        create(&store, &mut host, UID, "golden", 64 * MIB).unwrap();
        assert_eq!(
            code(import(&store, &mut host, origin.clone(), &path, &sha)),
            "image-generation-exists"
        );

        // The administrator argv, with an uppercase digest, seals another one.
        let argv = argv(&[
            "vm-image-import",
            "--image",
            "cirros",
            "--image-generation",
            "2",
            "--source",
            path.to_str().unwrap(),
            "--sha256",
            &sha.to_uppercase(),
        ]);
        let out = admin_dispatch(&argv, &store, &mut host).unwrap();
        assert_eq!(out["image"]["phase"], "sealed");
        assert_eq!(out["image"]["import"]["sha256"], sha.as_str());
        // A retired import accepts no new clones, like a published image.
        image_retire(&store, &mut host, &cirros(2)).unwrap();
        assert_eq!(
            code(clone(&store, &mut host, OTHER, "data", cirros(2), 64 * MIB)),
            "image-retired"
        );
    }

    #[test]
    fn import_is_an_administrator_command_never_a_daemon_wrapper_verb() {
        let (dir, store, _) = fixture();
        let mut host = Fake::default();
        let (path, sha, _) = raw(dir.path(), "cirros.raw", 4096);
        assert!(!COMMANDS.contains(&"vm-image-import"));
        assert!(ADMIN_COMMANDS.iter().all(|c| !COMMANDS.contains(c)));
        let import = argv(&[
            "vm-image-import",
            "--image",
            "cirros",
            "--image-generation",
            "1",
            "--source",
            path.to_str().unwrap(),
            "--sha256",
            &sha,
        ]);
        let mut empty: &[u8] = b"";
        assert_eq!(
            code(dispatch(&import, &mut empty, &store, &mut host)),
            "invalid-arguments"
        );
        let status = argv(&[
            "vm-image-status",
            "--image",
            "cirros",
            "--image-generation",
            "1",
        ]);
        assert_eq!(
            code(admin_dispatch(&status, &store, &mut host)),
            "invalid-arguments"
        );
        let mut reordered = import.clone();
        reordered.swap(5, 7);
        reordered.swap(6, 8);
        assert_eq!(
            code(admin_dispatch(&reordered, &store, &mut host)),
            "invalid-arguments"
        );
        assert_eq!(
            code(admin_dispatch(&import[..7], &store, &mut host)),
            "invalid-arguments"
        );
        assert!(host.calls.is_empty());
        nothing_left(&store, &host, &cirros(1));
    }

    #[test]
    fn refused_imports_leave_nothing_behind() {
        let (dir, store, _) = fixture();
        let mut host = Fake::default();
        let origin = cirros(1);
        let (path, sha, _) = raw(dir.path(), "cirros.raw", 1024 * 1024);

        // Refusals before any dataset exists.
        let symlink = dir.path().join("link.raw");
        std::os::unix::fs::symlink(&path, &symlink).unwrap();
        let fifo = dir.path().join("fifo.raw");
        nix::unistd::mkfifo(&fifo, nix::sys::stat::Mode::S_IRWXU).unwrap();
        let (empty, _, _) = raw(dir.path(), "empty.raw", 0);
        let (misaligned, _, _) = raw(dir.path(), "odd.raw", 513);
        let oversized = dir.path().join("huge.raw");
        File::create(&oversized)
            .unwrap()
            .set_len(MAX_IMPORT + 512)
            .unwrap();
        for (source, expected) in [
            (symlink.as_path(), "source-not-regular"),
            (fifo.as_path(), "source-not-regular"),
            (dir.path(), "source-not-regular"),
            (Path::new("/dev/null"), "source-not-regular"),
            (empty.as_path(), "source-empty"),
            (misaligned.as_path(), "source-misaligned"),
            (oversized.as_path(), "source-too-large"),
            (&dir.path().join("absent.raw"), "source-not-found"),
            (Path::new("cirros.raw"), "invalid-source"),
        ] {
            assert_eq!(
                code(import(&store, &mut host, origin.clone(), source, &sha)),
                expected,
                "{}",
                source.display()
            );
        }
        for bad in ["", "../x", "CirrOS", "-cirros", "cirros.raw"] {
            assert_eq!(code(Origin::new(bad.into(), 1)), "invalid-image", "{bad}");
        }
        for generation in ["0", "1000000", "-1", "+1", "x"] {
            let argv = argv(&[
                "vm-image-import",
                "--image",
                "cirros",
                "--image-generation",
                generation,
                "--source",
                path.to_str().unwrap(),
                "--sha256",
                &sha,
            ]);
            assert_ne!(code(admin_dispatch(&argv, &store, &mut host)), "ok");
        }
        let unvalidated = Origin {
            image: "../x".into(),
            generation: 1,
        };
        assert_eq!(
            code(import(&store, &mut host, unvalidated, &path, &sha)),
            "invalid-image"
        );
        for digest in ["", "abc", &"g".repeat(64), &format!("{sha}0")] {
            assert_eq!(
                code(import(&store, &mut host, origin.clone(), &path, digest)),
                "invalid-sha256"
            );
        }
        assert!(host.calls.is_empty());
        nothing_left(&store, &host, &origin);

        // A digest mismatch is found while copying; the new dataset goes.
        let (_, other, _) = raw(dir.path(), "other.raw", 512);
        assert_eq!(
            code(import(&store, &mut host, origin.clone(), &path, &other)),
            "sha256-mismatch"
        );
        assert_eq!(host.destroyed, vec![origin.key()]);
        nothing_left(&store, &host, &origin);

        // A dataset without a journal is never adopted or destroyed.
        host.volumes.insert(origin.dataset(), 16 * MIB);
        assert_eq!(
            code(import(&store, &mut host, origin.clone(), &path, &sha)),
            "image-generation-exists"
        );
        assert!(host.volumes.contains_key(&origin.dataset()));
        assert!(store.read_image(&origin).unwrap().is_none());
        host.volumes.remove(&origin.dataset());

        // An existing sealed generation is left exactly as it was.
        let published = published(&store, &mut host);
        let before = std::fs::read(store.manifest_path(&published.key())).unwrap();
        assert_eq!(
            code(import(&store, &mut host, published.clone(), &path, &sha)),
            "image-generation-exists"
        );
        assert_eq!(
            std::fs::read(store.manifest_path(&published.key())).unwrap(),
            before
        );
        image_status(&store, &mut host, &published).unwrap();
        assert_eq!(host.destroyed.len(), 1);

        // So is an interrupted publication, which only publish may resume.
        let publishing = Image {
            schema: 2,
            owner: OWNER.into(),
            origin: cirros(3),
            capacity_bytes: 64 * MIB,
            source_vm_uid: UID.into(),
            source_disk: "golden".into(),
            source_guid: "34".into(),
            import: None,
            phase: ImagePhase::Publishing,
            dataset_guid: None,
            snapshot_guid: None,
        };
        store.write_image(&publishing).unwrap();
        assert_eq!(
            code(import(&store, &mut host, cirros(3), &path, &sha)),
            "image-generation-exists"
        );
        assert!(store.read_image(&cirros(3)).unwrap().is_some());

        // The parent quota bounds an import like any other zvol.
        let (_dir, store, _) = fixture();
        let mut host = Fake::default();
        create(&store, &mut host, UID, "big", 8 * 1024 * MIB - 16 * MIB).unwrap();
        let (big, big_sha, _) = raw(dir.path(), "big.raw", 16 * 1024 * 1024 + 512);
        assert_eq!(
            code(import(&store, &mut host, origin.clone(), &big, &big_sha)),
            "budget-exhausted"
        );
        assert!(!host.calls.contains(&"import-create"));
        nothing_left(&store, &host, &origin);
    }

    #[test]
    fn failed_write_or_seal_destroys_the_new_dataset() {
        let (dir, store, _) = fixture();
        let origin = cirros(1);
        let (path, sha, _) = raw(dir.path(), "cirros.raw", 2 * 1024 * 1024);
        for (fail_write, fail_seal) in [(true, false), (false, true)] {
            let mut host = Fake {
                fail_write,
                fail_seal,
                ..Default::default()
            };
            assert!(import(&store, &mut host, origin.clone(), &path, &sha).is_err());
            assert_eq!(host.destroyed, vec![origin.key()]);
            nothing_left(&store, &host, &origin);
        }

        // A failed cleanup keeps the unsealed journal (with the new GUID) so
        // that rerunning the import finishes the cleanup and starts over.
        let mut host = Fake {
            fail_seal: true,
            fail_destroy: true,
            ..Default::default()
        };
        let Err(error) = import(&store, &mut host, origin.clone(), &path, &sha) else {
            panic!("a failed seal must not succeed");
        };
        assert!(format!("{error:#}").contains("rerun the same import"));
        let stale = store.read_image(&origin).unwrap().unwrap();
        assert_eq!(stale.phase, ImagePhase::Publishing);
        assert_eq!(stale.dataset_guid.as_deref(), Some("86"));
        assert_eq!(
            code(clone(
                &store,
                &mut host,
                OTHER,
                "root",
                origin.clone(),
                64 * MIB
            )),
            "image-not-published"
        );
        assert_eq!(
            code(image_retire(&store, &mut host, &origin)),
            "image-not-published"
        );
        host.fail_seal = false;
        host.fail_destroy = false;
        let report = import(&store, &mut host, origin.clone(), &path, &sha).unwrap();
        assert_eq!(report.phase, ImagePhase::Sealed);
        assert_eq!(host.destroyed, vec![origin.key()]);
        clone(&store, &mut host, OTHER, "root", origin, 64 * MIB).unwrap();
    }

    #[test]
    fn published_journals_keep_their_existing_format() {
        let (_dir, store, _) = fixture();
        let mut host = Fake::default();
        let origin = published(&store, &mut host);
        let text = std::fs::read_to_string(store.manifest_path(&origin.key())).unwrap();
        assert!(!text.contains("import"));
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["source_vm_uid"], UID);
        let report =
            serde_json::to_value(image_status(&store, &mut host, &origin).unwrap()).unwrap();
        assert_eq!(report["source_disk"], "golden");
        assert!(report.get("import").is_none());
    }

    #[test]
    fn boot_restore_resumes_only_active_disks() {
        let (_dir, store, _) = fixture();
        let mut host = Fake::default();
        create(&store, &mut host, UID, "fresh", MIN_VOLUME).unwrap();
        create(&store, &mut host, UID, "live", MIN_VOLUME).unwrap();
        export(&store, &mut host, UID, "live", 1, PW1).unwrap();
        create(&store, &mut host, UID, "off", MIN_VOLUME).unwrap();
        export(&store, &mut host, UID, "off", 1, PW1).unwrap();
        fence(&store, &mut host, UID, "off").unwrap();
        host.exports.clear();
        restore(&store, &mut host).unwrap();
        assert_eq!(host.exports.len(), 1);
        assert!(host.exports.contains_key(&format!("{UID}.live")));
        assert_eq!(
            store.read_disk(UID, "fresh").unwrap().unwrap().phase,
            Phase::Provisioned
        );
    }
}

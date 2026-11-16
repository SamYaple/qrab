//! cloud-init NoCloud seed: a small read-only FAT volume labelled `cidata`.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::OpenOptions,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

/// Bound on `meta_data`, `user_data` and `network_config` together.
pub const SEED_INPUT_LIMIT: usize = 64 * 1024;
/// FAT12 image size; holds the input limit with ample directory/FAT room.
pub const SEED_IMAGE_BYTES: u64 = 1 << 20;
/// Written next to the QMP socket in the per-start session directory.
pub const SEED_IMAGE: &str = "seed.img";
/// cloud-init accepts `cidata` or `CIDATA`; CirrOS matches only lowercase.
const LABEL: [u8; 11] = *b"cidata     ";
/// cloud-init expects a `user-data` file even when there is nothing to do.
const EMPTY_USER_DATA: &str = "#cloud-config\n";

/// NoCloud files. `user_data` may hold secrets: it is never shown in a view,
/// and `Debug` prints only its size.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Seed {
    pub meta_data: String,
    #[serde(default)]
    pub user_data: Option<String>,
    #[serde(default)]
    pub network_config: Option<String>,
}

impl std::fmt::Debug for Seed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Seed")
            .field("meta_data", &self.meta_data)
            .field("user_data_bytes", &self.user_data.as_ref().map(String::len))
            .field("network_config", &self.network_config)
            .finish()
    }
}

impl Seed {
    pub fn validate(&self) -> Result<()> {
        let total = self.meta_data.len()
            + self.user_data.as_ref().map_or(0, String::len)
            + self.network_config.as_ref().map_or(0, String::len);
        if self.meta_data.trim().is_empty() || total > SEED_INPUT_LIMIT {
            return Err(Error::Config(format!(
                "seed needs meta_data, and its files may total at most {SEED_INPUT_LIMIT} bytes"
            )));
        }
        Ok(())
    }

    /// The `user-data` file content: verbatim, or an empty `#cloud-config`.
    pub fn user_data(&self) -> &str {
        self.user_data.as_deref().unwrap_or(EMPTY_USER_DATA)
    }

    /// `local-hostname` from JSON meta-data (the operator's form) or a
    /// top-level YAML `local-hostname:` line, when it is a printable scalar.
    pub fn hostname(&self) -> Option<String> {
        let value = match serde_json::from_str::<serde_json::Value>(&self.meta_data) {
            Ok(json) => json.get("local-hostname")?.as_str()?.to_owned(),
            Err(_) => self.meta_data.lines().find_map(|line| {
                let value = line.strip_prefix("local-hostname:")?.trim();
                Some(
                    value
                        .strip_prefix('"')
                        .and_then(|v| v.strip_suffix('"'))
                        .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                        .unwrap_or(value)
                        .to_owned(),
                )
            })?,
        };
        (!value.is_empty() && value.len() <= 253 && value.bytes().all(|b| b.is_ascii_graphic()))
            .then_some(value)
    }

    /// The files in the volume, in write order.
    pub fn files(&self) -> Vec<(&'static str, &str)> {
        let mut files = vec![
            ("meta-data", self.meta_data.as_str()),
            ("user-data", self.user_data()),
        ];
        if let Some(network) = &self.network_config {
            files.push(("network-config", network));
        }
        files
    }

    /// Create a fresh FAT12 `cidata` image at `path` (mode 0600). The path
    /// must not exist: the session directory is new on every start.
    pub fn write(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.set_len(SEED_IMAGE_BYTES)?;
        fatfs::format_volume(
            &mut file,
            fatfs::FormatVolumeOptions::new()
                .fat_type(fatfs::FatType::Fat12)
                .volume_label(LABEL),
        )?;
        let fs =
            fatfs::FileSystem::new(&mut file, fatfs::FsOptions::new().time_provider(&FAT_EPOCH))?;
        {
            let root = fs.root_dir();
            for (name, content) in self.files() {
                root.create_file(name)?.write_all(content.as_bytes())?;
            }
        }
        fs.unmount()?;
        Ok(())
    }
}

/// Files are stamped 1980-01-01 00:00, the FAT epoch: the image is a
/// deterministic function of its input, without a clock dependency.
#[derive(Debug)]
pub(super) struct FatEpoch;
pub(super) static FAT_EPOCH: FatEpoch = FatEpoch;

impl fatfs::TimeProvider for FatEpoch {
    fn get_current_date(&self) -> fatfs::Date {
        fatfs::Date {
            year: 1980,
            month: 1,
            day: 1,
        }
    }

    fn get_current_date_time(&self) -> fatfs::DateTime {
        fatfs::DateTime {
            date: self.get_current_date(),
            time: fatfs::Time {
                hour: 0,
                min: 0,
                sec: 0,
                millis: 0,
            },
        }
    }
}

/// Seed image path for a session whose QMP socket is `socket`.
pub fn image_path(socket: &Path) -> PathBuf {
    socket.with_file_name(SEED_IMAGE)
}

/// Read-only virtio-blk with no boot index; NoCloud finds it by label.
pub(super) fn arguments(socket: &Path, args: &mut Vec<String>) {
    let path = image_path(socket);
    args.extend([
        "-blockdev".into(),
        serde_json::json!({"driver":"file", "filename":path, "node-name":"seed-file", "read-only":true}).to_string(),
        "-blockdev".into(),
        serde_json::json!({"driver":"raw", "file":"seed-file", "node-name":"seed", "read-only":true}).to_string(),
        "-device".into(),
        "virtio-blk-pci,drive=seed,id=seed".into(),
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn seed(user_data: Option<&str>, network: Option<&str>) -> Seed {
        Seed {
            meta_data: "instance-id: \"abc\"\nlocal-hostname: \"seedtest\"\n".into(),
            user_data: user_data.map(str::to_owned),
            network_config: network.map(str::to_owned),
        }
    }

    fn read_back(path: &Path) -> (String, Vec<(String, String)>) {
        let file = std::fs::File::open(path).unwrap();
        let fs = fatfs::FileSystem::new(file, fatfs::FsOptions::new()).unwrap();
        assert_eq!(fs.fat_type(), fatfs::FatType::Fat12);
        let label = fs.read_volume_label_from_root_dir().unwrap().unwrap();
        assert_eq!(fs.volume_label(), label);
        let mut files: Vec<_> = fs
            .root_dir()
            .iter()
            .map(|entry| entry.unwrap())
            .filter(|entry| entry.is_file())
            .map(|entry| {
                let mut content = String::new();
                entry.to_file().read_to_string(&mut content).unwrap();
                (entry.file_name(), content)
            })
            .collect();
        files.sort();
        (label, files)
    }

    #[test]
    fn image_has_cidata_label_and_exact_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SEED_IMAGE);
        let seed = seed(Some("#!/bin/sh\necho hi\n"), Some("version: 2\n"));
        seed.write(&path).unwrap();
        let metadata = std::fs::metadata(&path).unwrap();
        assert_eq!(metadata.len(), SEED_IMAGE_BYTES);
        assert_eq!(
            std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o777,
            0o600
        );
        let (label, files) = read_back(&path);
        assert_eq!(label, "cidata");
        assert_eq!(
            files,
            [
                ("meta-data".into(), seed.meta_data.clone()),
                ("network-config".into(), "version: 2\n".into()),
                ("user-data".into(), "#!/bin/sh\necho hi\n".into()),
            ]
        );
        // A session directory is fresh; an existing image is never reused.
        assert!(seed.write(&path).is_err());
        // No clock input: the same seed yields the same bytes.
        let again = dir.path().join("again.img");
        seed.write(&again).unwrap();
        assert_eq!(
            std::fs::read(&path).unwrap(),
            std::fs::read(&again).unwrap()
        );
    }

    #[test]
    fn missing_user_data_is_an_empty_cloud_config_and_network_is_omitted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SEED_IMAGE);
        seed(None, None).write(&path).unwrap();
        let (_, files) = read_back(&path);
        let mut secret = seed(Some("password: hunter2"), None);
        assert!(!format!("{secret:?}").contains("hunter2"));
        secret.user_data = None;
        assert!(format!("{secret:?}").contains("user_data_bytes: None"));
        let names: Vec<_> = files.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, ["meta-data", "user-data"]);
        assert_eq!(files[1].1, "#cloud-config\n");
    }

    #[test]
    fn input_is_bounded_and_meta_data_required() {
        let at_limit = SEED_INPUT_LIMIT - seed(None, None).meta_data.len();
        let full = seed(Some(&"x".repeat(at_limit)), None);
        full.validate().unwrap();
        let dir = tempfile::tempdir().unwrap();
        full.write(&dir.path().join(SEED_IMAGE)).unwrap();
        assert!(seed(Some(&"x".repeat(at_limit)), Some("y"))
            .validate()
            .is_err());
        let mut empty = seed(None, None);
        empty.meta_data = " \n".into();
        assert!(empty.validate().is_err());
    }

    #[test]
    fn hostname_is_read_from_meta_data() {
        assert_eq!(seed(None, None).hostname().as_deref(), Some("seedtest"));
        let mut plain = seed(None, None);
        plain.meta_data = "local-hostname: web-1\n".into();
        assert_eq!(plain.hostname().as_deref(), Some("web-1"));
        plain.meta_data = "instance-id: x\n".into();
        assert_eq!(plain.hostname(), None);
        plain.meta_data = "{\"instance-id\":\"u\",\"local-hostname\":\"no\"}\n".into();
        assert_eq!(plain.hostname().as_deref(), Some("no"));
        plain.meta_data = "{\"local-hostname\":\"bad name\"}".into();
        assert_eq!(plain.hostname(), None);
    }
}

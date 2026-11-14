//! Opt-in, bounded local read-only virtual CD-ROM media.
//! The media directory must be mounted read-only and managed by the operator;
//! no network retrieval, arbitrary host paths or credential-bearing URLs exist.
use crate::{Error, Result};
use qapi::commands::{
    BlockdevAdd, BlockdevChangeMedium, BlockdevDel, DeviceAdd, Eject, QueryBlock,
};
use qapi::BlockdevChangeReadOnlyMode;
use qapi_protocol::Qmp;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub const MEDIA_DEVICE: &str = "qrab-media-cd";
const MEDIA_NODE: &str = "qrab-media";
const DEFAULT_MAX_BYTES: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct MediaPolicy {
    root: PathBuf,
    max_bytes: u64,
}

impl MediaPolicy {
    /// Explicitly enabled by the manager's environment; never inherit a path
    /// supplied by an HTTP client. A read-only mount prevents rename/symlink
    /// races between canonical validation and QEMU opening the file.
    pub fn from_env() -> Result<Option<Self>> {
        let Some(root) = std::env::var_os("QRAB_MEDIA_ROOT") else {
            return Ok(None);
        };
        let max_bytes = match std::env::var("QRAB_MEDIA_MAX_BYTES") {
            Ok(value) => value
                .parse()
                .map_err(|_| Error::Config("invalid QRAB_MEDIA_MAX_BYTES".into()))?,
            Err(_) => DEFAULT_MAX_BYTES,
        };
        Self::new(PathBuf::from(root), max_bytes).map(Some)
    }

    pub fn new(root: PathBuf, max_bytes: u64) -> Result<Self> {
        if !root.is_absolute() || !(1..=64 * 1024 * 1024 * 1024).contains(&max_bytes) {
            return Err(Error::Config(
                "media requires an absolute directory and a size limit of at most 64 GiB".into(),
            ));
        }
        let root = root
            .canonicalize()
            .map_err(|_| Error::Config("media directory is unavailable".into()))?;
        if !root.is_dir() {
            return Err(Error::Config("media root is not a directory".into()));
        }
        Ok(Self { root, max_bytes })
    }

    fn image_path(&self, name: &str) -> Result<PathBuf> {
        if !valid_image_name(name) {
            return Err(Error::Config(
                "Image must be a local .iso or .img filename".into(),
            ));
        }
        let path = self
            .root
            .join(name)
            .canonicalize()
            .map_err(|_| Error::Config("media image is unavailable".into()))?;
        if !path.starts_with(&self.root) {
            return Err(Error::Config(
                "media image is outside its configured directory".into(),
            ));
        }
        let metadata = path
            .metadata()
            .map_err(|_| Error::Config("media image is unavailable".into()))?;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > self.max_bytes {
            return Err(Error::Config(
                "media image must be a nonempty regular file within the configured size limit"
                    .into(),
            ));
        }
        if path.to_str().is_none() {
            return Err(Error::Config("media image path is not UTF-8".into()));
        }
        Ok(path)
    }

    /// Empty IDE CD-ROM is provisioned at boot on q35. IDE hotplug is not
    /// universally supported; the fallback insertion path returns QMP errors.
    pub fn arguments(&self, args: &mut Vec<String>) {
        args.extend([
            "-device".into(),
            format!("ide-cd,id={MEDIA_DEVICE},bus=ide.0"),
        ]);
    }
}

pub fn valid_image_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        && (name.ends_with(".iso") || name.ends_with(".img"))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum MediaOperation {
    Status,
    Insert { image: String },
    Eject,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaStatus {
    pub enabled: bool,
    pub connected: bool,
    pub inserted: bool,
    pub image_name: Option<String>,
    pub size_bytes: Option<u64>,
}

fn media_block(block: &qapi::BlockInfo) -> bool {
    block
        .qdev
        .as_deref()
        .is_some_and(|id| id == MEDIA_DEVICE || id.ends_with(&format!("/{MEDIA_DEVICE}")))
        || block.device == MEDIA_DEVICE
}

#[allow(deprecated)] // Generated current commands retain deprecated optional device slots.
pub(super) async fn execute(
    qmp: &Qmp,
    policy: Option<&MediaPolicy>,
    operation: MediaOperation,
) -> Result<MediaStatus> {
    let Some(policy) = policy else {
        return if operation == MediaOperation::Status {
            Ok(MediaStatus::default())
        } else {
            Err(Error::Config("virtual media is disabled".into()))
        };
    };
    let current = status(qmp, policy).await?;
    match &operation {
        MediaOperation::Status => return Ok(current),
        MediaOperation::Insert { image } => {
            let path = policy.image_path(image)?;
            if current.connected {
                qmp.execute(&BlockdevChangeMedium {
                    device: None,
                    id: Some(MEDIA_DEVICE.into()),
                    filename: path.to_string_lossy().into_owned(),
                    format: Some("raw".into()),
                    force: Some(false),
                    read_only_mode: Some(BlockdevChangeReadOnlyMode::ReadOnly),
                })
                .await?;
            } else {
                // A QMP-only deployment can attempt to add the device. If its
                // bus cannot hotplug, fail honestly and release the new backend.
                let block: BlockdevAdd = serde_json::from_value(serde_json::json!({
                    "driver":"raw","node-name":MEDIA_NODE,"read-only":true,
                    "file":{"driver":"file","filename":path,"read-only":true}
                }))?;
                qmp.execute(&block).await?;
                let result = qmp
                    .execute(&DeviceAdd {
                        driver: "ide-cd".into(),
                        id: Some(MEDIA_DEVICE.into()),
                        bus: Some("ide.0".into()),
                        arguments: Some(HashMap::from([(
                            "drive".into(),
                            serde_json::json!(MEDIA_NODE),
                        )])),
                    })
                    .await;
                if let Err(error) = result {
                    let _ = qmp
                        .execute(&BlockdevDel {
                            node_name: MEDIA_NODE.into(),
                        })
                        .await;
                    return Err(error.into());
                }
            }
        }
        MediaOperation::Eject => {
            if !current.connected || !current.inserted {
                return Ok(current);
            }
            qmp.execute(&Eject {
                device: None,
                id: Some(MEDIA_DEVICE.into()),
                force: Some(false),
            })
            .await?;
        }
    }
    let observed = status(qmp, policy).await?;
    if matches!(operation, MediaOperation::Eject) && observed.inserted {
        return Err(Error::Spawn(
            "the guest has not released virtual media".into(),
        ));
    }
    if matches!(operation, MediaOperation::Insert { .. }) && !observed.inserted {
        return Err(Error::Spawn(
            "QEMU did not confirm inserted virtual media".into(),
        ));
    }
    Ok(observed)
}

async fn status(qmp: &Qmp, policy: &MediaPolicy) -> Result<MediaStatus> {
    let blocks = qmp.execute(&QueryBlock { flat: Some(true) }).await?;
    let Some(block) = blocks.iter().find(|b| media_block(b)) else {
        return Ok(MediaStatus {
            enabled: true,
            ..Default::default()
        });
    };
    let inserted = block.inserted.as_ref();
    // QEMU owns the authoritative medium state; only return a basename from
    // the allowlisted directory, never arbitrary QMP file strings.
    let image_name = inserted.and_then(|i| {
        let path = Path::new(&i.file);
        path.strip_prefix(&policy.root)
            .ok()?
            .file_name()?
            .to_str()
            .filter(|name| valid_image_name(name))
            .map(str::to_owned)
    });
    Ok(MediaStatus {
        enabled: true,
        connected: true,
        inserted: inserted.is_some() && !block.tray_open.unwrap_or(false),
        image_name,
        size_bytes: inserted
            .and_then(|i| i.image.as_ref())
            .and_then(|i| u64::try_from(i.virtual_size).ok()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_policy_rejects_traversal_urls_symlink_escape_and_size_excess() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("media");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("rescue.iso"), [1, 2, 3]).unwrap();
        std::fs::write(dir.path().join("private.iso"), [1]).unwrap();
        std::os::unix::fs::symlink(dir.path().join("private.iso"), root.join("escape.iso"))
            .unwrap();
        let policy = MediaPolicy::new(root, 3).unwrap();
        assert!(policy.image_path("rescue.iso").is_ok());
        for name in [
            "../private.iso",
            "https://example/rescue.iso",
            "/private.iso",
            "escape.iso",
            ".hidden.iso",
            "no.iso",
        ] {
            assert!(policy.image_path(name).is_err(), "{name}");
        }
        std::fs::write(policy.root.join("large.iso"), [0; 4]).unwrap();
        assert!(policy.image_path("large.iso").is_err());
    }
    #[tokio::test]
    async fn existing_cd_insert_and_eject_are_verified_read_only_nonforce_operations() {
        use qapi_protocol::server::QmpSession;
        use serde_json::json;
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("rescue.iso"), [1, 2, 3]).unwrap();
        let policy = MediaPolicy::new(dir.path().to_owned(), 1024).unwrap();
        let path = dir.path().join("qmp.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let image = dir.path().join("rescue.iso");
        let server = tokio::spawn(async move {
            let mut session = QmpSession::accept(&listener).await.unwrap();
            session.send_greeting(11, 1, 3).await.unwrap();
            session.negotiate().await.unwrap();
            let mut inserted = false;
            for expected in [
                "query-block",
                "blockdev-change-medium",
                "query-block",
                "query-block",
                "eject",
                "query-block",
            ] {
                let (name, args) = session.recv_command().await.unwrap();
                assert_eq!(name, expected);
                match name.as_str() {
                    "query-block" => {
                        let mut block = json!({"device":"", "qdev":format!("/machine/peripheral/{MEDIA_DEVICE}"),
                            "type":"unknown","removable":true,"locked":false});
                        if inserted {
                            block["inserted"] = json!({"drv":"raw","file":image,"ro":true,"encrypted":false,
                            "image":{"filename":image,"format":"raw","virtual-size":3}});
                        }
                        session.send_return(json!([block])).await.unwrap();
                    }
                    "blockdev-change-medium" => {
                        assert_eq!(args["filename"], image.to_str().unwrap());
                        assert_eq!(args["format"], "raw");
                        assert_eq!(args["read-only-mode"], "read-only");
                        assert_eq!(args["force"], false);
                        inserted = true;
                        session.send_return(json!({})).await.unwrap();
                    }
                    "eject" => {
                        assert_eq!(args["force"], false);
                        inserted = false;
                        session.send_return(json!({})).await.unwrap();
                    }
                    _ => unreachable!(),
                }
            }
        });
        let (qmp, _) = Qmp::new(&path).await.unwrap();
        let inserted = execute(
            &qmp,
            Some(&policy),
            MediaOperation::Insert {
                image: "rescue.iso".into(),
            },
        )
        .await
        .unwrap();
        assert!(inserted.inserted);
        assert_eq!(inserted.image_name.as_deref(), Some("rescue.iso"));
        assert!(!serde_json::to_string(&inserted)
            .unwrap()
            .contains(dir.path().to_str().unwrap()));
        let ejected = execute(&qmp, Some(&policy), MediaOperation::Eject)
            .await
            .unwrap();
        assert!(!ejected.inserted && ejected.image_name.is_none());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn failed_device_hotplug_releases_the_new_block_backend() {
        use qapi_protocol::server::QmpSession;
        use serde_json::json;
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("rescue.iso"), [1]).unwrap();
        let policy = MediaPolicy::new(dir.path().to_owned(), 1024).unwrap();
        let path = dir.path().join("qmp.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let server = tokio::spawn(async move {
            let mut session = QmpSession::accept(&listener).await.unwrap();
            session.send_greeting(11, 1, 3).await.unwrap();
            session.negotiate().await.unwrap();
            for expected in ["query-block", "blockdev-add", "device_add", "blockdev-del"] {
                let (name, args) = session.recv_command().await.unwrap();
                assert_eq!(name, expected);
                match name.as_str() {
                    "query-block" => session.send_return(json!([])).await.unwrap(),
                    "blockdev-add" => {
                        assert_eq!(args["read-only"], true);
                        assert_eq!(args["file"]["read-only"], true);
                        session.send_return(json!({})).await.unwrap();
                    }
                    "device_add" => session
                        .send_error("GenericError", "bus does not support hotplug")
                        .await
                        .unwrap(),
                    "blockdev-del" => {
                        assert_eq!(args["node-name"], MEDIA_NODE);
                        session.send_return(json!({})).await.unwrap();
                    }
                    _ => unreachable!(),
                }
            }
        });
        let (qmp, _) = Qmp::new(&path).await.unwrap();
        assert!(execute(
            &qmp,
            Some(&policy),
            MediaOperation::Insert {
                image: "rescue.iso".into()
            }
        )
        .await
        .is_err());
        server.await.unwrap();
    }
}

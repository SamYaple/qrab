//! Per-VM issuance records: `<state-dir>/<uid>.json`, replaced atomically.
use crate::VmUid;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

mod b64 {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&STANDARD.encode(bytes))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        STANDARD
            .decode(String::deserialize(d)?)
            .map_err(serde::de::Error::custom)
    }
}

/// One issued certificate and the canonical `TPMT_PUBLIC` it certifies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    #[serde(with = "b64")]
    pub public: Vec<u8>,
    #[serde(with = "b64")]
    pub certificate: Vec<u8>,
}

/// A DevID is only ever recorded under the EK it was issued against: an EK
/// re-key drops it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub ek: Option<Entry>,
    pub devid: Option<Entry>,
}

pub struct Store {
    dir: PathBuf,
}

impl Store {
    /// Open (creating with mode 0700 if missing) the state directory.
    pub fn open(dir: impl Into<PathBuf>) -> io::Result<Self> {
        let dir = dir.into();
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)?;
        let store = Self { dir };
        // Fail at startup, not at first issuance, on a read-only volume.
        let probe = store.dir.join(".write-probe");
        fs::write(&probe, b"")?;
        fs::remove_file(&probe)?;
        Ok(store)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, uid: &VmUid) -> PathBuf {
        self.dir.join(format!("{}.json", uid.as_str()))
    }

    pub fn load(&self, uid: &VmUid) -> io::Result<Record> {
        match fs::read(self.path(uid)) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Record::default()),
            Err(e) => Err(e),
        }
    }

    pub fn save(&self, uid: &VmUid, record: &Record) -> io::Result<()> {
        let path = self.path(uid);
        let tmp = self.dir.join(format!(".{}.json.tmp", uid.as_str()));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(&serde_json::to_vec(record)?)?;
        file.sync_all()?;
        fs::rename(&tmp, &path)?;
        fs::File::open(&self.dir)?.sync_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_round_trip_and_missing_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("state")).unwrap();
        let uid = VmUid::parse("00000000-0000-4000-8000-000000000001").unwrap();
        assert_eq!(store.load(&uid).unwrap(), Record::default());
        let record = Record {
            ek: Some(Entry {
                public: vec![1, 2],
                certificate: vec![3],
            }),
            devid: None,
        };
        store.save(&uid, &record).unwrap();
        assert_eq!(store.load(&uid).unwrap(), record);
        let names: Vec<_> = fs::read_dir(store.dir())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, [format!("{}.json", uid.as_str())]);
    }
}

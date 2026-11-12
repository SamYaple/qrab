//! The base-image catalog: ConfigMap `qrab-base-images`, key `images.json`,
//! written by `contrib/images/stage-base-image.sh`. Each entry names a sealed
//! ZFS snapshot `image-<name>-g<generation>@sealed` that VM root disks clone.
use serde::Deserialize;

pub const CONFIGMAP: &str = "qrab-base-images";
pub const KEY: &str = "images.json";
const GIB: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseImage {
    pub name: String,
    pub generation: u32,
    pub title: String,
    pub os: String,
    pub version: String,
    pub size_bytes: u64,
    /// SHA-256 of the imported raw image.
    pub sha256: String,
    /// SHA-256 of the file downloaded from `source_url`.
    pub upstream_sha256: String,
    pub source_url: String,
    pub boot: String,
}

/// Image names as the operator's `baseImage.name` accepts them.
pub fn image_name(name: &str) -> bool {
    let b = name.as_bytes();
    (1..=32).contains(&b.len())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
        && b[0] != b'-'
        && b[b.len() - 1] != b'-'
}

fn sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max && !value.chars().any(char::is_control)
}

impl BaseImage {
    fn check(&self) -> Result<(), String> {
        let problem = if !image_name(&self.name) {
            "name must be 1-32 lowercase letters, digits or '-'"
        } else if !(1..=999_999).contains(&self.generation) {
            "generation must be 1-999999"
        } else if self.size_bytes == 0 || self.size_bytes > crate::launch::MAX_DISK_GIB * GIB {
            "size_bytes must be 1 byte to 1 TiB"
        } else if !sha256(&self.sha256) || !sha256(&self.upstream_sha256) {
            "sha256 and upstream_sha256 must be 64 lowercase hex digits"
        } else if !text(&self.title, 80) || !text(&self.os, 32) || !text(&self.version, 32) {
            "title, os and version must be short printable text"
        } else if !url::Url::parse(&self.source_url).is_ok_and(|u| u.scheme() == "https") {
            "source_url must be an https URL"
        } else if self.boot != "uefi" {
            "boot must be uefi"
        } else {
            return Ok(());
        };
        Err(problem.into())
    }

    /// The launch form's value for this image.
    pub fn key(&self) -> String {
        format!("{}:{}", self.name, self.generation)
    }

    /// `cirros g1`.
    pub fn label(&self) -> String {
        format!("{} g{}", self.name, self.generation)
    }

    /// The smallest root disk that holds the image, in whole GiB (at least 1).
    pub fn min_root_gib(&self) -> u64 {
        self.size_bytes.div_ceil(GIB).max(1)
    }
}

/// Parses `images.json` strictly: a JSON array of entries with exactly the
/// documented fields. Malformed or duplicate entries are skipped with a
/// warning; the rest are sorted by name and generation.
///
/// ```
/// let images = qrab_dashboard::images::parse(r#"[{"name":"cirros","generation":1,
///   "title":"CirrOS 0.6.3","os":"cirros","version":"0.6.3","size_bytes":117440512,
///   "sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
///   "upstream_sha256":"7d6355852aeb6dbcd191bcda7cd74f1536cfe5cbf8a10495a7283a8396e4b75b",
///   "source_url":"https://download.cirros-cloud.net/0.6.3/cirros-0.6.3-x86_64-disk.img",
///   "boot":"uefi"}, {"name":"broken"}]"#);
/// assert_eq!(images.len(), 1);
/// assert_eq!(images[0].min_root_gib(), 1);
/// assert!(qrab_dashboard::images::parse("{}").is_empty());
/// ```
pub fn parse(json: &str) -> Vec<BaseImage> {
    let entries: Vec<serde_json::Value> = match serde_json::from_str(json) {
        Ok(entries) => entries,
        Err(error) => {
            tracing::warn!(%error, "{CONFIGMAP}/{KEY} is not a JSON array; no base images");
            return Vec::new();
        }
    };
    let mut images: Vec<BaseImage> = Vec::new();
    for (index, entry) in entries.into_iter().enumerate() {
        let image = serde_json::from_value::<BaseImage>(entry)
            .map_err(|e| e.to_string())
            .and_then(|image| image.check().map(|()| image));
        match image {
            Ok(image) if images.iter().any(|i| i.key() == image.key()) => {
                tracing::warn!(index, image=%image.key(), "skipping duplicate base image");
            }
            Ok(image) => images.push(image),
            Err(error) => tracing::warn!(index, %error, "skipping malformed base image"),
        }
    }
    images.sort_by(|a, b| (&a.name, a.generation).cmp(&(&b.name, b.generation)));
    images
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn cirros_json() -> serde_json::Value {
        serde_json::json!({"name": "cirros", "generation": 1, "title": "CirrOS 0.6.3", "os": "cirros",
            "version": "0.6.3", "size_bytes": 117440512u64,
            "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "upstream_sha256": "7d6355852aeb6dbcd191bcda7cd74f1536cfe5cbf8a10495a7283a8396e4b75b",
            "source_url": "https://download.cirros-cloud.net/0.6.3/cirros-0.6.3-x86_64-disk.img",
            "boot": "uefi"})
    }

    pub fn cirros() -> BaseImage {
        serde_json::from_value(cirros_json()).unwrap()
    }

    #[test]
    fn malformed_entries_are_skipped_and_the_rest_sorted() {
        let with = |key: &str, value: serde_json::Value| {
            let mut v = cirros_json();
            v[key] = value;
            v
        };
        let mut extra = cirros_json();
        extra["unexpected"] = "x".into();
        let mut missing = cirros_json();
        missing.as_object_mut().unwrap().remove("sha256");
        let catalog = serde_json::json!([
            with("generation", 2.into()),
            cirros_json(),
            cirros_json(),
            with("name", "Bad_Name".into()),
            with("generation", 0.into()),
            with("size_bytes", 0.into()),
            with("sha256", "ABC".into()),
            with("upstream_sha256", "0".repeat(63).into()),
            with("source_url", "http://download.example/x.img".into()),
            with("boot", "bios".into()),
            with("title", "a\nb".into()),
            with("generation", "1".into()),
            extra,
            missing,
            "not an object",
            with("name", "alpine".into()),
        ]);
        let images = parse(&catalog.to_string());
        let keys: Vec<_> = images.iter().map(BaseImage::key).collect();
        assert_eq!(keys, ["alpine:1", "cirros:1", "cirros:2"]);
        assert_eq!(images[1], cirros());
        assert!(parse("not json").is_empty());
        assert!(parse(r#"{"images": []}"#).is_empty());
        assert!(parse("[]").is_empty());
    }

    #[test]
    fn root_minimum_rounds_up_to_whole_gib() {
        let mut image = cirros();
        assert_eq!(image.min_root_gib(), 1);
        image.size_bytes = GIB + 1;
        assert_eq!(image.min_root_gib(), 2);
        image.size_bytes = 4 * GIB;
        assert_eq!(image.min_root_gib(), 4);
        assert_eq!(image.label(), "cirros g1");
    }
}

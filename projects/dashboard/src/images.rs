//! The base-image catalog. Each entry names a sealed ZFS snapshot
//! `image-<name>-g<generation>@sealed` that VM root disks clone. It comes from
//! the `BaseImage` resources the image importer seals ([`catalog`]), plus the
//! legacy ConfigMap `qrab-base-images` (key `images.json`, written by
//! `contrib/images/stage-base-image.sh`) for generations no BaseImage names.
use qrab_operator::image::BaseImage as Resource;
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

pub(crate) fn sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub(crate) fn text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max && !value.chars().any(char::is_control)
}

impl BaseImage {
    /// The entry for a BaseImage resource: display text and source from its
    /// spec, size (`sizeBytes`, else `capacityBytes`) and raw SHA-256 from its
    /// status, which are empty until the importer has measured them.
    pub fn from_resource(resource: &Resource) -> Self {
        let status = resource.status.clone().unwrap_or_default();
        let spec = &resource.spec;
        Self {
            name: spec.image.clone(),
            generation: spec.generation,
            title: spec.title.clone(),
            os: spec.os.clone(),
            version: spec.version.clone(),
            size_bytes: status.size_bytes.or(status.capacity_bytes).unwrap_or(0),
            sha256: status.sha256.unwrap_or_default(),
            upstream_sha256: spec.source.sha256.clone(),
            source_url: spec.source.url.clone(),
            boot: "uefi".into(),
        }
    }

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

/// Whether the resource is named `<image>-g<generation>`; the importer never
/// stages any other.
pub fn well_named(resource: &Resource) -> bool {
    resource.metadata.name.as_deref() == Some(Resource::expected_name(&resource.spec).as_str())
}

/// Launchable: well named, sealed, not retired and measured (a valid size
/// and raw SHA-256 in its status).
pub fn launchable(resource: &Resource) -> bool {
    well_named(resource)
        && resource.launchable()
        && BaseImage::from_resource(resource).check().is_ok()
}

/// The catalog from BaseImage resources and the legacy ConfigMap entries.
/// A BaseImage for a `(name, generation)` always wins, so a legacy entry
/// counts only while no BaseImage names its generation, launchable or not.
/// With `launchable_only`, only [`launchable`] BaseImages count; otherwise
/// every BaseImage is listed (for naming a VM's boot source). Sorted by name
/// and generation.
pub fn catalog(
    resources: &[Resource],
    legacy: Vec<BaseImage>,
    launchable_only: bool,
) -> Vec<BaseImage> {
    let mut ordered: Vec<&Resource> = resources
        .iter()
        .filter(|r| !launchable_only || launchable(r))
        .collect();
    ordered.sort_by_key(|r| !well_named(r));
    let mut images: Vec<BaseImage> = Vec::new();
    for image in ordered.into_iter().map(BaseImage::from_resource) {
        if !images.iter().any(|i| i.key() == image.key()) {
            images.push(image);
        }
    }
    for entry in legacy {
        let shadowed = resources
            .iter()
            .any(|r| r.spec.image == entry.name && r.spec.generation == entry.generation);
        if !shadowed && !images.iter().any(|i| i.key() == entry.key()) {
            images.push(entry);
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

    /// A BaseImage resource `<image>-g<generation>` in `phase`.
    pub fn resource(image: &str, generation: u32, phase: &str, retired: bool) -> Resource {
        serde_json::from_value(serde_json::json!({
            "apiVersion": "qrab.yapplesauce.com/v1alpha1", "kind": "BaseImage",
            "metadata": {"name": format!("{image}-g{generation}"), "namespace": "qrab",
                "uid": format!("uid-{image}-g{generation}")},
            "spec": {"image": image, "generation": generation, "title": format!("CirrOS 0.6.{generation}"),
                "os": "cirros", "version": format!("0.6.{generation}"), "retired": retired,
                "source": {"url": "https://download.cirros-cloud.net/0.6.3/cirros-0.6.3-x86_64-disk.img",
                    "sha256": "7d6355852aeb6dbcd191bcda7cd74f1536cfe5cbf8a10495a7283a8396e4b75b", "format": "qcow2"}},
            "status": {"phase": phase, "sizeBytes": 117440512u64, "capacityBytes": 117440512u64,
                "sha256": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"}
        }))
        .unwrap()
    }

    #[test]
    fn base_images_win_over_the_configmap_and_only_sealed_ones_launch() {
        let keys = |images: Vec<BaseImage>| images.iter().map(BaseImage::key).collect::<Vec<_>>();
        let mut alpine = cirros();
        alpine.name = "alpine".into();
        let legacy = vec![cirros(), alpine];
        // cirros g1 has a BaseImage still writing: it hides the ConfigMap entry.
        let resources = [
            resource("cirros", 1, "Writing", false),
            resource("cirros", 2, "Sealed", false),
            resource("cirros", 3, "Sealed", true),
            resource("cirros", 4, "Failed", false),
        ];
        assert_eq!(
            keys(catalog(&resources, legacy.clone(), true)),
            ["alpine:1", "cirros:2"]
        );
        let sealed = catalog(&resources, legacy.clone(), true);
        assert_eq!(sealed[1].title, "CirrOS 0.6.2");
        assert_eq!(sealed[1].size_bytes, 117440512);
        assert_eq!(
            sealed[1].sha256,
            "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"
        );
        assert_eq!(sealed[1].upstream_sha256, cirros().upstream_sha256);
        assert_eq!(sealed[1].boot, "uefi");
        // Every generation is known for naming a boot source.
        assert_eq!(
            keys(catalog(&resources, legacy.clone(), false)),
            ["alpine:1", "cirros:1", "cirros:2", "cirros:3", "cirros:4"]
        );
        // Without a BaseImage the ConfigMap entry stays launchable.
        assert_eq!(
            keys(catalog(&[], legacy.clone(), true)),
            ["alpine:1", "cirros:1"]
        );
        // Sealed but unmeasured, or misnamed: not launchable.
        let mut unmeasured = resource("cirros", 5, "Sealed", false);
        unmeasured.status.as_mut().unwrap().sha256 = None;
        let mut misnamed = resource("cirros", 6, "Sealed", false);
        misnamed.metadata.name = Some("cirros-latest".into());
        let mut capacity_only = resource("cirros", 7, "Sealed", false);
        capacity_only.status.as_mut().unwrap().size_bytes = None;
        capacity_only.status.as_mut().unwrap().capacity_bytes = Some(GIB + 1);
        let images = catalog(&[unmeasured, misnamed, capacity_only], Vec::new(), true);
        assert_eq!(keys(images.clone()), ["cirros:7"]);
        assert_eq!(images[0].min_root_gib(), 2);
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

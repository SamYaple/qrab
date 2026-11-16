//! `BaseImage`: one sealed base-image generation that VM root disks clone
//! (docs/images.md). An image admin creates it with a download URL and the
//! upstream SHA-256; the image importer stages, writes and seals it through
//! the storage service. A refresh is a new BaseImage with the next generation.
use kube::{CustomResource, CustomResourceExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq)]
#[kube(
    group = "qrab.yapplesauce.com",
    version = "v1alpha1",
    kind = "BaseImage",
    plural = "baseimages",
    namespaced,
    shortname = "bimg",
    status = "BaseImageStatus",
    printcolumn = r#"{"name":"Image","type":"string","jsonPath":".spec.image"}"#,
    printcolumn = r#"{"name":"Gen","type":"integer","jsonPath":".spec.generation"}"#,
    printcolumn = r#"{"name":"Title","type":"string","jsonPath":".spec.title"}"#,
    printcolumn = r#"{"name":"Phase","type":"string","jsonPath":".status.phase"}"#,
    printcolumn = r#"{"name":"Retired","type":"boolean","jsonPath":".spec.retired"}"#
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BaseImageSpec {
    /// Catalog name; VMs clone `image-<image>-g<generation>@sealed`.
    #[schemars(length(min = 1, max = 32), regex(pattern = r"^[a-z0-9]([a-z0-9-]{0,30}[a-z0-9])?$"))]
    pub image: String,
    #[schemars(with = "i32", range(min = 1, max = 999999))]
    pub generation: u32,
    #[schemars(length(min = 1, max = 80))]
    pub title: String,
    #[schemars(length(min = 1, max = 32))]
    pub os: String,
    #[schemars(length(min = 1, max = 32))]
    pub version: String,
    pub source: ImageSource,
    /// Stop new clones of this generation (existing VMs keep theirs).
    #[serde(default)]
    pub retired: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageSource {
    /// HTTPS download URL; redirects must stay on HTTPS.
    #[schemars(length(min = 9, max = 2048), regex(pattern = r"^https://"))]
    pub url: String,
    /// SHA-256 of the downloaded file, checked before anything parses it.
    #[schemars(regex(pattern = r"^[0-9a-f]{64}$"))]
    pub sha256: String,
    pub format: ImageFormat,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    Qcow2,
    Raw,
}

impl ImageFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Qcow2 => "qcow2",
            Self::Raw => "raw",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub enum ImagePhase {
    #[default]
    Pending,
    /// The inspect Job downloads, verifies and measures the image.
    Inspecting,
    /// The stage volume is created and exported.
    Staging,
    /// The write Job converts the image onto the stage volume.
    Writing,
    /// The storage service fences, hashes and seals the stage.
    Sealing,
    Sealed,
    Retired,
    Failed,
}

impl ImagePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Inspecting => "Inspecting",
            Self::Staging => "Staging",
            Self::Writing => "Writing",
            Self::Sealing => "Sealing",
            Self::Sealed => "Sealed",
            Self::Retired => "Retired",
            Self::Failed => "Failed",
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BaseImageStatus {
    #[serde(default)]
    pub phase: ImagePhase,
    /// The image's raw size (the virtual size of the source).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<i64>")]
    pub size_bytes: Option<u64>,
    /// The sealed volume's size; clones must be at least this big.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<i64>")]
    pub capacity_bytes: Option<u64>,
    /// SHA-256 of the sealed raw image, as the storage host measured it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// The generation was already sealed (by `vm-image-import` or publish).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub adopted: bool,
    /// Failed import attempts so far.
    #[serde(default, skip_serializing_if = "is_zero")]
    #[schemars(with = "i32")]
    pub attempts: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

impl BaseImage {
    /// `cirros-g1`: the only accepted metadata.name for a spec.
    pub fn expected_name(spec: &BaseImageSpec) -> String {
        format!("{}-g{}", spec.image, spec.generation)
    }

    pub fn phase(&self) -> ImagePhase {
        self.status.as_ref().map(|s| s.phase).unwrap_or_default()
    }

    /// Launchable: sealed and not retired.
    pub fn launchable(&self) -> bool {
        self.phase() == ImagePhase::Sealed && !self.spec.retired
    }
}

/// The BaseImage CRD with its CEL rules: everything but `retired` is
/// immutable, and a retired generation stays retired. The name rule
/// (`<image>-g<generation>`) exceeds the API server's CEL cost budget, so the
/// importer enforces it ([`BaseImage::expected_name`]) and never stages a
/// misnamed BaseImage.
///
/// ```
/// let crd = qrab_operator::image::crd();
/// let rules = crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["x-kubernetes-validations"].to_string();
/// assert!(rules.contains("self.spec.source == oldSelf.spec.source"));
/// ```
pub fn crd() -> serde_json::Value {
    let mut value = serde_json::to_value(BaseImage::crd()).expect("CRD serializes");
    let schema = &mut value["spec"]["versions"][0]["schema"]["openAPIV3Schema"];
    schema["x-kubernetes-validations"] = serde_json::json!([
        {"rule": "self.spec.image == oldSelf.spec.image && self.spec.generation == oldSelf.spec.generation && self.spec.title == oldSelf.spec.title && self.spec.os == oldSelf.spec.os && self.spec.version == oldSelf.spec.version && self.spec.source == oldSelf.spec.source",
         "message": "a generation is immutable except retired; refresh with a new generation"},
        {"rule": "!has(oldSelf.spec.retired) || !oldSelf.spec.retired || (has(self.spec.retired) && self.spec.retired)",
         "message": "a retired generation stays retired"}
    ]);
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> serde_json::Value {
        serde_json::json!({"image": "cirros", "generation": 2, "title": "CirrOS 0.6.3", "os": "cirros",
            "version": "0.6.3", "source": {"url": "https://download.cirros-cloud.net/0.6.3/cirros-0.6.3-x86_64-disk.img",
            "sha256": "7d6355852aeb6dbcd191bcda7cd74f1536cfe5cbf8a10495a7283a8396e4b75b", "format": "qcow2"}})
    }

    #[test]
    fn specs_parse_strictly_and_name_their_generation() {
        let spec: BaseImageSpec = serde_json::from_value(spec()).unwrap();
        assert_eq!(BaseImage::expected_name(&spec), "cirros-g2");
        assert!(!spec.retired);
        let mut extra = self::spec();
        extra["unexpected"] = true.into();
        assert!(serde_json::from_value::<BaseImageSpec>(extra).is_err());
        let mut image = BaseImage::new("cirros-g2", spec);
        assert!(!image.launchable());
        image.status = Some(BaseImageStatus {
            phase: ImagePhase::Sealed,
            ..Default::default()
        });
        assert!(image.launchable());
        image.spec.retired = true;
        assert!(!image.launchable());
    }

    #[test]
    fn the_crd_pins_the_schema_rules() {
        let crd = crd();
        assert_eq!(
            crd["spec"]["names"]["shortNames"],
            serde_json::json!(["bimg"])
        );
        let props = &crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]
            ["properties"];
        assert_eq!(
            props["source"]["properties"]["sha256"]["pattern"],
            "^[0-9a-f]{64}$"
        );
        assert_eq!(
            props["source"]["properties"]["format"]["enum"],
            serde_json::json!(["qcow2", "raw"])
        );
    }
}

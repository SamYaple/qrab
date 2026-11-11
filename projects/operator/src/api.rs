use kube::{CustomResource, CustomResourceExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn memory() -> u32 {
    512
}
fn cpus() -> u32 {
    1
}
fn running() -> bool {
    true
}
fn boot_disk() -> String {
    "/opt/qrab/demo/boot.img".into()
}

#[derive(CustomResource, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "qrab.yapplesauce.com",
    version = "v1alpha1",
    kind = "VirtualMachine",
    plural = "virtualmachines",
    namespaced,
    shortname = "qvm",
    status = "VirtualMachineStatus"
)]
#[kube(printcolumn = r#"{"name":"Phase","type":"string","jsonPath":".status.phase"}"#)]
#[kube(printcolumn = r#"{"name":"Ready","type":"boolean","jsonPath":".status.ready"}"#)]
#[kube(printcolumn = r#"{"name":"Node","type":"string","jsonPath":".status.nodeName"}"#)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VirtualMachineSpec {
    /// Existing filesystem PVC. Its UEFI/vTPM identity is never garbage collected.
    #[schemars(length(min = 1, max = 253))]
    pub state_volume_claim: String,
    #[serde(default = "memory", rename = "memoryMiB")]
    #[schemars(with = "i32", range(min = 256, max = 1048576))]
    pub memory_mib: u32,
    #[serde(default = "cpus")]
    #[schemars(with = "i32", range(min = 1, max = 256))]
    pub cpus: u32,
    /// Desired power state. A running VM is recreated after exit or Pod loss.
    #[serde(default = "running")]
    pub running: bool,
    /// Change this value to restart without discarding the retained identity.
    #[serde(default)]
    #[schemars(length(max = 128))]
    pub restart_nonce: String,
    /// Optional immutable manager image; defaults to the operator's release pin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(regex(pattern = r"^zot\.yapplesauce\.com/qrab/manager@sha256:[a-f0-9]{64}$"))]
    pub image: Option<String>,
    /// Readonly UEFI boot disk inside the manager image. No direct kernel boot.
    #[serde(default = "boot_disk")]
    #[schemars(length(min = 2, max = 4096))]
    pub boot_disk: String,
    #[serde(default)]
    #[schemars(length(max = 8))]
    pub iscsi_disks: Vec<IscsiDisk>,
    /// Initial iSCSI support is node-pinned; migration requires target fencing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_node: Option<String>,
    pub console: Console,
}

fn password_key() -> String {
    "password".into()
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IscsiDisk {
    #[schemars(regex(pattern = r"^[a-z0-9]([a-z0-9-]{0,18}[a-z0-9])?$"))]
    pub id: String,
    pub portal: String,
    pub target: String,
    #[serde(default)]
    #[schemars(with = "i32", range(min = 0, max = 16383))]
    pub lun: u32,
    #[schemars(length(min = 1, max = 223))]
    pub username: String,
    pub password_secret: PasswordSecret,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<i32>", range(min = 0, max = 65535))]
    pub boot_index: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PasswordSecret {
    pub name: String,
    #[serde(default = "password_key")]
    pub key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Console {
    /// Exact HTTPS origin. Service/Ingress/certificate and IdP callback are separate.
    pub public_url: String,
    #[schemars(length(min = 1, max = 128))]
    pub allowed_subjects: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VirtualMachineStatus {
    pub observed_generation: i64,
    pub phase: String,
    pub ready: bool,
    pub reason: String,
    pub message: String,
    pub pod_name: Option<String>,
    pub pod_uid: Option<String>,
    pub node_name: Option<String>,
    pub state_volume_uid: Option<String>,
    /// Recorded only after clean drain, before deleting the previous Pod.
    pub storage_released_pod_uid: Option<String>,
    /// Administrator acknowledgment of hard target fencing after unclean loss.
    pub storage_recovery: Option<String>,
}

pub fn crd() -> serde_json::Value {
    let mut value = serde_json::to_value(VirtualMachine::crd()).expect("CRD serializes");
    value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["x-kubernetes-validations"] = serde_json::json!([
        {"rule":"size(self.metadata.name) <= 50", "message":"VM names must be at most 50 characters"},
        {"rule":"!has(oldSelf.spec.storageNode) || (has(self.spec.storageNode) && self.spec.storageNode == oldSelf.spec.storageNode)", "message":"storageNode is immutable; disk migration requires explicit fencing"}
    ]);
    let disks = &mut value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]
        ["spec"]["properties"]["iscsiDisks"];
    disks["x-kubernetes-list-type"] = serde_json::json!("map");
    disks["x-kubernetes-list-map-keys"] = serde_json::json!(["id"]);
    disks["items"]["x-kubernetes-validations"] = serde_json::json!([
        {"rule":"self.id == oldSelf.id && self.portal == oldSelf.portal && self.target == oldSelf.target && self.lun == oldSelf.lun", "message":"disk identity is immutable; use a new id for deliberate replacement"}
    ]);
    value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]["properties"]
        ["stateVolumeClaim"]["x-kubernetes-validations"] = serde_json::json!([
        {"rule":"self == oldSelf", "message":"stateVolumeClaim is immutable; identity migration must be deliberate"}
    ]);
    value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]["properties"]
        ["bootDisk"]["enum"] = serde_json::json!(["/opt/qrab/demo/boot.img"]);
    // Optional means absent. Prune explicit null rather than pinning to it.
    let node = value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]
        ["properties"]["storageNode"]
        .as_object_mut()
        .expect("storageNode schema");
    node.remove("nullable");
    node.remove("default");
    value
}

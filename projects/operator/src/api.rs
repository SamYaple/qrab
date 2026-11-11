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
#[kube(printcolumn = r#"{"name":"Resize","type":"string","jsonPath":".status.resize.phase"}"#)]
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
    /// vCPU hot-plug ceiling fixed at boot. Defaults to 4x the booted count
    /// (at most 64). Growth beyond it, or changing it, restarts the VM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<i32>", range(min = 1, max = 256))]
    pub max_cpus: Option<u32>,
    /// Memory hot-plug ceiling fixed at boot. Defaults to 4x the booted size
    /// (at most +64 GiB). Growth beyond it, or changing it, restarts the VM.
    #[serde(
        default,
        rename = "maxMemoryMiB",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Option<i32>", range(min = 256, max = 1048576))]
    pub max_memory_mib: Option<u32>,
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
    /// Disks the operator provisions, exports, grows and fences through the
    /// storage service. CHAP credentials are generated into owned Secrets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(length(max = 8))]
    pub disks: Vec<ProvisionedDisk>,
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

/// A storage-service disk. Target, portal, CHAP account and Secret are derived
/// from the VM UID and disk ID; a CR cannot point it at another LUN.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProvisionedDisk {
    #[schemars(regex(pattern = r"^[a-z0-9]([a-z0-9-]{0,18}[a-z0-9])?$"))]
    pub id: String,
    /// Grow-only capacity, 16MiB..1TiB in whole MiB. Growth restarts the VM.
    #[schemars(with = "i64", range(min = 16777216, max = 1099511627776_i64))]
    pub size_bytes: u64,
    /// Immutable CoW source; absent provisions a blank thick disk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_image: Option<BaseImage>,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<i32>", range(min = 0, max = 65535))]
    pub boot_index: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BaseImage {
    #[schemars(
        length(min = 1, max = 32),
        regex(pattern = r"^[a-z0-9]([a-z0-9-]{0,30}[a-z0-9])?$")
    )]
    pub name: String,
    #[schemars(range(min = 1, max = 999999))]
    pub generation: u32,
}

/// Observed storage-service state for one provisioned disk.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiskStatus {
    pub id: String,
    pub capacity_bytes: u64,
    /// CHAP credential generation; increments only after a verified fence.
    pub generation: u64,
    pub phase: String,
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
    #[serde(default)]
    pub disks: Vec<DiskStatus>,
    /// In-place resource growth of the current Pod.
    pub resize: Option<ResizeStatus>,
}

/// The kubelet side of an in-place resize. `phase` is Applied, Proposed,
/// InProgress, Deferred, Infeasible or Error. The manager hot-adds guest
/// vCPUs/memory only after the applied cgroup limits have grown.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResizeStatus {
    pub phase: String,
    pub message: String,
    /// Guest size requested from the Pod's container resources.
    pub cpus: u32,
    #[serde(rename = "memoryMiB")]
    pub memory_mib: u32,
    /// Guest size whose limits the kubelet reports as applied.
    pub applied_cpus: Option<u32>,
    #[serde(rename = "appliedMemoryMiB")]
    pub applied_memory_mib: Option<u32>,
    /// Size and hot-plug ceilings this Pod's QEMU booted with.
    pub boot_cpus: u32,
    #[serde(rename = "bootMemoryMiB")]
    pub boot_memory_mib: u32,
    pub max_cpus: u32,
    #[serde(rename = "maxMemoryMiB")]
    pub max_memory_mib: u32,
}

pub fn crd() -> serde_json::Value {
    let mut value = serde_json::to_value(VirtualMachine::crd()).expect("CRD serializes");
    value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["x-kubernetes-validations"] = serde_json::json!([
        {"rule":"size(self.metadata.name) <= 50", "message":"VM names must be at most 50 characters"},
        {"rule":"!has(oldSelf.spec.storageNode) || (has(self.spec.storageNode) && self.spec.storageNode == oldSelf.spec.storageNode)", "message":"storageNode is immutable; disk migration requires explicit fencing"},
        {"rule":"!has(self.spec.maxCpus) || self.spec.cpus <= self.spec.maxCpus", "message":"cpus must not exceed maxCpus"},
        {"rule":"!has(self.spec.maxMemoryMiB) || self.spec.memoryMiB <= self.spec.maxMemoryMiB", "message":"memoryMiB must not exceed maxMemoryMiB"}
    ]);
    let disks = &mut value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]
        ["spec"]["properties"]["iscsiDisks"];
    disks["x-kubernetes-list-type"] = serde_json::json!("map");
    disks["x-kubernetes-list-map-keys"] = serde_json::json!(["id"]);
    disks["items"]["x-kubernetes-validations"] = serde_json::json!([
        {"rule":"self.id == oldSelf.id && self.portal == oldSelf.portal && self.target == oldSelf.target && self.lun == oldSelf.lun", "message":"disk identity is immutable; use a new id for deliberate replacement"}
    ]);
    let provisioned = &mut value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]
        ["spec"]["properties"]["disks"];
    provisioned["x-kubernetes-list-type"] = serde_json::json!("map");
    provisioned["x-kubernetes-list-map-keys"] = serde_json::json!(["id"]);
    provisioned["items"]["x-kubernetes-validations"] = serde_json::json!([
        {"rule":"self.sizeBytes >= oldSelf.sizeBytes", "message":"sizeBytes may only grow; ZFS volumes are never shrunk"},
        {"rule":"has(self.baseImage) == has(oldSelf.baseImage) && (!has(self.baseImage) || self.baseImage == oldSelf.baseImage)", "message":"baseImage is immutable; use a new disk id for another source"}
    ]);
    if let Some(image) = provisioned["items"]["properties"]["baseImage"].as_object_mut() {
        image.remove("nullable");
    }
    value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]["properties"]
        ["stateVolumeClaim"]["x-kubernetes-validations"] = serde_json::json!([
        {"rule":"self == oldSelf", "message":"stateVolumeClaim is immutable; identity migration must be deliberate"}
    ]);
    value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]["properties"]
        ["bootDisk"]["enum"] = serde_json::json!(["/opt/qrab/demo/boot.img"]);
    // Optional means absent. Prune explicit null rather than pinning to it.
    for field in ["storageNode", "maxCpus", "maxMemoryMiB"] {
        let schema = value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]
            ["spec"]["properties"][field]
            .as_object_mut()
            .expect("optional field schema");
        schema.remove("nullable");
        schema.remove("default");
    }
    value
}

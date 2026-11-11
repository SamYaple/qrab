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
    pub console: Console,
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
}

pub fn crd() -> serde_json::Value {
    let mut value = serde_json::to_value(VirtualMachine::crd()).expect("CRD serializes");
    value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["x-kubernetes-validations"] = serde_json::json!([
        {"rule":"size(self.metadata.name) <= 50", "message":"VM names must be at most 50 characters"}
    ]);
    value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]["properties"]
        ["stateVolumeClaim"]["x-kubernetes-validations"] = serde_json::json!([
        {"rule":"self == oldSelf", "message":"stateVolumeClaim is immutable; identity migration must be deliberate"}
    ]);
    value
}

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
fn enabled() -> bool {
    true
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
    /// `kvm` requests one `qrab.yapplesauce.com/kvm` device (CDI-injected
    /// /dev/kvm) and enables vCPU hot-plug; such Pods stay Pending on nodes
    /// without the qrab KVM device plugin. Changing it restarts the VM.
    #[serde(default, skip_serializing_if = "Accelerator::is_tcg")]
    pub accelerator: Accelerator,
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
    /// Browser console settings; required while `components.bmc` is enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub console: Option<Console>,
    /// Optional sidecars. The manager container always owns the VM.
    #[serde(default)]
    pub components: Components,
    /// Guest NICs. Each is backed by its own NIC Pod (`<vm>-nic-<name>`)
    /// whose IP, Cilium identity and security-group labels are the NIC's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(length(max = 4))]
    pub nics: Vec<Nic>,
    /// cloud-init NoCloud seed, attached as a read-only `cidata` disk.
    /// Opt-in; adding, changing or removing it restarts the VM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_init: Option<CloudInit>,
}

/// The guest configures itself from this seed without any DHCP or metadata
/// service. meta-data carries `instance-id` (the VM UID) and `local-hostname`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudInit {
    /// Guest hostname, a DNS label. Defaults to the VM name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(regex(pattern = r"^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$"))]
    pub hostname: Option<String>,
    /// user-data passed through verbatim, at most 32 KiB. It is stored in
    /// the VM's operator-owned ConfigMap; reference no secret you would not
    /// put there. Absent means an empty `#cloud-config`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 32768))]
    pub user_data: Option<String>,
}

/// A guest NIC. Guest traffic leaves through its NIC Pod, never the VM Pod.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Nic {
    #[schemars(regex(pattern = r"^[a-z0-9]([a-z0-9-]{0,13}[a-z0-9])?$"))]
    pub name: String,
    /// Each becomes the NIC Pod label `sg.qrab.yapplesauce.com/<group>: ""`
    /// for CiliumNetworkPolicies to select. Changes apply without a restart.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(length(max = 8))]
    pub security_groups: Vec<SecurityGroup>,
}

/// A DNS label naming a security group.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
pub struct SecurityGroup(
    #[schemars(regex(pattern = r"^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$"))] pub String,
);

/// Observed state of one guest NIC.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NicStatus {
    pub name: String,
    pub mac: String,
    pub pod_name: String,
    /// The NIC Pod IP, which is also the guest's address on this NIC.
    pub ip: Option<String>,
    pub ready: bool,
    /// The address this NIC keeps across nodes (its one-address
    /// CiliumPodIPPool), when the operator hands out stable addresses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

/// Sidecars reach the VM only through the manager's control socket.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Components {
    #[serde(default)]
    pub bmc: BmcComponent,
    #[serde(default)]
    pub redfish: RedfishComponent,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BmcComponent {
    /// The OIDC browser console and dashboard. Enabled unless set to false.
    #[serde(default = "enabled")]
    pub enabled: bool,
}

impl Default for BmcComponent {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RedfishComponent {
    /// A DMTF Redfish API authenticated by OIDC bearer tokens.
    #[serde(default)]
    pub enabled: bool,
    /// Subject UUIDs that may read and reset the VM.
    #[serde(default)]
    #[schemars(length(max = 128))]
    pub admin_subjects: Vec<String>,
    /// Subject UUIDs with read-only access.
    #[serde(default)]
    #[schemars(length(max = 128))]
    pub viewer_subjects: Vec<String>,
    /// Kanidm group UUIDs whose members may read and reset the VM.
    #[serde(default)]
    #[schemars(length(max = 16))]
    pub admin_groups: Vec<String>,
    /// Kanidm group UUIDs whose members have read-only access.
    #[serde(default)]
    #[schemars(length(max = 16))]
    pub viewer_groups: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Accelerator {
    /// Portable emulation; the guest refuses hot-added vCPUs.
    #[default]
    Tcg,
    Kvm,
}

impl Accelerator {
    pub fn is_tcg(&self) -> bool {
        *self == Self::Tcg
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tcg => "tcg",
            Self::Kvm => "kvm",
        }
    }
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
    /// Exact HTTPS origin of the BMC's own browser login and UI.
    /// Service/Ingress/certificate and IdP callback are separate. Omit it for a
    /// BMC reached only through the dashboard, which needs the operator's BMC
    /// bearer-token settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_url: Option<String>,
    /// Subject UUIDs with full (admin) BMC access.
    #[serde(default)]
    #[schemars(length(max = 128))]
    pub allowed_subjects: Vec<String>,
    /// Subject UUIDs with read-only BMC access: status, events and redacted configuration.
    #[serde(default)]
    #[schemars(length(max = 128))]
    pub viewer_subjects: Vec<String>,
    /// Kanidm group UUIDs whose members have full BMC access.
    #[serde(default)]
    #[schemars(length(max = 16))]
    pub admin_groups: Vec<String>,
    /// Kanidm group UUIDs whose members have read-only BMC access.
    #[serde(default)]
    #[schemars(length(max = 16))]
    pub viewer_groups: Vec<String>,
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
    #[serde(default)]
    pub nics: Vec<NicStatus>,
    /// The state PVC holding the VM's UEFI/TPM identity now, when a live
    /// migration moved it off `spec.stateVolumeClaim`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_volume_claim: Option<String>,
    /// The node the VM runs on since its last live migration; replaces
    /// `spec.storageNode` as the scheduling pin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement_node: Option<String>,
}

/// Move a running VM to another node without rebooting it. Create one per
/// move; it records the outcome and is never reused.
#[derive(CustomResource, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "qrab.yapplesauce.com",
    version = "v1alpha1",
    kind = "VirtualMachineMigration",
    plural = "virtualmachinemigrations",
    namespaced,
    shortname = "qvmm",
    status = "VirtualMachineMigrationStatus"
)]
#[kube(printcolumn = r#"{"name":"VM","type":"string","jsonPath":".spec.vmName"}"#)]
#[kube(printcolumn = r#"{"name":"Phase","type":"string","jsonPath":".status.phase"}"#)]
#[kube(printcolumn = r#"{"name":"Source","type":"string","jsonPath":".status.sourceNode"}"#)]
#[kube(printcolumn = r#"{"name":"Target","type":"string","jsonPath":".status.targetNode"}"#)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VirtualMachineMigrationSpec {
    /// The VirtualMachine in this namespace to move.
    #[schemars(length(min = 1, max = 50))]
    pub vm_name: String,
    /// Destination node; another Ready node is chosen when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 63))]
    pub target_node: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VirtualMachineMigrationStatus {
    /// Pending, TargetStarting, Migrating, Succeeded or Failed.
    pub phase: String,
    pub message: String,
    pub source_pod: Option<String>,
    pub source_pod_uid: Option<String>,
    pub source_node: Option<String>,
    pub target_pod: Option<String>,
    pub target_pod_uid: Option<String>,
    pub target_node: Option<String>,
    pub target_state_volume_claim: Option<String>,
    /// The source's previous state PVC, deleted after success when the
    /// operator created it for an earlier migration.
    pub previous_state_volume_claim: Option<String>,
    /// Unix seconds.
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    /// Source Pod, policy and replaced PVC are gone (success) or the target
    /// Pod, policy and PVC are gone (failure).
    #[serde(default)]
    pub cleaned: bool,
}

pub fn migration_crd() -> serde_json::Value {
    let mut value = serde_json::to_value(VirtualMachineMigration::crd()).expect("CRD serializes");
    value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["x-kubernetes-validations"] = serde_json::json!([
        {"rule":"self.spec == oldSelf.spec", "message":"a migration is immutable; create a new one"}
    ]);
    let schema = value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]
        ["properties"]["targetNode"]
        .as_object_mut()
        .expect("targetNode schema");
    schema.remove("nullable");
    schema.remove("default");
    value
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
        {"rule":"!has(self.spec.maxMemoryMiB) || self.spec.memoryMiB <= self.spec.maxMemoryMiB", "message":"memoryMiB must not exceed maxMemoryMiB"},
        {"rule":"has(self.spec.console) || (has(self.spec.components) && has(self.spec.components.bmc) && has(self.spec.components.bmc.enabled) && !self.spec.components.bmc.enabled)", "message":"console is required while components.bmc is enabled"}
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
    let nics = &mut value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]
        ["properties"]["nics"];
    nics["x-kubernetes-list-type"] = serde_json::json!("map");
    nics["x-kubernetes-list-map-keys"] = serde_json::json!(["name"]);
    nics["items"]["properties"]["securityGroups"]["x-kubernetes-list-type"] =
        serde_json::json!("set");
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

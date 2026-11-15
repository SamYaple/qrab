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
    /// Explicit read-only catalog PVC of approved virtual media. Mounted only
    /// into the manager; no file uploads, PVC creation or garbage collection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_volume_claim: Option<String>,
    /// Explicit read-only approved firmware catalog PVC. Its root-owned sealed
    /// manifest and CODE artifacts must satisfy manager catalog validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firmware_catalog_volume_claim: Option<String>,
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
    /// Desired power state. While true the operator keeps a VM Pod and recreates
    /// it after exit or loss; a guest powered off inside it stays off until a
    /// Redfish On. False deletes the Pod and keeps the VM off.
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
    /// Read-only UEFI boot media: the immutable image ESP, or one safe .img/.iso
    /// basename in /var/lib/qrab-media when mediaVolumeClaim is configured.
    /// State, Secrets, nested paths and direct kernel boot are never selected.
    #[serde(default = "boot_disk")]
    #[schemars(length(min = 2, max = 148))]
    #[schemars(regex(
        pattern = r"^(/opt/qrab/demo/boot[.]img|/var/lib/qrab-media/[A-Za-z0-9][A-Za-z0-9._-]{0,123}[.](img|iso))$"
    ))]
    pub boot_disk: String,
    /// Select explicit secure-capable OVMF with SMM-protected variable flash.
    /// Key enrollment and requested enablement remain separate operations.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub secure_boot_capable: bool,
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
    /// How guest NICs carry traffic. `stream` (default): QEMU's stream
    /// netdev to a relay and passt in each NIC Pod. `tap`: a kernel tap with
    /// vhost-net in a restricted anchor Pod on the VM Pod's node, built by
    /// qrab-netd; it needs `accelerator: kvm` and stable NIC addresses.
    /// Changing it restarts the VM.
    #[serde(default, skip_serializing_if = "NicDatapath::is_stream")]
    pub nic_datapath: NicDatapath,
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

impl Components {
    /// Whether the Pod gets a Redfish sidecar: always with the BMC, whose
    /// browser UI manages the VM through it, and otherwise when explicitly
    /// enabled. A stored `false` beside an enabled BMC (the old CRD default,
    /// persisted into every object written before) does not remove it.
    ///
    /// ```
    /// use qrab_operator::api::Components;
    /// let components = |json| serde_json::from_value::<Components>(json).unwrap();
    /// assert!(components(serde_json::json!({})).redfish_enabled());
    /// assert!(components(serde_json::json!({"redfish": {"enabled": false}})).redfish_enabled());
    /// assert!(!components(serde_json::json!({"bmc": {"enabled": false}})).redfish_enabled());
    /// assert!(components(serde_json::json!({"bmc": {"enabled": false}, "redfish": {"enabled": true}})).redfish_enabled());
    /// ```
    pub fn redfish_enabled(&self) -> bool {
        self.bmc.enabled || self.redfish.enabled == Some(true)
    }
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
    /// Optional periodic Redfish metric history cadence. Absent keeps reports
    /// on demand and avoids idle VM metric queries; enabled triggers still poll.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<i32>", range(min = 5, max = 3600))]
    pub telemetry_interval_seconds: Option<u32>,
    /// Exact HTTPS event receiver URLs, without credentials, query, fragment
    /// or non-443 ports. Empty disables outbound Redfish event subscriptions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(length(max = 32))]
    pub event_destinations: Vec<String>,
    /// Explicit separate PVC for Redfish subscription state. The operator never
    /// creates or deletes it; it must not be the VM's firmware/TPM state PVC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_volume_claim: Option<String>,
    /// A DMTF Redfish API authenticated by OIDC bearer tokens. Absent, it
    /// follows `components.bmc.enabled`: the BMC manages the VM through it,
    /// so it cannot be turned off while the BMC is enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
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
pub enum NicDatapath {
    /// QEMU stream netdev → relay → passt (userspace) in each NIC Pod.
    #[default]
    Stream,
    /// Kernel tap and vhost-net in an anchor Pod (docs/networking.md).
    Tap,
}

impl NicDatapath {
    pub fn is_stream(&self) -> bool {
        *self == Self::Stream
    }
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
    /// Console features with data-exfiltration or device-injection risk.
    /// Each is off unless set here, and even then only a console admin who
    /// requests it on a connection gets it (docs/bmc.md#admin-bucket).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_bucket: Option<AdminBucket>,
}

/// The admin bucket: risky console features a VM opts into one by one.
///
/// ```
/// use qrab_operator::api::AdminBucket;
/// let bucket: AdminBucket = serde_json::from_str(r#"{"usbRedirect":true}"#).unwrap();
/// assert_eq!(bucket.features(), ["usb-redirect"]);
/// assert!(AdminBucket::default().features().is_empty());
/// assert!(serde_json::from_str::<AdminBucket>(r#"{"microphone":true}"#).is_err());
/// ```
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdminBucket {
    /// Guest audio playback in the console (no microphone).
    #[serde(default)]
    pub audio: bool,
    /// Browser USB device redirection into the guest.
    #[serde(default)]
    pub usb_redirect: bool,
}

impl AdminBucket {
    /// The enabled features by their BMC names (`BMC_ADMIN_BUCKET`, and
    /// `?features=` at console admission), in a fixed order.
    pub fn features(&self) -> Vec<&'static str> {
        [(self.audio, "audio"), (self.usb_redirect, "usb-redirect")]
            .into_iter()
            .filter_map(|(enabled, name)| enabled.then_some(name))
            .collect()
    }
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
#[kube(printcolumn = r#"{"name":"Reason","type":"string","jsonPath":".status.reason"}"#)]
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
    /// Machine-readable cause of a `Failed` phase, when one is known:
    /// `PoweredOff` means the VM's QEMU was powered off in its Pod.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
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

/// The dashboard ServiceAccount, which may patch VMs only to power them on or off.
pub const DASHBOARD_USER: &str = "system:serviceaccount:qrab:qrab-dashboard";

/// A ValidatingAdmissionPolicy that refuses any dashboard update changing
/// anything but `spec.running`. CEL cannot iterate a CRD's typed object
/// fields, so the rule names every other top-level spec field from the CRD
/// itself and is regenerated with it (checked by `contrib/check-runtime.sh`).
///
/// ```
/// let policy = qrab_operator::api::dashboard_update_policy();
/// let rule = policy["spec"]["validations"][0]["expression"].as_str().unwrap();
/// assert!(rule.contains("object.spec.console == oldObject.spec.console"));
/// assert!(!rule.contains("spec.running"));
/// ```
pub fn dashboard_update_policy() -> serde_json::Value {
    let crd = crd();
    let mut fields: Vec<&String> = crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]
        ["properties"]["spec"]["properties"]
        .as_object()
        .expect("spec properties")
        .keys()
        .filter(|field| field.as_str() != "running")
        .collect();
    fields.sort();
    let unchanged = fields
        .iter()
        .map(|f| {
            format!(
                "(has(object.spec.{f}) == has(oldObject.spec.{f}) && (!has(object.spec.{f}) || object.spec.{f} == oldObject.spec.{f}))"
            )
        })
        .collect::<Vec<_>>()
        .join(" && ");
    let same = |key: &str| {
        format!(
            "(has(object.metadata.{key}) ? object.metadata.{key} : {{}}) == (has(oldObject.metadata.{key}) ? oldObject.metadata.{key} : {{}})"
        )
    };
    serde_json::json!({
        "apiVersion": "admissionregistration.k8s.io/v1",
        "kind": "ValidatingAdmissionPolicy",
        "metadata": {"name": "qrab-dashboard-vm-updates"},
        "spec": {
            "failurePolicy": "Fail",
            "matchConstraints": {"resourceRules": [{
                "apiGroups": ["qrab.yapplesauce.com"], "apiVersions": ["v1alpha1"],
                "operations": ["UPDATE"], "resources": ["virtualmachines"]
            }]},
            "matchConditions": [{
                "name": "dashboard",
                "expression": format!("request.userInfo.username == '{DASHBOARD_USER}'")
            }],
            "validations": [
                {"expression": unchanged,
                 "message": "the dashboard may change only spec.running on an existing VirtualMachine"},
                {"expression": format!("{} && {}", same("labels"), same("annotations")),
                 "message": "the dashboard may not change a VirtualMachine's labels or annotations"}
            ]
        }
    })
}

pub fn crd() -> serde_json::Value {
    let mut value = serde_json::to_value(VirtualMachine::crd()).expect("CRD serializes");
    value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["x-kubernetes-validations"] = serde_json::json!([
        {"rule":"size(self.metadata.name) <= 50", "message":"VM names must be at most 50 characters"},
        {"rule":"!has(oldSelf.spec.storageNode) || (has(self.spec.storageNode) && self.spec.storageNode == oldSelf.spec.storageNode)", "message":"storageNode is immutable; disk migration requires explicit fencing"},
        {"rule":"!has(self.spec.maxCpus) || self.spec.cpus <= self.spec.maxCpus", "message":"cpus must not exceed maxCpus"},
        {"rule":"!has(self.spec.maxMemoryMiB) || self.spec.memoryMiB <= self.spec.maxMemoryMiB", "message":"memoryMiB must not exceed maxMemoryMiB"},
        {"rule":"!has(self.spec.nicDatapath) || self.spec.nicDatapath != 'tap' || (has(self.spec.accelerator) && self.spec.accelerator == 'kvm')", "message":"nicDatapath tap needs accelerator kvm: the KVM device brings qrab-netd's socket into the VM Pod"},
        {"rule":"has(self.spec.console) || (has(self.spec.components) && has(self.spec.components.bmc) && has(self.spec.components.bmc.enabled) && !self.spec.components.bmc.enabled)", "message":"console is required while components.bmc is enabled"}
    ]);
    // On `components`, not the root: CRD validation ratcheting then keeps
    // admitting updates of objects whose stored components still carry the
    // old `redfish.enabled: false` default, while new values are checked.
    let components = &mut value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]
        ["spec"]["properties"]["components"];
    components["x-kubernetes-validations"] = serde_json::json!([
        {"rule":"!(has(self.redfish) && has(self.redfish.enabled) && !self.redfish.enabled) || (has(self.bmc) && has(self.bmc.enabled) && !self.bmc.enabled)", "message":"components.redfish.enabled cannot be false while components.bmc is enabled: the BMC manages the VM through its Redfish sidecar"}
    ]);
    components["properties"]["redfish"]["properties"]["enabled"]
        .as_object_mut()
        .expect("redfish.enabled schema")
        .remove("nullable");
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
    value["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]
        ["x-kubernetes-validations"] = serde_json::json!([
        {"rule":"self.bootDisk == '/opt/qrab/demo/boot.img' || (has(self.mediaVolumeClaim) && self.mediaVolumeClaim != '' && self.mediaVolumeClaim != self.stateVolumeClaim && !self.bootDisk.contains('..'))",
         "message":"bootDisk media requires a separate mediaVolumeClaim and a safe basename without '..'"}
    ]);
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

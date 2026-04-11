#![allow(unused_imports, non_camel_case_types, dead_code)]
use crate as qapi;
use qapi_macros::qapi;
use std::collections::HashMap;
use super::*;
/// Emitted when guest executes ACPI _OST method.
///
#[qapi(event)]
#[qapi(name = "ACPI_DEVICE_OST")]
#[qapi(added = "2.1.0")]
pub struct AcpiDeviceOstEvent {
    /// ACPIOSTInfo type as described in qapi-schema.json
    ///
    pub info: AcpiostInfo,
}

/// Emitted when the guest changes the actual BALLOON level. This value is
/// equivalent to the @actual field return by the 'query-balloon' command
///
#[qapi(event)]
#[qapi(name = "BALLOON_CHANGE")]
#[qapi(added = "2.1.0")]
pub struct BalloonChangeEvent {
    /// actual level of the guest memory balloon in bytes
    pub actual: i64,
}

/// Emitted when a block export is removed and its id can be reused.
///
#[qapi(event)]
#[qapi(name = "BLOCK_EXPORT_DELETED")]
#[qapi(added = "5.2.0")]
pub struct BlockExportDeletedEvent {
    /// Block export id.
    pub id: String,
}

/// Emitted when a disk image is being marked corrupt.  The image can be
/// identified by its device or node name.  The 'device' field is always
/// present for compatibility reasons, but it can be empty ("") if the
/// image does not have a device name associated.
///
#[qapi(event)]
#[qapi(name = "BLOCK_IMAGE_CORRUPTED")]
#[qapi(added = "2.1.0")]
pub struct BlockImageCorruptedEvent {
    /// device name.  This is always present for compatibility
    /// reasons, but it can be empty ("") if the image does not have a
    /// device name associated.
    pub device: String,
    /// if set, the image is marked corrupt and therefore unusable
    /// after this event and must be repaired (Since 2.2; before, every
    /// `BLOCK_IMAGE_CORRUPTED` event was fatal)
    ///
    /// .. note:: If action is "stop", a `STOP` event will eventually follow
    /// the `BLOCK_IO_ERROR` event.
    ///
    /// .. qmp-example::
    ///
    /// <- { "event": "BLOCK_IMAGE_CORRUPTED",
    /// "data": { "device": "", "node-name": "drive", "fatal": false,
    /// "msg": "L2 table offset 0x2a2a2a00 unaligned (L1 index: 0)" },
    /// "timestamp": { "seconds": 1648243240, "microseconds": 906060 } }
    #[qapi(added = "2.2.0")]
    pub fatal: Option<bool>,
    /// informative message for human consumption, such as the kind of
    /// corruption being detected.  It should not be parsed by machine
    /// as it is not guaranteed to be stable
    pub msg: String,
    /// node name (Since: 2.4)
    #[qapi(name = "node-name")]
    #[qapi(added = "2.4.0")]
    pub node_name: Option<String>,
    /// if the corruption resulted from an image access, this is
    /// the host's access offset into the image
    pub offset: Option<i64>,
    /// if the corruption resulted from an image access, this is the
    /// access size
    pub size: Option<i64>,
}

#[qapi(event)]
#[qapi(name = "BLOCK_IO_ERROR")]
#[qapi(added = "2.1.0")]
pub struct BlockIoErrorEvent {
    pub action: BlockErrorAction,
    pub device: String,
    #[qapi(name = "node-name")]
    #[qapi(added = "2.8.0")]
    pub node_name: Option<String>,
    #[qapi(added = "2.2.0")]
    pub nospace: Option<bool>,
    pub operation: IoOperationType,
    #[qapi(name = "qom-path")]
    #[qapi(added = "9.2.0")]
    pub qom_path: Option<String>,
    #[qapi(added = "2.2.0")]
    pub reason: Option<String>,
}

/// Emitted when a block job has been cancelled
///
#[qapi(event)]
#[qapi(name = "BLOCK_JOB_CANCELLED")]
#[qapi(added = "2.1.0")]
pub struct BlockJobCancelledEvent {
    /// The job identifier. Originally the device name but other
    /// values are allowed since QEMU 2.7
    pub device: String,
    /// maximum progress value
    pub len: i64,
    /// current progress value. On success this is equal to len.
    /// On failure this is less than len
    pub offset: i64,
    /// rate limit, bytes per second
    pub speed: i64,
    /// job type
    pub r#type: BlockJobType,
}

/// Emitted when a block job has completed
///
#[qapi(event)]
#[qapi(name = "BLOCK_JOB_COMPLETED")]
#[qapi(added = "2.1.0")]
pub struct BlockJobCompletedEvent {
    /// The job identifier. Originally the device name but other
    /// values are allowed since QEMU 2.7
    pub device: String,
    /// optional, error message. Only present on failure. This field
    /// contains a human-readable error message. There are no semantics
    /// other than that streaming has failed and clients should not try to
    /// interpret the error string
    pub error: Option<String>,
    /// maximum progress value
    pub len: i64,
    /// current progress value. On success this is equal to len.
    /// On failure this is less than len
    pub offset: i64,
    /// rate limit, bytes per second
    pub speed: i64,
    /// job type
    pub r#type: BlockJobType,
}

/// Emitted when a block job encounters an error
///
#[qapi(event)]
#[qapi(name = "BLOCK_JOB_ERROR")]
#[qapi(added = "2.1.0")]
pub struct BlockJobErrorEvent {
    /// action that has been taken
    pub action: BlockErrorAction,
    /// The job identifier. Originally the device name but other
    /// values are allowed since QEMU 2.7
    pub device: String,
    /// I/O operation
    pub operation: IoOperationType,
}

#[qapi(event)]
#[qapi(name = "BLOCK_JOB_PENDING")]
#[qapi(added = "2.12.0")]
pub struct BlockJobPendingEvent {
    pub id: String,
    pub r#type: BlockJobType,
}

#[qapi(event)]
#[qapi(name = "BLOCK_JOB_READY")]
#[qapi(added = "2.1.0")]
pub struct BlockJobReadyEvent {
    pub device: String,
    pub len: i64,
    pub offset: i64,
    pub speed: i64,
    pub r#type: BlockJobType,
}

/// Emitted when writes on block device reaches or exceeds the
/// configured write threshold.  For thin-provisioned devices, this
/// means the device should be extended to avoid pausing for disk
/// exhaustion.  The event is one shot.  Once triggered, it needs to be
/// re-registered with another `block-set-write-threshold` command.
///
#[qapi(event)]
#[qapi(name = "BLOCK_WRITE_THRESHOLD")]
#[qapi(added = "2.3.0")]
pub struct BlockWriteThresholdEvent {
    /// amount of data which exceeded the threshold, in
    /// bytes.
    #[qapi(name = "amount-exceeded")]
    pub amount_exceeded: u64,
    /// graph node name on which the threshold was exceeded.
    #[qapi(name = "node-name")]
    pub node_name: String,
    /// last configured threshold, in bytes.
    #[qapi(name = "write-threshold")]
    pub write_threshold: u64,
}

#[qapi(event)]
#[qapi(name = "COLO_EXIT")]
#[qapi(added = "3.1.0")]
pub struct ColoExitEvent {
    pub mode: ColoMode,
    pub reason: ColoExitReason,
}

#[qapi(event)]
#[qapi(name = "COMMAND_DROPPED")]
#[qapi(added = "2.12.0")]
#[qapi(removed = "4.0.0")]
pub struct CommandDroppedEvent {
    pub id: serde_json::Value,
    pub reason: CommandDropReason,
}

#[qapi(event)]
#[qapi(name = "CPU_POLARIZATION_CHANGE")]
#[qapi(condition = "(TARGET_S390X && CONFIG_KVM)")]
#[qapi(feature = "unstable")]
#[qapi(added = "8.2.0")]
#[qapi(unstable(added = "8.2.0"))]
pub struct CpuPolarizationChangeEvent {
    pub polarization: CpuS390Polarization,
}

/// Emitted whenever the device removal completion is acknowledged by the guest.
/// At this point, it's safe to reuse the specified device ID. Device removal can
/// be initiated by the guest or by HMP/QMP commands.
///
#[qapi(event)]
#[qapi(name = "DEVICE_DELETED")]
#[qapi(added = "2.1.0")]
pub struct DeviceDeletedEvent {
    /// optional, device name
    pub device: Option<String>,
    /// device path
    pub path: String,
}

/// Emitted whenever the tray of a removable device is moved by the guest or by
/// HMP/QMP commands
///
#[qapi(event)]
#[qapi(name = "DEVICE_TRAY_MOVED")]
#[qapi(added = "2.1.0")]
pub struct DeviceTrayMovedEvent {
    /// Block device name. This is always present for compatibility
    /// reasons, but it can be empty ("") if the image does not
    /// have a device name associated.
    pub device: String,
    /// The name or QOM path of the guest device (since 2.8)
    #[qapi(added = "2.8.0")]
    pub id: Option<String>,
    /// true if the tray has been opened or false if it has been closed
    #[qapi(name = "tray-open")]
    pub tray_open: bool,
}

#[qapi(event)]
#[qapi(name = "DEVICE_UNPLUG_GUEST_ERROR")]
#[qapi(added = "6.2.0")]
pub struct DeviceUnplugGuestErrorEvent {
    pub device: Option<String>,
    pub path: String,
}

/// Emitted when background dump has completed
///
#[qapi(event)]
#[qapi(name = "DUMP_COMPLETED")]
#[qapi(added = "2.6.0")]
pub struct DumpCompletedEvent {
    /// optional human-readable error string that provides
    /// hint on why dump failed. Only presents on failure. The
    /// user should not try to interpret the error string.
    pub error: Option<String>,
    /// DumpQueryResult type described in qapi-schema.json.
    pub result: DumpQueryResult,
}

#[qapi(event)]
#[qapi(name = "FAILOVER_NEGOTIATED")]
#[qapi(added = "4.2.0")]
pub struct FailoverNegotiatedEvent {
    #[qapi(name = "device-id")]
    pub device_id: String,
}

#[qapi(event)]
#[qapi(name = "GUEST_CRASHLOADED")]
#[qapi(added = "5.0.0")]
pub struct GuestCrashloadedEvent {
    pub action: GuestPanicAction,
    pub info: Option<GuestPanicInformation>,
}

/// Emitted when guest OS panic is detected
///
#[qapi(event)]
#[qapi(name = "GUEST_PANICKED")]
#[qapi(added = "2.1.0")]
pub struct GuestPanickedEvent {
    /// action that has been taken, currently always "pause"
    pub action: GuestPanicAction,
    #[qapi(added = "2.9.0")]
    pub info: Option<GuestPanicInformation>,
}

#[qapi(event)]
#[qapi(name = "GUEST_PVSHUTDOWN")]
#[qapi(added = "9.1.0")]
pub struct GuestPvshutdownEvent {}

#[qapi(event)]
#[qapi(name = "HV_BALLOON_STATUS_REPORT")]
#[qapi(added = "8.2.0")]
pub struct HvBalloonStatusReportEvent {
    /// the amount of the memory inside the guest available for
    /// new allocations ("free")
    pub available: u64,
    /// the amount of memory in use inside the guest plus the
    /// amount of the memory unusable inside the guest (ballooned out,
    /// offline, etc.)
    pub committed: u64,
}

/// Emitted when a job transitions to a different status.
///
#[qapi(event)]
#[qapi(name = "JOB_STATUS_CHANGE")]
#[qapi(added = "3.0.0")]
pub struct JobStatusChangeEvent {
    /// The job identifier
    pub id: String,
    /// The new job status
    pub status: JobStatus,
}

#[qapi(event)]
#[qapi(name = "MEMORY_DEVICE_SIZE_CHANGE")]
#[qapi(added = "5.1.0")]
pub struct MemoryDeviceSizeChangeEvent {
    pub id: Option<String>,
    #[qapi(name = "qom-path")]
    #[qapi(added = "6.2.0")]
    pub qom_path: Option<String>,
    pub size: u64,
}

#[qapi(event)]
#[qapi(name = "MEMORY_FAILURE")]
#[qapi(added = "5.2.0")]
pub struct MemoryFailureEvent {
    pub action: MemoryFailureAction,
    pub flags: MemoryFailureFlags,
    pub recipient: MemoryFailureRecipient,
}

/// Emitted when memory hot unplug error occurs.
///
#[qapi(event)]
#[qapi(name = "MEM_UNPLUG_ERROR")]
#[qapi(added = "2.4.0")]
#[qapi(deprecated(added = "6.2.0"))]
#[qapi(removed = "9.1.0")]
pub struct MemUnplugErrorEvent {
    /// device name
    pub device: String,
    /// Informative message
    pub msg: String,
}

/// Emitted when a migration event happens
///
#[qapi(event)]
#[qapi(name = "MIGRATION")]
#[qapi(added = "2.4.0")]
pub struct MigrationEvent {
    /// @MigrationStatus describing the current migration status.
    pub status: MigrationStatus,
}

/// Emitted from the source side of a migration at the start of each pass
/// (when it syncs the dirty bitmap)
///
#[qapi(event)]
#[qapi(name = "MIGRATION_PASS")]
#[qapi(added = "2.6.0")]
pub struct MigrationPassEvent {
    /// An incrementing count (starting at 1 on the first pass)
    pub pass: i64,
}

#[qapi(event)]
#[qapi(name = "NETDEV_STREAM_CONNECTED")]
#[qapi(added = "7.2.0")]
pub struct NetdevStreamConnectedEvent {
    pub addr: SocketAddress,
    #[qapi(name = "netdev-id")]
    pub netdev_id: String,
}

#[qapi(event)]
#[qapi(name = "NETDEV_STREAM_DISCONNECTED")]
#[qapi(added = "7.2.0")]
pub struct NetdevStreamDisconnectedEvent {
    #[qapi(name = "netdev-id")]
    pub netdev_id: String,
}

#[qapi(event)]
#[qapi(name = "NETDEV_VHOST_USER_CONNECTED")]
#[qapi(added = "10.0.0")]
pub struct NetdevVhostUserConnectedEvent {
    #[qapi(name = "chardev-id")]
    pub chardev_id: String,
    #[qapi(name = "netdev-id")]
    pub netdev_id: String,
}

#[qapi(event)]
#[qapi(name = "NETDEV_VHOST_USER_DISCONNECTED")]
#[qapi(added = "10.0.0")]
pub struct NetdevVhostUserDisconnectedEvent {
    #[qapi(name = "netdev-id")]
    pub netdev_id: String,
}

/// Emitted once until the 'query-rx-filter' command is executed, the first event
/// will always be emitted
///
#[qapi(event)]
#[qapi(name = "NIC_RX_FILTER_CHANGED")]
#[qapi(added = "2.1.0")]
pub struct NicRxFilterChangedEvent {
    /// optional, net client name
    pub name: Option<String>,
    /// device path
    pub path: String,
}

/// Emitted when the virtual machine is powered down through the power control
/// system, such as via ACPI.
///
#[qapi(event)]
#[qapi(name = "POWERDOWN")]
#[qapi(added = "2.1.0")]
pub struct PowerdownEvent {}

#[qapi(event)]
#[qapi(name = "PR_MANAGER_STATUS_CHANGED")]
#[qapi(added = "3.0.0")]
pub struct PrManagerStatusChangedEvent {
    pub connected: bool,
    pub id: String,
}

/// Emitted by the Quorum block driver if it fails to establish a quorum
///
#[qapi(event)]
#[qapi(name = "QUORUM_FAILURE")]
#[qapi(added = "2.1.0")]
pub struct QuorumFailureEvent {
    /// device name if defined else node name
    pub reference: String,
    /// number of the first sector of the failed read operation
    #[qapi(name = "sector-num")]
    pub sector_num: i64,
    /// failed read operation sector count
    #[qapi(name = "sectors-count")]
    pub sectors_count: i64,
}

/// Emitted to report a corruption of a Quorum file
///
#[qapi(event)]
#[qapi(name = "QUORUM_REPORT_BAD")]
#[qapi(added = "2.1.0")]
pub struct QuorumReportBadEvent {
    /// optional, error message. Only present on failure. This field
    /// contains a human-readable error message. There are no semantics other
    /// than that the block layer reported an error and clients should not
    /// try to interpret the error string.
    pub error: Option<String>,
    /// the graph node name of the block driver state
    #[qapi(name = "node-name")]
    pub node_name: String,
    /// number of the first sector of the failed read operation
    #[qapi(name = "sector-num")]
    pub sector_num: i64,
    /// failed read operation sector count
    #[qapi(name = "sectors-count")]
    pub sectors_count: i64,
    /// quorum operation type (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub r#type: Option<QuorumOpType>,
}

#[qapi(event)]
#[qapi(name = "RDMA_GID_STATUS_CHANGED")]
#[qapi(added = "4.0.0")]
#[qapi(removed = "9.1.0")]
pub struct RdmaGidStatusChangedEvent {
    #[qapi(name = "gid-status")]
    pub gid_status: bool,
    #[qapi(name = "interface-id")]
    pub interface_id: u64,
    pub netdev: String,
    #[qapi(name = "subnet-prefix")]
    pub subnet_prefix: u64,
}

/// Emitted when the virtual machine is reset
///
#[qapi(event)]
#[qapi(name = "RESET")]
#[qapi(added = "2.1.0")]
pub struct ResetEvent {
    #[qapi(added = "2.10.0")]
    pub guest: Option<bool>,
    #[qapi(added = "4.0.0")]
    pub reason: Option<ShutdownCause>,
}

/// Emitted when the virtual machine resumes execution
///
#[qapi(event)]
#[qapi(name = "RESUME")]
#[qapi(added = "2.1.0")]
pub struct ResumeEvent {}

/// Emitted when the guest changes the RTC time.
///
#[qapi(event)]
#[qapi(name = "RTC_CHANGE")]
#[qapi(added = "2.1.0")]
pub struct RtcChangeEvent {
    /// offset between base RTC clock (as specified by -rtc base), and
    /// new RTC clock value
    pub offset: i64,
    #[qapi(name = "qom-path")]
    #[qapi(added = "7.0.0")]
    pub qom_path: Option<String>,
}

#[qapi(event)]
#[qapi(name = "SCLP_CPI_INFO_AVAILABLE")]
#[qapi(feature = "unstable")]
#[qapi(added = "10.2.0")]
#[qapi(unstable(added = "10.2.0"))]
pub struct SclpCpiInfoAvailableEvent {}

#[qapi(event)]
#[qapi(name = "SHUTDOWN")]
#[qapi(added = "2.1.0")]
pub struct ShutdownEvent {
    #[qapi(added = "2.10.0")]
    pub guest: Option<bool>,
    #[qapi(added = "4.0.0")]
    pub reason: Option<ShutdownCause>,
}

/// Emitted when a SPICE client establishes a connection
///
#[qapi(event)]
#[qapi(name = "SPICE_CONNECTED")]
#[qapi(added = "2.1.0")]
pub struct SpiceConnectedEvent {
    /// client information
    pub client: SpiceBasicInfo,
    /// server information
    pub server: SpiceBasicInfo,
}

/// Emitted when the SPICE connection is closed
///
#[qapi(event)]
#[qapi(name = "SPICE_DISCONNECTED")]
#[qapi(added = "2.1.0")]
pub struct SpiceDisconnectedEvent {
    /// client information
    pub client: SpiceBasicInfo,
    /// server information
    pub server: SpiceBasicInfo,
}

/// Emitted after initial handshake and authentication takes place (if any)
/// and the SPICE channel is up and running
///
#[qapi(event)]
#[qapi(name = "SPICE_INITIALIZED")]
#[qapi(added = "2.1.0")]
pub struct SpiceInitializedEvent {
    /// client information
    pub client: SpiceChannel,
    /// server information
    pub server: SpiceServerInfo,
}

/// Emitted when SPICE migration has completed
///
#[qapi(event)]
#[qapi(name = "SPICE_MIGRATE_COMPLETED")]
#[qapi(added = "2.1.0")]
pub struct SpiceMigrateCompletedEvent {}

/// Emitted when the virtual machine is stopped
///
#[qapi(event)]
#[qapi(name = "STOP")]
#[qapi(added = "2.1.0")]
pub struct StopEvent {}

#[qapi(event)]
#[qapi(name = "SUSPEND_DISK")]
#[qapi(added = "2.1.0")]
pub struct SuspendDiskEvent {}

/// Emitted when guest enters a hardware suspension state, for example, S3 state,
/// which is sometimes called standby state
///
#[qapi(event)]
#[qapi(name = "SUSPEND")]
#[qapi(added = "2.1.0")]
pub struct SuspendEvent {}

#[qapi(event)]
#[qapi(name = "UNPLUG_PRIMARY")]
#[qapi(added = "4.2.0")]
pub struct UnplugPrimaryEvent {
    #[qapi(name = "device-id")]
    pub device_id: String,
}

#[qapi(event)]
#[qapi(name = "VFIO_MIGRATION")]
#[qapi(added = "9.1.0")]
pub struct VfioMigrationEvent {
    #[qapi(name = "device-id")]
    pub device_id: String,
    #[qapi(name = "device-state")]
    pub device_state: VfioMigrationState,
    #[qapi(name = "qom-path")]
    pub qom_path: String,
}

#[qapi(event)]
#[qapi(name = "VFU_CLIENT_HANGUP")]
#[qapi(added = "7.1.0")]
pub struct VfuClientHangupEvent {
    #[qapi(name = "dev-id")]
    pub dev_id: String,
    #[qapi(name = "dev-qom-path")]
    pub dev_qom_path: String,
    #[qapi(name = "vfu-id")]
    pub vfu_id: String,
    #[qapi(name = "vfu-qom-path")]
    pub vfu_qom_path: String,
}

#[qapi(event)]
#[qapi(name = "VNC_CONNECTED")]
#[qapi(added = "2.1.0")]
pub struct VncConnectedEvent {
    pub client: VncBasicInfo,
    pub server: VncServerInfo,
}

/// Emitted when the connection is closed
///
#[qapi(event)]
#[qapi(name = "VNC_DISCONNECTED")]
#[qapi(added = "2.1.0")]
pub struct VncDisconnectedEvent {
    /// client information
    pub client: VncClientInfo,
    /// server information
    pub server: VncServerInfo,
}

/// Emitted after authentication takes place (if any) and the VNC session is
/// made active
///
#[qapi(event)]
#[qapi(name = "VNC_INITIALIZED")]
#[qapi(added = "2.1.0")]
pub struct VncInitializedEvent {
    /// client information
    pub client: VncClientInfo,
    /// server information
    pub server: VncServerInfo,
}

/// Emitted when the guest opens or closes a virtio-serial port.
///
#[qapi(event)]
#[qapi(name = "VSERPORT_CHANGE")]
#[qapi(added = "2.1.0")]
pub struct VserportChangeEvent {
    /// device identifier of the virtio-serial port
    pub id: String,
    /// true if the guest has opened the virtio-serial port
    pub open: bool,
}

/// Emitted when the guest has woken up from suspend state and is running
///
#[qapi(event)]
#[qapi(name = "WAKEUP")]
#[qapi(added = "2.1.0")]
pub struct WakeupEvent {}

#[qapi(event)]
#[qapi(name = "WATCHDOG")]
#[qapi(added = "2.1.0")]
pub struct WatchdogEvent {
    pub action: WatchdogExpirationAction,
}

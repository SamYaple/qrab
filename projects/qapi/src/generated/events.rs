#![allow(unused_imports, non_camel_case_types, dead_code)]
use crate as qapi;
use qapi_macros::qapi;
use std::collections::HashMap;
use super::*;
/// Emitted when guest executes ACPI _OST method.
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "ACPI_DEVICE_OST",
///          "data": { "info": { "device": "d1", "slot": "0",
///                              "slot-type": "DIMM", "source": 1, "status": 0 } },
///          "timestamp": { "seconds": 1265044230, "microseconds": 450486 } }
/// ```
#[qapi(event)]
#[qapi(name = "ACPI_DEVICE_OST")]
#[qapi(added = "2.1.0")]
pub struct AcpiDeviceOstEvent {
    /// OSPM Status Indication
    pub info: AcpiostInfo,
}

/// Emitted when the guest changes the actual BALLOON level.  This value
/// is equivalent to the @actual field return by the `query-balloon`
/// command
///
/// .. note:: This event is rate-limited.
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "BALLOON_CHANGE",
///          "data": { "actual": 944766976 },
///          "timestamp": { "seconds": 1267020223, "microseconds": 435656 } }
/// ```
#[qapi(event)]
#[qapi(name = "BALLOON_CHANGE")]
#[qapi(added = "2.1.0")]
pub struct BalloonChangeEvent {
    /// the logical size of the VM in bytes.  Formula used:
    /// logical_vm_size = vm_ram_size - balloon_size
    pub actual: i64,
}

/// Emitted when a block export is removed and its id can be reused.
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
/// .. note:: If action is "stop", a `STOP` event will eventually follow
/// ```text
///    the `BLOCK_IO_ERROR` event.
/// ```
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "BLOCK_IMAGE_CORRUPTED",
///          "data": { "device": "", "node-name": "drive", "fatal": false,
///                    "msg": "L2 table offset 0x2a2a2a00 unaligned (L1 index: 0)" },
///          "timestamp": { "seconds": 1648243240, "microseconds": 906060 } }
/// ```
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
    #[qapi(added = "2.2.0")]
    #[qapi(required)]
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

/// Emitted when a disk I/O error occurs
///
/// .. note:: If action is "stop", a `STOP` event will eventually follow
/// ```text
///    the `BLOCK_IO_ERROR` event.
/// ```
///
/// .. note:: This event is rate-limited, except if action is "stop".
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "BLOCK_IO_ERROR",
///          "data": { "qom-path": "/machine/unattached/device[0]",
///                    "device": "ide0-hd1",
///                    "node-name": "#block212",
///                    "operation": "write",
///                    "action": "stop",
///                    "reason": "No space left on device" },
///          "timestamp": { "seconds": 1265044230, "microseconds": 450486 } }
/// ```
#[qapi(event)]
#[qapi(name = "BLOCK_IO_ERROR")]
#[qapi(added = "2.1.0")]
pub struct BlockIoErrorEvent {
    /// action that has been taken
    pub action: BlockErrorAction,
    /// device name.  This is always present for compatibility
    /// reasons, but it can be empty ("") if the image does not have a
    /// device name associated.
    pub device: String,
    /// node name.  Note that errors may be reported for the
    /// root node that is directly attached to a guest device rather
    /// than for the node where the error occurred.  The node name is
    /// not present if the drive is empty.  (Since: 2.8)
    #[qapi(name = "node-name")]
    #[qapi(added = "2.12.0")]
    pub node_name: Option<String>,
    /// node name. Note that errors may be reported for the root node
    /// that is directly attached to a guest device rather than for the
    /// node where the error occurred. (Since: 2.8)
    #[qapi(name = "node-name")]
    #[qapi(added = "2.8.0")]
    #[qapi(removed = "2.12.0")]
    #[qapi(required)]
    pub node_name_legacy: Option<String>,
    /// true if I/O error was caused due to a no-space condition.
    /// This key is only present if query-block's io-status is present,
    /// please see `query-block` documentation for more information
    /// (since: 2.2)
    #[qapi(added = "2.2.0")]
    pub nospace: Option<bool>,
    /// I/O operation
    pub operation: IoOperationType,
    /// path to the device object in the QOM tree (since 9.2)
    #[qapi(name = "qom-path")]
    #[qapi(added = "9.2.0")]
    #[qapi(required)]
    pub qom_path: Option<String>,
    /// human readable string describing the error cause.  (This
    /// field is a debugging aid for humans, it should not be parsed by
    /// applications) (since: 2.2)
    #[qapi(added = "2.2.0")]
    #[qapi(required)]
    pub reason: Option<String>,
}

/// Emitted when a block job has been cancelled
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "BLOCK_JOB_CANCELLED",
///          "data": { "type": "stream", "device": "virtio-disk0",
///                    "len": 10737418240, "offset": 134217728,
///                    "speed": 0 },
///          "timestamp": { "seconds": 1267061043, "microseconds": 959568 } }
/// ```
#[qapi(event)]
#[qapi(name = "BLOCK_JOB_CANCELLED")]
#[qapi(added = "2.1.0")]
pub struct BlockJobCancelledEvent {
    /// The job identifier.  Originally the device name but other
    /// values are allowed since QEMU 2.7
    pub device: String,
    /// maximum progress value
    pub len: i64,
    /// current progress value.  On success this is equal to len.
    /// On failure this is less than len
    pub offset: i64,
    /// rate limit, bytes per second
    pub speed: i64,
    /// job type
    #[qapi(added = "3.0.0")]
    #[qapi(required)]
    pub r#type: Option<JobType>,
    /// job type
    #[qapi(name = "type")]
    #[qapi(removed = "3.0.0")]
    #[qapi(required)]
    pub type_legacy: Option<BlockJobType>,
}

/// Emitted when a block job has completed
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "BLOCK_JOB_COMPLETED",
///          "data": { "type": "stream", "device": "virtio-disk0",
///                    "len": 10737418240, "offset": 10737418240,
///                    "speed": 0 },
///          "timestamp": { "seconds": 1267061043, "microseconds": 959568 } }
/// ```
#[qapi(event)]
#[qapi(name = "BLOCK_JOB_COMPLETED")]
#[qapi(added = "2.1.0")]
pub struct BlockJobCompletedEvent {
    /// The job identifier.  Originally the device name but other
    /// values are allowed since QEMU 2.7
    pub device: String,
    /// error message.  Only present on failure.  This field
    /// contains a human-readable error message.  There are no semantics
    /// other than that streaming has failed and clients should not try
    /// to interpret the error string
    pub error: Option<String>,
    /// maximum progress value
    pub len: i64,
    /// current progress value.  On success this is equal to len.
    /// On failure this is less than len
    pub offset: i64,
    /// rate limit, bytes per second
    pub speed: i64,
    /// job type
    #[qapi(added = "3.0.0")]
    #[qapi(required)]
    pub r#type: Option<JobType>,
    /// job type
    #[qapi(name = "type")]
    #[qapi(removed = "3.0.0")]
    #[qapi(required)]
    pub type_legacy: Option<BlockJobType>,
}

/// Emitted when a block job encounters an error
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "BLOCK_JOB_ERROR",
///          "data": { "device": "ide0-hd1",
///                    "operation": "write",
///                    "action": "stop" },
///          "timestamp": { "seconds": 1265044230, "microseconds": 450486 } }
/// ```
#[qapi(event)]
#[qapi(name = "BLOCK_JOB_ERROR")]
#[qapi(added = "2.1.0")]
pub struct BlockJobErrorEvent {
    /// action that has been taken
    pub action: BlockErrorAction,
    /// The job identifier.  Originally the device name but other
    /// values are allowed since QEMU 2.7
    pub device: String,
    /// I/O operation
    pub operation: IoOperationType,
}

/// Emitted when a block job is awaiting explicit authorization to
/// finalize graph changes via `job-finalize`.  If this job is part of a
/// transaction, it will not emit this event until the transaction has
/// converged first.
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "BLOCK_JOB_PENDING",
///          "data": { "type": "mirror", "id": "backup_1" },
///          "timestamp": { "seconds": 1265044230, "microseconds": 450486 } }
/// ```
#[qapi(event)]
#[qapi(name = "BLOCK_JOB_PENDING")]
#[qapi(added = "2.12.0")]
pub struct BlockJobPendingEvent {
    /// The job identifier.
    pub id: String,
    /// job type
    #[qapi(added = "3.0.0")]
    #[qapi(required)]
    pub r#type: Option<JobType>,
    /// job type
    #[qapi(name = "type")]
    #[qapi(removed = "3.0.0")]
    #[qapi(required)]
    pub type_legacy: Option<BlockJobType>,
}

/// Emitted when a block job is ready to complete
///
/// .. note:: The "ready to complete" status is always reset by a
/// ```text
///    `BLOCK_JOB_ERROR` event.
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "BLOCK_JOB_READY",
///          "data": { "device": "drive0", "type": "mirror", "speed": 0,
///                    "len": 2097152, "offset": 2097152 },
///          "timestamp": { "seconds": 1265044230, "microseconds": 450486 } }
/// ```
#[qapi(event)]
#[qapi(name = "BLOCK_JOB_READY")]
#[qapi(added = "2.1.0")]
pub struct BlockJobReadyEvent {
    /// The job identifier.  Originally the device name but other
    /// values are allowed since QEMU 2.7
    pub device: String,
    /// maximum progress value
    pub len: i64,
    /// current progress value.  On success this is equal to len.
    /// On failure this is less than len
    pub offset: i64,
    /// rate limit, bytes per second
    pub speed: i64,
    /// job type
    #[qapi(added = "3.0.0")]
    #[qapi(required)]
    pub r#type: Option<JobType>,
    /// job type
    #[qapi(name = "type")]
    #[qapi(removed = "3.0.0")]
    #[qapi(required)]
    pub type_legacy: Option<BlockJobType>,
}

/// Emitted when writes on block device reaches or exceeds the
/// configured write threshold.  For thin-provisioned devices, this
/// means the device should be extended to avoid pausing for disk
/// exhaustion.  The event is one shot.  Once triggered, it needs to be
/// re-registered with another `block-set-write-threshold` command.
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

/// Emitted when VM finishes COLO mode due to some errors happening or
/// at the request of users.
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "timestamp": {"seconds": 2032141960, "microseconds": 417172},
///          "event": "COLO_EXIT", "data": {"mode": "primary", "reason": "request" } }
/// ```
#[qapi(event)]
#[qapi(name = "COLO_EXIT")]
#[qapi(added = "3.1.0")]
pub struct ColoExitEvent {
    /// report COLO mode when COLO exited.
    pub mode: ColoMode,
    /// describes the reason for the COLO exit.
    pub reason: ColoExitReason,
}

/// Emitted when a command is dropped due to some reason.  Commands can
/// only be dropped when the oob capability is enabled.
///
///
/// Example:
///
/// { "event": "COMMAND_DROPPED",
///   "data": {"result": {"id": "libvirt-102",
/// ```text
///                       "reason": "queue-full" } } }
/// ```
#[qapi(event)]
#[qapi(name = "COMMAND_DROPPED")]
#[qapi(added = "2.12.0")]
#[qapi(removed = "4.0.0")]
pub struct CommandDroppedEvent {
    /// The dropped command's "id" field.
    /// FIXME Broken by design.  Events are broadcast to all monitors.  If
    /// another monitor's client has a command with the same ID in flight,
    /// the event will incorrectly claim that command was dropped.
    pub id: serde_json::Value,
    /// The reason why the command is dropped.
    pub reason: CommandDropReason,
}

/// Emitted when the guest asks to change the polarization.
///
/// The guest can tell the host (via the PTF instruction) whether the
/// CPUs should be provisioned using horizontal or vertical
/// polarization.
///
/// On horizontal polarization the host is expected to provision all
/// vCPUs equally.
///
/// On vertical polarization the host can provision each vCPU
/// differently.  The guest will get information on the details of the
/// provisioning the next time it uses the STSI(15) instruction.
///
/// Features:
///
/// @unstable: This event is experimental.
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "CPU_POLARIZATION_CHANGE",
///          "data": { "polarization": "horizontal" },
///          "timestamp": { "seconds": 1401385907, "microseconds": 422329 } }
/// ```
#[qapi(event)]
#[qapi(name = "CPU_POLARIZATION_CHANGE")]
#[qapi(added = "8.2.0")]
#[qapi(unstable(added = "8.2.0"))]
pub struct CpuPolarizationChangeEvent {
    /// polarization specified by the guest
    #[qapi(added = "9.2.0")]
    #[qapi(required)]
    pub polarization: Option<S390CpuPolarization>,
    /// polarization specified by the guest
    #[qapi(name = "polarization")]
    #[qapi(removed = "9.2.0")]
    #[qapi(required)]
    pub polarization_legacy: Option<CpuS390Polarization>,
}

/// Emitted whenever the device removal completion is acknowledged by
/// the guest.  At this point, it's safe to reuse the specified device
/// ID.  Device removal can be initiated by the guest or by HMP/QMP
/// commands.
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "DEVICE_DELETED",
///          "data": { "device": "virtio-net-pci-0",
///                    "path": "/machine/peripheral/virtio-net-pci-0" },
///          "timestamp": { "seconds": 1265044230, "microseconds": 450486 } }
/// ```
#[qapi(event)]
#[qapi(name = "DEVICE_DELETED")]
#[qapi(added = "2.1.0")]
pub struct DeviceDeletedEvent {
    /// the device's ID if it has one
    pub device: Option<String>,
    /// the device's QOM path
    pub path: String,
}

/// Emitted whenever the tray of a removable device is moved by the
/// guest or by HMP/QMP commands
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "DEVICE_TRAY_MOVED",
///          "data": { "device": "ide1-cd0",
///                    "id": "/machine/unattached/device[22]",
///                    "tray-open": true
///          },
///          "timestamp": { "seconds": 1265044230, "microseconds": 450486 } }
/// ```
#[qapi(event)]
#[qapi(name = "DEVICE_TRAY_MOVED")]
#[qapi(added = "2.1.0")]
pub struct DeviceTrayMovedEvent {
    /// Block device name.  This is always present for
    /// compatibility reasons, but it can be empty ("") if the image
    /// does not have a device name associated.
    pub device: String,
    /// The name or QOM path of the guest device (since 2.8)
    #[qapi(added = "2.8.0")]
    #[qapi(required)]
    pub id: Option<String>,
    /// true if the tray has been opened or false if it has been
    /// closed
    #[qapi(name = "tray-open")]
    pub tray_open: bool,
}

/// Emitted when a device hot unplug fails due to a guest reported
/// error.
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "DEVICE_UNPLUG_GUEST_ERROR",
///          "data": { "device": "core1",
///                    "path": "/machine/peripheral/core1" },
///          "timestamp": { "seconds": 1615570772, "microseconds": 202844 } }
/// ```
#[qapi(event)]
#[qapi(name = "DEVICE_UNPLUG_GUEST_ERROR")]
#[qapi(added = "6.2.0")]
pub struct DeviceUnplugGuestErrorEvent {
    /// the device's ID if it has one
    pub device: Option<String>,
    /// the device's QOM path
    pub path: String,
}

/// Emitted when background dump has completed
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "DUMP_COMPLETED",
///          "data": { "result": { "total": 1090650112, "status": "completed",
///                                "completed": 1090650112 } },
///          "timestamp": { "seconds": 1648244171, "microseconds": 950316 } }
/// ```
#[qapi(event)]
#[qapi(name = "DUMP_COMPLETED")]
#[qapi(added = "2.6.0")]
pub struct DumpCompletedEvent {
    /// human-readable error string that provides hint on why dump
    /// failed.  Only presents on failure.  The user should not try to
    /// interpret the error string.
    pub error: Option<String>,
    /// final dump status
    pub result: DumpQueryResult,
}

/// Emitted when VIRTIO_NET_F_STANDBY was enabled during feature
/// negotiation.  Failover primary devices which were hidden (not
/// hotplugged when requested) before will now be hotplugged by the
/// virtio-net standby device.
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "FAILOVER_NEGOTIATED",
///          "data": { "device-id": "net1" },
///          "timestamp": { "seconds": 1368697518, "microseconds": 326866 } }
/// ```
#[qapi(event)]
#[qapi(name = "FAILOVER_NEGOTIATED")]
#[qapi(added = "4.2.0")]
pub struct FailoverNegotiatedEvent {
    /// QEMU device id of the unplugged device
    #[qapi(name = "device-id")]
    pub device_id: String,
}

/// Emitted when guest OS crash loaded is detected
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "GUEST_CRASHLOADED",
///          "data": { "action": "run" },
///          "timestamp": { "seconds": 1648245259, "microseconds": 893771 } }
/// ```
#[qapi(event)]
#[qapi(name = "GUEST_CRASHLOADED")]
#[qapi(added = "5.0.0")]
pub struct GuestCrashloadedEvent {
    /// action that has been taken, currently always "run"
    pub action: GuestPanicAction,
    /// information about a panic
    pub info: Option<GuestPanicInformation>,
}

/// Emitted when guest OS panic is detected
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "GUEST_PANICKED",
///          "data": { "action": "pause" },
///          "timestamp": { "seconds": 1648245231, "microseconds": 900001 } }
/// ```
#[qapi(event)]
#[qapi(name = "GUEST_PANICKED")]
#[qapi(added = "2.1.0")]
pub struct GuestPanickedEvent {
    /// action that has been taken, currently always "pause"
    pub action: GuestPanicAction,
    /// information about a panic (since 2.9)
    #[qapi(added = "2.9.0")]
    pub info: Option<GuestPanicInformation>,
}

/// Emitted when guest submits a shutdown request via pvpanic interface
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "GUEST_PVSHUTDOWN",
///          "timestamp": { "seconds": 1648245259, "microseconds": 893771 } }
/// ```
#[qapi(event)]
#[qapi(name = "GUEST_PVSHUTDOWN")]
#[qapi(added = "9.1.0")]
pub struct GuestPvshutdownEvent {}

/// Emitted when the hv-balloon driver receives a "STATUS" message from
/// the guest.
///
/// .. note:: This event is rate-limited.
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "HV_BALLOON_STATUS_REPORT",
///          "data": { "committed": 816640000, "available": 3333054464 },
///          "timestamp": { "seconds": 1600295492, "microseconds": 661044 } }
/// ```
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
#[qapi(event)]
#[qapi(name = "JOB_STATUS_CHANGE")]
#[qapi(added = "3.0.0")]
pub struct JobStatusChangeEvent {
    /// The job identifier
    pub id: String,
    /// The new job status
    pub status: JobStatus,
}

/// Emitted when the size of a memory device changes.  Only emitted for
/// memory devices that can actually change the size (e.g., virtio-mem
/// due to guest action).
///
/// .. note:: This event is rate-limited.
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "MEMORY_DEVICE_SIZE_CHANGE",
///          "data": { "id": "vm0", "size": 1073741824,
///                    "qom-path": "/machine/unattached/device[2]" },
///          "timestamp": { "seconds": 1588168529, "microseconds": 201316 } }
/// ```
#[qapi(event)]
#[qapi(name = "MEMORY_DEVICE_SIZE_CHANGE")]
#[qapi(added = "5.1.0")]
pub struct MemoryDeviceSizeChangeEvent {
    /// device's ID
    pub id: Option<String>,
    /// path to the device object in the QOM tree (since 6.2)
    #[qapi(name = "qom-path")]
    #[qapi(added = "6.2.0")]
    #[qapi(required)]
    pub qom_path: Option<String>,
    /// the new size of memory that the device provides
    pub size: u64,
}

/// Emitted when a memory failure occurs on host side.
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "MEMORY_FAILURE",
///          "data": { "recipient": "hypervisor",
///                    "action": "fatal",
///                    "flags": { "action-required": false,
///                               "recursive": false } },
///          "timestamp": { "seconds": 1267061043, "microseconds": 959568 } }
/// ```
#[qapi(event)]
#[qapi(name = "MEMORY_FAILURE")]
#[qapi(added = "5.2.0")]
pub struct MemoryFailureEvent {
    /// action that has been taken.
    pub action: MemoryFailureAction,
    /// flags for `MemoryFailureAction`.
    pub flags: MemoryFailureFlags,
    /// recipient is defined as `MemoryFailureRecipient`.
    pub recipient: MemoryFailureRecipient,
}

/// Emitted when memory hot unplug error occurs.
///
/// Features:
///
/// @deprecated: This event is deprecated.  Use
/// ```text
///     @DEVICE_UNPLUG_GUEST_ERROR instead.
/// ```
///
///
/// Example:
///
/// ```text
///     <- { "event": "MEM_UNPLUG_ERROR",
///          "data": { "device": "dimm1",
///                    "msg": "acpi: device unplug for unsupported device"
///          },
///          "timestamp": { "seconds": 1265044230, "microseconds": 450486 } }
/// ```
#[qapi(event)]
#[qapi(name = "MEM_UNPLUG_ERROR")]
#[qapi(feature = "deprecated")]
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
///
/// .. qmp-example::
///
/// ```text
///     <- {"timestamp": {"seconds": 1432121972, "microseconds": 744001},
///         "event": "MIGRATION",
///         "data": {"status": "completed"} }
/// ```
#[qapi(event)]
#[qapi(name = "MIGRATION")]
#[qapi(added = "2.4.0")]
pub struct MigrationEvent {
    /// `MigrationStatus` describing the current migration status.
    pub status: MigrationStatus,
}

/// Emitted from the source side of a migration at the start of each
/// pass (when it syncs the dirty bitmap)
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "timestamp": {"seconds": 1449669631, "microseconds": 239225},
///           "event": "MIGRATION_PASS", "data": {"pass": 2} }
/// ```
#[qapi(event)]
#[qapi(name = "MIGRATION_PASS")]
#[qapi(added = "2.6.0")]
pub struct MigrationPassEvent {
    /// An incrementing count (starting at 1 on the first pass)
    pub pass: i64,
}

/// Emitted when the netdev stream backend is connected
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "NETDEV_STREAM_CONNECTED",
///          "data": { "netdev-id": "netdev0",
///                    "addr": { "port": "47666", "ipv6": true,
///                              "host": "::1", "type": "inet" } },
///          "timestamp": { "seconds": 1666269863, "microseconds": 311222 } }
/// ```
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "NETDEV_STREAM_CONNECTED",
///          "data": { "netdev-id": "netdev0",
///                    "addr": { "path": "/tmp/qemu0", "type": "unix" } },
///          "timestamp": { "seconds": 1666269706, "microseconds": 413651 } }
/// ```
#[qapi(event)]
#[qapi(name = "NETDEV_STREAM_CONNECTED")]
#[qapi(added = "7.2.0")]
pub struct NetdevStreamConnectedEvent {
    /// The destination address
    pub addr: SocketAddress,
    /// QEMU netdev id that is connected
    #[qapi(name = "netdev-id")]
    pub netdev_id: String,
}

/// Emitted when the netdev stream backend is disconnected
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "NETDEV_STREAM_DISCONNECTED",
///          "data": {"netdev-id": "netdev0"},
///          "timestamp": {"seconds": 1663330937, "microseconds": 526695} }
/// ```
#[qapi(event)]
#[qapi(name = "NETDEV_STREAM_DISCONNECTED")]
#[qapi(added = "7.2.0")]
pub struct NetdevStreamDisconnectedEvent {
    /// QEMU netdev id that is disconnected
    #[qapi(name = "netdev-id")]
    pub netdev_id: String,
}

/// Emitted when the vhost-user chardev is connected
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "timestamp": {"seconds": 1739538638, "microseconds": 354181 },
///          "event": "NETDEV_VHOST_USER_CONNECTED",
///          "data": { "netdev-id": "netdev0", "chardev-id": "chr0" } }
/// ```
#[qapi(event)]
#[qapi(name = "NETDEV_VHOST_USER_CONNECTED")]
#[qapi(added = "10.0.0")]
pub struct NetdevVhostUserConnectedEvent {
    /// The character device id used by the QEMU netdev
    #[qapi(name = "chardev-id")]
    pub chardev_id: String,
    /// QEMU netdev id that is connected
    #[qapi(name = "netdev-id")]
    pub netdev_id: String,
}

/// Emitted when the vhost-user chardev is disconnected
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "timestamp": { "seconds": 1739538634, "microseconds": 920450 },
///          "event": "NETDEV_VHOST_USER_DISCONNECTED",
///          "data": { "netdev-id": "netdev0" } }
/// ```
#[qapi(event)]
#[qapi(name = "NETDEV_VHOST_USER_DISCONNECTED")]
#[qapi(added = "10.0.0")]
pub struct NetdevVhostUserDisconnectedEvent {
    /// QEMU netdev id that is disconnected
    #[qapi(name = "netdev-id")]
    pub netdev_id: String,
}

/// Emitted once until the `query-rx-filter` command is executed, the
/// first event will always be emitted
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "NIC_RX_FILTER_CHANGED",
///          "data": { "name": "vnet0",
///                    "path": "/machine/peripheral/vnet0/virtio-backend" },
///          "timestamp": { "seconds": 1368697518, "microseconds": 326866 } }
/// ```
#[qapi(event)]
#[qapi(name = "NIC_RX_FILTER_CHANGED")]
#[qapi(added = "2.1.0")]
pub struct NicRxFilterChangedEvent {
    /// net client name
    pub name: Option<String>,
    /// device path
    pub path: String,
}

/// Emitted when the virtual machine is powered down through the power
/// control system, such as via ACPI.
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "POWERDOWN",
///          "timestamp": { "seconds": 1267040730, "microseconds": 682951 } }
/// ```
#[qapi(event)]
#[qapi(name = "POWERDOWN")]
#[qapi(added = "2.1.0")]
pub struct PowerdownEvent {}

/// Emitted whenever the connected status of a persistent reservation
/// manager changes.
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "PR_MANAGER_STATUS_CHANGED",
///          "data": { "id": "pr-helper0",
///                    "connected": true
///          },
///          "timestamp": { "seconds": 1519840375, "microseconds": 450486 } }
/// ```
#[qapi(event)]
#[qapi(name = "PR_MANAGER_STATUS_CHANGED")]
#[qapi(added = "3.0.0")]
pub struct PrManagerStatusChangedEvent {
    /// true if the PR manager is connected to a backend
    pub connected: bool,
    /// The id of the PR manager object
    pub id: String,
}

/// Emitted by the Quorum block driver if it fails to establish a quorum
///
/// .. note:: This event is rate-limited.
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "QUORUM_FAILURE",
///          "data": { "reference": "usr1", "sector-num": 345435, "sectors-count": 5 },
///          "timestamp": { "seconds": 1344522075, "microseconds": 745528 } }
/// ```
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
/// .. note:: This event is rate-limited.
///
///
/// .. qmp-example::
/// ```text
///    :title: Read operation
/// ```
///
/// ```text
///     <- { "event": "QUORUM_REPORT_BAD",
///          "data": { "node-name": "node0", "sector-num": 345435, "sectors-count": 5,
///                    "type": "read" },
///          "timestamp": { "seconds": 1344522075, "microseconds": 745528 } }
/// ```
///
/// .. qmp-example::
/// ```text
///    :title: Flush operation
/// ```
///
/// ```text
///     <- { "event": "QUORUM_REPORT_BAD",
///          "data": { "node-name": "node0", "sector-num": 0, "sectors-count": 2097120,
///                    "type": "flush", "error": "Broken pipe" },
///          "timestamp": { "seconds": 1456406829, "microseconds": 291763 } }
/// ```
#[qapi(event)]
#[qapi(name = "QUORUM_REPORT_BAD")]
#[qapi(added = "2.1.0")]
pub struct QuorumReportBadEvent {
    /// error message.  Only present on failure.  This field
    /// contains a human-readable error message.  There are no semantics
    /// other than that the block layer reported an error and clients
    /// should not try to interpret the error string.
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
    #[qapi(required)]
    pub r#type: Option<QuorumOpType>,
}

/// Emitted when guest driver adds/deletes GID to/from device
///
///
/// Example:
///
/// ```text
///     <- {"timestamp": {"seconds": 1541579657, "microseconds": 986760},
///         "event": "RDMA_GID_STATUS_CHANGED",
///         "data":
///             {"netdev": "bridge0",
///             "interface-id": 15880512517475447892,
///             "gid-status": true,
///             "subnet-prefix": 33022}}
/// ```
#[qapi(event)]
#[qapi(name = "RDMA_GID_STATUS_CHANGED")]
#[qapi(added = "4.0.0")]
#[qapi(removed = "9.1.0")]
pub struct RdmaGidStatusChangedEvent {
    /// Add or delete indication
    #[qapi(name = "gid-status")]
    pub gid_status: bool,
    /// Interface ID
    #[qapi(name = "interface-id")]
    pub interface_id: u64,
    /// RoCE Network Device name
    pub netdev: String,
    /// Subnet Prefix
    #[qapi(name = "subnet-prefix")]
    pub subnet_prefix: u64,
}

/// Emitted when the virtual machine is reset
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "RESET",
///          "data": { "guest": false, "reason": "guest-reset" },
///          "timestamp": { "seconds": 1267041653, "microseconds": 9518 } }
/// ```
#[qapi(event)]
#[qapi(name = "RESET")]
#[qapi(added = "2.1.0")]
pub struct ResetEvent {
    /// If true, the reset was triggered by a guest request (such as
    /// a guest-initiated ACPI reboot request or other hardware-specific
    /// action) rather than a host request (such as the QMP command
    /// `system_reset`).  (since 2.10)
    #[qapi(added = "2.10.0")]
    #[qapi(required)]
    pub guest: Option<bool>,
    /// The `ShutdownCause` of the `RESET`.  (since 4.0)
    #[qapi(added = "4.0.0")]
    #[qapi(required)]
    pub reason: Option<ShutdownCause>,
}

/// Emitted when the virtual machine resumes execution
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "RESUME",
///          "timestamp": { "seconds": 1271770767, "microseconds": 582542 } }
/// ```
#[qapi(event)]
#[qapi(name = "RESUME")]
#[qapi(added = "2.1.0")]
pub struct ResumeEvent {}

/// Emitted when the guest changes the RTC time.
///
/// .. note:: This event is rate-limited.  It is not guaranteed that the
/// ```text
///    RTC in the system implements this event, or even that the system
///    has an RTC at all.
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "RTC_CHANGE",
///          "data": { "offset": 78 },
///          "timestamp": { "seconds": 1267020223, "microseconds": 435656 } }
/// ```
#[qapi(event)]
#[qapi(name = "RTC_CHANGE")]
#[qapi(added = "2.1.0")]
pub struct RtcChangeEvent {
    /// offset in seconds between base RTC clock (as specified by
    /// -rtc base), and new RTC clock value
    pub offset: i64,
    /// path to the RTC object in the QOM tree
    #[qapi(name = "qom-path")]
    #[qapi(added = "7.0.0")]
    #[qapi(required)]
    pub qom_path: Option<String>,
}

/// Emitted when the Control-Program Identification data is available in
/// the QOM tree.
///
/// Features:
///
/// @unstable: This event is experimental.
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "SCLP_CPI_INFO_AVAILABLE",
///          "timestamp": { "seconds": 1401385907, "microseconds": 422329 } }
/// ```
#[qapi(event)]
#[qapi(name = "SCLP_CPI_INFO_AVAILABLE")]
#[qapi(added = "10.2.0")]
#[qapi(unstable(added = "10.2.0"))]
pub struct SclpCpiInfoAvailableEvent {}

/// Emitted when the virtual machine has shut down, indicating that QEMU
/// is about to exit.
///
/// .. note:: If the command-line option ``-no-shutdown`` has been
/// ```text
///    specified, QEMU will not exit, and a `STOP` event will eventually
///    follow the `SHUTDOWN` event.
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "SHUTDOWN",
///          "data": { "guest": true, "reason": "guest-shutdown" },
///          "timestamp": { "seconds": 1267040730, "microseconds": 682951 } }
/// ```
#[qapi(event)]
#[qapi(name = "SHUTDOWN")]
#[qapi(added = "2.1.0")]
pub struct ShutdownEvent {
    /// If true, the shutdown was triggered by a guest request (such
    /// as a guest-initiated ACPI shutdown request or other
    /// hardware-specific action) rather than a host request (such as
    /// sending QEMU a SIGINT).  (since 2.10)
    #[qapi(added = "2.10.0")]
    #[qapi(required)]
    pub guest: Option<bool>,
    /// The `ShutdownCause` which resulted in the `SHUTDOWN`.
    /// (since 4.0)
    #[qapi(added = "4.0.0")]
    #[qapi(required)]
    pub reason: Option<ShutdownCause>,
}

/// Emitted when a SPICE client establishes a connection
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "timestamp": {"seconds": 1290688046, "microseconds": 388707},
///          "event": "SPICE_CONNECTED",
///          "data": {
///            "server": { "port": "5920", "family": "ipv4", "host": "127.0.0.1"},
///            "client": {"port": "52873", "family": "ipv4", "host": "127.0.0.1"}
///        }}
/// ```
#[qapi(event)]
#[qapi(name = "SPICE_CONNECTED")]
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(added = "2.1.0")]
pub struct SpiceConnectedEvent {
    /// client information
    pub client: SpiceBasicInfo,
    /// server information
    pub server: SpiceBasicInfo,
}

/// Emitted when the SPICE connection is closed
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "timestamp": {"seconds": 1290688046, "microseconds": 388707},
///          "event": "SPICE_DISCONNECTED",
///          "data": {
///            "server": { "port": "5920", "family": "ipv4", "host": "127.0.0.1"},
///            "client": {"port": "52873", "family": "ipv4", "host": "127.0.0.1"}
///        }}
/// ```
#[qapi(event)]
#[qapi(name = "SPICE_DISCONNECTED")]
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(added = "2.1.0")]
pub struct SpiceDisconnectedEvent {
    /// client information
    pub client: SpiceBasicInfo,
    /// server information
    pub server: SpiceBasicInfo,
}

/// Emitted after initial handshake and authentication takes place (if
/// any) and the SPICE channel is up and running
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "timestamp": {"seconds": 1290688046, "microseconds": 417172},
///          "event": "SPICE_INITIALIZED",
///          "data": {"server": {"auth": "spice", "port": "5921",
///                              "family": "ipv4", "host": "127.0.0.1"},
///                   "client": {"port": "49004", "family": "ipv4", "channel-type": 3,
///                              "connection-id": 1804289383, "host": "127.0.0.1",
///                              "channel-id": 0, "tls": true}
///        }}
/// ```
#[qapi(event)]
#[qapi(name = "SPICE_INITIALIZED")]
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(added = "2.1.0")]
pub struct SpiceInitializedEvent {
    /// client information
    pub client: SpiceChannel,
    /// server information
    pub server: SpiceServerInfo,
}

/// Emitted when SPICE migration has completed
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "timestamp": {"seconds": 1290688046, "microseconds": 417172},
///          "event": "SPICE_MIGRATE_COMPLETED" }
/// ```
#[qapi(event)]
#[qapi(name = "SPICE_MIGRATE_COMPLETED")]
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(added = "2.1.0")]
pub struct SpiceMigrateCompletedEvent {}

/// Emitted when the virtual machine is stopped
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "STOP",
///          "timestamp": { "seconds": 1267041730, "microseconds": 281295 } }
/// ```
#[qapi(event)]
#[qapi(name = "STOP")]
#[qapi(added = "2.1.0")]
pub struct StopEvent {}

/// Emitted when guest enters a hardware suspension state with data
/// saved on disk, for example, S4 state, which is sometimes called
/// hibernate state
///
/// .. note:: QEMU shuts down (similar to event `SHUTDOWN`) when
/// ```text
///    entering this state.
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "SUSPEND_DISK",
///          "timestamp": { "seconds": 1344456160, "microseconds": 309119 } }
/// ```
#[qapi(event)]
#[qapi(name = "SUSPEND_DISK")]
#[qapi(added = "2.1.0")]
pub struct SuspendDiskEvent {}

/// Emitted when guest enters a hardware suspension state, for example,
/// S3 state, which is sometimes called standby state
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "SUSPEND",
///          "timestamp": { "seconds": 1344456160, "microseconds": 309119 } }
/// ```
#[qapi(event)]
#[qapi(name = "SUSPEND")]
#[qapi(added = "2.1.0")]
pub struct SuspendEvent {}

/// Emitted from source side of a migration when migration state is
/// WAIT_UNPLUG.  Device was unplugged by guest operating system.
/// Device resources in QEMU are kept on standby to be able to re-plug
/// it in case of migration failure.
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "UNPLUG_PRIMARY",
///          "data": { "device-id": "hostdev0" },
///          "timestamp": { "seconds": 1265044230, "microseconds": 450486 } }
/// ```
#[qapi(event)]
#[qapi(name = "UNPLUG_PRIMARY")]
#[qapi(added = "4.2.0")]
pub struct UnplugPrimaryEvent {
    /// QEMU device id of the unplugged device
    #[qapi(name = "device-id")]
    pub device_id: String,
}

/// This event is emitted when a VFIO device migration state is changed.
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "timestamp": { "seconds": 1713771323, "microseconds": 212268 },
///          "event": "VFIO_MIGRATION",
///          "data": {
///              "device-id": "vfio_dev1",
///              "qom-path": "/machine/peripheral/vfio_dev1",
///              "device-state": "stop" } }
/// ```
#[qapi(event)]
#[qapi(name = "VFIO_MIGRATION")]
#[qapi(added = "9.1.0")]
pub struct VfioMigrationEvent {
    /// The device's id, if it has one.
    #[qapi(name = "device-id")]
    pub device_id: String,
    /// The new changed device migration state.
    #[qapi(name = "device-state")]
    #[qapi(added = "9.2.0")]
    #[qapi(required)]
    pub device_state: Option<QapiVfioMigrationState>,
    /// The new changed device migration state.
    #[qapi(name = "device-state")]
    #[qapi(removed = "9.2.0")]
    #[qapi(required)]
    pub device_state_legacy: Option<VfioMigrationState>,
    /// The device's QOM path.
    #[qapi(name = "qom-path")]
    pub qom_path: String,
}

/// Emitted when the client of a TYPE_VFIO_USER_SERVER closes the
/// communication channel
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "VFU_CLIENT_HANGUP",
///          "data": { "vfu-id": "vfu1",
///                    "vfu-qom-path": "/objects/vfu1",
///                    "dev-id": "sas1",
///                    "dev-qom-path": "/machine/peripheral/sas1" },
///          "timestamp": { "seconds": 1265044230, "microseconds": 450486 } }
/// ```
#[qapi(event)]
#[qapi(name = "VFU_CLIENT_HANGUP")]
#[qapi(added = "7.1.0")]
pub struct VfuClientHangupEvent {
    /// ID of attached PCI device
    #[qapi(name = "dev-id")]
    pub dev_id: String,
    /// path to attached PCI device in the QOM tree
    #[qapi(name = "dev-qom-path")]
    pub dev_qom_path: String,
    /// ID of the TYPE_VFIO_USER_SERVER object.  It is the last
    /// component of @vfu-qom-path referenced below
    #[qapi(name = "vfu-id")]
    pub vfu_id: String,
    /// path to the TYPE_VFIO_USER_SERVER object in the QOM
    /// tree
    #[qapi(name = "vfu-qom-path")]
    pub vfu_qom_path: String,
}

/// Emitted when a VNC client establishes a connection
///
/// .. note:: This event is emitted before any authentication takes
/// ```text
///    place, thus the authentication ID is not provided.
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "VNC_CONNECTED",
///          "data": {
///                "server": { "auth": "sasl", "family": "ipv4", "websocket": false,
///                            "service": "5901", "host": "0.0.0.0" },
///                "client": { "family": "ipv4", "service": "58425",
///                            "host": "127.0.0.1", "websocket": false } },
///          "timestamp": { "seconds": 1262976601, "microseconds": 975795 } }
/// ```
#[qapi(event)]
#[qapi(name = "VNC_CONNECTED")]
#[qapi(condition = "CONFIG_VNC")]
#[qapi(added = "2.1.0")]
pub struct VncConnectedEvent {
    /// client information
    pub client: VncBasicInfo,
    /// server information
    pub server: VncServerInfo,
}

/// Emitted when the connection is closed
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "VNC_DISCONNECTED",
///          "data": {
///                "server": { "auth": "sasl", "family": "ipv4", "websocket": false,
///                            "service": "5901", "host": "0.0.0.0" },
///                "client": { "family": "ipv4", "service": "58425", "websocket": false,
///                            "host": "127.0.0.1", "sasl_username": "luiz" } },
///          "timestamp": { "seconds": 1262976601, "microseconds": 975795 } }
/// ```
#[qapi(event)]
#[qapi(name = "VNC_DISCONNECTED")]
#[qapi(condition = "CONFIG_VNC")]
#[qapi(added = "2.1.0")]
pub struct VncDisconnectedEvent {
    /// client information
    pub client: VncClientInfo,
    /// server information
    pub server: VncServerInfo,
}

/// Emitted after authentication takes place (if any) and the VNC
/// session is made active
///
///
/// .. qmp-example::
///
/// ```text
///     <-  { "event": "VNC_INITIALIZED",
///           "data": {
///                "server": { "auth": "sasl", "family": "ipv4", "websocket": false,
///                            "service": "5901", "host": "0.0.0.0"},
///                "client": { "family": "ipv4", "service": "46089", "websocket": false,
///                            "host": "127.0.0.1", "sasl_username": "luiz" } },
///           "timestamp": { "seconds": 1263475302, "microseconds": 150772 } }
/// ```
#[qapi(event)]
#[qapi(name = "VNC_INITIALIZED")]
#[qapi(condition = "CONFIG_VNC")]
#[qapi(added = "2.1.0")]
pub struct VncInitializedEvent {
    /// client information
    pub client: VncClientInfo,
    /// server information
    pub server: VncServerInfo,
}

/// Emitted when the guest opens or closes a virtio-serial port.
///
/// .. note:: This event is rate-limited.
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "VSERPORT_CHANGE",
///          "data": { "id": "channel0", "open": true },
///          "timestamp": { "seconds": 1401385907, "microseconds": 422329 } }
/// ```
#[qapi(event)]
#[qapi(name = "VSERPORT_CHANGE")]
#[qapi(added = "2.1.0")]
pub struct VserportChangeEvent {
    /// device identifier of the virtio-serial port
    pub id: String,
    /// true if the guest has opened the virtio-serial port
    pub open: bool,
}

/// Emitted when the guest has woken up from suspend state and is
/// running
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "WAKEUP",
///          "timestamp": { "seconds": 1344522075, "microseconds": 745528 } }
/// ```
#[qapi(event)]
#[qapi(name = "WAKEUP")]
#[qapi(added = "2.1.0")]
pub struct WakeupEvent {}

/// Emitted when the watchdog device's timer is expired
///
/// .. note:: If action is "reset", "shutdown", or "pause" the
/// ```text
///    `WATCHDOG` event is followed respectively by the `RESET`,
///    `SHUTDOWN`, or `STOP` events.
/// ```
///
/// .. note:: This event is rate-limited.
///
///
/// .. qmp-example::
///
/// ```text
///     <- { "event": "WATCHDOG",
///          "data": { "action": "reset" },
///          "timestamp": { "seconds": 1267061043, "microseconds": 959568 } }
/// ```
#[qapi(event)]
#[qapi(name = "WATCHDOG")]
#[qapi(added = "2.1.0")]
pub struct WatchdogEvent {
    /// action that has been taken
    #[qapi(added = "2.11.0")]
    #[qapi(required)]
    pub action: Option<WatchdogAction>,
    /// action that has been taken
    #[qapi(name = "action")]
    #[qapi(removed = "2.11.0")]
    #[qapi(required)]
    pub action_legacy: Option<WatchdogExpirationAction>,
}

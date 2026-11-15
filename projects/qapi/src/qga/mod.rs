#![allow(unused_imports, non_camel_case_types, dead_code)]
use crate as qapi;
use qapi_macros::qapi;
use std::collections::HashMap;
/// Information about guest agent commands.
#[qapi(added = "1.4.0")]
pub struct GuestAgentCommandInfo {
    /// whether command is currently enabled by guest admin
    pub enabled: bool,
    /// name of the command
    pub name: String,
    /// whether command returns a response on success
    /// (since 1.7)
    #[qapi(name = "success-response")]
    #[qapi(added = "1.7.0")]
    #[qapi(required)]
    pub success_response: Option<bool>,
}

/// Information about guest agent.
#[qapi(added = "1.4.0")]
pub struct GuestAgentInfo {
    /// Information about guest agent commands
    pub supported_commands: Vec<GuestAgentCommandInfo>,
    /// guest agent version
    pub version: String,
}

#[qapi(condition = "defined(CONFIG_POSIX)")]
#[qapi(added = "5.2.0")]
pub struct GuestAuthorizedKeys {
    /// public keys (in OpenSSH/sshd(8) authorized_keys format)
    pub keys: Vec<String>,
}

#[qapi(name = "GuestCCWAddress")]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_LINUX)")]
#[qapi(added = "6.0.0")]
pub struct GuestCcwAddress {
    /// channel subsystem image id
    pub cssid: i64,
    /// device number
    pub devno: i64,
    /// subchannel set id
    pub ssid: i64,
    /// subchannel number
    pub subchno: i64,
}

/// Get statistics of each CPU in millisecond.
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "7.1.0")]
pub struct GuestCpuStats {
    /// guest operating system
    #[qapi(discriminator)]
    pub r#type: GuestCpuStatsType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<GuestCpuStatsBranch>,
}

#[qapi(union_branch)]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "7.1.0")]
pub enum GuestCpuStatsBranch {
    #[qapi(name = "linux")]
    Linux(GuestLinuxCpuStats),
}

/// Guest operating systems supporting CPU statistics
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "7.1.0")]
pub enum GuestCpuStatsType {
    /// Linux
    #[qapi(name = "linux")]
    Linux,
}

/// Id of the device
#[qapi(condition = "CONFIG_WIN32")]
#[qapi(added = "5.2.0")]
pub struct GuestDeviceId {
    /// device type
    #[qapi(discriminator)]
    pub r#type: GuestDeviceType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<GuestDeviceIdBranch>,
}

#[qapi(union_branch)]
#[qapi(condition = "CONFIG_WIN32")]
#[qapi(added = "5.2.0")]
pub enum GuestDeviceIdBranch {
    #[qapi(name = "pci")]
    Pci(GuestDeviceIdPci),
}

#[qapi(name = "GuestDeviceIdPCI")]
#[qapi(condition = "CONFIG_WIN32")]
#[qapi(added = "5.2.0")]
pub struct GuestDeviceIdPci {
    /// device ID
    #[qapi(name = "device-id")]
    pub device_id: u16,
    /// vendor ID
    #[qapi(name = "vendor-id")]
    pub vendor_id: u16,
}

#[qapi(condition = "CONFIG_WIN32")]
#[qapi(added = "5.2.0")]
pub struct GuestDeviceInfo {
    /// driver release date, in nanoseconds since the epoch
    #[qapi(name = "driver-date")]
    pub driver_date: Option<i64>,
    /// name of the associated driver
    #[qapi(name = "driver-name")]
    pub driver_name: String,
    /// driver version
    #[qapi(name = "driver-version")]
    pub driver_version: Option<String>,
    /// device ID
    pub id: Option<GuestDeviceId>,
}

#[qapi(condition = "CONFIG_WIN32")]
#[qapi(added = "5.2.0")]
pub enum GuestDeviceType {
    /// PCI device
    #[qapi(name = "pci")]
    Pci,
}

#[qapi(condition = "(CONFIG_WIN32 || CONFIG_LINUX)")]
#[qapi(added = "2.2.0")]
pub struct GuestDiskAddress {
    /// bus id
    pub bus: i64,
    /// bus type
    #[qapi(name = "bus-type")]
    pub bus_type: GuestDiskBusType,
    /// CCW address on s390x (since: 6.0)
    #[qapi(name = "ccw-address")]
    #[qapi(added = "6.0.0")]
    pub ccw_address: Option<GuestCcwAddress>,
    /// device node (POSIX) or device UNC (Windows) (since: 3.1)
    #[qapi(added = "3.1.0")]
    pub dev: Option<String>,
    /// controller's PCI address (fields are set to -1 if
    /// invalid)
    #[qapi(name = "pci-controller")]
    pub pci_controller: GuestPciAddress,
    /// serial number (since: 3.1)
    #[qapi(added = "3.1.0")]
    pub serial: Option<String>,
    /// target id
    pub target: i64,
    /// unit id
    pub unit: i64,
}

/// An enumeration of bus type of disks
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_LINUX)")]
#[qapi(added = "2.2.0")]
pub enum GuestDiskBusType {
    /// floppy disks
    #[qapi(name = "fdc")]
    Fdc,
    /// Win fiber channel bus type
    #[qapi(name = "fibre")]
    #[qapi(added = "2.4.0")]
    Fibre,
    /// Win file-backed bus type
    #[qapi(name = "file-backed-virtual")]
    #[qapi(added = "2.4.0")]
    FileBackedVirtual,
    /// IDE disks
    #[qapi(name = "ide")]
    Ide,
    /// Win IEEE 1394 bus type
    #[qapi(name = "ieee1394")]
    #[qapi(added = "2.4.0")]
    Ieee1394,
    /// Win iScsi bus type
    #[qapi(name = "iscsi")]
    #[qapi(added = "2.4.0")]
    Iscsi,
    /// Win multimedia card (MMC) bus type
    #[qapi(name = "mmc")]
    #[qapi(added = "2.4.0")]
    Mmc,
    /// NVMe disks (since 7.1)
    #[qapi(name = "nvme")]
    #[qapi(added = "7.1.0")]
    Nvme,
    /// Win RAID bus type
    #[qapi(name = "raid")]
    #[qapi(added = "2.4.0")]
    Raid,
    /// Win serial-attaches SCSI bus type
    #[qapi(name = "sas")]
    #[qapi(added = "2.4.0")]
    Sas,
    /// SATA disks
    #[qapi(name = "sata")]
    Sata,
    /// SCSI disks
    #[qapi(name = "scsi")]
    Scsi,
    /// SD cards
    #[qapi(name = "sd")]
    Sd,
    /// Win SSA bus type
    #[qapi(name = "ssa")]
    #[qapi(added = "2.4.0")]
    Ssa,
    /// UML disks
    #[qapi(name = "uml")]
    Uml,
    /// Unknown bus type
    #[qapi(name = "unknown")]
    #[qapi(added = "2.4.0")]
    Unknown,
    /// USB disks
    #[qapi(name = "usb")]
    Usb,
    /// virtio disks
    #[qapi(name = "virtio")]
    Virtio,
    /// Win virtual bus type
    #[qapi(name = "virtual")]
    #[qapi(added = "2.4.0")]
    Virtual,
    /// Xen disks
    #[qapi(name = "xen")]
    Xen,
}

/// Since 5.2
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_LIBUDEV)")]
#[qapi(added = "5.2.0")]
pub struct GuestDiskInfo {
    /// disk address information (only for non-virtual devices)
    pub address: Option<GuestDiskAddress>,
    /// optional alias assigned to the disk, on Linux this is a name
    /// assigned by device mapper
    pub alias: Option<String>,
    /// list of device dependencies; e.g. for LVs of the LVM
    /// this will hold the list of PVs, for LUKS encrypted volume this
    /// will contain the disk where the volume is placed.  (Linux)
    pub dependencies: Option<Vec<String>>,
    /// device node (Linux) or device UNC (Windows)
    pub name: String,
    /// whether this is a partition or disk
    pub partition: bool,
    /// disk smart information (Since 7.1)
    #[qapi(added = "7.1.0")]
    pub smart: Option<GuestDiskSmart>,
}

/// Disk type related smart information.
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_LIBUDEV)")]
#[qapi(added = "7.1.0")]
pub struct GuestDiskSmart {
    /// disk bus type
    #[qapi(discriminator)]
    pub r#type: GuestDiskBusType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<GuestDiskSmartBranch>,
}

#[qapi(union_branch)]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_LIBUDEV)")]
#[qapi(added = "7.1.0")]
pub enum GuestDiskSmartBranch {
    /// floppy disks
    #[qapi(name = "fdc")]
    Fdc,
    /// Win fiber channel bus type
    #[qapi(name = "fibre")]
    Fibre,
    /// Win file-backed bus type
    #[qapi(name = "file-backed-virtual")]
    FileBackedVirtual,
    /// IDE disks
    #[qapi(name = "ide")]
    Ide,
    /// Win IEEE 1394 bus type
    #[qapi(name = "ieee1394")]
    Ieee1394,
    /// Win iScsi bus type
    #[qapi(name = "iscsi")]
    Iscsi,
    /// Win multimedia card (MMC) bus type
    #[qapi(name = "mmc")]
    Mmc,
    #[qapi(name = "nvme")]
    Nvme(GuestNvMeSmart),
    /// Win RAID bus type
    #[qapi(name = "raid")]
    Raid,
    /// Win serial-attaches SCSI bus type
    #[qapi(name = "sas")]
    Sas,
    /// SATA disks
    #[qapi(name = "sata")]
    Sata,
    /// SCSI disks
    #[qapi(name = "scsi")]
    Scsi,
    /// SD cards
    #[qapi(name = "sd")]
    Sd,
    /// Win SSA bus type
    #[qapi(name = "ssa")]
    Ssa,
    /// UML disks
    #[qapi(name = "uml")]
    Uml,
    /// Unknown bus type
    #[qapi(name = "unknown")]
    Unknown,
    /// USB disks
    #[qapi(name = "usb")]
    Usb,
    /// virtio disks
    #[qapi(name = "virtio")]
    Virtio,
    /// Win virtual bus type
    #[qapi(name = "virtual")]
    Virtual,
    /// Xen disks
    #[qapi(name = "xen")]
    Xen,
}

#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "7.1.0")]
pub struct GuestDiskStats {
    /// discards completed successfully
    #[qapi(name = "discard-ios")]
    pub discard_ios: Option<u64>,
    /// discard requests merged
    #[qapi(name = "discard-merges")]
    pub discard_merges: Option<u64>,
    /// sectors discarded
    #[qapi(name = "discard-sectors")]
    pub discard_sectors: Option<u64>,
    /// time spent discarding(ms)
    #[qapi(name = "discard-ticks")]
    pub discard_ticks: Option<u64>,
    /// flush requests completed successfully
    #[qapi(name = "flush-ios")]
    pub flush_ios: Option<u64>,
    /// time spent flushing(ms)
    #[qapi(name = "flush-ticks")]
    pub flush_ticks: Option<u64>,
    /// number of I/Os currently in flight
    #[qapi(name = "ios-pgr")]
    pub ios_pgr: Option<u64>,
    /// reads completed successfully
    #[qapi(name = "read-ios")]
    pub read_ios: Option<u64>,
    /// read requests merged
    #[qapi(name = "read-merges")]
    pub read_merges: Option<u64>,
    /// sectors read
    #[qapi(name = "read-sectors")]
    pub read_sectors: Option<u64>,
    /// time spent reading(ms)
    #[qapi(name = "read-ticks")]
    pub read_ticks: Option<u64>,
    /// time spent doing I/Os (ms)
    #[qapi(name = "total-ticks")]
    pub total_ticks: Option<u64>,
    /// weighted time spent doing I/Os since the last update
    /// of this field(ms)
    #[qapi(name = "weight-ticks")]
    pub weight_ticks: Option<u64>,
    /// writes completed
    #[qapi(name = "write-ios")]
    pub write_ios: Option<u64>,
    /// write requests merged
    #[qapi(name = "write-merges")]
    pub write_merges: Option<u64>,
    /// sectors written
    #[qapi(name = "write-sectors")]
    pub write_sectors: Option<u64>,
    /// time spent writing(ms)
    #[qapi(name = "write-ticks")]
    pub write_ticks: Option<u64>,
}

/// @name disk name
///
/// @major major device number of disk
///
/// @minor minor device number of disk
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "7.1.0")]
pub struct GuestDiskStatsInfo {
    /// major device number of disk
    pub major: u64,
    /// minor device number of disk
    pub minor: u64,
    /// disk name
    pub name: String,
    /// I/O statistics
    pub stats: GuestDiskStats,
}

#[qapi(added = "2.5.0")]
pub struct GuestExec {
    /// pid of child process in guest OS
    pub pid: i64,
}

/// Controls what `guest-exec` output gets captures.
#[qapi(added = "8.1.0")]
pub enum GuestExecCaptureOutput {
    /// captures both stdout and stderr if true.  Equivalent to
    /// `GuestExecCaptureOutputMode`::all.  (since 2.5)
    #[qapi(name = "flag")]
    Flag(bool),
    /// capture mode; preferred interface
    #[qapi(name = "mode")]
    Mode(GuestExecCaptureOutputMode),
}

/// An enumeration of `guest-exec` capture modes.
#[qapi(added = "8.1.0")]
pub enum GuestExecCaptureOutputMode {
    /// capture both stdout and stderr, but merge together into
    /// out-data.  Not effective on windows guests.
    #[qapi(name = "merged")]
    #[qapi(condition = "!CONFIG_WIN32")]
    Merged,
    /// do not capture any output
    #[qapi(name = "none")]
    None,
    /// capture both stdout and stderr, but separated into
    /// `GuestExecStatus` out-data and err-data, respectively
    #[qapi(name = "separated")]
    Separated,
    /// only capture stderr
    #[qapi(name = "stderr")]
    Stderr,
    /// only capture stdout
    #[qapi(name = "stdout")]
    Stdout,
}

#[qapi(added = "2.5.0")]
pub struct GuestExecStatus {
    /// base64-encoded stderr of the process.  Note: @out-data
    /// and @err-data are present only if 'capture-output' was specified
    /// for `guest-exec`.  This field will only be populated after the
    /// process exits.
    #[qapi(name = "err-data")]
    pub err_data: Option<String>,
    /// true if stderr was not fully captured due to size
    /// limitation.
    #[qapi(name = "err-truncated")]
    pub err_truncated: Option<bool>,
    /// process exit code if it was normally terminated.
    pub exitcode: Option<i64>,
    /// true if process has already terminated.
    pub exited: bool,
    /// base64-encoded stdout of the process.  This field will
    /// only be populated after the process exits.
    #[qapi(name = "out-data")]
    pub out_data: Option<String>,
    /// true if stdout was not fully captured due to size
    /// limitation.
    #[qapi(name = "out-truncated")]
    pub out_truncated: Option<bool>,
    /// signal number (linux) or unhandled exception code (windows)
    /// if the process was abnormally terminated.
    pub signal: Option<i64>,
}

/// Result of guest agent file-read operation
#[qapi(added = "1.4.0")]
pub struct GuestFileRead {
    /// base64-encoded bytes read
    #[qapi(name = "buf-b64")]
    pub buf_b64: String,
    /// number of bytes read (note: count is *before*
    /// base64-encoding is applied)
    pub count: i64,
    /// whether EOF was encountered during read operation.
    pub eof: bool,
}

/// Result of guest agent file-seek operation
#[qapi(added = "1.4.0")]
pub struct GuestFileSeek {
    /// whether EOF was encountered during file seek
    pub eof: bool,
    /// current file position
    pub position: i64,
}

#[qapi(condition = "(CONFIG_WIN32 || CONFIG_LINUX)")]
#[qapi(added = "2.2.0")]
pub struct GuestFilesystemInfo {
    /// an array of disk hardware information that the volume lies
    /// on, which may be empty if the disk type is not supported
    pub disk: Vec<GuestDiskAddress>,
    /// mount point path
    pub mountpoint: String,
    /// disk name
    pub name: String,
    /// filesystem capacity in bytes for unprivileged users
    /// (since 3.0)
    #[qapi(name = "total-bytes")]
    #[qapi(added = "3.0.0")]
    pub total_bytes: Option<u64>,
    /// filesystem capacity in bytes for privileged
    /// users (since 9.1)
    #[qapi(name = "total-bytes-privileged")]
    #[qapi(added = "9.1.0")]
    pub total_bytes_privileged: Option<u64>,
    /// file system type string
    pub r#type: String,
    /// file system used bytes (since 3.0)
    #[qapi(name = "used-bytes")]
    #[qapi(added = "3.0.0")]
    pub used_bytes: Option<u64>,
}

#[qapi(condition = "(CONFIG_WIN32 || CONFIG_FSTRIM)")]
#[qapi(added = "2.4.0")]
pub struct GuestFilesystemTrimResponse {
    /// list of `GuestFilesystemTrimResult` per path that was
    /// trimmed
    pub paths: Vec<GuestFilesystemTrimResult>,
}

#[qapi(condition = "(CONFIG_WIN32 || CONFIG_FSTRIM)")]
#[qapi(added = "2.4.0")]
pub struct GuestFilesystemTrimResult {
    /// an error message when trim failed
    pub error: Option<String>,
    /// reported effective minimum for this path
    pub minimum: Option<i64>,
    /// path that was trimmed
    pub path: String,
    /// bytes trimmed for this path
    pub trimmed: Option<i64>,
}

/// Controls the meaning of offset to `guest-file-seek`.
#[qapi(added = "2.6.0")]
pub enum GuestFileWhence {
    /// Symbolic name, and preferred interface
    #[qapi(name = "name")]
    Name(QgaSeek),
    /// Integral value (0 for set, 1 for cur, 2 for end), available
    /// for historical reasons, and might differ from the host's or
    /// guest's SEEK_* values (since: 0.15)
    #[qapi(name = "value")]
    Value(i64),
}

/// Result of guest agent file-write operation
#[qapi(added = "1.4.0")]
pub struct GuestFileWrite {
    /// number of bytes written (note: count is actual bytes
    /// written, after base64-decoding of provided buffer)
    pub count: i64,
    /// whether EOF was encountered during write operation.
    pub eof: bool,
}

/// An enumeration of filesystem freeze states
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_FSFREEZE)")]
#[qapi(added = "1.4.0")]
pub enum GuestFsfreezeStatus {
    /// all non-network guest filesystems frozen
    #[qapi(name = "frozen")]
    Frozen,
    /// filesystems thawed/unfrozen
    #[qapi(name = "thawed")]
    Thawed,
}

#[qapi(added = "2.10.0")]
pub struct GuestHostName {
    /// Fully qualified domain name of the guest OS
    #[qapi(name = "host-name")]
    pub host_name: String,
}

#[qapi(condition = "(CONFIG_WIN32 || HAVE_GETIFADDRS)")]
#[qapi(added = "1.4.0")]
pub struct GuestIpAddress {
    /// IP address
    #[qapi(name = "ip-address")]
    pub ip_address: String,
    /// Type of @ip-address (e.g. ipv4, ipv6)
    #[qapi(name = "ip-address-type")]
    pub ip_address_type: GuestIpAddressType,
    /// Network prefix length of @ip-address
    pub prefix: i64,
}

/// An enumeration of supported IP address types
#[qapi(condition = "(CONFIG_WIN32 || HAVE_GETIFADDRS)")]
#[qapi(added = "1.4.0")]
pub enum GuestIpAddressType {
    /// IP version 4
    #[qapi(name = "ipv4")]
    Ipv4,
    /// IP version 6
    #[qapi(name = "ipv6")]
    Ipv6,
}

/// CPU statistics of Linux
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "7.1.0")]
pub struct GuestLinuxCpuStats {
    /// CPU index in guest OS
    pub cpu: i64,
    /// ime spent running a virtual CPU for guest operating systems
    /// under the  control of the Linux kernel (since Linux 2.6.24)
    pub guest: Option<u64>,
    /// Time spent running a niced guest (since Linux 2.6.33)
    pub guestnice: Option<u64>,
    /// Time spent in the idle task
    pub idle: u64,
    /// Time waiting for I/O to complete (since Linux 2.5.41)
    pub iowait: Option<u64>,
    /// Time servicing interrupts (since Linux 2.6.0-test4)
    pub irq: Option<u64>,
    /// Time spent in user mode with low priority (nice)
    pub nice: u64,
    /// Time servicing softirqs (since Linux 2.6.0-test4)
    pub softirq: Option<u64>,
    /// Stolen time by host (since Linux 2.6.11)
    pub steal: Option<u64>,
    /// Time spent in system mode
    pub system: u64,
    /// Time spent in user mode
    pub user: u64,
}

/// Statistics about process load information
#[qapi(condition = "CONFIG_GETLOADAVG")]
#[qapi(added = "10.0.0")]
pub struct GuestLoadAverage {
    /// 15-minute load avage
    pub load15m: f64,
    /// 1-minute load avage
    pub load1m: f64,
    /// 5-minute load avage
    pub load5m: f64,
}

#[qapi(condition = "(CONFIG_LINUX || CONFIG_WIN32)")]
#[qapi(added = "1.5.0")]
pub struct GuestLogicalProcessor {
    /// Whether offlining the VCPU is possible.  This member
    /// is always filled in by the guest agent when the structure is
    /// returned, and always ignored on input (hence it can be omitted
    /// then).
    #[qapi(name = "can-offline")]
    pub can_offline: Option<bool>,
    /// Arbitrary guest-specific unique identifier of the VCPU.
    #[qapi(name = "logical-id")]
    pub logical_id: i64,
    /// Whether the VCPU is enabled.
    pub online: bool,
}

#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "2.3.0")]
pub struct GuestMemoryBlock {
    /// Whether offlining the MEMORY BLOCK is possible.  This
    /// member is always filled in by the guest agent when the structure
    /// is returned, and always ignored on input (hence it can be
    /// omitted then).
    #[qapi(name = "can-offline")]
    pub can_offline: Option<bool>,
    /// Whether the MEMORY BLOCK is enabled in guest.
    pub online: bool,
    /// Arbitrary guest-specific unique identifier of the
    /// MEMORY BLOCK.
    #[qapi(name = "phys-index")]
    pub phys_index: u64,
}

#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "2.3.0")]
pub struct GuestMemoryBlockInfo {
    /// the size (in bytes) of the guest memory blocks, which are the
    /// minimal units of memory block online/offline operations (also
    /// called Logical Memory Hotplug).
    pub size: u64,
}

#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "2.3.0")]
pub struct GuestMemoryBlockResponse {
    /// the error number.  When memory block operation fails,
    /// we assign the value of 'errno' to this member, it indicates what
    /// goes wrong.  When the operation succeeds, it will be omitted.
    #[qapi(name = "error-code")]
    pub error_code: Option<i64>,
    /// same with the 'phys-index' member of
    /// `GuestMemoryBlock`.
    #[qapi(name = "phys-index")]
    pub phys_index: u64,
    /// the result of memory block operation.
    pub response: GuestMemoryBlockResponseType,
}

/// An enumeration of memory block operation result.
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "2.3.0")]
pub enum GuestMemoryBlockResponseType {
    /// can't find the corresponding memoryXXX directory in
    /// sysfs.
    #[qapi(name = "not-found")]
    NotFound,
    /// the operation of online/offline memory block
    /// fails, because of some errors happen.
    #[qapi(name = "operation-failed")]
    OperationFailed,
    /// for some old kernels, it does not support
    /// online or offline memory block.
    #[qapi(name = "operation-not-supported")]
    OperationNotSupported,
    /// the operation of online/offline memory block is
    /// successful.
    #[qapi(name = "success")]
    Success,
}

#[qapi(condition = "(CONFIG_WIN32 || HAVE_GETIFADDRS)")]
#[qapi(added = "1.4.0")]
pub struct GuestNetworkInterface {
    /// Hardware address of @name
    #[qapi(name = "hardware-address")]
    pub hardware_address: Option<String>,
    /// List of addresses assigned to @name
    #[qapi(name = "ip-addresses")]
    pub ip_addresses: Option<Vec<GuestIpAddress>>,
    /// The name of interface for which info are being delivered
    pub name: String,
    /// various statistic counters related to @name (since
    /// 2.11)
    #[qapi(added = "2.11.0")]
    pub statistics: Option<GuestNetworkInterfaceStat>,
}

#[qapi(condition = "(CONFIG_WIN32 || HAVE_GETIFADDRS)")]
#[qapi(added = "2.11.0")]
pub struct GuestNetworkInterfaceStat {
    /// total bytes received
    #[qapi(name = "rx-bytes")]
    pub rx_bytes: u64,
    /// receiver dropped packets
    #[qapi(name = "rx-dropped")]
    pub rx_dropped: u64,
    /// bad packets received
    #[qapi(name = "rx-errs")]
    pub rx_errs: u64,
    /// total packets received
    #[qapi(name = "rx-packets")]
    pub rx_packets: u64,
    /// total bytes transmitted
    #[qapi(name = "tx-bytes")]
    pub tx_bytes: u64,
    /// dropped packets transmitted
    #[qapi(name = "tx-dropped")]
    pub tx_dropped: u64,
    /// packet transmit problems
    #[qapi(name = "tx-errs")]
    pub tx_errs: u64,
    /// total packets transmitted
    #[qapi(name = "tx-packets")]
    pub tx_packets: u64,
}

/// Route information.  Supported on Linux (since 9.1) and Windows
/// (since 11.0).
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "9.1.0")]
pub struct GuestNetworkRoute {
    /// Destination prefix length (for IPv6)
    pub desprefixlen: Option<String>,
    /// The IP address of the target network or host, The
    /// final destination of the packet
    pub destination: String,
    /// Route flags (not for windows)
    pub flags: Option<u64>,
    /// The IP address of the next hop router
    pub gateway: Option<String>,
    /// The destination network or host's egress network interface
    /// in the routing table
    pub iface: String,
    /// Initial round-trip delay (not for windows, IPv4 only)
    pub irtt: Option<i64>,
    /// Subnet Mask (IPv4 only)
    pub mask: Option<String>,
    /// Route metric
    pub metric: i64,
    /// Data link layer maximum packet size (not for windows)
    pub mtu: Option<i64>,
    /// Next hop IP address (for IPv6)
    pub nexthop: Option<String>,
    /// The route's reference count (not for windows)
    pub refcnt: Option<i64>,
    /// Source IP address (for IPv6)
    pub source: Option<String>,
    /// Source prefix length (for IPv6)
    pub srcprefixlen: Option<String>,
    /// Route usage count (not for windows)
    pub r#use: Option<i64>,
    /// IP version (4 or 6)
    pub version: i64,
    /// TCP window size, used for flow control (not for windows,
    /// IPv4 only)
    pub window: Option<i64>,
}

/// NVMe smart information, based on NVMe specification, section
/// <SMART / Health Information (Log Identifier 02h)>
///
/// TODO: document members briefly
#[qapi(name = "GuestNVMeSmart")]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_LIBUDEV)")]
#[qapi(added = "7.1.0")]
pub struct GuestNvMeSmart {
    #[qapi(name = "available-spare")]
    pub available_spare: i64,
    #[qapi(name = "available-spare-threshold")]
    pub available_spare_threshold: i64,
    #[qapi(name = "controller-busy-time-hi")]
    pub controller_busy_time_hi: u64,
    #[qapi(name = "controller-busy-time-lo")]
    pub controller_busy_time_lo: u64,
    #[qapi(name = "critical-warning")]
    pub critical_warning: i64,
    #[qapi(name = "data-units-read-hi")]
    pub data_units_read_hi: u64,
    #[qapi(name = "data-units-read-lo")]
    pub data_units_read_lo: u64,
    #[qapi(name = "data-units-written-hi")]
    pub data_units_written_hi: u64,
    #[qapi(name = "data-units-written-lo")]
    pub data_units_written_lo: u64,
    #[qapi(name = "host-read-commands-hi")]
    pub host_read_commands_hi: u64,
    #[qapi(name = "host-read-commands-lo")]
    pub host_read_commands_lo: u64,
    #[qapi(name = "host-write-commands-hi")]
    pub host_write_commands_hi: u64,
    #[qapi(name = "host-write-commands-lo")]
    pub host_write_commands_lo: u64,
    #[qapi(name = "media-errors-hi")]
    pub media_errors_hi: u64,
    #[qapi(name = "media-errors-lo")]
    pub media_errors_lo: u64,
    #[qapi(name = "number-of-error-log-entries-hi")]
    pub number_of_error_log_entries_hi: u64,
    #[qapi(name = "number-of-error-log-entries-lo")]
    pub number_of_error_log_entries_lo: u64,
    #[qapi(name = "percentage-used")]
    pub percentage_used: i64,
    #[qapi(name = "power-cycles-hi")]
    pub power_cycles_hi: u64,
    #[qapi(name = "power-cycles-lo")]
    pub power_cycles_lo: u64,
    #[qapi(name = "power-on-hours-hi")]
    pub power_on_hours_hi: u64,
    #[qapi(name = "power-on-hours-lo")]
    pub power_on_hours_lo: u64,
    pub temperature: i64,
    #[qapi(name = "unsafe-shutdowns-hi")]
    pub unsafe_shutdowns_hi: u64,
    #[qapi(name = "unsafe-shutdowns-lo")]
    pub unsafe_shutdowns_lo: u64,
}

/// .. note:: On POSIX systems the fields @id, @name, @pretty-name,
///    @version, @version-id, @variant and @variant-id follow the
///    definition specified in os-release(5).  Refer to the manual page
///    for exact description of the fields.  Their values are taken from
///    the os-release file.  If the file is not present in the system,
///    or the values are not present in the file, the fields are not
///    included.
///
///    On Windows the values are filled from information gathered from
///    the system.
#[qapi(name = "GuestOSInfo")]
#[qapi(added = "2.10.0")]
pub struct GuestOsInfo {
    /// * POSIX: as defined by os-release(5)
    /// * Windows: contains string "mswindows"
    pub id: Option<String>,
    /// * POSIX: release field returned by uname(2)
    /// * Windows: build number of the OS
    #[qapi(name = "kernel-release")]
    pub kernel_release: Option<String>,
    /// * POSIX: version field returned by uname(2)
    /// * Windows: version number of the OS
    #[qapi(name = "kernel-version")]
    pub kernel_version: Option<String>,
    /// * POSIX: machine field returned by uname(2)
    /// * Windows: one of x86, x86_64, arm, ia64
    pub machine: Option<String>,
    /// * POSIX: as defined by os-release(5)
    /// * Windows: contains string "Microsoft Windows"
    pub name: Option<String>,
    /// * POSIX: as defined by os-release(5)
    /// * Windows: product name, e.g. "Microsoft Windows 10 Enterprise"
    #[qapi(name = "pretty-name")]
    pub pretty_name: Option<String>,
    /// * POSIX: as defined by os-release(5)
    /// * Windows: contains string "server" or "client"
    pub variant: Option<String>,
    /// * POSIX: as defined by os-release(5)
    /// * Windows: contains string "server" or "client"
    #[qapi(name = "variant-id")]
    pub variant_id: Option<String>,
    /// * POSIX: as defined by os-release(5)
    /// * Windows: long version string, e.g. "Microsoft Windows Server
    ///   2008"
    pub version: Option<String>,
    /// * POSIX: as defined by os-release(5)
    /// * Windows: short version identifier, e.g. "7" or "20012r2"
    #[qapi(name = "version-id")]
    pub version_id: Option<String>,
}

#[qapi(name = "GuestPCIAddress")]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_LINUX)")]
#[qapi(added = "2.2.0")]
pub struct GuestPciAddress {
    /// bus id
    pub bus: i64,
    /// domain id
    pub domain: i64,
    /// function id
    pub function: i64,
    /// slot id
    pub slot: i64,
}

#[qapi(added = "2.10.0")]
pub struct GuestTimezone {
    /// Offset to UTC in seconds, negative numbers for time zones
    /// west of GMT, positive numbers for east
    pub offset: i64,
    /// Timezone name.  These values may differ depending on guest/OS
    /// and should only be used for informational purposes.
    pub zone: Option<String>,
}

#[qapi(condition = "(CONFIG_WIN32 || HAVE_UTMPX)")]
#[qapi(added = "2.10.0")]
pub struct GuestUser {
    /// Logon domain (windows only)
    pub domain: Option<String>,
    /// Time of login of this user on the computer.  If
    /// multiple instances of the user are logged in, the earliest login
    /// time is reported.  The value is in fractional seconds since
    /// epoch time.
    #[qapi(name = "login-time")]
    pub login_time: f64,
    /// Username
    pub user: String,
}

/// Symbolic names for use in `guest-file-seek`
#[qapi(name = "QGASeek")]
#[qapi(added = "2.6.0")]
pub enum QgaSeek {
    /// Add offset to the current location (same effect as 'whence':1)
    #[qapi(name = "cur")]
    Cur,
    /// Add offset to the end of the file (same effect as 'whence':2)
    #[qapi(name = "end")]
    End,
    /// Set to the specified offset (same effect as 'whence':0)
    #[qapi(name = "set")]
    Set,
}

pub mod commands;
pub mod events;
pub mod metadata;

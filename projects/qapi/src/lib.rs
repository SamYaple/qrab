use qapi_macros::qapi;
use serde_json;
// path begin:	qapi/pragma.json
// path end:	qapi/pragma.json
// path begin:	qapi/error.json
/// QEMU error classes
#[qapi(name = "QapiErrorClass")]
#[qapi(since = "1.2")]
pub enum QapiErrorClass {
    /// this is used for errors that don't require a specific
    /// error class.  This should be the default case for most errors
    #[qapi(name = "GenericError")]
    GenericError,
    /// the requested command has not been found
    #[qapi(name = "CommandNotFound")]
    CommandNotFound,
    /// a device has failed to be become active
    #[qapi(name = "DeviceNotActive")]
    DeviceNotActive,
    /// the requested device has not been found
    #[qapi(name = "DeviceNotFound")]
    DeviceNotFound,
    /// the requested operation can't be fulfilled because a
    /// required KVM capability is missing
    #[qapi(name = "KVMMissingCap")]
    KvmMissingCap,
}
// path end:	qapi/error.json
// path begin:	qapi/common.json
/// An enumeration of the I/O operation types
#[qapi(name = "IoOperationType")]
#[qapi(since = "2.1")]
pub enum IoOperationType {
    /// read operation
    #[qapi(name = "read")]
    Read,
    /// write operation
    #[qapi(name = "write")]
    Write,
}
/// An enumeration of three options: on, off, and auto
#[qapi(name = "OnOffAuto")]
#[qapi(since = "2.2")]
pub enum OnOffAuto {
    /// QEMU selects the value between on and off
    #[qapi(name = "auto")]
    Auto,
    /// Enabled
    #[qapi(name = "on")]
    On,
    /// Disabled
    #[qapi(name = "off")]
    Off,
}
/// An enumeration of three values: on, off, and split
#[qapi(name = "OnOffSplit")]
#[qapi(since = "2.6")]
pub enum OnOffSplit {
    /// Enabled
    #[qapi(name = "on")]
    On,
    /// Disabled
    #[qapi(name = "off")]
    Off,
    /// Mixed
    #[qapi(name = "split")]
    Split,
}
/// This is a string value or the explicit lack of a string (null
/// pointer in C).  Intended for cases when 'optional absent' already
/// has a different meaning.
#[qapi(name = "StrOrNull")]
#[qapi(since = "2.10")]
pub enum StrOrNull {
    /// the string value
    #[qapi(name = "s")]
    S(String),
    /// no string value
    #[qapi(name = "n")]
    N(Null),
}
/// An enumeration of options for specifying a PCI BAR
#[qapi(name = "OffAutoPCIBAR")]
#[qapi(since = "2.12")]
pub enum OffAutoPcibar {
    /// The specified feature is disabled
    #[qapi(name = "off")]
    Off,
    /// The PCI BAR for the feature is automatically selected
    #[qapi(name = "auto")]
    Auto,
    /// PCI BAR0 is used for the feature
    #[qapi(name = "bar0")]
    Bar0,
    /// PCI BAR1 is used for the feature
    #[qapi(name = "bar1")]
    Bar1,
    /// PCI BAR2 is used for the feature
    #[qapi(name = "bar2")]
    Bar2,
    /// PCI BAR3 is used for the feature
    #[qapi(name = "bar3")]
    Bar3,
    /// PCI BAR4 is used for the feature
    #[qapi(name = "bar4")]
    Bar4,
    /// PCI BAR5 is used for the feature
    #[qapi(name = "bar5")]
    Bar5,
}
/// An enumeration of PCIe link speeds in units of GT/s
#[qapi(name = "PCIELinkSpeed")]
#[qapi(since = "4.0")]
pub enum PcieLinkSpeed {
    /// 2.5GT/s
    #[qapi(name = "2_5")]
    _25,
    /// 5.0GT/s
    #[qapi(name = "5")]
    _5,
    /// 8.0GT/s
    #[qapi(name = "8")]
    _8,
    /// 16.0GT/s
    #[qapi(name = "16")]
    _16,
    /// 32.0GT/s (since 9.0)
    #[qapi(name = "32")]
    _32,
    /// 64.0GT/s (since 9.0)
    #[qapi(name = "64")]
    _64,
}
/// An enumeration of PCIe link width
#[qapi(name = "PCIELinkWidth")]
#[qapi(since = "4.0")]
pub enum PcieLinkWidth {
    /// x1
    #[qapi(name = "1")]
    _1,
    /// x2
    #[qapi(name = "2")]
    _2,
    /// x4
    #[qapi(name = "4")]
    _4,
    /// x8
    #[qapi(name = "8")]
    _8,
    /// x12
    #[qapi(name = "12")]
    _12,
    /// x16
    #[qapi(name = "16")]
    _16,
    /// x32
    #[qapi(name = "32")]
    _32,
}
/// Host memory policy types
#[qapi(name = "HostMemPolicy")]
#[qapi(since = "2.1")]
pub enum HostMemPolicy {
    /// restore default policy, remove any nondefault policy
    #[qapi(name = "default")]
    Default,
    /// set the preferred host nodes for allocation
    #[qapi(name = "preferred")]
    Preferred,
    /// a strict policy that restricts memory allocation to the host
    /// nodes specified
    #[qapi(name = "bind")]
    Bind,
    /// memory allocations are interleaved across the set of
    /// host nodes specified
    #[qapi(name = "interleave")]
    Interleave,
}
/// Indicates whether a netfilter is attached to a netdev's transmit
/// queue or receive queue or both.
#[qapi(name = "NetFilterDirection")]
#[qapi(since = "2.5")]
pub enum NetFilterDirection {
    /// the filter is attached both to the receive and the transmit
    /// queue of the netdev (default).
    #[qapi(name = "all")]
    All,
    /// the filter is attached to the receive queue of the netdev,
    /// where it will receive packets sent to the netdev.
    #[qapi(name = "rx")]
    Rx,
    /// the filter is attached to the transmit queue of the netdev,
    /// where it will receive packets sent by the netdev.
    #[qapi(name = "tx")]
    Tx,
}
/// Keys to toggle input-linux between host and guest.
#[qapi(name = "GrabToggleKeys")]
#[qapi(since = "4.0")]
pub enum GrabToggleKeys {
    #[qapi(name = "ctrl-ctrl")]
    CtrlCtrl,
    #[qapi(name = "alt-alt")]
    AltAlt,
    #[qapi(name = "shift-shift")]
    ShiftShift,
    #[qapi(name = "meta-meta")]
    MetaMeta,
    #[qapi(name = "scrolllock")]
    Scrolllock,
    #[qapi(name = "ctrl-scrolllock")]
    CtrlScrolllock,
}
#[qapi(name = "HumanReadableText")]
#[qapi(since = "6.2")]
pub struct HumanReadableText {
    /// Formatted output intended for humans.
    #[qapi(name = "human-readable-text")]
    pub human_readable_text: String,
}
// path end:	qapi/common.json
// path begin:	qapi/sockets.json
/// The network address family
#[qapi(name = "NetworkAddressFamily")]
#[qapi(since = "2.1")]
pub enum NetworkAddressFamily {
    /// IPV4 family
    #[qapi(name = "ipv4")]
    Ipv4,
    /// IPV6 family
    #[qapi(name = "ipv6")]
    Ipv6,
    /// unix socket
    #[qapi(name = "unix")]
    Unix,
    /// vsock family (since 2.8)
    #[qapi(name = "vsock")]
    Vsock,
    /// otherwise
    #[qapi(name = "unknown")]
    Unknown,
}
#[qapi(name = "InetSocketAddressBase")]
pub struct InetSocketAddressBase {
    /// host part of the address
    #[qapi(name = "host")]
    pub host: String,
    /// port part of the address
    #[qapi(name = "port")]
    pub port: String,
}
/// Captures a socket address or address range in the Internet
/// namespace.
#[qapi(name = "InetSocketAddress")]
#[qapi(since = "1.3")]
pub struct InetSocketAddress {
    /// host part of the address
    #[qapi(name = "host")]
    pub host: String,
    /// port part of the address
    #[qapi(name = "port")]
    pub port: String,
    /// true if the host/port are guaranteed to be numeric, false
    /// if name resolution should be attempted.  Defaults to false.
    /// (Since 2.9)
    #[qapi(name = "numeric")]
    pub numeric: Option<bool>,
    /// If present, this is range of possible addresses, with port
    /// between @port and @to.
    #[qapi(name = "to")]
    pub to: Option<u16>,
    /// whether to accept IPv4 addresses, default try both IPv4 and
    /// IPv6
    #[qapi(name = "ipv4")]
    pub ipv4: Option<bool>,
    /// whether to accept IPv6 addresses, default try both IPv4 and
    /// IPv6
    #[qapi(name = "ipv6")]
    pub ipv6: Option<bool>,
    /// enable keep-alive when connecting to this socket.  Not
    /// supported for passive sockets.  (Since 4.2)
    #[qapi(name = "keep-alive")]
    pub keep_alive: Option<bool>,
    /// enable multi-path TCP.  (Since 6.1)
    #[qapi(name = "mptcp")]
    #[qapi(condition = "HAVE_IPPROTO_MPTCP")]
    pub mptcp: Option<bool>,
}
/// Captures a socket address in the local ("Unix socket") namespace.
#[qapi(name = "UnixSocketAddress")]
#[qapi(since = "1.3")]
pub struct UnixSocketAddress {
    /// filesystem path to use
    #[qapi(name = "path")]
    pub path: String,
    /// if true, this is a Linux abstract socket address.  @path
    /// will be prefixed by a null byte, and optionally padded with null
    /// bytes.  Defaults to false.  (Since 5.1)
    #[qapi(name = "abstract")]
    #[qapi(condition = "CONFIG_LINUX")]
    pub r#abstract: Option<bool>,
    /// if false, pad an abstract socket address with enough null
    /// bytes to make it fill struct sockaddr_un member sun_path.
    /// Defaults to true.  (Since 5.1)
    #[qapi(name = "tight")]
    #[qapi(condition = "CONFIG_LINUX")]
    pub tight: Option<bool>,
}
/// Captures a socket address in the vsock namespace.
#[qapi(name = "VsockSocketAddress")]
#[qapi(since = "2.8")]
pub struct VsockSocketAddress {
    /// unique host identifier
    #[qapi(name = "cid")]
    pub cid: String,
    /// port
    #[qapi(name = "port")]
    pub port: String,
}
/// A file descriptor name or number.
#[qapi(name = "FdSocketAddress")]
#[qapi(since = "1.2")]
pub struct FdSocketAddress {
    /// decimal is for file descriptor number, otherwise it's a file
    /// descriptor name.  Named file descriptors are permitted in
    /// monitor commands, in combination with the 'getfd' command.
    /// Decimal file descriptors are permitted at startup or other
    /// contexts where no monitor context is active.
    #[qapi(name = "str")]
    pub str: String,
}
#[qapi(name = "InetSocketAddressWrapper")]
#[qapi(since = "1.3")]
pub struct InetSocketAddressWrapper {
    /// internet domain socket address
    #[qapi(name = "data")]
    pub data: InetSocketAddress,
}
#[qapi(name = "UnixSocketAddressWrapper")]
#[qapi(since = "1.3")]
pub struct UnixSocketAddressWrapper {
    /// UNIX domain socket address
    #[qapi(name = "data")]
    pub data: UnixSocketAddress,
}
#[qapi(name = "VsockSocketAddressWrapper")]
#[qapi(since = "2.8")]
pub struct VsockSocketAddressWrapper {
    /// VSOCK domain socket address
    #[qapi(name = "data")]
    pub data: VsockSocketAddress,
}
#[qapi(name = "FdSocketAddressWrapper")]
#[qapi(since = "1.3")]
pub struct FdSocketAddressWrapper {
    /// file descriptor name or number
    #[qapi(name = "data")]
    pub data: FdSocketAddress,
}
pub enum SocketAddressLegacyBranch {
    #[qapi(name = "inet")]
    Inet(InetSocketAddressWrapper),
    #[qapi(name = "unix")]
    Unix(UnixSocketAddressWrapper),
    #[qapi(name = "vsock")]
    Vsock(VsockSocketAddressWrapper),
    #[qapi(name = "fd")]
    Fd(FdSocketAddressWrapper),
}
/// Captures the address of a socket, which could also be a named file
/// descriptor
#[qapi(name = "SocketAddressLegacy")]
#[qapi(since = "1.3")]
pub struct SocketAddressLegacy {
    /// Transport type
    #[qapi(name = "type")]
    #[qapi(discriminator)]
    pub r#type: SocketAddressType,
    #[qapi(union)]
    pub u: Option<SocketAddressLegacyBranch>,
}
/// Available SocketAddress types
#[qapi(name = "SocketAddressType")]
#[qapi(since = "2.9")]
pub enum SocketAddressType {
    /// Internet address
    #[qapi(name = "inet")]
    Inet,
    /// Unix domain socket
    #[qapi(name = "unix")]
    Unix,
    /// VMCI address
    #[qapi(name = "vsock")]
    Vsock,
    /// Socket file descriptor
    #[qapi(name = "fd")]
    Fd,
}
pub enum SocketAddressBranch {
    #[qapi(name = "inet")]
    Inet(InetSocketAddress),
    #[qapi(name = "unix")]
    Unix(UnixSocketAddress),
    #[qapi(name = "vsock")]
    Vsock(VsockSocketAddress),
    #[qapi(name = "fd")]
    Fd(FdSocketAddress),
}
/// Captures the address of a socket, which could also be a socket file
/// descriptor
#[qapi(name = "SocketAddress")]
#[qapi(since = "2.9")]
pub struct SocketAddress {
    /// Transport type
    #[qapi(name = "type")]
    #[qapi(discriminator)]
    pub r#type: SocketAddressType,
    #[qapi(union)]
    pub u: Option<SocketAddressBranch>,
}
// path end:	qapi/sockets.json
// path begin:	qapi/run-state.json
/// An enumeration of VM run states.
#[qapi(name = "RunState")]
pub enum RunState {
    /// QEMU is running on a debugger
    #[qapi(name = "debug")]
    Debug,
    /// guest is paused waiting for an incoming migration.  Note
    /// that this state does not tell whether the machine will start at
    /// the end of the migration.  This depends on the command-line -S
    /// option and any invocation of 'stop' or 'cont' that has happened
    /// since QEMU was started.
    #[qapi(name = "inmigrate")]
    Inmigrate,
    /// An internal error that prevents further guest
    /// execution has occurred
    #[qapi(name = "internal-error")]
    InternalError,
    /// the last IOP has failed and the device is configured to
    /// pause on I/O errors
    #[qapi(name = "io-error")]
    IoError,
    /// guest has been paused via the 'stop' command
    #[qapi(name = "paused")]
    Paused,
    /// guest is paused following a successful 'migrate'
    #[qapi(name = "postmigrate")]
    Postmigrate,
    /// QEMU was started with -S and guest has not started
    #[qapi(name = "prelaunch")]
    Prelaunch,
    /// guest is paused to finish the migration process
    #[qapi(name = "finish-migrate")]
    FinishMigrate,
    /// guest is paused to restore VM state
    #[qapi(name = "restore-vm")]
    RestoreVm,
    /// guest is actively running
    #[qapi(name = "running")]
    Running,
    /// guest is paused to save the VM state
    #[qapi(name = "save-vm")]
    SaveVm,
    /// guest is shut down (and -no-shutdown is in use)
    #[qapi(name = "shutdown")]
    Shutdown,
    /// guest is suspended (ACPI S3)
    #[qapi(name = "suspended")]
    Suspended,
    /// the watchdog action is configured to pause and has been
    /// triggered
    #[qapi(name = "watchdog")]
    Watchdog,
    /// guest has been panicked as a result of guest OS
    /// panic
    #[qapi(name = "guest-panicked")]
    GuestPanicked,
    /// guest is paused to save/restore VM state under colo
    /// checkpoint, VM can not get into this state unless colo
    /// capability is enabled for migration.  (since 2.8)
    #[qapi(name = "colo")]
    Colo,
}
/// An enumeration of reasons for a Shutdown.
#[qapi(name = "ShutdownCause")]
pub enum ShutdownCause {
    /// No shutdown request pending
    #[qapi(name = "none")]
    None,
    /// An error prevents further use of guest
    #[qapi(name = "host-error")]
    HostError,
    /// Reaction to the QMP command 'quit'
    #[qapi(name = "host-qmp-quit")]
    HostQmpQuit,
    /// Reaction to the QMP command 'system_reset'
    #[qapi(name = "host-qmp-system-reset")]
    HostQmpSystemReset,
    /// Reaction to a signal, such as SIGINT
    #[qapi(name = "host-signal")]
    HostSignal,
    /// Reaction to a UI event, like window close
    #[qapi(name = "host-ui")]
    HostUi,
    /// Guest shutdown/suspend request, via ACPI or other
    /// hardware-specific means
    #[qapi(name = "guest-shutdown")]
    GuestShutdown,
    /// Guest reset request, and command line turns that into
    /// a shutdown
    #[qapi(name = "guest-reset")]
    GuestReset,
    /// Guest panicked, and command line turns that into a
    /// shutdown
    #[qapi(name = "guest-panic")]
    GuestPanic,
    /// Partial guest reset that does not trigger QMP
    /// events and ignores --no-reboot.  This is useful for sanitizing
    /// hypercalls on s390 that are used during kexec/kdump/boot
    #[qapi(name = "subsystem-reset")]
    SubsystemReset,
    /// A snapshot is being loaded by the record & replay
    /// subsystem.  This value is used only within QEMU.  It doesn't
    /// occur in QMP.  (since 7.2)
    #[qapi(name = "snapshot-load")]
    SnapshotLoad,
}
/// Information about VM run state
#[qapi(name = "StatusInfo")]
#[qapi(since = "0.14")]
pub struct StatusInfo {
    /// true if all VCPUs are runnable, false if not runnable
    #[qapi(name = "running")]
    pub running: bool,
    /// the virtual machine @RunState
    #[qapi(name = "status")]
    pub status: RunState,
}
/// Query the run status of the VM
#[qapi(name = "query-status")]
#[qapi(since = "0.14")]
#[qapi(returns = "StatusInfo")]
#[qapi(allow_preconfig)]
pub struct QueryStatus {}
/// Emitted when the virtual machine has shut down, indicating that qemu
/// is about to exit.
#[qapi(name = "SHUTDOWN")]
#[qapi(since = "0.12")]
pub struct Shutdown {
    /// If true, the shutdown was triggered by a guest request (such
    /// as a guest-initiated ACPI shutdown request or other
    /// hardware-specific action) rather than a host request (such as
    /// sending qemu a SIGINT).  (since 2.10)
    #[qapi(name = "guest")]
    pub guest: bool,
    /// The @ShutdownCause which resulted in the SHUTDOWN.
    /// (since 4.0)
    #[qapi(name = "reason")]
    pub reason: ShutdownCause,
}
/// Emitted when the virtual machine is powered down through the power
/// control system, such as via ACPI.
#[qapi(name = "POWERDOWN")]
#[qapi(since = "0.12")]
pub struct Powerdown {}
/// Emitted when the virtual machine is reset
#[qapi(name = "RESET")]
#[qapi(since = "0.12")]
pub struct Reset {
    /// If true, the reset was triggered by a guest request (such as
    /// a guest-initiated ACPI reboot request or other hardware-specific
    /// action) rather than a host request (such as the QMP command
    /// system_reset).  (since 2.10)
    #[qapi(name = "guest")]
    pub guest: bool,
    /// The @ShutdownCause of the RESET.  (since 4.0)
    #[qapi(name = "reason")]
    pub reason: ShutdownCause,
}
/// Emitted when the virtual machine is stopped
#[qapi(name = "STOP")]
#[qapi(since = "0.12")]
pub struct Stop {}
/// Emitted when the virtual machine resumes execution
#[qapi(name = "RESUME")]
#[qapi(since = "0.12")]
pub struct Resume {}
/// Emitted when guest enters a hardware suspension state, for example,
/// S3 state, which is sometimes called standby state
#[qapi(name = "SUSPEND")]
#[qapi(since = "1.1")]
pub struct Suspend {}
/// Emitted when guest enters a hardware suspension state with data
/// saved on disk, for example, S4 state, which is sometimes called
/// hibernate state
#[qapi(name = "SUSPEND_DISK")]
#[qapi(since = "1.2")]
pub struct SuspendDisk {}
/// Emitted when the guest has woken up from suspend state and is
/// running
#[qapi(name = "WAKEUP")]
#[qapi(since = "1.1")]
pub struct Wakeup {}
/// Emitted when the watchdog device's timer is expired
#[qapi(name = "WATCHDOG")]
#[qapi(since = "0.13")]
pub struct Watchdog {
    /// action that has been taken
    #[qapi(name = "action")]
    pub action: WatchdogAction,
}
/// An enumeration of the actions taken when the watchdog device's timer
/// is expired
#[qapi(name = "WatchdogAction")]
#[qapi(since = "2.1")]
pub enum WatchdogAction {
    /// system resets
    #[qapi(name = "reset")]
    Reset,
    /// system shutdown, note that it is similar to @powerdown,
    /// which tries to set to system status and notify guest
    #[qapi(name = "shutdown")]
    Shutdown,
    /// system poweroff, the emulator program exits
    #[qapi(name = "poweroff")]
    Poweroff,
    /// system pauses, similar to @stop
    #[qapi(name = "pause")]
    Pause,
    /// system enters debug state
    #[qapi(name = "debug")]
    Debug,
    /// nothing is done
    #[qapi(name = "none")]
    None,
    /// a non-maskable interrupt is injected into the first
    /// VCPU (all VCPUS on x86) (since 2.4)
    #[qapi(name = "inject-nmi")]
    InjectNmi,
}
/// Possible QEMU actions upon guest reboot
#[qapi(name = "RebootAction")]
#[qapi(since = "6.0")]
pub enum RebootAction {
    /// Reset the VM
    #[qapi(name = "reset")]
    Reset,
    /// Shutdown the VM and exit, according to the shutdown
    /// action
    #[qapi(name = "shutdown")]
    Shutdown,
}
/// Possible QEMU actions upon guest shutdown
#[qapi(name = "ShutdownAction")]
#[qapi(since = "6.0")]
pub enum ShutdownAction {
    /// Shutdown the VM and exit
    #[qapi(name = "poweroff")]
    Poweroff,
    /// pause the VM
    #[qapi(name = "pause")]
    Pause,
}
#[qapi(name = "PanicAction")]
#[qapi(since = "6.0")]
pub enum PanicAction {
    /// Pause the VM
    #[qapi(name = "pause")]
    Pause,
    /// Shutdown the VM and exit, according to the shutdown
    /// action
    #[qapi(name = "shutdown")]
    Shutdown,
    /// Shutdown the VM and exit with nonzero status (since
    /// 7.1)
    #[qapi(name = "exit-failure")]
    ExitFailure,
    /// Continue VM execution
    #[qapi(name = "none")]
    None,
}
/// Set watchdog action.
#[qapi(name = "watchdog-set-action")]
#[qapi(since = "2.11")]
#[qapi(returns = "()")]
pub struct WatchdogSetAction {
    /// @WatchdogAction action taken when watchdog timer expires.
    #[qapi(name = "action")]
    pub action: WatchdogAction,
}
/// Set the actions that will be taken by the emulator in response to
/// guest events.
#[qapi(name = "set-action")]
#[qapi(since = "6.0")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct SetAction {
    /// @RebootAction action taken on guest reboot.
    #[qapi(name = "reboot")]
    pub reboot: Option<RebootAction>,
    /// @ShutdownAction action taken on guest shutdown.
    #[qapi(name = "shutdown")]
    pub shutdown: Option<ShutdownAction>,
    /// @PanicAction action taken on guest panic.
    #[qapi(name = "panic")]
    pub panic: Option<PanicAction>,
    /// @WatchdogAction action taken when watchdog timer expires.
    #[qapi(name = "watchdog")]
    pub watchdog: Option<WatchdogAction>,
}
/// Emitted when guest OS panic is detected
#[qapi(name = "GUEST_PANICKED")]
#[qapi(since = "1.5")]
pub struct GuestPanicked {
    /// action that has been taken, currently always "pause"
    #[qapi(name = "action")]
    pub action: GuestPanicAction,
    /// information about a panic (since 2.9)
    #[qapi(name = "info")]
    pub info: Option<GuestPanicInformation>,
}
/// Emitted when guest OS crash loaded is detected
#[qapi(name = "GUEST_CRASHLOADED")]
#[qapi(since = "5.0")]
pub struct GuestCrashloaded {
    /// action that has been taken, currently always "run"
    #[qapi(name = "action")]
    pub action: GuestPanicAction,
    /// information about a panic
    #[qapi(name = "info")]
    pub info: Option<GuestPanicInformation>,
}
/// Emitted when guest submits a shutdown request via pvpanic interface
#[qapi(name = "GUEST_PVSHUTDOWN")]
#[qapi(since = "9.1")]
pub struct GuestPvshutdown {}
/// An enumeration of the actions taken when guest OS panic is detected
#[qapi(name = "GuestPanicAction")]
#[qapi(since = "2.1")]
pub enum GuestPanicAction {
    /// system pauses
    #[qapi(name = "pause")]
    Pause,
    /// system powers off (since 2.8)
    #[qapi(name = "poweroff")]
    Poweroff,
    /// system continues to run (since 5.0)
    #[qapi(name = "run")]
    Run,
}
/// An enumeration of the guest panic information types
#[qapi(name = "GuestPanicInformationType")]
#[qapi(since = "2.9")]
pub enum GuestPanicInformationType {
    /// hyper-v guest panic information type
    #[qapi(name = "hyper-v")]
    HyperV,
    /// s390 guest panic information type (Since: 2.12)
    #[qapi(name = "s390")]
    S390,
}
pub enum GuestPanicInformationBranch {
    #[qapi(name = "hyper-v")]
    HyperV(GuestPanicInformationHyperV),
    #[qapi(name = "s390")]
    S390(GuestPanicInformationS390),
}
/// Information about a guest panic
#[qapi(name = "GuestPanicInformation")]
#[qapi(since = "2.9")]
pub struct GuestPanicInformation {
    /// Crash type that defines the hypervisor specific information
    #[qapi(name = "type")]
    #[qapi(discriminator)]
    pub r#type: GuestPanicInformationType,
    #[qapi(union)]
    pub u: Option<GuestPanicInformationBranch>,
}
/// Hyper-V specific guest panic information (HV crash MSRs)
#[qapi(name = "GuestPanicInformationHyperV")]
#[qapi(since = "2.9")]
pub struct GuestPanicInformationHyperV {
    /// for Windows, STOP code for the guest crash.  For Linux,
    /// an error code.
    #[qapi(name = "arg1")]
    pub arg1: u64,
    /// for Windows, first argument of the STOP.  For Linux, the
    /// guest OS ID, which has the kernel version in bits 16-47 and
    /// 0x8100 in bits 48-63.
    #[qapi(name = "arg2")]
    pub arg2: u64,
    /// for Windows, second argument of the STOP.  For Linux, the
    /// program counter of the guest.
    #[qapi(name = "arg3")]
    pub arg3: u64,
    /// for Windows, third argument of the STOP.  For Linux, the
    /// RAX register (x86) or the stack pointer (aarch64) of the guest.
    #[qapi(name = "arg4")]
    pub arg4: u64,
    /// for Windows, fourth argument of the STOP.  For x86 Linux, the
    /// stack pointer of the guest.
    #[qapi(name = "arg5")]
    pub arg5: u64,
}
/// Reason why the CPU is in a crashed state.
#[qapi(name = "S390CrashReason")]
#[qapi(since = "2.12")]
pub enum S390CrashReason {
    /// no crash reason was set
    #[qapi(name = "unknown")]
    Unknown,
    /// the CPU has entered a disabled wait state
    #[qapi(name = "disabled-wait")]
    DisabledWait,
    /// clock comparator or cpu timer interrupt with new PSW
    /// enabled for external interrupts
    #[qapi(name = "extint-loop")]
    ExtintLoop,
    /// program interrupt with BAD new PSW
    #[qapi(name = "pgmint-loop")]
    PgmintLoop,
    /// operation exception interrupt with invalid code at the
    /// program interrupt new PSW
    #[qapi(name = "opint-loop")]
    OpintLoop,
}
/// S390 specific guest panic information (PSW)
#[qapi(name = "GuestPanicInformationS390")]
#[qapi(since = "2.12")]
pub struct GuestPanicInformationS390 {
    /// core id of the CPU that crashed
    #[qapi(name = "core")]
    pub core: u32,
    /// control fields of guest PSW
    #[qapi(name = "psw-mask")]
    pub psw_mask: u64,
    /// guest instruction address
    #[qapi(name = "psw-addr")]
    pub psw_addr: u64,
    /// guest crash reason
    #[qapi(name = "reason")]
    pub reason: S390CrashReason,
}
/// Emitted when a memory failure occurs on host side.
#[qapi(name = "MEMORY_FAILURE")]
#[qapi(since = "5.2")]
pub struct MemoryFailure {
    /// recipient is defined as @MemoryFailureRecipient.
    #[qapi(name = "recipient")]
    pub recipient: MemoryFailureRecipient,
    /// action that has been taken.
    #[qapi(name = "action")]
    pub action: MemoryFailureAction,
    /// flags for MemoryFailureAction.
    #[qapi(name = "flags")]
    pub flags: MemoryFailureFlags,
}
/// Hardware memory failure occurs, handled by recipient.
#[qapi(name = "MemoryFailureRecipient")]
#[qapi(since = "5.2")]
pub enum MemoryFailureRecipient {
    /// memory failure at QEMU process address space.  (none
    /// guest memory, but used by QEMU itself).
    #[qapi(name = "hypervisor")]
    Hypervisor,
    /// memory failure at guest memory,
    #[qapi(name = "guest")]
    Guest,
}
/// Actions taken by QEMU in response to a hardware memory failure.
#[qapi(name = "MemoryFailureAction")]
#[qapi(since = "5.2")]
pub enum MemoryFailureAction {
    /// the memory failure could be ignored.  This will only be the
    /// case for action-optional failures.
    #[qapi(name = "ignore")]
    Ignore,
    /// memory failure occurred in guest memory, the guest enabled
    /// MCE handling mechanism, and QEMU could inject the MCE into the
    /// guest successfully.
    #[qapi(name = "inject")]
    Inject,
    /// the failure is unrecoverable.  This occurs for
    /// action-required failures if the recipient is the hypervisor;
    /// QEMU will exit.
    #[qapi(name = "fatal")]
    Fatal,
    /// the failure is unrecoverable but confined to the guest.
    /// This occurs if the recipient is a guest guest which is not ready
    /// to handle memory failures.
    #[qapi(name = "reset")]
    Reset,
}
/// Additional information on memory failures.
#[qapi(name = "MemoryFailureFlags")]
#[qapi(since = "5.2")]
pub struct MemoryFailureFlags {
    /// whether a memory failure event is action-required
    /// or action-optional (e.g. a failure during memory scrub).
    #[qapi(name = "action-required")]
    pub action_required: bool,
    /// whether the failure occurred while the previous failure
    /// was still in progress.
    #[qapi(name = "recursive")]
    pub recursive: bool,
}
/// An enumeration of the options specified when enabling notify VM exit
#[qapi(name = "NotifyVmexitOption")]
#[qapi(since = "7.2")]
pub enum NotifyVmexitOption {
    /// enable the feature, do nothing and continue if the notify VM
    /// exit happens.
    #[qapi(name = "run")]
    Run,
    /// enable the feature, raise a internal error if the
    /// notify VM exit happens.
    #[qapi(name = "internal-error")]
    InternalError,
    /// disable the feature.
    #[qapi(name = "disable")]
    Disable,
}
// path end:	qapi/run-state.json
// path begin:	qapi/crypto.json
/// The type of network endpoint that will be using the credentials.
/// Most types of credential require different setup / structures
/// depending on whether they will be used in a server versus a client.
#[qapi(name = "QCryptoTLSCredsEndpoint")]
#[qapi(since = "2.5")]
pub enum QCryptoTlsCredsEndpoint {
    /// the network endpoint is acting as the client
    #[qapi(name = "client")]
    Client,
    /// the network endpoint is acting as the server
    #[qapi(name = "server")]
    Server,
}
/// The data format that the secret is provided in
#[qapi(name = "QCryptoSecretFormat")]
#[qapi(since = "2.6")]
pub enum QCryptoSecretFormat {
    /// raw bytes.  When encoded in JSON only valid UTF-8 sequences
    /// can be used
    #[qapi(name = "raw")]
    Raw,
    /// arbitrary base64 encoded binary data
    #[qapi(name = "base64")]
    Base64,
}
/// The supported algorithms for computing content digests
#[qapi(name = "QCryptoHashAlgorithm")]
#[qapi(since = "2.6")]
pub enum QCryptoHashAlgorithm {
    /// MD5.  Should not be used in any new code, legacy compat only
    #[qapi(name = "md5")]
    Md5,
    /// SHA-1.  Should not be used in any new code, legacy compat only
    #[qapi(name = "sha1")]
    Sha1,
    /// SHA-224.  (since 2.7)
    #[qapi(name = "sha224")]
    Sha224,
    /// SHA-256.  Current recommended strong hash.
    #[qapi(name = "sha256")]
    Sha256,
    /// SHA-384.  (since 2.7)
    #[qapi(name = "sha384")]
    Sha384,
    /// SHA-512.  (since 2.7)
    #[qapi(name = "sha512")]
    Sha512,
    /// RIPEMD-160.  (since 2.7)
    #[qapi(name = "ripemd160")]
    Ripemd160,
}
/// The supported algorithms for content encryption ciphers
#[qapi(name = "QCryptoCipherAlgorithm")]
#[qapi(since = "2.6")]
pub enum QCryptoCipherAlgorithm {
    /// AES with 128 bit / 16 byte keys
    #[qapi(name = "aes-128")]
    Aes128,
    /// AES with 192 bit / 24 byte keys
    #[qapi(name = "aes-192")]
    Aes192,
    /// AES with 256 bit / 32 byte keys
    #[qapi(name = "aes-256")]
    Aes256,
    /// DES with 56 bit / 8 byte keys.  Do not use except in VNC.
    /// (since 6.1)
    #[qapi(name = "des")]
    Des,
    /// 3DES(EDE) with 192 bit / 24 byte keys (since 2.9)
    #[qapi(name = "3des")]
    _3des,
    /// Cast5 with 128 bit / 16 byte keys
    #[qapi(name = "cast5-128")]
    Cast5128,
    /// Serpent with 128 bit / 16 byte keys
    #[qapi(name = "serpent-128")]
    Serpent128,
    /// Serpent with 192 bit / 24 byte keys
    #[qapi(name = "serpent-192")]
    Serpent192,
    /// Serpent with 256 bit / 32 byte keys
    #[qapi(name = "serpent-256")]
    Serpent256,
    /// Twofish with 128 bit / 16 byte keys
    #[qapi(name = "twofish-128")]
    Twofish128,
    /// Twofish with 192 bit / 24 byte keys
    #[qapi(name = "twofish-192")]
    Twofish192,
    /// Twofish with 256 bit / 32 byte keys
    #[qapi(name = "twofish-256")]
    Twofish256,
    /// SM4 with 128 bit / 16 byte keys (since 9.0)
    #[qapi(name = "sm4")]
    Sm4,
}
/// The supported modes for content encryption ciphers
#[qapi(name = "QCryptoCipherMode")]
#[qapi(since = "2.6")]
pub enum QCryptoCipherMode {
    /// Electronic Code Book
    #[qapi(name = "ecb")]
    Ecb,
    /// Cipher Block Chaining
    #[qapi(name = "cbc")]
    Cbc,
    /// XEX with tweaked code book and ciphertext stealing
    #[qapi(name = "xts")]
    Xts,
    /// Counter (Since 2.8)
    #[qapi(name = "ctr")]
    Ctr,
}
/// The supported algorithms for generating initialization vectors for
/// full disk encryption.  The 'plain' generator should not be used for
/// disks with sector numbers larger than 2^32, except where
/// compatibility with pre-existing Linux dm-crypt volumes is required.
#[qapi(name = "QCryptoIVGenAlgorithm")]
#[qapi(since = "2.6")]
pub enum QCryptoIvGenAlgorithm {
    /// 64-bit sector number truncated to 32-bits
    #[qapi(name = "plain")]
    Plain,
    /// 64-bit sector number
    #[qapi(name = "plain64")]
    Plain64,
    /// 64-bit sector number encrypted with a hash of the encryption
    /// key
    #[qapi(name = "essiv")]
    Essiv,
}
/// The supported full disk encryption formats
#[qapi(name = "QCryptoBlockFormat")]
#[qapi(since = "2.6")]
pub enum QCryptoBlockFormat {
    /// QCow/QCow2 built-in AES-CBC encryption.  Use only for
    /// liberating data from old images.
    #[qapi(name = "qcow")]
    Qcow,
    /// LUKS encryption format.  Recommended for new images
    #[qapi(name = "luks")]
    Luks,
}
/// The common options that apply to all full disk encryption formats
#[qapi(name = "QCryptoBlockOptionsBase")]
#[qapi(since = "2.6")]
pub struct QCryptoBlockOptionsBase {
    /// the encryption format
    #[qapi(name = "format")]
    pub format: QCryptoBlockFormat,
}
/// The options that apply to QCow/QCow2 AES-CBC encryption format
#[qapi(name = "QCryptoBlockOptionsQCow")]
#[qapi(since = "2.6")]
pub struct QCryptoBlockOptionsQCow {
    /// the ID of a QCryptoSecret object providing the
    /// decryption key.  Mandatory except when probing image for
    /// metadata only.
    #[qapi(name = "key-secret")]
    pub key_secret: Option<String>,
}
/// The options that apply to LUKS encryption format
#[qapi(name = "QCryptoBlockOptionsLUKS")]
#[qapi(since = "2.6")]
pub struct QCryptoBlockOptionsLuks {
    /// the ID of a QCryptoSecret object providing the
    /// decryption key.  Mandatory except when probing image for
    /// metadata only.
    #[qapi(name = "key-secret")]
    pub key_secret: Option<String>,
}
/// The options that apply to LUKS encryption format initialization
#[qapi(name = "QCryptoBlockCreateOptionsLUKS")]
#[qapi(since = "2.6")]
pub struct QCryptoBlockCreateOptionsLuks {
    /// the ID of a QCryptoSecret object providing the
    /// decryption key.  Mandatory except when probing image for
    /// metadata only.
    #[qapi(name = "key-secret")]
    pub key_secret: Option<String>,
    /// the cipher algorithm for data encryption Currently
    /// defaults to 'aes-256'.
    #[qapi(name = "cipher-alg")]
    pub cipher_alg: Option<QCryptoCipherAlgorithm>,
    /// the cipher mode for data encryption Currently defaults
    /// to 'xts'
    #[qapi(name = "cipher-mode")]
    pub cipher_mode: Option<QCryptoCipherMode>,
    /// the initialization vector generator Currently defaults
    /// to 'plain64'
    #[qapi(name = "ivgen-alg")]
    pub ivgen_alg: Option<QCryptoIvGenAlgorithm>,
    /// the initialization vector generator hash Currently
    /// defaults to 'sha256'
    #[qapi(name = "ivgen-hash-alg")]
    pub ivgen_hash_alg: Option<QCryptoHashAlgorithm>,
    /// the master key hash algorithm Currently defaults to
    /// 'sha256'
    #[qapi(name = "hash-alg")]
    pub hash_alg: Option<QCryptoHashAlgorithm>,
    /// number of milliseconds to spend in PBKDF passphrase
    /// processing.  Currently defaults to 2000.  (since 2.8)
    #[qapi(name = "iter-time")]
    pub iter_time: Option<i64>,
}
pub enum QCryptoBlockOpenOptionsBranch {
    #[qapi(name = "qcow")]
    Qcow(QCryptoBlockOptionsQCow),
    #[qapi(name = "luks")]
    Luks(QCryptoBlockOptionsLuks),
}
/// The options that are available for all encryption formats when
/// opening an existing volume
#[qapi(name = "QCryptoBlockOpenOptions")]
#[qapi(since = "2.6")]
pub struct QCryptoBlockOpenOptions {
    /// the encryption format
    #[qapi(name = "format")]
    #[qapi(discriminator)]
    pub format: QCryptoBlockFormat,
    #[qapi(union)]
    pub u: Option<QCryptoBlockOpenOptionsBranch>,
}
pub enum QCryptoBlockCreateOptionsBranch {
    #[qapi(name = "qcow")]
    Qcow(QCryptoBlockOptionsQCow),
    #[qapi(name = "luks")]
    Luks(QCryptoBlockCreateOptionsLuks),
}
/// The options that are available for all encryption formats when
/// initializing a new volume
#[qapi(name = "QCryptoBlockCreateOptions")]
#[qapi(since = "2.6")]
pub struct QCryptoBlockCreateOptions {
    /// the encryption format
    #[qapi(name = "format")]
    #[qapi(discriminator)]
    pub format: QCryptoBlockFormat,
    #[qapi(union)]
    pub u: Option<QCryptoBlockCreateOptionsBranch>,
}
/// The common information that applies to all full disk encryption
/// formats
#[qapi(name = "QCryptoBlockInfoBase")]
#[qapi(since = "2.7")]
pub struct QCryptoBlockInfoBase {
    /// the encryption format
    #[qapi(name = "format")]
    pub format: QCryptoBlockFormat,
}
/// Information about the LUKS block encryption key slot options
#[qapi(name = "QCryptoBlockInfoLUKSSlot")]
#[qapi(since = "2.7")]
pub struct QCryptoBlockInfoLuksSlot {
    /// whether the key slot is currently in use
    #[qapi(name = "active")]
    pub active: bool,
    /// number of PBKDF2 iterations for key material
    #[qapi(name = "iters")]
    pub iters: Option<i64>,
    /// number of stripes for splitting key material
    #[qapi(name = "stripes")]
    pub stripes: Option<i64>,
    /// offset to the key material in bytes
    #[qapi(name = "key-offset")]
    pub key_offset: i64,
}
/// Information about the LUKS block encryption options
#[qapi(name = "QCryptoBlockInfoLUKS")]
#[qapi(since = "2.7")]
pub struct QCryptoBlockInfoLuks {
    /// the cipher algorithm for data encryption
    #[qapi(name = "cipher-alg")]
    pub cipher_alg: QCryptoCipherAlgorithm,
    /// the cipher mode for data encryption
    #[qapi(name = "cipher-mode")]
    pub cipher_mode: QCryptoCipherMode,
    /// the initialization vector generator
    #[qapi(name = "ivgen-alg")]
    pub ivgen_alg: QCryptoIvGenAlgorithm,
    /// the initialization vector generator hash
    #[qapi(name = "ivgen-hash-alg")]
    pub ivgen_hash_alg: Option<QCryptoHashAlgorithm>,
    /// the master key hash algorithm
    #[qapi(name = "hash-alg")]
    pub hash_alg: QCryptoHashAlgorithm,
    /// whether the LUKS header is detached (Since 9.0)
    #[qapi(name = "detached-header")]
    pub detached_header: bool,
    /// offset to the payload data in bytes
    #[qapi(name = "payload-offset")]
    pub payload_offset: i64,
    /// number of PBKDF2 iterations for key material
    #[qapi(name = "master-key-iters")]
    pub master_key_iters: i64,
    /// unique identifier for the volume
    #[qapi(name = "uuid")]
    pub uuid: String,
    /// information about each key slot
    #[qapi(name = "slots")]
    pub slots: Vec<QCryptoBlockInfoLuksSlot>,
}
pub enum QCryptoBlockInfoBranch {
    #[qapi(name = "luks")]
    Luks(QCryptoBlockInfoLuks),
}
/// Information about the block encryption options
#[qapi(name = "QCryptoBlockInfo")]
#[qapi(since = "2.7")]
pub struct QCryptoBlockInfo {
    /// the encryption format
    #[qapi(name = "format")]
    #[qapi(discriminator)]
    pub format: QCryptoBlockFormat,
    #[qapi(union)]
    pub u: Option<QCryptoBlockInfoBranch>,
}
/// Defines state of keyslots that are affected by the update
#[qapi(name = "QCryptoBlockLUKSKeyslotState")]
#[qapi(since = "5.1")]
pub enum QCryptoBlockLuksKeyslotState {
    /// The slots contain the given password and marked as active
    #[qapi(name = "active")]
    Active,
    /// The slots are erased (contain garbage) and marked as
    /// inactive
    #[qapi(name = "inactive")]
    Inactive,
}
/// This struct defines the update parameters that activate/de-activate
/// set of keyslots
#[qapi(name = "QCryptoBlockAmendOptionsLUKS")]
#[qapi(since = "5.1")]
pub struct QCryptoBlockAmendOptionsLuks {
    /// the desired state of the keyslots
    #[qapi(name = "state")]
    pub state: QCryptoBlockLuksKeyslotState,
    /// The ID of a QCryptoSecret object providing the password
    /// to be written into added active keyslots
    #[qapi(name = "new-secret")]
    pub new_secret: Option<String>,
    /// Optional (for deactivation only) If given will
    /// deactivate all keyslots that match password located in
    /// QCryptoSecret with this ID
    #[qapi(name = "old-secret")]
    pub old_secret: Option<String>,
    /// Optional.  ID of the keyslot to activate/deactivate.  For
    /// keyslot activation, keyslot should not be active already (this
    /// is unsafe to update an active keyslot), but possible if 'force'
    /// parameter is given.  If keyslot is not given, first free keyslot
    /// will be written.
    ///
    /// For keyslot deactivation, this parameter specifies the exact
    /// keyslot to deactivate
    #[qapi(name = "keyslot")]
    pub keyslot: Option<i64>,
    /// Optional (for activation only) Number of milliseconds to
    /// spend in PBKDF passphrase processing for the newly activated
    /// keyslot.  Currently defaults to 2000.
    #[qapi(name = "iter-time")]
    pub iter_time: Option<i64>,
    /// Optional.  The ID of a QCryptoSecret object providing the
    /// password to use to retrieve current master key.  Defaults to the
    /// same secret that was used to open the image
    #[qapi(name = "secret")]
    pub secret: Option<String>,
}
pub enum QCryptoBlockAmendOptionsBranch {
    #[qapi(name = "luks")]
    Luks(QCryptoBlockAmendOptionsLuks),
}
/// The options that are available for all encryption formats when
/// amending encryption settings
#[qapi(name = "QCryptoBlockAmendOptions")]
#[qapi(since = "5.1")]
pub struct QCryptoBlockAmendOptions {
    /// the encryption format
    #[qapi(name = "format")]
    #[qapi(discriminator)]
    pub format: QCryptoBlockFormat,
    #[qapi(union)]
    pub u: Option<QCryptoBlockAmendOptionsBranch>,
}
/// Properties for objects of classes derived from secret-common.
#[qapi(name = "SecretCommonProperties")]
#[qapi(since = "2.6")]
pub struct SecretCommonProperties {
    /// if true, the secret is loaded immediately when applying
    /// this option and will probably fail when processing the next
    /// option.  Don't use; only provided for compatibility.
    /// (default: false)
    #[qapi(name = "loaded")]
    #[qapi(feature = "deprecated")]
    pub loaded: Option<bool>,
    /// the data format that the secret is provided in
    /// (default: raw)
    #[qapi(name = "format")]
    pub format: Option<QCryptoSecretFormat>,
    /// the name of another secret that should be used to decrypt
    /// the provided data.  If not present, the data is assumed to be
    /// unencrypted.
    #[qapi(name = "keyid")]
    pub keyid: Option<String>,
    /// the random initialization vector used for encryption of this
    /// particular secret.  Should be a base64 encrypted string of the
    /// 16-byte IV.  Mandatory if @keyid is given.  Ignored if @keyid is
    /// absent.
    #[qapi(name = "iv")]
    pub iv: Option<String>,
}
/// Properties for secret objects.
///
/// Either @data or @file must be provided, but not both.
#[qapi(name = "SecretProperties")]
#[qapi(since = "2.6")]
pub struct SecretProperties {
    /// if true, the secret is loaded immediately when applying
    /// this option and will probably fail when processing the next
    /// option.  Don't use; only provided for compatibility.
    /// (default: false)
    #[qapi(name = "loaded")]
    #[qapi(feature = "deprecated")]
    pub loaded: Option<bool>,
    /// the data format that the secret is provided in
    /// (default: raw)
    #[qapi(name = "format")]
    pub format: Option<QCryptoSecretFormat>,
    /// the name of another secret that should be used to decrypt
    /// the provided data.  If not present, the data is assumed to be
    /// unencrypted.
    #[qapi(name = "keyid")]
    pub keyid: Option<String>,
    /// the random initialization vector used for encryption of this
    /// particular secret.  Should be a base64 encrypted string of the
    /// 16-byte IV.  Mandatory if @keyid is given.  Ignored if @keyid is
    /// absent.
    #[qapi(name = "iv")]
    pub iv: Option<String>,
    /// the associated with the secret from
    #[qapi(name = "data")]
    pub data: Option<String>,
    /// the filename to load the data associated with the secret from
    #[qapi(name = "file")]
    pub file: Option<String>,
}
/// Properties for secret_keyring objects.
#[qapi(name = "SecretKeyringProperties")]
#[qapi(condition = "CONFIG_SECRET_KEYRING")]
#[qapi(since = "5.1")]
pub struct SecretKeyringProperties {
    /// if true, the secret is loaded immediately when applying
    /// this option and will probably fail when processing the next
    /// option.  Don't use; only provided for compatibility.
    /// (default: false)
    #[qapi(name = "loaded")]
    #[qapi(feature = "deprecated")]
    pub loaded: Option<bool>,
    /// the data format that the secret is provided in
    /// (default: raw)
    #[qapi(name = "format")]
    pub format: Option<QCryptoSecretFormat>,
    /// the name of another secret that should be used to decrypt
    /// the provided data.  If not present, the data is assumed to be
    /// unencrypted.
    #[qapi(name = "keyid")]
    pub keyid: Option<String>,
    /// the random initialization vector used for encryption of this
    /// particular secret.  Should be a base64 encrypted string of the
    /// 16-byte IV.  Mandatory if @keyid is given.  Ignored if @keyid is
    /// absent.
    #[qapi(name = "iv")]
    pub iv: Option<String>,
    /// serial number that identifies a key to get from the kernel
    #[qapi(name = "serial")]
    pub serial: i32,
}
/// Properties for objects of classes derived from tls-creds.
#[qapi(name = "TlsCredsProperties")]
#[qapi(since = "2.5")]
pub struct TlsCredsProperties {
    /// if true the peer credentials will be verified once the
    /// handshake is completed.  This is a no-op for anonymous
    /// credentials.  (default: true)
    #[qapi(name = "verify-peer")]
    pub verify_peer: Option<bool>,
    /// the path of the directory that contains the credential files
    #[qapi(name = "dir")]
    pub dir: Option<String>,
    /// whether the QEMU network backend that uses the
    /// credentials will be acting as a client or as a server
    /// (default: client)
    #[qapi(name = "endpoint")]
    pub endpoint: Option<QCryptoTlsCredsEndpoint>,
    /// a gnutls priority string as described at
    /// https://gnutls.org/manual/html_node/Priority-Strings.html
    #[qapi(name = "priority")]
    pub priority: Option<String>,
}
/// Properties for tls-creds-anon objects.
#[qapi(name = "TlsCredsAnonProperties")]
#[qapi(since = "2.5")]
pub struct TlsCredsAnonProperties {
    /// if true the peer credentials will be verified once the
    /// handshake is completed.  This is a no-op for anonymous
    /// credentials.  (default: true)
    #[qapi(name = "verify-peer")]
    pub verify_peer: Option<bool>,
    /// the path of the directory that contains the credential files
    #[qapi(name = "dir")]
    pub dir: Option<String>,
    /// whether the QEMU network backend that uses the
    /// credentials will be acting as a client or as a server
    /// (default: client)
    #[qapi(name = "endpoint")]
    pub endpoint: Option<QCryptoTlsCredsEndpoint>,
    /// a gnutls priority string as described at
    /// https://gnutls.org/manual/html_node/Priority-Strings.html
    #[qapi(name = "priority")]
    pub priority: Option<String>,
    /// if true, the credentials are loaded immediately when
    /// applying this option and will ignore options that are processed
    /// later.  Don't use; only provided for compatibility.
    /// (default: false)
    #[qapi(name = "loaded")]
    #[qapi(feature = "deprecated")]
    pub loaded: Option<bool>,
}
/// Properties for tls-creds-psk objects.
#[qapi(name = "TlsCredsPskProperties")]
#[qapi(since = "3.0")]
pub struct TlsCredsPskProperties {
    /// if true the peer credentials will be verified once the
    /// handshake is completed.  This is a no-op for anonymous
    /// credentials.  (default: true)
    #[qapi(name = "verify-peer")]
    pub verify_peer: Option<bool>,
    /// the path of the directory that contains the credential files
    #[qapi(name = "dir")]
    pub dir: Option<String>,
    /// whether the QEMU network backend that uses the
    /// credentials will be acting as a client or as a server
    /// (default: client)
    #[qapi(name = "endpoint")]
    pub endpoint: Option<QCryptoTlsCredsEndpoint>,
    /// a gnutls priority string as described at
    /// https://gnutls.org/manual/html_node/Priority-Strings.html
    #[qapi(name = "priority")]
    pub priority: Option<String>,
    /// if true, the credentials are loaded immediately when
    /// applying this option and will ignore options that are processed
    /// later.  Don't use; only provided for compatibility.
    /// (default: false)
    #[qapi(name = "loaded")]
    #[qapi(feature = "deprecated")]
    pub loaded: Option<bool>,
    /// the username which will be sent to the server.  For
    /// clients only.  If absent, "qemu" is sent and the property will
    /// read back as an empty string.
    #[qapi(name = "username")]
    pub username: Option<String>,
}
/// Properties for tls-creds-x509 objects.
#[qapi(name = "TlsCredsX509Properties")]
#[qapi(since = "2.5")]
pub struct TlsCredsX509Properties {
    /// if true the peer credentials will be verified once the
    /// handshake is completed.  This is a no-op for anonymous
    /// credentials.  (default: true)
    #[qapi(name = "verify-peer")]
    pub verify_peer: Option<bool>,
    /// the path of the directory that contains the credential files
    #[qapi(name = "dir")]
    pub dir: Option<String>,
    /// whether the QEMU network backend that uses the
    /// credentials will be acting as a client or as a server
    /// (default: client)
    #[qapi(name = "endpoint")]
    pub endpoint: Option<QCryptoTlsCredsEndpoint>,
    /// a gnutls priority string as described at
    /// https://gnutls.org/manual/html_node/Priority-Strings.html
    #[qapi(name = "priority")]
    pub priority: Option<String>,
    /// if true, the credentials are loaded immediately when
    /// applying this option and will ignore options that are processed
    /// later.  Don't use; only provided for compatibility.
    /// (default: false)
    #[qapi(name = "loaded")]
    #[qapi(feature = "deprecated")]
    pub loaded: Option<bool>,
    /// if true, perform some sanity checks before using the
    /// credentials (default: true)
    #[qapi(name = "sanity-check")]
    pub sanity_check: Option<bool>,
    /// For the server-key.pem and client-key.pem files which
    /// contain sensitive private keys, it is possible to use an
    /// encrypted version by providing the @passwordid parameter.  This
    /// provides the ID of a previously created secret object containing
    /// the password for decryption.
    #[qapi(name = "passwordid")]
    pub passwordid: Option<String>,
}
/// The supported algorithms for asymmetric encryption ciphers
#[qapi(name = "QCryptoAkCipherAlgorithm")]
#[qapi(since = "7.1")]
pub enum QCryptoAkCipherAlgorithm {
    /// RSA algorithm
    #[qapi(name = "rsa")]
    Rsa,
}
/// The type of asymmetric keys.
#[qapi(name = "QCryptoAkCipherKeyType")]
#[qapi(since = "7.1")]
pub enum QCryptoAkCipherKeyType {
    #[qapi(name = "public")]
    Public,
    #[qapi(name = "private")]
    Private,
}
/// The padding algorithm for RSA.
#[qapi(name = "QCryptoRSAPaddingAlgorithm")]
#[qapi(since = "7.1")]
pub enum QCryptoRsaPaddingAlgorithm {
    /// no padding used
    #[qapi(name = "raw")]
    Raw,
    /// pkcs1#v1.5
    #[qapi(name = "pkcs1")]
    Pkcs1,
}
/// Specific parameters for RSA algorithm.
#[qapi(name = "QCryptoAkCipherOptionsRSA")]
#[qapi(since = "7.1")]
pub struct QCryptoAkCipherOptionsRsa {
    /// QCryptoHashAlgorithm
    #[qapi(name = "hash-alg")]
    pub hash_alg: QCryptoHashAlgorithm,
    /// QCryptoRSAPaddingAlgorithm
    #[qapi(name = "padding-alg")]
    pub padding_alg: QCryptoRsaPaddingAlgorithm,
}
pub enum QCryptoAkCipherOptionsBranch {
    #[qapi(name = "rsa")]
    Rsa(QCryptoAkCipherOptionsRsa),
}
/// The options that are available for all asymmetric key algorithms
/// when creating a new QCryptoAkCipher.
#[qapi(name = "QCryptoAkCipherOptions")]
#[qapi(since = "7.1")]
pub struct QCryptoAkCipherOptions {
    /// encryption cipher algorithm
    #[qapi(name = "alg")]
    #[qapi(discriminator)]
    pub alg: QCryptoAkCipherAlgorithm,
    #[qapi(union)]
    pub u: Option<QCryptoAkCipherOptionsBranch>,
}
// path end:	qapi/crypto.json
// path begin:	qapi/job.json
/// Type of a background job.
#[qapi(name = "JobType")]
#[qapi(since = "1.7")]
pub enum JobType {
    /// block commit job type, see "block-commit"
    #[qapi(name = "commit")]
    Commit,
    /// block stream job type, see "block-stream"
    #[qapi(name = "stream")]
    Stream,
    /// drive mirror job type, see "drive-mirror"
    #[qapi(name = "mirror")]
    Mirror,
    /// drive backup job type, see "drive-backup"
    #[qapi(name = "backup")]
    Backup,
    /// image creation job type, see "blockdev-create" (since 3.0)
    #[qapi(name = "create")]
    Create,
    /// image options amend job type, see "x-blockdev-amend" (since
    /// 5.1)
    #[qapi(name = "amend")]
    Amend,
    /// snapshot load job type, see "snapshot-load" (since
    /// 6.0)
    #[qapi(name = "snapshot-load")]
    SnapshotLoad,
    /// snapshot save job type, see "snapshot-save" (since
    /// 6.0)
    #[qapi(name = "snapshot-save")]
    SnapshotSave,
    /// snapshot delete job type, see "snapshot-delete"
    /// (since 6.0)
    #[qapi(name = "snapshot-delete")]
    SnapshotDelete,
}
/// Indicates the present state of a given job in its lifetime.
#[qapi(name = "JobStatus")]
#[qapi(since = "2.12")]
pub enum JobStatus {
    /// Erroneous, default state.  Should not ever be visible.
    #[qapi(name = "undefined")]
    Undefined,
    /// The job has been created, but not yet started.
    #[qapi(name = "created")]
    Created,
    /// The job is currently running.
    #[qapi(name = "running")]
    Running,
    /// The job is running, but paused.  The pause may be requested
    /// by either the QMP user or by internal processes.
    #[qapi(name = "paused")]
    Paused,
    /// The job is running, but is ready for the user to signal
    /// completion.  This is used for long-running jobs like mirror that
    /// are designed to run indefinitely.
    #[qapi(name = "ready")]
    Ready,
    /// The job is ready, but paused.  This is nearly identical to
    /// @paused.  The job may return to @ready or otherwise be canceled.
    #[qapi(name = "standby")]
    Standby,
    /// The job is waiting for other jobs in the transaction to
    /// converge to the waiting state.  This status will likely not be
    /// visible for the last job in a transaction.
    #[qapi(name = "waiting")]
    Waiting,
    /// The job has finished its work, but has finalization steps
    /// that it needs to make prior to completing.  These changes will
    /// require manual intervention via @job-finalize if auto-finalize
    /// was set to false.  These pending changes may still fail.
    #[qapi(name = "pending")]
    Pending,
    /// The job is in the process of being aborted, and will
    /// finish with an error.  The job will afterwards report that it is
    /// @concluded.  This status may not be visible to the management
    /// process.
    #[qapi(name = "aborting")]
    Aborting,
    /// The job has finished all work.  If auto-dismiss was set
    /// to false, the job will remain in the query list until it is
    /// dismissed via @job-dismiss.
    #[qapi(name = "concluded")]
    Concluded,
    /// The job is in the process of being dismantled.  This state
    /// should not ever be visible externally.
    #[qapi(name = "null")]
    Null,
}
/// Represents command verbs that can be applied to a job.
#[qapi(name = "JobVerb")]
#[qapi(since = "2.12")]
pub enum JobVerb {
    /// see @job-cancel
    #[qapi(name = "cancel")]
    Cancel,
    /// see @job-pause
    #[qapi(name = "pause")]
    Pause,
    /// see @job-resume
    #[qapi(name = "resume")]
    Resume,
    /// see @block-job-set-speed
    #[qapi(name = "set-speed")]
    SetSpeed,
    /// see @job-complete
    #[qapi(name = "complete")]
    Complete,
    /// see @job-dismiss
    #[qapi(name = "dismiss")]
    Dismiss,
    /// see @job-finalize
    #[qapi(name = "finalize")]
    Finalize,
    /// see @block-job-change (since 8.2)
    #[qapi(name = "change")]
    Change,
}
/// Emitted when a job transitions to a different status.
#[qapi(name = "JOB_STATUS_CHANGE")]
#[qapi(since = "3.0")]
pub struct JobStatusChange {
    /// The job identifier
    #[qapi(name = "id")]
    pub id: String,
    /// The new job status
    #[qapi(name = "status")]
    pub status: JobStatus,
}
/// Pause an active job.
///
/// This command returns immediately after marking the active job for
/// pausing.  Pausing an already paused job is an error.
///
/// The job will pause as soon as possible, which means transitioning
/// into the PAUSED state if it was RUNNING, or into STANDBY if it was
/// READY.  The corresponding JOB_STATUS_CHANGE event will be emitted.
///
/// Cancelling a paused job automatically resumes it.
#[qapi(name = "job-pause")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
pub struct JobPause {
    /// The job identifier.
    #[qapi(name = "id")]
    pub id: String,
}
/// Resume a paused job.
///
/// This command returns immediately after resuming a paused job.
/// Resuming an already running job is an error.
#[qapi(name = "job-resume")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
pub struct JobResume {
    /// The job identifier.
    #[qapi(name = "id")]
    pub id: String,
}
/// Instruct an active background job to cancel at the next opportunity.
/// This command returns immediately after marking the active job for
/// cancellation.
///
/// The job will cancel as soon as possible and then emit a
/// JOB_STATUS_CHANGE event.  Usually, the status will change to
/// ABORTING, but it is possible that a job successfully completes (e.g.
/// because it was almost done and there was no opportunity to cancel
/// earlier than completing the job) and transitions to PENDING instead.
#[qapi(name = "job-cancel")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
pub struct JobCancel {
    /// The job identifier.
    #[qapi(name = "id")]
    pub id: String,
}
/// Manually trigger completion of an active job in the READY state.
#[qapi(name = "job-complete")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
pub struct JobComplete {
    /// The job identifier.
    #[qapi(name = "id")]
    pub id: String,
}
/// Deletes a job that is in the CONCLUDED state.  This command only
/// needs to be run explicitly for jobs that don't have automatic
/// dismiss enabled.
///
/// This command will refuse to operate on any job that has not yet
/// reached its terminal state, JOB_STATUS_CONCLUDED.  For jobs that
/// make use of JOB_READY event, job-cancel or job-complete will still
/// need to be used as appropriate.
#[qapi(name = "job-dismiss")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
pub struct JobDismiss {
    /// The job identifier.
    #[qapi(name = "id")]
    pub id: String,
}
/// Instructs all jobs in a transaction (or a single job if it is not
/// part of any transaction) to finalize any graph changes and do any
/// necessary cleanup.  This command requires that all involved jobs are
/// in the PENDING state.
///
/// For jobs in a transaction, instructing one job to finalize will
/// force ALL jobs in the transaction to finalize, so it is only
/// necessary to instruct a single member job to finalize.
#[qapi(name = "job-finalize")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
pub struct JobFinalize {
    /// The identifier of any job in the transaction, or of a job that
    /// is not part of any transaction.
    #[qapi(name = "id")]
    pub id: String,
}
/// Information about a job.
#[qapi(name = "JobInfo")]
#[qapi(since = "3.0")]
pub struct JobInfo {
    /// The job identifier
    #[qapi(name = "id")]
    pub id: String,
    /// The kind of job that is being performed
    #[qapi(name = "type")]
    pub r#type: JobType,
    /// Current job state/status
    #[qapi(name = "status")]
    pub status: JobStatus,
    /// Progress made until now.  The unit is arbitrary
    /// and the value can only meaningfully be used for the ratio of
    /// @current-progress to @total-progress.  The value is
    /// monotonically increasing.
    #[qapi(name = "current-progress")]
    pub current_progress: i64,
    /// Estimated @current-progress value at the completion
    /// of the job.  This value can arbitrarily change while the job is
    /// running, in both directions.
    #[qapi(name = "total-progress")]
    pub total_progress: i64,
    /// If this field is present, the job failed; if it is still
    /// missing in the CONCLUDED state, this indicates successful
    /// completion.
    ///
    /// The value is a human-readable error message to describe the
    /// reason for the job failure.  It should not be parsed by
    /// applications.
    #[qapi(name = "error")]
    pub error: Option<String>,
}
/// Return information about jobs.
#[qapi(name = "query-jobs")]
#[qapi(since = "3.0")]
#[qapi(returns = "Vec<JobInfo>")]
pub struct QueryJobs {}
// path end:	qapi/job.json
// path begin:	qapi/block.json
/// Policy that BIOS should use to interpret cylinder/head/sector
/// addresses.  Note that Bochs BIOS and SeaBIOS will not actually
/// translate logical CHS to physical; instead, they will use logical
/// block addressing.
#[qapi(name = "BiosAtaTranslation")]
#[qapi(since = "2.0")]
pub enum BiosAtaTranslation {
    /// If cylinder/heads/sizes are passed, choose between none and
    /// LBA depending on the size of the disk.  If they are not passed,
    /// choose none if QEMU can guess that the disk had 16 or fewer
    /// heads, large if QEMU can guess that the disk had 131072 or fewer
    /// tracks across all heads (i.e. cylinders*heads<131072), otherwise
    /// LBA.
    #[qapi(name = "auto")]
    Auto,
    /// The physical disk geometry is equal to the logical geometry.
    #[qapi(name = "none")]
    None,
    /// Assume 63 sectors per track and one of 16, 32, 64, 128 or 255
    /// heads (if fewer than 255 are enough to cover the whole disk with
    /// 1024 cylinders/head).  The number of cylinders/head is then
    /// computed based on the number of sectors and heads.
    #[qapi(name = "lba")]
    Lba,
    /// The number of cylinders per head is scaled down to 1024 by
    /// correspondingly scaling up the number of heads.
    #[qapi(name = "large")]
    Large,
    /// Same as @large, but first convert a 16-head geometry to
    /// 15-head, by proportionally scaling up the number of
    /// cylinders/head.
    #[qapi(name = "rechs")]
    Rechs,
}
/// Type of Floppy drive to be emulated by the Floppy Disk Controller.
#[qapi(name = "FloppyDriveType")]
#[qapi(since = "2.6")]
pub enum FloppyDriveType {
    /// 1.44MB 3.5" drive
    #[qapi(name = "144")]
    _144,
    /// 2.88MB 3.5" drive
    #[qapi(name = "288")]
    _288,
    /// 1.2MB 5.25" drive
    #[qapi(name = "120")]
    _120,
    /// No drive connected
    #[qapi(name = "none")]
    None,
    /// Automatically determined by inserted media at boot
    #[qapi(name = "auto")]
    Auto,
}
/// Information about a persistent reservation manager
#[qapi(name = "PRManagerInfo")]
#[qapi(since = "3.0")]
pub struct PrManagerInfo {
    /// the identifier of the persistent reservation manager
    #[qapi(name = "id")]
    pub id: String,
    /// true if the persistent reservation manager is connected
    /// to the underlying storage or helper
    #[qapi(name = "connected")]
    pub connected: bool,
}
/// Returns a list of information about each persistent reservation
/// manager.
#[qapi(name = "query-pr-managers")]
#[qapi(since = "3.0")]
#[qapi(returns = "Vec<PRManagerInfo>")]
#[qapi(allow_preconfig)]
pub struct QueryPrManagers {}
/// Ejects the medium from a removable drive.
#[qapi(name = "eject")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct Eject {
    /// Block device name
    #[qapi(name = "device")]
    #[qapi(feature = "deprecated")]
    pub device: Option<String>,
    /// The name or QOM path of the guest device (since: 2.8)
    #[qapi(name = "id")]
    pub id: Option<String>,
    /// If true, eject regardless of whether the drive is locked.
    /// If not specified, the default value is false.
    #[qapi(name = "force")]
    pub force: Option<bool>,
}
/// Opens a block device's tray.  If there is a block driver state tree
/// inserted as a medium, it will become inaccessible to the guest (but
/// it will remain associated to the block device, so closing the tray
/// will make it accessible again).
///
/// If the tray was already open before, this will be a no-op.
///
/// Once the tray opens, a DEVICE_TRAY_MOVED event is emitted.  There
/// are cases in which no such event will be generated, these include:
///
/// - if the guest has locked the tray, @force is false and the guest
/// does not respond to the eject request
/// - if the BlockBackend denoted by @device does not have a guest
/// device attached to it
/// - if the guest device does not have an actual tray
#[qapi(name = "blockdev-open-tray")]
#[qapi(since = "2.5")]
#[qapi(returns = "()")]
pub struct BlockdevOpenTray {
    /// Block device name
    #[qapi(name = "device")]
    #[qapi(feature = "deprecated")]
    pub device: Option<String>,
    /// The name or QOM path of the guest device (since: 2.8)
    #[qapi(name = "id")]
    pub id: Option<String>,
    /// if false (the default), an eject request will be sent to the
    /// guest if it has locked the tray (and the tray will not be opened
    /// immediately); if true, the tray will be opened regardless of
    /// whether it is locked
    #[qapi(name = "force")]
    pub force: Option<bool>,
}
/// Closes a block device's tray.  If there is a block driver state tree
/// associated with the block device (which is currently ejected), that
/// tree will be loaded as the medium.
///
/// If the tray was already closed before, this will be a no-op.
#[qapi(name = "blockdev-close-tray")]
#[qapi(since = "2.5")]
#[qapi(returns = "()")]
pub struct BlockdevCloseTray {
    /// Block device name
    #[qapi(name = "device")]
    #[qapi(feature = "deprecated")]
    pub device: Option<String>,
    /// The name or QOM path of the guest device (since: 2.8)
    #[qapi(name = "id")]
    pub id: Option<String>,
}
/// Removes a medium (a block driver state tree) from a block device.
/// That block device's tray must currently be open (unless there is no
/// attached guest device).
///
/// If the tray is open and there is no medium inserted, this will be a
/// no-op.
#[qapi(name = "blockdev-remove-medium")]
#[qapi(since = "2.12")]
#[qapi(returns = "()")]
pub struct BlockdevRemoveMedium {
    /// The name or QOM path of the guest device
    #[qapi(name = "id")]
    pub id: String,
}
/// Inserts a medium (a block driver state tree) into a block device.
/// That block device's tray must currently be open (unless there is no
/// attached guest device) and there must be no medium inserted already.
#[qapi(name = "blockdev-insert-medium")]
#[qapi(since = "2.12")]
#[qapi(returns = "()")]
pub struct BlockdevInsertMedium {
    /// The name or QOM path of the guest device
    #[qapi(name = "id")]
    pub id: String,
    /// name of a node in the block driver state graph
    #[qapi(name = "node-name")]
    pub node_name: String,
}
/// Specifies the new read-only mode of a block device subject to the
/// @blockdev-change-medium command.
#[qapi(name = "BlockdevChangeReadOnlyMode")]
#[qapi(since = "2.3")]
pub enum BlockdevChangeReadOnlyMode {
    /// Retains the current read-only mode
    #[qapi(name = "retain")]
    Retain,
    /// Makes the device read-only
    #[qapi(name = "read-only")]
    ReadOnly,
    /// Makes the device writable
    #[qapi(name = "read-write")]
    ReadWrite,
}
/// Changes the medium inserted into a block device by ejecting the
/// current medium and loading a new image file which is inserted as the
/// new medium (this command combines blockdev-open-tray,
/// blockdev-remove-medium, blockdev-insert-medium and
/// blockdev-close-tray).
#[qapi(name = "blockdev-change-medium")]
#[qapi(since = "2.5")]
#[qapi(returns = "()")]
pub struct BlockdevChangeMedium {
    /// Block device name
    #[qapi(name = "device")]
    #[qapi(feature = "deprecated")]
    pub device: Option<String>,
    /// The name or QOM path of the guest device (since: 2.8)
    #[qapi(name = "id")]
    pub id: Option<String>,
    /// filename of the new image to be loaded
    #[qapi(name = "filename")]
    pub filename: String,
    /// format to open the new image with (defaults to the probed
    /// format)
    #[qapi(name = "format")]
    pub format: Option<String>,
    /// if false (the default), an eject request through
    /// blockdev-open-tray will be sent to the guest if it has locked
    /// the tray (and the tray will not be opened immediately); if true,
    /// the tray will be opened regardless of whether it is locked.
    /// (since 7.1)
    #[qapi(name = "force")]
    pub force: Option<bool>,
    /// change the read-only mode of the device; defaults
    /// to 'retain'
    #[qapi(name = "read-only-mode")]
    pub read_only_mode: Option<BlockdevChangeReadOnlyMode>,
}
/// Emitted whenever the tray of a removable device is moved by the
/// guest or by HMP/QMP commands
#[qapi(name = "DEVICE_TRAY_MOVED")]
#[qapi(since = "1.1")]
pub struct DeviceTrayMoved {
    /// Block device name.  This is always present for
    /// compatibility reasons, but it can be empty ("") if the image
    /// does not have a device name associated.
    #[qapi(name = "device")]
    pub device: String,
    /// The name or QOM path of the guest device (since 2.8)
    #[qapi(name = "id")]
    pub id: String,
    /// true if the tray has been opened or false if it has been
    /// closed
    #[qapi(name = "tray-open")]
    pub tray_open: bool,
}
/// Emitted whenever the connected status of a persistent reservation
/// manager changes.
#[qapi(name = "PR_MANAGER_STATUS_CHANGED")]
#[qapi(since = "3.0")]
pub struct PrManagerStatusChanged {
    /// The id of the PR manager object
    #[qapi(name = "id")]
    pub id: String,
    /// true if the PR manager is connected to a backend
    #[qapi(name = "connected")]
    pub connected: bool,
}
/// Change I/O throttle limits for a block drive.
///
/// Since QEMU 2.4, each device with I/O limits is member of a throttle
/// group.
///
/// If two or more devices are members of the same group, the limits
/// will apply to the combined I/O of the whole group in a round-robin
/// fashion.  Therefore, setting new I/O limits to a device will affect
/// the whole group.
///
/// The name of the group can be specified using the 'group' parameter.
/// If the parameter is unset, it is assumed to be the current group of
/// that device.  If it's not in any group yet, the name of the device
/// will be used as the name for its group.
///
/// The 'group' parameter can also be used to move a device to a
/// different group.  In this case the limits specified in the
/// parameters will be applied to the new group only.
///
/// I/O limits can be disabled by setting all of them to 0.  In this
/// case the device will be removed from its group and the rest of its
/// members will not be affected.  The 'group' parameter is ignored.
#[qapi(name = "block_set_io_throttle")]
#[qapi(since = "1.1")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockSetIoThrottle {
    #[qapi(flatten)]
    pub data: BlockIoThrottle,
}
/// Manage read, write and flush latency histograms for the device.
///
/// If only @id parameter is specified, remove all present latency
/// histograms for the device.  Otherwise, add/reset some of (or all)
/// latency histograms.
#[qapi(name = "block-latency-histogram-set")]
#[qapi(since = "4.0")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockLatencyHistogramSet {
    /// The name or QOM path of the guest device.
    #[qapi(name = "id")]
    pub id: String,
    /// list of interval boundary values (see description in
    /// BlockLatencyHistogramInfo definition).  If specified, all
    /// latency histograms are removed, and empty ones created for all
    /// io types with intervals corresponding to @boundaries (except for
    /// io types, for which specific boundaries are set through the
    /// following parameters).
    #[qapi(name = "boundaries")]
    pub boundaries: Option<Vec<u64>>,
    /// list of interval boundary values for read latency
    /// histogram.  If specified, old read latency histogram is removed,
    /// and empty one created with intervals corresponding to
    /// @boundaries-read.  The parameter has higher priority then
    /// @boundaries.
    #[qapi(name = "boundaries-read")]
    pub boundaries_read: Option<Vec<u64>>,
    /// list of interval boundary values for write
    /// latency histogram.
    #[qapi(name = "boundaries-write")]
    pub boundaries_write: Option<Vec<u64>>,
    /// list of interval boundary values for zone append
    /// write latency histogram.
    #[qapi(name = "boundaries-zap")]
    pub boundaries_zap: Option<Vec<u64>>,
    /// list of interval boundary values for flush
    /// latency histogram.
    #[qapi(name = "boundaries-flush")]
    pub boundaries_flush: Option<Vec<u64>>,
}
// path end:	qapi/block.json
// path begin:	qapi/block-core.json
#[qapi(name = "SnapshotInfo")]
#[qapi(since = "1.3")]
pub struct SnapshotInfo {
    /// unique snapshot id
    #[qapi(name = "id")]
    pub id: String,
    /// user chosen name
    #[qapi(name = "name")]
    pub name: String,
    /// size of the VM state
    #[qapi(name = "vm-state-size")]
    pub vm_state_size: i64,
    /// UTC date of the snapshot in seconds
    #[qapi(name = "date-sec")]
    pub date_sec: i64,
    /// fractional part in nano seconds to be used with date-sec
    #[qapi(name = "date-nsec")]
    pub date_nsec: i64,
    /// VM clock relative to boot in seconds
    #[qapi(name = "vm-clock-sec")]
    pub vm_clock_sec: i64,
    /// fractional part in nano seconds to be used with
    /// vm-clock-sec
    #[qapi(name = "vm-clock-nsec")]
    pub vm_clock_nsec: i64,
    /// Current instruction count.  Appears when execution
    /// record/replay is enabled.  Used for "time-traveling" to match
    /// the moment in the recorded execution with the snapshots.  This
    /// counter may be obtained through @query-replay command (since
    /// 5.2)
    #[qapi(name = "icount")]
    pub icount: Option<i64>,
}
#[qapi(name = "ImageInfoSpecificQCow2EncryptionBase")]
#[qapi(since = "2.10")]
pub struct ImageInfoSpecificQCow2EncryptionBase {
    /// The encryption format
    #[qapi(name = "format")]
    pub format: BlockdevQcow2EncryptionFormat,
}
pub enum ImageInfoSpecificQCow2EncryptionBranch {
    #[qapi(name = "luks")]
    Luks(QCryptoBlockInfoLuks),
}
#[qapi(name = "ImageInfoSpecificQCow2Encryption")]
#[qapi(since = "2.10")]
pub struct ImageInfoSpecificQCow2Encryption {
    /// The encryption format
    #[qapi(name = "format")]
    #[qapi(discriminator)]
    pub format: BlockdevQcow2EncryptionFormat,
    #[qapi(union)]
    pub u: Option<ImageInfoSpecificQCow2EncryptionBranch>,
}
#[qapi(name = "ImageInfoSpecificQCow2")]
#[qapi(since = "1.7")]
pub struct ImageInfoSpecificQCow2 {
    /// compatibility level
    #[qapi(name = "compat")]
    pub compat: String,
    /// the filename of the external data file that is stored in
    /// the image and used as a default for opening the image
    /// (since: 4.0)
    #[qapi(name = "data-file")]
    pub data_file: Option<String>,
    /// True if the external data file must stay valid as a
    /// standalone (read-only) raw image without looking at qcow2
    /// metadata (since: 4.0)
    #[qapi(name = "data-file-raw")]
    pub data_file_raw: Option<bool>,
    /// true if the image has extended L2 entries; only valid
    /// for compat >= 1.1 (since 5.2)
    #[qapi(name = "extended-l2")]
    pub extended_l2: Option<bool>,
    /// on or off; only valid for compat >= 1.1
    #[qapi(name = "lazy-refcounts")]
    pub lazy_refcounts: Option<bool>,
    /// true if the image has been marked corrupt; only valid for
    /// compat >= 1.1 (since 2.2)
    #[qapi(name = "corrupt")]
    pub corrupt: Option<bool>,
    /// width of a refcount entry in bits (since 2.3)
    #[qapi(name = "refcount-bits")]
    pub refcount_bits: i64,
    /// details about encryption parameters; only set if image is
    /// encrypted (since 2.10)
    #[qapi(name = "encrypt")]
    pub encrypt: Option<ImageInfoSpecificQCow2Encryption>,
    /// A list of qcow2 bitmap details (since 4.0)
    #[qapi(name = "bitmaps")]
    pub bitmaps: Option<Vec<Qcow2BitmapInfo>>,
    /// the image cluster compression method (since 5.1)
    #[qapi(name = "compression-type")]
    pub compression_type: Qcow2CompressionType,
}
#[qapi(name = "ImageInfoSpecificVmdk")]
#[qapi(since = "1.7")]
pub struct ImageInfoSpecificVmdk {
    /// The create type of VMDK image
    #[qapi(name = "create-type")]
    pub create_type: String,
    /// Content id of image
    #[qapi(name = "cid")]
    pub cid: i64,
    /// Parent VMDK image's cid
    #[qapi(name = "parent-cid")]
    pub parent_cid: i64,
    /// List of extent files
    #[qapi(name = "extents")]
    pub extents: Vec<VmdkExtentInfo>,
}
/// Information about a VMDK extent file
#[qapi(name = "VmdkExtentInfo")]
#[qapi(since = "8.0")]
pub struct VmdkExtentInfo {
    /// Name of the extent file
    #[qapi(name = "filename")]
    pub filename: String,
    /// Extent type (e.g. FLAT or SPARSE)
    #[qapi(name = "format")]
    pub format: String,
    /// Number of bytes covered by this extent
    #[qapi(name = "virtual-size")]
    pub virtual_size: i64,
    /// Cluster size in bytes (for non-flat extents)
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<i64>,
    /// Whether this extent contains compressed data
    #[qapi(name = "compressed")]
    pub compressed: Option<bool>,
}
#[qapi(name = "ImageInfoSpecificRbd")]
#[qapi(since = "6.1")]
pub struct ImageInfoSpecificRbd {
    /// Image encryption format
    #[qapi(name = "encryption-format")]
    pub encryption_format: Option<RbdImageEncryptionFormat>,
}
#[qapi(name = "ImageInfoSpecificFile")]
#[qapi(since = "8.0")]
pub struct ImageInfoSpecificFile {
    /// Extent size hint (if available)
    #[qapi(name = "extent-size-hint")]
    pub extent_size_hint: Option<u64>,
}
#[qapi(name = "ImageInfoSpecificKind")]
#[qapi(since = "1.7")]
pub enum ImageInfoSpecificKind {
    #[qapi(name = "qcow2")]
    Qcow2,
    #[qapi(name = "vmdk")]
    Vmdk,
    /// Since 2.7
    #[qapi(name = "luks")]
    Luks,
    /// Since 6.1
    #[qapi(name = "rbd")]
    Rbd,
    /// Since 8.0
    #[qapi(name = "file")]
    File,
}
#[qapi(name = "ImageInfoSpecificQCow2Wrapper")]
#[qapi(since = "1.7")]
pub struct ImageInfoSpecificQCow2Wrapper {
    /// image information specific to QCOW2
    #[qapi(name = "data")]
    pub data: ImageInfoSpecificQCow2,
}
#[qapi(name = "ImageInfoSpecificVmdkWrapper")]
#[qapi(since = "6.1")]
pub struct ImageInfoSpecificVmdkWrapper {
    /// image information specific to VMDK
    #[qapi(name = "data")]
    pub data: ImageInfoSpecificVmdk,
}
#[qapi(name = "ImageInfoSpecificLUKSWrapper")]
#[qapi(since = "2.7")]
pub struct ImageInfoSpecificLuksWrapper {
    /// image information specific to LUKS
    #[qapi(name = "data")]
    pub data: QCryptoBlockInfoLuks,
}
#[qapi(name = "ImageInfoSpecificRbdWrapper")]
#[qapi(since = "6.1")]
pub struct ImageInfoSpecificRbdWrapper {
    /// image information specific to RBD
    #[qapi(name = "data")]
    pub data: ImageInfoSpecificRbd,
}
#[qapi(name = "ImageInfoSpecificFileWrapper")]
#[qapi(since = "8.0")]
pub struct ImageInfoSpecificFileWrapper {
    /// image information specific to files
    #[qapi(name = "data")]
    pub data: ImageInfoSpecificFile,
}
pub enum ImageInfoSpecificBranch {
    #[qapi(name = "qcow2")]
    Qcow2(ImageInfoSpecificQCow2Wrapper),
    #[qapi(name = "vmdk")]
    Vmdk(ImageInfoSpecificVmdkWrapper),
    #[qapi(name = "luks")]
    Luks(ImageInfoSpecificLuksWrapper),
    #[qapi(name = "rbd")]
    Rbd(ImageInfoSpecificRbdWrapper),
    #[qapi(name = "file")]
    File(ImageInfoSpecificFileWrapper),
}
/// A discriminated record of image format specific information
/// structures.
#[qapi(name = "ImageInfoSpecific")]
#[qapi(since = "1.7")]
pub struct ImageInfoSpecific {
    /// block driver name
    #[qapi(name = "type")]
    #[qapi(discriminator)]
    pub r#type: ImageInfoSpecificKind,
    #[qapi(union)]
    pub u: Option<ImageInfoSpecificBranch>,
}
/// Information about a QEMU image file
#[qapi(name = "BlockNodeInfo")]
#[qapi(since = "8.0")]
pub struct BlockNodeInfo {
    /// name of the image file
    #[qapi(name = "filename")]
    pub filename: String,
    /// format of the image file
    #[qapi(name = "format")]
    pub format: String,
    /// true if image is not cleanly closed
    #[qapi(name = "dirty-flag")]
    pub dirty_flag: Option<bool>,
    /// actual size on disk in bytes of the image
    #[qapi(name = "actual-size")]
    pub actual_size: Option<i64>,
    /// maximum capacity in bytes of the image
    #[qapi(name = "virtual-size")]
    pub virtual_size: i64,
    /// size of a cluster in bytes
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<i64>,
    /// true if the image is encrypted
    #[qapi(name = "encrypted")]
    pub encrypted: Option<bool>,
    /// true if the image is compressed (Since 1.7)
    #[qapi(name = "compressed")]
    pub compressed: Option<bool>,
    /// name of the backing file
    #[qapi(name = "backing-filename")]
    pub backing_filename: Option<String>,
    /// full path of the backing file
    #[qapi(name = "full-backing-filename")]
    pub full_backing_filename: Option<String>,
    /// the format of the backing file
    #[qapi(name = "backing-filename-format")]
    pub backing_filename_format: Option<String>,
    /// list of VM snapshots
    #[qapi(name = "snapshots")]
    pub snapshots: Option<Vec<SnapshotInfo>>,
    /// structure supplying additional format-specific
    /// information (since 1.7)
    #[qapi(name = "format-specific")]
    pub format_specific: Option<ImageInfoSpecific>,
}
/// Information about a QEMU image file, and potentially its backing
/// image
#[qapi(name = "ImageInfo")]
#[qapi(since = "1.3")]
pub struct ImageInfo {
    /// name of the image file
    #[qapi(name = "filename")]
    pub filename: String,
    /// format of the image file
    #[qapi(name = "format")]
    pub format: String,
    /// true if image is not cleanly closed
    #[qapi(name = "dirty-flag")]
    pub dirty_flag: Option<bool>,
    /// actual size on disk in bytes of the image
    #[qapi(name = "actual-size")]
    pub actual_size: Option<i64>,
    /// maximum capacity in bytes of the image
    #[qapi(name = "virtual-size")]
    pub virtual_size: i64,
    /// size of a cluster in bytes
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<i64>,
    /// true if the image is encrypted
    #[qapi(name = "encrypted")]
    pub encrypted: Option<bool>,
    /// true if the image is compressed (Since 1.7)
    #[qapi(name = "compressed")]
    pub compressed: Option<bool>,
    /// name of the backing file
    #[qapi(name = "backing-filename")]
    pub backing_filename: Option<String>,
    /// full path of the backing file
    #[qapi(name = "full-backing-filename")]
    pub full_backing_filename: Option<String>,
    /// the format of the backing file
    #[qapi(name = "backing-filename-format")]
    pub backing_filename_format: Option<String>,
    /// list of VM snapshots
    #[qapi(name = "snapshots")]
    pub snapshots: Option<Vec<SnapshotInfo>>,
    /// structure supplying additional format-specific
    /// information (since 1.7)
    #[qapi(name = "format-specific")]
    pub format_specific: Option<ImageInfoSpecific>,
    /// info of the backing image
    #[qapi(name = "backing-image")]
    pub backing_image: Option<ImageInfo>,
}
/// Information about all nodes in the block graph starting at some
/// node, annotated with information about that node in relation to its
/// parent.
#[qapi(name = "BlockChildInfo")]
#[qapi(since = "8.0")]
pub struct BlockChildInfo {
    /// Child name of the root node in the BlockGraphInfo struct, in
    /// its role as the child of some undescribed parent node
    #[qapi(name = "name")]
    pub name: String,
    /// Block graph information starting at this node
    #[qapi(name = "info")]
    pub info: BlockGraphInfo,
}
/// Information about all nodes in a block (sub)graph in the form of
/// BlockNodeInfo data.  The base BlockNodeInfo struct contains the
/// information for the (sub)graph's root node.
#[qapi(name = "BlockGraphInfo")]
#[qapi(since = "8.0")]
pub struct BlockGraphInfo {
    /// name of the image file
    #[qapi(name = "filename")]
    pub filename: String,
    /// format of the image file
    #[qapi(name = "format")]
    pub format: String,
    /// true if image is not cleanly closed
    #[qapi(name = "dirty-flag")]
    pub dirty_flag: Option<bool>,
    /// actual size on disk in bytes of the image
    #[qapi(name = "actual-size")]
    pub actual_size: Option<i64>,
    /// maximum capacity in bytes of the image
    #[qapi(name = "virtual-size")]
    pub virtual_size: i64,
    /// size of a cluster in bytes
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<i64>,
    /// true if the image is encrypted
    #[qapi(name = "encrypted")]
    pub encrypted: Option<bool>,
    /// true if the image is compressed (Since 1.7)
    #[qapi(name = "compressed")]
    pub compressed: Option<bool>,
    /// name of the backing file
    #[qapi(name = "backing-filename")]
    pub backing_filename: Option<String>,
    /// full path of the backing file
    #[qapi(name = "full-backing-filename")]
    pub full_backing_filename: Option<String>,
    /// the format of the backing file
    #[qapi(name = "backing-filename-format")]
    pub backing_filename_format: Option<String>,
    /// list of VM snapshots
    #[qapi(name = "snapshots")]
    pub snapshots: Option<Vec<SnapshotInfo>>,
    /// structure supplying additional format-specific
    /// information (since 1.7)
    #[qapi(name = "format-specific")]
    pub format_specific: Option<ImageInfoSpecific>,
    /// Array of links to this node's child nodes' information
    #[qapi(name = "children")]
    pub children: Vec<BlockChildInfo>,
}
/// Information about a QEMU image file check
#[qapi(name = "ImageCheck")]
#[qapi(since = "1.4")]
pub struct ImageCheck {
    /// name of the image file checked
    #[qapi(name = "filename")]
    pub filename: String,
    /// format of the image file checked
    #[qapi(name = "format")]
    pub format: String,
    /// number of unexpected errors occurred during check
    #[qapi(name = "check-errors")]
    pub check_errors: i64,
    /// offset (in bytes) where the image ends, this
    /// field is present if the driver for the image format supports it
    #[qapi(name = "image-end-offset")]
    pub image_end_offset: Option<i64>,
    /// number of corruptions found during the check if any
    #[qapi(name = "corruptions")]
    pub corruptions: Option<i64>,
    /// number of leaks found during the check if any
    #[qapi(name = "leaks")]
    pub leaks: Option<i64>,
    /// number of corruptions fixed during the check if
    /// any
    #[qapi(name = "corruptions-fixed")]
    pub corruptions_fixed: Option<i64>,
    /// number of leaks fixed during the check if any
    #[qapi(name = "leaks-fixed")]
    pub leaks_fixed: Option<i64>,
    /// total number of clusters, this field is present if
    /// the driver for the image format supports it
    #[qapi(name = "total-clusters")]
    pub total_clusters: Option<i64>,
    /// total number of allocated clusters, this field
    /// is present if the driver for the image format supports it
    #[qapi(name = "allocated-clusters")]
    pub allocated_clusters: Option<i64>,
    /// total number of fragmented clusters, this
    /// field is present if the driver for the image format supports it
    #[qapi(name = "fragmented-clusters")]
    pub fragmented_clusters: Option<i64>,
    /// total number of compressed clusters, this
    /// field is present if the driver for the image format supports it
    #[qapi(name = "compressed-clusters")]
    pub compressed_clusters: Option<i64>,
}
/// Mapping information from a virtual block range to a host file range
#[qapi(name = "MapEntry")]
#[qapi(since = "2.6")]
pub struct MapEntry {
    /// virtual (guest) offset of the first byte described by this
    /// entry
    #[qapi(name = "start")]
    pub start: i64,
    /// the number of bytes of the mapped virtual range
    #[qapi(name = "length")]
    pub length: i64,
    /// reading the image will actually read data from a file (in
    /// particular, if @offset is present this means that the sectors
    /// are not simply preallocated, but contain actual data in raw
    /// format)
    #[qapi(name = "data")]
    pub data: bool,
    /// whether the virtual blocks read as zeroes
    #[qapi(name = "zero")]
    pub zero: bool,
    /// true if the data is stored compressed (since 8.2)
    #[qapi(name = "compressed")]
    pub compressed: bool,
    /// number of layers (0 = top image, 1 = top image's backing
    /// file, ..., n - 1 = bottom image (where n is the number of images
    /// in the chain)) before reaching one for which the range is
    /// allocated
    #[qapi(name = "depth")]
    pub depth: i64,
    /// true if this layer provides the data, false if adding a
    /// backing layer could impact this region (since 6.1)
    #[qapi(name = "present")]
    pub present: bool,
    /// if present, the image file stores the data for this range
    /// in raw format at the given (host) offset
    #[qapi(name = "offset")]
    pub offset: Option<i64>,
    /// filename that is referred to by @offset
    #[qapi(name = "filename")]
    pub filename: Option<String>,
}
/// Cache mode information for a block device
#[qapi(name = "BlockdevCacheInfo")]
#[qapi(since = "2.3")]
pub struct BlockdevCacheInfo {
    /// true if writeback mode is enabled
    #[qapi(name = "writeback")]
    pub writeback: bool,
    /// true if the host page cache is bypassed (O_DIRECT)
    #[qapi(name = "direct")]
    pub direct: bool,
    /// true if flush requests are ignored for the device
    #[qapi(name = "no-flush")]
    pub no_flush: bool,
}
/// Information about the backing device for a block device.
#[qapi(name = "BlockDeviceInfo")]
#[qapi(since = "0.14")]
pub struct BlockDeviceInfo {
    /// the filename of the backing device
    #[qapi(name = "file")]
    pub file: String,
    /// the name of the block driver node (Since 2.0)
    #[qapi(name = "node-name")]
    pub node_name: Option<String>,
    /// true if the backing device was open read-only
    #[qapi(name = "ro")]
    pub ro: bool,
    /// the name of the block format used to open the backing device.
    /// As of 0.14 this can be: 'blkdebug', 'bochs', 'cloop', 'cow',
    /// 'dmg', 'file', 'file', 'ftp', 'ftps', 'host_cdrom',
    /// 'host_device', 'http', 'https', 'luks', 'nbd', 'parallels',
    /// 'qcow', 'qcow2', 'raw', 'vdi', 'vmdk', 'vpc', 'vvfat' 2.2:
    /// 'archipelago' added, 'cow' dropped 2.3: 'host_floppy' deprecated
    /// 2.5: 'host_floppy' dropped 2.6: 'luks' added 2.8: 'replication'
    /// added, 'tftp' dropped 2.9: 'archipelago' dropped
    #[qapi(name = "drv")]
    pub drv: String,
    /// the name of the backing file (for copy-on-write)
    #[qapi(name = "backing_file")]
    pub backing_file: Option<String>,
    /// number of files in the backing file chain
    /// (since: 1.2)
    #[qapi(name = "backing_file_depth")]
    pub backing_file_depth: i64,
    /// true if the backing device is encrypted
    #[qapi(name = "encrypted")]
    pub encrypted: bool,
    /// detect and optimize zero writes (Since 2.1)
    #[qapi(name = "detect_zeroes")]
    pub detect_zeroes: BlockdevDetectZeroesOptions,
    /// total throughput limit in bytes per second is specified
    #[qapi(name = "bps")]
    pub bps: i64,
    /// read throughput limit in bytes per second is specified
    #[qapi(name = "bps_rd")]
    pub bps_rd: i64,
    /// write throughput limit in bytes per second is specified
    #[qapi(name = "bps_wr")]
    pub bps_wr: i64,
    /// total I/O operations per second is specified
    #[qapi(name = "iops")]
    pub iops: i64,
    /// read I/O operations per second is specified
    #[qapi(name = "iops_rd")]
    pub iops_rd: i64,
    /// write I/O operations per second is specified
    #[qapi(name = "iops_wr")]
    pub iops_wr: i64,
    /// the info of image used (since: 1.6)
    #[qapi(name = "image")]
    pub image: ImageInfo,
    /// total throughput limit during bursts, in bytes (Since 1.7)
    #[qapi(name = "bps_max")]
    pub bps_max: Option<i64>,
    /// read throughput limit during bursts, in bytes (Since
    /// 1.7)
    #[qapi(name = "bps_rd_max")]
    pub bps_rd_max: Option<i64>,
    /// write throughput limit during bursts, in bytes (Since
    /// 1.7)
    #[qapi(name = "bps_wr_max")]
    pub bps_wr_max: Option<i64>,
    /// total I/O operations per second during bursts, in bytes
    /// (Since 1.7)
    #[qapi(name = "iops_max")]
    pub iops_max: Option<i64>,
    /// read I/O operations per second during bursts, in bytes
    /// (Since 1.7)
    #[qapi(name = "iops_rd_max")]
    pub iops_rd_max: Option<i64>,
    /// write I/O operations per second during bursts, in
    /// bytes (Since 1.7)
    #[qapi(name = "iops_wr_max")]
    pub iops_wr_max: Option<i64>,
    /// maximum length of the @bps_max burst period, in
    /// seconds.  (Since 2.6)
    #[qapi(name = "bps_max_length")]
    pub bps_max_length: Option<i64>,
    /// maximum length of the @bps_rd_max burst period,
    /// in seconds.  (Since 2.6)
    #[qapi(name = "bps_rd_max_length")]
    pub bps_rd_max_length: Option<i64>,
    /// maximum length of the @bps_wr_max burst period,
    /// in seconds.  (Since 2.6)
    #[qapi(name = "bps_wr_max_length")]
    pub bps_wr_max_length: Option<i64>,
    /// maximum length of the @iops burst period, in
    /// seconds.  (Since 2.6)
    #[qapi(name = "iops_max_length")]
    pub iops_max_length: Option<i64>,
    /// maximum length of the @iops_rd_max burst
    /// period, in seconds.  (Since 2.6)
    #[qapi(name = "iops_rd_max_length")]
    pub iops_rd_max_length: Option<i64>,
    /// maximum length of the @iops_wr_max burst
    /// period, in seconds.  (Since 2.6)
    #[qapi(name = "iops_wr_max_length")]
    pub iops_wr_max_length: Option<i64>,
    /// an I/O size in bytes (Since 1.7)
    #[qapi(name = "iops_size")]
    pub iops_size: Option<i64>,
    /// throttle group name (Since 2.4)
    #[qapi(name = "group")]
    pub group: Option<String>,
    /// the cache mode used for the block device (since: 2.3)
    #[qapi(name = "cache")]
    pub cache: BlockdevCacheInfo,
    /// configured write threshold for the device.  0 if
    /// disabled.  (Since 2.3)
    #[qapi(name = "write_threshold")]
    pub write_threshold: i64,
    /// dirty bitmaps information (only present if node has
    /// one or more dirty bitmaps) (Since 4.2)
    #[qapi(name = "dirty-bitmaps")]
    pub dirty_bitmaps: Option<Vec<BlockDirtyInfo>>,
}
/// An enumeration of block device I/O status.
#[qapi(name = "BlockDeviceIoStatus")]
#[qapi(since = "1.0")]
pub enum BlockDeviceIoStatus {
    /// The last I/O operation has succeeded
    #[qapi(name = "ok")]
    Ok,
    /// The last I/O operation has failed
    #[qapi(name = "failed")]
    Failed,
    /// The last I/O operation has failed due to a no-space
    /// condition
    #[qapi(name = "nospace")]
    Nospace,
}
/// Block dirty bitmap information.
#[qapi(name = "BlockDirtyInfo")]
#[qapi(since = "1.3")]
pub struct BlockDirtyInfo {
    /// the name of the dirty bitmap (Since 2.4)
    #[qapi(name = "name")]
    pub name: Option<String>,
    /// number of dirty bytes according to the dirty bitmap
    #[qapi(name = "count")]
    pub count: i64,
    /// granularity of the dirty bitmap in bytes (since 1.4)
    #[qapi(name = "granularity")]
    pub granularity: u32,
    /// true if the bitmap is recording new writes from the
    /// guest.  (since 4.0)
    #[qapi(name = "recording")]
    pub recording: bool,
    /// true if the bitmap is in-use by some operation (NBD or jobs)
    /// and cannot be modified via QMP or used by another operation.
    /// (since 4.0)
    #[qapi(name = "busy")]
    pub busy: bool,
    /// true if the bitmap was stored on disk, is scheduled to
    /// be stored on disk, or both.  (since 4.0)
    #[qapi(name = "persistent")]
    pub persistent: bool,
    /// true if this is a persistent bitmap that was
    /// improperly stored.  Implies @persistent to be true; @recording
    /// and @busy to be false.  This bitmap cannot be used.  To remove
    /// it, use @block-dirty-bitmap-remove.  (Since 4.0)
    #[qapi(name = "inconsistent")]
    pub inconsistent: Option<bool>,
}
/// An enumeration of flags that a bitmap can report to the user.
#[qapi(name = "Qcow2BitmapInfoFlags")]
#[qapi(since = "4.0")]
pub enum Qcow2BitmapInfoFlags {
    /// This flag is set by any process actively modifying the
    /// qcow2 file, and cleared when the updated bitmap is flushed to
    /// the qcow2 image.  The presence of this flag in an offline image
    /// means that the bitmap was not saved correctly after its last
    /// usage, and may contain inconsistent data.
    #[qapi(name = "in-use")]
    InUse,
    /// The bitmap must reflect all changes of the virtual disk by
    /// any application that would write to this qcow2 file.
    #[qapi(name = "auto")]
    Auto,
}
/// Qcow2 bitmap information.
#[qapi(name = "Qcow2BitmapInfo")]
#[qapi(since = "4.0")]
pub struct Qcow2BitmapInfo {
    /// the name of the bitmap
    #[qapi(name = "name")]
    pub name: String,
    /// granularity of the bitmap in bytes
    #[qapi(name = "granularity")]
    pub granularity: u32,
    /// flags of the bitmap
    #[qapi(name = "flags")]
    pub flags: Vec<Qcow2BitmapInfoFlags>,
}
/// Block latency histogram.
#[qapi(name = "BlockLatencyHistogramInfo")]
#[qapi(since = "4.0")]
pub struct BlockLatencyHistogramInfo {
    /// list of interval boundary values in nanoseconds, all
    /// greater than zero and in ascending order.  For example, the list
    /// [10, 50, 100] produces the following histogram intervals: [0,
    /// 10), [10, 50), [50, 100), [100, +inf).
    #[qapi(name = "boundaries")]
    pub boundaries: Vec<u64>,
    /// list of io request counts corresponding to histogram
    /// intervals, one more element than @boundaries has.  For the
    /// example above, @bins may be something like [3, 1, 5, 2], and
    /// corresponding histogram looks like::
    ///
    /// 5|           *
    /// 4|           *
    /// 3| *         *
    /// 2| *         *    *
    /// 1| *    *    *    *
    /// +------------------
    /// 10   50   100
    #[qapi(name = "bins")]
    pub bins: Vec<u64>,
}
/// Block device information.  This structure describes a virtual device
/// and the backing device associated with it.
#[qapi(name = "BlockInfo")]
#[qapi(since = "0.14")]
pub struct BlockInfo {
    /// The device name associated with the virtual device.
    #[qapi(name = "device")]
    pub device: String,
    /// The qdev ID, or if no ID is assigned, the QOM path of the
    /// block device.  (since 2.10)
    #[qapi(name = "qdev")]
    pub qdev: Option<String>,
    /// This field is returned only for compatibility reasons, it
    /// should not be used (always returns 'unknown')
    #[qapi(name = "type")]
    pub r#type: String,
    /// True if the device supports removable media.
    #[qapi(name = "removable")]
    pub removable: bool,
    /// True if the guest has locked this device from having its
    /// media removed
    #[qapi(name = "locked")]
    pub locked: bool,
    /// @BlockDeviceInfo describing the device if media is
    /// present
    #[qapi(name = "inserted")]
    pub inserted: Option<BlockDeviceInfo>,
    /// True if the device's tray is open (only present if it
    /// has a tray)
    #[qapi(name = "tray_open")]
    pub tray_open: Option<bool>,
    /// @BlockDeviceIoStatus.  Only present if the device
    /// supports it and the VM is configured to stop on errors
    /// (supported device models: virtio-blk, IDE, SCSI except
    /// scsi-generic)
    #[qapi(name = "io-status")]
    pub io_status: Option<BlockDeviceIoStatus>,
}
/// Image file size calculation information.  This structure describes
/// the size requirements for creating a new image file.
///
/// The size requirements depend on the new image file format.  File
/// size always equals virtual disk size for the 'raw' format, even for
/// sparse POSIX files.  Compact formats such as 'qcow2' represent
/// unallocated and zero regions efficiently so file size may be smaller
/// than virtual disk size.
///
/// The values are upper bounds that are guaranteed to fit the new image
/// file.  Subsequent modification, such as internal snapshot or further
/// bitmap creation, may require additional space and is not covered
/// here.
#[qapi(name = "BlockMeasureInfo")]
#[qapi(since = "2.10")]
pub struct BlockMeasureInfo {
    /// Size required for a new image file, in bytes, when
    /// copying just allocated guest-visible contents.
    #[qapi(name = "required")]
    pub required: i64,
    /// Image file size, in bytes, once data has been
    /// written to all sectors, when copying just guest-visible
    /// contents.
    #[qapi(name = "fully-allocated")]
    pub fully_allocated: i64,
    /// Additional size required if all the top-level bitmap
    /// metadata in the source image were to be copied to the
    /// destination, present only when source and destination both
    /// support persistent bitmaps.  (since 5.1)
    #[qapi(name = "bitmaps")]
    pub bitmaps: Option<i64>,
}
/// Get a list of BlockInfo for all virtual block devices.
#[qapi(name = "query-block")]
#[qapi(since = "0.14")]
#[qapi(returns = "Vec<BlockInfo>")]
#[qapi(allow_preconfig)]
pub struct QueryBlock {}
/// Statistics of a block device during a given interval of time.
#[qapi(name = "BlockDeviceTimedStats")]
#[qapi(since = "2.5")]
pub struct BlockDeviceTimedStats {
    /// Interval used for calculating the statistics, in
    /// seconds.
    #[qapi(name = "interval_length")]
    pub interval_length: i64,
    /// Minimum latency of read operations in the
    /// defined interval, in nanoseconds.
    #[qapi(name = "min_rd_latency_ns")]
    pub min_rd_latency_ns: i64,
    /// Maximum latency of read operations in the
    /// defined interval, in nanoseconds.
    #[qapi(name = "max_rd_latency_ns")]
    pub max_rd_latency_ns: i64,
    /// Average latency of read operations in the
    /// defined interval, in nanoseconds.
    #[qapi(name = "avg_rd_latency_ns")]
    pub avg_rd_latency_ns: i64,
    /// Minimum latency of write operations in the
    /// defined interval, in nanoseconds.
    #[qapi(name = "min_wr_latency_ns")]
    pub min_wr_latency_ns: i64,
    /// Maximum latency of write operations in the
    /// defined interval, in nanoseconds.
    #[qapi(name = "max_wr_latency_ns")]
    pub max_wr_latency_ns: i64,
    /// Average latency of write operations in the
    /// defined interval, in nanoseconds.
    #[qapi(name = "avg_wr_latency_ns")]
    pub avg_wr_latency_ns: i64,
    /// Minimum latency of zone append
    /// operations in the defined interval, in nanoseconds (since 8.1)
    #[qapi(name = "min_zone_append_latency_ns")]
    pub min_zone_append_latency_ns: i64,
    /// Maximum latency of zone append
    /// operations in the defined interval, in nanoseconds (since 8.1)
    #[qapi(name = "max_zone_append_latency_ns")]
    pub max_zone_append_latency_ns: i64,
    /// Average latency of zone append
    /// operations in the defined interval, in nanoseconds (since 8.1)
    #[qapi(name = "avg_zone_append_latency_ns")]
    pub avg_zone_append_latency_ns: i64,
    /// Minimum latency of flush operations in the
    /// defined interval, in nanoseconds.
    #[qapi(name = "min_flush_latency_ns")]
    pub min_flush_latency_ns: i64,
    /// Maximum latency of flush operations in the
    /// defined interval, in nanoseconds.
    #[qapi(name = "max_flush_latency_ns")]
    pub max_flush_latency_ns: i64,
    /// Average latency of flush operations in the
    /// defined interval, in nanoseconds.
    #[qapi(name = "avg_flush_latency_ns")]
    pub avg_flush_latency_ns: i64,
    /// Average number of pending read operations in
    /// the defined interval.
    #[qapi(name = "avg_rd_queue_depth")]
    pub avg_rd_queue_depth: f64,
    /// Average number of pending write operations in
    /// the defined interval.
    #[qapi(name = "avg_wr_queue_depth")]
    pub avg_wr_queue_depth: f64,
    /// Average number of pending zone append
    /// operations in the defined interval (since 8.1).
    #[qapi(name = "avg_zone_append_queue_depth")]
    pub avg_zone_append_queue_depth: f64,
}
/// Statistics of a virtual block device or a block backing device.
#[qapi(name = "BlockDeviceStats")]
#[qapi(since = "0.14")]
pub struct BlockDeviceStats {
    /// The number of bytes read by the device.
    #[qapi(name = "rd_bytes")]
    pub rd_bytes: i64,
    /// The number of bytes written by the device.
    #[qapi(name = "wr_bytes")]
    pub wr_bytes: i64,
    /// The number of bytes appended by the zoned
    /// devices (since 8.1)
    #[qapi(name = "zone_append_bytes")]
    pub zone_append_bytes: i64,
    /// The number of bytes unmapped by the device (Since 4.2)
    #[qapi(name = "unmap_bytes")]
    pub unmap_bytes: i64,
    /// The number of read operations performed by the
    /// device.
    #[qapi(name = "rd_operations")]
    pub rd_operations: i64,
    /// The number of write operations performed by the
    /// device.
    #[qapi(name = "wr_operations")]
    pub wr_operations: i64,
    /// The number of zone append operations
    /// performed by the zoned devices (since 8.1)
    #[qapi(name = "zone_append_operations")]
    pub zone_append_operations: i64,
    /// The number of cache flush operations performed by
    /// the device (since 0.15)
    #[qapi(name = "flush_operations")]
    pub flush_operations: i64,
    /// The number of unmap operations performed by the
    /// device (Since 4.2)
    #[qapi(name = "unmap_operations")]
    pub unmap_operations: i64,
    /// Total time spent on reads in nanoseconds (since
    /// 0.15).
    #[qapi(name = "rd_total_time_ns")]
    pub rd_total_time_ns: i64,
    /// Total time spent on writes in nanoseconds (since
    /// 0.15).
    #[qapi(name = "wr_total_time_ns")]
    pub wr_total_time_ns: i64,
    /// Total time spent on zone append writes
    /// in nanoseconds (since 8.1)
    #[qapi(name = "zone_append_total_time_ns")]
    pub zone_append_total_time_ns: i64,
    /// Total time spent on cache flushes in
    /// nanoseconds (since 0.15).
    #[qapi(name = "flush_total_time_ns")]
    pub flush_total_time_ns: i64,
    /// Total time spent on unmap operations in
    /// nanoseconds (Since 4.2)
    #[qapi(name = "unmap_total_time_ns")]
    pub unmap_total_time_ns: i64,
    /// The offset after the greatest byte written to
    /// the device.  The intended use of this information is for
    /// growable sparse files (like qcow2) that are used on top of a
    /// physical device.
    #[qapi(name = "wr_highest_offset")]
    pub wr_highest_offset: i64,
    /// Number of read requests that have been merged into
    /// another request (Since 2.3).
    #[qapi(name = "rd_merged")]
    pub rd_merged: i64,
    /// Number of write requests that have been merged into
    /// another request (Since 2.3).
    #[qapi(name = "wr_merged")]
    pub wr_merged: i64,
    /// Number of zone append requests that have been
    /// merged into another request (since 8.1)
    #[qapi(name = "zone_append_merged")]
    pub zone_append_merged: i64,
    /// Number of unmap requests that have been merged into
    /// another request (Since 4.2)
    #[qapi(name = "unmap_merged")]
    pub unmap_merged: i64,
    /// Time since the last I/O operation, in nanoseconds.
    /// If the field is absent it means that there haven't been any
    /// operations yet (Since 2.5).
    #[qapi(name = "idle_time_ns")]
    pub idle_time_ns: Option<i64>,
    /// The number of failed read operations
    /// performed by the device (Since 2.5)
    #[qapi(name = "failed_rd_operations")]
    pub failed_rd_operations: i64,
    /// The number of failed write operations
    /// performed by the device (Since 2.5)
    #[qapi(name = "failed_wr_operations")]
    pub failed_wr_operations: i64,
    /// The number of failed zone append
    /// write operations performed by the zoned devices (since 8.1)
    #[qapi(name = "failed_zone_append_operations")]
    pub failed_zone_append_operations: i64,
    /// The number of failed flush operations
    /// performed by the device (Since 2.5)
    #[qapi(name = "failed_flush_operations")]
    pub failed_flush_operations: i64,
    /// The number of failed unmap operations
    /// performed by the device (Since 4.2)
    #[qapi(name = "failed_unmap_operations")]
    pub failed_unmap_operations: i64,
    /// The number of invalid read operations
    /// performed by the device (Since 2.5)
    #[qapi(name = "invalid_rd_operations")]
    pub invalid_rd_operations: i64,
    /// The number of invalid write operations
    /// performed by the device (Since 2.5)
    #[qapi(name = "invalid_wr_operations")]
    pub invalid_wr_operations: i64,
    /// The number of invalid zone append
    /// operations performed by the zoned device (since 8.1)
    #[qapi(name = "invalid_zone_append_operations")]
    pub invalid_zone_append_operations: i64,
    /// The number of invalid flush operations
    /// performed by the device (Since 2.5)
    #[qapi(name = "invalid_flush_operations")]
    pub invalid_flush_operations: i64,
    /// The number of invalid unmap operations
    /// performed by the device (Since 4.2)
    #[qapi(name = "invalid_unmap_operations")]
    pub invalid_unmap_operations: i64,
    /// Whether invalid operations are included in the
    /// last access statistics (Since 2.5)
    #[qapi(name = "account_invalid")]
    pub account_invalid: bool,
    /// Whether failed operations are included in the
    /// latency and last access statistics (Since 2.5)
    #[qapi(name = "account_failed")]
    pub account_failed: bool,
    /// Statistics specific to the set of previously defined
    /// intervals of time (Since 2.5)
    #[qapi(name = "timed_stats")]
    pub timed_stats: Vec<BlockDeviceTimedStats>,
    /// @BlockLatencyHistogramInfo.  (Since 4.0)
    #[qapi(name = "rd_latency_histogram")]
    pub rd_latency_histogram: Option<BlockLatencyHistogramInfo>,
    /// @BlockLatencyHistogramInfo.  (Since 4.0)
    #[qapi(name = "wr_latency_histogram")]
    pub wr_latency_histogram: Option<BlockLatencyHistogramInfo>,
    /// @BlockLatencyHistogramInfo.
    /// (since 8.1)
    #[qapi(name = "zone_append_latency_histogram")]
    pub zone_append_latency_histogram: Option<BlockLatencyHistogramInfo>,
    /// @BlockLatencyHistogramInfo.  (Since 4.0)
    #[qapi(name = "flush_latency_histogram")]
    pub flush_latency_histogram: Option<BlockLatencyHistogramInfo>,
}
/// File driver statistics
#[qapi(name = "BlockStatsSpecificFile")]
#[qapi(since = "4.2")]
pub struct BlockStatsSpecificFile {
    /// The number of successful discard operations
    /// performed by the driver.
    #[qapi(name = "discard-nb-ok")]
    pub discard_nb_ok: u64,
    /// The number of failed discard operations
    /// performed by the driver.
    #[qapi(name = "discard-nb-failed")]
    pub discard_nb_failed: u64,
    /// The number of bytes discarded by the driver.
    #[qapi(name = "discard-bytes-ok")]
    pub discard_bytes_ok: u64,
}
/// NVMe driver statistics
#[qapi(name = "BlockStatsSpecificNvme")]
#[qapi(since = "5.2")]
pub struct BlockStatsSpecificNvme {
    /// The number of completion errors.
    #[qapi(name = "completion-errors")]
    pub completion_errors: u64,
    /// The number of aligned accesses performed by the
    /// driver.
    #[qapi(name = "aligned-accesses")]
    pub aligned_accesses: u64,
    /// The number of unaligned accesses performed by
    /// the driver.
    #[qapi(name = "unaligned-accesses")]
    pub unaligned_accesses: u64,
}
pub enum BlockStatsSpecificBranch {
    #[qapi(name = "file")]
    File(BlockStatsSpecificFile),
    #[qapi(name = "host_device")]
    #[qapi(condition = "HAVE_HOST_BLOCK_DEVICE")]
    HostDevice(BlockStatsSpecificFile),
    #[qapi(name = "nvme")]
    Nvme(BlockStatsSpecificNvme),
}
/// Block driver specific statistics
#[qapi(name = "BlockStatsSpecific")]
#[qapi(since = "4.2")]
pub struct BlockStatsSpecific {
    /// block driver name
    #[qapi(name = "driver")]
    #[qapi(discriminator)]
    pub driver: BlockdevDriver,
    #[qapi(union)]
    pub u: Option<BlockStatsSpecificBranch>,
}
/// Statistics of a virtual block device or a block backing device.
#[qapi(name = "BlockStats")]
#[qapi(since = "0.14")]
pub struct BlockStats {
    /// If the stats are for a virtual block device, the name
    /// corresponding to the virtual block device.
    #[qapi(name = "device")]
    pub device: Option<String>,
    /// The qdev ID, or if no ID is assigned, the QOM path of the
    /// block device.  (since 3.0)
    #[qapi(name = "qdev")]
    pub qdev: Option<String>,
    /// The node name of the device.  (Since 2.3)
    #[qapi(name = "node-name")]
    pub node_name: Option<String>,
    /// A @BlockDeviceStats for the device.
    #[qapi(name = "stats")]
    pub stats: BlockDeviceStats,
    /// Optional driver-specific stats.  (Since 4.2)
    #[qapi(name = "driver-specific")]
    pub driver_specific: Option<BlockStatsSpecific>,
    /// This describes the file block device if it has one.
    /// Contains recursively the statistics of the underlying protocol
    /// (e.g. the host file for a qcow2 image).  If there is no
    /// underlying protocol, this field is omitted
    #[qapi(name = "parent")]
    pub parent: Option<BlockStats>,
    /// This describes the backing block device if it has one.
    /// (Since 2.0)
    #[qapi(name = "backing")]
    pub backing: Option<BlockStats>,
}
/// Query the @BlockStats for all virtual block devices.
#[qapi(name = "query-blockstats")]
#[qapi(since = "0.14")]
#[qapi(returns = "Vec<BlockStats>")]
#[qapi(allow_preconfig)]
pub struct QueryBlockstats {
    /// If true, the command will query all the block nodes
    /// that have a node name, in a list which will include "parent"
    /// information, but not "backing".  If false or omitted, the
    /// behavior is as before - query all the device backends,
    /// recursively including their "parent" and "backing".  Filter
    /// nodes that were created implicitly are skipped over in this
    /// mode.  (Since 2.3)
    #[qapi(name = "query-nodes")]
    pub query_nodes: Option<bool>,
}
/// An enumeration of possible behaviors for errors on I/O operations.
/// The exact meaning depends on whether the I/O was initiated by a
/// guest or by a block job
#[qapi(name = "BlockdevOnError")]
#[qapi(since = "1.3")]
pub enum BlockdevOnError {
    /// for guest operations, report the error to the guest; for
    /// jobs, cancel the job
    #[qapi(name = "report")]
    Report,
    /// ignore the error, only report a QMP event (BLOCK_IO_ERROR
    /// or BLOCK_JOB_ERROR).  The backup, mirror and commit block jobs
    /// retry the failing request later and may still complete
    /// successfully.  The stream block job continues to stream and will
    /// complete with an error.
    #[qapi(name = "ignore")]
    Ignore,
    /// same as @stop on ENOSPC, same as @report otherwise.
    #[qapi(name = "enospc")]
    Enospc,
    /// for guest operations, stop the virtual machine; for jobs,
    /// pause the job
    #[qapi(name = "stop")]
    Stop,
    /// inherit the error handling policy of the backend (since: 2.7)
    #[qapi(name = "auto")]
    Auto,
}
/// An enumeration of possible behaviors for the initial synchronization
/// phase of storage mirroring.
#[qapi(name = "MirrorSyncMode")]
#[qapi(since = "1.3")]
pub enum MirrorSyncMode {
    /// copies data in the topmost image to the destination
    #[qapi(name = "top")]
    Top,
    /// copies data from all images to the destination
    #[qapi(name = "full")]
    Full,
    /// only copy data written from now on
    #[qapi(name = "none")]
    None,
    /// only copy data described by the dirty bitmap.
    /// (since: 2.4)
    #[qapi(name = "incremental")]
    Incremental,
    /// only copy data described by the dirty bitmap.  (since: 4.2)
    /// Behavior on completion is determined by the BitmapSyncMode.
    #[qapi(name = "bitmap")]
    Bitmap,
}
/// An enumeration of possible behaviors for the synchronization of a
/// bitmap when used for data copy operations.
#[qapi(name = "BitmapSyncMode")]
#[qapi(since = "4.2")]
pub enum BitmapSyncMode {
    /// The bitmap is only synced when the operation is
    /// successful.  This is the behavior always used for 'INCREMENTAL'
    /// backups.
    #[qapi(name = "on-success")]
    OnSuccess,
    /// The bitmap is never synchronized with the operation, and is
    /// treated solely as a read-only manifest of blocks to copy.
    #[qapi(name = "never")]
    Never,
    /// The bitmap is always synchronized with the operation,
    /// regardless of whether or not the operation was successful.
    #[qapi(name = "always")]
    Always,
}
/// An enumeration whose values tell the mirror block job when to
/// trigger writes to the target.
#[qapi(name = "MirrorCopyMode")]
#[qapi(since = "3.0")]
pub enum MirrorCopyMode {
    /// copy data in background only.
    #[qapi(name = "background")]
    Background,
    /// when data is written to the source, write it
    /// (synchronously) to the target as well.  In addition, data is
    /// copied in background just like in @background mode.
    #[qapi(name = "write-blocking")]
    WriteBlocking,
}
/// Information specific to mirror block jobs.
#[qapi(name = "BlockJobInfoMirror")]
#[qapi(since = "8.2")]
pub struct BlockJobInfoMirror {
    /// Whether the source is actively synced to the
    /// target, i.e. same data and new writes are done synchronously to
    /// both.
    #[qapi(name = "actively-synced")]
    pub actively_synced: bool,
}
pub enum BlockJobInfoBranch {
    #[qapi(name = "mirror")]
    Mirror(BlockJobInfoMirror),
}
/// Information about a long-running block device operation.
#[qapi(name = "BlockJobInfo")]
#[qapi(since = "1.1")]
pub struct BlockJobInfo {
    /// the job type ('stream' for image streaming)
    #[qapi(name = "type")]
    #[qapi(discriminator)]
    pub r#type: JobType,
    /// The job identifier.  Originally the device name but other
    /// values are allowed since QEMU 2.7
    #[qapi(name = "device")]
    pub device: String,
    /// Estimated @offset value at the completion of the job.  This
    /// value can arbitrarily change while the job is running, in both
    /// directions.
    #[qapi(name = "len")]
    pub len: i64,
    /// Progress made until now.  The unit is arbitrary and the
    /// value can only meaningfully be used for the ratio of @offset to
    /// @len.  The value is monotonically increasing.
    #[qapi(name = "offset")]
    pub offset: i64,
    /// false if the job is known to be in a quiescent state, with no
    /// pending I/O.  (Since 1.3)
    #[qapi(name = "busy")]
    pub busy: bool,
    /// whether the job is paused or, if @busy is true, will pause
    /// itself as soon as possible.  (Since 1.3)
    #[qapi(name = "paused")]
    pub paused: bool,
    /// the rate limit, bytes per second
    #[qapi(name = "speed")]
    pub speed: i64,
    /// the status of the job (since 1.3)
    #[qapi(name = "io-status")]
    pub io_status: BlockDeviceIoStatus,
    /// true if the job may be completed (since 2.2)
    #[qapi(name = "ready")]
    pub ready: bool,
    /// Current job state/status (since 2.12)
    #[qapi(name = "status")]
    pub status: JobStatus,
    /// Job will finalize itself when PENDING, moving to the
    /// CONCLUDED state.  (since 2.12)
    #[qapi(name = "auto-finalize")]
    pub auto_finalize: bool,
    /// Job will dismiss itself when CONCLUDED, moving to the
    /// NULL state and disappearing from the query list.  (since 2.12)
    #[qapi(name = "auto-dismiss")]
    pub auto_dismiss: bool,
    /// Error information if the job did not complete successfully.
    /// Not set if the job completed successfully.  (since 2.12.1)
    #[qapi(name = "error")]
    pub error: Option<String>,
    #[qapi(union)]
    pub u: Option<BlockJobInfoBranch>,
}
/// Return information about long-running block device operations.
#[qapi(name = "query-block-jobs")]
#[qapi(since = "1.1")]
#[qapi(returns = "Vec<BlockJobInfo>")]
#[qapi(allow_preconfig)]
pub struct QueryBlockJobs {}
/// Resize a block image while a guest is running.
///
/// Either @device or @node-name must be set but not both.
#[qapi(name = "block_resize")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockResize {
    /// the name of the device to get the image resized
    #[qapi(name = "device")]
    pub device: Option<String>,
    /// graph node name to get the image resized (Since 2.0)
    #[qapi(name = "node-name")]
    pub node_name: Option<String>,
    /// new image size in bytes
    #[qapi(name = "size")]
    pub size: i64,
}
/// An enumeration that tells QEMU how to set the backing file path in a
/// new image file.
#[qapi(name = "NewImageMode")]
#[qapi(since = "1.1")]
pub enum NewImageMode {
    /// QEMU should look for an existing image file.
    #[qapi(name = "existing")]
    Existing,
    /// QEMU should create a new image with absolute paths
    /// for the backing file.  If there is no backing file available,
    /// the new image will not be backed either.
    #[qapi(name = "absolute-paths")]
    AbsolutePaths,
}
/// Either @device or @node-name must be set but not both.
#[qapi(name = "BlockdevSnapshotSync")]
pub struct BlockdevSnapshotSync {
    /// the name of the device to take a snapshot of.
    #[qapi(name = "device")]
    pub device: Option<String>,
    /// graph node name to generate the snapshot from (Since
    /// 2.0)
    #[qapi(name = "node-name")]
    pub node_name: Option<String>,
    /// the target of the new overlay image.  If the file
    /// exists, or if it is a device, the overlay will be created in the
    /// existing file/device.  Otherwise, a new file will be created.
    #[qapi(name = "snapshot-file")]
    pub snapshot_file: String,
    /// the graph node name of the new image (Since
    /// 2.0)
    #[qapi(name = "snapshot-node-name")]
    pub snapshot_node_name: Option<String>,
    /// the format of the overlay image, default is 'qcow2'.
    #[qapi(name = "format")]
    pub format: Option<String>,
    /// whether and how QEMU should create a new image, default is
    /// 'absolute-paths'.
    #[qapi(name = "mode")]
    pub mode: Option<NewImageMode>,
}
#[qapi(name = "BlockdevSnapshot")]
#[qapi(since = "2.5")]
pub struct BlockdevSnapshot {
    /// device or node name that will have a snapshot taken.
    #[qapi(name = "node")]
    pub node: String,
    /// reference to the existing block device that will become
    /// the overlay of @node, as part of taking the snapshot.  It must
    /// not have a current backing file (this can be achieved by passing
    /// "backing": null to blockdev-add).
    #[qapi(name = "overlay")]
    pub overlay: String,
}
/// Optional parameters for backup.  These parameters don't affect
/// functionality, but may significantly affect performance.
#[qapi(name = "BackupPerf")]
#[qapi(since = "6.0")]
pub struct BackupPerf {
    /// Use copy offloading.  Default false.
    #[qapi(name = "use-copy-range")]
    pub use_copy_range: Option<bool>,
    /// Maximum number of parallel requests for the sustained
    /// background copying process.  Doesn't influence copy-before-write
    /// operations.  Default 64.
    #[qapi(name = "max-workers")]
    pub max_workers: Option<i64>,
    /// Maximum request length for the sustained background
    /// copying process.  Doesn't influence copy-before-write
    /// operations.  0 means unlimited.  If max-chunk is non-zero then
    /// it should not be less than job cluster size which is calculated
    /// as maximum of target image cluster size and 64k.  Default 0.
    #[qapi(name = "max-chunk")]
    pub max_chunk: Option<i64>,
}
#[qapi(name = "BackupCommon")]
#[qapi(since = "4.2")]
pub struct BackupCommon {
    /// identifier for the newly-created block job.  If omitted,
    /// the device name will be used.  (Since 2.7)
    #[qapi(name = "job-id")]
    pub job_id: Option<String>,
    /// the device name or node-name of a root node which should be
    /// copied.
    #[qapi(name = "device")]
    pub device: String,
    /// what parts of the disk image should be copied to the
    /// destination (all the disk, only the sectors allocated in the
    /// topmost image, from a dirty bitmap, or only new I/O).
    #[qapi(name = "sync")]
    pub sync: MirrorSyncMode,
    /// the maximum speed, in bytes per second.  The default is 0,
    /// for unlimited.
    #[qapi(name = "speed")]
    pub speed: Option<i64>,
    /// The name of a dirty bitmap to use.  Must be present if sync
    /// is "bitmap" or "incremental".  Can be present if sync is "full"
    /// or "top".  Must not be present otherwise.
    /// (Since 2.4 (drive-backup), 3.1 (blockdev-backup))
    #[qapi(name = "bitmap")]
    pub bitmap: Option<String>,
    /// Specifies the type of data the bitmap should contain
    /// after the operation concludes.  Must be present if a bitmap was
    /// provided, Must NOT be present otherwise.  (Since 4.2)
    #[qapi(name = "bitmap-mode")]
    pub bitmap_mode: Option<BitmapSyncMode>,
    /// true to compress data, if the target format supports it.
    /// (default: false) (since 2.8)
    #[qapi(name = "compress")]
    pub compress: Option<bool>,
    /// the action to take on an error on the source,
    /// default 'report'.  'stop' and 'enospc' can only be used if the
    /// block device supports io-status (see BlockInfo).
    #[qapi(name = "on-source-error")]
    pub on_source_error: Option<BlockdevOnError>,
    /// the action to take on an error on the target,
    /// default 'report' (no limitations, since this applies to a
    /// different block device than @device).
    #[qapi(name = "on-target-error")]
    pub on_target_error: Option<BlockdevOnError>,
    /// When false, this job will wait in a PENDING state
    /// after it has finished its work, waiting for @block-job-finalize
    /// before making any block graph changes.  When true, this job will
    /// automatically perform its abort or commit actions.  Defaults to
    /// true.  (Since 2.12)
    #[qapi(name = "auto-finalize")]
    pub auto_finalize: Option<bool>,
    /// When false, this job will wait in a CONCLUDED state
    /// after it has completely ceased all work, and awaits
    /// @block-job-dismiss.  When true, this job will automatically
    /// disappear from the query list without user intervention.
    /// Defaults to true.  (Since 2.12)
    #[qapi(name = "auto-dismiss")]
    pub auto_dismiss: Option<bool>,
    /// the node name that should be assigned to the
    /// filter driver that the backup job inserts into the graph above
    /// node specified by @drive.  If this option is not given, a node
    /// name is autogenerated.  (Since: 4.2)
    #[qapi(name = "filter-node-name")]
    pub filter_node_name: Option<String>,
    /// Discard blocks on source which have already been
    /// copied to the target.  (Since 9.1)
    #[qapi(name = "discard-source")]
    pub discard_source: Option<bool>,
    /// Performance options.  (Since 6.0)
    #[qapi(name = "x-perf")]
    #[qapi(feature = "unstable")]
    pub x_perf: Option<BackupPerf>,
}
#[qapi(name = "DriveBackup")]
#[qapi(since = "1.6")]
pub struct DriveBackup {
    /// identifier for the newly-created block job.  If omitted,
    /// the device name will be used.  (Since 2.7)
    #[qapi(name = "job-id")]
    pub job_id: Option<String>,
    /// the device name or node-name of a root node which should be
    /// copied.
    #[qapi(name = "device")]
    pub device: String,
    /// what parts of the disk image should be copied to the
    /// destination (all the disk, only the sectors allocated in the
    /// topmost image, from a dirty bitmap, or only new I/O).
    #[qapi(name = "sync")]
    pub sync: MirrorSyncMode,
    /// the maximum speed, in bytes per second.  The default is 0,
    /// for unlimited.
    #[qapi(name = "speed")]
    pub speed: Option<i64>,
    /// The name of a dirty bitmap to use.  Must be present if sync
    /// is "bitmap" or "incremental".  Can be present if sync is "full"
    /// or "top".  Must not be present otherwise.
    /// (Since 2.4 (drive-backup), 3.1 (blockdev-backup))
    #[qapi(name = "bitmap")]
    pub bitmap: Option<String>,
    /// Specifies the type of data the bitmap should contain
    /// after the operation concludes.  Must be present if a bitmap was
    /// provided, Must NOT be present otherwise.  (Since 4.2)
    #[qapi(name = "bitmap-mode")]
    pub bitmap_mode: Option<BitmapSyncMode>,
    /// true to compress data, if the target format supports it.
    /// (default: false) (since 2.8)
    #[qapi(name = "compress")]
    pub compress: Option<bool>,
    /// the action to take on an error on the source,
    /// default 'report'.  'stop' and 'enospc' can only be used if the
    /// block device supports io-status (see BlockInfo).
    #[qapi(name = "on-source-error")]
    pub on_source_error: Option<BlockdevOnError>,
    /// the action to take on an error on the target,
    /// default 'report' (no limitations, since this applies to a
    /// different block device than @device).
    #[qapi(name = "on-target-error")]
    pub on_target_error: Option<BlockdevOnError>,
    /// When false, this job will wait in a PENDING state
    /// after it has finished its work, waiting for @block-job-finalize
    /// before making any block graph changes.  When true, this job will
    /// automatically perform its abort or commit actions.  Defaults to
    /// true.  (Since 2.12)
    #[qapi(name = "auto-finalize")]
    pub auto_finalize: Option<bool>,
    /// When false, this job will wait in a CONCLUDED state
    /// after it has completely ceased all work, and awaits
    /// @block-job-dismiss.  When true, this job will automatically
    /// disappear from the query list without user intervention.
    /// Defaults to true.  (Since 2.12)
    #[qapi(name = "auto-dismiss")]
    pub auto_dismiss: Option<bool>,
    /// the node name that should be assigned to the
    /// filter driver that the backup job inserts into the graph above
    /// node specified by @drive.  If this option is not given, a node
    /// name is autogenerated.  (Since: 4.2)
    #[qapi(name = "filter-node-name")]
    pub filter_node_name: Option<String>,
    /// Discard blocks on source which have already been
    /// copied to the target.  (Since 9.1)
    #[qapi(name = "discard-source")]
    pub discard_source: Option<bool>,
    /// Performance options.  (Since 6.0)
    #[qapi(name = "x-perf")]
    #[qapi(feature = "unstable")]
    pub x_perf: Option<BackupPerf>,
    /// the target of the new image.  If the file exists, or if it
    /// is a device, the existing file/device will be used as the new
    /// destination.  If it does not exist, a new file will be created.
    #[qapi(name = "target")]
    pub target: String,
    /// the format of the new destination, default is to probe if
    /// @mode is 'existing', else the format of the source
    #[qapi(name = "format")]
    pub format: Option<String>,
    /// whether and how QEMU should create a new image, default is
    /// 'absolute-paths'.
    #[qapi(name = "mode")]
    pub mode: Option<NewImageMode>,
}
#[qapi(name = "BlockdevBackup")]
#[qapi(since = "2.3")]
pub struct BlockdevBackup {
    /// identifier for the newly-created block job.  If omitted,
    /// the device name will be used.  (Since 2.7)
    #[qapi(name = "job-id")]
    pub job_id: Option<String>,
    /// the device name or node-name of a root node which should be
    /// copied.
    #[qapi(name = "device")]
    pub device: String,
    /// what parts of the disk image should be copied to the
    /// destination (all the disk, only the sectors allocated in the
    /// topmost image, from a dirty bitmap, or only new I/O).
    #[qapi(name = "sync")]
    pub sync: MirrorSyncMode,
    /// the maximum speed, in bytes per second.  The default is 0,
    /// for unlimited.
    #[qapi(name = "speed")]
    pub speed: Option<i64>,
    /// The name of a dirty bitmap to use.  Must be present if sync
    /// is "bitmap" or "incremental".  Can be present if sync is "full"
    /// or "top".  Must not be present otherwise.
    /// (Since 2.4 (drive-backup), 3.1 (blockdev-backup))
    #[qapi(name = "bitmap")]
    pub bitmap: Option<String>,
    /// Specifies the type of data the bitmap should contain
    /// after the operation concludes.  Must be present if a bitmap was
    /// provided, Must NOT be present otherwise.  (Since 4.2)
    #[qapi(name = "bitmap-mode")]
    pub bitmap_mode: Option<BitmapSyncMode>,
    /// true to compress data, if the target format supports it.
    /// (default: false) (since 2.8)
    #[qapi(name = "compress")]
    pub compress: Option<bool>,
    /// the action to take on an error on the source,
    /// default 'report'.  'stop' and 'enospc' can only be used if the
    /// block device supports io-status (see BlockInfo).
    #[qapi(name = "on-source-error")]
    pub on_source_error: Option<BlockdevOnError>,
    /// the action to take on an error on the target,
    /// default 'report' (no limitations, since this applies to a
    /// different block device than @device).
    #[qapi(name = "on-target-error")]
    pub on_target_error: Option<BlockdevOnError>,
    /// When false, this job will wait in a PENDING state
    /// after it has finished its work, waiting for @block-job-finalize
    /// before making any block graph changes.  When true, this job will
    /// automatically perform its abort or commit actions.  Defaults to
    /// true.  (Since 2.12)
    #[qapi(name = "auto-finalize")]
    pub auto_finalize: Option<bool>,
    /// When false, this job will wait in a CONCLUDED state
    /// after it has completely ceased all work, and awaits
    /// @block-job-dismiss.  When true, this job will automatically
    /// disappear from the query list without user intervention.
    /// Defaults to true.  (Since 2.12)
    #[qapi(name = "auto-dismiss")]
    pub auto_dismiss: Option<bool>,
    /// the node name that should be assigned to the
    /// filter driver that the backup job inserts into the graph above
    /// node specified by @drive.  If this option is not given, a node
    /// name is autogenerated.  (Since: 4.2)
    #[qapi(name = "filter-node-name")]
    pub filter_node_name: Option<String>,
    /// Discard blocks on source which have already been
    /// copied to the target.  (Since 9.1)
    #[qapi(name = "discard-source")]
    pub discard_source: Option<bool>,
    /// Performance options.  (Since 6.0)
    #[qapi(name = "x-perf")]
    #[qapi(feature = "unstable")]
    pub x_perf: Option<BackupPerf>,
    /// the device name or node-name of the backup target node.
    #[qapi(name = "target")]
    pub target: String,
}
/// Takes a synchronous snapshot of a block device.
#[qapi(name = "blockdev-snapshot-sync")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockdevSnapshotSync {
    #[qapi(flatten)]
    pub data: BlockdevSnapshotSync,
}
/// Takes a snapshot of a block device.
///
/// Take a snapshot, by installing 'node' as the backing image of
/// 'overlay'.  Additionally, if 'node' is associated with a block
/// device, the block device changes to using 'overlay' as its new
/// active image.
#[qapi(name = "blockdev-snapshot")]
#[qapi(feature = "allow-write-only-overlay")]
#[qapi(since = "2.5")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockdevSnapshot {
    #[qapi(flatten)]
    pub data: BlockdevSnapshot,
}
/// Change the backing file in the image file metadata.  This does not
/// cause QEMU to reopen the image file to reparse the backing filename
/// (it may, however, perform a reopen to change permissions from r/o ->
/// r/w -> r/o, if needed).  The new backing file string is written into
/// the image file metadata, and the QEMU internal strings are updated.
#[qapi(name = "change-backing-file")]
#[qapi(since = "2.1")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct ChangeBackingFile {
    /// The device name or node-name of the root node that owns
    /// image-node-name.
    #[qapi(name = "device")]
    pub device: String,
    /// The name of the block driver state node of the
    /// image to modify.  The "device" argument is used to verify
    /// "image-node-name" is in the chain described by "device".
    #[qapi(name = "image-node-name")]
    pub image_node_name: String,
    /// The string to write as the backing file.  This string
    /// is not validated, so care should be taken when specifying the
    /// string or the image chain may not be able to be reopened again.
    #[qapi(name = "backing-file")]
    pub backing_file: String,
}
/// Live commit of data from overlay image nodes into backing nodes -
/// i.e., writes data between 'top' and 'base' into 'base'.
///
/// If top == base, that is an error.  If top has no overlays on top of
/// it, or if it is in use by a writer, the job will not be completed by
/// itself.  The user needs to complete the job with the
/// block-job-complete command after getting the ready event.  (Since
/// 2.0)
///
/// If the base image is smaller than top, then the base image will be
/// resized to be the same size as top.  If top is smaller than the base
/// image, the base will not be truncated.  If you want the base image
/// size to match the size of the smaller top, you can safely truncate
/// it yourself once the commit operation successfully completes.
#[qapi(name = "block-commit")]
#[qapi(since = "1.3")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockCommit {
    /// identifier for the newly-created block job.  If omitted,
    /// the device name will be used.  (Since 2.7)
    #[qapi(name = "job-id")]
    pub job_id: Option<String>,
    /// the device name or node-name of a root node
    #[qapi(name = "device")]
    pub device: String,
    /// The node name of the backing image to write data into.
    /// If not specified, this is the deepest backing image.
    /// (since: 3.1)
    #[qapi(name = "base-node")]
    pub base_node: Option<String>,
    /// Same as @base-node, except that it is a file name rather than
    /// a node name.  This must be the exact filename string that was
    /// used to open the node; other strings, even if addressing the
    /// same file, are not accepted
    #[qapi(name = "base")]
    #[qapi(feature = "deprecated")]
    pub base: Option<String>,
    /// The node name of the backing image within the image chain
    /// which contains the topmost data to be committed down.  If not
    /// specified, this is the active layer.  (since: 3.1)
    #[qapi(name = "top-node")]
    pub top_node: Option<String>,
    /// Same as @top-node, except that it is a file name rather than a
    /// node name.  This must be the exact filename string that was used
    /// to open the node; other strings, even if addressing the same
    /// file, are not accepted
    #[qapi(name = "top")]
    #[qapi(feature = "deprecated")]
    pub top: Option<String>,
    /// The backing file string to write into the overlay
    /// image of 'top'.  If 'top' does not have an overlay image, or if
    /// 'top' is in use by a writer, specifying a backing file string is
    /// an error.
    ///
    /// This filename is not validated.  If a pathname string is such
    /// that it cannot be resolved by QEMU, that means that subsequent
    /// QMP or HMP commands must use node-names for the image in
    /// question, as filename lookup methods will fail.
    ///
    /// If not specified, QEMU will automatically determine the backing
    /// file string to use, or error out if there is no obvious choice.
    /// Care should be taken when specifying the string, to specify a
    /// valid filename or protocol.  (Since 2.1)
    #[qapi(name = "backing-file")]
    pub backing_file: Option<String>,
    /// If true, replace any protocol mentioned in
    /// the 'backing file format' with 'raw', rather than storing the
    /// protocol name as the backing format.  Can be used even when no
    /// image header will be updated (default false; since 9.0).
    #[qapi(name = "backing-mask-protocol")]
    pub backing_mask_protocol: Option<bool>,
    /// the maximum speed, in bytes per second
    #[qapi(name = "speed")]
    pub speed: Option<i64>,
    /// the action to take on an error.  'ignore' means that the
    /// request should be retried.  (default: report; Since: 5.0)
    #[qapi(name = "on-error")]
    pub on_error: Option<BlockdevOnError>,
    /// the node name that should be assigned to the
    /// filter driver that the commit job inserts into the graph above
    /// @top.  If this option is not given, a node name is
    /// autogenerated.  (Since: 2.9)
    #[qapi(name = "filter-node-name")]
    pub filter_node_name: Option<String>,
    /// When false, this job will wait in a PENDING state
    /// after it has finished its work, waiting for @block-job-finalize
    /// before making any block graph changes.  When true, this job will
    /// automatically perform its abort or commit actions.  Defaults to
    /// true.  (Since 3.1)
    #[qapi(name = "auto-finalize")]
    pub auto_finalize: Option<bool>,
    /// When false, this job will wait in a CONCLUDED state
    /// after it has completely ceased all work, and awaits
    /// @block-job-dismiss.  When true, this job will automatically
    /// disappear from the query list without user intervention.
    /// Defaults to true.  (Since 3.1)
    #[qapi(name = "auto-dismiss")]
    pub auto_dismiss: Option<bool>,
}
/// Start a point-in-time copy of a block device to a new destination.
/// The status of ongoing drive-backup operations can be checked with
/// query-block-jobs where the BlockJobInfo.type field has the value
/// 'backup'.  The operation can be stopped before it has completed
/// using the block-job-cancel command.
#[qapi(name = "drive-backup")]
#[qapi(feature = "deprecated")]
#[qapi(since = "1.6")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct DriveBackup {
    #[qapi(flatten)]
    pub data: DriveBackup,
}
/// Start a point-in-time copy of a block device to a new destination.
/// The status of ongoing blockdev-backup operations can be checked with
/// query-block-jobs where the BlockJobInfo.type field has the value
/// 'backup'.  The operation can be stopped before it has completed
/// using the block-job-cancel command.
#[qapi(name = "blockdev-backup")]
#[qapi(since = "2.3")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockdevBackup {
    #[qapi(flatten)]
    pub data: BlockdevBackup,
}
/// Get the named block driver list
#[qapi(name = "query-named-block-nodes")]
#[qapi(since = "2.0")]
#[qapi(returns = "Vec<BlockDeviceInfo>")]
#[qapi(allow_preconfig)]
pub struct QueryNamedBlockNodes {
    /// Omit the nested data about backing image ("backing-image"
    /// key) if true.  Default is false (Since 5.0)
    #[qapi(name = "flat")]
    pub flat: Option<bool>,
}
#[qapi(name = "XDbgBlockGraphNodeType")]
#[qapi(since = "4.0")]
pub enum XDbgBlockGraphNodeType {
    /// corresponds to BlockBackend
    #[qapi(name = "block-backend")]
    BlockBackend,
    /// corresponds to BlockJob
    #[qapi(name = "block-job")]
    BlockJob,
    /// corresponds to BlockDriverState
    #[qapi(name = "block-driver")]
    BlockDriver,
}
#[qapi(name = "XDbgBlockGraphNode")]
#[qapi(since = "4.0")]
pub struct XDbgBlockGraphNode {
    /// Block graph node identifier.  This @id is generated only for
    /// x-debug-query-block-graph and does not relate to any other
    /// identifiers in Qemu.
    #[qapi(name = "id")]
    pub id: u64,
    /// Type of graph node.  Can be one of block-backend, block-job
    /// or block-driver-state.
    #[qapi(name = "type")]
    pub r#type: XDbgBlockGraphNodeType,
    /// Human readable name of the node.  Corresponds to node-name
    /// for block-driver-state nodes; is not guaranteed to be unique in
    /// the whole graph (with block-jobs and block-backends).
    #[qapi(name = "name")]
    pub name: String,
}
/// Enum of base block permissions.
#[qapi(name = "BlockPermission")]
#[qapi(since = "4.0")]
pub enum BlockPermission {
    /// A user that has the "permission" of consistent
    /// reads is guaranteed that their view of the contents of the block
    /// device is complete and self-consistent, representing the
    /// contents of a disk at a specific point.  For most block devices
    /// (including their backing files) this is true, but the property
    /// cannot be maintained in a few situations like for intermediate
    /// nodes of a commit block job.
    #[qapi(name = "consistent-read")]
    ConsistentRead,
    /// This permission is required to change the visible disk
    /// contents.
    #[qapi(name = "write")]
    Write,
    /// This permission (which is weaker than
    /// BLK_PERM_WRITE) is both enough and required for writes to the
    /// block node when the caller promises that the visible disk
    /// content doesn't change.  As the BLK_PERM_WRITE permission is
    /// strictly stronger, either is sufficient to perform an unchanging
    /// write.
    #[qapi(name = "write-unchanged")]
    WriteUnchanged,
    /// This permission is required to change the size of a block
    /// node.
    #[qapi(name = "resize")]
    Resize,
}
/// Block Graph edge description for x-debug-query-block-graph.
#[qapi(name = "XDbgBlockGraphEdge")]
#[qapi(since = "4.0")]
pub struct XDbgBlockGraphEdge {
    /// parent id
    #[qapi(name = "parent")]
    pub parent: u64,
    /// child id
    #[qapi(name = "child")]
    pub child: u64,
    /// name of the relation (examples are 'file' and 'backing')
    #[qapi(name = "name")]
    pub name: String,
    /// granted permissions for the parent operating on the child
    #[qapi(name = "perm")]
    pub perm: Vec<BlockPermission>,
    /// permissions that can still be granted to other users
    /// of the child while it is still attached to this parent
    #[qapi(name = "shared-perm")]
    pub shared_perm: Vec<BlockPermission>,
}
/// Block Graph - list of nodes and list of edges.
#[qapi(name = "XDbgBlockGraph")]
#[qapi(since = "4.0")]
pub struct XDbgBlockGraph {
    #[qapi(name = "nodes")]
    pub nodes: Vec<XDbgBlockGraphNode>,
    #[qapi(name = "edges")]
    pub edges: Vec<XDbgBlockGraphEdge>,
}
/// Get the block graph.
#[qapi(name = "x-debug-query-block-graph")]
#[qapi(feature = "unstable")]
#[qapi(since = "4.0")]
#[qapi(returns = "XDbgBlockGraph")]
#[qapi(allow_preconfig)]
pub struct XDebugQueryBlockGraph {}
/// Start mirroring a block device's writes to a new destination.
/// target specifies the target of the new image.  If the file exists,
/// or if it is a device, it will be used as the new destination for
/// writes.  If it does not exist, a new file will be created.  @format
/// specifies the format of the mirror image, default is to probe if
/// mode='existing', else the format of the source.
#[qapi(name = "drive-mirror")]
#[qapi(since = "1.3")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct DriveMirror {
    #[qapi(flatten)]
    pub data: DriveMirror,
}
/// A set of parameters describing drive mirror setup.
#[qapi(name = "DriveMirror")]
#[qapi(since = "1.3")]
pub struct DriveMirror {
    /// identifier for the newly-created block job.  If omitted,
    /// the device name will be used.  (Since 2.7)
    #[qapi(name = "job-id")]
    pub job_id: Option<String>,
    /// the device name or node-name of a root node whose writes
    /// should be mirrored.
    #[qapi(name = "device")]
    pub device: String,
    /// the target of the new image.  If the file exists, or if it
    /// is a device, the existing file/device will be used as the new
    /// destination.  If it does not exist, a new file will be created.
    #[qapi(name = "target")]
    pub target: String,
    /// the format of the new destination, default is to probe if
    /// @mode is 'existing', else the format of the source
    #[qapi(name = "format")]
    pub format: Option<String>,
    /// the new block driver state node name in the graph (Since
    /// 2.1)
    #[qapi(name = "node-name")]
    pub node_name: Option<String>,
    /// with sync=full graph node name to be replaced by the new
    /// image when a whole image copy is done.  This can be used to
    /// repair broken Quorum files.  By default, @device is replaced,
    /// although implicitly created filters on it are kept.  (Since 2.1)
    #[qapi(name = "replaces")]
    pub replaces: Option<String>,
    /// what parts of the disk image should be copied to the
    /// destination (all the disk, only the sectors allocated in the
    /// topmost image, or only new I/O).
    #[qapi(name = "sync")]
    pub sync: MirrorSyncMode,
    /// whether and how QEMU should create a new image, default is
    /// 'absolute-paths'.
    #[qapi(name = "mode")]
    pub mode: Option<NewImageMode>,
    /// the maximum speed, in bytes per second
    #[qapi(name = "speed")]
    pub speed: Option<i64>,
    /// granularity of the dirty bitmap, default is 64K if the
    /// image format doesn't have clusters, 4K if the clusters are
    /// smaller than that, else the cluster size.  Must be a power of 2
    /// between 512 and 64M (since 1.4).
    #[qapi(name = "granularity")]
    pub granularity: Option<u32>,
    /// maximum amount of data in flight from source to target
    /// (since 1.4).
    #[qapi(name = "buf-size")]
    pub buf_size: Option<i64>,
    /// the action to take on an error on the source,
    /// default 'report'.  'stop' and 'enospc' can only be used if the
    /// block device supports io-status (see BlockInfo).
    #[qapi(name = "on-source-error")]
    pub on_source_error: Option<BlockdevOnError>,
    /// the action to take on an error on the target,
    /// default 'report' (no limitations, since this applies to a
    /// different block device than @device).
    #[qapi(name = "on-target-error")]
    pub on_target_error: Option<BlockdevOnError>,
    /// Whether to try to unmap target sectors where source has only
    /// zero.  If true, and target unallocated sectors will read as
    /// zero, target image sectors will be unmapped; otherwise, zeroes
    /// will be written.  Both will result in identical contents.
    /// Default is true.  (Since 2.4)
    #[qapi(name = "unmap")]
    pub unmap: Option<bool>,
    /// when to copy data to the destination; defaults to
    /// 'background' (Since: 3.0)
    #[qapi(name = "copy-mode")]
    pub copy_mode: Option<MirrorCopyMode>,
    /// When false, this job will wait in a PENDING state
    /// after it has finished its work, waiting for @block-job-finalize
    /// before making any block graph changes.  When true, this job will
    /// automatically perform its abort or commit actions.  Defaults to
    /// true.  (Since 3.1)
    #[qapi(name = "auto-finalize")]
    pub auto_finalize: Option<bool>,
    /// When false, this job will wait in a CONCLUDED state
    /// after it has completely ceased all work, and awaits
    /// @block-job-dismiss.  When true, this job will automatically
    /// disappear from the query list without user intervention.
    /// Defaults to true.  (Since 3.1)
    #[qapi(name = "auto-dismiss")]
    pub auto_dismiss: Option<bool>,
}
#[qapi(name = "BlockDirtyBitmap")]
#[qapi(since = "2.4")]
pub struct BlockDirtyBitmap {
    /// name of device/node which the bitmap is tracking
    #[qapi(name = "node")]
    pub node: String,
    /// name of the dirty bitmap
    #[qapi(name = "name")]
    pub name: String,
}
#[qapi(name = "BlockDirtyBitmapAdd")]
#[qapi(since = "2.4")]
pub struct BlockDirtyBitmapAdd {
    /// name of device/node which the bitmap is tracking
    #[qapi(name = "node")]
    pub node: String,
    /// name of the dirty bitmap (must be less than 1024 bytes)
    #[qapi(name = "name")]
    pub name: String,
    /// the bitmap granularity, default is 64k for
    /// block-dirty-bitmap-add
    #[qapi(name = "granularity")]
    pub granularity: Option<u32>,
    /// the bitmap is persistent, i.e. it will be saved to the
    /// corresponding block device image file on its close.  For now
    /// only Qcow2 disks support persistent bitmaps.  Default is false
    /// for block-dirty-bitmap-add.  (Since: 2.10)
    #[qapi(name = "persistent")]
    pub persistent: Option<bool>,
    /// the bitmap is created in the disabled state, which means
    /// that it will not track drive changes.  The bitmap may be enabled
    /// with block-dirty-bitmap-enable.  Default is false.  (Since: 4.0)
    #[qapi(name = "disabled")]
    pub disabled: Option<bool>,
}
#[qapi(name = "BlockDirtyBitmapOrStr")]
#[qapi(since = "4.1")]
pub enum BlockDirtyBitmapOrStr {
    /// name of the bitmap, attached to the same node as target
    /// bitmap.
    #[qapi(name = "local")]
    Local(String),
    /// bitmap with specified node
    #[qapi(name = "external")]
    External(BlockDirtyBitmap),
}
#[qapi(name = "BlockDirtyBitmapMerge")]
#[qapi(since = "4.0")]
pub struct BlockDirtyBitmapMerge {
    /// name of device/node which the @target bitmap is tracking
    #[qapi(name = "node")]
    pub node: String,
    /// name of the destination dirty bitmap
    #[qapi(name = "target")]
    pub target: String,
    /// name(s) of the source dirty bitmap(s) at @node and/or
    /// fully specified BlockDirtyBitmap elements.  The latter are
    /// supported since 4.1.
    #[qapi(name = "bitmaps")]
    pub bitmaps: Vec<BlockDirtyBitmapOrStr>,
}
/// Create a dirty bitmap with a name on the node, and start tracking
/// the writes.
#[qapi(name = "block-dirty-bitmap-add")]
#[qapi(since = "2.4")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockDirtyBitmapAdd {
    #[qapi(flatten)]
    pub data: BlockDirtyBitmapAdd,
}
/// Stop write tracking and remove the dirty bitmap that was created
/// with block-dirty-bitmap-add.  If the bitmap is persistent, remove it
/// from its storage too.
#[qapi(name = "block-dirty-bitmap-remove")]
#[qapi(since = "2.4")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockDirtyBitmapRemove {
    #[qapi(flatten)]
    pub data: BlockDirtyBitmap,
}
/// Clear (reset) a dirty bitmap on the device, so that an incremental
/// backup from this point in time forward will only backup clusters
/// modified after this clear operation.
#[qapi(name = "block-dirty-bitmap-clear")]
#[qapi(since = "2.4")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockDirtyBitmapClear {
    #[qapi(flatten)]
    pub data: BlockDirtyBitmap,
}
/// Enables a dirty bitmap so that it will begin tracking disk changes.
#[qapi(name = "block-dirty-bitmap-enable")]
#[qapi(since = "4.0")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockDirtyBitmapEnable {
    #[qapi(flatten)]
    pub data: BlockDirtyBitmap,
}
/// Disables a dirty bitmap so that it will stop tracking disk changes.
#[qapi(name = "block-dirty-bitmap-disable")]
#[qapi(since = "4.0")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockDirtyBitmapDisable {
    #[qapi(flatten)]
    pub data: BlockDirtyBitmap,
}
/// Merge dirty bitmaps listed in @bitmaps to the @target dirty bitmap.
/// Dirty bitmaps in @bitmaps will be unchanged, except if it also
/// appears as the @target bitmap.  Any bits already set in @target will
/// still be set after the merge, i.e., this operation does not clear
/// the target.  On error, @target is unchanged.
///
/// The resulting bitmap will count as dirty any clusters that were
/// dirty in any of the source bitmaps.  This can be used to achieve
/// backup checkpoints, or in simpler usages, to copy bitmaps.
#[qapi(name = "block-dirty-bitmap-merge")]
#[qapi(since = "4.0")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockDirtyBitmapMerge {
    #[qapi(flatten)]
    pub data: BlockDirtyBitmapMerge,
}
/// SHA256 hash of dirty bitmap data
#[qapi(name = "BlockDirtyBitmapSha256")]
#[qapi(since = "2.10")]
pub struct BlockDirtyBitmapSha256 {
    /// ASCII representation of SHA256 bitmap hash
    #[qapi(name = "sha256")]
    pub sha256: String,
}
/// Get bitmap SHA256.
#[qapi(name = "x-debug-block-dirty-bitmap-sha256")]
#[qapi(feature = "unstable")]
#[qapi(since = "2.10")]
#[qapi(returns = "BlockDirtyBitmapSha256")]
#[qapi(allow_preconfig)]
pub struct XDebugBlockDirtyBitmapSha256 {
    #[qapi(flatten)]
    pub data: BlockDirtyBitmap,
}
/// Start mirroring a block device's writes to a new destination.
#[qapi(name = "blockdev-mirror")]
#[qapi(since = "2.6")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockdevMirror {
    /// identifier for the newly-created block job.  If omitted,
    /// the device name will be used.  (Since 2.7)
    #[qapi(name = "job-id")]
    pub job_id: Option<String>,
    /// The device name or node-name of a root node whose writes
    /// should be mirrored.
    #[qapi(name = "device")]
    pub device: String,
    /// the id or node-name of the block device to mirror to.  This
    /// mustn't be attached to guest.
    #[qapi(name = "target")]
    pub target: String,
    /// with sync=full graph node name to be replaced by the new
    /// image when a whole image copy is done.  This can be used to
    /// repair broken Quorum files.  By default, @device is replaced,
    /// although implicitly created filters on it are kept.
    #[qapi(name = "replaces")]
    pub replaces: Option<String>,
    /// what parts of the disk image should be copied to the
    /// destination (all the disk, only the sectors allocated in the
    /// topmost image, or only new I/O).
    #[qapi(name = "sync")]
    pub sync: MirrorSyncMode,
    /// the maximum speed, in bytes per second
    #[qapi(name = "speed")]
    pub speed: Option<i64>,
    /// granularity of the dirty bitmap, default is 64K if the
    /// image format doesn't have clusters, 4K if the clusters are
    /// smaller than that, else the cluster size.  Must be a power of 2
    /// between 512 and 64M
    #[qapi(name = "granularity")]
    pub granularity: Option<u32>,
    /// maximum amount of data in flight from source to target
    #[qapi(name = "buf-size")]
    pub buf_size: Option<i64>,
    /// the action to take on an error on the source,
    /// default 'report'.  'stop' and 'enospc' can only be used if the
    /// block device supports io-status (see BlockInfo).
    #[qapi(name = "on-source-error")]
    pub on_source_error: Option<BlockdevOnError>,
    /// the action to take on an error on the target,
    /// default 'report' (no limitations, since this applies to a
    /// different block device than @device).
    #[qapi(name = "on-target-error")]
    pub on_target_error: Option<BlockdevOnError>,
    /// the node name that should be assigned to the
    /// filter driver that the mirror job inserts into the graph above
    /// @device.  If this option is not given, a node name is
    /// autogenerated.  (Since: 2.9)
    #[qapi(name = "filter-node-name")]
    pub filter_node_name: Option<String>,
    /// when to copy data to the destination; defaults to
    /// 'background' (Since: 3.0)
    #[qapi(name = "copy-mode")]
    pub copy_mode: Option<MirrorCopyMode>,
    /// When false, this job will wait in a PENDING state
    /// after it has finished its work, waiting for @block-job-finalize
    /// before making any block graph changes.  When true, this job will
    /// automatically perform its abort or commit actions.  Defaults to
    /// true.  (Since 3.1)
    #[qapi(name = "auto-finalize")]
    pub auto_finalize: Option<bool>,
    /// When false, this job will wait in a CONCLUDED state
    /// after it has completely ceased all work, and awaits
    /// @block-job-dismiss.  When true, this job will automatically
    /// disappear from the query list without user intervention.
    /// Defaults to true.  (Since 3.1)
    #[qapi(name = "auto-dismiss")]
    pub auto_dismiss: Option<bool>,
}
/// A set of parameters describing block throttling.
#[qapi(name = "BlockIOThrottle")]
#[qapi(since = "1.1")]
pub struct BlockIoThrottle {
    /// Block device name
    #[qapi(name = "device")]
    #[qapi(feature = "deprecated")]
    pub device: Option<String>,
    /// The name or QOM path of the guest device (since: 2.8)
    #[qapi(name = "id")]
    pub id: Option<String>,
    /// total throughput limit in bytes per second
    #[qapi(name = "bps")]
    pub bps: i64,
    /// read throughput limit in bytes per second
    #[qapi(name = "bps_rd")]
    pub bps_rd: i64,
    /// write throughput limit in bytes per second
    #[qapi(name = "bps_wr")]
    pub bps_wr: i64,
    /// total I/O operations per second
    #[qapi(name = "iops")]
    pub iops: i64,
    /// read I/O operations per second
    #[qapi(name = "iops_rd")]
    pub iops_rd: i64,
    /// write I/O operations per second
    #[qapi(name = "iops_wr")]
    pub iops_wr: i64,
    /// total throughput limit during bursts, in bytes (Since 1.7)
    #[qapi(name = "bps_max")]
    pub bps_max: Option<i64>,
    /// read throughput limit during bursts, in bytes (Since
    /// 1.7)
    #[qapi(name = "bps_rd_max")]
    pub bps_rd_max: Option<i64>,
    /// write throughput limit during bursts, in bytes (Since
    /// 1.7)
    #[qapi(name = "bps_wr_max")]
    pub bps_wr_max: Option<i64>,
    /// total I/O operations per second during bursts, in bytes
    /// (Since 1.7)
    #[qapi(name = "iops_max")]
    pub iops_max: Option<i64>,
    /// read I/O operations per second during bursts, in bytes
    /// (Since 1.7)
    #[qapi(name = "iops_rd_max")]
    pub iops_rd_max: Option<i64>,
    /// write I/O operations per second during bursts, in
    /// bytes (Since 1.7)
    #[qapi(name = "iops_wr_max")]
    pub iops_wr_max: Option<i64>,
    /// maximum length of the @bps_max burst period, in
    /// seconds.  It must only be set if @bps_max is set as well.
    /// Defaults to 1.  (Since 2.6)
    #[qapi(name = "bps_max_length")]
    pub bps_max_length: Option<i64>,
    /// maximum length of the @bps_rd_max burst period,
    /// in seconds.  It must only be set if @bps_rd_max is set as well.
    /// Defaults to 1.  (Since 2.6)
    #[qapi(name = "bps_rd_max_length")]
    pub bps_rd_max_length: Option<i64>,
    /// maximum length of the @bps_wr_max burst period,
    /// in seconds.  It must only be set if @bps_wr_max is set as well.
    /// Defaults to 1.  (Since 2.6)
    #[qapi(name = "bps_wr_max_length")]
    pub bps_wr_max_length: Option<i64>,
    /// maximum length of the @iops burst period, in
    /// seconds.  It must only be set if @iops_max is set as well.
    /// Defaults to 1.  (Since 2.6)
    #[qapi(name = "iops_max_length")]
    pub iops_max_length: Option<i64>,
    /// maximum length of the @iops_rd_max burst
    /// period, in seconds.  It must only be set if @iops_rd_max is set
    /// as well.  Defaults to 1.  (Since 2.6)
    #[qapi(name = "iops_rd_max_length")]
    pub iops_rd_max_length: Option<i64>,
    /// maximum length of the @iops_wr_max burst
    /// period, in seconds.  It must only be set if @iops_wr_max is set
    /// as well.  Defaults to 1.  (Since 2.6)
    #[qapi(name = "iops_wr_max_length")]
    pub iops_wr_max_length: Option<i64>,
    /// an I/O size in bytes (Since 1.7)
    #[qapi(name = "iops_size")]
    pub iops_size: Option<i64>,
    /// throttle group name (Since 2.4)
    #[qapi(name = "group")]
    pub group: Option<String>,
}
/// Limit parameters for throttling.  Since some limit combinations are
/// illegal, limits should always be set in one transaction.  All fields
/// are optional.  When setting limits, if a field is missing the
/// current value is not changed.
#[qapi(name = "ThrottleLimits")]
#[qapi(since = "2.11")]
pub struct ThrottleLimits {
    /// limit total I/O operations per second
    #[qapi(name = "iops-total")]
    pub iops_total: Option<i64>,
    /// I/O operations burst
    #[qapi(name = "iops-total-max")]
    pub iops_total_max: Option<i64>,
    /// length of the iops-total-max burst period,
    /// in seconds It must only be set if @iops-total-max is set as
    /// well.
    #[qapi(name = "iops-total-max-length")]
    pub iops_total_max_length: Option<i64>,
    /// limit read operations per second
    #[qapi(name = "iops-read")]
    pub iops_read: Option<i64>,
    /// I/O operations read burst
    #[qapi(name = "iops-read-max")]
    pub iops_read_max: Option<i64>,
    /// length of the iops-read-max burst period, in
    /// seconds It must only be set if @iops-read-max is set as well.
    #[qapi(name = "iops-read-max-length")]
    pub iops_read_max_length: Option<i64>,
    /// limit write operations per second
    #[qapi(name = "iops-write")]
    pub iops_write: Option<i64>,
    /// I/O operations write burst
    #[qapi(name = "iops-write-max")]
    pub iops_write_max: Option<i64>,
    /// length of the iops-write-max burst period,
    /// in seconds It must only be set if @iops-write-max is set as
    /// well.
    #[qapi(name = "iops-write-max-length")]
    pub iops_write_max_length: Option<i64>,
    /// limit total bytes per second
    #[qapi(name = "bps-total")]
    pub bps_total: Option<i64>,
    /// total bytes burst
    #[qapi(name = "bps-total-max")]
    pub bps_total_max: Option<i64>,
    /// length of the bps-total-max burst period, in
    /// seconds.  It must only be set if @bps-total-max is set as well.
    #[qapi(name = "bps-total-max-length")]
    pub bps_total_max_length: Option<i64>,
    /// limit read bytes per second
    #[qapi(name = "bps-read")]
    pub bps_read: Option<i64>,
    /// total bytes read burst
    #[qapi(name = "bps-read-max")]
    pub bps_read_max: Option<i64>,
    /// length of the bps-read-max burst period, in
    /// seconds It must only be set if @bps-read-max is set as well.
    #[qapi(name = "bps-read-max-length")]
    pub bps_read_max_length: Option<i64>,
    /// limit write bytes per second
    #[qapi(name = "bps-write")]
    pub bps_write: Option<i64>,
    /// total bytes write burst
    #[qapi(name = "bps-write-max")]
    pub bps_write_max: Option<i64>,
    /// length of the bps-write-max burst period, in
    /// seconds It must only be set if @bps-write-max is set as well.
    #[qapi(name = "bps-write-max-length")]
    pub bps_write_max_length: Option<i64>,
    /// when limiting by iops max size of an I/O in bytes
    #[qapi(name = "iops-size")]
    pub iops_size: Option<i64>,
}
/// Properties for throttle-group objects.
#[qapi(name = "ThrottleGroupProperties")]
#[qapi(since = "2.11")]
pub struct ThrottleGroupProperties {
    /// limits to apply for this throttle group
    #[qapi(name = "limits")]
    pub limits: Option<ThrottleLimits>,
    #[qapi(name = "x-iops-total")]
    #[qapi(feature = "unstable")]
    pub x_iops_total: Option<i64>,
    #[qapi(name = "x-iops-total-max")]
    #[qapi(feature = "unstable")]
    pub x_iops_total_max: Option<i64>,
    #[qapi(name = "x-iops-total-max-length")]
    #[qapi(feature = "unstable")]
    pub x_iops_total_max_length: Option<i64>,
    #[qapi(name = "x-iops-read")]
    #[qapi(feature = "unstable")]
    pub x_iops_read: Option<i64>,
    #[qapi(name = "x-iops-read-max")]
    #[qapi(feature = "unstable")]
    pub x_iops_read_max: Option<i64>,
    #[qapi(name = "x-iops-read-max-length")]
    #[qapi(feature = "unstable")]
    pub x_iops_read_max_length: Option<i64>,
    #[qapi(name = "x-iops-write")]
    #[qapi(feature = "unstable")]
    pub x_iops_write: Option<i64>,
    #[qapi(name = "x-iops-write-max")]
    #[qapi(feature = "unstable")]
    pub x_iops_write_max: Option<i64>,
    #[qapi(name = "x-iops-write-max-length")]
    #[qapi(feature = "unstable")]
    pub x_iops_write_max_length: Option<i64>,
    #[qapi(name = "x-bps-total")]
    #[qapi(feature = "unstable")]
    pub x_bps_total: Option<i64>,
    #[qapi(name = "x-bps-total-max")]
    #[qapi(feature = "unstable")]
    pub x_bps_total_max: Option<i64>,
    #[qapi(name = "x-bps-total-max-length")]
    #[qapi(feature = "unstable")]
    pub x_bps_total_max_length: Option<i64>,
    #[qapi(name = "x-bps-read")]
    #[qapi(feature = "unstable")]
    pub x_bps_read: Option<i64>,
    #[qapi(name = "x-bps-read-max")]
    #[qapi(feature = "unstable")]
    pub x_bps_read_max: Option<i64>,
    #[qapi(name = "x-bps-read-max-length")]
    #[qapi(feature = "unstable")]
    pub x_bps_read_max_length: Option<i64>,
    #[qapi(name = "x-bps-write")]
    #[qapi(feature = "unstable")]
    pub x_bps_write: Option<i64>,
    #[qapi(name = "x-bps-write-max")]
    #[qapi(feature = "unstable")]
    pub x_bps_write_max: Option<i64>,
    #[qapi(name = "x-bps-write-max-length")]
    #[qapi(feature = "unstable")]
    pub x_bps_write_max_length: Option<i64>,
    #[qapi(name = "x-iops-size")]
    #[qapi(feature = "unstable")]
    pub x_iops_size: Option<i64>,
}
/// Copy data from a backing file into a block device.
///
/// The block streaming operation is performed in the background until
/// the entire backing file has been copied.  This command returns
/// immediately once streaming has started.  The status of ongoing block
/// streaming operations can be checked with query-block-jobs.  The
/// operation can be stopped before it has completed using the
/// block-job-cancel command.
///
/// The node that receives the data is called the top image, can be
/// located in any part of the chain (but always above the base image;
/// see below) and can be specified using its device or node name.
/// Earlier qemu versions only allowed 'device' to name the top level
/// node; presence of the 'base-node' parameter during introspection can
/// be used as a witness of the enhanced semantics of 'device'.
///
/// If a base file is specified then sectors are not copied from that
/// base file and its backing chain.  This can be used to stream a
/// subset of the backing file chain instead of flattening the entire
/// image.  When streaming completes the image file will have the base
/// file as its backing file, unless that node was changed while the job
/// was running.  In that case, base's parent's backing (or filtered,
/// whichever exists) child (i.e., base at the beginning of the job)
/// will be the new backing file.
///
/// On successful completion the image file is updated to drop the
/// backing file and the BLOCK_JOB_COMPLETED event is emitted.
///
/// In case @device is a filter node, block-stream modifies the first
/// non-filter overlay node below it to point to the new backing node
/// instead of modifying @device itself.
#[qapi(name = "block-stream")]
#[qapi(since = "1.1")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockStream {
    /// identifier for the newly-created block job.  If omitted,
    /// the device name will be used.  (Since 2.7)
    #[qapi(name = "job-id")]
    pub job_id: Option<String>,
    /// the device or node name of the top image
    #[qapi(name = "device")]
    pub device: String,
    /// the common backing file name.  It cannot be set if @base-node
    /// or @bottom is also set.
    #[qapi(name = "base")]
    pub base: Option<String>,
    /// the node name of the backing file.  It cannot be set if
    /// @base or @bottom is also set.  (Since 2.8)
    #[qapi(name = "base-node")]
    pub base_node: Option<String>,
    /// The backing file string to write into the top image.
    /// This filename is not validated.
    ///
    /// If a pathname string is such that it cannot be resolved by QEMU,
    /// that means that subsequent QMP or HMP commands must use
    /// node-names for the image in question, as filename lookup methods
    /// will fail.
    ///
    /// If not specified, QEMU will automatically determine the backing
    /// file string to use, or error out if there is no obvious choice.
    /// Care should be taken when specifying the string, to specify a
    /// valid filename or protocol.  (Since 2.1)
    #[qapi(name = "backing-file")]
    pub backing_file: Option<String>,
    /// If true, replace any protocol mentioned in
    /// the 'backing file format' with 'raw', rather than storing the
    /// protocol name as the backing format.  Can be used even when no
    /// image header will be updated (default false; since 9.0).
    #[qapi(name = "backing-mask-protocol")]
    pub backing_mask_protocol: Option<bool>,
    /// the last node in the chain that should be streamed into
    /// top.  It cannot be set if @base or @base-node is also set.  It
    /// cannot be filter node.  (Since 6.0)
    #[qapi(name = "bottom")]
    pub bottom: Option<String>,
    /// the maximum speed, in bytes per second
    #[qapi(name = "speed")]
    pub speed: Option<i64>,
    /// the action to take on an error (default report).  'stop'
    /// and 'enospc' can only be used if the block device supports
    /// io-status (see BlockInfo).  (Since 1.3)
    #[qapi(name = "on-error")]
    pub on_error: Option<BlockdevOnError>,
    /// the node name that should be assigned to the
    /// filter driver that the stream job inserts into the graph above
    /// @device.  If this option is not given, a node name is
    /// autogenerated.  (Since: 6.0)
    #[qapi(name = "filter-node-name")]
    pub filter_node_name: Option<String>,
    /// When false, this job will wait in a PENDING state
    /// after it has finished its work, waiting for @block-job-finalize
    /// before making any block graph changes.  When true, this job will
    /// automatically perform its abort or commit actions.  Defaults to
    /// true.  (Since 3.1)
    #[qapi(name = "auto-finalize")]
    pub auto_finalize: Option<bool>,
    /// When false, this job will wait in a CONCLUDED state
    /// after it has completely ceased all work, and awaits
    /// @block-job-dismiss.  When true, this job will automatically
    /// disappear from the query list without user intervention.
    /// Defaults to true.  (Since 3.1)
    #[qapi(name = "auto-dismiss")]
    pub auto_dismiss: Option<bool>,
}
/// Set maximum speed for a background block operation.
///
/// This command can only be issued when there is an active block job.
///
/// Throttling can be disabled by setting the speed to 0.
#[qapi(name = "block-job-set-speed")]
#[qapi(since = "1.1")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockJobSetSpeed {
    /// The job identifier.  This used to be a device name (hence
    /// the name of the parameter), but since QEMU 2.7 it can have other
    /// values.
    #[qapi(name = "device")]
    pub device: String,
    /// the maximum speed, in bytes per second, or 0 for unlimited.
    /// Defaults to 0.
    #[qapi(name = "speed")]
    pub speed: i64,
}
/// Stop an active background block operation.
///
/// This command returns immediately after marking the active background
/// block operation for cancellation.  It is an error to call this
/// command if no operation is in progress.
///
/// The operation will cancel as soon as possible and then emit the
/// BLOCK_JOB_CANCELLED event.  Before that happens the job is still
/// visible when enumerated using query-block-jobs.
///
/// Note that if you issue 'block-job-cancel' after 'drive-mirror' has
/// indicated (via the event BLOCK_JOB_READY) that the source and
/// destination are synchronized, then the event triggered by this
/// command changes to BLOCK_JOB_COMPLETED, to indicate that the
/// mirroring has ended and the destination now has a point-in-time copy
/// tied to the time of the cancellation.
///
/// For streaming, the image file retains its backing file unless the
/// streaming operation happens to complete just as it is being
/// cancelled.  A new streaming operation can be started at a later time
/// to finish copying all data from the backing file.
#[qapi(name = "block-job-cancel")]
#[qapi(since = "1.1")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockJobCancel {
    /// The job identifier.  This used to be a device name (hence
    /// the name of the parameter), but since QEMU 2.7 it can have other
    /// values.
    #[qapi(name = "device")]
    pub device: String,
    /// If true, and the job has already emitted the event
    /// BLOCK_JOB_READY, abandon the job immediately (even if it is
    /// paused) instead of waiting for the destination to complete its
    /// final synchronization (since 1.3)
    #[qapi(name = "force")]
    pub force: Option<bool>,
}
/// Pause an active background block operation.
///
/// This command returns immediately after marking the active background
/// block operation for pausing.  It is an error to call this command if
/// no operation is in progress or if the job is already paused.
///
/// The operation will pause as soon as possible.  No event is emitted
/// when the operation is actually paused.  Cancelling a paused job
/// automatically resumes it.
#[qapi(name = "block-job-pause")]
#[qapi(since = "1.3")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockJobPause {
    /// The job identifier.  This used to be a device name (hence
    /// the name of the parameter), but since QEMU 2.7 it can have other
    /// values.
    #[qapi(name = "device")]
    pub device: String,
}
/// Resume an active background block operation.
///
/// This command returns immediately after resuming a paused background
/// block operation.  It is an error to call this command if no
/// operation is in progress or if the job is not paused.
///
/// This command also clears the error status of the job.
#[qapi(name = "block-job-resume")]
#[qapi(since = "1.3")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockJobResume {
    /// The job identifier.  This used to be a device name (hence
    /// the name of the parameter), but since QEMU 2.7 it can have other
    /// values.
    #[qapi(name = "device")]
    pub device: String,
}
/// Manually trigger completion of an active background block operation.
/// This is supported for drive mirroring, where it also switches the
/// device to write to the target path only.  The ability to complete is
/// signaled with a BLOCK_JOB_READY event.
///
/// This command completes an active background block operation
/// synchronously.  The ordering of this command's return with the
/// BLOCK_JOB_COMPLETED event is not defined.  Note that if an I/O error
/// occurs during the processing of this command: 1) the command itself
/// will fail; 2) the error will be processed according to the
/// rerror/werror arguments that were specified when starting the
/// operation.
///
/// A cancelled or paused job cannot be completed.
#[qapi(name = "block-job-complete")]
#[qapi(since = "1.3")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockJobComplete {
    /// The job identifier.  This used to be a device name (hence
    /// the name of the parameter), but since QEMU 2.7 it can have other
    /// values.
    #[qapi(name = "device")]
    pub device: String,
}
/// For jobs that have already concluded, remove them from the
/// block-job-query list.  This command only needs to be run for jobs
/// which were started with QEMU 2.12+ job lifetime management
/// semantics.
///
/// This command will refuse to operate on any job that has not yet
/// reached its terminal state, JOB_STATUS_CONCLUDED.  For jobs that
/// make use of the BLOCK_JOB_READY event, block-job-cancel or
/// block-job-complete will still need to be used as appropriate.
#[qapi(name = "block-job-dismiss")]
#[qapi(since = "2.12")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockJobDismiss {
    /// The job identifier.
    #[qapi(name = "id")]
    pub id: String,
}
/// Once a job that has manual=true reaches the pending state, it can be
/// instructed to finalize any graph changes and do any necessary
/// cleanup via this command.  For jobs in a transaction, instructing
/// one job to finalize will force ALL jobs in the transaction to
/// finalize, so it is only necessary to instruct a single member job to
/// finalize.
#[qapi(name = "block-job-finalize")]
#[qapi(since = "2.12")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockJobFinalize {
    /// The job identifier.
    #[qapi(name = "id")]
    pub id: String,
}
#[qapi(name = "BlockJobChangeOptionsMirror")]
#[qapi(since = "8.2")]
pub struct BlockJobChangeOptionsMirror {
    /// Switch to this copy mode.  Currently, only the switch
    /// from 'background' to 'write-blocking' is implemented.
    #[qapi(name = "copy-mode")]
    pub copy_mode: MirrorCopyMode,
}
pub enum BlockJobChangeOptionsBranch {
    #[qapi(name = "mirror")]
    Mirror(BlockJobChangeOptionsMirror),
}
/// Block job options that can be changed after job creation.
#[qapi(name = "BlockJobChangeOptions")]
#[qapi(since = "8.2")]
pub struct BlockJobChangeOptions {
    /// The job identifier
    #[qapi(name = "id")]
    pub id: String,
    /// The job type
    #[qapi(name = "type")]
    #[qapi(discriminator)]
    pub r#type: JobType,
    #[qapi(union)]
    pub u: Option<BlockJobChangeOptionsBranch>,
}
/// Change the block job's options.
#[qapi(name = "block-job-change")]
#[qapi(since = "8.2")]
#[qapi(returns = "()")]
pub struct BlockJobChange {
    #[qapi(flatten)]
    pub data: BlockJobChangeOptions,
}
/// Determines how to handle discard requests.
#[qapi(name = "BlockdevDiscardOptions")]
#[qapi(since = "2.9")]
pub enum BlockdevDiscardOptions {
    /// Ignore the request
    #[qapi(name = "ignore")]
    Ignore,
    /// Forward as an unmap request
    #[qapi(name = "unmap")]
    Unmap,
}
/// Describes the operation mode for the automatic conversion of plain
/// zero writes by the OS to driver specific optimized zero write
/// commands.
#[qapi(name = "BlockdevDetectZeroesOptions")]
#[qapi(since = "2.1")]
pub enum BlockdevDetectZeroesOptions {
    /// Disabled (default)
    #[qapi(name = "off")]
    Off,
    /// Enabled
    #[qapi(name = "on")]
    On,
    /// Enabled and even try to unmap blocks if possible.  This
    /// requires also that @BlockdevDiscardOptions is set to unmap for
    /// this device.
    #[qapi(name = "unmap")]
    Unmap,
}
/// Selects the AIO backend to handle I/O requests
#[qapi(name = "BlockdevAioOptions")]
#[qapi(since = "2.9")]
pub enum BlockdevAioOptions {
    /// Use qemu's thread pool
    #[qapi(name = "threads")]
    Threads,
    /// Use native AIO backend (only Linux and Windows)
    #[qapi(name = "native")]
    Native,
    /// Use linux io_uring (since 5.0)
    #[qapi(name = "io_uring")]
    #[qapi(condition = "CONFIG_LINUX_IO_URING")]
    IoUring,
}
/// Includes cache-related options for block devices
#[qapi(name = "BlockdevCacheOptions")]
#[qapi(since = "2.9")]
pub struct BlockdevCacheOptions {
    /// enables use of O_DIRECT (bypass the host page cache;
    /// default: false)
    #[qapi(name = "direct")]
    pub direct: Option<bool>,
    /// ignore any flush requests for the device (default: false)
    #[qapi(name = "no-flush")]
    pub no_flush: Option<bool>,
}
/// Drivers that are supported in block device operations.
#[qapi(name = "BlockdevDriver")]
#[qapi(since = "2.9")]
pub enum BlockdevDriver {
    #[qapi(name = "blkdebug")]
    Blkdebug,
    /// Since 3.0
    #[qapi(name = "blklogwrites")]
    Blklogwrites,
    /// Since 4.2
    #[qapi(name = "blkreplay")]
    Blkreplay,
    #[qapi(name = "blkverify")]
    Blkverify,
    #[qapi(name = "bochs")]
    Bochs,
    #[qapi(name = "cloop")]
    Cloop,
    /// Since 5.0
    #[qapi(name = "compress")]
    Compress,
    /// Since 6.2
    #[qapi(name = "copy-before-write")]
    CopyBeforeWrite,
    /// Since 3.0
    #[qapi(name = "copy-on-read")]
    CopyOnRead,
    #[qapi(name = "dmg")]
    Dmg,
    #[qapi(name = "file")]
    File,
    /// Since 7.0
    #[qapi(name = "snapshot-access")]
    SnapshotAccess,
    #[qapi(name = "ftp")]
    Ftp,
    #[qapi(name = "ftps")]
    Ftps,
    #[qapi(name = "gluster")]
    Gluster,
    #[qapi(name = "host_cdrom")]
    #[qapi(condition = "HAVE_HOST_BLOCK_DEVICE")]
    HostCdrom,
    #[qapi(name = "host_device")]
    #[qapi(condition = "HAVE_HOST_BLOCK_DEVICE")]
    HostDevice,
    #[qapi(name = "http")]
    Http,
    #[qapi(name = "https")]
    Https,
    #[qapi(name = "io_uring")]
    #[qapi(condition = "CONFIG_BLKIO")]
    IoUring,
    #[qapi(name = "iscsi")]
    Iscsi,
    #[qapi(name = "luks")]
    Luks,
    #[qapi(name = "nbd")]
    Nbd,
    #[qapi(name = "nfs")]
    Nfs,
    #[qapi(name = "null-aio")]
    NullAio,
    #[qapi(name = "null-co")]
    NullCo,
    /// Since 2.12
    #[qapi(name = "nvme")]
    Nvme,
    #[qapi(name = "nvme-io_uring")]
    #[qapi(condition = "CONFIG_BLKIO")]
    NvmeIoUring,
    #[qapi(name = "parallels")]
    Parallels,
    #[qapi(name = "preallocate")]
    Preallocate,
    #[qapi(name = "qcow")]
    Qcow,
    #[qapi(name = "qcow2")]
    Qcow2,
    #[qapi(name = "qed")]
    Qed,
    #[qapi(name = "quorum")]
    Quorum,
    #[qapi(name = "raw")]
    Raw,
    #[qapi(name = "rbd")]
    Rbd,
    #[qapi(name = "replication")]
    #[qapi(condition = "CONFIG_REPLICATION")]
    Replication,
    #[qapi(name = "ssh")]
    Ssh,
    /// Since 2.11
    #[qapi(name = "throttle")]
    Throttle,
    #[qapi(name = "vdi")]
    Vdi,
    #[qapi(name = "vhdx")]
    Vhdx,
    #[qapi(name = "virtio-blk-vfio-pci")]
    #[qapi(condition = "CONFIG_BLKIO")]
    VirtioBlkVfioPci,
    #[qapi(name = "virtio-blk-vhost-user")]
    #[qapi(condition = "CONFIG_BLKIO")]
    VirtioBlkVhostUser,
    #[qapi(name = "virtio-blk-vhost-vdpa")]
    #[qapi(condition = "CONFIG_BLKIO")]
    VirtioBlkVhostVdpa,
    #[qapi(name = "vmdk")]
    Vmdk,
    #[qapi(name = "vpc")]
    Vpc,
    #[qapi(name = "vvfat")]
    Vvfat,
}
/// Driver specific block device options for the file backend.
#[qapi(name = "BlockdevOptionsFile")]
#[qapi(feature = "dynamic-auto-read-only")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsFile {
    /// path to the image file
    #[qapi(name = "filename")]
    pub filename: String,
    /// the id for the object that will handle persistent
    /// reservations for this device (default: none, forward the
    /// commands via SG_IO; since 2.11)
    #[qapi(name = "pr-manager")]
    pub pr_manager: Option<String>,
    /// whether to enable file locking.  If set to 'auto', only
    /// enable when Open File Descriptor (OFD) locking API is available
    /// (default: auto, since 2.10)
    #[qapi(name = "locking")]
    pub locking: Option<OnOffAuto>,
    /// AIO backend (default: threads) (since: 2.8)
    #[qapi(name = "aio")]
    pub aio: Option<BlockdevAioOptions>,
    /// maximum number of requests to batch together into a
    /// single submission in the AIO backend.  The smallest value
    /// between this and the aio-max-batch value of the IOThread object
    /// is chosen.  0 means that the AIO backend will handle it
    /// automatically.  (default: 0, since 6.2)
    #[qapi(name = "aio-max-batch")]
    pub aio_max_batch: Option<i64>,
    /// invalidate page cache during live migration.  This
    /// prevents stale data on the migration destination with
    /// cache.direct=off.  Currently only supported on Linux hosts.
    /// (default: on, since: 4.0)
    #[qapi(name = "drop-cache")]
    #[qapi(condition = "CONFIG_LINUX")]
    pub drop_cache: Option<bool>,
    /// whether to check that page cache was dropped
    /// on live migration.  May cause noticeable delays if the image
    /// file is large, do not use in production.  (default: off)
    /// (since: 3.0)
    #[qapi(name = "x-check-cache-dropped")]
    #[qapi(feature = "unstable")]
    pub x_check_cache_dropped: Option<bool>,
}
/// Driver specific block device options for the null backend.
#[qapi(name = "BlockdevOptionsNull")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsNull {
    /// size of the device in bytes.
    #[qapi(name = "size")]
    pub size: Option<i64>,
    /// emulated latency (in nanoseconds) in processing
    /// requests.  Default to zero which completes requests immediately.
    /// (Since 2.4)
    #[qapi(name = "latency-ns")]
    pub latency_ns: Option<u64>,
    /// if true, reads from the device produce zeroes; if
    /// false, the buffer is left unchanged.
    /// (default: false; since: 4.1)
    #[qapi(name = "read-zeroes")]
    pub read_zeroes: Option<bool>,
}
/// Driver specific block device options for the NVMe backend.
#[qapi(name = "BlockdevOptionsNVMe")]
#[qapi(since = "2.12")]
pub struct BlockdevOptionsNvMe {
    /// PCI controller address of the NVMe device in format
    /// hhhh:bb:ss.f (host:bus:slot.function)
    #[qapi(name = "device")]
    pub device: String,
    /// namespace number of the device, starting from 1.
    ///
    /// Note that the PCI @device must have been unbound from any host
    /// kernel driver before instructing QEMU to add the blockdev.
    #[qapi(name = "namespace")]
    pub namespace: i64,
}
/// Driver specific block device options for the vvfat protocol.
#[qapi(name = "BlockdevOptionsVVFAT")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsVvfat {
    /// directory to be exported as FAT image
    #[qapi(name = "dir")]
    pub dir: String,
    /// FAT type: 12, 16 or 32
    #[qapi(name = "fat-type")]
    pub fat_type: Option<i64>,
    /// whether to export a floppy image (true) or partitioned hard
    /// disk (false; default)
    #[qapi(name = "floppy")]
    pub floppy: Option<bool>,
    /// set the volume label, limited to 11 bytes.  FAT16 and FAT32
    /// traditionally have some restrictions on labels, which are
    /// ignored by most operating systems.  Defaults to "QEMU VVFAT".
    /// (since 2.4)
    #[qapi(name = "label")]
    pub label: Option<String>,
    /// whether to allow write operations (default: false)
    #[qapi(name = "rw")]
    pub rw: Option<bool>,
}
/// Driver specific block device options for image format that have no
/// option besides their data source.
#[qapi(name = "BlockdevOptionsGenericFormat")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsGenericFormat {
    /// reference to or definition of the data source block device
    #[qapi(name = "file")]
    pub file: BlockdevRef,
}
/// Driver specific block device options for LUKS.
#[qapi(name = "BlockdevOptionsLUKS")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsLuks {
    /// reference to or definition of the data source block device
    #[qapi(name = "file")]
    pub file: BlockdevRef,
    /// the ID of a QCryptoSecret object providing the
    /// decryption key (since 2.6).  Mandatory except when doing a
    /// metadata-only probe of the image.
    #[qapi(name = "key-secret")]
    pub key_secret: Option<String>,
    /// block device holding a detached LUKS header.  (since 9.0)
    #[qapi(name = "header")]
    pub header: Option<BlockdevRef>,
}
/// Driver specific block device options for image format that have no
/// option besides their data source and an optional backing file.
#[qapi(name = "BlockdevOptionsGenericCOWFormat")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsGenericCowFormat {
    /// reference to or definition of the data source block device
    #[qapi(name = "file")]
    pub file: BlockdevRef,
    /// reference to or definition of the backing file block
    /// device, null disables the backing file entirely.  Defaults to
    /// the backing file stored the image file.
    #[qapi(name = "backing")]
    pub backing: Option<BlockdevRefOrNull>,
}
/// General overlap check modes.
#[qapi(name = "Qcow2OverlapCheckMode")]
#[qapi(since = "2.9")]
pub enum Qcow2OverlapCheckMode {
    /// Do not perform any checks
    #[qapi(name = "none")]
    None,
    /// Perform only checks which can be done in constant time
    /// and without reading anything from disk
    #[qapi(name = "constant")]
    Constant,
    /// Perform only checks which can be done without reading
    /// anything from disk
    #[qapi(name = "cached")]
    Cached,
    /// Perform all available overlap checks
    #[qapi(name = "all")]
    All,
}
/// Structure of flags for each metadata structure.  Setting a field to
/// 'true' makes QEMU guard that Qcow2 format structure against
/// unintended overwriting.  See Qcow2 format specification for detailed
/// information on these structures.  The default value is chosen
/// according to the template given.
#[qapi(name = "Qcow2OverlapCheckFlags")]
#[qapi(since = "2.9")]
pub struct Qcow2OverlapCheckFlags {
    /// Specifies a template mode which can be adjusted using the
    /// other flags, defaults to 'cached'
    #[qapi(name = "template")]
    pub template: Option<Qcow2OverlapCheckMode>,
    /// Qcow2 format header
    #[qapi(name = "main-header")]
    pub main_header: Option<bool>,
    /// Qcow2 active L1 table
    #[qapi(name = "active-l1")]
    pub active_l1: Option<bool>,
    /// Qcow2 active L2 table
    #[qapi(name = "active-l2")]
    pub active_l2: Option<bool>,
    /// Qcow2 refcount table
    #[qapi(name = "refcount-table")]
    pub refcount_table: Option<bool>,
    /// Qcow2 refcount blocks
    #[qapi(name = "refcount-block")]
    pub refcount_block: Option<bool>,
    /// Qcow2 snapshot table
    #[qapi(name = "snapshot-table")]
    pub snapshot_table: Option<bool>,
    /// Qcow2 inactive L1 tables
    #[qapi(name = "inactive-l1")]
    pub inactive_l1: Option<bool>,
    /// Qcow2 inactive L2 tables
    #[qapi(name = "inactive-l2")]
    pub inactive_l2: Option<bool>,
    /// Qcow2 bitmap directory (since 3.0)
    #[qapi(name = "bitmap-directory")]
    pub bitmap_directory: Option<bool>,
}
/// Specifies which metadata structures should be guarded against
/// unintended overwriting.
#[qapi(name = "Qcow2OverlapChecks")]
#[qapi(since = "2.9")]
pub enum Qcow2OverlapChecks {
    /// set of flags for separate specification of each metadata
    /// structure type
    #[qapi(name = "flags")]
    Flags(Qcow2OverlapCheckFlags),
    /// named mode which chooses a specific set of flags
    #[qapi(name = "mode")]
    Mode(Qcow2OverlapCheckMode),
}
#[qapi(name = "BlockdevQcowEncryptionFormat")]
#[qapi(since = "2.10")]
pub enum BlockdevQcowEncryptionFormat {
    /// AES-CBC with plain64 initialization vectors
    #[qapi(name = "aes")]
    Aes,
}
pub enum BlockdevQcowEncryptionBranch {
    #[qapi(name = "aes")]
    Aes(QCryptoBlockOptionsQCow),
}
#[qapi(name = "BlockdevQcowEncryption")]
#[qapi(since = "2.10")]
pub struct BlockdevQcowEncryption {
    /// encryption format
    #[qapi(name = "format")]
    #[qapi(discriminator)]
    pub format: BlockdevQcowEncryptionFormat,
    #[qapi(union)]
    pub u: Option<BlockdevQcowEncryptionBranch>,
}
/// Driver specific block device options for qcow.
#[qapi(name = "BlockdevOptionsQcow")]
#[qapi(since = "2.10")]
pub struct BlockdevOptionsQcow {
    /// reference to or definition of the data source block device
    #[qapi(name = "file")]
    pub file: BlockdevRef,
    /// reference to or definition of the backing file block
    /// device, null disables the backing file entirely.  Defaults to
    /// the backing file stored the image file.
    #[qapi(name = "backing")]
    pub backing: Option<BlockdevRefOrNull>,
    /// Image decryption options.  Mandatory for encrypted images,
    /// except when doing a metadata-only probe of the image.
    #[qapi(name = "encrypt")]
    pub encrypt: Option<BlockdevQcowEncryption>,
}
#[qapi(name = "BlockdevQcow2EncryptionFormat")]
#[qapi(since = "2.10")]
pub enum BlockdevQcow2EncryptionFormat {
    /// AES-CBC with plain64 initialization vectors
    #[qapi(name = "aes")]
    Aes,
    #[qapi(name = "luks")]
    Luks,
}
pub enum BlockdevQcow2EncryptionBranch {
    #[qapi(name = "aes")]
    Aes(QCryptoBlockOptionsQCow),
    #[qapi(name = "luks")]
    Luks(QCryptoBlockOptionsLuks),
}
#[qapi(name = "BlockdevQcow2Encryption")]
#[qapi(since = "2.10")]
pub struct BlockdevQcow2Encryption {
    /// encryption format
    #[qapi(name = "format")]
    #[qapi(discriminator)]
    pub format: BlockdevQcow2EncryptionFormat,
    #[qapi(union)]
    pub u: Option<BlockdevQcow2EncryptionBranch>,
}
/// Filter driver intended to be inserted between format and protocol
/// node and do preallocation in protocol node on write.
#[qapi(name = "BlockdevOptionsPreallocate")]
#[qapi(since = "6.0")]
pub struct BlockdevOptionsPreallocate {
    /// reference to or definition of the data source block device
    #[qapi(name = "file")]
    pub file: BlockdevRef,
    /// on preallocation, align file length to this number,
    /// default 1048576 (1M)
    #[qapi(name = "prealloc-align")]
    pub prealloc_align: Option<i64>,
    /// how much to preallocate, default 134217728 (128M)
    #[qapi(name = "prealloc-size")]
    pub prealloc_size: Option<i64>,
}
/// Driver specific block device options for qcow2.
#[qapi(name = "BlockdevOptionsQcow2")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsQcow2 {
    /// reference to or definition of the data source block device
    #[qapi(name = "file")]
    pub file: BlockdevRef,
    /// reference to or definition of the backing file block
    /// device, null disables the backing file entirely.  Defaults to
    /// the backing file stored the image file.
    #[qapi(name = "backing")]
    pub backing: Option<BlockdevRefOrNull>,
    /// whether to enable the lazy refcounts feature
    /// (default is taken from the image file)
    #[qapi(name = "lazy-refcounts")]
    pub lazy_refcounts: Option<bool>,
    /// whether discard requests to the qcow2 device
    /// should be forwarded to the data source
    #[qapi(name = "pass-discard-request")]
    pub pass_discard_request: Option<bool>,
    /// whether discard requests for the data source
    /// should be issued when a snapshot operation (e.g. deleting a
    /// snapshot) frees clusters in the qcow2 file
    #[qapi(name = "pass-discard-snapshot")]
    pub pass_discard_snapshot: Option<bool>,
    /// whether discard requests for the data source
    /// should be issued on other occasions where a cluster gets freed
    #[qapi(name = "pass-discard-other")]
    pub pass_discard_other: Option<bool>,
    /// when enabled, data clusters will remain
    /// preallocated when they are no longer used, e.g. because they are
    /// discarded or converted to zero clusters.  As usual, whether the
    /// old data is discarded or kept on the protocol level (i.e. in the
    /// image file) depends on the setting of the pass-discard-request
    /// option.  Keeping the clusters preallocated prevents qcow2
    /// fragmentation that would otherwise be caused by freeing and
    /// re-allocating them later.  Besides potential performance
    /// degradation, such fragmentation can lead to increased allocation
    /// of clusters past the end of the image file, resulting in image
    /// files whose file length can grow much larger than their guest
    /// disk size would suggest.  If image file length is of concern
    /// (e.g. when storing qcow2 images directly on block devices), you
    /// should consider enabling this option.  (since 8.1)
    #[qapi(name = "discard-no-unref")]
    pub discard_no_unref: Option<bool>,
    /// which overlap checks to perform for writes to the
    /// image, defaults to 'cached' (since 2.2)
    #[qapi(name = "overlap-check")]
    pub overlap_check: Option<Qcow2OverlapChecks>,
    /// the maximum total size of the L2 table and refcount
    /// block caches in bytes (since 2.2)
    #[qapi(name = "cache-size")]
    pub cache_size: Option<i64>,
    /// the maximum size of the L2 table cache in bytes
    /// (since 2.2)
    #[qapi(name = "l2-cache-size")]
    pub l2_cache_size: Option<i64>,
    /// the size of each entry in the L2 cache in
    /// bytes.  It must be a power of two between 512 and the cluster
    /// size.  The default value is the cluster size (since 2.12)
    #[qapi(name = "l2-cache-entry-size")]
    pub l2_cache_entry_size: Option<i64>,
    /// the maximum size of the refcount block cache
    /// in bytes (since 2.2)
    #[qapi(name = "refcount-cache-size")]
    pub refcount_cache_size: Option<i64>,
    /// clean unused entries in the L2 and refcount
    /// caches.  The interval is in seconds.  The default value is 600
    /// on supporting platforms, and 0 on other platforms.  0 disables
    /// this feature.  (since 2.5)
    #[qapi(name = "cache-clean-interval")]
    pub cache_clean_interval: Option<i64>,
    /// Image decryption options.  Mandatory for encrypted images,
    /// except when doing a metadata-only probe of the image.  (since
    /// 2.10)
    #[qapi(name = "encrypt")]
    pub encrypt: Option<BlockdevQcow2Encryption>,
    /// reference to or definition of the external data file.
    /// This may only be specified for images that require an external
    /// data file.  If it is not specified for such an image, the data
    /// file name is loaded from the image file.  (since 4.0)
    #[qapi(name = "data-file")]
    pub data_file: Option<BlockdevRef>,
}
#[qapi(name = "SshHostKeyCheckMode")]
#[qapi(since = "2.12")]
pub enum SshHostKeyCheckMode {
    /// Don't check the host key at all
    #[qapi(name = "none")]
    None,
    /// Compare the host key with a given hash
    #[qapi(name = "hash")]
    Hash,
    /// Check the host key against the known_hosts file
    #[qapi(name = "known_hosts")]
    KnownHosts,
}
#[qapi(name = "SshHostKeyCheckHashType")]
#[qapi(since = "2.12")]
pub enum SshHostKeyCheckHashType {
    /// The given hash is an md5 hash
    #[qapi(name = "md5")]
    Md5,
    /// The given hash is an sha1 hash
    #[qapi(name = "sha1")]
    Sha1,
    /// The given hash is an sha256 hash
    #[qapi(name = "sha256")]
    Sha256,
}
#[qapi(name = "SshHostKeyHash")]
#[qapi(since = "2.12")]
pub struct SshHostKeyHash {
    /// The hash algorithm used for the hash
    #[qapi(name = "type")]
    pub r#type: SshHostKeyCheckHashType,
    /// The expected hash value
    #[qapi(name = "hash")]
    pub hash: String,
}
pub enum SshHostKeyCheckBranch {
    #[qapi(name = "hash")]
    Hash(SshHostKeyHash),
}
#[qapi(name = "SshHostKeyCheck")]
#[qapi(since = "2.12")]
pub struct SshHostKeyCheck {
    /// How to check the host key
    #[qapi(name = "mode")]
    #[qapi(discriminator)]
    pub mode: SshHostKeyCheckMode,
    #[qapi(union)]
    pub u: Option<SshHostKeyCheckBranch>,
}
#[qapi(name = "BlockdevOptionsSsh")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsSsh {
    /// host address
    #[qapi(name = "server")]
    pub server: InetSocketAddress,
    /// path to the image on the host
    #[qapi(name = "path")]
    pub path: String,
    /// user as which to connect, defaults to current local user name
    #[qapi(name = "user")]
    pub user: Option<String>,
    /// Defines how and what to check the host key against
    /// (default: known_hosts)
    #[qapi(name = "host-key-check")]
    pub host_key_check: Option<SshHostKeyCheck>,
}
/// Trigger events supported by blkdebug.
#[qapi(name = "BlkdebugEvent")]
#[qapi(since = "2.9")]
pub enum BlkdebugEvent {
    #[qapi(name = "l1_update")]
    L1Update,
    #[qapi(name = "l1_grow_alloc_table")]
    L1GrowAllocTable,
    #[qapi(name = "l1_grow_write_table")]
    L1GrowWriteTable,
    #[qapi(name = "l1_grow_activate_table")]
    L1GrowActivateTable,
    #[qapi(name = "l2_load")]
    L2Load,
    #[qapi(name = "l2_update")]
    L2Update,
    #[qapi(name = "l2_update_compressed")]
    L2UpdateCompressed,
    #[qapi(name = "l2_alloc_cow_read")]
    L2AllocCowRead,
    #[qapi(name = "l2_alloc_write")]
    L2AllocWrite,
    #[qapi(name = "read_aio")]
    ReadAio,
    #[qapi(name = "read_backing_aio")]
    ReadBackingAio,
    #[qapi(name = "read_compressed")]
    ReadCompressed,
    #[qapi(name = "write_aio")]
    WriteAio,
    #[qapi(name = "write_compressed")]
    WriteCompressed,
    #[qapi(name = "vmstate_load")]
    VmstateLoad,
    #[qapi(name = "vmstate_save")]
    VmstateSave,
    #[qapi(name = "cow_read")]
    CowRead,
    #[qapi(name = "cow_write")]
    CowWrite,
    #[qapi(name = "reftable_load")]
    ReftableLoad,
    #[qapi(name = "reftable_grow")]
    ReftableGrow,
    #[qapi(name = "reftable_update")]
    ReftableUpdate,
    #[qapi(name = "refblock_load")]
    RefblockLoad,
    #[qapi(name = "refblock_update")]
    RefblockUpdate,
    #[qapi(name = "refblock_update_part")]
    RefblockUpdatePart,
    #[qapi(name = "refblock_alloc")]
    RefblockAlloc,
    #[qapi(name = "refblock_alloc_hookup")]
    RefblockAllocHookup,
    #[qapi(name = "refblock_alloc_write")]
    RefblockAllocWrite,
    #[qapi(name = "refblock_alloc_write_blocks")]
    RefblockAllocWriteBlocks,
    #[qapi(name = "refblock_alloc_write_table")]
    RefblockAllocWriteTable,
    #[qapi(name = "refblock_alloc_switch_table")]
    RefblockAllocSwitchTable,
    #[qapi(name = "cluster_alloc")]
    ClusterAlloc,
    #[qapi(name = "cluster_alloc_bytes")]
    ClusterAllocBytes,
    #[qapi(name = "cluster_free")]
    ClusterFree,
    #[qapi(name = "flush_to_os")]
    FlushToOs,
    #[qapi(name = "flush_to_disk")]
    FlushToDisk,
    #[qapi(name = "pwritev_rmw_head")]
    PwritevRmwHead,
    #[qapi(name = "pwritev_rmw_after_head")]
    PwritevRmwAfterHead,
    #[qapi(name = "pwritev_rmw_tail")]
    PwritevRmwTail,
    #[qapi(name = "pwritev_rmw_after_tail")]
    PwritevRmwAfterTail,
    #[qapi(name = "pwritev")]
    Pwritev,
    #[qapi(name = "pwritev_zero")]
    PwritevZero,
    #[qapi(name = "pwritev_done")]
    PwritevDone,
    #[qapi(name = "empty_image_prepare")]
    EmptyImagePrepare,
    /// write zeros to the l1 table to shrink image.
    /// (since 2.11)
    #[qapi(name = "l1_shrink_write_table")]
    L1ShrinkWriteTable,
    /// discard the l2 tables.  (since 2.11)
    #[qapi(name = "l1_shrink_free_l2_clusters")]
    L1ShrinkFreeL2Clusters,
    /// a write due to copy-on-read (since 2.11)
    #[qapi(name = "cor_write")]
    CorWrite,
    /// an allocation of file space for a cluster
    /// (since 4.1)
    #[qapi(name = "cluster_alloc_space")]
    ClusterAllocSpace,
    /// triggers once at creation of the blkdebug node (since 4.1)
    #[qapi(name = "none")]
    None,
}
/// Kinds of I/O that blkdebug can inject errors in.
#[qapi(name = "BlkdebugIOType")]
#[qapi(since = "4.1")]
pub enum BlkdebugIoType {
    /// .bdrv_co_preadv()
    #[qapi(name = "read")]
    Read,
    /// .bdrv_co_pwritev()
    #[qapi(name = "write")]
    Write,
    /// .bdrv_co_pwrite_zeroes()
    #[qapi(name = "write-zeroes")]
    WriteZeroes,
    /// .bdrv_co_pdiscard()
    #[qapi(name = "discard")]
    Discard,
    /// .bdrv_co_flush_to_disk()
    #[qapi(name = "flush")]
    Flush,
    /// .bdrv_co_block_status()
    #[qapi(name = "block-status")]
    BlockStatus,
}
/// Describes a single error injection for blkdebug.
#[qapi(name = "BlkdebugInjectErrorOptions")]
#[qapi(since = "2.9")]
pub struct BlkdebugInjectErrorOptions {
    /// trigger event
    #[qapi(name = "event")]
    pub event: BlkdebugEvent,
    /// the state identifier blkdebug needs to be in to actually
    /// trigger the event; defaults to "any"
    #[qapi(name = "state")]
    pub state: Option<i64>,
    /// the type of I/O operations on which this error should be
    /// injected; defaults to "all read, write, write-zeroes, discard,
    /// and flush operations" (since: 4.1)
    #[qapi(name = "iotype")]
    pub iotype: Option<BlkdebugIoType>,
    /// error identifier (errno) to be returned; defaults to EIO
    #[qapi(name = "errno")]
    pub errno: Option<i64>,
    /// specifies the sector index which has to be affected in
    /// order to actually trigger the event; defaults to "any sector"
    #[qapi(name = "sector")]
    pub sector: Option<i64>,
    /// disables further events after this one has been triggered;
    /// defaults to false
    #[qapi(name = "once")]
    pub once: Option<bool>,
    /// fail immediately; defaults to false
    #[qapi(name = "immediately")]
    pub immediately: Option<bool>,
}
/// Describes a single state-change event for blkdebug.
#[qapi(name = "BlkdebugSetStateOptions")]
#[qapi(since = "2.9")]
pub struct BlkdebugSetStateOptions {
    /// trigger event
    #[qapi(name = "event")]
    pub event: BlkdebugEvent,
    /// the current state identifier blkdebug needs to be in;
    /// defaults to "any"
    #[qapi(name = "state")]
    pub state: Option<i64>,
    /// the state identifier blkdebug is supposed to assume if
    /// this event is triggered
    #[qapi(name = "new_state")]
    pub new_state: i64,
}
/// Driver specific block device options for blkdebug.
#[qapi(name = "BlockdevOptionsBlkdebug")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsBlkdebug {
    /// underlying raw block device (or image file)
    #[qapi(name = "image")]
    pub image: BlockdevRef,
    /// filename of the configuration file
    #[qapi(name = "config")]
    pub config: Option<String>,
    /// required alignment for requests in bytes, must be positive
    /// power of 2, or 0 for default
    #[qapi(name = "align")]
    pub align: Option<i64>,
    /// maximum size for I/O transfers in bytes, must be
    /// positive multiple of @align and of the underlying file's request
    /// alignment (but need not be a power of 2), or 0 for default
    /// (since 2.10)
    #[qapi(name = "max-transfer")]
    pub max_transfer: Option<i32>,
    /// preferred alignment for write zero requests in
    /// bytes, must be positive multiple of @align and of the underlying
    /// file's request alignment (but need not be a power of 2), or 0
    /// for default (since 2.10)
    #[qapi(name = "opt-write-zero")]
    pub opt_write_zero: Option<i32>,
    /// maximum size for write zero requests in bytes, must
    /// be positive multiple of @align, of @opt-write-zero, and of the
    /// underlying file's request alignment (but need not be a power of
    /// 2), or 0 for default (since 2.10)
    #[qapi(name = "max-write-zero")]
    pub max_write_zero: Option<i32>,
    /// preferred alignment for discard requests in bytes,
    /// must be positive multiple of @align and of the underlying file's
    /// request alignment (but need not be a power of 2), or 0 for
    /// default (since 2.10)
    #[qapi(name = "opt-discard")]
    pub opt_discard: Option<i32>,
    /// maximum size for discard requests in bytes, must be
    /// positive multiple of @align, of @opt-discard, and of the
    /// underlying file's request alignment (but need not be a power of
    /// 2), or 0 for default (since 2.10)
    #[qapi(name = "max-discard")]
    pub max_discard: Option<i32>,
    /// array of error injection descriptions
    #[qapi(name = "inject-error")]
    pub inject_error: Option<Vec<BlkdebugInjectErrorOptions>>,
    /// array of state-change descriptions
    #[qapi(name = "set-state")]
    pub set_state: Option<Vec<BlkdebugSetStateOptions>>,
    /// Permissions to take on @image in addition to what
    /// is necessary anyway (which depends on how the blkdebug node is
    /// used).  Defaults to none.  (since 5.0)
    #[qapi(name = "take-child-perms")]
    pub take_child_perms: Option<Vec<BlockPermission>>,
    /// Permissions not to share on @image in addition
    /// to what cannot be shared anyway (which depends on how the
    /// blkdebug node is used).  Defaults to none.  (since 5.0)
    #[qapi(name = "unshare-child-perms")]
    pub unshare_child_perms: Option<Vec<BlockPermission>>,
}
/// Driver specific block device options for blklogwrites.
#[qapi(name = "BlockdevOptionsBlklogwrites")]
#[qapi(since = "3.0")]
pub struct BlockdevOptionsBlklogwrites {
    /// block device
    #[qapi(name = "file")]
    pub file: BlockdevRef,
    /// block device used to log writes to @file
    #[qapi(name = "log")]
    pub log: BlockdevRef,
    /// sector size used in logging writes to @file,
    /// determines granularity of offsets and sizes of writes
    /// (default: 512)
    #[qapi(name = "log-sector-size")]
    pub log_sector_size: Option<u32>,
    /// append to an existing log (default: false)
    #[qapi(name = "log-append")]
    pub log_append: Option<bool>,
    /// interval of write requests after which
    /// the log super block is updated to disk (default: 4096)
    #[qapi(name = "log-super-update-interval")]
    pub log_super_update_interval: Option<u64>,
}
/// Driver specific block device options for blkverify.
#[qapi(name = "BlockdevOptionsBlkverify")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsBlkverify {
    /// block device to be tested
    #[qapi(name = "test")]
    pub test: BlockdevRef,
    /// raw image used for verification
    #[qapi(name = "raw")]
    pub raw: BlockdevRef,
}
/// Driver specific block device options for blkreplay.
#[qapi(name = "BlockdevOptionsBlkreplay")]
#[qapi(since = "4.2")]
pub struct BlockdevOptionsBlkreplay {
    /// disk image which should be controlled with blkreplay
    #[qapi(name = "image")]
    pub image: BlockdevRef,
}
/// An enumeration of quorum read patterns.
#[qapi(name = "QuorumReadPattern")]
#[qapi(since = "2.9")]
pub enum QuorumReadPattern {
    /// read all the children and do a quorum vote on reads
    #[qapi(name = "quorum")]
    Quorum,
    /// read only from the first child that has not failed
    #[qapi(name = "fifo")]
    Fifo,
}
/// Driver specific block device options for Quorum
#[qapi(name = "BlockdevOptionsQuorum")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsQuorum {
    /// true if the driver must print content mismatch set to
    /// false by default
    #[qapi(name = "blkverify")]
    pub blkverify: Option<bool>,
    /// the children block devices to use
    #[qapi(name = "children")]
    pub children: Vec<BlockdevRef>,
    /// the vote limit under which a read will fail
    #[qapi(name = "vote-threshold")]
    pub vote_threshold: i64,
    /// rewrite corrupted data when quorum is reached
    /// (Since 2.1)
    #[qapi(name = "rewrite-corrupted")]
    pub rewrite_corrupted: Option<bool>,
    /// choose read pattern and set to quorum by default
    /// (Since 2.2)
    #[qapi(name = "read-pattern")]
    pub read_pattern: Option<QuorumReadPattern>,
}
/// Driver specific block device options for Gluster
#[qapi(name = "BlockdevOptionsGluster")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsGluster {
    /// name of gluster volume where VM image resides
    #[qapi(name = "volume")]
    pub volume: String,
    /// absolute path to image file in gluster volume
    #[qapi(name = "path")]
    pub path: String,
    /// gluster servers description
    #[qapi(name = "server")]
    pub server: Vec<SocketAddress>,
    /// libgfapi log level (default '4' which is Error) (Since 2.8)
    #[qapi(name = "debug")]
    pub debug: Option<i64>,
    /// libgfapi log file (default /dev/stderr) (Since 2.8)
    #[qapi(name = "logfile")]
    pub logfile: Option<String>,
}
/// Driver specific block device options for the io_uring backend.
#[qapi(name = "BlockdevOptionsIoUring")]
#[qapi(condition = "CONFIG_BLKIO")]
#[qapi(since = "7.2")]
pub struct BlockdevOptionsIoUring {
    /// path to the image file
    #[qapi(name = "filename")]
    pub filename: String,
}
/// Driver specific block device options for the nvme-io_uring backend.
#[qapi(name = "BlockdevOptionsNvmeIoUring")]
#[qapi(condition = "CONFIG_BLKIO")]
#[qapi(since = "7.2")]
pub struct BlockdevOptionsNvmeIoUring {
    /// path to the NVMe namespace's character device (e.g.
    /// /dev/ng0n1).
    #[qapi(name = "path")]
    pub path: String,
}
/// Driver specific block device options for the virtio-blk-vfio-pci
/// backend.
#[qapi(name = "BlockdevOptionsVirtioBlkVfioPci")]
#[qapi(condition = "CONFIG_BLKIO")]
#[qapi(since = "7.2")]
pub struct BlockdevOptionsVirtioBlkVfioPci {
    /// path to the PCI device's sysfs directory (e.g.
    /// /sys/bus/pci/devices/0000:00:01.0).
    #[qapi(name = "path")]
    pub path: String,
}
/// Driver specific block device options for the virtio-blk-vhost-user
/// backend.
#[qapi(name = "BlockdevOptionsVirtioBlkVhostUser")]
#[qapi(condition = "CONFIG_BLKIO")]
#[qapi(since = "7.2")]
pub struct BlockdevOptionsVirtioBlkVhostUser {
    /// path to the vhost-user UNIX domain socket.
    #[qapi(name = "path")]
    pub path: String,
}
/// Driver specific block device options for the virtio-blk-vhost-vdpa
/// backend.
#[qapi(name = "BlockdevOptionsVirtioBlkVhostVdpa")]
#[qapi(condition = "CONFIG_BLKIO")]
#[qapi(feature = "fdset")]
#[qapi(since = "7.2")]
pub struct BlockdevOptionsVirtioBlkVhostVdpa {
    /// path to the vhost-vdpa character device.
    #[qapi(name = "path")]
    pub path: String,
}
/// An enumeration of libiscsi transport types
#[qapi(name = "IscsiTransport")]
#[qapi(since = "2.9")]
pub enum IscsiTransport {
    #[qapi(name = "tcp")]
    Tcp,
    #[qapi(name = "iser")]
    Iser,
}
/// An enumeration of header digests supported by libiscsi
#[qapi(name = "IscsiHeaderDigest")]
#[qapi(since = "2.9")]
pub enum IscsiHeaderDigest {
    #[qapi(name = "crc32c")]
    Crc32c,
    #[qapi(name = "none")]
    None,
    #[qapi(name = "crc32c-none")]
    Crc32cNone,
    #[qapi(name = "none-crc32c")]
    NoneCrc32c,
}
/// Driver specific block device options for iscsi
#[qapi(name = "BlockdevOptionsIscsi")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsIscsi {
    /// The iscsi transport type
    #[qapi(name = "transport")]
    pub transport: IscsiTransport,
    /// The address of the iscsi portal
    #[qapi(name = "portal")]
    pub portal: String,
    /// The target iqn name
    #[qapi(name = "target")]
    pub target: String,
    /// LUN to connect to.  Defaults to 0.
    #[qapi(name = "lun")]
    pub lun: Option<i64>,
    /// User name to log in with.  If omitted, no CHAP authentication
    /// is performed.
    #[qapi(name = "user")]
    pub user: Option<String>,
    /// The ID of a QCryptoSecret object providing the
    /// password for the login.  This option is required if @user is
    /// specified.
    #[qapi(name = "password-secret")]
    pub password_secret: Option<String>,
    /// The iqn name we want to identify to the target as.
    /// If this option is not specified, an initiator name is generated
    /// automatically.
    #[qapi(name = "initiator-name")]
    pub initiator_name: Option<String>,
    /// The desired header digest.  Defaults to none-crc32c.
    #[qapi(name = "header-digest")]
    pub header_digest: Option<IscsiHeaderDigest>,
    /// Timeout in seconds after which a request will timeout.  0
    /// means no timeout and is the default.
    #[qapi(name = "timeout")]
    pub timeout: Option<i64>,
}
#[qapi(name = "RbdAuthMode")]
#[qapi(since = "3.0")]
pub enum RbdAuthMode {
    #[qapi(name = "cephx")]
    Cephx,
    #[qapi(name = "none")]
    None,
}
#[qapi(name = "RbdImageEncryptionFormat")]
#[qapi(since = "6.1")]
pub enum RbdImageEncryptionFormat {
    #[qapi(name = "luks")]
    Luks,
    #[qapi(name = "luks2")]
    Luks2,
    /// Used for opening either luks or luks2 (Since 8.0)
    #[qapi(name = "luks-any")]
    LuksAny,
}
#[qapi(name = "RbdEncryptionOptionsLUKSBase")]
#[qapi(since = "6.1")]
pub struct RbdEncryptionOptionsLuksBase {
    /// ID of a QCryptoSecret object providing a passphrase for
    /// unlocking the encryption
    #[qapi(name = "key-secret")]
    pub key_secret: String,
}
#[qapi(name = "RbdEncryptionCreateOptionsLUKSBase")]
#[qapi(since = "6.1")]
pub struct RbdEncryptionCreateOptionsLuksBase {
    /// ID of a QCryptoSecret object providing a passphrase for
    /// unlocking the encryption
    #[qapi(name = "key-secret")]
    pub key_secret: String,
    /// The encryption algorithm
    #[qapi(name = "cipher-alg")]
    pub cipher_alg: Option<QCryptoCipherAlgorithm>,
}
#[qapi(name = "RbdEncryptionOptionsLUKS")]
#[qapi(since = "6.1")]
pub struct RbdEncryptionOptionsLuks {
    /// ID of a QCryptoSecret object providing a passphrase for
    /// unlocking the encryption
    #[qapi(name = "key-secret")]
    pub key_secret: String,
}
#[qapi(name = "RbdEncryptionOptionsLUKS2")]
#[qapi(since = "6.1")]
pub struct RbdEncryptionOptionsLuks2 {
    /// ID of a QCryptoSecret object providing a passphrase for
    /// unlocking the encryption
    #[qapi(name = "key-secret")]
    pub key_secret: String,
}
#[qapi(name = "RbdEncryptionOptionsLUKSAny")]
#[qapi(since = "8.0")]
pub struct RbdEncryptionOptionsLuksAny {
    /// ID of a QCryptoSecret object providing a passphrase for
    /// unlocking the encryption
    #[qapi(name = "key-secret")]
    pub key_secret: String,
}
#[qapi(name = "RbdEncryptionCreateOptionsLUKS")]
#[qapi(since = "6.1")]
pub struct RbdEncryptionCreateOptionsLuks {
    /// ID of a QCryptoSecret object providing a passphrase for
    /// unlocking the encryption
    #[qapi(name = "key-secret")]
    pub key_secret: String,
    /// The encryption algorithm
    #[qapi(name = "cipher-alg")]
    pub cipher_alg: Option<QCryptoCipherAlgorithm>,
}
#[qapi(name = "RbdEncryptionCreateOptionsLUKS2")]
#[qapi(since = "6.1")]
pub struct RbdEncryptionCreateOptionsLuks2 {
    /// ID of a QCryptoSecret object providing a passphrase for
    /// unlocking the encryption
    #[qapi(name = "key-secret")]
    pub key_secret: String,
    /// The encryption algorithm
    #[qapi(name = "cipher-alg")]
    pub cipher_alg: Option<QCryptoCipherAlgorithm>,
}
pub enum RbdEncryptionOptionsBranch {
    #[qapi(name = "luks")]
    Luks(RbdEncryptionOptionsLuks),
    #[qapi(name = "luks2")]
    Luks2(RbdEncryptionOptionsLuks2),
    #[qapi(name = "luks-any")]
    LuksAny(RbdEncryptionOptionsLuksAny),
}
#[qapi(name = "RbdEncryptionOptions")]
#[qapi(since = "6.1")]
pub struct RbdEncryptionOptions {
    /// Encryption format.
    #[qapi(name = "format")]
    #[qapi(discriminator)]
    pub format: RbdImageEncryptionFormat,
    /// Parent image encryption options (for cloned images).  Can
    /// be left unspecified if this cloned image is encrypted using the
    /// same format and secret as its parent image (i.e. not explicitly
    /// formatted) or if its parent image is not encrypted.  (Since 8.0)
    #[qapi(name = "parent")]
    pub parent: Option<RbdEncryptionOptions>,
    #[qapi(union)]
    pub u: Option<RbdEncryptionOptionsBranch>,
}
pub enum RbdEncryptionCreateOptionsBranch {
    #[qapi(name = "luks")]
    Luks(RbdEncryptionCreateOptionsLuks),
    #[qapi(name = "luks2")]
    Luks2(RbdEncryptionCreateOptionsLuks2),
}
#[qapi(name = "RbdEncryptionCreateOptions")]
#[qapi(since = "6.1")]
pub struct RbdEncryptionCreateOptions {
    /// Encryption format.
    #[qapi(name = "format")]
    #[qapi(discriminator)]
    pub format: RbdImageEncryptionFormat,
    #[qapi(union)]
    pub u: Option<RbdEncryptionCreateOptionsBranch>,
}
#[qapi(name = "BlockdevOptionsRbd")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsRbd {
    /// Ceph pool name.
    #[qapi(name = "pool")]
    pub pool: String,
    /// Rados namespace name in the Ceph pool.  (Since 5.0)
    #[qapi(name = "namespace")]
    pub namespace: Option<String>,
    /// Image name in the Ceph pool.
    #[qapi(name = "image")]
    pub image: String,
    /// path to Ceph configuration file.  Values in the configuration
    /// file will be overridden by options specified via QAPI.
    #[qapi(name = "conf")]
    pub conf: Option<String>,
    /// Ceph snapshot name.
    #[qapi(name = "snapshot")]
    pub snapshot: Option<String>,
    /// Image encryption options.  (Since 6.1)
    #[qapi(name = "encrypt")]
    pub encrypt: Option<RbdEncryptionOptions>,
    /// Ceph id name.
    #[qapi(name = "user")]
    pub user: Option<String>,
    /// Acceptable authentication modes.  This maps
    /// to Ceph configuration option "auth_client_required".  (Since
    /// 3.0)
    #[qapi(name = "auth-client-required")]
    pub auth_client_required: Option<Vec<RbdAuthMode>>,
    /// ID of a QCryptoSecret object providing a key for cephx
    /// authentication.  This maps to Ceph configuration option "key".
    /// (Since 3.0)
    #[qapi(name = "key-secret")]
    pub key_secret: Option<String>,
    /// Monitor host address and port.  This maps to the "mon_host"
    /// Ceph option.
    #[qapi(name = "server")]
    pub server: Option<Vec<InetSocketAddressBase>>,
}
/// An enumeration of replication modes.
#[qapi(name = "ReplicationMode")]
#[qapi(condition = "CONFIG_REPLICATION")]
#[qapi(since = "2.9")]
pub enum ReplicationMode {
    /// Primary mode, the vm's state will be sent to secondary
    /// QEMU.
    #[qapi(name = "primary")]
    Primary,
    /// Secondary mode, receive the vm's state from primary
    /// QEMU.
    #[qapi(name = "secondary")]
    Secondary,
}
/// Driver specific block device options for replication
#[qapi(name = "BlockdevOptionsReplication")]
#[qapi(condition = "CONFIG_REPLICATION")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsReplication {
    /// reference to or definition of the data source block device
    #[qapi(name = "file")]
    pub file: BlockdevRef,
    /// the replication mode
    #[qapi(name = "mode")]
    pub mode: ReplicationMode,
    /// In secondary mode, node name or device ID of the root node
    /// who owns the replication node chain.  Must not be given in
    /// primary mode.
    #[qapi(name = "top-id")]
    pub top_id: Option<String>,
}
/// An enumeration of NFS transport types
#[qapi(name = "NFSTransport")]
#[qapi(since = "2.9")]
pub enum NfsTransport {
    /// TCP transport
    #[qapi(name = "inet")]
    Inet,
}
/// Captures the address of the socket
#[qapi(name = "NFSServer")]
#[qapi(since = "2.9")]
pub struct NfsServer {
    /// transport type used for NFS (only TCP supported)
    #[qapi(name = "type")]
    pub r#type: NfsTransport,
    /// host address for NFS server
    #[qapi(name = "host")]
    pub host: String,
}
/// Driver specific block device option for NFS
#[qapi(name = "BlockdevOptionsNfs")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsNfs {
    /// host address
    #[qapi(name = "server")]
    pub server: NfsServer,
    /// path of the image on the host
    #[qapi(name = "path")]
    pub path: String,
    /// UID value to use when talking to the server (defaults to
    /// 65534 on Windows and getuid() on unix)
    #[qapi(name = "user")]
    pub user: Option<i64>,
    /// GID value to use when talking to the server (defaults to
    /// 65534 on Windows and getgid() in unix)
    #[qapi(name = "group")]
    pub group: Option<i64>,
    /// number of SYNs during the session establishment
    /// (defaults to libnfs default)
    #[qapi(name = "tcp-syn-count")]
    pub tcp_syn_count: Option<i64>,
    /// set the readahead size in bytes (defaults to libnfs
    /// default)
    #[qapi(name = "readahead-size")]
    pub readahead_size: Option<i64>,
    /// set the pagecache size in bytes (defaults to
    /// libnfs default)
    #[qapi(name = "page-cache-size")]
    pub page_cache_size: Option<i64>,
    /// set the NFS debug level (max 2) (defaults to libnfs default)
    #[qapi(name = "debug")]
    pub debug: Option<i64>,
}
/// Driver specific block device options shared by all protocols
/// supported by the curl backend.
#[qapi(name = "BlockdevOptionsCurlBase")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsCurlBase {
    /// URL of the image file
    #[qapi(name = "url")]
    pub url: String,
    /// Size of the read-ahead cache; must be a multiple of 512
    /// (defaults to 256 kB)
    #[qapi(name = "readahead")]
    pub readahead: Option<i64>,
    /// Timeout for connections, in seconds (defaults to 5)
    #[qapi(name = "timeout")]
    pub timeout: Option<i64>,
    /// Username for authentication (defaults to none)
    #[qapi(name = "username")]
    pub username: Option<String>,
    /// ID of a QCryptoSecret object providing a password
    /// for authentication (defaults to no password)
    #[qapi(name = "password-secret")]
    pub password_secret: Option<String>,
    /// Username for proxy authentication (defaults to
    /// none)
    #[qapi(name = "proxy-username")]
    pub proxy_username: Option<String>,
    /// ID of a QCryptoSecret object providing a
    /// password for proxy authentication (defaults to no password)
    #[qapi(name = "proxy-password-secret")]
    pub proxy_password_secret: Option<String>,
}
/// Driver specific block device options for HTTP connections over the
/// curl backend.  URLs must start with "http://".
#[qapi(name = "BlockdevOptionsCurlHttp")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsCurlHttp {
    /// URL of the image file
    #[qapi(name = "url")]
    pub url: String,
    /// Size of the read-ahead cache; must be a multiple of 512
    /// (defaults to 256 kB)
    #[qapi(name = "readahead")]
    pub readahead: Option<i64>,
    /// Timeout for connections, in seconds (defaults to 5)
    #[qapi(name = "timeout")]
    pub timeout: Option<i64>,
    /// Username for authentication (defaults to none)
    #[qapi(name = "username")]
    pub username: Option<String>,
    /// ID of a QCryptoSecret object providing a password
    /// for authentication (defaults to no password)
    #[qapi(name = "password-secret")]
    pub password_secret: Option<String>,
    /// Username for proxy authentication (defaults to
    /// none)
    #[qapi(name = "proxy-username")]
    pub proxy_username: Option<String>,
    /// ID of a QCryptoSecret object providing a
    /// password for proxy authentication (defaults to no password)
    #[qapi(name = "proxy-password-secret")]
    pub proxy_password_secret: Option<String>,
    /// List of cookies to set; format is "name1=content1;
    /// name2=content2;" as explained by CURLOPT_COOKIE(3).  Defaults to
    /// no cookies.
    #[qapi(name = "cookie")]
    pub cookie: Option<String>,
    /// ID of a QCryptoSecret object providing the cookie
    /// data in a secure way.  See @cookie for the format.  (since 2.10)
    #[qapi(name = "cookie-secret")]
    pub cookie_secret: Option<String>,
}
/// Driver specific block device options for HTTPS connections over the
/// curl backend.  URLs must start with "https://".
#[qapi(name = "BlockdevOptionsCurlHttps")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsCurlHttps {
    /// URL of the image file
    #[qapi(name = "url")]
    pub url: String,
    /// Size of the read-ahead cache; must be a multiple of 512
    /// (defaults to 256 kB)
    #[qapi(name = "readahead")]
    pub readahead: Option<i64>,
    /// Timeout for connections, in seconds (defaults to 5)
    #[qapi(name = "timeout")]
    pub timeout: Option<i64>,
    /// Username for authentication (defaults to none)
    #[qapi(name = "username")]
    pub username: Option<String>,
    /// ID of a QCryptoSecret object providing a password
    /// for authentication (defaults to no password)
    #[qapi(name = "password-secret")]
    pub password_secret: Option<String>,
    /// Username for proxy authentication (defaults to
    /// none)
    #[qapi(name = "proxy-username")]
    pub proxy_username: Option<String>,
    /// ID of a QCryptoSecret object providing a
    /// password for proxy authentication (defaults to no password)
    #[qapi(name = "proxy-password-secret")]
    pub proxy_password_secret: Option<String>,
    /// List of cookies to set; format is "name1=content1;
    /// name2=content2;" as explained by CURLOPT_COOKIE(3).  Defaults to
    /// no cookies.
    #[qapi(name = "cookie")]
    pub cookie: Option<String>,
    /// Whether to verify the SSL certificate's validity
    /// (defaults to true)
    #[qapi(name = "sslverify")]
    pub sslverify: Option<bool>,
    /// ID of a QCryptoSecret object providing the cookie
    /// data in a secure way.  See @cookie for the format.  (since 2.10)
    #[qapi(name = "cookie-secret")]
    pub cookie_secret: Option<String>,
}
/// Driver specific block device options for FTP connections over the
/// curl backend.  URLs must start with "ftp://".
#[qapi(name = "BlockdevOptionsCurlFtp")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsCurlFtp {
    /// URL of the image file
    #[qapi(name = "url")]
    pub url: String,
    /// Size of the read-ahead cache; must be a multiple of 512
    /// (defaults to 256 kB)
    #[qapi(name = "readahead")]
    pub readahead: Option<i64>,
    /// Timeout for connections, in seconds (defaults to 5)
    #[qapi(name = "timeout")]
    pub timeout: Option<i64>,
    /// Username for authentication (defaults to none)
    #[qapi(name = "username")]
    pub username: Option<String>,
    /// ID of a QCryptoSecret object providing a password
    /// for authentication (defaults to no password)
    #[qapi(name = "password-secret")]
    pub password_secret: Option<String>,
    /// Username for proxy authentication (defaults to
    /// none)
    #[qapi(name = "proxy-username")]
    pub proxy_username: Option<String>,
    /// ID of a QCryptoSecret object providing a
    /// password for proxy authentication (defaults to no password)
    #[qapi(name = "proxy-password-secret")]
    pub proxy_password_secret: Option<String>,
}
/// Driver specific block device options for FTPS connections over the
/// curl backend.  URLs must start with "ftps://".
#[qapi(name = "BlockdevOptionsCurlFtps")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsCurlFtps {
    /// URL of the image file
    #[qapi(name = "url")]
    pub url: String,
    /// Size of the read-ahead cache; must be a multiple of 512
    /// (defaults to 256 kB)
    #[qapi(name = "readahead")]
    pub readahead: Option<i64>,
    /// Timeout for connections, in seconds (defaults to 5)
    #[qapi(name = "timeout")]
    pub timeout: Option<i64>,
    /// Username for authentication (defaults to none)
    #[qapi(name = "username")]
    pub username: Option<String>,
    /// ID of a QCryptoSecret object providing a password
    /// for authentication (defaults to no password)
    #[qapi(name = "password-secret")]
    pub password_secret: Option<String>,
    /// Username for proxy authentication (defaults to
    /// none)
    #[qapi(name = "proxy-username")]
    pub proxy_username: Option<String>,
    /// ID of a QCryptoSecret object providing a
    /// password for proxy authentication (defaults to no password)
    #[qapi(name = "proxy-password-secret")]
    pub proxy_password_secret: Option<String>,
    /// Whether to verify the SSL certificate's validity
    /// (defaults to true)
    #[qapi(name = "sslverify")]
    pub sslverify: Option<bool>,
}
/// Driver specific block device options for NBD.
#[qapi(name = "BlockdevOptionsNbd")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsNbd {
    /// NBD server address
    #[qapi(name = "server")]
    pub server: SocketAddress,
    /// export name
    #[qapi(name = "export")]
    pub export: Option<String>,
    /// TLS credentials ID
    #[qapi(name = "tls-creds")]
    pub tls_creds: Option<String>,
    /// TLS hostname override for certificate validation
    /// (Since 7.0)
    #[qapi(name = "tls-hostname")]
    pub tls_hostname: Option<String>,
    /// A metadata context name such as
    /// "qemu:dirty-bitmap:NAME" or "qemu:allocation-depth" to query in
    /// place of the traditional "base:allocation" block status (see
    /// NBD_OPT_LIST_META_CONTEXT in the NBD protocol; and yes, naming
    /// this option x-context would have made more sense) (since 3.0)
    #[qapi(name = "x-dirty-bitmap")]
    #[qapi(feature = "unstable")]
    pub x_dirty_bitmap: Option<String>,
    /// On an unexpected disconnect, the nbd client tries
    /// to connect again until succeeding or encountering a serious
    /// error.  During the first @reconnect-delay seconds, all requests
    /// are paused and will be rerun on a successful reconnect.  After
    /// that time, any delayed requests and all future requests before a
    /// successful reconnect will immediately fail.  Default 0 (Since
    /// 4.2)
    #[qapi(name = "reconnect-delay")]
    pub reconnect_delay: Option<u32>,
    /// In seconds.  If zero, the nbd driver tries the
    /// connection only once, and fails to open if the connection fails.
    /// If non-zero, the nbd driver will repeat connection attempts
    /// until successful or until @open-timeout seconds have elapsed.
    /// Default 0 (Since 7.0)
    #[qapi(name = "open-timeout")]
    pub open_timeout: Option<u32>,
}
/// Driver specific block device options for the raw driver.
#[qapi(name = "BlockdevOptionsRaw")]
#[qapi(since = "2.9")]
pub struct BlockdevOptionsRaw {
    /// reference to or definition of the data source block device
    #[qapi(name = "file")]
    pub file: BlockdevRef,
    /// position where the block device starts
    #[qapi(name = "offset")]
    pub offset: Option<i64>,
    /// the assumed size of the device
    #[qapi(name = "size")]
    pub size: Option<i64>,
}
/// Driver specific block device options for the throttle driver
#[qapi(name = "BlockdevOptionsThrottle")]
#[qapi(since = "2.11")]
pub struct BlockdevOptionsThrottle {
    /// the name of the throttle-group object to use.  It
    /// must already exist.
    #[qapi(name = "throttle-group")]
    pub throttle_group: String,
    /// reference to or definition of the data source block device
    #[qapi(name = "file")]
    pub file: BlockdevRef,
}
/// Driver specific block device options for the copy-on-read driver.
#[qapi(name = "BlockdevOptionsCor")]
#[qapi(since = "6.0")]
pub struct BlockdevOptionsCor {
    /// reference to or definition of the data source block device
    #[qapi(name = "file")]
    pub file: BlockdevRef,
    /// The name of a non-filter node (allocation-bearing layer)
    /// that limits the COR operations in the backing chain (inclusive),
    /// so that no data below this node will be copied by this filter.
    /// If option is absent, the limit is not applied, so that data from
    /// all backing layers may be copied.
    #[qapi(name = "bottom")]
    pub bottom: Option<String>,
}
/// An enumeration of possible behaviors for copy-before-write operation
/// failures.
#[qapi(name = "OnCbwError")]
#[qapi(since = "7.1")]
pub enum OnCbwError {
    /// report the error to the guest.  This way, the
    /// guest will not be able to overwrite areas that cannot be backed
    /// up, so the backup process remains valid.
    #[qapi(name = "break-guest-write")]
    BreakGuestWrite,
    /// continue guest write.  Doing so will make the
    /// provided snapshot state invalid and any backup or export process
    /// based on it will finally fail.
    #[qapi(name = "break-snapshot")]
    BreakSnapshot,
}
/// Driver specific block device options for the copy-before-write
/// driver, which does so called copy-before-write operations: when data
/// is written to the filter, the filter first reads corresponding
/// blocks from its file child and copies them to @target child.  After
/// successfully copying, the write request is propagated to file child.
/// If copying fails, the original write request is failed too and no
/// data is written to file child.
#[qapi(name = "BlockdevOptionsCbw")]
#[qapi(since = "6.2")]
pub struct BlockdevOptionsCbw {
    /// reference to or definition of the data source block device
    #[qapi(name = "file")]
    pub file: BlockdevRef,
    /// The target for copy-before-write operations.
    #[qapi(name = "target")]
    pub target: BlockdevRef,
    /// If specified, copy-before-write filter will do
    /// copy-before-write operations only for dirty regions of the
    /// bitmap.  Bitmap size must be equal to length of file and target
    /// child of the filter.  Note also, that bitmap is used only to
    /// initialize internal bitmap of the process, so further
    /// modifications (or removing) of specified bitmap doesn't
    /// influence the filter.  (Since 7.0)
    #[qapi(name = "bitmap")]
    pub bitmap: Option<BlockDirtyBitmap>,
    /// Behavior on failure of copy-before-write operation.
    /// Default is @break-guest-write.  (Since 7.1)
    #[qapi(name = "on-cbw-error")]
    pub on_cbw_error: Option<OnCbwError>,
    /// Zero means no limit.  Non-zero sets the timeout in
    /// seconds for copy-before-write operation.  When a timeout occurs,
    /// the respective copy-before-write operation will fail, and the
    /// @on-cbw-error parameter will decide how this failure is handled.
    /// Default 0.  (Since 7.1)
    #[qapi(name = "cbw-timeout")]
    pub cbw_timeout: Option<u32>,
}
pub enum BlockdevOptionsBranch {
    #[qapi(name = "blkdebug")]
    Blkdebug(BlockdevOptionsBlkdebug),
    #[qapi(name = "blklogwrites")]
    Blklogwrites(BlockdevOptionsBlklogwrites),
    #[qapi(name = "blkverify")]
    Blkverify(BlockdevOptionsBlkverify),
    #[qapi(name = "blkreplay")]
    Blkreplay(BlockdevOptionsBlkreplay),
    #[qapi(name = "bochs")]
    Bochs(BlockdevOptionsGenericFormat),
    #[qapi(name = "cloop")]
    Cloop(BlockdevOptionsGenericFormat),
    #[qapi(name = "compress")]
    Compress(BlockdevOptionsGenericFormat),
    #[qapi(name = "copy-before-write")]
    CopyBeforeWrite(BlockdevOptionsCbw),
    #[qapi(name = "copy-on-read")]
    CopyOnRead(BlockdevOptionsCor),
    #[qapi(name = "dmg")]
    Dmg(BlockdevOptionsGenericFormat),
    #[qapi(name = "file")]
    File(BlockdevOptionsFile),
    #[qapi(name = "ftp")]
    Ftp(BlockdevOptionsCurlFtp),
    #[qapi(name = "ftps")]
    Ftps(BlockdevOptionsCurlFtps),
    #[qapi(name = "gluster")]
    Gluster(BlockdevOptionsGluster),
    #[qapi(name = "host_cdrom")]
    #[qapi(condition = "HAVE_HOST_BLOCK_DEVICE")]
    HostCdrom(BlockdevOptionsFile),
    #[qapi(name = "host_device")]
    #[qapi(condition = "HAVE_HOST_BLOCK_DEVICE")]
    HostDevice(BlockdevOptionsFile),
    #[qapi(name = "http")]
    Http(BlockdevOptionsCurlHttp),
    #[qapi(name = "https")]
    Https(BlockdevOptionsCurlHttps),
    #[qapi(name = "io_uring")]
    #[qapi(condition = "CONFIG_BLKIO")]
    IoUring(BlockdevOptionsIoUring),
    #[qapi(name = "iscsi")]
    Iscsi(BlockdevOptionsIscsi),
    #[qapi(name = "luks")]
    Luks(BlockdevOptionsLuks),
    #[qapi(name = "nbd")]
    Nbd(BlockdevOptionsNbd),
    #[qapi(name = "nfs")]
    Nfs(BlockdevOptionsNfs),
    #[qapi(name = "null-aio")]
    NullAio(BlockdevOptionsNull),
    #[qapi(name = "null-co")]
    NullCo(BlockdevOptionsNull),
    #[qapi(name = "nvme")]
    Nvme(BlockdevOptionsNvMe),
    #[qapi(name = "nvme-io_uring")]
    #[qapi(condition = "CONFIG_BLKIO")]
    NvmeIoUring(BlockdevOptionsNvmeIoUring),
    #[qapi(name = "parallels")]
    Parallels(BlockdevOptionsGenericFormat),
    #[qapi(name = "preallocate")]
    Preallocate(BlockdevOptionsPreallocate),
    #[qapi(name = "qcow2")]
    Qcow2(BlockdevOptionsQcow2),
    #[qapi(name = "qcow")]
    Qcow(BlockdevOptionsQcow),
    #[qapi(name = "qed")]
    Qed(BlockdevOptionsGenericCowFormat),
    #[qapi(name = "quorum")]
    Quorum(BlockdevOptionsQuorum),
    #[qapi(name = "raw")]
    Raw(BlockdevOptionsRaw),
    #[qapi(name = "rbd")]
    Rbd(BlockdevOptionsRbd),
    #[qapi(name = "replication")]
    #[qapi(condition = "CONFIG_REPLICATION")]
    Replication(BlockdevOptionsReplication),
    #[qapi(name = "snapshot-access")]
    SnapshotAccess(BlockdevOptionsGenericFormat),
    #[qapi(name = "ssh")]
    Ssh(BlockdevOptionsSsh),
    #[qapi(name = "throttle")]
    Throttle(BlockdevOptionsThrottle),
    #[qapi(name = "vdi")]
    Vdi(BlockdevOptionsGenericFormat),
    #[qapi(name = "vhdx")]
    Vhdx(BlockdevOptionsGenericFormat),
    #[qapi(name = "virtio-blk-vfio-pci")]
    #[qapi(condition = "CONFIG_BLKIO")]
    VirtioBlkVfioPci(BlockdevOptionsVirtioBlkVfioPci),
    #[qapi(name = "virtio-blk-vhost-user")]
    #[qapi(condition = "CONFIG_BLKIO")]
    VirtioBlkVhostUser(BlockdevOptionsVirtioBlkVhostUser),
    #[qapi(name = "virtio-blk-vhost-vdpa")]
    #[qapi(condition = "CONFIG_BLKIO")]
    VirtioBlkVhostVdpa(BlockdevOptionsVirtioBlkVhostVdpa),
    #[qapi(name = "vmdk")]
    Vmdk(BlockdevOptionsGenericCowFormat),
    #[qapi(name = "vpc")]
    Vpc(BlockdevOptionsGenericFormat),
    #[qapi(name = "vvfat")]
    Vvfat(BlockdevOptionsVvfat),
}
/// Options for creating a block device.  Many options are available for
/// all block devices, independent of the block driver:
#[qapi(name = "BlockdevOptions")]
#[qapi(since = "2.9")]
pub struct BlockdevOptions {
    /// block driver name
    #[qapi(name = "driver")]
    #[qapi(discriminator)]
    pub driver: BlockdevDriver,
    /// the node name of the new node (Since 2.0).  This option
    /// is required on the top level of blockdev-add.  Valid node names
    /// start with an alphabetic character and may contain only
    /// alphanumeric characters, '-', '.' and '_'.  Their maximum length
    /// is 31 characters.
    #[qapi(name = "node-name")]
    pub node_name: Option<String>,
    /// discard-related options (default: ignore)
    #[qapi(name = "discard")]
    pub discard: Option<BlockdevDiscardOptions>,
    /// cache-related options
    #[qapi(name = "cache")]
    pub cache: Option<BlockdevCacheOptions>,
    /// whether the block device should be read-only (default:
    /// false).  Note that some block drivers support only read-only
    /// access, either generally or in certain configurations.  In this
    /// case, the default value does not work and the option must be
    /// specified explicitly.
    #[qapi(name = "read-only")]
    pub read_only: Option<bool>,
    /// if true and @read-only is false, QEMU may
    /// automatically decide not to open the image read-write as
    /// requested, but fall back to read-only instead (and switch
    /// between the modes later), e.g. depending on whether the image
    /// file is writable or whether a writing user is attached to the
    /// node (default: false, since 3.1)
    #[qapi(name = "auto-read-only")]
    pub auto_read_only: Option<bool>,
    /// force share all permission on added nodes.  Requires
    /// read-only=true.  (Since 2.10)
    #[qapi(name = "force-share")]
    pub force_share: Option<bool>,
    /// detect and optimize zero writes (Since 2.1)
    /// (default: off)
    #[qapi(name = "detect-zeroes")]
    pub detect_zeroes: Option<BlockdevDetectZeroesOptions>,
    #[qapi(union)]
    pub u: Option<BlockdevOptionsBranch>,
}
/// Reference to a block device.
#[qapi(name = "BlockdevRef")]
#[qapi(since = "2.9")]
pub enum BlockdevRef {
    /// defines a new block device inline
    #[qapi(name = "definition")]
    Definition(BlockdevOptions),
    /// references the ID of an existing block device
    #[qapi(name = "reference")]
    Reference(String),
}
/// Reference to a block device.
#[qapi(name = "BlockdevRefOrNull")]
#[qapi(since = "2.9")]
pub enum BlockdevRefOrNull {
    /// defines a new block device inline
    #[qapi(name = "definition")]
    Definition(BlockdevOptions),
    /// references the ID of an existing block device.  An empty
    /// string means that no block device should be referenced.
    /// Deprecated; use null instead.
    #[qapi(name = "reference")]
    Reference(String),
    /// No block device should be referenced (since 2.10)
    #[qapi(name = "null")]
    Null(Null),
}
/// Creates a new block device.
#[qapi(name = "blockdev-add")]
#[qapi(since = "2.9")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockdevAdd {
    #[qapi(flatten)]
    pub data: BlockdevOptions,
}
/// Reopens one or more block devices using the given set of options.
/// Any option not specified will be reset to its default value
/// regardless of its previous status.  If an option cannot be changed
/// or a particular driver does not support reopening then the command
/// will return an error.  All devices in the list are reopened in one
/// transaction, so if one of them fails then the whole transaction is
/// cancelled.
///
/// The command receives a list of block devices to reopen.  For each
/// one of them, the top-level @node-name option (from BlockdevOptions)
/// must be specified and is used to select the block device to be
/// reopened.  Other @node-name options must be either omitted or set to
/// the current name of the appropriate node.  This command won't change
/// any node name and any attempt to do it will result in an error.
///
/// In the case of options that refer to child nodes, the behavior of
/// this command depends on the value:
///
/// 1) A set of options (BlockdevOptions): the child is reopened with
/// the specified set of options.
///
/// 2) A reference to the current child: the child is reopened using
/// its existing set of options.
///
/// 3) A reference to a different node: the current child is replaced
/// with the specified one.
///
/// 4) NULL: the current child (if any) is detached.
///
/// Options (1) and (2) are supported in all cases.  Option (3) is
/// supported for @file and @backing, and option (4) for @backing only.
///
/// Unlike with blockdev-add, the @backing option must always be present
/// unless the node being reopened does not have a backing file and its
/// image does not have a default backing file name as part of its
/// metadata.
#[qapi(name = "blockdev-reopen")]
#[qapi(since = "6.1")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockdevReopen {
    #[qapi(name = "options")]
    pub options: Vec<BlockdevOptions>,
}
/// Deletes a block device that has been added using blockdev-add.  The
/// command will fail if the node is attached to a device or is
/// otherwise being used.
#[qapi(name = "blockdev-del")]
#[qapi(since = "2.9")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockdevDel {
    /// Name of the graph node to delete.
    #[qapi(name = "node-name")]
    pub node_name: String,
}
/// Driver specific image creation options for file.
#[qapi(name = "BlockdevCreateOptionsFile")]
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsFile {
    /// Filename for the new image file
    #[qapi(name = "filename")]
    pub filename: String,
    /// Size of the virtual disk in bytes
    #[qapi(name = "size")]
    pub size: u64,
    /// Preallocation mode for the new image (default: off;
    /// allowed values: off, falloc (if CONFIG_POSIX_FALLOCATE), full
    /// (if CONFIG_POSIX))
    #[qapi(name = "preallocation")]
    pub preallocation: Option<PreallocMode>,
    /// Turn off copy-on-write (valid only on btrfs; default: off)
    #[qapi(name = "nocow")]
    pub nocow: Option<bool>,
    /// Extent size hint to add to the image file; 0 for
    /// not adding an extent size hint (default: 1 MB, since 5.1)
    #[qapi(name = "extent-size-hint")]
    pub extent_size_hint: Option<u64>,
}
/// Driver specific image creation options for gluster.
#[qapi(name = "BlockdevCreateOptionsGluster")]
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsGluster {
    /// Where to store the new image file
    #[qapi(name = "location")]
    pub location: BlockdevOptionsGluster,
    /// Size of the virtual disk in bytes
    #[qapi(name = "size")]
    pub size: u64,
    /// Preallocation mode for the new image (default: off;
    /// allowed values: off, falloc (if CONFIG_GLUSTERFS_FALLOCATE),
    /// full (if CONFIG_GLUSTERFS_ZEROFILL))
    #[qapi(name = "preallocation")]
    pub preallocation: Option<PreallocMode>,
}
/// Driver specific image creation options for LUKS.
#[qapi(name = "BlockdevCreateOptionsLUKS")]
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsLuks {
    /// the ID of a QCryptoSecret object providing the
    /// decryption key.  Mandatory except when probing image for
    /// metadata only.
    #[qapi(name = "key-secret")]
    pub key_secret: Option<String>,
    /// the cipher algorithm for data encryption Currently
    /// defaults to 'aes-256'.
    #[qapi(name = "cipher-alg")]
    pub cipher_alg: Option<QCryptoCipherAlgorithm>,
    /// the cipher mode for data encryption Currently defaults
    /// to 'xts'
    #[qapi(name = "cipher-mode")]
    pub cipher_mode: Option<QCryptoCipherMode>,
    /// the initialization vector generator Currently defaults
    /// to 'plain64'
    #[qapi(name = "ivgen-alg")]
    pub ivgen_alg: Option<QCryptoIvGenAlgorithm>,
    /// the initialization vector generator hash Currently
    /// defaults to 'sha256'
    #[qapi(name = "ivgen-hash-alg")]
    pub ivgen_hash_alg: Option<QCryptoHashAlgorithm>,
    /// the master key hash algorithm Currently defaults to
    /// 'sha256'
    #[qapi(name = "hash-alg")]
    pub hash_alg: Option<QCryptoHashAlgorithm>,
    /// number of milliseconds to spend in PBKDF passphrase
    /// processing.  Currently defaults to 2000.  (since 2.8)
    #[qapi(name = "iter-time")]
    pub iter_time: Option<i64>,
    /// Node to create the image format on, mandatory except when
    /// 'preallocation' is not requested
    #[qapi(name = "file")]
    pub file: Option<BlockdevRef>,
    /// Block device holding a detached LUKS header.  (since 9.0)
    #[qapi(name = "header")]
    pub header: Option<BlockdevRef>,
    /// Size of the virtual disk in bytes
    #[qapi(name = "size")]
    pub size: u64,
    /// Preallocation mode for the new image (since: 4.2)
    /// (default: off; allowed values: off, metadata, falloc, full)
    #[qapi(name = "preallocation")]
    pub preallocation: Option<PreallocMode>,
}
/// Driver specific image creation options for NFS.
#[qapi(name = "BlockdevCreateOptionsNfs")]
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsNfs {
    /// Where to store the new image file
    #[qapi(name = "location")]
    pub location: BlockdevOptionsNfs,
    /// Size of the virtual disk in bytes
    #[qapi(name = "size")]
    pub size: u64,
}
/// Driver specific image creation options for parallels.
#[qapi(name = "BlockdevCreateOptionsParallels")]
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsParallels {
    /// Node to create the image format on
    #[qapi(name = "file")]
    pub file: BlockdevRef,
    /// Size of the virtual disk in bytes
    #[qapi(name = "size")]
    pub size: u64,
    /// Cluster size in bytes (default: 1 MB)
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<u64>,
}
/// Driver specific image creation options for qcow.
#[qapi(name = "BlockdevCreateOptionsQcow")]
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsQcow {
    /// Node to create the image format on
    #[qapi(name = "file")]
    pub file: BlockdevRef,
    /// Size of the virtual disk in bytes
    #[qapi(name = "size")]
    pub size: u64,
    /// File name of the backing file if a backing file
    /// should be used
    #[qapi(name = "backing-file")]
    pub backing_file: Option<String>,
    /// Encryption options if the image should be encrypted
    #[qapi(name = "encrypt")]
    pub encrypt: Option<QCryptoBlockCreateOptions>,
}
#[qapi(name = "BlockdevQcow2Version")]
#[qapi(since = "2.12")]
pub enum BlockdevQcow2Version {
    /// The original QCOW2 format as introduced in qemu 0.10 (version
    /// 2)
    #[qapi(name = "v2")]
    V2,
    /// The extended QCOW2 format as introduced in qemu 1.1 (version 3)
    #[qapi(name = "v3")]
    V3,
}
/// Compression type used in qcow2 image file
#[qapi(name = "Qcow2CompressionType")]
#[qapi(since = "5.1")]
pub enum Qcow2CompressionType {
    /// zlib compression, see <http://zlib.net/>
    #[qapi(name = "zlib")]
    Zlib,
    /// zstd compression, see <http://github.com/facebook/zstd>
    #[qapi(name = "zstd")]
    #[qapi(condition = "CONFIG_ZSTD")]
    Zstd,
}
/// Driver specific image creation options for qcow2.
#[qapi(name = "BlockdevCreateOptionsQcow2")]
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsQcow2 {
    /// Node to create the image format on
    #[qapi(name = "file")]
    pub file: BlockdevRef,
    /// Node to use as an external data file in which all guest
    /// data is stored so that only metadata remains in the qcow2 file
    /// (since: 4.0)
    #[qapi(name = "data-file")]
    pub data_file: Option<BlockdevRef>,
    /// True if the external data file must stay valid as a
    /// standalone (read-only) raw image without looking at qcow2
    /// metadata (default: false; since: 4.0)
    #[qapi(name = "data-file-raw")]
    pub data_file_raw: Option<bool>,
    /// True to make the image have extended L2 entries
    /// (default: false; since 5.2)
    #[qapi(name = "extended-l2")]
    pub extended_l2: Option<bool>,
    /// Size of the virtual disk in bytes
    #[qapi(name = "size")]
    pub size: u64,
    /// Compatibility level (default: v3)
    #[qapi(name = "version")]
    pub version: Option<BlockdevQcow2Version>,
    /// File name of the backing file if a backing file
    /// should be used
    #[qapi(name = "backing-file")]
    pub backing_file: Option<String>,
    /// Name of the block driver to use for the backing file
    #[qapi(name = "backing-fmt")]
    pub backing_fmt: Option<BlockdevDriver>,
    /// Encryption options if the image should be encrypted
    #[qapi(name = "encrypt")]
    pub encrypt: Option<QCryptoBlockCreateOptions>,
    /// qcow2 cluster size in bytes (default: 65536)
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<u64>,
    /// Preallocation mode for the new image (default: off;
    /// allowed values: off, falloc, full, metadata)
    #[qapi(name = "preallocation")]
    pub preallocation: Option<PreallocMode>,
    /// True if refcounts may be updated lazily
    /// (default: off)
    #[qapi(name = "lazy-refcounts")]
    pub lazy_refcounts: Option<bool>,
    /// Width of reference counts in bits (default: 16)
    #[qapi(name = "refcount-bits")]
    pub refcount_bits: Option<i64>,
    /// The image cluster compression method
    /// (default: zlib, since 5.1)
    #[qapi(name = "compression-type")]
    pub compression_type: Option<Qcow2CompressionType>,
}
/// Driver specific image creation options for qed.
#[qapi(name = "BlockdevCreateOptionsQed")]
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsQed {
    /// Node to create the image format on
    #[qapi(name = "file")]
    pub file: BlockdevRef,
    /// Size of the virtual disk in bytes
    #[qapi(name = "size")]
    pub size: u64,
    /// File name of the backing file if a backing file
    /// should be used
    #[qapi(name = "backing-file")]
    pub backing_file: Option<String>,
    /// Name of the block driver to use for the backing file
    #[qapi(name = "backing-fmt")]
    pub backing_fmt: Option<BlockdevDriver>,
    /// Cluster size in bytes (default: 65536)
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<u64>,
    /// L1/L2 table size (in clusters)
    #[qapi(name = "table-size")]
    pub table_size: Option<i64>,
}
/// Driver specific image creation options for rbd/Ceph.
#[qapi(name = "BlockdevCreateOptionsRbd")]
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsRbd {
    /// Where to store the new image file.  This location cannot
    /// point to a snapshot.
    #[qapi(name = "location")]
    pub location: BlockdevOptionsRbd,
    /// Size of the virtual disk in bytes
    #[qapi(name = "size")]
    pub size: u64,
    /// RBD object size
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<u64>,
    /// Image encryption options.  (Since 6.1)
    #[qapi(name = "encrypt")]
    pub encrypt: Option<RbdEncryptionCreateOptions>,
}
/// Subformat options for VMDK images
#[qapi(name = "BlockdevVmdkSubformat")]
#[qapi(since = "4.0")]
pub enum BlockdevVmdkSubformat {
    /// Single file image with sparse cluster allocation
    #[qapi(name = "monolithicSparse")]
    MonolithicSparse,
    /// Single flat data image and a descriptor file
    #[qapi(name = "monolithicFlat")]
    MonolithicFlat,
    /// Data is split into 2GB (per virtual LBA)
    /// sparse extent files, in addition to a descriptor file
    #[qapi(name = "twoGbMaxExtentSparse")]
    TwoGbMaxExtentSparse,
    /// Data is split into 2GB (per virtual LBA) flat
    /// extent files, in addition to a descriptor file
    #[qapi(name = "twoGbMaxExtentFlat")]
    TwoGbMaxExtentFlat,
    /// Single file image sparse cluster allocation,
    /// optimized for streaming over network.
    #[qapi(name = "streamOptimized")]
    StreamOptimized,
}
/// Adapter type info for VMDK images
#[qapi(name = "BlockdevVmdkAdapterType")]
#[qapi(since = "4.0")]
pub enum BlockdevVmdkAdapterType {
    #[qapi(name = "ide")]
    Ide,
    #[qapi(name = "buslogic")]
    Buslogic,
    #[qapi(name = "lsilogic")]
    Lsilogic,
    #[qapi(name = "legacyESX")]
    LegacyEsx,
}
/// Driver specific image creation options for VMDK.
#[qapi(name = "BlockdevCreateOptionsVmdk")]
#[qapi(since = "4.0")]
pub struct BlockdevCreateOptionsVmdk {
    /// Where to store the new image file.  This refers to the image
    /// file for monolithcSparse and streamOptimized format, or the
    /// descriptor file for other formats.
    #[qapi(name = "file")]
    pub file: BlockdevRef,
    /// Size of the virtual disk in bytes
    #[qapi(name = "size")]
    pub size: u64,
    /// Where to store the data extents.  Required for
    /// monolithcFlat, twoGbMaxExtentSparse and twoGbMaxExtentFlat
    /// formats.  For monolithicFlat, only one entry is required; for
    /// twoGbMaxExtent* formats, the number of entries required is
    /// calculated as extent_number = virtual_size / 2GB.  Providing
    /// more extents than will be used is an error.
    #[qapi(name = "extents")]
    pub extents: Option<Vec<BlockdevRef>>,
    /// The subformat of the VMDK image.  Default:
    /// "monolithicSparse".
    #[qapi(name = "subformat")]
    pub subformat: Option<BlockdevVmdkSubformat>,
    /// The path of backing file.  Default: no backing file
    /// is used.
    #[qapi(name = "backing-file")]
    pub backing_file: Option<String>,
    /// The adapter type used to fill in the descriptor.
    /// Default: ide.
    #[qapi(name = "adapter-type")]
    pub adapter_type: Option<BlockdevVmdkAdapterType>,
    /// Hardware version.  The meaningful options are "4" or
    /// "6".  Default: "4".
    #[qapi(name = "hwversion")]
    pub hwversion: Option<String>,
    /// VMware guest tools version.  Default: "2147483647"
    /// (Since 6.2)
    #[qapi(name = "toolsversion")]
    pub toolsversion: Option<String>,
    /// Whether to enable zeroed-grain feature for sparse
    /// subformats.  Default: false.
    #[qapi(name = "zeroed-grain")]
    pub zeroed_grain: Option<bool>,
}
/// Driver specific image creation options for SSH.
#[qapi(name = "BlockdevCreateOptionsSsh")]
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsSsh {
    /// Where to store the new image file
    #[qapi(name = "location")]
    pub location: BlockdevOptionsSsh,
    /// Size of the virtual disk in bytes
    #[qapi(name = "size")]
    pub size: u64,
}
/// Driver specific image creation options for VDI.
#[qapi(name = "BlockdevCreateOptionsVdi")]
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsVdi {
    /// Node to create the image format on
    #[qapi(name = "file")]
    pub file: BlockdevRef,
    /// Size of the virtual disk in bytes
    #[qapi(name = "size")]
    pub size: u64,
    /// Preallocation mode for the new image (default: off;
    /// allowed values: off, metadata)
    #[qapi(name = "preallocation")]
    pub preallocation: Option<PreallocMode>,
}
#[qapi(name = "BlockdevVhdxSubformat")]
#[qapi(since = "2.12")]
pub enum BlockdevVhdxSubformat {
    /// Growing image file
    #[qapi(name = "dynamic")]
    Dynamic,
    /// Preallocated fixed-size image file
    #[qapi(name = "fixed")]
    Fixed,
}
/// Driver specific image creation options for vhdx.
#[qapi(name = "BlockdevCreateOptionsVhdx")]
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsVhdx {
    /// Node to create the image format on
    #[qapi(name = "file")]
    pub file: BlockdevRef,
    /// Size of the virtual disk in bytes
    #[qapi(name = "size")]
    pub size: u64,
    /// Log size in bytes, must be a multiple of 1 MB (default: 1
    /// MB)
    #[qapi(name = "log-size")]
    pub log_size: Option<u64>,
    /// Block size in bytes, must be a multiple of 1 MB and not
    /// larger than 256 MB (default: automatically choose a block size
    /// depending on the image size)
    #[qapi(name = "block-size")]
    pub block_size: Option<u64>,
    /// vhdx subformat (default: dynamic)
    #[qapi(name = "subformat")]
    pub subformat: Option<BlockdevVhdxSubformat>,
    /// Force use of payload blocks of type 'ZERO'.
    /// Non-standard, but default.  Do not set to 'off' when using
    /// 'qemu-img convert' with subformat=dynamic.
    #[qapi(name = "block-state-zero")]
    pub block_state_zero: Option<bool>,
}
#[qapi(name = "BlockdevVpcSubformat")]
#[qapi(since = "2.12")]
pub enum BlockdevVpcSubformat {
    /// Growing image file
    #[qapi(name = "dynamic")]
    Dynamic,
    /// Preallocated fixed-size image file
    #[qapi(name = "fixed")]
    Fixed,
}
/// Driver specific image creation options for vpc (VHD).
#[qapi(name = "BlockdevCreateOptionsVpc")]
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsVpc {
    /// Node to create the image format on
    #[qapi(name = "file")]
    pub file: BlockdevRef,
    /// Size of the virtual disk in bytes
    #[qapi(name = "size")]
    pub size: u64,
    /// vhdx subformat (default: dynamic)
    #[qapi(name = "subformat")]
    pub subformat: Option<BlockdevVpcSubformat>,
    /// Force use of the exact byte size instead of rounding to
    /// the next size that can be represented in CHS geometry
    /// (default: false)
    #[qapi(name = "force-size")]
    pub force_size: Option<bool>,
}
pub enum BlockdevCreateOptionsBranch {
    #[qapi(name = "file")]
    File(BlockdevCreateOptionsFile),
    #[qapi(name = "gluster")]
    Gluster(BlockdevCreateOptionsGluster),
    #[qapi(name = "luks")]
    Luks(BlockdevCreateOptionsLuks),
    #[qapi(name = "nfs")]
    Nfs(BlockdevCreateOptionsNfs),
    #[qapi(name = "parallels")]
    Parallels(BlockdevCreateOptionsParallels),
    #[qapi(name = "qcow")]
    Qcow(BlockdevCreateOptionsQcow),
    #[qapi(name = "qcow2")]
    Qcow2(BlockdevCreateOptionsQcow2),
    #[qapi(name = "qed")]
    Qed(BlockdevCreateOptionsQed),
    #[qapi(name = "rbd")]
    Rbd(BlockdevCreateOptionsRbd),
    #[qapi(name = "ssh")]
    Ssh(BlockdevCreateOptionsSsh),
    #[qapi(name = "vdi")]
    Vdi(BlockdevCreateOptionsVdi),
    #[qapi(name = "vhdx")]
    Vhdx(BlockdevCreateOptionsVhdx),
    #[qapi(name = "vmdk")]
    Vmdk(BlockdevCreateOptionsVmdk),
    #[qapi(name = "vpc")]
    Vpc(BlockdevCreateOptionsVpc),
}
/// Options for creating an image format on a given node.
#[qapi(name = "BlockdevCreateOptions")]
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptions {
    /// block driver to create the image format
    #[qapi(name = "driver")]
    #[qapi(discriminator)]
    pub driver: BlockdevDriver,
    #[qapi(union)]
    pub u: Option<BlockdevCreateOptionsBranch>,
}
/// Starts a job to create an image format on a given node.  The job is
/// automatically finalized, but a manual job-dismiss is required.
#[qapi(name = "blockdev-create")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockdevCreate {
    /// Identifier for the newly created job.
    #[qapi(name = "job-id")]
    pub job_id: String,
    /// Options for the image creation.
    #[qapi(name = "options")]
    pub options: BlockdevCreateOptions,
}
/// Driver specific image amend options for LUKS.
#[qapi(name = "BlockdevAmendOptionsLUKS")]
#[qapi(since = "5.1")]
pub struct BlockdevAmendOptionsLuks {
    /// the desired state of the keyslots
    #[qapi(name = "state")]
    pub state: QCryptoBlockLuksKeyslotState,
    /// The ID of a QCryptoSecret object providing the password
    /// to be written into added active keyslots
    #[qapi(name = "new-secret")]
    pub new_secret: Option<String>,
    /// Optional (for deactivation only) If given will
    /// deactivate all keyslots that match password located in
    /// QCryptoSecret with this ID
    #[qapi(name = "old-secret")]
    pub old_secret: Option<String>,
    /// Optional.  ID of the keyslot to activate/deactivate.  For
    /// keyslot activation, keyslot should not be active already (this
    /// is unsafe to update an active keyslot), but possible if 'force'
    /// parameter is given.  If keyslot is not given, first free keyslot
    /// will be written.
    ///
    /// For keyslot deactivation, this parameter specifies the exact
    /// keyslot to deactivate
    #[qapi(name = "keyslot")]
    pub keyslot: Option<i64>,
    /// Optional (for activation only) Number of milliseconds to
    /// spend in PBKDF passphrase processing for the newly activated
    /// keyslot.  Currently defaults to 2000.
    #[qapi(name = "iter-time")]
    pub iter_time: Option<i64>,
    /// Optional.  The ID of a QCryptoSecret object providing the
    /// password to use to retrieve current master key.  Defaults to the
    /// same secret that was used to open the image
    #[qapi(name = "secret")]
    pub secret: Option<String>,
}
/// Driver specific image amend options for qcow2.  For now, only
/// encryption options can be amended
#[qapi(name = "BlockdevAmendOptionsQcow2")]
#[qapi(since = "5.1")]
pub struct BlockdevAmendOptionsQcow2 {
    /// Encryption options to be amended
    #[qapi(name = "encrypt")]
    pub encrypt: Option<QCryptoBlockAmendOptions>,
}
pub enum BlockdevAmendOptionsBranch {
    #[qapi(name = "luks")]
    Luks(BlockdevAmendOptionsLuks),
    #[qapi(name = "qcow2")]
    Qcow2(BlockdevAmendOptionsQcow2),
}
/// Options for amending an image format
#[qapi(name = "BlockdevAmendOptions")]
#[qapi(since = "5.1")]
pub struct BlockdevAmendOptions {
    /// Block driver of the node to amend.
    #[qapi(name = "driver")]
    #[qapi(discriminator)]
    pub driver: BlockdevDriver,
    #[qapi(union)]
    pub u: Option<BlockdevAmendOptionsBranch>,
}
/// Starts a job to amend format specific options of an existing open
/// block device The job is automatically finalized, but a manual
/// job-dismiss is required.
#[qapi(name = "x-blockdev-amend")]
#[qapi(feature = "unstable")]
#[qapi(since = "5.1")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct XBlockdevAmend {
    /// Identifier for the newly created job.
    #[qapi(name = "job-id")]
    pub job_id: String,
    /// Name of the block node to work on
    #[qapi(name = "node-name")]
    pub node_name: String,
    /// Options (driver specific)
    #[qapi(name = "options")]
    pub options: BlockdevAmendOptions,
    /// Allow unsafe operations, format specific For luks that
    /// allows erase of the last active keyslot (permanent loss of
    /// data), and replacement of an active keyslot (possible loss of
    /// data if IO error happens)
    #[qapi(name = "force")]
    pub force: Option<bool>,
}
/// An enumeration of action that has been taken when a DISK I/O occurs
#[qapi(name = "BlockErrorAction")]
#[qapi(since = "2.1")]
pub enum BlockErrorAction {
    /// error has been ignored
    #[qapi(name = "ignore")]
    Ignore,
    /// error has been reported to the device
    #[qapi(name = "report")]
    Report,
    /// error caused VM to be stopped
    #[qapi(name = "stop")]
    Stop,
}
/// Emitted when a disk image is being marked corrupt.  The image can be
/// identified by its device or node name.  The 'device' field is always
/// present for compatibility reasons, but it can be empty ("") if the
/// image does not have a device name associated.
#[qapi(name = "BLOCK_IMAGE_CORRUPTED")]
#[qapi(since = "1.7")]
pub struct BlockImageCorrupted {
    /// device name.  This is always present for compatibility
    /// reasons, but it can be empty ("") if the image does not have a
    /// device name associated.
    #[qapi(name = "device")]
    pub device: String,
    /// node name (Since: 2.4)
    #[qapi(name = "node-name")]
    pub node_name: Option<String>,
    /// informative message for human consumption, such as the kind of
    /// corruption being detected.  It should not be parsed by machine
    /// as it is not guaranteed to be stable
    #[qapi(name = "msg")]
    pub msg: String,
    /// if the corruption resulted from an image access, this is
    /// the host's access offset into the image
    #[qapi(name = "offset")]
    pub offset: Option<i64>,
    /// if the corruption resulted from an image access, this is the
    /// access size
    #[qapi(name = "size")]
    pub size: Option<i64>,
    /// if set, the image is marked corrupt and therefore unusable
    /// after this event and must be repaired (Since 2.2; before, every
    /// BLOCK_IMAGE_CORRUPTED event was fatal)
    #[qapi(name = "fatal")]
    pub fatal: bool,
}
/// Emitted when a disk I/O error occurs
#[qapi(name = "BLOCK_IO_ERROR")]
#[qapi(since = "0.13")]
pub struct BlockIoError {
    /// device name.  This is always present for compatibility
    /// reasons, but it can be empty ("") if the image does not have a
    /// device name associated.
    #[qapi(name = "device")]
    pub device: String,
    /// node name.  Note that errors may be reported for the
    /// root node that is directly attached to a guest device rather
    /// than for the node where the error occurred.  The node name is
    /// not present if the drive is empty.  (Since: 2.8)
    #[qapi(name = "node-name")]
    pub node_name: Option<String>,
    /// I/O operation
    #[qapi(name = "operation")]
    pub operation: IoOperationType,
    /// action that has been taken
    #[qapi(name = "action")]
    pub action: BlockErrorAction,
    /// true if I/O error was caused due to a no-space condition.
    /// This key is only present if query-block's io-status is present,
    /// please see query-block documentation for more information
    /// (since: 2.2)
    #[qapi(name = "nospace")]
    pub nospace: Option<bool>,
    /// human readable string describing the error cause.  (This
    /// field is a debugging aid for humans, it should not be parsed by
    /// applications) (since: 2.2)
    #[qapi(name = "reason")]
    pub reason: String,
}
/// Emitted when a block job has completed
#[qapi(name = "BLOCK_JOB_COMPLETED")]
#[qapi(since = "1.1")]
pub struct BlockJobCompleted {
    /// job type
    #[qapi(name = "type")]
    pub r#type: JobType,
    /// The job identifier.  Originally the device name but other
    /// values are allowed since QEMU 2.7
    #[qapi(name = "device")]
    pub device: String,
    /// maximum progress value
    #[qapi(name = "len")]
    pub len: i64,
    /// current progress value.  On success this is equal to len.
    /// On failure this is less than len
    #[qapi(name = "offset")]
    pub offset: i64,
    /// rate limit, bytes per second
    #[qapi(name = "speed")]
    pub speed: i64,
    /// error message.  Only present on failure.  This field
    /// contains a human-readable error message.  There are no semantics
    /// other than that streaming has failed and clients should not try
    /// to interpret the error string
    #[qapi(name = "error")]
    pub error: Option<String>,
}
/// Emitted when a block job has been cancelled
#[qapi(name = "BLOCK_JOB_CANCELLED")]
#[qapi(since = "1.1")]
pub struct BlockJobCancelled {
    /// job type
    #[qapi(name = "type")]
    pub r#type: JobType,
    /// The job identifier.  Originally the device name but other
    /// values are allowed since QEMU 2.7
    #[qapi(name = "device")]
    pub device: String,
    /// maximum progress value
    #[qapi(name = "len")]
    pub len: i64,
    /// current progress value.  On success this is equal to len.
    /// On failure this is less than len
    #[qapi(name = "offset")]
    pub offset: i64,
    /// rate limit, bytes per second
    #[qapi(name = "speed")]
    pub speed: i64,
}
/// Emitted when a block job encounters an error
#[qapi(name = "BLOCK_JOB_ERROR")]
#[qapi(since = "1.3")]
pub struct BlockJobError {
    /// The job identifier.  Originally the device name but other
    /// values are allowed since QEMU 2.7
    #[qapi(name = "device")]
    pub device: String,
    /// I/O operation
    #[qapi(name = "operation")]
    pub operation: IoOperationType,
    /// action that has been taken
    #[qapi(name = "action")]
    pub action: BlockErrorAction,
}
/// Emitted when a block job is ready to complete
#[qapi(name = "BLOCK_JOB_READY")]
#[qapi(since = "1.3")]
pub struct BlockJobReady {
    /// job type
    #[qapi(name = "type")]
    pub r#type: JobType,
    /// The job identifier.  Originally the device name but other
    /// values are allowed since QEMU 2.7
    #[qapi(name = "device")]
    pub device: String,
    /// maximum progress value
    #[qapi(name = "len")]
    pub len: i64,
    /// current progress value.  On success this is equal to len.
    /// On failure this is less than len
    #[qapi(name = "offset")]
    pub offset: i64,
    /// rate limit, bytes per second
    #[qapi(name = "speed")]
    pub speed: i64,
}
/// Emitted when a block job is awaiting explicit authorization to
/// finalize graph changes via @block-job-finalize.  If this job is part
/// of a transaction, it will not emit this event until the transaction
/// has converged first.
#[qapi(name = "BLOCK_JOB_PENDING")]
#[qapi(since = "2.12")]
pub struct BlockJobPending {
    /// job type
    #[qapi(name = "type")]
    pub r#type: JobType,
    /// The job identifier.
    #[qapi(name = "id")]
    pub id: String,
}
/// Preallocation mode of QEMU image file
#[qapi(name = "PreallocMode")]
#[qapi(since = "2.2")]
pub enum PreallocMode {
    /// no preallocation
    #[qapi(name = "off")]
    Off,
    /// preallocate only for metadata
    #[qapi(name = "metadata")]
    Metadata,
    /// like @full preallocation but allocate disk space by
    /// posix_fallocate() rather than writing data.
    #[qapi(name = "falloc")]
    Falloc,
    /// preallocate all data by writing it to the device to ensure
    /// disk space is really available.  This data may or may not be
    /// zero, depending on the image format and storage.  @full
    /// preallocation also sets up metadata correctly.
    #[qapi(name = "full")]
    Full,
}
/// Emitted when writes on block device reaches or exceeds the
/// configured write threshold.  For thin-provisioned devices, this
/// means the device should be extended to avoid pausing for disk
/// exhaustion.  The event is one shot.  Once triggered, it needs to be
/// re-registered with another block-set-write-threshold command.
#[qapi(name = "BLOCK_WRITE_THRESHOLD")]
#[qapi(since = "2.3")]
pub struct BlockWriteThreshold {
    /// graph node name on which the threshold was exceeded.
    #[qapi(name = "node-name")]
    pub node_name: String,
    /// amount of data which exceeded the threshold, in
    /// bytes.
    #[qapi(name = "amount-exceeded")]
    pub amount_exceeded: u64,
    /// last configured threshold, in bytes.
    #[qapi(name = "write-threshold")]
    pub write_threshold: u64,
}
/// Change the write threshold for a block drive.  An event will be
/// delivered if a write to this block drive crosses the configured
/// threshold.  The threshold is an offset, thus must be non-negative.
/// Default is no write threshold.  Setting the threshold to zero
/// disables it.
///
/// This is useful to transparently resize thin-provisioned drives
/// without the guest OS noticing.
#[qapi(name = "block-set-write-threshold")]
#[qapi(since = "2.3")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockSetWriteThreshold {
    /// graph node name on which the threshold must be set.
    #[qapi(name = "node-name")]
    pub node_name: String,
    /// configured threshold for the block device, bytes.
    /// Use 0 to disable the threshold.
    #[qapi(name = "write-threshold")]
    pub write_threshold: u64,
}
/// Dynamically reconfigure the block driver state graph.  It can be
/// used to add, remove, insert or replace a graph node.  Currently only
/// the Quorum driver implements this feature to add or remove its
/// child.  This is useful to fix a broken quorum child.
///
/// If @node is specified, it will be inserted under @parent.  @child
/// may not be specified in this case.  If both @parent and @child are
/// specified but @node is not, @child will be detached from @parent.
#[qapi(name = "x-blockdev-change")]
#[qapi(feature = "unstable")]
#[qapi(since = "2.7")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct XBlockdevChange {
    /// the id or name of the parent node.
    #[qapi(name = "parent")]
    pub parent: String,
    /// the name of a child under the given parent node.
    #[qapi(name = "child")]
    pub child: Option<String>,
    /// the name of the node that will be added.
    #[qapi(name = "node")]
    pub node: Option<String>,
}
/// Move @node and its children into the @iothread.  If @iothread is
/// null then move @node and its children into the main loop.
///
/// The node must not be attached to a BlockBackend.
#[qapi(name = "x-blockdev-set-iothread")]
#[qapi(feature = "unstable")]
#[qapi(since = "2.12")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct XBlockdevSetIothread {
    /// the name of the block driver node
    #[qapi(name = "node-name")]
    pub node_name: String,
    /// the name of the IOThread object or null for the main loop
    #[qapi(name = "iothread")]
    pub iothread: StrOrNull,
    /// true if the node and its children should be moved when a
    /// BlockBackend is already attached
    #[qapi(name = "force")]
    pub force: Option<bool>,
}
/// An enumeration of the quorum operation types
#[qapi(name = "QuorumOpType")]
#[qapi(since = "2.6")]
pub enum QuorumOpType {
    /// read operation
    #[qapi(name = "read")]
    Read,
    /// write operation
    #[qapi(name = "write")]
    Write,
    /// flush operation
    #[qapi(name = "flush")]
    Flush,
}
/// Emitted by the Quorum block driver if it fails to establish a quorum
#[qapi(name = "QUORUM_FAILURE")]
#[qapi(since = "2.0")]
pub struct QuorumFailure {
    /// device name if defined else node name
    #[qapi(name = "reference")]
    pub reference: String,
    /// number of the first sector of the failed read operation
    #[qapi(name = "sector-num")]
    pub sector_num: i64,
    /// failed read operation sector count
    #[qapi(name = "sectors-count")]
    pub sectors_count: i64,
}
/// Emitted to report a corruption of a Quorum file
#[qapi(name = "QUORUM_REPORT_BAD")]
#[qapi(since = "2.0")]
pub struct QuorumReportBad {
    /// quorum operation type (Since 2.6)
    #[qapi(name = "type")]
    pub r#type: QuorumOpType,
    /// error message.  Only present on failure.  This field
    /// contains a human-readable error message.  There are no semantics
    /// other than that the block layer reported an error and clients
    /// should not try to interpret the error string.
    #[qapi(name = "error")]
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
}
#[qapi(name = "BlockdevSnapshotInternal")]
#[qapi(since = "1.7")]
pub struct BlockdevSnapshotInternal {
    /// the device name or node-name of a root node to generate the
    /// snapshot from
    #[qapi(name = "device")]
    pub device: String,
    /// the name of the internal snapshot to be created
    #[qapi(name = "name")]
    pub name: String,
}
/// Synchronously take an internal snapshot of a block device, when the
/// format of the image used supports it.  If the name is an empty
/// string, or a snapshot with name already exists, the operation will
/// fail.
#[qapi(name = "blockdev-snapshot-internal-sync")]
#[qapi(since = "1.7")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockdevSnapshotInternalSync {
    #[qapi(flatten)]
    pub data: BlockdevSnapshotInternal,
}
/// Synchronously delete an internal snapshot of a block device, when
/// the format of the image used support it.  The snapshot is identified
/// by name or id or both.  One of the name or id is required.  Return
/// SnapshotInfo for the successfully deleted snapshot.
#[qapi(name = "blockdev-snapshot-delete-internal-sync")]
#[qapi(since = "1.7")]
#[qapi(returns = "SnapshotInfo")]
#[qapi(allow_preconfig)]
pub struct BlockdevSnapshotDeleteInternalSync {
    /// the device name or node-name of a root node to delete the
    /// snapshot from
    #[qapi(name = "device")]
    pub device: String,
    /// optional the snapshot's ID to be deleted
    #[qapi(name = "id")]
    pub id: Option<String>,
    /// optional the snapshot's name to be deleted
    #[qapi(name = "name")]
    pub name: Option<String>,
}
/// Not used by QMP; hack to let us use BlockGraphInfoList internally
#[qapi(name = "DummyBlockCoreForceArrays")]
#[qapi(since = "8.0")]
pub struct DummyBlockCoreForceArrays {
    #[qapi(name = "unused-block-graph-info")]
    pub unused_block_graph_info: Vec<BlockGraphInfo>,
}
// path end:	qapi/block-core.json
// path begin:	qapi/block-export.json
/// Keep this type consistent with the nbd-server-start arguments.  The
/// only intended difference is using SocketAddress instead of
/// SocketAddressLegacy.
#[qapi(name = "NbdServerOptions")]
#[qapi(since = "4.2")]
pub struct NbdServerOptions {
    /// Address on which to listen.
    #[qapi(name = "addr")]
    pub addr: SocketAddress,
    /// ID of the TLS credentials object (since 2.6).
    #[qapi(name = "tls-creds")]
    pub tls_creds: Option<String>,
    /// ID of the QAuthZ authorization object used to validate
    /// the client's x509 distinguished name.  This object is is only
    /// resolved at time of use, so can be deleted and recreated on the
    /// fly while the NBD server is active.  If missing, it will default
    /// to denying access (since 4.0).
    #[qapi(name = "tls-authz")]
    pub tls_authz: Option<String>,
    /// The maximum number of connections to allow at the
    /// same time, 0 for unlimited.  Setting this to 1 also stops the
    /// server from advertising multiple client support (since 5.2;
    /// default: 100)
    #[qapi(name = "max-connections")]
    pub max_connections: Option<u32>,
}
/// Start an NBD server listening on the given host and port.  Block
/// devices can then be exported using @nbd-server-add.  The NBD server
/// will present them as named exports; for example, another QEMU
/// instance could refer to them as "nbd:HOST:PORT:exportname=NAME".
///
/// Keep this type consistent with the NbdServerOptions type.  The only
/// intended difference is using SocketAddressLegacy instead of
/// SocketAddress.
#[qapi(name = "nbd-server-start")]
#[qapi(since = "1.3")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct NbdServerStart {
    /// Address on which to listen.
    #[qapi(name = "addr")]
    pub addr: SocketAddressLegacy,
    /// ID of the TLS credentials object (since 2.6).
    #[qapi(name = "tls-creds")]
    pub tls_creds: Option<String>,
    /// ID of the QAuthZ authorization object used to validate
    /// the client's x509 distinguished name.  This object is is only
    /// resolved at time of use, so can be deleted and recreated on the
    /// fly while the NBD server is active.  If missing, it will default
    /// to denying access (since 4.0).
    #[qapi(name = "tls-authz")]
    pub tls_authz: Option<String>,
    /// The maximum number of connections to allow at the
    /// same time, 0 for unlimited.  Setting this to 1 also stops the
    /// server from advertising multiple client support (since 5.2;
    /// default: 100).
    #[qapi(name = "max-connections")]
    pub max_connections: Option<u32>,
}
/// An NBD block export (common options shared between nbd-server-add
/// and the NBD branch of block-export-add).
#[qapi(name = "BlockExportOptionsNbdBase")]
#[qapi(since = "5.0")]
pub struct BlockExportOptionsNbdBase {
    /// Export name.  If unspecified, the @device parameter is used
    /// as the export name.  (Since 2.12)
    #[qapi(name = "name")]
    pub name: Option<String>,
    /// Free-form description of the export, up to 4096 bytes.
    /// (Since 5.0)
    #[qapi(name = "description")]
    pub description: Option<String>,
}
/// An NBD block export (distinct options used in the NBD branch of
/// block-export-add).
#[qapi(name = "BlockExportOptionsNbd")]
#[qapi(since = "5.2")]
pub struct BlockExportOptionsNbd {
    /// Export name.  If unspecified, the @device parameter is used
    /// as the export name.  (Since 2.12)
    #[qapi(name = "name")]
    pub name: Option<String>,
    /// Free-form description of the export, up to 4096 bytes.
    /// (Since 5.0)
    #[qapi(name = "description")]
    pub description: Option<String>,
    /// Also export each of the named dirty bitmaps reachable from
    /// @device, so the NBD client can use NBD_OPT_SET_META_CONTEXT with
    /// the metadata context name "qemu:dirty-bitmap:BITMAP" to inspect
    /// each bitmap.  Since 7.1 bitmap may be specified by node/name
    /// pair.
    #[qapi(name = "bitmaps")]
    pub bitmaps: Option<Vec<BlockDirtyBitmapOrStr>>,
    /// Also export the allocation depth map for @device,
    /// so the NBD client can use NBD_OPT_SET_META_CONTEXT with the
    /// metadata context name "qemu:allocation-depth" to inspect
    /// allocation details.  (since 5.2)
    #[qapi(name = "allocation-depth")]
    pub allocation_depth: Option<bool>,
}
/// A vhost-user-blk block export.
#[qapi(name = "BlockExportOptionsVhostUserBlk")]
#[qapi(since = "5.2")]
pub struct BlockExportOptionsVhostUserBlk {
    /// The vhost-user socket on which to listen.  Both 'unix' and
    /// 'fd' SocketAddress types are supported.  Passed fds must be UNIX
    /// domain sockets.
    #[qapi(name = "addr")]
    pub addr: SocketAddress,
    /// Logical block size in bytes.  Defaults to 512
    /// bytes.
    #[qapi(name = "logical-block-size")]
    pub logical_block_size: Option<u64>,
    /// Number of request virtqueues.  Must be greater than 0.
    /// Defaults to 1.
    #[qapi(name = "num-queues")]
    pub num_queues: Option<u16>,
}
/// Possible allow_other modes for FUSE exports.
#[qapi(name = "FuseExportAllowOther")]
#[qapi(since = "6.1")]
pub enum FuseExportAllowOther {
    /// Do not pass allow_other as a mount option.
    #[qapi(name = "off")]
    Off,
    /// Pass allow_other as a mount option.
    #[qapi(name = "on")]
    On,
    /// Try mounting with allow_other first, and if that fails, retry
    /// without allow_other.
    #[qapi(name = "auto")]
    Auto,
}
/// Options for exporting a block graph node on some (file) mountpoint
/// as a raw image.
#[qapi(name = "BlockExportOptionsFuse")]
#[qapi(condition = "CONFIG_FUSE")]
#[qapi(since = "6.0")]
pub struct BlockExportOptionsFuse {
    /// Path on which to export the block device via FUSE.
    /// This must point to an existing regular file.
    #[qapi(name = "mountpoint")]
    pub mountpoint: String,
    /// Whether writes beyond the EOF should grow the block node
    /// accordingly.  (default: false)
    #[qapi(name = "growable")]
    pub growable: Option<bool>,
    /// If this is off, only qemu's user is allowed access to
    /// this export.  That cannot be changed even with chmod or chown.
    /// Enabling this option will allow other users access to the export
    /// with the FUSE mount option "allow_other".  Note that using
    /// allow_other as a non-root user requires user_allow_other to be
    /// enabled in the global fuse.conf configuration file.  In auto
    /// mode (the default), the FUSE export driver will first attempt to
    /// mount the export with allow_other, and if that fails, try again
    /// without.  (since 6.1; default: auto)
    #[qapi(name = "allow-other")]
    pub allow_other: Option<FuseExportAllowOther>,
}
/// A vduse-blk block export.
#[qapi(name = "BlockExportOptionsVduseBlk")]
#[qapi(since = "7.1")]
pub struct BlockExportOptionsVduseBlk {
    /// the name of VDUSE device (must be unique across the host).
    #[qapi(name = "name")]
    pub name: String,
    /// the number of virtqueues.  Defaults to 1.
    #[qapi(name = "num-queues")]
    pub num_queues: Option<u16>,
    /// the size of virtqueue.  Defaults to 256.
    #[qapi(name = "queue-size")]
    pub queue_size: Option<u16>,
    /// Logical block size in bytes.  Range [512,
    /// PAGE_SIZE] and must be power of 2.  Defaults to 512 bytes.
    #[qapi(name = "logical-block-size")]
    pub logical_block_size: Option<u64>,
    /// the serial number of virtio block device.  Defaults to
    /// empty string.
    #[qapi(name = "serial")]
    pub serial: Option<String>,
}
/// An NBD block export, per legacy nbd-server-add command.
#[qapi(name = "NbdServerAddOptions")]
#[qapi(since = "5.0")]
pub struct NbdServerAddOptions {
    /// Export name.  If unspecified, the @device parameter is used
    /// as the export name.  (Since 2.12)
    #[qapi(name = "name")]
    pub name: Option<String>,
    /// Free-form description of the export, up to 4096 bytes.
    /// (Since 5.0)
    #[qapi(name = "description")]
    pub description: Option<String>,
    /// The device name or node name of the node to be exported
    #[qapi(name = "device")]
    pub device: String,
    /// Whether clients should be able to write to the device via
    /// the NBD connection (default false).
    #[qapi(name = "writable")]
    pub writable: Option<bool>,
    /// Also export a single dirty bitmap reachable from @device,
    /// so the NBD client can use NBD_OPT_SET_META_CONTEXT with the
    /// metadata context name "qemu:dirty-bitmap:BITMAP" to inspect the
    /// bitmap (since 4.0).
    #[qapi(name = "bitmap")]
    pub bitmap: Option<String>,
}
/// Export a block node to QEMU's embedded NBD server.
///
/// The export name will be used as the id for the resulting block
/// export.
#[qapi(name = "nbd-server-add")]
#[qapi(feature = "deprecated")]
#[qapi(since = "1.3")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct NbdServerAdd {
    #[qapi(flatten)]
    pub data: NbdServerAddOptions,
}
/// Mode for removing a block export.
#[qapi(name = "BlockExportRemoveMode")]
#[qapi(since = "2.12")]
pub enum BlockExportRemoveMode {
    /// Remove export if there are no existing connections, fail
    /// otherwise.
    #[qapi(name = "safe")]
    Safe,
    /// Drop all connections immediately and remove export.
    ///
    /// TODO: Potential additional modes to be added in the future:
    ///
    /// - hide: Just hide export from new clients, leave existing
    /// connections as is.  Remove export after all clients are
    /// disconnected.
    ///
    /// - soft: Hide export from new clients, answer with ESHUTDOWN for
    /// all further requests from existing clients.
    #[qapi(name = "hard")]
    Hard,
}
/// Remove NBD export by name.
#[qapi(name = "nbd-server-remove")]
#[qapi(feature = "deprecated")]
#[qapi(since = "2.12")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct NbdServerRemove {
    /// Block export id.
    #[qapi(name = "name")]
    pub name: String,
    /// Mode of command operation.  See @BlockExportRemoveMode
    /// description.  Default is 'safe'.
    #[qapi(name = "mode")]
    pub mode: Option<BlockExportRemoveMode>,
}
/// Stop QEMU's embedded NBD server, and unregister all devices
/// previously added via @nbd-server-add.
#[qapi(name = "nbd-server-stop")]
#[qapi(since = "1.3")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct NbdServerStop {}
/// An enumeration of block export types
#[qapi(name = "BlockExportType")]
#[qapi(since = "4.2")]
pub enum BlockExportType {
    /// NBD export
    #[qapi(name = "nbd")]
    Nbd,
    /// vhost-user-blk export (since 5.2)
    #[qapi(name = "vhost-user-blk")]
    #[qapi(condition = "CONFIG_VHOST_USER_BLK_SERVER")]
    VhostUserBlk,
    /// FUSE export (since: 6.0)
    #[qapi(name = "fuse")]
    #[qapi(condition = "CONFIG_FUSE")]
    Fuse,
    /// vduse-blk export (since 7.1)
    #[qapi(name = "vduse-blk")]
    #[qapi(condition = "CONFIG_VDUSE_BLK_EXPORT")]
    VduseBlk,
}
pub enum BlockExportOptionsBranch {
    #[qapi(name = "nbd")]
    Nbd(BlockExportOptionsNbd),
    #[qapi(name = "vhost-user-blk")]
    #[qapi(condition = "CONFIG_VHOST_USER_BLK_SERVER")]
    VhostUserBlk(BlockExportOptionsVhostUserBlk),
    #[qapi(name = "fuse")]
    #[qapi(condition = "CONFIG_FUSE")]
    Fuse(BlockExportOptionsFuse),
    #[qapi(name = "vduse-blk")]
    #[qapi(condition = "CONFIG_VDUSE_BLK_EXPORT")]
    VduseBlk(BlockExportOptionsVduseBlk),
}
/// Describes a block export, i.e. how single node should be exported on
/// an external interface.
#[qapi(name = "BlockExportOptions")]
#[qapi(since = "4.2")]
pub struct BlockExportOptions {
    /// Block export type
    #[qapi(name = "type")]
    #[qapi(discriminator)]
    pub r#type: BlockExportType,
    /// A unique identifier for the block export (across all export
    /// types)
    #[qapi(name = "id")]
    pub id: String,
    /// True prevents the block node from being moved to
    /// another thread while the export is active.  If true and
    /// @iothread is given, export creation fails if the block node
    /// cannot be moved to the iothread.  The default is false.
    /// (since: 5.2)
    #[qapi(name = "fixed-iothread")]
    pub fixed_iothread: Option<bool>,
    /// The name of the iothread object where the export will
    /// run.  The default is to use the thread currently associated with
    /// the block node.  (since: 5.2)
    #[qapi(name = "iothread")]
    pub iothread: Option<String>,
    /// The node name of the block node to be exported
    /// (since: 5.2)
    #[qapi(name = "node-name")]
    pub node_name: String,
    /// True if clients should be able to write to the export
    /// (default false)
    #[qapi(name = "writable")]
    pub writable: Option<bool>,
    /// If true, caches are flushed after every write request
    /// to the export before completion is signalled.  (since: 5.2;
    /// default: false)
    #[qapi(name = "writethrough")]
    pub writethrough: Option<bool>,
    #[qapi(union)]
    pub u: Option<BlockExportOptionsBranch>,
}
/// Creates a new block export.
#[qapi(name = "block-export-add")]
#[qapi(since = "5.2")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockExportAdd {
    #[qapi(flatten)]
    pub data: BlockExportOptions,
}
/// Request to remove a block export.  This drops the user's reference
/// to the export, but the export may still stay around after this
/// command returns until the shutdown of the export has completed.
#[qapi(name = "block-export-del")]
#[qapi(since = "5.2")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockExportDel {
    /// Block export id.
    #[qapi(name = "id")]
    pub id: String,
    /// Mode of command operation.  See @BlockExportRemoveMode
    /// description.  Default is 'safe'.
    #[qapi(name = "mode")]
    pub mode: Option<BlockExportRemoveMode>,
}
/// Emitted when a block export is removed and its id can be reused.
#[qapi(name = "BLOCK_EXPORT_DELETED")]
#[qapi(since = "5.2")]
pub struct BlockExportDeleted {
    /// Block export id.
    #[qapi(name = "id")]
    pub id: String,
}
/// Information about a single block export.
#[qapi(name = "BlockExportInfo")]
#[qapi(since = "5.2")]
pub struct BlockExportInfo {
    /// The unique identifier for the block export
    #[qapi(name = "id")]
    pub id: String,
    /// The block export type
    #[qapi(name = "type")]
    pub r#type: BlockExportType,
    /// The node name of the block node that is exported
    #[qapi(name = "node-name")]
    pub node_name: String,
    /// True if the export is shutting down (e.g. after a
    /// block-export-del command, but before the shutdown has completed)
    #[qapi(name = "shutting-down")]
    pub shutting_down: bool,
}
#[qapi(name = "query-block-exports")]
#[qapi(since = "5.2")]
#[qapi(returns = "Vec<BlockExportInfo>")]
#[qapi(allow_preconfig)]
pub struct QueryBlockExports {}
// path end:	qapi/block-export.json
// path begin:	qapi/char.json
/// Information about a character device.
#[qapi(name = "ChardevInfo")]
#[qapi(since = "0.14")]
pub struct ChardevInfo {
    /// the label of the character device
    #[qapi(name = "label")]
    pub label: String,
    /// the filename of the character device
    #[qapi(name = "filename")]
    pub filename: String,
    /// shows whether the frontend device attached to this
    /// backend (e.g. with the chardev=... option) is in open or closed
    /// state (since 2.1)
    #[qapi(name = "frontend-open")]
    pub frontend_open: bool,
}
/// Returns information about current character devices.
#[qapi(name = "query-chardev")]
#[qapi(since = "0.14")]
#[qapi(returns = "Vec<ChardevInfo>")]
#[qapi(allow_preconfig)]
pub struct QueryChardev {}
/// Information about a character device backend
#[qapi(name = "ChardevBackendInfo")]
#[qapi(since = "2.0")]
pub struct ChardevBackendInfo {
    /// The backend name
    #[qapi(name = "name")]
    pub name: String,
}
/// Returns information about character device backends.
#[qapi(name = "query-chardev-backends")]
#[qapi(since = "2.0")]
#[qapi(returns = "Vec<ChardevBackendInfo>")]
pub struct QueryChardevBackends {}
/// An enumeration of data format.
#[qapi(name = "DataFormat")]
#[qapi(since = "1.4")]
pub enum DataFormat {
    /// Data is a UTF-8 string (RFC 3629)
    #[qapi(name = "utf8")]
    Utf8,
    /// Data is Base64 encoded binary (RFC 3548)
    #[qapi(name = "base64")]
    Base64,
}
/// Write to a ring buffer character device.
#[qapi(name = "ringbuf-write")]
#[qapi(since = "1.4")]
#[qapi(returns = "()")]
pub struct RingbufWrite {
    /// the ring buffer character device name
    #[qapi(name = "device")]
    pub device: String,
    /// data to write
    #[qapi(name = "data")]
    pub data: String,
    /// data encoding (default 'utf8').
    ///
    /// - base64: data must be base64 encoded text.  Its binary decoding
    /// gets written.
    /// - utf8: data's UTF-8 encoding is written
    /// - data itself is always Unicode regardless of format, like any
    /// other string.
    #[qapi(name = "format")]
    pub format: Option<DataFormat>,
}
/// Read from a ring buffer character device.
#[qapi(name = "ringbuf-read")]
#[qapi(since = "1.4")]
#[qapi(returns = "str")]
pub struct RingbufRead {
    /// the ring buffer character device name
    #[qapi(name = "device")]
    pub device: String,
    /// how many bytes to read at most
    #[qapi(name = "size")]
    pub size: i64,
    /// data encoding (default 'utf8').
    ///
    /// - base64: the data read is returned in base64 encoding.
    /// - utf8: the data read is interpreted as UTF-8.
    /// Bug: can screw up when the buffer contains invalid UTF-8
    /// sequences, NUL characters, after the ring buffer lost data,
    /// and when reading stops because the size limit is reached.
    /// - The return value is always Unicode regardless of format, like
    /// any other string.
    #[qapi(name = "format")]
    pub format: Option<DataFormat>,
}
/// Configuration shared across all chardev backends
#[qapi(name = "ChardevCommon")]
#[qapi(since = "2.6")]
pub struct ChardevCommon {
    /// The name of a logfile to save output
    #[qapi(name = "logfile")]
    pub logfile: Option<String>,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(name = "logappend")]
    pub logappend: Option<bool>,
}
/// Configuration info for file chardevs.
#[qapi(name = "ChardevFile")]
#[qapi(since = "1.4")]
pub struct ChardevFile {
    /// The name of a logfile to save output
    #[qapi(name = "logfile")]
    pub logfile: Option<String>,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(name = "logappend")]
    pub logappend: Option<bool>,
    /// The name of the input file
    #[qapi(name = "in")]
    pub r#in: Option<String>,
    /// The name of the output file
    #[qapi(name = "out")]
    pub out: String,
    /// Open the file in append mode (default false to truncate)
    /// (Since 2.6)
    #[qapi(name = "append")]
    pub append: Option<bool>,
}
/// Configuration info for device and pipe chardevs.
#[qapi(name = "ChardevHostdev")]
#[qapi(since = "1.4")]
pub struct ChardevHostdev {
    /// The name of a logfile to save output
    #[qapi(name = "logfile")]
    pub logfile: Option<String>,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(name = "logappend")]
    pub logappend: Option<bool>,
    /// The name of the special file for the device, i.e.
    /// /dev/ttyS0 on Unix or COM1: on Windows
    #[qapi(name = "device")]
    pub device: String,
}
/// Configuration info for (stream) socket chardevs.
#[qapi(name = "ChardevSocket")]
#[qapi(since = "1.4")]
pub struct ChardevSocket {
    /// The name of a logfile to save output
    #[qapi(name = "logfile")]
    pub logfile: Option<String>,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(name = "logappend")]
    pub logappend: Option<bool>,
    /// socket address to listen on (server=true) or connect to
    /// (server=false)
    #[qapi(name = "addr")]
    pub addr: SocketAddressLegacy,
    /// the ID of the TLS credentials object (since 2.6)
    #[qapi(name = "tls-creds")]
    pub tls_creds: Option<String>,
    /// the ID of the QAuthZ authorization object against which
    /// the client's x509 distinguished name will be validated.  This
    /// object is only resolved at time of use, so can be deleted and
    /// recreated on the fly while the chardev server is active.  If
    /// missing, it will default to denying access (since 4.0)
    #[qapi(name = "tls-authz")]
    pub tls_authz: Option<String>,
    /// create server socket (default: true)
    #[qapi(name = "server")]
    pub server: Option<bool>,
    /// wait for incoming connection on server sockets (default:
    /// false).  Silently ignored with server: false.  This use is
    /// deprecated.
    #[qapi(name = "wait")]
    pub wait: Option<bool>,
    /// set TCP_NODELAY socket option (default: false)
    #[qapi(name = "nodelay")]
    pub nodelay: Option<bool>,
    /// enable telnet protocol on server sockets (default: false)
    #[qapi(name = "telnet")]
    pub telnet: Option<bool>,
    /// enable tn3270 protocol on server sockets (default: false)
    /// (Since: 2.10)
    #[qapi(name = "tn3270")]
    pub tn3270: Option<bool>,
    /// enable websocket protocol on server sockets
    /// (default: false) (Since: 3.1)
    #[qapi(name = "websocket")]
    pub websocket: Option<bool>,
    /// For a client socket, if a socket is disconnected, then
    /// attempt a reconnect after the given number of seconds.  Setting
    /// this to zero disables this function.  (default: 0) (Since: 2.2)
    #[qapi(name = "reconnect")]
    pub reconnect: Option<i64>,
}
/// Configuration info for datagram socket chardevs.
#[qapi(name = "ChardevUdp")]
#[qapi(since = "1.5")]
pub struct ChardevUdp {
    /// The name of a logfile to save output
    #[qapi(name = "logfile")]
    pub logfile: Option<String>,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(name = "logappend")]
    pub logappend: Option<bool>,
    /// remote address
    #[qapi(name = "remote")]
    pub remote: SocketAddressLegacy,
    /// local address
    #[qapi(name = "local")]
    pub local: Option<SocketAddressLegacy>,
}
/// Configuration info for mux chardevs.
#[qapi(name = "ChardevMux")]
#[qapi(since = "1.5")]
pub struct ChardevMux {
    /// The name of a logfile to save output
    #[qapi(name = "logfile")]
    pub logfile: Option<String>,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(name = "logappend")]
    pub logappend: Option<bool>,
    /// name of the base chardev.
    #[qapi(name = "chardev")]
    pub chardev: String,
}
/// Configuration info for stdio chardevs.
#[qapi(name = "ChardevStdio")]
#[qapi(since = "1.5")]
pub struct ChardevStdio {
    /// The name of a logfile to save output
    #[qapi(name = "logfile")]
    pub logfile: Option<String>,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(name = "logappend")]
    pub logappend: Option<bool>,
    /// Allow signals (such as SIGINT triggered by ^C) be delivered
    /// to qemu.  Default: true.
    #[qapi(name = "signal")]
    pub signal: Option<bool>,
}
/// Configuration info for spice vm channel chardevs.
#[qapi(name = "ChardevSpiceChannel")]
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(since = "1.5")]
pub struct ChardevSpiceChannel {
    /// The name of a logfile to save output
    #[qapi(name = "logfile")]
    pub logfile: Option<String>,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(name = "logappend")]
    pub logappend: Option<bool>,
    /// kind of channel (for example vdagent).
    #[qapi(name = "type")]
    pub r#type: String,
}
/// Configuration info for spice port chardevs.
#[qapi(name = "ChardevSpicePort")]
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(since = "1.5")]
pub struct ChardevSpicePort {
    /// The name of a logfile to save output
    #[qapi(name = "logfile")]
    pub logfile: Option<String>,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(name = "logappend")]
    pub logappend: Option<bool>,
    /// name of the channel (see docs/spice-port-fqdn.txt)
    #[qapi(name = "fqdn")]
    pub fqdn: String,
}
/// Configuration info for DBus chardevs.
#[qapi(name = "ChardevDBus")]
#[qapi(condition = "CONFIG_DBUS_DISPLAY")]
#[qapi(since = "7.0")]
pub struct ChardevDBus {
    /// The name of a logfile to save output
    #[qapi(name = "logfile")]
    pub logfile: Option<String>,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(name = "logappend")]
    pub logappend: Option<bool>,
    /// name of the channel (following docs/spice-port-fqdn.txt)
    #[qapi(name = "name")]
    pub name: String,
}
/// Configuration info for virtual console chardevs.
#[qapi(name = "ChardevVC")]
#[qapi(since = "1.5")]
pub struct ChardevVc {
    /// The name of a logfile to save output
    #[qapi(name = "logfile")]
    pub logfile: Option<String>,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(name = "logappend")]
    pub logappend: Option<bool>,
    /// console width, in pixels
    #[qapi(name = "width")]
    pub width: Option<i64>,
    /// console height, in pixels
    #[qapi(name = "height")]
    pub height: Option<i64>,
    /// console width, in chars
    #[qapi(name = "cols")]
    pub cols: Option<i64>,
    /// console height, in chars
    #[qapi(name = "rows")]
    pub rows: Option<i64>,
}
/// Configuration info for ring buffer chardevs.
#[qapi(name = "ChardevRingbuf")]
#[qapi(since = "1.5")]
pub struct ChardevRingbuf {
    /// The name of a logfile to save output
    #[qapi(name = "logfile")]
    pub logfile: Option<String>,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(name = "logappend")]
    pub logappend: Option<bool>,
    /// ring buffer size, must be power of two, default is 65536
    #[qapi(name = "size")]
    pub size: Option<i64>,
}
/// Configuration info for qemu vdagent implementation.
#[qapi(name = "ChardevQemuVDAgent")]
#[qapi(condition = "CONFIG_SPICE_PROTOCOL")]
#[qapi(since = "6.1")]
pub struct ChardevQemuVdAgent {
    /// The name of a logfile to save output
    #[qapi(name = "logfile")]
    pub logfile: Option<String>,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(name = "logappend")]
    pub logappend: Option<bool>,
    /// enable/disable mouse, default is enabled.
    #[qapi(name = "mouse")]
    pub mouse: Option<bool>,
    /// enable/disable clipboard, default is disabled.
    #[qapi(name = "clipboard")]
    pub clipboard: Option<bool>,
}
#[qapi(name = "ChardevBackendKind")]
#[qapi(since = "1.4")]
pub enum ChardevBackendKind {
    #[qapi(name = "file")]
    File,
    #[qapi(name = "serial")]
    #[qapi(condition = "HAVE_CHARDEV_SERIAL")]
    Serial,
    #[qapi(name = "parallel")]
    #[qapi(condition = "HAVE_CHARDEV_PARALLEL")]
    Parallel,
    /// Since 1.5
    #[qapi(name = "pipe")]
    Pipe,
    #[qapi(name = "socket")]
    Socket,
    /// Since 1.5
    #[qapi(name = "udp")]
    Udp,
    #[qapi(name = "pty")]
    Pty,
    #[qapi(name = "null")]
    Null,
    /// Since 1.5
    #[qapi(name = "mux")]
    Mux,
    /// Since 1.5
    #[qapi(name = "msmouse")]
    Msmouse,
    /// Since 2.9
    #[qapi(name = "wctablet")]
    Wctablet,
    /// Since 1.5
    #[qapi(name = "braille")]
    #[qapi(condition = "CONFIG_BRLAPI")]
    Braille,
    /// Since 2.2
    #[qapi(name = "testdev")]
    Testdev,
    /// Since 1.5
    #[qapi(name = "stdio")]
    Stdio,
    /// Since 1.5
    #[qapi(name = "console")]
    #[qapi(condition = "CONFIG_WIN32")]
    Console,
    /// Since 1.5
    #[qapi(name = "spicevmc")]
    #[qapi(condition = "CONFIG_SPICE")]
    Spicevmc,
    /// Since 1.5
    #[qapi(name = "spiceport")]
    #[qapi(condition = "CONFIG_SPICE")]
    Spiceport,
    /// Since 6.1
    #[qapi(name = "qemu-vdagent")]
    #[qapi(condition = "CONFIG_SPICE_PROTOCOL")]
    QemuVdagent,
    /// Since 7.0
    #[qapi(name = "dbus")]
    #[qapi(condition = "CONFIG_DBUS_DISPLAY")]
    Dbus,
    /// v1.5
    #[qapi(name = "vc")]
    Vc,
    /// Since 1.6
    #[qapi(name = "ringbuf")]
    Ringbuf,
    /// Since 1.5
    #[qapi(name = "memory")]
    #[qapi(feature = "deprecated")]
    Memory,
}
#[qapi(name = "ChardevFileWrapper")]
#[qapi(since = "1.4")]
pub struct ChardevFileWrapper {
    /// Configuration info for file chardevs
    #[qapi(name = "data")]
    pub data: ChardevFile,
}
#[qapi(name = "ChardevHostdevWrapper")]
#[qapi(since = "1.4")]
pub struct ChardevHostdevWrapper {
    /// Configuration info for device and pipe chardevs
    #[qapi(name = "data")]
    pub data: ChardevHostdev,
}
#[qapi(name = "ChardevSocketWrapper")]
#[qapi(since = "1.4")]
pub struct ChardevSocketWrapper {
    /// Configuration info for (stream) socket chardevs
    #[qapi(name = "data")]
    pub data: ChardevSocket,
}
#[qapi(name = "ChardevUdpWrapper")]
#[qapi(since = "1.5")]
pub struct ChardevUdpWrapper {
    /// Configuration info for datagram socket chardevs
    #[qapi(name = "data")]
    pub data: ChardevUdp,
}
#[qapi(name = "ChardevCommonWrapper")]
#[qapi(since = "2.6")]
pub struct ChardevCommonWrapper {
    /// Configuration shared across all chardev backends
    #[qapi(name = "data")]
    pub data: ChardevCommon,
}
#[qapi(name = "ChardevMuxWrapper")]
#[qapi(since = "1.5")]
pub struct ChardevMuxWrapper {
    /// Configuration info for mux chardevs
    #[qapi(name = "data")]
    pub data: ChardevMux,
}
#[qapi(name = "ChardevStdioWrapper")]
#[qapi(since = "1.5")]
pub struct ChardevStdioWrapper {
    /// Configuration info for stdio chardevs
    #[qapi(name = "data")]
    pub data: ChardevStdio,
}
#[qapi(name = "ChardevSpiceChannelWrapper")]
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(since = "1.5")]
pub struct ChardevSpiceChannelWrapper {
    /// Configuration info for spice vm channel chardevs
    #[qapi(name = "data")]
    pub data: ChardevSpiceChannel,
}
#[qapi(name = "ChardevSpicePortWrapper")]
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(since = "1.5")]
pub struct ChardevSpicePortWrapper {
    /// Configuration info for spice port chardevs
    #[qapi(name = "data")]
    pub data: ChardevSpicePort,
}
#[qapi(name = "ChardevQemuVDAgentWrapper")]
#[qapi(condition = "CONFIG_SPICE_PROTOCOL")]
#[qapi(since = "6.1")]
pub struct ChardevQemuVdAgentWrapper {
    /// Configuration info for qemu vdagent implementation
    #[qapi(name = "data")]
    pub data: ChardevQemuVdAgent,
}
#[qapi(name = "ChardevDBusWrapper")]
#[qapi(condition = "CONFIG_DBUS_DISPLAY")]
#[qapi(since = "7.0")]
pub struct ChardevDBusWrapper {
    /// Configuration info for DBus chardevs
    #[qapi(name = "data")]
    pub data: ChardevDBus,
}
#[qapi(name = "ChardevVCWrapper")]
#[qapi(since = "1.5")]
pub struct ChardevVcWrapper {
    /// Configuration info for virtual console chardevs
    #[qapi(name = "data")]
    pub data: ChardevVc,
}
#[qapi(name = "ChardevRingbufWrapper")]
#[qapi(since = "1.5")]
pub struct ChardevRingbufWrapper {
    /// Configuration info for ring buffer chardevs
    #[qapi(name = "data")]
    pub data: ChardevRingbuf,
}
pub enum ChardevBackendBranch {
    #[qapi(name = "file")]
    File(ChardevFileWrapper),
    #[qapi(name = "serial")]
    #[qapi(condition = "HAVE_CHARDEV_SERIAL")]
    Serial(ChardevHostdevWrapper),
    #[qapi(name = "parallel")]
    #[qapi(condition = "HAVE_CHARDEV_PARALLEL")]
    Parallel(ChardevHostdevWrapper),
    #[qapi(name = "pipe")]
    Pipe(ChardevHostdevWrapper),
    #[qapi(name = "socket")]
    Socket(ChardevSocketWrapper),
    #[qapi(name = "udp")]
    Udp(ChardevUdpWrapper),
    #[qapi(name = "pty")]
    Pty(ChardevCommonWrapper),
    #[qapi(name = "null")]
    Null(ChardevCommonWrapper),
    #[qapi(name = "mux")]
    Mux(ChardevMuxWrapper),
    #[qapi(name = "msmouse")]
    Msmouse(ChardevCommonWrapper),
    #[qapi(name = "wctablet")]
    Wctablet(ChardevCommonWrapper),
    #[qapi(name = "braille")]
    #[qapi(condition = "CONFIG_BRLAPI")]
    Braille(ChardevCommonWrapper),
    #[qapi(name = "testdev")]
    Testdev(ChardevCommonWrapper),
    #[qapi(name = "stdio")]
    Stdio(ChardevStdioWrapper),
    #[qapi(name = "console")]
    #[qapi(condition = "CONFIG_WIN32")]
    Console(ChardevCommonWrapper),
    #[qapi(name = "spicevmc")]
    #[qapi(condition = "CONFIG_SPICE")]
    Spicevmc(ChardevSpiceChannelWrapper),
    #[qapi(name = "spiceport")]
    #[qapi(condition = "CONFIG_SPICE")]
    Spiceport(ChardevSpicePortWrapper),
    #[qapi(name = "qemu-vdagent")]
    #[qapi(condition = "CONFIG_SPICE_PROTOCOL")]
    QemuVdagent(ChardevQemuVdAgentWrapper),
    #[qapi(name = "dbus")]
    #[qapi(condition = "CONFIG_DBUS_DISPLAY")]
    Dbus(ChardevDBusWrapper),
    #[qapi(name = "vc")]
    Vc(ChardevVcWrapper),
    #[qapi(name = "ringbuf")]
    Ringbuf(ChardevRingbufWrapper),
    #[qapi(name = "memory")]
    Memory(ChardevRingbufWrapper),
}
/// Configuration info for the new chardev backend.
#[qapi(name = "ChardevBackend")]
#[qapi(since = "1.4")]
pub struct ChardevBackend {
    /// backend type
    #[qapi(name = "type")]
    #[qapi(discriminator)]
    pub r#type: ChardevBackendKind,
    #[qapi(union)]
    pub u: Option<ChardevBackendBranch>,
}
/// Return info about the chardev backend just created.
#[qapi(name = "ChardevReturn")]
#[qapi(since = "1.4")]
pub struct ChardevReturn {
    /// name of the slave pseudoterminal device, present if and only
    /// if a chardev of type 'pty' was created
    #[qapi(name = "pty")]
    pub pty: Option<String>,
}
/// Add a character device backend
#[qapi(name = "chardev-add")]
#[qapi(since = "1.4")]
#[qapi(returns = "ChardevReturn")]
pub struct ChardevAdd {
    /// the chardev's ID, must be unique
    #[qapi(name = "id")]
    pub id: String,
    /// backend type and parameters
    #[qapi(name = "backend")]
    pub backend: ChardevBackend,
}
/// Change a character device backend
#[qapi(name = "chardev-change")]
#[qapi(since = "2.10")]
#[qapi(returns = "ChardevReturn")]
pub struct ChardevChange {
    /// the chardev's ID, must exist
    #[qapi(name = "id")]
    pub id: String,
    /// new backend type and parameters
    #[qapi(name = "backend")]
    pub backend: ChardevBackend,
}
/// Remove a character device backend
#[qapi(name = "chardev-remove")]
#[qapi(since = "1.4")]
#[qapi(returns = "()")]
pub struct ChardevRemove {
    /// the chardev's ID, must exist and not be in use
    #[qapi(name = "id")]
    pub id: String,
}
/// Send a break to a character device
#[qapi(name = "chardev-send-break")]
#[qapi(since = "2.10")]
#[qapi(returns = "()")]
pub struct ChardevSendBreak {
    /// the chardev's ID, must exist
    #[qapi(name = "id")]
    pub id: String,
}
/// Emitted when the guest opens or closes a virtio-serial port.
#[qapi(name = "VSERPORT_CHANGE")]
#[qapi(since = "2.1")]
pub struct VserportChange {
    /// device identifier of the virtio-serial port
    #[qapi(name = "id")]
    pub id: String,
    /// true if the guest has opened the virtio-serial port
    #[qapi(name = "open")]
    pub open: bool,
}
// path end:	qapi/char.json
// path begin:	qapi/dump.json
/// An enumeration of guest-memory-dump's format.
#[qapi(name = "DumpGuestMemoryFormat")]
#[qapi(since = "2.0")]
pub enum DumpGuestMemoryFormat {
    /// elf format
    #[qapi(name = "elf")]
    Elf,
    /// makedumpfile flattened, kdump-compressed format with
    /// zlib compression
    #[qapi(name = "kdump-zlib")]
    KdumpZlib,
    /// makedumpfile flattened, kdump-compressed format with lzo
    /// compression
    #[qapi(name = "kdump-lzo")]
    KdumpLzo,
    /// makedumpfile flattened, kdump-compressed format with
    /// snappy compression
    #[qapi(name = "kdump-snappy")]
    KdumpSnappy,
    /// raw assembled kdump-compressed format with zlib
    /// compression (since 8.2)
    #[qapi(name = "kdump-raw-zlib")]
    KdumpRawZlib,
    /// raw assembled kdump-compressed format with lzo
    /// compression (since 8.2)
    #[qapi(name = "kdump-raw-lzo")]
    KdumpRawLzo,
    /// raw assembled kdump-compressed format with snappy
    /// compression (since 8.2)
    #[qapi(name = "kdump-raw-snappy")]
    KdumpRawSnappy,
    /// Windows full crashdump format, can be used instead of ELF
    /// converting (since 2.13)
    #[qapi(name = "win-dmp")]
    WinDmp,
}
/// Dump guest's memory to vmcore.  It is a synchronous operation that
/// can take very long depending on the amount of guest memory.
#[qapi(name = "dump-guest-memory")]
#[qapi(since = "1.2")]
#[qapi(returns = "()")]
pub struct DumpGuestMemory {
    /// if true, do paging to get guest's memory mapping.  This
    /// allows using gdb to process the core file.
    ///
    /// IMPORTANT: this option can make QEMU allocate several gigabytes
    /// of RAM.  This can happen for a large guest, or a malicious guest
    /// pretending to be large.
    ///
    /// Also, paging=true has the following limitations:
    ///
    /// 1. The guest may be in a catastrophic state or can have
    /// corrupted memory, which cannot be trusted
    /// 2. The guest can be in real-mode even if paging is enabled.  For
    /// example, the guest uses ACPI to sleep, and ACPI sleep state
    /// goes in real-mode
    /// 3. Currently only supported on i386 and x86_64.
    #[qapi(name = "paging")]
    pub paging: bool,
    /// the filename or file descriptor of the vmcore.  The
    /// supported protocols are:
    ///
    /// 1. file: the protocol starts with "file:", and the following
    /// string is the file's path.
    /// 2. fd: the protocol starts with "fd:", and the following string
    /// is the fd's name.
    #[qapi(name = "protocol")]
    pub protocol: String,
    /// if true, QMP will return immediately rather than waiting
    /// for the dump to finish.  The user can track progress using
    /// "query-dump".  (since 2.6).
    #[qapi(name = "detach")]
    pub detach: Option<bool>,
    /// if specified, the starting physical address.
    #[qapi(name = "begin")]
    pub begin: Option<i64>,
    /// if specified, the memory size, in bytes.  If you don't want
    /// to dump all guest's memory, please specify the start @begin and
    /// @length
    #[qapi(name = "length")]
    pub length: Option<i64>,
    /// if specified, the format of guest memory dump.  But non-elf
    /// format is conflict with paging and filter, ie.  @paging, @begin
    /// and @length is not allowed to be specified with non-elf @format
    /// at the same time (since 2.0)
    #[qapi(name = "format")]
    pub format: Option<DumpGuestMemoryFormat>,
}
/// Describe the status of a long-running background guest memory dump.
#[qapi(name = "DumpStatus")]
#[qapi(since = "2.6")]
pub enum DumpStatus {
    /// no dump-guest-memory has started yet.
    #[qapi(name = "none")]
    None,
    /// there is one dump running in background.
    #[qapi(name = "active")]
    Active,
    /// the last dump has finished successfully.
    #[qapi(name = "completed")]
    Completed,
    /// the last dump has failed.
    #[qapi(name = "failed")]
    Failed,
}
/// The result format for 'query-dump'.
#[qapi(name = "DumpQueryResult")]
#[qapi(since = "2.6")]
pub struct DumpQueryResult {
    /// enum of @DumpStatus, which shows current dump status
    #[qapi(name = "status")]
    pub status: DumpStatus,
    /// bytes written in latest dump (uncompressed)
    #[qapi(name = "completed")]
    pub completed: i64,
    /// total bytes to be written in latest dump (uncompressed)
    #[qapi(name = "total")]
    pub total: i64,
}
/// Query latest dump status.
#[qapi(name = "query-dump")]
#[qapi(since = "2.6")]
#[qapi(returns = "DumpQueryResult")]
pub struct QueryDump {}
/// Emitted when background dump has completed
#[qapi(name = "DUMP_COMPLETED")]
#[qapi(since = "2.6")]
pub struct DumpCompleted {
    /// final dump status
    #[qapi(name = "result")]
    pub result: DumpQueryResult,
    /// human-readable error string that provides hint on why dump
    /// failed.  Only presents on failure.  The user should not try to
    /// interpret the error string.
    #[qapi(name = "error")]
    pub error: Option<String>,
}
#[qapi(name = "DumpGuestMemoryCapability")]
#[qapi(since = "2.0")]
pub struct DumpGuestMemoryCapability {
    /// the available formats for dump-guest-memory
    #[qapi(name = "formats")]
    pub formats: Vec<DumpGuestMemoryFormat>,
}
/// Returns the available formats for dump-guest-memory
#[qapi(name = "query-dump-guest-memory-capability")]
#[qapi(since = "2.0")]
#[qapi(returns = "DumpGuestMemoryCapability")]
pub struct QueryDumpGuestMemoryCapability {}
// path end:	qapi/dump.json
// path begin:	qapi/net.json
/// Sets the link status of a virtual network adapter.
#[qapi(name = "set_link")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct SetLink {
    /// the device name of the virtual network adapter
    #[qapi(name = "name")]
    pub name: String,
    /// true to set the link status to be up
    #[qapi(name = "up")]
    pub up: bool,
}
/// Add a network backend.
///
/// Additional arguments depend on the type.
#[qapi(name = "netdev_add")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct NetdevAdd {
    #[qapi(flatten)]
    pub data: Netdev,
}
/// Remove a network backend.
#[qapi(name = "netdev_del")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct NetdevDel {
    /// the name of the network backend to remove
    #[qapi(name = "id")]
    pub id: String,
}
/// Create a new Network Interface Card.
#[qapi(name = "NetLegacyNicOptions")]
#[qapi(since = "1.2")]
pub struct NetLegacyNicOptions {
    /// id of -netdev to connect to
    #[qapi(name = "netdev")]
    pub netdev: Option<String>,
    /// MAC address
    #[qapi(name = "macaddr")]
    pub macaddr: Option<String>,
    /// device model (e1000, rtl8139, virtio etc.)
    #[qapi(name = "model")]
    pub model: Option<String>,
    /// PCI device address
    #[qapi(name = "addr")]
    pub addr: Option<String>,
    /// number of MSI-x vectors, 0 to disable MSI-X
    #[qapi(name = "vectors")]
    pub vectors: Option<u32>,
}
/// A fat type wrapping 'str', to be embedded in lists.
#[qapi(name = "String")]
#[qapi(since = "1.2")]
pub struct String {
    #[qapi(name = "str")]
    pub str: String,
}
/// Use the user mode network stack which requires no administrator
/// privilege to run.
#[qapi(name = "NetdevUserOptions")]
#[qapi(since = "1.2")]
pub struct NetdevUserOptions {
    /// client hostname reported by the builtin DHCP server
    #[qapi(name = "hostname")]
    pub hostname: Option<String>,
    /// isolate the guest from the host
    #[qapi(name = "restrict")]
    pub restrict: Option<bool>,
    /// whether to support IPv4, default true for enabled (since 2.6)
    #[qapi(name = "ipv4")]
    pub ipv4: Option<bool>,
    /// whether to support IPv6, default true for enabled (since 2.6)
    #[qapi(name = "ipv6")]
    pub ipv6: Option<bool>,
    /// legacy parameter, use net= instead
    #[qapi(name = "ip")]
    pub ip: Option<String>,
    /// IP network address that the guest will see, in the form
    /// addr[/netmask] The netmask is optional, and can be either in the
    /// form a.b.c.d or as a number of valid top-most bits.  Default is
    /// 10.0.2.0/24.
    #[qapi(name = "net")]
    pub net: Option<String>,
    /// guest-visible address of the host
    #[qapi(name = "host")]
    pub host: Option<String>,
    /// root directory of the built-in TFTP server
    #[qapi(name = "tftp")]
    pub tftp: Option<String>,
    /// BOOTP filename, for use with tftp=
    #[qapi(name = "bootfile")]
    pub bootfile: Option<String>,
    /// the first of the 16 IPs the built-in DHCP server can
    /// assign
    #[qapi(name = "dhcpstart")]
    pub dhcpstart: Option<String>,
    /// guest-visible address of the virtual nameserver
    #[qapi(name = "dns")]
    pub dns: Option<String>,
    /// list of DNS suffixes to search, passed as DHCP option to
    /// the guest
    #[qapi(name = "dnssearch")]
    pub dnssearch: Option<Vec<String>>,
    /// guest-visible domain name of the virtual nameserver
    /// (since 3.0)
    #[qapi(name = "domainname")]
    pub domainname: Option<String>,
    /// IPv6 network prefix (default is fec0::) (since 2.6).
    /// The network prefix is given in the usual hexadecimal IPv6
    /// address notation.
    #[qapi(name = "ipv6-prefix")]
    pub ipv6_prefix: Option<String>,
    /// IPv6 network prefix length (default is 64) (since
    /// 2.6)
    #[qapi(name = "ipv6-prefixlen")]
    pub ipv6_prefixlen: Option<i64>,
    /// guest-visible IPv6 address of the host (since 2.6)
    #[qapi(name = "ipv6-host")]
    pub ipv6_host: Option<String>,
    /// guest-visible IPv6 address of the virtual nameserver
    /// (since 2.6)
    #[qapi(name = "ipv6-dns")]
    pub ipv6_dns: Option<String>,
    /// root directory of the built-in SMB server
    #[qapi(name = "smb")]
    pub smb: Option<String>,
    /// IP address of the built-in SMB server
    #[qapi(name = "smbserver")]
    pub smbserver: Option<String>,
    /// redirect incoming TCP or UDP host connections to guest
    /// endpoints
    #[qapi(name = "hostfwd")]
    pub hostfwd: Option<Vec<String>>,
    /// forward guest TCP connections
    #[qapi(name = "guestfwd")]
    pub guestfwd: Option<Vec<String>>,
    /// RFC2132 "TFTP server name" string (Since 3.1)
    #[qapi(name = "tftp-server-name")]
    pub tftp_server_name: Option<String>,
}
/// Used to configure a host TAP network interface backend.
#[qapi(name = "NetdevTapOptions")]
#[qapi(since = "1.2")]
pub struct NetdevTapOptions {
    /// interface name
    #[qapi(name = "ifname")]
    pub ifname: Option<String>,
    /// file descriptor of an already opened tap
    #[qapi(name = "fd")]
    pub fd: Option<String>,
    /// multiple file descriptors of already opened multiqueue capable
    /// tap
    #[qapi(name = "fds")]
    pub fds: Option<String>,
    /// script to initialize the interface
    #[qapi(name = "script")]
    pub script: Option<String>,
    /// script to shut down the interface
    #[qapi(name = "downscript")]
    pub downscript: Option<String>,
    /// bridge name (since 2.8)
    #[qapi(name = "br")]
    pub br: Option<String>,
    /// command to execute to configure bridge
    #[qapi(name = "helper")]
    pub helper: Option<String>,
    /// send buffer limit.  Understands [TGMKkb] suffixes.
    #[qapi(name = "sndbuf")]
    pub sndbuf: Option<u64>,
    /// enable the IFF_VNET_HDR flag on the tap interface
    #[qapi(name = "vnet_hdr")]
    pub vnet_hdr: Option<bool>,
    /// enable vhost-net network accelerator
    #[qapi(name = "vhost")]
    pub vhost: Option<bool>,
    /// file descriptor of an already opened vhost net device
    #[qapi(name = "vhostfd")]
    pub vhostfd: Option<String>,
    /// file descriptors of multiple already opened vhost net
    /// devices
    #[qapi(name = "vhostfds")]
    pub vhostfds: Option<String>,
    /// vhost on for non-MSIX virtio guests
    #[qapi(name = "vhostforce")]
    pub vhostforce: Option<bool>,
    /// number of queues to be created for multiqueue capable tap
    #[qapi(name = "queues")]
    pub queues: Option<u32>,
    /// maximum number of microseconds that could be spent on busy
    /// polling for tap (since 2.7)
    #[qapi(name = "poll-us")]
    pub poll_us: Option<u32>,
}
/// Socket netdevs are used to establish a network connection to another
/// QEMU virtual machine via a TCP socket.
#[qapi(name = "NetdevSocketOptions")]
#[qapi(since = "1.2")]
pub struct NetdevSocketOptions {
    /// file descriptor of an already opened socket
    #[qapi(name = "fd")]
    pub fd: Option<String>,
    /// port number, and optional hostname, to listen on
    #[qapi(name = "listen")]
    pub listen: Option<String>,
    /// port number, and optional hostname, to connect to
    #[qapi(name = "connect")]
    pub connect: Option<String>,
    /// UDP multicast address and port number
    #[qapi(name = "mcast")]
    pub mcast: Option<String>,
    /// source address and port for multicast and udp packets
    #[qapi(name = "localaddr")]
    pub localaddr: Option<String>,
    /// UDP unicast address and port number
    #[qapi(name = "udp")]
    pub udp: Option<String>,
}
/// Configure an Ethernet over L2TPv3 tunnel.
#[qapi(name = "NetdevL2TPv3Options")]
#[qapi(since = "2.1")]
pub struct NetdevL2tPv3Options {
    /// source address
    #[qapi(name = "src")]
    pub src: String,
    /// destination address
    #[qapi(name = "dst")]
    pub dst: String,
    /// source port - mandatory for udp, optional for ip
    #[qapi(name = "srcport")]
    pub srcport: Option<String>,
    /// destination port - mandatory for udp, optional for ip
    #[qapi(name = "dstport")]
    pub dstport: Option<String>,
    /// force the use of ipv6
    #[qapi(name = "ipv6")]
    pub ipv6: Option<bool>,
    /// use the udp version of l2tpv3 encapsulation
    #[qapi(name = "udp")]
    pub udp: Option<bool>,
    /// use 64 bit cookies
    #[qapi(name = "cookie64")]
    pub cookie64: Option<bool>,
    /// have sequence counter
    #[qapi(name = "counter")]
    pub counter: Option<bool>,
    /// pin sequence counter to zero - workaround for buggy
    /// implementations or networks with packet reorder
    #[qapi(name = "pincounter")]
    pub pincounter: Option<bool>,
    /// 32 or 64 bit transmit cookie
    #[qapi(name = "txcookie")]
    pub txcookie: Option<u64>,
    /// 32 or 64 bit receive cookie
    #[qapi(name = "rxcookie")]
    pub rxcookie: Option<u64>,
    /// 32 bit transmit session
    #[qapi(name = "txsession")]
    pub txsession: u32,
    /// 32 bit receive session - if not specified set to the
    /// same value as transmit
    #[qapi(name = "rxsession")]
    pub rxsession: Option<u32>,
    /// additional offset - allows the insertion of additional
    /// application-specific data before the packet payload
    #[qapi(name = "offset")]
    pub offset: Option<u32>,
}
/// Connect to a vde switch running on the host.
#[qapi(name = "NetdevVdeOptions")]
#[qapi(since = "1.2")]
pub struct NetdevVdeOptions {
    /// socket path
    #[qapi(name = "sock")]
    pub sock: Option<String>,
    /// port number
    #[qapi(name = "port")]
    pub port: Option<u16>,
    /// group owner of socket
    #[qapi(name = "group")]
    pub group: Option<String>,
    /// permissions for socket
    #[qapi(name = "mode")]
    pub mode: Option<u16>,
}
/// Connect a host TAP network interface to a host bridge device.
#[qapi(name = "NetdevBridgeOptions")]
#[qapi(since = "1.2")]
pub struct NetdevBridgeOptions {
    /// bridge name
    #[qapi(name = "br")]
    pub br: Option<String>,
    /// command to execute to configure bridge
    #[qapi(name = "helper")]
    pub helper: Option<String>,
}
/// Connect two or more net clients through a software hub.
#[qapi(name = "NetdevHubPortOptions")]
#[qapi(since = "1.2")]
pub struct NetdevHubPortOptions {
    /// hub identifier number
    #[qapi(name = "hubid")]
    pub hubid: i32,
    /// used to connect hub to a netdev instead of a device (since
    /// 2.12)
    #[qapi(name = "netdev")]
    pub netdev: Option<String>,
}
/// Connect a client to a netmap-enabled NIC or to a VALE switch port
#[qapi(name = "NetdevNetmapOptions")]
#[qapi(since = "2.0")]
pub struct NetdevNetmapOptions {
    /// Either the name of an existing network interface supported
    /// by netmap, or the name of a VALE port (created on the fly).  A
    /// VALE port name is in the form 'valeXXX:YYY', where XXX and YYY
    /// are non-negative integers.  XXX identifies a switch and YYY
    /// identifies a port of the switch.  VALE ports having the same XXX
    /// are therefore connected to the same switch.
    #[qapi(name = "ifname")]
    pub ifname: String,
    /// path of the netmap device (default: '/dev/netmap').
    #[qapi(name = "devname")]
    pub devname: Option<String>,
}
/// Attach mode for a default XDP program
#[qapi(name = "AFXDPMode")]
#[qapi(condition = "CONFIG_AF_XDP")]
#[qapi(since = "8.2")]
pub enum AfxdpMode {
    /// DRV mode, program is attached to a driver, packets are
    /// passed to the socket without allocation of skb.
    #[qapi(name = "native")]
    Native,
    /// generic mode, no driver support necessary
    #[qapi(name = "skb")]
    Skb,
}
/// AF_XDP network backend
#[qapi(name = "NetdevAFXDPOptions")]
#[qapi(condition = "CONFIG_AF_XDP")]
#[qapi(since = "8.2")]
pub struct NetdevAfxdpOptions {
    /// The name of an existing network interface.
    #[qapi(name = "ifname")]
    pub ifname: String,
    /// Attach mode for a default XDP program.  If not specified,
    /// then 'native' will be tried first, then 'skb'.
    #[qapi(name = "mode")]
    pub mode: Option<AfxdpMode>,
    /// Force XDP copy mode even if device supports zero-copy.
    /// (default: false)
    #[qapi(name = "force-copy")]
    pub force_copy: Option<bool>,
    /// number of queues to be used for multiqueue interfaces
    /// (default: 1).
    #[qapi(name = "queues")]
    pub queues: Option<i64>,
    /// Use @queues starting from this queue number
    /// (default: 0).
    #[qapi(name = "start-queue")]
    pub start_queue: Option<i64>,
    /// Don't load a default XDP program, use one already loaded
    /// to the interface (default: false).  Requires @sock-fds.
    #[qapi(name = "inhibit")]
    pub inhibit: Option<bool>,
    /// A colon (:) separated list of file descriptors for
    /// already open but not bound AF_XDP sockets in the queue order.
    /// One fd per queue.  These descriptors should already be added
    /// into XDP socket map for corresponding queues.  Requires
    /// @inhibit.
    #[qapi(name = "sock-fds")]
    pub sock_fds: Option<String>,
}
/// Vhost-user network backend
#[qapi(name = "NetdevVhostUserOptions")]
#[qapi(since = "2.1")]
pub struct NetdevVhostUserOptions {
    /// name of a unix socket chardev
    #[qapi(name = "chardev")]
    pub chardev: String,
    /// vhost on for non-MSIX virtio guests (default: false).
    #[qapi(name = "vhostforce")]
    pub vhostforce: Option<bool>,
    /// number of queues to be created for multiqueue vhost-user
    /// (default: 1) (Since 2.5)
    #[qapi(name = "queues")]
    pub queues: Option<i64>,
}
/// Vhost-vdpa network backend
///
/// vDPA device is a device that uses a datapath which complies with the
/// virtio specifications with a vendor specific control path.
#[qapi(name = "NetdevVhostVDPAOptions")]
#[qapi(since = "5.1")]
pub struct NetdevVhostVdpaOptions {
    /// path of vhost-vdpa device (default:'/dev/vhost-vdpa-0')
    #[qapi(name = "vhostdev")]
    pub vhostdev: Option<String>,
    /// file descriptor of an already opened vhost vdpa device
    #[qapi(name = "vhostfd")]
    pub vhostfd: Option<String>,
    /// number of queues to be created for multiqueue vhost-vdpa
    /// (default: 1)
    #[qapi(name = "queues")]
    pub queues: Option<i64>,
    /// Start device with (experimental) shadow virtqueue.  (Since
    /// 7.1) (default: false)
    #[qapi(name = "x-svq")]
    #[qapi(feature = "unstable")]
    pub x_svq: Option<bool>,
}
/// vmnet (host mode) network backend.
///
/// Allows the vmnet interface to communicate with other vmnet
/// interfaces that are in host mode and also with the host.
#[qapi(name = "NetdevVmnetHostOptions")]
#[qapi(condition = "CONFIG_VMNET")]
#[qapi(since = "7.1")]
pub struct NetdevVmnetHostOptions {
    /// The starting IPv4 address to use for the interface.
    /// Must be in the private IP range (RFC 1918).  Must be specified
    /// along with @end-address and @subnet-mask.  This address is used
    /// as the gateway address.  The subsequent address up to and
    /// including end-address are placed in the DHCP pool.
    #[qapi(name = "start-address")]
    pub start_address: Option<String>,
    /// The DHCP IPv4 range end address to use for the
    /// interface.  Must be in the private IP range (RFC 1918).  Must be
    /// specified along with @start-address and @subnet-mask.
    #[qapi(name = "end-address")]
    pub end_address: Option<String>,
    /// The IPv4 subnet mask to use on the interface.  Must be
    /// specified along with @start-address and @subnet-mask.
    #[qapi(name = "subnet-mask")]
    pub subnet_mask: Option<String>,
    /// Enable isolation for this interface.  Interface isolation
    /// ensures that vmnet interface is not able to communicate with any
    /// other vmnet interfaces.  Only communication with host is
    /// allowed.  Requires at least macOS Big Sur 11.0.
    #[qapi(name = "isolated")]
    pub isolated: Option<bool>,
    /// The identifier (UUID) to uniquely identify the isolated
    /// network vmnet interface should be added to.  If set, no DHCP
    /// service is provided for this interface and network communication
    /// is allowed only with other interfaces added to this network
    /// identified by the UUID.  Requires at least macOS Big Sur 11.0.
    #[qapi(name = "net-uuid")]
    pub net_uuid: Option<String>,
}
/// vmnet (shared mode) network backend.
///
/// Allows traffic originating from the vmnet interface to reach the
/// Internet through a network address translator (NAT).  The vmnet
/// interface can communicate with the host and with other shared mode
/// interfaces on the same subnet.  If no DHCP settings, subnet mask and
/// IPv6 prefix specified, the interface can communicate with any of
/// other interfaces in shared mode.
#[qapi(name = "NetdevVmnetSharedOptions")]
#[qapi(condition = "CONFIG_VMNET")]
#[qapi(since = "7.1")]
pub struct NetdevVmnetSharedOptions {
    /// The starting IPv4 address to use for the interface.
    /// Must be in the private IP range (RFC 1918).  Must be specified
    /// along with @end-address and @subnet-mask.  This address is used
    /// as the gateway address.  The subsequent address up to and
    /// including end-address are placed in the DHCP pool.
    #[qapi(name = "start-address")]
    pub start_address: Option<String>,
    /// The DHCP IPv4 range end address to use for the
    /// interface.  Must be in the private IP range (RFC 1918).  Must be
    /// specified along with @start-address and @subnet-mask.
    #[qapi(name = "end-address")]
    pub end_address: Option<String>,
    /// The IPv4 subnet mask to use on the interface.  Must be
    /// specified along with @start-address and @subnet-mask.
    #[qapi(name = "subnet-mask")]
    pub subnet_mask: Option<String>,
    /// Enable isolation for this interface.  Interface isolation
    /// ensures that vmnet interface is not able to communicate with any
    /// other vmnet interfaces.  Only communication with host is
    /// allowed.  Requires at least macOS Big Sur 11.0.
    #[qapi(name = "isolated")]
    pub isolated: Option<bool>,
    /// The IPv6 prefix to use into guest network.  Must be a
    /// unique local address i.e. start with fd00::/8 and have length of
    /// 64.
    #[qapi(name = "nat66-prefix")]
    pub nat66_prefix: Option<String>,
}
/// vmnet (bridged mode) network backend.
///
/// Bridges the vmnet interface with a physical network interface.
#[qapi(name = "NetdevVmnetBridgedOptions")]
#[qapi(condition = "CONFIG_VMNET")]
#[qapi(since = "7.1")]
pub struct NetdevVmnetBridgedOptions {
    /// The name of the physical interface to be bridged.
    #[qapi(name = "ifname")]
    pub ifname: String,
    /// Enable isolation for this interface.  Interface isolation
    /// ensures that vmnet interface is not able to communicate with any
    /// other vmnet interfaces.  Only communication with host is
    /// allowed.  Requires at least macOS Big Sur 11.0.
    #[qapi(name = "isolated")]
    pub isolated: Option<bool>,
}
/// Configuration info for stream socket netdev
#[qapi(name = "NetdevStreamOptions")]
#[qapi(since = "7.2")]
pub struct NetdevStreamOptions {
    /// socket address to listen on (server=true) or connect to
    /// (server=false)
    #[qapi(name = "addr")]
    pub addr: SocketAddress,
    /// create server socket (default: false)
    #[qapi(name = "server")]
    pub server: Option<bool>,
    /// For a client socket, if a socket is disconnected, then
    /// attempt a reconnect after the given number of seconds.  Setting
    /// this to zero disables this function.  (default: 0) (since 8.0)
    ///
    /// Only SocketAddress types 'unix', 'inet' and 'fd' are supported.
    #[qapi(name = "reconnect")]
    pub reconnect: Option<u32>,
}
/// Configuration info for datagram socket netdev.
#[qapi(name = "NetdevDgramOptions")]
#[qapi(since = "7.2")]
pub struct NetdevDgramOptions {
    /// local address
    ///
    /// Only SocketAddress types 'unix', 'inet' and 'fd' are supported.
    ///
    /// If remote address is present and it's a multicast address, local
    /// address is optional.  Otherwise local address is required and remote
    /// address is optional.
    #[qapi(name = "local")]
    pub local: Option<SocketAddress>,
    /// remote address
    #[qapi(name = "remote")]
    pub remote: Option<SocketAddress>,
}
/// Available netdev drivers.
#[qapi(name = "NetClientDriver")]
#[qapi(since = "2.7")]
pub enum NetClientDriver {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "nic")]
    Nic,
    #[qapi(name = "user")]
    User,
    #[qapi(name = "tap")]
    Tap,
    /// since 2.1
    #[qapi(name = "l2tpv3")]
    L2tpv3,
    #[qapi(name = "socket")]
    Socket,
    /// since 7.2
    #[qapi(name = "stream")]
    Stream,
    /// since 7.2
    #[qapi(name = "dgram")]
    Dgram,
    #[qapi(name = "vde")]
    Vde,
    #[qapi(name = "bridge")]
    Bridge,
    #[qapi(name = "hubport")]
    Hubport,
    #[qapi(name = "netmap")]
    Netmap,
    #[qapi(name = "vhost-user")]
    VhostUser,
    /// since 5.1
    #[qapi(name = "vhost-vdpa")]
    VhostVdpa,
    /// since 8.2
    #[qapi(name = "af-xdp")]
    #[qapi(condition = "CONFIG_AF_XDP")]
    AfXdp,
    /// since 7.1
    #[qapi(name = "vmnet-host")]
    #[qapi(condition = "CONFIG_VMNET")]
    VmnetHost,
    /// since 7.1
    #[qapi(name = "vmnet-shared")]
    #[qapi(condition = "CONFIG_VMNET")]
    VmnetShared,
    /// since 7.1
    #[qapi(name = "vmnet-bridged")]
    #[qapi(condition = "CONFIG_VMNET")]
    VmnetBridged,
}
pub enum NetdevBranch {
    #[qapi(name = "nic")]
    Nic(NetLegacyNicOptions),
    #[qapi(name = "user")]
    User(NetdevUserOptions),
    #[qapi(name = "tap")]
    Tap(NetdevTapOptions),
    #[qapi(name = "l2tpv3")]
    L2tpv3(NetdevL2tPv3Options),
    #[qapi(name = "socket")]
    Socket(NetdevSocketOptions),
    #[qapi(name = "stream")]
    Stream(NetdevStreamOptions),
    #[qapi(name = "dgram")]
    Dgram(NetdevDgramOptions),
    #[qapi(name = "vde")]
    Vde(NetdevVdeOptions),
    #[qapi(name = "bridge")]
    Bridge(NetdevBridgeOptions),
    #[qapi(name = "hubport")]
    Hubport(NetdevHubPortOptions),
    #[qapi(name = "netmap")]
    Netmap(NetdevNetmapOptions),
    #[qapi(name = "af-xdp")]
    #[qapi(condition = "CONFIG_AF_XDP")]
    AfXdp(NetdevAfxdpOptions),
    #[qapi(name = "vhost-user")]
    VhostUser(NetdevVhostUserOptions),
    #[qapi(name = "vhost-vdpa")]
    VhostVdpa(NetdevVhostVdpaOptions),
    #[qapi(name = "vmnet-host")]
    #[qapi(condition = "CONFIG_VMNET")]
    VmnetHost(NetdevVmnetHostOptions),
    #[qapi(name = "vmnet-shared")]
    #[qapi(condition = "CONFIG_VMNET")]
    VmnetShared(NetdevVmnetSharedOptions),
    #[qapi(name = "vmnet-bridged")]
    #[qapi(condition = "CONFIG_VMNET")]
    VmnetBridged(NetdevVmnetBridgedOptions),
}
/// Captures the configuration of a network device.
#[qapi(name = "Netdev")]
#[qapi(since = "1.2")]
pub struct Netdev {
    /// identifier for monitor commands.
    #[qapi(name = "id")]
    pub id: String,
    /// Specify the driver used for interpreting remaining arguments.
    #[qapi(name = "type")]
    #[qapi(discriminator)]
    pub r#type: NetClientDriver,
    #[qapi(union)]
    pub u: Option<NetdevBranch>,
}
/// Packets receiving state
#[qapi(name = "RxState")]
#[qapi(since = "1.6")]
pub enum RxState {
    /// filter assigned packets according to the mac-table
    #[qapi(name = "normal")]
    Normal,
    /// don't receive any assigned packet
    #[qapi(name = "none")]
    None,
    /// receive all assigned packets
    #[qapi(name = "all")]
    All,
}
/// Rx-filter information for a NIC.
#[qapi(name = "RxFilterInfo")]
#[qapi(since = "1.6")]
pub struct RxFilterInfo {
    /// net client name
    #[qapi(name = "name")]
    pub name: String,
    /// whether promiscuous mode is enabled
    #[qapi(name = "promiscuous")]
    pub promiscuous: bool,
    /// multicast receive state
    #[qapi(name = "multicast")]
    pub multicast: RxState,
    /// unicast receive state
    #[qapi(name = "unicast")]
    pub unicast: RxState,
    /// vlan receive state (Since 2.0)
    #[qapi(name = "vlan")]
    pub vlan: RxState,
    /// whether to receive broadcast
    #[qapi(name = "broadcast-allowed")]
    pub broadcast_allowed: bool,
    /// multicast table is overflowed or not
    #[qapi(name = "multicast-overflow")]
    pub multicast_overflow: bool,
    /// unicast table is overflowed or not
    #[qapi(name = "unicast-overflow")]
    pub unicast_overflow: bool,
    /// the main macaddr string
    #[qapi(name = "main-mac")]
    pub main_mac: String,
    /// a list of active vlan id
    #[qapi(name = "vlan-table")]
    pub vlan_table: Vec<i64>,
    /// a list of unicast macaddr string
    #[qapi(name = "unicast-table")]
    pub unicast_table: Vec<String>,
    /// a list of multicast macaddr string
    #[qapi(name = "multicast-table")]
    pub multicast_table: Vec<String>,
}
/// Return rx-filter information for all NICs (or for the given NIC).
#[qapi(name = "query-rx-filter")]
#[qapi(since = "1.6")]
#[qapi(returns = "Vec<RxFilterInfo>")]
pub struct QueryRxFilter {
    /// net client name
    #[qapi(name = "name")]
    pub name: Option<String>,
}
/// Emitted once until the 'query-rx-filter' command is executed, the
/// first event will always be emitted
#[qapi(name = "NIC_RX_FILTER_CHANGED")]
#[qapi(since = "1.6")]
pub struct NicRxFilterChanged {
    /// net client name
    #[qapi(name = "name")]
    pub name: Option<String>,
    /// device path
    #[qapi(name = "path")]
    pub path: String,
}
/// Parameters for self-announce timers
#[qapi(name = "AnnounceParameters")]
#[qapi(since = "4.0")]
pub struct AnnounceParameters {
    /// Initial delay (in ms) before sending the first GARP/RARP
    /// announcement
    #[qapi(name = "initial")]
    pub initial: i64,
    /// Maximum delay (in ms) between GARP/RARP announcement packets
    #[qapi(name = "max")]
    pub max: i64,
    /// Number of self-announcement attempts
    #[qapi(name = "rounds")]
    pub rounds: i64,
    /// Delay increase (in ms) after each self-announcement attempt
    #[qapi(name = "step")]
    pub step: i64,
    /// An optional list of interface names, which restricts
    /// the announcement to the listed interfaces.  (Since 4.1)
    #[qapi(name = "interfaces")]
    pub interfaces: Option<Vec<String>>,
    /// A name to be used to identify an instance of announce-timers
    /// and to allow it to modified later.  Not for use as part of the
    /// migration parameters.  (Since 4.1)
    #[qapi(name = "id")]
    pub id: Option<String>,
}
/// Trigger generation of broadcast RARP frames to update network
/// switches.  This can be useful when network bonds fail-over the
/// active slave.
///
/// TODO: This line is a hack to separate the example from the body
#[qapi(name = "announce-self")]
#[qapi(since = "4.0")]
#[qapi(returns = "()")]
pub struct AnnounceSelf {
    #[qapi(flatten)]
    pub data: AnnounceParameters,
}
/// Emitted when VIRTIO_NET_F_STANDBY was enabled during feature
/// negotiation.  Failover primary devices which were hidden (not
/// hotplugged when requested) before will now be hotplugged by the
/// virtio-net standby device.
#[qapi(name = "FAILOVER_NEGOTIATED")]
#[qapi(since = "4.2")]
pub struct FailoverNegotiated {
    /// QEMU device id of the unplugged device
    #[qapi(name = "device-id")]
    pub device_id: String,
}
/// Emitted when the netdev stream backend is connected
#[qapi(name = "NETDEV_STREAM_CONNECTED")]
#[qapi(since = "7.2")]
pub struct NetdevStreamConnected {
    /// QEMU netdev id that is connected
    #[qapi(name = "netdev-id")]
    pub netdev_id: String,
    /// The destination address
    #[qapi(name = "addr")]
    pub addr: SocketAddress,
}
/// Emitted when the netdev stream backend is disconnected
#[qapi(name = "NETDEV_STREAM_DISCONNECTED")]
#[qapi(since = "7.2")]
pub struct NetdevStreamDisconnected {
    /// QEMU netdev id that is disconnected
    #[qapi(name = "netdev-id")]
    pub netdev_id: String,
}
// path end:	qapi/net.json
// path begin:	qapi/ebpf.json
/// An eBPF ELF object.
#[qapi(name = "EbpfObject")]
#[qapi(condition = "CONFIG_EBPF")]
#[qapi(since = "9.0")]
pub struct EbpfObject {
    /// the eBPF object encoded in base64
    #[qapi(name = "object")]
    pub object: String,
}
/// The eBPF programs that can be gotten with request-ebpf.
#[qapi(name = "EbpfProgramID")]
#[qapi(condition = "CONFIG_EBPF")]
#[qapi(since = "9.0")]
pub enum EbpfProgramId {
    /// Receive side scaling, technology that allows steering traffic
    /// between queues by calculation hash.  Users may set up
    /// indirection table and hash/packet types configurations.  Used
    /// with virtio-net.
    #[qapi(name = "rss")]
    Rss,
}
/// Retrieve an eBPF object that can be loaded with libbpf.  Management
/// applications (e.g. libvirt) may load it and pass file descriptors to
/// QEMU, so they can run running QEMU without BPF capabilities.
#[qapi(name = "request-ebpf")]
#[qapi(condition = "CONFIG_EBPF")]
#[qapi(since = "9.0")]
#[qapi(returns = "EbpfObject")]
pub struct RequestEbpf {
    /// The ID of the program to return.
    #[qapi(name = "id")]
    pub id: EbpfProgramId,
}
// path end:	qapi/ebpf.json
// path begin:	qapi/rocker.json
/// Rocker switch information.
#[qapi(name = "RockerSwitch")]
#[qapi(since = "2.4")]
pub struct RockerSwitch {
    /// switch name
    #[qapi(name = "name")]
    pub name: String,
    /// switch ID
    #[qapi(name = "id")]
    pub id: u64,
    /// number of front-panel ports
    #[qapi(name = "ports")]
    pub ports: u32,
}
/// Return rocker switch information.
#[qapi(name = "query-rocker")]
#[qapi(since = "2.4")]
#[qapi(returns = "RockerSwitch")]
pub struct QueryRocker {
    #[qapi(name = "name")]
    pub name: String,
}
/// An enumeration of port duplex states.
#[qapi(name = "RockerPortDuplex")]
#[qapi(since = "2.4")]
pub enum RockerPortDuplex {
    /// half duplex
    #[qapi(name = "half")]
    Half,
    /// full duplex
    #[qapi(name = "full")]
    Full,
}
/// An enumeration of port autoneg states.
#[qapi(name = "RockerPortAutoneg")]
#[qapi(since = "2.4")]
pub enum RockerPortAutoneg {
    /// autoneg is off
    #[qapi(name = "off")]
    Off,
    /// autoneg is on
    #[qapi(name = "on")]
    On,
}
/// Rocker switch port information.
#[qapi(name = "RockerPort")]
#[qapi(since = "2.4")]
pub struct RockerPort {
    /// port name
    #[qapi(name = "name")]
    pub name: String,
    /// port is enabled for I/O
    #[qapi(name = "enabled")]
    pub enabled: bool,
    /// physical link is UP on port
    #[qapi(name = "link-up")]
    pub link_up: bool,
    /// port link speed in Mbps
    #[qapi(name = "speed")]
    pub speed: u32,
    /// port link duplex
    #[qapi(name = "duplex")]
    pub duplex: RockerPortDuplex,
    /// port link autoneg
    #[qapi(name = "autoneg")]
    pub autoneg: RockerPortAutoneg,
}
/// Return rocker switch port information.
#[qapi(name = "query-rocker-ports")]
#[qapi(since = "2.4")]
#[qapi(returns = "Vec<RockerPort>")]
pub struct QueryRockerPorts {
    #[qapi(name = "name")]
    pub name: String,
}
/// Rocker switch OF-DPA flow key
#[qapi(name = "RockerOfDpaFlowKey")]
#[qapi(since = "2.4")]
pub struct RockerOfDpaFlowKey {
    /// key priority, 0 being lowest priority
    #[qapi(name = "priority")]
    pub priority: u32,
    /// flow table ID
    #[qapi(name = "tbl-id")]
    pub tbl_id: u32,
    /// physical input port
    #[qapi(name = "in-pport")]
    pub in_pport: Option<u32>,
    /// tunnel ID
    #[qapi(name = "tunnel-id")]
    pub tunnel_id: Option<u32>,
    /// VLAN ID
    #[qapi(name = "vlan-id")]
    pub vlan_id: Option<u16>,
    /// Ethernet header type
    #[qapi(name = "eth-type")]
    pub eth_type: Option<u16>,
    /// Ethernet header source MAC address
    #[qapi(name = "eth-src")]
    pub eth_src: Option<String>,
    /// Ethernet header destination MAC address
    #[qapi(name = "eth-dst")]
    pub eth_dst: Option<String>,
    /// IP Header protocol field
    #[qapi(name = "ip-proto")]
    pub ip_proto: Option<u8>,
    /// IP header TOS field
    #[qapi(name = "ip-tos")]
    pub ip_tos: Option<u8>,
    /// IP header destination address
    #[qapi(name = "ip-dst")]
    pub ip_dst: Option<String>,
}
/// Rocker switch OF-DPA flow mask
#[qapi(name = "RockerOfDpaFlowMask")]
#[qapi(since = "2.4")]
pub struct RockerOfDpaFlowMask {
    /// physical input port
    #[qapi(name = "in-pport")]
    pub in_pport: Option<u32>,
    /// tunnel ID
    #[qapi(name = "tunnel-id")]
    pub tunnel_id: Option<u32>,
    /// VLAN ID
    #[qapi(name = "vlan-id")]
    pub vlan_id: Option<u16>,
    /// Ethernet header source MAC address
    #[qapi(name = "eth-src")]
    pub eth_src: Option<String>,
    /// Ethernet header destination MAC address
    #[qapi(name = "eth-dst")]
    pub eth_dst: Option<String>,
    /// IP Header protocol field
    #[qapi(name = "ip-proto")]
    pub ip_proto: Option<u8>,
    /// IP header TOS field
    #[qapi(name = "ip-tos")]
    pub ip_tos: Option<u8>,
}
/// Rocker switch OF-DPA flow action
#[qapi(name = "RockerOfDpaFlowAction")]
#[qapi(since = "2.4")]
pub struct RockerOfDpaFlowAction {
    /// next table ID
    #[qapi(name = "goto-tbl")]
    pub goto_tbl: Option<u32>,
    /// group ID
    #[qapi(name = "group-id")]
    pub group_id: Option<u32>,
    /// tunnel logical port ID
    #[qapi(name = "tunnel-lport")]
    pub tunnel_lport: Option<u32>,
    /// VLAN ID
    #[qapi(name = "vlan-id")]
    pub vlan_id: Option<u16>,
    /// new VLAN ID
    #[qapi(name = "new-vlan-id")]
    pub new_vlan_id: Option<u16>,
    /// physical output port
    #[qapi(name = "out-pport")]
    pub out_pport: Option<u32>,
}
/// Rocker switch OF-DPA flow
#[qapi(name = "RockerOfDpaFlow")]
#[qapi(since = "2.4")]
pub struct RockerOfDpaFlow {
    /// flow unique cookie ID
    #[qapi(name = "cookie")]
    pub cookie: u64,
    /// count of matches (hits) on flow
    #[qapi(name = "hits")]
    pub hits: u64,
    /// flow key
    #[qapi(name = "key")]
    pub key: RockerOfDpaFlowKey,
    /// flow mask
    #[qapi(name = "mask")]
    pub mask: RockerOfDpaFlowMask,
    /// flow action
    #[qapi(name = "action")]
    pub action: RockerOfDpaFlowAction,
}
/// Return rocker OF-DPA flow information.
#[qapi(name = "query-rocker-of-dpa-flows")]
#[qapi(since = "2.4")]
#[qapi(returns = "Vec<RockerOfDpaFlow>")]
pub struct QueryRockerOfDpaFlows {
    /// switch name
    #[qapi(name = "name")]
    pub name: String,
    /// flow table ID.  If tbl-id is not specified, returns flow
    /// information for all tables.
    #[qapi(name = "tbl-id")]
    pub tbl_id: Option<u32>,
}
/// Rocker switch OF-DPA group
#[qapi(name = "RockerOfDpaGroup")]
#[qapi(since = "2.4")]
pub struct RockerOfDpaGroup {
    /// group unique ID
    #[qapi(name = "id")]
    pub id: u32,
    /// group type
    #[qapi(name = "type")]
    pub r#type: u8,
    /// VLAN ID
    #[qapi(name = "vlan-id")]
    pub vlan_id: Option<u16>,
    /// physical port number
    #[qapi(name = "pport")]
    pub pport: Option<u32>,
    /// group index, unique with group type
    #[qapi(name = "index")]
    pub index: Option<u32>,
    /// output physical port number
    #[qapi(name = "out-pport")]
    pub out_pport: Option<u32>,
    /// next group ID
    #[qapi(name = "group-id")]
    pub group_id: Option<u32>,
    /// VLAN ID to set
    #[qapi(name = "set-vlan-id")]
    pub set_vlan_id: Option<u16>,
    /// pop VLAN headr from packet
    #[qapi(name = "pop-vlan")]
    pub pop_vlan: Option<u8>,
    /// list of next group IDs
    #[qapi(name = "group-ids")]
    pub group_ids: Option<Vec<u32>>,
    /// set source MAC address in Ethernet header
    #[qapi(name = "set-eth-src")]
    pub set_eth_src: Option<String>,
    /// set destination MAC address in Ethernet header
    #[qapi(name = "set-eth-dst")]
    pub set_eth_dst: Option<String>,
    /// perform TTL check
    #[qapi(name = "ttl-check")]
    pub ttl_check: Option<u8>,
}
/// Return rocker OF-DPA group information.
#[qapi(name = "query-rocker-of-dpa-groups")]
#[qapi(since = "2.4")]
#[qapi(returns = "Vec<RockerOfDpaGroup>")]
pub struct QueryRockerOfDpaGroups {
    /// switch name
    #[qapi(name = "name")]
    pub name: String,
    /// group type.  If type is not specified, returns group
    /// information for all group types.
    #[qapi(name = "type")]
    pub r#type: Option<u8>,
}
// path end:	qapi/rocker.json
// path begin:	qapi/tpm.json
/// An enumeration of TPM models
#[qapi(name = "TpmModel")]
#[qapi(condition = "CONFIG_TPM")]
#[qapi(since = "1.5")]
pub enum TpmModel {
    /// TPM TIS model
    #[qapi(name = "tpm-tis")]
    TpmTis,
    /// TPM CRB model (since 2.12)
    #[qapi(name = "tpm-crb")]
    TpmCrb,
    /// TPM SPAPR model (since 5.0)
    #[qapi(name = "tpm-spapr")]
    TpmSpapr,
}
/// Return a list of supported TPM models
#[qapi(name = "query-tpm-models")]
#[qapi(condition = "CONFIG_TPM")]
#[qapi(since = "1.5")]
#[qapi(returns = "Vec<TpmModel>")]
pub struct QueryTpmModels {}
/// An enumeration of TPM types
#[qapi(name = "TpmType")]
#[qapi(condition = "CONFIG_TPM")]
#[qapi(since = "1.5")]
pub enum TpmType {
    /// TPM passthrough type
    #[qapi(name = "passthrough")]
    Passthrough,
    /// Software Emulator TPM type (since 2.11)
    #[qapi(name = "emulator")]
    Emulator,
}
/// Return a list of supported TPM types
#[qapi(name = "query-tpm-types")]
#[qapi(condition = "CONFIG_TPM")]
#[qapi(since = "1.5")]
#[qapi(returns = "Vec<TpmType>")]
pub struct QueryTpmTypes {}
/// Information about the TPM passthrough type
#[qapi(name = "TPMPassthroughOptions")]
#[qapi(condition = "CONFIG_TPM")]
#[qapi(since = "1.5")]
pub struct TpmPassthroughOptions {
    /// string describing the path used for accessing the TPM device
    #[qapi(name = "path")]
    pub path: Option<String>,
    /// string showing the TPM's sysfs cancel file for
    /// cancellation of TPM commands while they are executing
    #[qapi(name = "cancel-path")]
    pub cancel_path: Option<String>,
}
/// Information about the TPM emulator type
#[qapi(name = "TPMEmulatorOptions")]
#[qapi(condition = "CONFIG_TPM")]
#[qapi(since = "2.11")]
pub struct TpmEmulatorOptions {
    /// Name of a unix socket chardev
    #[qapi(name = "chardev")]
    pub chardev: String,
}
#[qapi(name = "TPMPassthroughOptionsWrapper")]
#[qapi(condition = "CONFIG_TPM")]
#[qapi(since = "1.5")]
pub struct TpmPassthroughOptionsWrapper {
    /// Information about the TPM passthrough type
    #[qapi(name = "data")]
    pub data: TpmPassthroughOptions,
}
#[qapi(name = "TPMEmulatorOptionsWrapper")]
#[qapi(condition = "CONFIG_TPM")]
#[qapi(since = "2.11")]
pub struct TpmEmulatorOptionsWrapper {
    /// Information about the TPM emulator type
    #[qapi(name = "data")]
    pub data: TpmEmulatorOptions,
}
pub enum TpmTypeOptionsBranch {
    #[qapi(name = "passthrough")]
    Passthrough(TpmPassthroughOptionsWrapper),
    #[qapi(name = "emulator")]
    Emulator(TpmEmulatorOptionsWrapper),
}
/// A union referencing different TPM backend types' configuration
/// options
#[qapi(name = "TpmTypeOptions")]
#[qapi(condition = "CONFIG_TPM")]
#[qapi(since = "1.5")]
pub struct TpmTypeOptions {
    /// - 'passthrough' The configuration options for the TPM
    /// passthrough type
    /// - 'emulator' The configuration options for TPM emulator backend
    /// type
    #[qapi(name = "type")]
    #[qapi(discriminator)]
    pub r#type: TpmType,
    #[qapi(union)]
    pub u: Option<TpmTypeOptionsBranch>,
}
/// Information about the TPM
#[qapi(name = "TPMInfo")]
#[qapi(condition = "CONFIG_TPM")]
#[qapi(since = "1.5")]
pub struct TpmInfo {
    /// The Id of the TPM
    #[qapi(name = "id")]
    pub id: String,
    /// The TPM frontend model
    #[qapi(name = "model")]
    pub model: TpmModel,
    /// The TPM (backend) type configuration options
    #[qapi(name = "options")]
    pub options: TpmTypeOptions,
}
/// Return information about the TPM device
#[qapi(name = "query-tpm")]
#[qapi(condition = "CONFIG_TPM")]
#[qapi(since = "1.5")]
#[qapi(returns = "Vec<TPMInfo>")]
pub struct QueryTpm {}
// path end:	qapi/tpm.json
// path begin:	qapi/ui.json
/// Display protocols which support changing password options.
#[qapi(name = "DisplayProtocol")]
#[qapi(since = "7.0")]
pub enum DisplayProtocol {
    #[qapi(name = "vnc")]
    Vnc,
    #[qapi(name = "spice")]
    Spice,
}
/// An action to take on changing a password on a connection with active
/// clients.
#[qapi(name = "SetPasswordAction")]
#[qapi(since = "7.0")]
pub enum SetPasswordAction {
    /// maintain existing clients
    #[qapi(name = "keep")]
    Keep,
    /// fail the command if clients are connected
    #[qapi(name = "fail")]
    Fail,
    /// disconnect existing clients
    #[qapi(name = "disconnect")]
    Disconnect,
}
pub enum SetPasswordOptionsBranch {
    #[qapi(name = "vnc")]
    Vnc(SetPasswordOptionsVnc),
}
/// Options for set_password.
#[qapi(name = "SetPasswordOptions")]
#[qapi(since = "7.0")]
pub struct SetPasswordOptions {
    /// - 'vnc' to modify the VNC server password
    /// - 'spice' to modify the Spice server password
    #[qapi(name = "protocol")]
    #[qapi(discriminator)]
    pub protocol: DisplayProtocol,
    /// the new password
    #[qapi(name = "password")]
    pub password: String,
    /// How to handle existing clients when changing the
    /// password.  If nothing is specified, defaults to 'keep'.  For
    /// VNC, only 'keep' is currently implemented.
    #[qapi(name = "connected")]
    pub connected: Option<SetPasswordAction>,
    #[qapi(union)]
    pub u: Option<SetPasswordOptionsBranch>,
}
/// Options for set_password specific to the VNC protocol.
#[qapi(name = "SetPasswordOptionsVnc")]
#[qapi(since = "7.0")]
pub struct SetPasswordOptionsVnc {
    /// The id of the display where the password should be
    /// changed.  Defaults to the first.
    #[qapi(name = "display")]
    pub display: Option<String>,
}
/// Set the password of a remote display server.
#[qapi(name = "set_password")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct SetPassword {
    #[qapi(flatten)]
    pub data: SetPasswordOptions,
}
pub enum ExpirePasswordOptionsBranch {
    #[qapi(name = "vnc")]
    Vnc(ExpirePasswordOptionsVnc),
}
/// General options for expire_password.
#[qapi(name = "ExpirePasswordOptions")]
#[qapi(since = "7.0")]
pub struct ExpirePasswordOptions {
    /// - 'vnc' to modify the VNC server expiration
    /// - 'spice' to modify the Spice server expiration
    #[qapi(name = "protocol")]
    #[qapi(discriminator)]
    pub protocol: DisplayProtocol,
    /// when to expire the password.
    ///
    /// - 'now' to expire the password immediately
    /// - 'never' to cancel password expiration
    /// - '+INT' where INT is the number of seconds from now (integer)
    /// - 'INT' where INT is the absolute time in seconds
    #[qapi(name = "time")]
    pub time: String,
    #[qapi(union)]
    pub u: Option<ExpirePasswordOptionsBranch>,
}
/// Options for expire_password specific to the VNC protocol.
#[qapi(name = "ExpirePasswordOptionsVnc")]
#[qapi(since = "7.0")]
pub struct ExpirePasswordOptionsVnc {
    /// The id of the display where the expiration should be
    /// changed.  Defaults to the first.
    #[qapi(name = "display")]
    pub display: Option<String>,
}
/// Expire the password of a remote display server.
#[qapi(name = "expire_password")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct ExpirePassword {
    #[qapi(flatten)]
    pub data: ExpirePasswordOptions,
}
/// Supported image format types.
#[qapi(name = "ImageFormat")]
#[qapi(since = "7.1")]
pub enum ImageFormat {
    /// PPM format
    #[qapi(name = "ppm")]
    Ppm,
    /// PNG format
    #[qapi(name = "png")]
    Png,
}
/// Capture the contents of a screen and write it to a file.
#[qapi(name = "screendump")]
#[qapi(condition = "CONFIG_PIXMAN")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct Screendump {
    /// the path of a new file to store the image
    #[qapi(name = "filename")]
    pub filename: String,
    /// ID of the display device that should be dumped.  If this
    /// parameter is missing, the primary display will be used.  (Since
    /// 2.12)
    #[qapi(name = "device")]
    pub device: Option<String>,
    /// head to use in case the device supports multiple heads.  If
    /// this parameter is missing, head #0 will be used.  Also note that
    /// the head can only be specified in conjunction with the device
    /// ID.  (Since 2.12)
    #[qapi(name = "head")]
    pub head: Option<i64>,
    /// image format for screendump.  (default: ppm) (Since 7.1)
    #[qapi(name = "format")]
    pub format: Option<ImageFormat>,
}
/// The basic information for SPICE network connection
#[qapi(name = "SpiceBasicInfo")]
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(since = "2.1")]
pub struct SpiceBasicInfo {
    /// IP address
    #[qapi(name = "host")]
    pub host: String,
    /// port number
    #[qapi(name = "port")]
    pub port: String,
    /// address family
    #[qapi(name = "family")]
    pub family: NetworkAddressFamily,
}
/// Information about a SPICE server
#[qapi(name = "SpiceServerInfo")]
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(since = "2.1")]
pub struct SpiceServerInfo {
    /// IP address
    #[qapi(name = "host")]
    pub host: String,
    /// port number
    #[qapi(name = "port")]
    pub port: String,
    /// address family
    #[qapi(name = "family")]
    pub family: NetworkAddressFamily,
    /// authentication method
    #[qapi(name = "auth")]
    pub auth: Option<String>,
}
/// Information about a SPICE client channel.
#[qapi(name = "SpiceChannel")]
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(since = "0.14")]
pub struct SpiceChannel {
    /// IP address
    #[qapi(name = "host")]
    pub host: String,
    /// port number
    #[qapi(name = "port")]
    pub port: String,
    /// address family
    #[qapi(name = "family")]
    pub family: NetworkAddressFamily,
    /// SPICE connection id number.  All channels with the
    /// same id belong to the same SPICE session.
    #[qapi(name = "connection-id")]
    pub connection_id: i64,
    /// SPICE channel type number.  "1" is the main control
    /// channel, filter for this one if you want to track spice sessions
    /// only
    #[qapi(name = "channel-type")]
    pub channel_type: i64,
    /// SPICE channel ID number.  Usually "0", might be
    /// different when multiple channels of the same type exist, such as
    /// multiple display channels in a multihead setup
    #[qapi(name = "channel-id")]
    pub channel_id: i64,
    /// true if the channel is encrypted, false otherwise.
    #[qapi(name = "tls")]
    pub tls: bool,
}
/// An enumeration of Spice mouse states.
#[qapi(name = "SpiceQueryMouseMode")]
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(since = "1.1")]
pub enum SpiceQueryMouseMode {
    /// Mouse cursor position is determined by the client.
    #[qapi(name = "client")]
    Client,
    /// Mouse cursor position is determined by the server.
    #[qapi(name = "server")]
    Server,
    /// No information is available about mouse mode used by the
    /// spice server.
    #[qapi(name = "unknown")]
    Unknown,
}
/// Information about the SPICE session.
#[qapi(name = "SpiceInfo")]
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(since = "0.14")]
pub struct SpiceInfo {
    /// true if the SPICE server is enabled, false otherwise
    #[qapi(name = "enabled")]
    pub enabled: bool,
    /// true if the last guest migration completed and spice
    /// migration had completed as well, false otherwise (since 1.4)
    #[qapi(name = "migrated")]
    pub migrated: bool,
    /// The hostname the SPICE server is bound to.  This depends on
    /// the name resolution on the host and may be an IP address.
    #[qapi(name = "host")]
    pub host: Option<String>,
    /// The SPICE server's port number.
    #[qapi(name = "port")]
    pub port: Option<i64>,
    /// The SPICE server's TLS port number.
    #[qapi(name = "tls-port")]
    pub tls_port: Option<i64>,
    /// the current authentication type used by the server
    ///
    /// - 'none' if no authentication is being used
    /// - 'spice' uses SASL or direct TLS authentication, depending on
    /// command line options
    #[qapi(name = "auth")]
    pub auth: Option<String>,
    /// SPICE server version.
    #[qapi(name = "compiled-version")]
    pub compiled_version: Option<String>,
    /// The mode in which the mouse cursor is displayed
    /// currently.  Can be determined by the client or the server, or
    /// unknown if spice server doesn't provide this information.
    /// (since: 1.1)
    #[qapi(name = "mouse-mode")]
    pub mouse_mode: SpiceQueryMouseMode,
    /// a list of @SpiceChannel for each active spice channel
    #[qapi(name = "channels")]
    pub channels: Option<Vec<SpiceChannel>>,
}
/// Returns information about the current SPICE server
#[qapi(name = "query-spice")]
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(since = "0.14")]
#[qapi(returns = "SpiceInfo")]
pub struct QuerySpice {}
/// Emitted when a SPICE client establishes a connection
#[qapi(name = "SPICE_CONNECTED")]
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(since = "0.14")]
pub struct SpiceConnected {
    /// server information
    #[qapi(name = "server")]
    pub server: SpiceBasicInfo,
    /// client information
    #[qapi(name = "client")]
    pub client: SpiceBasicInfo,
}
/// Emitted after initial handshake and authentication takes place (if
/// any) and the SPICE channel is up and running
#[qapi(name = "SPICE_INITIALIZED")]
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(since = "0.14")]
pub struct SpiceInitialized {
    /// server information
    #[qapi(name = "server")]
    pub server: SpiceServerInfo,
    /// client information
    #[qapi(name = "client")]
    pub client: SpiceChannel,
}
/// Emitted when the SPICE connection is closed
#[qapi(name = "SPICE_DISCONNECTED")]
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(since = "0.14")]
pub struct SpiceDisconnected {
    /// server information
    #[qapi(name = "server")]
    pub server: SpiceBasicInfo,
    /// client information
    #[qapi(name = "client")]
    pub client: SpiceBasicInfo,
}
/// Emitted when SPICE migration has completed
#[qapi(name = "SPICE_MIGRATE_COMPLETED")]
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(since = "1.3")]
pub struct SpiceMigrateCompleted {}
/// The basic information for vnc network connection
#[qapi(name = "VncBasicInfo")]
#[qapi(condition = "CONFIG_VNC")]
#[qapi(since = "2.1")]
pub struct VncBasicInfo {
    /// IP address
    #[qapi(name = "host")]
    pub host: String,
    /// The service name of the vnc port.  This may depend on the
    /// host system's service database so symbolic names should not be
    /// relied on.
    #[qapi(name = "service")]
    pub service: String,
    /// address family
    #[qapi(name = "family")]
    pub family: NetworkAddressFamily,
    /// true in case the socket is a websocket (since 2.3).
    #[qapi(name = "websocket")]
    pub websocket: bool,
}
/// The network connection information for server
#[qapi(name = "VncServerInfo")]
#[qapi(condition = "CONFIG_VNC")]
#[qapi(since = "2.1")]
pub struct VncServerInfo {
    /// IP address
    #[qapi(name = "host")]
    pub host: String,
    /// The service name of the vnc port.  This may depend on the
    /// host system's service database so symbolic names should not be
    /// relied on.
    #[qapi(name = "service")]
    pub service: String,
    /// address family
    #[qapi(name = "family")]
    pub family: NetworkAddressFamily,
    /// true in case the socket is a websocket (since 2.3).
    #[qapi(name = "websocket")]
    pub websocket: bool,
    /// authentication method used for the plain (non-websocket) VNC
    /// server
    #[qapi(name = "auth")]
    pub auth: Option<String>,
}
/// Information about a connected VNC client.
#[qapi(name = "VncClientInfo")]
#[qapi(condition = "CONFIG_VNC")]
#[qapi(since = "0.14")]
pub struct VncClientInfo {
    /// IP address
    #[qapi(name = "host")]
    pub host: String,
    /// The service name of the vnc port.  This may depend on the
    /// host system's service database so symbolic names should not be
    /// relied on.
    #[qapi(name = "service")]
    pub service: String,
    /// address family
    #[qapi(name = "family")]
    pub family: NetworkAddressFamily,
    /// true in case the socket is a websocket (since 2.3).
    #[qapi(name = "websocket")]
    pub websocket: bool,
    /// If x509 authentication is in use, the Distinguished
    /// Name of the client.
    #[qapi(name = "x509_dname")]
    pub x509_dname: Option<String>,
    /// If SASL authentication is in use, the SASL username
    /// used for authentication.
    #[qapi(name = "sasl_username")]
    pub sasl_username: Option<String>,
}
/// Information about the VNC session.
#[qapi(name = "VncInfo")]
#[qapi(condition = "CONFIG_VNC")]
#[qapi(since = "0.14")]
pub struct VncInfo {
    /// true if the VNC server is enabled, false otherwise
    #[qapi(name = "enabled")]
    pub enabled: bool,
    /// The hostname the VNC server is bound to.  This depends on the
    /// name resolution on the host and may be an IP address.
    #[qapi(name = "host")]
    pub host: Option<String>,
    /// - 'ipv6' if the host is listening for IPv6 connections
    /// - 'ipv4' if the host is listening for IPv4 connections
    /// - 'unix' if the host is listening on a unix domain socket
    /// - 'unknown' otherwise
    #[qapi(name = "family")]
    pub family: Option<NetworkAddressFamily>,
    /// The service name of the server's port.  This may depends
    /// on the host system's service database so symbolic names should
    /// not be relied on.
    #[qapi(name = "service")]
    pub service: Option<String>,
    /// the current authentication type used by the server
    ///
    /// - 'none' if no authentication is being used
    /// - 'vnc' if VNC authentication is being used
    /// - 'vencrypt+plain' if VEncrypt is used with plain text
    /// authentication
    /// - 'vencrypt+tls+none' if VEncrypt is used with TLS and no
    /// authentication
    /// - 'vencrypt+tls+vnc' if VEncrypt is used with TLS and VNC
    /// authentication
    /// - 'vencrypt+tls+plain' if VEncrypt is used with TLS and plain
    /// text auth
    /// - 'vencrypt+x509+none' if VEncrypt is used with x509 and no auth
    /// - 'vencrypt+x509+vnc' if VEncrypt is used with x509 and VNC auth
    /// - 'vencrypt+x509+plain' if VEncrypt is used with x509 and plain
    /// text auth
    /// - 'vencrypt+tls+sasl' if VEncrypt is used with TLS and SASL auth
    /// - 'vencrypt+x509+sasl' if VEncrypt is used with x509 and SASL
    /// auth
    #[qapi(name = "auth")]
    pub auth: Option<String>,
    /// a list of @VncClientInfo of all currently connected
    /// clients
    #[qapi(name = "clients")]
    pub clients: Option<Vec<VncClientInfo>>,
}
/// vnc primary authentication method.
#[qapi(name = "VncPrimaryAuth")]
#[qapi(condition = "CONFIG_VNC")]
#[qapi(since = "2.3")]
pub enum VncPrimaryAuth {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "vnc")]
    Vnc,
    #[qapi(name = "ra2")]
    Ra2,
    #[qapi(name = "ra2ne")]
    Ra2ne,
    #[qapi(name = "tight")]
    Tight,
    #[qapi(name = "ultra")]
    Ultra,
    #[qapi(name = "tls")]
    Tls,
    #[qapi(name = "vencrypt")]
    Vencrypt,
    #[qapi(name = "sasl")]
    Sasl,
}
/// vnc sub authentication method with vencrypt.
#[qapi(name = "VncVencryptSubAuth")]
#[qapi(condition = "CONFIG_VNC")]
#[qapi(since = "2.3")]
pub enum VncVencryptSubAuth {
    #[qapi(name = "plain")]
    Plain,
    #[qapi(name = "tls-none")]
    TlsNone,
    #[qapi(name = "x509-none")]
    X509None,
    #[qapi(name = "tls-vnc")]
    TlsVnc,
    #[qapi(name = "x509-vnc")]
    X509Vnc,
    #[qapi(name = "tls-plain")]
    TlsPlain,
    #[qapi(name = "x509-plain")]
    X509Plain,
    #[qapi(name = "tls-sasl")]
    TlsSasl,
    #[qapi(name = "x509-sasl")]
    X509Sasl,
}
/// The network connection information for server
#[qapi(name = "VncServerInfo2")]
#[qapi(condition = "CONFIG_VNC")]
#[qapi(since = "2.9")]
pub struct VncServerInfo2 {
    /// IP address
    #[qapi(name = "host")]
    pub host: String,
    /// The service name of the vnc port.  This may depend on the
    /// host system's service database so symbolic names should not be
    /// relied on.
    #[qapi(name = "service")]
    pub service: String,
    /// address family
    #[qapi(name = "family")]
    pub family: NetworkAddressFamily,
    /// true in case the socket is a websocket (since 2.3).
    #[qapi(name = "websocket")]
    pub websocket: bool,
    /// The current authentication type used by the servers
    #[qapi(name = "auth")]
    pub auth: VncPrimaryAuth,
    /// The vencrypt sub authentication type used by the servers,
    /// only specified in case auth == vencrypt.
    #[qapi(name = "vencrypt")]
    pub vencrypt: Option<VncVencryptSubAuth>,
}
/// Information about a vnc server
#[qapi(name = "VncInfo2")]
#[qapi(condition = "CONFIG_VNC")]
#[qapi(since = "2.3")]
pub struct VncInfo2 {
    /// vnc server name.
    #[qapi(name = "id")]
    pub id: String,
    /// A list of @VncBasincInfo describing all listening sockets.
    /// The list can be empty (in case the vnc server is disabled).  It
    /// also may have multiple entries: normal + websocket, possibly
    /// also ipv4 + ipv6 in the future.
    #[qapi(name = "server")]
    pub server: Vec<VncServerInfo2>,
    /// A list of @VncClientInfo of all currently connected
    /// clients.  The list can be empty, for obvious reasons.
    #[qapi(name = "clients")]
    pub clients: Vec<VncClientInfo>,
    /// The current authentication type used by the non-websockets
    /// servers
    #[qapi(name = "auth")]
    pub auth: VncPrimaryAuth,
    /// The vencrypt authentication type used by the servers,
    /// only specified in case auth == vencrypt.
    #[qapi(name = "vencrypt")]
    pub vencrypt: Option<VncVencryptSubAuth>,
    /// The display device the vnc server is linked to.
    #[qapi(name = "display")]
    pub display: Option<String>,
}
/// Returns information about the current VNC server
#[qapi(name = "query-vnc")]
#[qapi(condition = "CONFIG_VNC")]
#[qapi(since = "0.14")]
#[qapi(returns = "VncInfo")]
pub struct QueryVnc {}
/// Returns a list of vnc servers.  The list can be empty.
#[qapi(name = "query-vnc-servers")]
#[qapi(condition = "CONFIG_VNC")]
#[qapi(since = "2.3")]
#[qapi(returns = "Vec<VncInfo2>")]
pub struct QueryVncServers {}
/// Change the VNC server password.
#[qapi(name = "change-vnc-password")]
#[qapi(condition = "CONFIG_VNC")]
#[qapi(since = "1.1")]
#[qapi(returns = "()")]
pub struct ChangeVncPassword {
    /// the new password to use with VNC authentication
    #[qapi(name = "password")]
    pub password: String,
}
/// Emitted when a VNC client establishes a connection
#[qapi(name = "VNC_CONNECTED")]
#[qapi(condition = "CONFIG_VNC")]
#[qapi(since = "0.13")]
pub struct VncConnected {
    /// server information
    #[qapi(name = "server")]
    pub server: VncServerInfo,
    /// client information
    #[qapi(name = "client")]
    pub client: VncBasicInfo,
}
/// Emitted after authentication takes place (if any) and the VNC
/// session is made active
#[qapi(name = "VNC_INITIALIZED")]
#[qapi(condition = "CONFIG_VNC")]
#[qapi(since = "0.13")]
pub struct VncInitialized {
    /// server information
    #[qapi(name = "server")]
    pub server: VncServerInfo,
    /// client information
    #[qapi(name = "client")]
    pub client: VncClientInfo,
}
/// Emitted when the connection is closed
#[qapi(name = "VNC_DISCONNECTED")]
#[qapi(condition = "CONFIG_VNC")]
#[qapi(since = "0.13")]
pub struct VncDisconnected {
    /// server information
    #[qapi(name = "server")]
    pub server: VncServerInfo,
    /// client information
    #[qapi(name = "client")]
    pub client: VncClientInfo,
}
/// Information about a mouse device.
#[qapi(name = "MouseInfo")]
#[qapi(since = "0.14")]
pub struct MouseInfo {
    /// the name of the mouse device
    #[qapi(name = "name")]
    pub name: String,
    /// the index of the mouse device
    #[qapi(name = "index")]
    pub index: i64,
    /// true if this device is currently receiving mouse events
    #[qapi(name = "current")]
    pub current: bool,
    /// true if this device supports absolute coordinates as
    /// input
    #[qapi(name = "absolute")]
    pub absolute: bool,
}
/// Returns information about each active mouse device
#[qapi(name = "query-mice")]
#[qapi(since = "0.14")]
#[qapi(returns = "Vec<MouseInfo>")]
pub struct QueryMice {}
/// An enumeration of key name.
///
/// This is used by the @send-key command.
#[qapi(name = "QKeyCode")]
#[qapi(since = "1.3")]
pub enum QKeyCode {
    /// since 2.0
    #[qapi(name = "unmapped")]
    Unmapped,
    #[qapi(name = "shift")]
    Shift,
    #[qapi(name = "shift_r")]
    ShiftR,
    #[qapi(name = "alt")]
    Alt,
    #[qapi(name = "alt_r")]
    AltR,
    #[qapi(name = "ctrl")]
    Ctrl,
    #[qapi(name = "ctrl_r")]
    CtrlR,
    #[qapi(name = "menu")]
    Menu,
    #[qapi(name = "esc")]
    Esc,
    #[qapi(name = "1")]
    _1,
    #[qapi(name = "2")]
    _2,
    #[qapi(name = "3")]
    _3,
    #[qapi(name = "4")]
    _4,
    #[qapi(name = "5")]
    _5,
    #[qapi(name = "6")]
    _6,
    #[qapi(name = "7")]
    _7,
    #[qapi(name = "8")]
    _8,
    #[qapi(name = "9")]
    _9,
    #[qapi(name = "0")]
    _0,
    #[qapi(name = "minus")]
    Minus,
    #[qapi(name = "equal")]
    Equal,
    #[qapi(name = "backspace")]
    Backspace,
    #[qapi(name = "tab")]
    Tab,
    #[qapi(name = "q")]
    Q,
    #[qapi(name = "w")]
    W,
    #[qapi(name = "e")]
    E,
    #[qapi(name = "r")]
    R,
    #[qapi(name = "t")]
    T,
    #[qapi(name = "y")]
    Y,
    #[qapi(name = "u")]
    U,
    #[qapi(name = "i")]
    I,
    #[qapi(name = "o")]
    O,
    #[qapi(name = "p")]
    P,
    #[qapi(name = "bracket_left")]
    BracketLeft,
    #[qapi(name = "bracket_right")]
    BracketRight,
    #[qapi(name = "ret")]
    Ret,
    #[qapi(name = "a")]
    A,
    #[qapi(name = "s")]
    S,
    #[qapi(name = "d")]
    D,
    #[qapi(name = "f")]
    F,
    #[qapi(name = "g")]
    G,
    #[qapi(name = "h")]
    H,
    #[qapi(name = "j")]
    J,
    #[qapi(name = "k")]
    K,
    #[qapi(name = "l")]
    L,
    #[qapi(name = "semicolon")]
    Semicolon,
    #[qapi(name = "apostrophe")]
    Apostrophe,
    #[qapi(name = "grave_accent")]
    GraveAccent,
    #[qapi(name = "backslash")]
    Backslash,
    #[qapi(name = "z")]
    Z,
    #[qapi(name = "x")]
    X,
    #[qapi(name = "c")]
    C,
    #[qapi(name = "v")]
    V,
    #[qapi(name = "b")]
    B,
    #[qapi(name = "n")]
    N,
    #[qapi(name = "m")]
    M,
    #[qapi(name = "comma")]
    Comma,
    #[qapi(name = "dot")]
    Dot,
    #[qapi(name = "slash")]
    Slash,
    #[qapi(name = "asterisk")]
    Asterisk,
    #[qapi(name = "spc")]
    Spc,
    #[qapi(name = "caps_lock")]
    CapsLock,
    #[qapi(name = "f1")]
    F1,
    #[qapi(name = "f2")]
    F2,
    #[qapi(name = "f3")]
    F3,
    #[qapi(name = "f4")]
    F4,
    #[qapi(name = "f5")]
    F5,
    #[qapi(name = "f6")]
    F6,
    #[qapi(name = "f7")]
    F7,
    #[qapi(name = "f8")]
    F8,
    #[qapi(name = "f9")]
    F9,
    #[qapi(name = "f10")]
    F10,
    #[qapi(name = "num_lock")]
    NumLock,
    #[qapi(name = "scroll_lock")]
    ScrollLock,
    #[qapi(name = "kp_divide")]
    KpDivide,
    #[qapi(name = "kp_multiply")]
    KpMultiply,
    #[qapi(name = "kp_subtract")]
    KpSubtract,
    #[qapi(name = "kp_add")]
    KpAdd,
    #[qapi(name = "kp_enter")]
    KpEnter,
    #[qapi(name = "kp_decimal")]
    KpDecimal,
    #[qapi(name = "sysrq")]
    Sysrq,
    #[qapi(name = "kp_0")]
    Kp0,
    #[qapi(name = "kp_1")]
    Kp1,
    #[qapi(name = "kp_2")]
    Kp2,
    #[qapi(name = "kp_3")]
    Kp3,
    #[qapi(name = "kp_4")]
    Kp4,
    #[qapi(name = "kp_5")]
    Kp5,
    #[qapi(name = "kp_6")]
    Kp6,
    #[qapi(name = "kp_7")]
    Kp7,
    #[qapi(name = "kp_8")]
    Kp8,
    #[qapi(name = "kp_9")]
    Kp9,
    #[qapi(name = "less")]
    Less,
    #[qapi(name = "f11")]
    F11,
    #[qapi(name = "f12")]
    F12,
    #[qapi(name = "print")]
    Print,
    #[qapi(name = "home")]
    Home,
    #[qapi(name = "pgup")]
    Pgup,
    #[qapi(name = "pgdn")]
    Pgdn,
    #[qapi(name = "end")]
    End,
    #[qapi(name = "left")]
    Left,
    #[qapi(name = "up")]
    Up,
    #[qapi(name = "down")]
    Down,
    #[qapi(name = "right")]
    Right,
    #[qapi(name = "insert")]
    Insert,
    #[qapi(name = "delete")]
    Delete,
    #[qapi(name = "stop")]
    Stop,
    #[qapi(name = "again")]
    Again,
    #[qapi(name = "props")]
    Props,
    #[qapi(name = "undo")]
    Undo,
    #[qapi(name = "front")]
    Front,
    #[qapi(name = "copy")]
    Copy,
    #[qapi(name = "open")]
    Open,
    #[qapi(name = "paste")]
    Paste,
    #[qapi(name = "find")]
    Find,
    #[qapi(name = "cut")]
    Cut,
    #[qapi(name = "lf")]
    Lf,
    #[qapi(name = "help")]
    Help,
    #[qapi(name = "meta_l")]
    MetaL,
    #[qapi(name = "meta_r")]
    MetaR,
    #[qapi(name = "compose")]
    Compose,
    /// since 2.0
    #[qapi(name = "pause")]
    Pause,
    /// since 2.4
    #[qapi(name = "ro")]
    Ro,
    /// since 2.9
    #[qapi(name = "hiragana")]
    Hiragana,
    /// since 2.9
    #[qapi(name = "henkan")]
    Henkan,
    /// since 2.9
    #[qapi(name = "yen")]
    Yen,
    /// since 2.12
    #[qapi(name = "muhenkan")]
    Muhenkan,
    /// since 2.12
    #[qapi(name = "katakanahiragana")]
    Katakanahiragana,
    /// since 2.4
    #[qapi(name = "kp_comma")]
    KpComma,
    /// since 2.6
    #[qapi(name = "kp_equals")]
    KpEquals,
    /// since 2.6
    #[qapi(name = "power")]
    Power,
    /// since 2.10
    #[qapi(name = "sleep")]
    Sleep,
    /// since 2.10
    #[qapi(name = "wake")]
    Wake,
    /// since 2.10
    #[qapi(name = "audionext")]
    Audionext,
    /// since 2.10
    #[qapi(name = "audioprev")]
    Audioprev,
    /// since 2.10
    #[qapi(name = "audiostop")]
    Audiostop,
    /// since 2.10
    #[qapi(name = "audioplay")]
    Audioplay,
    /// since 2.10
    #[qapi(name = "audiomute")]
    Audiomute,
    /// since 2.10
    #[qapi(name = "volumeup")]
    Volumeup,
    /// since 2.10
    #[qapi(name = "volumedown")]
    Volumedown,
    /// since 2.10
    #[qapi(name = "mediaselect")]
    Mediaselect,
    /// since 2.10
    #[qapi(name = "mail")]
    Mail,
    /// since 2.10
    #[qapi(name = "calculator")]
    Calculator,
    /// since 2.10
    #[qapi(name = "computer")]
    Computer,
    /// since 2.10
    #[qapi(name = "ac_home")]
    AcHome,
    /// since 2.10
    #[qapi(name = "ac_back")]
    AcBack,
    /// since 2.10
    #[qapi(name = "ac_forward")]
    AcForward,
    /// since 2.10
    #[qapi(name = "ac_refresh")]
    AcRefresh,
    /// since 2.10
    #[qapi(name = "ac_bookmarks")]
    AcBookmarks,
    /// since 6.1
    #[qapi(name = "lang1")]
    Lang1,
    /// since 6.1
    #[qapi(name = "lang2")]
    Lang2,
    /// since 8.0
    #[qapi(name = "f13")]
    F13,
    /// since 8.0
    #[qapi(name = "f14")]
    F14,
    /// since 8.0
    #[qapi(name = "f15")]
    F15,
    /// since 8.0
    #[qapi(name = "f16")]
    F16,
    /// since 8.0
    #[qapi(name = "f17")]
    F17,
    /// since 8.0
    #[qapi(name = "f18")]
    F18,
    /// since 8.0
    #[qapi(name = "f19")]
    F19,
    /// since 8.0
    #[qapi(name = "f20")]
    F20,
    /// since 8.0
    #[qapi(name = "f21")]
    F21,
    /// since 8.0
    #[qapi(name = "f22")]
    F22,
    /// since 8.0
    #[qapi(name = "f23")]
    F23,
    /// since 8.0
    ///
    /// 'sysrq' was mistakenly added to hack around the fact that the ps2
    /// driver was not generating correct scancodes sequences when
    /// 'alt+print' was pressed.  This flaw is now fixed and the 'sysrq' key
    /// serves no further purpose.  Any further use of 'sysrq' will be
    /// transparently changed to 'print', so they are effectively synonyms.
    #[qapi(name = "f24")]
    F24,
}
#[qapi(name = "KeyValueKind")]
#[qapi(since = "1.3")]
pub enum KeyValueKind {
    #[qapi(name = "number")]
    f64,
    #[qapi(name = "qcode")]
    Qcode,
}
#[qapi(name = "IntWrapper")]
#[qapi(since = "1.3")]
pub struct IntWrapper {
    /// a numeric key code
    #[qapi(name = "data")]
    pub data: i64,
}
#[qapi(name = "QKeyCodeWrapper")]
#[qapi(since = "1.3")]
pub struct QKeyCodeWrapper {
    /// An enumeration of key name
    #[qapi(name = "data")]
    pub data: QKeyCode,
}
pub enum KeyValueBranch {
    #[qapi(name = "number")]
    f64(IntWrapper),
    #[qapi(name = "qcode")]
    Qcode(QKeyCodeWrapper),
}
/// Represents a keyboard key.
#[qapi(name = "KeyValue")]
#[qapi(since = "1.3")]
pub struct KeyValue {
    /// key encoding
    #[qapi(name = "type")]
    #[qapi(discriminator)]
    pub r#type: KeyValueKind,
    #[qapi(union)]
    pub u: Option<KeyValueBranch>,
}
/// Send keys to guest.
#[qapi(name = "send-key")]
#[qapi(since = "1.3")]
#[qapi(returns = "()")]
pub struct SendKey {
    /// An array of @KeyValue elements.  All @KeyValues in this array
    /// are simultaneously sent to the guest.  A @KeyValue.number value
    /// is sent directly to the guest, while @KeyValue.qcode must be a
    /// valid @QKeyCode value
    #[qapi(name = "keys")]
    pub keys: Vec<KeyValue>,
    /// time to delay key up events, milliseconds.  Defaults to
    /// 100
    #[qapi(name = "hold-time")]
    pub hold_time: Option<i64>,
}
/// Button of a pointer input device (mouse, tablet).
#[qapi(name = "InputButton")]
#[qapi(since = "2.0")]
pub enum InputButton {
    #[qapi(name = "left")]
    Left,
    #[qapi(name = "middle")]
    Middle,
    #[qapi(name = "right")]
    Right,
    #[qapi(name = "wheel-up")]
    WheelUp,
    #[qapi(name = "wheel-down")]
    WheelDown,
    /// front side button of a 5-button mouse (since 2.9)
    #[qapi(name = "side")]
    Side,
    /// rear side button of a 5-button mouse (since 2.9)
    #[qapi(name = "extra")]
    Extra,
    #[qapi(name = "wheel-left")]
    WheelLeft,
    #[qapi(name = "wheel-right")]
    WheelRight,
    /// screen contact on a multi-touch device (since 8.1)
    #[qapi(name = "touch")]
    Touch,
}
/// Position axis of a pointer input device (mouse, tablet).
#[qapi(name = "InputAxis")]
#[qapi(since = "2.0")]
pub enum InputAxis {
    #[qapi(name = "x")]
    X,
    #[qapi(name = "y")]
    Y,
}
/// Type of a multi-touch event.
#[qapi(name = "InputMultiTouchType")]
#[qapi(since = "8.1")]
pub enum InputMultiTouchType {
    /// A new touch event sequence has just started.
    #[qapi(name = "begin")]
    Begin,
    /// A touch event sequence has been updated.
    #[qapi(name = "update")]
    Update,
    /// A touch event sequence has finished.
    #[qapi(name = "end")]
    End,
    /// A touch event sequence has been canceled.
    #[qapi(name = "cancel")]
    Cancel,
    /// Absolute position data.
    #[qapi(name = "data")]
    Data,
}
/// Keyboard input event.
#[qapi(name = "InputKeyEvent")]
#[qapi(since = "2.0")]
pub struct InputKeyEvent {
    /// Which key this event is for.
    #[qapi(name = "key")]
    pub key: KeyValue,
    /// True for key-down and false for key-up events.
    #[qapi(name = "down")]
    pub down: bool,
}
/// Pointer button input event.
#[qapi(name = "InputBtnEvent")]
#[qapi(since = "2.0")]
pub struct InputBtnEvent {
    /// Which button this event is for.
    #[qapi(name = "button")]
    pub button: InputButton,
    /// True for key-down and false for key-up events.
    #[qapi(name = "down")]
    pub down: bool,
}
/// Pointer motion input event.
#[qapi(name = "InputMoveEvent")]
#[qapi(since = "2.0")]
pub struct InputMoveEvent {
    /// Which axis is referenced by @value.
    #[qapi(name = "axis")]
    pub axis: InputAxis,
    /// Pointer position.  For absolute coordinates the valid range
    /// is 0 -> 0x7ffff
    #[qapi(name = "value")]
    pub value: i64,
}
/// MultiTouch input event.
#[qapi(name = "InputMultiTouchEvent")]
#[qapi(since = "8.1")]
pub struct InputMultiTouchEvent {
    /// The type of multi-touch event.
    #[qapi(name = "type")]
    pub r#type: InputMultiTouchType,
    /// Which slot has generated the event.
    #[qapi(name = "slot")]
    pub slot: i64,
    /// ID to correlate this event with previously generated
    /// events.
    #[qapi(name = "tracking-id")]
    pub tracking_id: i64,
    /// Which axis is referenced by @value.
    #[qapi(name = "axis")]
    pub axis: InputAxis,
    /// Contact position.
    #[qapi(name = "value")]
    pub value: i64,
}
#[qapi(name = "InputEventKind")]
#[qapi(since = "2.0")]
pub enum InputEventKind {
    /// a keyboard input event
    #[qapi(name = "key")]
    Key,
    /// a pointer button input event
    #[qapi(name = "btn")]
    Btn,
    /// a relative pointer motion input event
    #[qapi(name = "rel")]
    Rel,
    /// an absolute pointer motion input event
    #[qapi(name = "abs")]
    Abs,
    /// a multi-touch input event
    #[qapi(name = "mtt")]
    Mtt,
}
#[qapi(name = "InputKeyEventWrapper")]
#[qapi(since = "2.0")]
pub struct InputKeyEventWrapper {
    /// Keyboard input event
    #[qapi(name = "data")]
    pub data: InputKeyEvent,
}
#[qapi(name = "InputBtnEventWrapper")]
#[qapi(since = "2.0")]
pub struct InputBtnEventWrapper {
    /// Pointer button input event
    #[qapi(name = "data")]
    pub data: InputBtnEvent,
}
#[qapi(name = "InputMoveEventWrapper")]
#[qapi(since = "2.0")]
pub struct InputMoveEventWrapper {
    /// Pointer motion input event
    #[qapi(name = "data")]
    pub data: InputMoveEvent,
}
#[qapi(name = "InputMultiTouchEventWrapper")]
#[qapi(since = "8.1")]
pub struct InputMultiTouchEventWrapper {
    /// MultiTouch input event
    #[qapi(name = "data")]
    pub data: InputMultiTouchEvent,
}
pub enum InputEventBranch {
    #[qapi(name = "key")]
    Key(InputKeyEventWrapper),
    #[qapi(name = "btn")]
    Btn(InputBtnEventWrapper),
    #[qapi(name = "rel")]
    Rel(InputMoveEventWrapper),
    #[qapi(name = "abs")]
    Abs(InputMoveEventWrapper),
    #[qapi(name = "mtt")]
    Mtt(InputMultiTouchEventWrapper),
}
/// Input event union.
#[qapi(name = "InputEvent")]
#[qapi(since = "2.0")]
pub struct InputEvent {
    /// the type of input event
    #[qapi(name = "type")]
    #[qapi(discriminator)]
    pub r#type: InputEventKind,
    #[qapi(union)]
    pub u: Option<InputEventBranch>,
}
/// Send input event(s) to guest.
///
/// The @device and @head parameters can be used to send the input event
/// to specific input devices in case (a) multiple input devices of the
/// same kind are added to the virtual machine and (b) you have
/// configured input routing (see docs/multiseat.txt) for those input
/// devices.  The parameters work exactly like the device and head
/// properties of input devices.  If @device is missing, only devices
/// that have no input routing config are admissible.  If @device is
/// specified, both input devices with and without input routing config
/// are admissible, but devices with input routing config take
/// precedence.
#[qapi(name = "input-send-event")]
#[qapi(since = "2.6")]
#[qapi(returns = "()")]
pub struct InputSendEvent {
    /// display device to send event(s) to.
    #[qapi(name = "device")]
    pub device: Option<String>,
    /// head to send event(s) to, in case the display device supports
    /// multiple scanouts.
    #[qapi(name = "head")]
    pub head: Option<i64>,
    /// List of InputEvent union.
    #[qapi(name = "events")]
    pub events: Vec<InputEvent>,
}
/// GTK display options.
#[qapi(name = "DisplayGTK")]
#[qapi(since = "2.12")]
pub struct DisplayGtk {
    /// Grab keyboard input on mouse hover.
    #[qapi(name = "grab-on-hover")]
    pub grab_on_hover: Option<bool>,
    /// Zoom guest display to fit into the host window.  When
    /// turned off the host window will be resized instead.  In case the
    /// display device can notify the guest on window resizes
    /// (virtio-gpu) this will default to "on", assuming the guest will
    /// resize the display to match the window size then.  Otherwise it
    /// defaults to "off".  (Since 3.1)
    #[qapi(name = "zoom-to-fit")]
    pub zoom_to_fit: Option<bool>,
    /// Display the tab bar for switching between the various
    /// graphical interfaces (e.g. VGA and virtual console character
    /// devices) by default.  (Since 7.1)
    #[qapi(name = "show-tabs")]
    pub show_tabs: Option<bool>,
    /// Display the main window menubar.  Defaults to "on".
    /// (Since 8.0)
    #[qapi(name = "show-menubar")]
    pub show_menubar: Option<bool>,
}
/// EGL headless display options.
#[qapi(name = "DisplayEGLHeadless")]
#[qapi(since = "3.1")]
pub struct DisplayEglHeadless {
    /// Which DRM render node should be used.  Default is the
    /// first available node on the host.
    #[qapi(name = "rendernode")]
    pub rendernode: Option<String>,
}
/// DBus display options.
#[qapi(name = "DisplayDBus")]
#[qapi(since = "7.0")]
pub struct DisplayDBus {
    /// Which DRM render node should be used.  Default is the
    /// first available node on the host.
    #[qapi(name = "rendernode")]
    pub rendernode: Option<String>,
    /// The D-Bus bus address (default to the session bus).
    #[qapi(name = "addr")]
    pub addr: Option<String>,
    /// Whether to use peer-to-peer connections (accepted through
    /// @add_client).
    #[qapi(name = "p2p")]
    pub p2p: Option<bool>,
    /// Use the specified DBus audiodev to export audio.
    #[qapi(name = "audiodev")]
    pub audiodev: Option<String>,
}
/// Display OpenGL mode.
#[qapi(name = "DisplayGLMode")]
#[qapi(since = "3.0")]
pub enum DisplayGlMode {
    /// Disable OpenGL (default).
    #[qapi(name = "off")]
    Off,
    /// Use OpenGL, pick context type automatically.  Would better be
    /// named 'auto' but is called 'on' for backward compatibility with
    /// bool type.
    #[qapi(name = "on")]
    On,
    /// Use OpenGL with Core (desktop) Context.
    #[qapi(name = "core")]
    Core,
    /// Use OpenGL with ES (embedded systems) Context.
    #[qapi(name = "es")]
    Es,
}
/// Curses display options.
#[qapi(name = "DisplayCurses")]
#[qapi(since = "4.0")]
pub struct DisplayCurses {
    /// Font charset used by guest (default: CP437).
    #[qapi(name = "charset")]
    pub charset: Option<String>,
}
/// Cocoa display options.
#[qapi(name = "DisplayCocoa")]
#[qapi(since = "7.0")]
pub struct DisplayCocoa {
    /// Enable/disable forwarding of left command key to
    /// guest.  Allows command-tab window switching on the host without
    /// sending this key to the guest when "off".  Defaults to "on"
    #[qapi(name = "left-command-key")]
    pub left_command_key: Option<bool>,
    /// Capture all key presses, including system combos.  This
    /// requires accessibility permissions, since it performs a global
    /// grab on key events.  (default: off)  See
    /// https://support.apple.com/en-in/guide/mac-help/mh32356/mac
    #[qapi(name = "full-grab")]
    pub full_grab: Option<bool>,
    /// Swap the Option and Command keys so that their key
    /// codes match their position on non-Mac keyboards and you can use
    /// Meta/Super and Alt where you expect them.  (default: off)
    #[qapi(name = "swap-opt-cmd")]
    pub swap_opt_cmd: Option<bool>,
    /// Zoom guest display to fit into the host window.  When
    /// turned off the host window will be resized instead.  Defaults to
    /// "off".  (Since 8.2)
    #[qapi(name = "zoom-to-fit")]
    pub zoom_to_fit: Option<bool>,
    /// Apply interpolation to smooth output when
    /// zoom-to-fit is enabled.  Defaults to "off".  (Since 9.0)
    #[qapi(name = "zoom-interpolation")]
    pub zoom_interpolation: Option<bool>,
}
/// Set of modifier keys that need to be held for shortcut key actions.
#[qapi(name = "HotKeyMod")]
#[qapi(since = "7.1")]
pub enum HotKeyMod {
    #[qapi(name = "lctrl-lalt")]
    LctrlLalt,
    #[qapi(name = "lshift-lctrl-lalt")]
    LshiftLctrlLalt,
    #[qapi(name = "rctrl")]
    Rctrl,
}
/// SDL2 display options.
#[qapi(name = "DisplaySDL")]
#[qapi(since = "7.1")]
pub struct DisplaySdl {
    /// Modifier keys that should be pressed together with the
    /// "G" key to release the mouse grab.
    #[qapi(name = "grab-mod")]
    pub grab_mod: Option<HotKeyMod>,
}
/// Display (user interface) type.
#[qapi(name = "DisplayType")]
#[qapi(since = "2.12")]
pub enum DisplayType {
    /// The default user interface, selecting from the first
    /// available of gtk, sdl, cocoa, and vnc.
    #[qapi(name = "default")]
    Default,
    /// No user interface or video output display.  The guest will
    /// still see an emulated graphics card, but its output will not be
    /// displayed to the QEMU user.
    #[qapi(name = "none")]
    None,
    /// The GTK user interface.
    #[qapi(name = "gtk")]
    #[qapi(condition = "CONFIG_GTK")]
    Gtk,
    /// The SDL user interface.
    #[qapi(name = "sdl")]
    #[qapi(condition = "CONFIG_SDL")]
    Sdl,
    /// No user interface, offload GL operations to a local
    /// DRI device.  Graphical display need to be paired with VNC or
    /// Spice.  (Since 3.1)
    #[qapi(name = "egl-headless")]
    #[qapi(condition = "CONFIG_OPENGL")]
    EglHeadless,
    /// Display video output via curses.  For graphics device
    /// models which support a text mode, QEMU can display this output
    /// using a curses/ncurses interface.  Nothing is displayed when the
    /// graphics device is in graphical mode or if the graphics device
    /// does not support a text mode.  Generally only the VGA device
    /// models support text mode.
    #[qapi(name = "curses")]
    #[qapi(condition = "CONFIG_CURSES")]
    Curses,
    /// The Cocoa user interface.
    #[qapi(name = "cocoa")]
    #[qapi(condition = "CONFIG_COCOA")]
    Cocoa,
    /// Set up a Spice server and run the default associated
    /// application to connect to it.  The server will redirect the
    /// serial console and QEMU monitors.  (Since 4.0)
    #[qapi(name = "spice-app")]
    #[qapi(condition = "CONFIG_SPICE")]
    SpiceApp,
    /// Start a D-Bus service for the display.  (Since 7.0)
    #[qapi(name = "dbus")]
    #[qapi(condition = "CONFIG_DBUS_DISPLAY")]
    Dbus,
}
pub enum DisplayOptionsBranch {
    #[qapi(name = "gtk")]
    #[qapi(condition = "CONFIG_GTK")]
    Gtk(DisplayGtk),
    #[qapi(name = "cocoa")]
    #[qapi(condition = "CONFIG_COCOA")]
    Cocoa(DisplayCocoa),
    #[qapi(name = "curses")]
    #[qapi(condition = "CONFIG_CURSES")]
    Curses(DisplayCurses),
    #[qapi(name = "egl-headless")]
    #[qapi(condition = "CONFIG_OPENGL")]
    EglHeadless(DisplayEglHeadless),
    #[qapi(name = "dbus")]
    #[qapi(condition = "CONFIG_DBUS_DISPLAY")]
    Dbus(DisplayDBus),
    #[qapi(name = "sdl")]
    #[qapi(condition = "CONFIG_SDL")]
    Sdl(DisplaySdl),
}
/// Display (user interface) options.
#[qapi(name = "DisplayOptions")]
#[qapi(since = "2.12")]
pub struct DisplayOptions {
    /// Which DisplayType qemu should use.
    #[qapi(name = "type")]
    #[qapi(discriminator)]
    pub r#type: DisplayType,
    /// Start user interface in fullscreen mode
    /// (default: off).
    #[qapi(name = "full-screen")]
    pub full_screen: Option<bool>,
    /// Allow to quit qemu with window close button
    /// (default: on).
    #[qapi(name = "window-close")]
    pub window_close: Option<bool>,
    /// Force showing the mouse cursor (default: off).
    /// (since: 5.0)
    #[qapi(name = "show-cursor")]
    pub show_cursor: Option<bool>,
    /// Enable OpenGL support (default: off).
    #[qapi(name = "gl")]
    pub gl: Option<DisplayGlMode>,
    #[qapi(union)]
    pub u: Option<DisplayOptionsBranch>,
}
/// Returns information about display configuration
#[qapi(name = "query-display-options")]
#[qapi(since = "3.1")]
#[qapi(returns = "DisplayOptions")]
pub struct QueryDisplayOptions {}
/// Available DisplayReload types.
#[qapi(name = "DisplayReloadType")]
#[qapi(since = "6.0")]
pub enum DisplayReloadType {
    /// VNC display
    #[qapi(name = "vnc")]
    Vnc,
}
/// Specify the VNC reload options.
#[qapi(name = "DisplayReloadOptionsVNC")]
#[qapi(since = "6.0")]
pub struct DisplayReloadOptionsVnc {
    /// reload tls certs or not.
    #[qapi(name = "tls-certs")]
    pub tls_certs: Option<bool>,
}
pub enum DisplayReloadOptionsBranch {
    #[qapi(name = "vnc")]
    Vnc(DisplayReloadOptionsVnc),
}
/// Options of the display configuration reload.
#[qapi(name = "DisplayReloadOptions")]
#[qapi(since = "6.0")]
pub struct DisplayReloadOptions {
    /// Specify the display type.
    #[qapi(name = "type")]
    #[qapi(discriminator)]
    pub r#type: DisplayReloadType,
    #[qapi(union)]
    pub u: Option<DisplayReloadOptionsBranch>,
}
/// Reload display configuration.
#[qapi(name = "display-reload")]
#[qapi(since = "6.0")]
#[qapi(returns = "()")]
pub struct DisplayReload {
    #[qapi(flatten)]
    pub data: DisplayReloadOptions,
}
/// Available DisplayUpdate types.
#[qapi(name = "DisplayUpdateType")]
#[qapi(since = "7.1")]
pub enum DisplayUpdateType {
    /// VNC display
    #[qapi(name = "vnc")]
    Vnc,
}
/// Specify the VNC reload options.
#[qapi(name = "DisplayUpdateOptionsVNC")]
#[qapi(since = "7.1")]
pub struct DisplayUpdateOptionsVnc {
    /// If specified, change set of addresses to listen for
    /// connections.  Addresses configured for websockets are not
    /// touched.
    #[qapi(name = "addresses")]
    pub addresses: Option<Vec<SocketAddress>>,
}
pub enum DisplayUpdateOptionsBranch {
    #[qapi(name = "vnc")]
    Vnc(DisplayUpdateOptionsVnc),
}
/// Options of the display configuration reload.
#[qapi(name = "DisplayUpdateOptions")]
#[qapi(since = "7.1")]
pub struct DisplayUpdateOptions {
    /// Specify the display type.
    #[qapi(name = "type")]
    #[qapi(discriminator)]
    pub r#type: DisplayUpdateType,
    #[qapi(union)]
    pub u: Option<DisplayUpdateOptionsBranch>,
}
/// Update display configuration.
#[qapi(name = "display-update")]
#[qapi(since = "7.1")]
#[qapi(returns = "()")]
pub struct DisplayUpdate {
    #[qapi(flatten)]
    pub data: DisplayUpdateOptions,
}
/// Set migration information for remote display.  This makes the server
/// ask the client to automatically reconnect using the new parameters
/// once migration finished successfully.  Only implemented for SPICE.
#[qapi(name = "client_migrate_info")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct ClientMigrateInfo {
    /// must be "spice"
    #[qapi(name = "protocol")]
    pub protocol: String,
    /// migration target hostname
    #[qapi(name = "hostname")]
    pub hostname: String,
    /// spice tcp port for plaintext channels
    #[qapi(name = "port")]
    pub port: Option<i64>,
    /// spice tcp port for tls-secured channels
    #[qapi(name = "tls-port")]
    pub tls_port: Option<i64>,
    /// server certificate subject
    #[qapi(name = "cert-subject")]
    pub cert_subject: Option<String>,
}
// path end:	qapi/ui.json
// path begin:	qapi/authz.json
/// The authorization policy result
#[qapi(name = "QAuthZListPolicy")]
#[qapi(since = "4.0")]
pub enum QAuthZListPolicy {
    /// deny access
    #[qapi(name = "deny")]
    Deny,
    /// allow access
    #[qapi(name = "allow")]
    Allow,
}
/// The authorization policy match format
#[qapi(name = "QAuthZListFormat")]
#[qapi(since = "4.0")]
pub enum QAuthZListFormat {
    /// an exact string match
    #[qapi(name = "exact")]
    Exact,
    /// string with ? and * shell wildcard support
    #[qapi(name = "glob")]
    Glob,
}
/// A single authorization rule.
#[qapi(name = "QAuthZListRule")]
#[qapi(since = "4.0")]
pub struct QAuthZListRule {
    /// a string or glob to match against a user identity
    #[qapi(name = "match")]
    pub r#match: String,
    /// the result to return if @match evaluates to true
    #[qapi(name = "policy")]
    pub policy: QAuthZListPolicy,
    /// the format of the @match rule (default 'exact')
    #[qapi(name = "format")]
    pub format: Option<QAuthZListFormat>,
}
/// Properties for authz-list objects.
#[qapi(name = "AuthZListProperties")]
#[qapi(since = "4.0")]
pub struct AuthZListProperties {
    /// Default policy to apply when no rule matches (default:
    /// deny)
    #[qapi(name = "policy")]
    pub policy: Option<QAuthZListPolicy>,
    /// Authorization rules based on matching user
    #[qapi(name = "rules")]
    pub rules: Option<Vec<QAuthZListRule>>,
}
/// Properties for authz-listfile objects.
#[qapi(name = "AuthZListFileProperties")]
#[qapi(since = "4.0")]
pub struct AuthZListFileProperties {
    /// File name to load the configuration from.  The file must
    /// contain valid JSON for AuthZListProperties.
    #[qapi(name = "filename")]
    pub filename: String,
    /// If true, inotify is used to monitor the file,
    /// automatically reloading changes.  If an error occurs during
    /// reloading, all authorizations will fail until the file is next
    /// successfully loaded.  (default: true if the binary was built
    /// with CONFIG_INOTIFY1, false otherwise)
    #[qapi(name = "refresh")]
    pub refresh: Option<bool>,
}
/// Properties for authz-pam objects.
#[qapi(name = "AuthZPAMProperties")]
#[qapi(since = "4.0")]
pub struct AuthZpamProperties {
    /// PAM service name to use for authorization
    #[qapi(name = "service")]
    pub service: String,
}
/// Properties for authz-simple objects.
#[qapi(name = "AuthZSimpleProperties")]
#[qapi(since = "4.0")]
pub struct AuthZSimpleProperties {
    /// Identifies the allowed user.  Its format depends on the
    /// network service that authorization object is associated with.
    /// For authorizing based on TLS x509 certificates, the identity
    /// must be the x509 distinguished name.
    #[qapi(name = "identity")]
    pub identity: String,
}
// path end:	qapi/authz.json
// path begin:	qapi/migration.json
/// Detailed migration status.
#[qapi(name = "MigrationStats")]
#[qapi(since = "0.14")]
pub struct MigrationStats {
    /// amount of bytes already transferred to the target VM
    #[qapi(name = "transferred")]
    pub transferred: i64,
    /// amount of bytes remaining to be transferred to the
    /// target VM
    #[qapi(name = "remaining")]
    pub remaining: i64,
    /// total amount of bytes involved in the migration process
    #[qapi(name = "total")]
    pub total: i64,
    /// number of duplicate (zero) pages (since 1.2)
    #[qapi(name = "duplicate")]
    pub duplicate: i64,
    /// number of normal pages (since 1.2)
    #[qapi(name = "normal")]
    pub normal: i64,
    /// number of normal bytes sent (since 1.2)
    #[qapi(name = "normal-bytes")]
    pub normal_bytes: i64,
    /// number of pages dirtied by second by the guest
    /// (since 1.3)
    #[qapi(name = "dirty-pages-rate")]
    pub dirty_pages_rate: i64,
    /// throughput in megabits/sec.  (since 1.6)
    #[qapi(name = "mbps")]
    pub mbps: f64,
    /// number of times that dirty ram was synchronized
    /// (since 2.1)
    #[qapi(name = "dirty-sync-count")]
    pub dirty_sync_count: i64,
    /// The number of page requests received from the
    /// destination (since 2.7)
    #[qapi(name = "postcopy-requests")]
    pub postcopy_requests: i64,
    /// The number of bytes per page for the various page-based
    /// statistics (since 2.10)
    #[qapi(name = "page-size")]
    pub page_size: i64,
    /// The number of bytes sent through multifd (since 3.0)
    #[qapi(name = "multifd-bytes")]
    pub multifd_bytes: u64,
    /// the number of memory pages transferred per second
    /// (Since 4.0)
    #[qapi(name = "pages-per-second")]
    pub pages_per_second: u64,
    /// The number of bytes sent in the pre-copy phase
    /// (since 7.0).
    #[qapi(name = "precopy-bytes")]
    pub precopy_bytes: u64,
    /// The number of bytes sent while the guest is paused
    /// (since 7.0).
    #[qapi(name = "downtime-bytes")]
    pub downtime_bytes: u64,
    /// The number of bytes sent during the post-copy phase
    /// (since 7.0).
    #[qapi(name = "postcopy-bytes")]
    pub postcopy_bytes: u64,
    /// Number of times dirty RAM
    /// synchronization could not avoid copying dirty pages.  This is
    /// between 0 and @dirty-sync-count * @multifd-channels.  (since
    /// 7.1)
    #[qapi(name = "dirty-sync-missed-zero-copy")]
    pub dirty_sync_missed_zero_copy: u64,
}
/// Detailed XBZRLE migration cache statistics
#[qapi(name = "XBZRLECacheStats")]
#[qapi(since = "1.2")]
pub struct XbzrleCacheStats {
    /// XBZRLE cache size
    #[qapi(name = "cache-size")]
    pub cache_size: u64,
    /// amount of bytes already transferred to the target VM
    #[qapi(name = "bytes")]
    pub bytes: i64,
    /// amount of pages transferred to the target VM
    #[qapi(name = "pages")]
    pub pages: i64,
    /// number of cache miss
    #[qapi(name = "cache-miss")]
    pub cache_miss: i64,
    /// rate of cache miss (since 2.1)
    #[qapi(name = "cache-miss-rate")]
    pub cache_miss_rate: f64,
    /// rate of encoded bytes (since 5.1)
    #[qapi(name = "encoding-rate")]
    pub encoding_rate: f64,
    /// number of overflows
    #[qapi(name = "overflow")]
    pub overflow: i64,
}
/// Detailed migration compression statistics
#[qapi(name = "CompressionStats")]
#[qapi(since = "3.1")]
pub struct CompressionStats {
    /// amount of pages compressed and transferred to the target VM
    #[qapi(name = "pages")]
    pub pages: i64,
    /// count of times that no free thread was available to compress
    /// data
    #[qapi(name = "busy")]
    pub busy: i64,
    /// rate of thread busy
    #[qapi(name = "busy-rate")]
    pub busy_rate: f64,
    /// amount of bytes after compression
    #[qapi(name = "compressed-size")]
    pub compressed_size: i64,
    /// rate of compressed size
    #[qapi(name = "compression-rate")]
    pub compression_rate: f64,
}
/// An enumeration of migration status.
#[qapi(name = "MigrationStatus")]
#[qapi(since = "2.3")]
pub enum MigrationStatus {
    /// no migration has ever happened.
    #[qapi(name = "none")]
    None,
    /// migration process has been initiated.
    #[qapi(name = "setup")]
    Setup,
    /// in the process of cancelling migration.
    #[qapi(name = "cancelling")]
    Cancelling,
    /// cancelling migration is finished.
    #[qapi(name = "cancelled")]
    Cancelled,
    /// in the process of doing migration.
    #[qapi(name = "active")]
    Active,
    /// like active, but now in postcopy mode.  (since
    /// 2.5)
    #[qapi(name = "postcopy-active")]
    PostcopyActive,
    /// during postcopy but paused.  (since 3.0)
    #[qapi(name = "postcopy-paused")]
    PostcopyPaused,
    /// setup phase for a postcopy recovery
    /// process, preparing for a recovery phase to start.  (since 9.1)
    #[qapi(name = "postcopy-recover-setup")]
    PostcopyRecoverSetup,
    /// trying to recover from a paused postcopy.  (since
    /// 3.0)
    #[qapi(name = "postcopy-recover")]
    PostcopyRecover,
    /// migration is finished.
    #[qapi(name = "completed")]
    Completed,
    /// some error occurred during migration process.
    #[qapi(name = "failed")]
    Failed,
    /// VM is in the process of fault tolerance, VM can not get into
    /// this state unless colo capability is enabled for migration.
    /// (since 2.8)
    #[qapi(name = "colo")]
    Colo,
    /// Paused before device serialisation.  (since 2.11)
    #[qapi(name = "pre-switchover")]
    PreSwitchover,
    /// During device serialisation when pause-before-switchover is
    /// enabled (since 2.11)
    #[qapi(name = "device")]
    Device,
    /// wait for device unplug request by guest OS to be
    /// completed.  (since 4.2)
    #[qapi(name = "wait-unplug")]
    WaitUnplug,
}
/// Detailed VFIO devices migration statistics
#[qapi(name = "VfioStats")]
#[qapi(since = "5.2")]
pub struct VfioStats {
    /// amount of bytes transferred to the target VM by VFIO
    /// devices
    #[qapi(name = "transferred")]
    pub transferred: i64,
}
/// Information about current migration process.
#[qapi(name = "MigrationInfo")]
#[qapi(since = "0.14")]
pub struct MigrationInfo {
    /// @MigrationStatus describing the current migration status.
    /// If this field is not returned, no migration process has been
    /// initiated
    #[qapi(name = "status")]
    pub status: Option<MigrationStatus>,
    /// @MigrationStats containing detailed migration status, only
    /// returned if status is 'active' or 'completed'(since 1.2)
    #[qapi(name = "ram")]
    pub ram: Option<MigrationStats>,
    /// @VfioStats containing detailed VFIO devices migration
    /// statistics, only returned if VFIO device is present, migration
    /// is supported by all VFIO devices and status is 'active' or
    /// 'completed' (since 5.2)
    #[qapi(name = "vfio")]
    pub vfio: Option<VfioStats>,
    /// @XBZRLECacheStats containing detailed XBZRLE
    /// migration statistics, only returned if XBZRLE feature is on and
    /// status is 'active' or 'completed' (since 1.2)
    #[qapi(name = "xbzrle-cache")]
    pub xbzrle_cache: Option<XbzrleCacheStats>,
    /// total amount of milliseconds since migration started.
    /// If migration has ended, it returns the total migration time.
    /// (since 1.2)
    #[qapi(name = "total-time")]
    pub total_time: Option<i64>,
    /// only present while migration is active expected
    /// downtime in milliseconds for the guest in last walk of the dirty
    /// bitmap.  (since 1.3)
    #[qapi(name = "expected-downtime")]
    pub expected_downtime: Option<i64>,
    /// only present when migration finishes correctly total
    /// downtime in milliseconds for the guest.  (since 1.3)
    #[qapi(name = "downtime")]
    pub downtime: Option<i64>,
    /// amount of setup time in milliseconds *before* the
    /// iterations begin but *after* the QMP command is issued.  This is
    /// designed to provide an accounting of any activities (such as
    /// RDMA pinning) which may be expensive, but do not actually occur
    /// during the iterative migration rounds themselves.  (since 1.6)
    #[qapi(name = "setup-time")]
    pub setup_time: Option<i64>,
    /// percentage of time guest cpus are being
    /// throttled during auto-converge.  This is only present when
    /// auto-converge has started throttling guest cpus.  (Since 2.7)
    #[qapi(name = "cpu-throttle-percentage")]
    pub cpu_throttle_percentage: Option<i64>,
    /// the human readable error description string.  Clients
    /// should not attempt to parse the error strings.  (Since 2.7)
    #[qapi(name = "error-desc")]
    pub error_desc: Option<String>,
    /// A list of reasons an outgoing migration is
    /// blocked.  Present and non-empty when migration is blocked.
    /// (since 6.0)
    #[qapi(name = "blocked-reasons")]
    pub blocked_reasons: Option<Vec<String>>,
    /// total time when all vCPU were blocked during
    /// postcopy live migration.  This is only present when the
    /// postcopy-blocktime migration capability is enabled.  (Since 3.0)
    #[qapi(name = "postcopy-blocktime")]
    pub postcopy_blocktime: Option<u32>,
    /// list of the postcopy blocktime per vCPU.
    /// This is only present when the postcopy-blocktime migration
    /// capability is enabled.  (Since 3.0)
    #[qapi(name = "postcopy-vcpu-blocktime")]
    pub postcopy_vcpu_blocktime: Option<Vec<u32>>,
    /// Only used for tcp, to know what the real port is
    /// (Since 4.0)
    #[qapi(name = "socket-address")]
    pub socket_address: Option<Vec<SocketAddress>>,
    /// Maximum throttle time (in
    /// microseconds) of virtual CPUs each dirty ring full round, which
    /// shows how MigrationCapability dirty-limit affects the guest
    /// during live migration.  (Since 8.1)
    #[qapi(name = "dirty-limit-throttle-time-per-round")]
    pub dirty_limit_throttle_time_per_round: Option<u64>,
    /// Estimated average dirty ring full time
    /// (in microseconds) for each dirty ring full round.  The value
    /// equals the dirty ring memory size divided by the average dirty
    /// page rate of the virtual CPU, which can be used to observe the
    /// average memory load of the virtual CPU indirectly.  Note that
    /// zero means guest doesn't dirty memory.  (Since 8.1)
    #[qapi(name = "dirty-limit-ring-full-time")]
    pub dirty_limit_ring_full_time: Option<u64>,
}
/// Returns information about current migration process.  If migration
/// is active there will be another json-object with RAM migration
/// status.
#[qapi(name = "query-migrate")]
#[qapi(since = "0.14")]
#[qapi(returns = "MigrationInfo")]
pub struct QueryMigrate {}
/// Migration capabilities enumeration
#[qapi(name = "MigrationCapability")]
#[qapi(since = "1.2")]
pub enum MigrationCapability {
    /// Migration supports xbzrle (Xor Based Zero Run Length
    /// Encoding).  This feature allows us to minimize migration traffic
    /// for certain work loads, by sending compressed difference of the
    /// pages
    #[qapi(name = "xbzrle")]
    Xbzrle,
    /// Controls whether or not the entire VM memory
    /// footprint is mlock()'d on demand or all at once.  Refer to
    /// docs/rdma.txt for usage.  Disabled by default.  (since 2.0)
    #[qapi(name = "rdma-pin-all")]
    RdmaPinAll,
    /// If enabled, QEMU will automatically throttle down
    /// the guest to speed up convergence of RAM migration.  (since 1.6)
    #[qapi(name = "auto-converge")]
    AutoConverge,
    /// During storage migration encode blocks of zeroes
    /// efficiently.  This essentially saves 1MB of zeroes per block on
    /// the wire.  Enabling requires source and target VM to support
    /// this feature.  To enable it is sufficient to enable the
    /// capability on the source VM.  The feature is disabled by
    /// default.  (since 1.6)
    #[qapi(name = "zero-blocks")]
    ZeroBlocks,
    /// generate events for each migration state change (since 2.4)
    #[qapi(name = "events")]
    Events,
    /// Start executing on the migration target before all of
    /// RAM has been migrated, pulling the remaining pages along as
    /// needed.  The capacity must have the same setting on both source
    /// and target or migration will not even start.  NOTE: If the
    /// migration fails during postcopy the VM will fail.  (since 2.6)
    #[qapi(name = "postcopy-ram")]
    PostcopyRam,
    /// If enabled, migration will never end, and the state of the
    /// VM on the primary side will be migrated continuously to the VM
    /// on secondary side, this process is called COarse-Grain LOck
    /// Stepping (COLO) for Non-stop Service.  (since 2.8)
    #[qapi(name = "x-colo")]
    #[qapi(feature = "unstable")]
    XColo,
    /// if enabled, qemu will free the migrated ram pages on
    /// the source during postcopy-ram migration.  (since 2.9)
    #[qapi(name = "release-ram")]
    ReleaseRam,
    /// If enabled, migration will use the return path even
    /// for precopy.  (since 2.10)
    #[qapi(name = "return-path")]
    ReturnPath,
    /// Pause outgoing migration before
    /// serialising device state and before disabling block IO (since
    /// 2.11)
    #[qapi(name = "pause-before-switchover")]
    PauseBeforeSwitchover,
    /// Use more than one fd for migration (since 4.0)
    #[qapi(name = "multifd")]
    Multifd,
    /// If enabled, QEMU will migrate named dirty bitmaps.
    /// (since 2.12)
    #[qapi(name = "dirty-bitmaps")]
    DirtyBitmaps,
    /// Calculate downtime for postcopy live migration
    /// (since 3.0)
    #[qapi(name = "postcopy-blocktime")]
    PostcopyBlocktime,
    /// If enabled, the destination will not activate
    /// block devices (and thus take locks) immediately at the end of
    /// migration.  (since 3.0)
    #[qapi(name = "late-block-activate")]
    LateBlockActivate,
    /// If enabled, QEMU will not migrate shared memory
    /// that is accessible on the destination machine.  (since 4.0)
    #[qapi(name = "x-ignore-shared")]
    #[qapi(feature = "unstable")]
    XIgnoreShared,
    /// Send the UUID of the source to allow the destination
    /// to ensure it is the same.  (since 4.2)
    #[qapi(name = "validate-uuid")]
    ValidateUuid,
    /// If enabled, the migration stream will be a
    /// snapshot of the VM exactly at the point when the migration
    /// procedure starts.  The VM RAM is saved with running VM.
    /// (since 6.0)
    #[qapi(name = "background-snapshot")]
    BackgroundSnapshot,
    /// Controls behavior on sending memory pages on
    /// migration.  When true, enables a zero-copy mechanism for sending
    /// memory pages, if host supports it.  Requires that QEMU be
    /// permitted to use locked memory for guest RAM pages.  (since 7.1)
    #[qapi(name = "zero-copy-send")]
    ZeroCopySend,
    /// If enabled, the migration process will allow
    /// postcopy requests to preempt precopy stream, so postcopy
    /// requests will be handled faster.  This is a performance feature
    /// and should not affect the correctness of postcopy migration.
    /// (since 7.1)
    #[qapi(name = "postcopy-preempt")]
    PostcopyPreempt,
    /// If enabled, migration will not stop the source VM
    /// and complete the migration until an ACK is received from the
    /// destination that it's OK to do so.  Exactly when this ACK is
    /// sent depends on the migrated devices that use this feature.  For
    /// example, a device can use it to make sure some of its data is
    /// sent and loaded in the destination before doing switchover.
    /// This can reduce downtime if devices that support this capability
    /// are present.  'return-path' capability must be enabled to use
    /// it.  (since 8.1)
    #[qapi(name = "switchover-ack")]
    SwitchoverAck,
    /// If enabled, migration will throttle vCPUs as needed to
    /// keep their dirty page rate within @vcpu-dirty-limit.  This can
    /// improve responsiveness of large guests during live migration,
    /// and can result in more stable read performance.  Requires KVM
    /// with accelerator property "dirty-ring-size" set.  (Since 8.1)
    #[qapi(name = "dirty-limit")]
    DirtyLimit,
    /// Migrate using fixed offsets in the migration file for
    /// each RAM page.  Requires a migration URI that supports seeking,
    /// such as a file.  (since 9.0)
    #[qapi(name = "mapped-ram")]
    MappedRam,
}
/// Migration capability information
#[qapi(name = "MigrationCapabilityStatus")]
#[qapi(since = "1.2")]
pub struct MigrationCapabilityStatus {
    /// capability enum
    #[qapi(name = "capability")]
    pub capability: MigrationCapability,
    /// capability state bool
    #[qapi(name = "state")]
    pub state: bool,
}
/// Enable/Disable the following migration capabilities (like xbzrle)
#[qapi(name = "migrate-set-capabilities")]
#[qapi(since = "1.2")]
#[qapi(returns = "()")]
pub struct MigrateSetCapabilities {
    /// json array of capability modifications to make
    #[qapi(name = "capabilities")]
    pub capabilities: Vec<MigrationCapabilityStatus>,
}
/// Returns information about the current migration capabilities status
#[qapi(name = "query-migrate-capabilities")]
#[qapi(since = "1.2")]
#[qapi(returns = "Vec<MigrationCapabilityStatus>")]
pub struct QueryMigrateCapabilities {}
/// An enumeration of multifd compression methods.
#[qapi(name = "MultiFDCompression")]
#[qapi(since = "5.0")]
pub enum MultiFdCompression {
    /// no compression.
    #[qapi(name = "none")]
    None,
    /// use zlib compression method.
    #[qapi(name = "zlib")]
    Zlib,
    /// use zstd compression method.
    #[qapi(name = "zstd")]
    #[qapi(condition = "CONFIG_ZSTD")]
    Zstd,
    /// use qpl compression method.  Query Processing Library(qpl) is
    /// based on the deflate compression algorithm and use the Intel
    /// In-Memory Analytics Accelerator(IAA) accelerated compression and
    /// decompression.  (Since 9.1)
    #[qapi(name = "qpl")]
    #[qapi(condition = "CONFIG_QPL")]
    Qpl,
    /// use UADK library compression method.  (Since 9.1)
    #[qapi(name = "uadk")]
    #[qapi(condition = "CONFIG_UADK")]
    Uadk,
}
#[qapi(name = "MigMode")]
pub enum MigMode {
    /// the original form of migration.  (since 8.2)
    #[qapi(name = "normal")]
    Normal,
    /// The migrate command stops the VM and saves state to the
    /// URI.  After quitting QEMU, the user resumes by running QEMU
    /// -incoming.
    ///
    /// This mode allows the user to quit QEMU, optionally update and
    /// reboot the OS, and restart QEMU.  If the user reboots, the URI
    /// must persist across the reboot, such as by using a file.
    ///
    /// Unlike normal mode, the use of certain local storage options
    /// does not block the migration, but the user must not modify the
    /// contents of guest block devices between the quit and restart.
    ///
    /// This mode supports VFIO devices provided the user first puts the
    /// guest in the suspended runstate, such as by issuing
    /// guest-suspend-ram to the QEMU guest agent.
    ///
    /// Best performance is achieved when the memory backend is shared
    /// and the @x-ignore-shared migration capability is set, but this
    /// is not required.  Further, if the user reboots before restarting
    /// such a configuration, the shared memory must persist across the
    /// reboot, such as by backing it with a dax device.
    ///
    /// @cpr-reboot may not be used with postcopy, background-snapshot,
    /// or COLO.
    ///
    /// (since 8.2)
    #[qapi(name = "cpr-reboot")]
    CprReboot,
}
#[qapi(name = "ZeroPageDetection")]
#[qapi(since = "9.0")]
pub enum ZeroPageDetection {
    /// Do not perform zero page checking.
    #[qapi(name = "none")]
    None,
    /// Perform zero page checking in main migration thread.
    #[qapi(name = "legacy")]
    Legacy,
    /// Perform zero page checking in multifd sender thread if
    /// multifd migration is enabled, else in the main migration thread
    /// as for @legacy.
    #[qapi(name = "multifd")]
    Multifd,
}
#[qapi(name = "BitmapMigrationBitmapAliasTransform")]
#[qapi(since = "6.0")]
pub struct BitmapMigrationBitmapAliasTransform {
    /// If present, the bitmap will be made persistent or
    /// transient depending on this parameter.
    #[qapi(name = "persistent")]
    pub persistent: Option<bool>,
}
#[qapi(name = "BitmapMigrationBitmapAlias")]
#[qapi(since = "5.2")]
pub struct BitmapMigrationBitmapAlias {
    /// The name of the bitmap.
    #[qapi(name = "name")]
    pub name: String,
    /// An alias name for migration (for example the bitmap name on
    /// the opposite site).
    #[qapi(name = "alias")]
    pub alias: String,
    /// Allows the modification of the migrated bitmap.  (since
    /// 6.0)
    #[qapi(name = "transform")]
    pub transform: Option<BitmapMigrationBitmapAliasTransform>,
}
/// Maps a block node name and the bitmaps it has to aliases for dirty
/// bitmap migration.
#[qapi(name = "BitmapMigrationNodeAlias")]
#[qapi(since = "5.2")]
pub struct BitmapMigrationNodeAlias {
    /// A block node name.
    #[qapi(name = "node-name")]
    pub node_name: String,
    /// An alias block node name for migration (for example the node
    /// name on the opposite site).
    #[qapi(name = "alias")]
    pub alias: String,
    /// Mappings for the bitmaps on this node.
    #[qapi(name = "bitmaps")]
    pub bitmaps: Vec<BitmapMigrationBitmapAlias>,
}
/// Migration parameters enumeration
#[qapi(name = "MigrationParameter")]
#[qapi(since = "2.4")]
pub enum MigrationParameter {
    /// Initial delay (in milliseconds) before sending
    /// the first announce (Since 4.0)
    #[qapi(name = "announce-initial")]
    AnnounceInitial,
    /// Maximum delay (in milliseconds) between packets in
    /// the announcement (Since 4.0)
    #[qapi(name = "announce-max")]
    AnnounceMax,
    /// Number of self-announce packets sent after
    /// migration (Since 4.0)
    #[qapi(name = "announce-rounds")]
    AnnounceRounds,
    /// Increase in delay (in milliseconds) between
    /// subsequent packets in the announcement (Since 4.0)
    #[qapi(name = "announce-step")]
    AnnounceStep,
    /// The ratio of bytes_dirty_period and
    /// bytes_xfer_period to trigger throttling.  It is expressed as
    /// percentage.  The default value is 50.  (Since 5.0)
    #[qapi(name = "throttle-trigger-threshold")]
    ThrottleTriggerThreshold,
    /// Initial percentage of time guest cpus are
    /// throttled when migration auto-converge is activated.  The
    /// default value is 20.  (Since 2.7)
    #[qapi(name = "cpu-throttle-initial")]
    CpuThrottleInitial,
    /// throttle percentage increase each time
    /// auto-converge detects that migration is not making progress.
    /// The default value is 10.  (Since 2.7)
    #[qapi(name = "cpu-throttle-increment")]
    CpuThrottleIncrement,
    /// Make CPU throttling slower at tail stage At
    /// the tail stage of throttling, the Guest is very sensitive to CPU
    /// percentage while the @cpu-throttle -increment is excessive
    /// usually at tail stage.  If this parameter is true, we will
    /// compute the ideal CPU percentage used by the Guest, which may
    /// exactly make the dirty rate match the dirty rate threshold.
    /// Then we will choose a smaller throttle increment between the one
    /// specified by @cpu-throttle-increment and the one generated by
    /// ideal CPU percentage.  Therefore, it is compatible to
    /// traditional throttling, meanwhile the throttle increment won't
    /// be excessive at tail stage.  The default value is false.  (Since
    /// 5.1)
    #[qapi(name = "cpu-throttle-tailslow")]
    CpuThrottleTailslow,
    /// ID of the 'tls-creds' object that provides credentials
    /// for establishing a TLS connection over the migration data
    /// channel.  On the outgoing side of the migration, the credentials
    /// must be for a 'client' endpoint, while for the incoming side the
    /// credentials must be for a 'server' endpoint.  Setting this to a
    /// non-empty string enables TLS for all migrations.  An empty
    /// string means that QEMU will use plain text mode for migration,
    /// rather than TLS.  (Since 2.7)
    #[qapi(name = "tls-creds")]
    TlsCreds,
    /// migration target's hostname for validating the
    /// server's x509 certificate identity.  If empty, QEMU will use the
    /// hostname from the migration URI, if any.  A non-empty value is
    /// required when using x509 based TLS credentials and the migration
    /// URI does not include a hostname, such as fd: or exec: based
    /// migration.  (Since 2.7)
    ///
    /// Note: empty value works only since 2.9.
    #[qapi(name = "tls-hostname")]
    TlsHostname,
    /// ID of the 'authz' object subclass that provides access
    /// control checking of the TLS x509 certificate distinguished name.
    /// This object is only resolved at time of use, so can be deleted
    /// and recreated on the fly while the migration server is active.
    /// If missing, it will default to denying access (Since 4.0)
    #[qapi(name = "tls-authz")]
    TlsAuthz,
    /// maximum speed for migration, in bytes per second.
    /// (Since 2.8)
    #[qapi(name = "max-bandwidth")]
    MaxBandwidth,
    /// to set the available bandwidth that
    /// migration can use during switchover phase.  NOTE!  This does not
    /// limit the bandwidth during switchover, but only for calculations
    /// when making decisions to switchover.  By default, this value is
    /// zero, which means QEMU will estimate the bandwidth
    /// automatically.  This can be set when the estimated value is not
    /// accurate, while the user is able to guarantee such bandwidth is
    /// available when switching over.  When specified correctly, this
    /// can make the switchover decision much more accurate.
    /// (Since 8.2)
    #[qapi(name = "avail-switchover-bandwidth")]
    AvailSwitchoverBandwidth,
    /// set maximum tolerated downtime for migration.
    /// maximum downtime in milliseconds (Since 2.8)
    #[qapi(name = "downtime-limit")]
    DowntimeLimit,
    /// The delay time (in ms) between two COLO
    /// checkpoints in periodic mode.  (Since 2.8)
    #[qapi(name = "x-checkpoint-delay")]
    #[qapi(feature = "unstable")]
    XCheckpointDelay,
    /// Number of channels used to migrate data in
    /// parallel.  This is the same number that the number of sockets
    /// used for migration.  The default value is 2 (since 4.0)
    #[qapi(name = "multifd-channels")]
    MultifdChannels,
    /// cache size to be used by XBZRLE migration.  It
    /// needs to be a multiple of the target page size and a power of 2
    /// (Since 2.11)
    #[qapi(name = "xbzrle-cache-size")]
    XbzrleCacheSize,
    /// Background transfer bandwidth during
    /// postcopy.  Defaults to 0 (unlimited).  In bytes per second.
    /// (Since 3.0)
    #[qapi(name = "max-postcopy-bandwidth")]
    MaxPostcopyBandwidth,
    /// maximum cpu throttle percentage.  Defaults to 99.
    /// (Since 3.1)
    #[qapi(name = "max-cpu-throttle")]
    MaxCpuThrottle,
    /// Which compression method to use.  Defaults to
    /// none.  (Since 5.0)
    #[qapi(name = "multifd-compression")]
    MultifdCompression,
    /// Set the compression level to be used in live
    /// migration, the compression level is an integer between 0 and 9,
    /// where 0 means no compression, 1 means the best compression
    /// speed, and 9 means best compression ratio which will consume
    /// more CPU.  Defaults to 1.  (Since 5.0)
    #[qapi(name = "multifd-zlib-level")]
    MultifdZlibLevel,
    /// Set the compression level to be used in live
    /// migration, the compression level is an integer between 0 and 20,
    /// where 0 means no compression, 1 means the best compression
    /// speed, and 20 means best compression ratio which will consume
    /// more CPU.  Defaults to 1.  (Since 5.0)
    #[qapi(name = "multifd-zstd-level")]
    MultifdZstdLevel,
    /// Maps block nodes and bitmaps on them to
    /// aliases for the purpose of dirty bitmap migration.  Such aliases
    /// may for example be the corresponding names on the opposite site.
    /// The mapping must be one-to-one, but not necessarily complete: On
    /// the source, unmapped bitmaps and all bitmaps on unmapped nodes
    /// will be ignored.  On the destination, encountering an unmapped
    /// alias in the incoming migration stream will result in a report,
    /// and all further bitmap migration data will then be discarded.
    /// Note that the destination does not know about bitmaps it does
    /// not receive, so there is no limitation or requirement regarding
    /// the number of bitmaps received, or how they are named, or on
    /// which nodes they are placed.  By default (when this parameter
    /// has never been set), bitmap names are mapped to themselves.
    /// Nodes are mapped to their block device name if there is one, and
    /// to their node name otherwise.  (Since 5.2)
    #[qapi(name = "block-bitmap-mapping")]
    BlockBitmapMapping,
    /// Periodic time (in milliseconds) of dirty
    /// limit during live migration.  Should be in the range 1 to
    /// 1000ms.  Defaults to 1000ms.  (Since 8.1)
    #[qapi(name = "x-vcpu-dirty-limit-period")]
    #[qapi(feature = "unstable")]
    XVcpuDirtyLimitPeriod,
    /// Dirtyrate limit (MB/s) during live migration.
    /// Defaults to 1.  (Since 8.1)
    #[qapi(name = "vcpu-dirty-limit")]
    VcpuDirtyLimit,
    /// Migration mode.  See description in @MigMode.  Default is
    /// 'normal'.  (Since 8.2)
    #[qapi(name = "mode")]
    Mode,
    /// Whether and how to detect zero pages.
    /// See description in @ZeroPageDetection.  Default is 'multifd'.
    /// (since 9.0)
    #[qapi(name = "zero-page-detection")]
    ZeroPageDetection,
    /// Open migration files with O_DIRECT when possible.  This
    /// only has effect if the @mapped-ram capability is enabled.
    /// (Since 9.1)
    #[qapi(name = "direct-io")]
    DirectIo,
}
#[qapi(name = "MigrateSetParameters")]
#[qapi(since = "2.4")]
pub struct MigrateSetParameters {
    /// Initial delay (in milliseconds) before sending
    /// the first announce (Since 4.0)
    #[qapi(name = "announce-initial")]
    pub announce_initial: Option<u64>,
    /// Maximum delay (in milliseconds) between packets in
    /// the announcement (Since 4.0)
    #[qapi(name = "announce-max")]
    pub announce_max: Option<u64>,
    /// Number of self-announce packets sent after
    /// migration (Since 4.0)
    #[qapi(name = "announce-rounds")]
    pub announce_rounds: Option<u64>,
    /// Increase in delay (in milliseconds) between
    /// subsequent packets in the announcement (Since 4.0)
    #[qapi(name = "announce-step")]
    pub announce_step: Option<u64>,
    /// The ratio of bytes_dirty_period and
    /// bytes_xfer_period to trigger throttling.  It is expressed as
    /// percentage.  The default value is 50.  (Since 5.0)
    #[qapi(name = "throttle-trigger-threshold")]
    pub throttle_trigger_threshold: Option<u8>,
    /// Initial percentage of time guest cpus are
    /// throttled when migration auto-converge is activated.  The
    /// default value is 20.  (Since 2.7)
    #[qapi(name = "cpu-throttle-initial")]
    pub cpu_throttle_initial: Option<u8>,
    /// throttle percentage increase each time
    /// auto-converge detects that migration is not making progress.
    /// The default value is 10.  (Since 2.7)
    #[qapi(name = "cpu-throttle-increment")]
    pub cpu_throttle_increment: Option<u8>,
    /// Make CPU throttling slower at tail stage At
    /// the tail stage of throttling, the Guest is very sensitive to CPU
    /// percentage while the @cpu-throttle -increment is excessive
    /// usually at tail stage.  If this parameter is true, we will
    /// compute the ideal CPU percentage used by the Guest, which may
    /// exactly make the dirty rate match the dirty rate threshold.
    /// Then we will choose a smaller throttle increment between the one
    /// specified by @cpu-throttle-increment and the one generated by
    /// ideal CPU percentage.  Therefore, it is compatible to
    /// traditional throttling, meanwhile the throttle increment won't
    /// be excessive at tail stage.  The default value is false.  (Since
    /// 5.1)
    #[qapi(name = "cpu-throttle-tailslow")]
    pub cpu_throttle_tailslow: Option<bool>,
    /// ID of the 'tls-creds' object that provides credentials
    /// for establishing a TLS connection over the migration data
    /// channel.  On the outgoing side of the migration, the credentials
    /// must be for a 'client' endpoint, while for the incoming side the
    /// credentials must be for a 'server' endpoint.  Setting this to a
    /// non-empty string enables TLS for all migrations.  An empty
    /// string means that QEMU will use plain text mode for migration,
    /// rather than TLS.  This is the default.  (Since 2.7)
    #[qapi(name = "tls-creds")]
    pub tls_creds: Option<StrOrNull>,
    /// migration target's hostname for validating the
    /// server's x509 certificate identity.  If empty, QEMU will use the
    /// hostname from the migration URI, if any.  A non-empty value is
    /// required when using x509 based TLS credentials and the migration
    /// URI does not include a hostname, such as fd: or exec: based
    /// migration.  (Since 2.7)
    ///
    /// Note: empty value works only since 2.9.
    #[qapi(name = "tls-hostname")]
    pub tls_hostname: Option<StrOrNull>,
    /// ID of the 'authz' object subclass that provides access
    /// control checking of the TLS x509 certificate distinguished name.
    /// This object is only resolved at time of use, so can be deleted
    /// and recreated on the fly while the migration server is active.
    /// If missing, it will default to denying access (Since 4.0)
    #[qapi(name = "tls-authz")]
    pub tls_authz: Option<StrOrNull>,
    /// maximum speed for migration, in bytes per second.
    /// (Since 2.8)
    #[qapi(name = "max-bandwidth")]
    pub max_bandwidth: Option<u64>,
    /// to set the available bandwidth that
    /// migration can use during switchover phase.  NOTE!  This does not
    /// limit the bandwidth during switchover, but only for calculations
    /// when making decisions to switchover.  By default, this value is
    /// zero, which means QEMU will estimate the bandwidth
    /// automatically.  This can be set when the estimated value is not
    /// accurate, while the user is able to guarantee such bandwidth is
    /// available when switching over.  When specified correctly, this
    /// can make the switchover decision much more accurate.
    /// (Since 8.2)
    #[qapi(name = "avail-switchover-bandwidth")]
    pub avail_switchover_bandwidth: Option<u64>,
    /// set maximum tolerated downtime for migration.
    /// maximum downtime in milliseconds (Since 2.8)
    #[qapi(name = "downtime-limit")]
    pub downtime_limit: Option<u64>,
    /// The delay time (in ms) between two COLO
    /// checkpoints in periodic mode.  (Since 2.8)
    #[qapi(name = "x-checkpoint-delay")]
    #[qapi(feature = "unstable")]
    pub x_checkpoint_delay: Option<u32>,
    /// Number of channels used to migrate data in
    /// parallel.  This is the same number that the number of sockets
    /// used for migration.  The default value is 2 (since 4.0)
    #[qapi(name = "multifd-channels")]
    pub multifd_channels: Option<u8>,
    /// cache size to be used by XBZRLE migration.  It
    /// needs to be a multiple of the target page size and a power of 2
    /// (Since 2.11)
    #[qapi(name = "xbzrle-cache-size")]
    pub xbzrle_cache_size: Option<u64>,
    /// Background transfer bandwidth during
    /// postcopy.  Defaults to 0 (unlimited).  In bytes per second.
    /// (Since 3.0)
    #[qapi(name = "max-postcopy-bandwidth")]
    pub max_postcopy_bandwidth: Option<u64>,
    /// maximum cpu throttle percentage.  Defaults to 99.
    /// (Since 3.1)
    #[qapi(name = "max-cpu-throttle")]
    pub max_cpu_throttle: Option<u8>,
    /// Which compression method to use.  Defaults to
    /// none.  (Since 5.0)
    #[qapi(name = "multifd-compression")]
    pub multifd_compression: Option<MultiFdCompression>,
    /// Set the compression level to be used in live
    /// migration, the compression level is an integer between 0 and 9,
    /// where 0 means no compression, 1 means the best compression
    /// speed, and 9 means best compression ratio which will consume
    /// more CPU.  Defaults to 1.  (Since 5.0)
    #[qapi(name = "multifd-zlib-level")]
    pub multifd_zlib_level: Option<u8>,
    /// Set the compression level to be used in live
    /// migration, the compression level is an integer between 0 and 20,
    /// where 0 means no compression, 1 means the best compression
    /// speed, and 20 means best compression ratio which will consume
    /// more CPU.  Defaults to 1.  (Since 5.0)
    #[qapi(name = "multifd-zstd-level")]
    pub multifd_zstd_level: Option<u8>,
    /// Maps block nodes and bitmaps on them to
    /// aliases for the purpose of dirty bitmap migration.  Such aliases
    /// may for example be the corresponding names on the opposite site.
    /// The mapping must be one-to-one, but not necessarily complete: On
    /// the source, unmapped bitmaps and all bitmaps on unmapped nodes
    /// will be ignored.  On the destination, encountering an unmapped
    /// alias in the incoming migration stream will result in a report,
    /// and all further bitmap migration data will then be discarded.
    /// Note that the destination does not know about bitmaps it does
    /// not receive, so there is no limitation or requirement regarding
    /// the number of bitmaps received, or how they are named, or on
    /// which nodes they are placed.  By default (when this parameter
    /// has never been set), bitmap names are mapped to themselves.
    /// Nodes are mapped to their block device name if there is one, and
    /// to their node name otherwise.  (Since 5.2)
    #[qapi(name = "block-bitmap-mapping")]
    pub block_bitmap_mapping: Option<Vec<BitmapMigrationNodeAlias>>,
    /// Periodic time (in milliseconds) of dirty
    /// limit during live migration.  Should be in the range 1 to
    /// 1000ms.  Defaults to 1000ms.  (Since 8.1)
    #[qapi(name = "x-vcpu-dirty-limit-period")]
    #[qapi(feature = "unstable")]
    pub x_vcpu_dirty_limit_period: Option<u64>,
    /// Dirtyrate limit (MB/s) during live migration.
    /// Defaults to 1.  (Since 8.1)
    #[qapi(name = "vcpu-dirty-limit")]
    pub vcpu_dirty_limit: Option<u64>,
    /// Migration mode.  See description in @MigMode.  Default is
    /// 'normal'.  (Since 8.2)
    #[qapi(name = "mode")]
    pub mode: Option<MigMode>,
    /// Whether and how to detect zero pages.
    /// See description in @ZeroPageDetection.  Default is 'multifd'.
    /// (since 9.0)
    #[qapi(name = "zero-page-detection")]
    pub zero_page_detection: Option<ZeroPageDetection>,
    /// Open migration files with O_DIRECT when possible.  This
    /// only has effect if the @mapped-ram capability is enabled.
    /// (Since 9.1)
    #[qapi(name = "direct-io")]
    pub direct_io: Option<bool>,
}
/// Set various migration parameters.
#[qapi(name = "migrate-set-parameters")]
#[qapi(since = "2.4")]
#[qapi(returns = "()")]
pub struct MigrateSetParameters {
    #[qapi(flatten)]
    pub data: MigrateSetParameters,
}
/// The optional members aren't actually optional.
#[qapi(name = "MigrationParameters")]
#[qapi(since = "2.4")]
pub struct MigrationParameters {
    /// Initial delay (in milliseconds) before sending
    /// the first announce (Since 4.0)
    #[qapi(name = "announce-initial")]
    pub announce_initial: Option<u64>,
    /// Maximum delay (in milliseconds) between packets in
    /// the announcement (Since 4.0)
    #[qapi(name = "announce-max")]
    pub announce_max: Option<u64>,
    /// Number of self-announce packets sent after
    /// migration (Since 4.0)
    #[qapi(name = "announce-rounds")]
    pub announce_rounds: Option<u64>,
    /// Increase in delay (in milliseconds) between
    /// subsequent packets in the announcement (Since 4.0)
    #[qapi(name = "announce-step")]
    pub announce_step: Option<u64>,
    /// The ratio of bytes_dirty_period and
    /// bytes_xfer_period to trigger throttling.  It is expressed as
    /// percentage.  The default value is 50.  (Since 5.0)
    #[qapi(name = "throttle-trigger-threshold")]
    pub throttle_trigger_threshold: Option<u8>,
    /// Initial percentage of time guest cpus are
    /// throttled when migration auto-converge is activated.  (Since
    /// 2.7)
    #[qapi(name = "cpu-throttle-initial")]
    pub cpu_throttle_initial: Option<u8>,
    /// throttle percentage increase each time
    /// auto-converge detects that migration is not making progress.
    /// (Since 2.7)
    #[qapi(name = "cpu-throttle-increment")]
    pub cpu_throttle_increment: Option<u8>,
    /// Make CPU throttling slower at tail stage At
    /// the tail stage of throttling, the Guest is very sensitive to CPU
    /// percentage while the @cpu-throttle -increment is excessive
    /// usually at tail stage.  If this parameter is true, we will
    /// compute the ideal CPU percentage used by the Guest, which may
    /// exactly make the dirty rate match the dirty rate threshold.
    /// Then we will choose a smaller throttle increment between the one
    /// specified by @cpu-throttle-increment and the one generated by
    /// ideal CPU percentage.  Therefore, it is compatible to
    /// traditional throttling, meanwhile the throttle increment won't
    /// be excessive at tail stage.  The default value is false.  (Since
    /// 5.1)
    #[qapi(name = "cpu-throttle-tailslow")]
    pub cpu_throttle_tailslow: Option<bool>,
    /// ID of the 'tls-creds' object that provides credentials
    /// for establishing a TLS connection over the migration data
    /// channel.  On the outgoing side of the migration, the credentials
    /// must be for a 'client' endpoint, while for the incoming side the
    /// credentials must be for a 'server' endpoint.  An empty string
    /// means that QEMU will use plain text mode for migration, rather
    /// than TLS.  (Since 2.7)
    ///
    /// Note: 2.8 omits empty @tls-creds instead.
    #[qapi(name = "tls-creds")]
    pub tls_creds: Option<String>,
    /// migration target's hostname for validating the
    /// server's x509 certificate identity.  If empty, QEMU will use the
    /// hostname from the migration URI, if any.  (Since 2.7)
    ///
    /// Note: 2.8 omits empty @tls-hostname instead.
    #[qapi(name = "tls-hostname")]
    pub tls_hostname: Option<String>,
    /// ID of the 'authz' object subclass that provides access
    /// control checking of the TLS x509 certificate distinguished name.
    /// (Since 4.0)
    #[qapi(name = "tls-authz")]
    pub tls_authz: Option<String>,
    /// maximum speed for migration, in bytes per second.
    /// (Since 2.8)
    #[qapi(name = "max-bandwidth")]
    pub max_bandwidth: Option<u64>,
    /// to set the available bandwidth that
    /// migration can use during switchover phase.  NOTE!  This does not
    /// limit the bandwidth during switchover, but only for calculations
    /// when making decisions to switchover.  By default, this value is
    /// zero, which means QEMU will estimate the bandwidth
    /// automatically.  This can be set when the estimated value is not
    /// accurate, while the user is able to guarantee such bandwidth is
    /// available when switching over.  When specified correctly, this
    /// can make the switchover decision much more accurate.
    /// (Since 8.2)
    #[qapi(name = "avail-switchover-bandwidth")]
    pub avail_switchover_bandwidth: Option<u64>,
    /// set maximum tolerated downtime for migration.
    /// maximum downtime in milliseconds (Since 2.8)
    #[qapi(name = "downtime-limit")]
    pub downtime_limit: Option<u64>,
    /// the delay time between two COLO checkpoints.
    /// (Since 2.8)
    #[qapi(name = "x-checkpoint-delay")]
    #[qapi(feature = "unstable")]
    pub x_checkpoint_delay: Option<u32>,
    /// Number of channels used to migrate data in
    /// parallel.  This is the same number that the number of sockets
    /// used for migration.  The default value is 2 (since 4.0)
    #[qapi(name = "multifd-channels")]
    pub multifd_channels: Option<u8>,
    /// cache size to be used by XBZRLE migration.  It
    /// needs to be a multiple of the target page size and a power of 2
    /// (Since 2.11)
    #[qapi(name = "xbzrle-cache-size")]
    pub xbzrle_cache_size: Option<u64>,
    /// Background transfer bandwidth during
    /// postcopy.  Defaults to 0 (unlimited).  In bytes per second.
    /// (Since 3.0)
    #[qapi(name = "max-postcopy-bandwidth")]
    pub max_postcopy_bandwidth: Option<u64>,
    /// maximum cpu throttle percentage.  Defaults to 99.
    /// (Since 3.1)
    #[qapi(name = "max-cpu-throttle")]
    pub max_cpu_throttle: Option<u8>,
    /// Which compression method to use.  Defaults to
    /// none.  (Since 5.0)
    #[qapi(name = "multifd-compression")]
    pub multifd_compression: Option<MultiFdCompression>,
    /// Set the compression level to be used in live
    /// migration, the compression level is an integer between 0 and 9,
    /// where 0 means no compression, 1 means the best compression
    /// speed, and 9 means best compression ratio which will consume
    /// more CPU.  Defaults to 1.  (Since 5.0)
    #[qapi(name = "multifd-zlib-level")]
    pub multifd_zlib_level: Option<u8>,
    /// Set the compression level to be used in live
    /// migration, the compression level is an integer between 0 and 20,
    /// where 0 means no compression, 1 means the best compression
    /// speed, and 20 means best compression ratio which will consume
    /// more CPU.  Defaults to 1.  (Since 5.0)
    #[qapi(name = "multifd-zstd-level")]
    pub multifd_zstd_level: Option<u8>,
    /// Maps block nodes and bitmaps on them to
    /// aliases for the purpose of dirty bitmap migration.  Such aliases
    /// may for example be the corresponding names on the opposite site.
    /// The mapping must be one-to-one, but not necessarily complete: On
    /// the source, unmapped bitmaps and all bitmaps on unmapped nodes
    /// will be ignored.  On the destination, encountering an unmapped
    /// alias in the incoming migration stream will result in a report,
    /// and all further bitmap migration data will then be discarded.
    /// Note that the destination does not know about bitmaps it does
    /// not receive, so there is no limitation or requirement regarding
    /// the number of bitmaps received, or how they are named, or on
    /// which nodes they are placed.  By default (when this parameter
    /// has never been set), bitmap names are mapped to themselves.
    /// Nodes are mapped to their block device name if there is one, and
    /// to their node name otherwise.  (Since 5.2)
    #[qapi(name = "block-bitmap-mapping")]
    pub block_bitmap_mapping: Option<Vec<BitmapMigrationNodeAlias>>,
    /// Periodic time (in milliseconds) of dirty
    /// limit during live migration.  Should be in the range 1 to
    /// 1000ms.  Defaults to 1000ms.  (Since 8.1)
    #[qapi(name = "x-vcpu-dirty-limit-period")]
    #[qapi(feature = "unstable")]
    pub x_vcpu_dirty_limit_period: Option<u64>,
    /// Dirtyrate limit (MB/s) during live migration.
    /// Defaults to 1.  (Since 8.1)
    #[qapi(name = "vcpu-dirty-limit")]
    pub vcpu_dirty_limit: Option<u64>,
    /// Migration mode.  See description in @MigMode.  Default is
    /// 'normal'.  (Since 8.2)
    #[qapi(name = "mode")]
    pub mode: Option<MigMode>,
    /// Whether and how to detect zero pages.
    /// See description in @ZeroPageDetection.  Default is 'multifd'.
    /// (since 9.0)
    #[qapi(name = "zero-page-detection")]
    pub zero_page_detection: Option<ZeroPageDetection>,
    /// Open migration files with O_DIRECT when possible.  This
    /// only has effect if the @mapped-ram capability is enabled.
    /// (Since 9.1)
    #[qapi(name = "direct-io")]
    pub direct_io: Option<bool>,
}
/// Returns information about the current migration parameters
#[qapi(name = "query-migrate-parameters")]
#[qapi(since = "2.4")]
#[qapi(returns = "MigrationParameters")]
pub struct QueryMigrateParameters {}
/// Followup to a migration command to switch the migration to postcopy
/// mode.  The postcopy-ram capability must be set on both source and
/// destination before the original migration command.
#[qapi(name = "migrate-start-postcopy")]
#[qapi(since = "2.5")]
#[qapi(returns = "()")]
pub struct MigrateStartPostcopy {}
/// Emitted when a migration event happens
#[qapi(name = "MIGRATION")]
#[qapi(since = "2.4")]
pub struct Migration {
    /// @MigrationStatus describing the current migration status.
    #[qapi(name = "status")]
    pub status: MigrationStatus,
}
/// Emitted from the source side of a migration at the start of each
/// pass (when it syncs the dirty bitmap)
#[qapi(name = "MIGRATION_PASS")]
#[qapi(since = "2.6")]
pub struct MigrationPass {
    /// An incrementing count (starting at 1 on the first pass)
    #[qapi(name = "pass")]
    pub pass: i64,
}
/// The message transmission between Primary side and Secondary side.
#[qapi(name = "COLOMessage")]
#[qapi(since = "2.8")]
pub enum ColoMessage {
    /// Secondary VM (SVM) is ready for checkpointing
    #[qapi(name = "checkpoint-ready")]
    CheckpointReady,
    /// Primary VM (PVM) tells SVM to prepare for
    /// checkpointing
    #[qapi(name = "checkpoint-request")]
    CheckpointRequest,
    /// SVM gets PVM's checkpoint request
    #[qapi(name = "checkpoint-reply")]
    CheckpointReply,
    /// VM's state will be sent by PVM.
    #[qapi(name = "vmstate-send")]
    VmstateSend,
    /// The total size of VMstate.
    #[qapi(name = "vmstate-size")]
    VmstateSize,
    /// VM's state has been received by SVM.
    #[qapi(name = "vmstate-received")]
    VmstateReceived,
    /// VM's state has been loaded by SVM.
    #[qapi(name = "vmstate-loaded")]
    VmstateLoaded,
}
/// The COLO current mode.
#[qapi(name = "COLOMode")]
#[qapi(since = "2.8")]
pub enum ColoMode {
    /// COLO is disabled.
    #[qapi(name = "none")]
    None,
    /// COLO node in primary side.
    #[qapi(name = "primary")]
    Primary,
    /// COLO node in slave side.
    #[qapi(name = "secondary")]
    Secondary,
}
/// An enumeration of COLO failover status
#[qapi(name = "FailoverStatus")]
#[qapi(since = "2.8")]
pub enum FailoverStatus {
    /// no failover has ever happened
    #[qapi(name = "none")]
    None,
    /// got failover requirement but not handled
    #[qapi(name = "require")]
    Require,
    /// in the process of doing failover
    #[qapi(name = "active")]
    Active,
    /// finish the process of failover
    #[qapi(name = "completed")]
    Completed,
    /// restart the failover process, from 'none' -> 'completed'
    /// (Since 2.9)
    #[qapi(name = "relaunch")]
    Relaunch,
}
/// Emitted when VM finishes COLO mode due to some errors happening or
/// at the request of users.
#[qapi(name = "COLO_EXIT")]
#[qapi(since = "3.1")]
pub struct ColoExit {
    /// report COLO mode when COLO exited.
    #[qapi(name = "mode")]
    pub mode: ColoMode,
    /// describes the reason for the COLO exit.
    #[qapi(name = "reason")]
    pub reason: ColoExitReason,
}
/// The reason for a COLO exit.
#[qapi(name = "COLOExitReason")]
#[qapi(since = "3.1")]
pub enum ColoExitReason {
    /// failover has never happened.  This state does not occur in
    /// the COLO_EXIT event, and is only visible in the result of
    /// query-colo-status.
    #[qapi(name = "none")]
    None,
    /// COLO exit is due to an external request.
    #[qapi(name = "request")]
    Request,
    /// COLO exit is due to an internal error.
    #[qapi(name = "error")]
    Error,
    /// COLO is currently handling a failover (since 4.0).
    #[qapi(name = "processing")]
    Processing,
}
/// Tell qemu that heartbeat is lost, request it to do takeover
/// procedures.  If this command is sent to the PVM, the Primary side
/// will exit COLO mode.  If sent to the Secondary, the Secondary side
/// will run failover work, then takes over server operation to become
/// the service VM.
#[qapi(name = "x-colo-lost-heartbeat")]
#[qapi(condition = "CONFIG_REPLICATION")]
#[qapi(feature = "unstable")]
#[qapi(since = "2.8")]
#[qapi(returns = "()")]
pub struct XColoLostHeartbeat {}
/// Cancel the current executing migration process.
#[qapi(name = "migrate_cancel")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct MigrateCancel {}
/// Continue migration when it's in a paused state.
#[qapi(name = "migrate-continue")]
#[qapi(since = "2.11")]
#[qapi(returns = "()")]
pub struct MigrateContinue {
    /// The state the migration is currently expected to be in
    #[qapi(name = "state")]
    pub state: MigrationStatus,
}
/// The migration stream transport mechanisms.
#[qapi(name = "MigrationAddressType")]
#[qapi(since = "8.2")]
pub enum MigrationAddressType {
    /// Migrate via socket.
    #[qapi(name = "socket")]
    Socket,
    /// Direct the migration stream to another process.
    #[qapi(name = "exec")]
    Exec,
    /// Migrate via RDMA.
    #[qapi(name = "rdma")]
    Rdma,
    /// Direct the migration stream to a file.
    #[qapi(name = "file")]
    File,
}
#[qapi(name = "FileMigrationArgs")]
#[qapi(since = "8.2")]
pub struct FileMigrationArgs {
    /// The file to receive the migration stream
    #[qapi(name = "filename")]
    pub filename: String,
    /// The file offset where the migration stream will start
    #[qapi(name = "offset")]
    pub offset: u64,
}
#[qapi(name = "MigrationExecCommand")]
#[qapi(since = "8.2")]
pub struct MigrationExecCommand {
    /// command (list head) and arguments to execute.
    #[qapi(name = "args")]
    pub args: Vec<String>,
}
pub enum MigrationAddressBranch {
    #[qapi(name = "socket")]
    Socket(SocketAddress),
    #[qapi(name = "exec")]
    Exec(MigrationExecCommand),
    #[qapi(name = "rdma")]
    Rdma(InetSocketAddress),
    #[qapi(name = "file")]
    File(FileMigrationArgs),
}
/// Migration endpoint configuration.
#[qapi(name = "MigrationAddress")]
#[qapi(since = "8.2")]
pub struct MigrationAddress {
    /// The migration stream transport mechanism
    #[qapi(name = "transport")]
    #[qapi(discriminator)]
    pub transport: MigrationAddressType,
    #[qapi(union)]
    pub u: Option<MigrationAddressBranch>,
}
/// The migration channel-type request options.
#[qapi(name = "MigrationChannelType")]
#[qapi(since = "8.1")]
pub enum MigrationChannelType {
    /// Main outbound migration channel.
    #[qapi(name = "main")]
    Main,
}
/// Migration stream channel parameters.
#[qapi(name = "MigrationChannel")]
#[qapi(since = "8.1")]
pub struct MigrationChannel {
    /// Channel type for transferring packet information.
    #[qapi(name = "channel-type")]
    pub channel_type: MigrationChannelType,
    /// Migration endpoint configuration on destination interface.
    #[qapi(name = "addr")]
    pub addr: MigrationAddress,
}
/// Migrates the current running guest to another Virtual Machine.
#[qapi(name = "migrate")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct Migrate {
    /// the Uniform Resource Identifier of the destination VM
    #[qapi(name = "uri")]
    pub uri: Option<String>,
    /// list of migration stream channels with each stream in the
    /// list connected to a destination interface endpoint.
    #[qapi(name = "channels")]
    pub channels: Option<Vec<MigrationChannel>>,
    /// this argument exists only for compatibility reasons and is
    /// ignored by QEMU
    #[qapi(name = "detach")]
    pub detach: Option<bool>,
    /// resume one paused migration, default "off".  (since 3.0)
    #[qapi(name = "resume")]
    pub resume: Option<bool>,
}
/// Start an incoming migration, the qemu must have been started with
/// -incoming defer
#[qapi(name = "migrate-incoming")]
#[qapi(since = "2.3")]
#[qapi(returns = "()")]
pub struct MigrateIncoming {
    /// The Uniform Resource Identifier identifying the source or
    /// address to listen on
    #[qapi(name = "uri")]
    pub uri: Option<String>,
    /// list of migration stream channels with each stream in the
    /// list connected to a destination interface endpoint.
    #[qapi(name = "channels")]
    pub channels: Option<Vec<MigrationChannel>>,
    /// Exit on incoming migration failure.  Default true.
    /// When set to false, the failure triggers a MIGRATION event, and
    /// error details could be retrieved with query-migrate.
    /// (since 9.1)
    #[qapi(name = "exit-on-error")]
    pub exit_on_error: Option<bool>,
}
/// Save the state of all devices to file.  The RAM and the block
/// devices of the VM are not saved by this command.
#[qapi(name = "xen-save-devices-state")]
#[qapi(since = "1.1")]
#[qapi(returns = "()")]
pub struct XenSaveDevicesState {
    /// the file to save the state of the devices to as binary
    /// data.  See xen-save-devices-state.txt for a description of the
    /// binary format.
    #[qapi(name = "filename")]
    pub filename: String,
    /// Optional argument to ask QEMU to treat this command as part
    /// of a live migration.  Default to true.  (since 2.11)
    #[qapi(name = "live")]
    pub live: Option<bool>,
}
/// Enable or disable the global dirty log mode.
#[qapi(name = "xen-set-global-dirty-log")]
#[qapi(since = "1.3")]
#[qapi(returns = "()")]
pub struct XenSetGlobalDirtyLog {
    /// true to enable, false to disable.
    #[qapi(name = "enable")]
    pub enable: bool,
}
/// Load the state of all devices from file.  The RAM and the block
/// devices of the VM are not loaded by this command.
#[qapi(name = "xen-load-devices-state")]
#[qapi(since = "2.7")]
#[qapi(returns = "()")]
pub struct XenLoadDevicesState {
    /// the file to load the state of the devices from as binary
    /// data.  See xen-save-devices-state.txt for a description of the
    /// binary format.
    #[qapi(name = "filename")]
    pub filename: String,
}
/// Enable or disable replication.
#[qapi(name = "xen-set-replication")]
#[qapi(condition = "CONFIG_REPLICATION")]
#[qapi(since = "2.9")]
#[qapi(returns = "()")]
pub struct XenSetReplication {
    /// true to enable, false to disable.
    #[qapi(name = "enable")]
    pub enable: bool,
    /// true for primary or false for secondary.
    #[qapi(name = "primary")]
    pub primary: bool,
    /// true to do failover, false to stop.  Cannot be specified
    /// if 'enable' is true.  Default value is false.
    #[qapi(name = "failover")]
    pub failover: Option<bool>,
}
/// The result format for 'query-xen-replication-status'.
#[qapi(name = "ReplicationStatus")]
#[qapi(condition = "CONFIG_REPLICATION")]
#[qapi(since = "2.9")]
pub struct ReplicationStatus {
    /// true if an error happened, false if replication is normal.
    #[qapi(name = "error")]
    pub error: bool,
    /// the human readable error description string, when @error is
    /// 'true'.
    #[qapi(name = "desc")]
    pub desc: Option<String>,
}
/// Query replication status while the vm is running.
#[qapi(name = "query-xen-replication-status")]
#[qapi(condition = "CONFIG_REPLICATION")]
#[qapi(since = "2.9")]
#[qapi(returns = "ReplicationStatus")]
pub struct QueryXenReplicationStatus {}
/// Xen uses this command to notify replication to trigger a checkpoint.
#[qapi(name = "xen-colo-do-checkpoint")]
#[qapi(condition = "CONFIG_REPLICATION")]
#[qapi(since = "2.9")]
#[qapi(returns = "()")]
pub struct XenColoDoCheckpoint {}
/// The result format for 'query-colo-status'.
#[qapi(name = "COLOStatus")]
#[qapi(condition = "CONFIG_REPLICATION")]
#[qapi(since = "3.1")]
pub struct ColoStatus {
    /// COLO running mode.  If COLO is running, this field will
    /// return 'primary' or 'secondary'.
    #[qapi(name = "mode")]
    pub mode: ColoMode,
    /// COLO last running mode.  If COLO is running, this field
    /// will return same like mode field, after failover we can use this
    /// field to get last colo mode.  (since 4.0)
    #[qapi(name = "last-mode")]
    pub last_mode: ColoMode,
    /// describes the reason for the COLO exit.
    #[qapi(name = "reason")]
    pub reason: ColoExitReason,
}
/// Query COLO status while the vm is running.
#[qapi(name = "query-colo-status")]
#[qapi(condition = "CONFIG_REPLICATION")]
#[qapi(since = "3.1")]
#[qapi(returns = "COLOStatus")]
pub struct QueryColoStatus {}
/// Provide a recovery migration stream URI.
#[qapi(name = "migrate-recover")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
#[qapi(allow_oob)]
pub struct MigrateRecover {
    /// the URI to be used for the recovery of migration stream.
    #[qapi(name = "uri")]
    pub uri: String,
}
/// Pause a migration.  Currently it only supports postcopy.
#[qapi(name = "migrate-pause")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
#[qapi(allow_oob)]
pub struct MigratePause {}
/// Emitted from source side of a migration when migration state is
/// WAIT_UNPLUG.  Device was unplugged by guest operating system.
/// Device resources in QEMU are kept on standby to be able to re-plug
/// it in case of migration failure.
#[qapi(name = "UNPLUG_PRIMARY")]
#[qapi(since = "4.2")]
pub struct UnplugPrimary {
    /// QEMU device id of the unplugged device
    #[qapi(name = "device-id")]
    pub device_id: String,
}
/// Dirty rate of vcpu.
#[qapi(name = "DirtyRateVcpu")]
#[qapi(since = "6.2")]
pub struct DirtyRateVcpu {
    /// vcpu index.
    #[qapi(name = "id")]
    pub id: i64,
    /// dirty rate.
    #[qapi(name = "dirty-rate")]
    pub dirty_rate: i64,
}
/// Dirty page rate measurement status.
#[qapi(name = "DirtyRateStatus")]
#[qapi(since = "5.2")]
pub enum DirtyRateStatus {
    /// measuring thread has not been started yet
    #[qapi(name = "unstarted")]
    Unstarted,
    /// measuring thread is running
    #[qapi(name = "measuring")]
    Measuring,
    /// dirty page rate is measured and the results are available
    #[qapi(name = "measured")]
    Measured,
}
/// Method used to measure dirty page rate.  Differences between
/// available methods are explained in @calc-dirty-rate.
#[qapi(name = "DirtyRateMeasureMode")]
#[qapi(since = "6.2")]
pub enum DirtyRateMeasureMode {
    /// use page sampling
    #[qapi(name = "page-sampling")]
    PageSampling,
    /// use dirty ring
    #[qapi(name = "dirty-ring")]
    DirtyRing,
    /// use dirty bitmap
    #[qapi(name = "dirty-bitmap")]
    DirtyBitmap,
}
/// Specifies unit in which time-related value is specified.
#[qapi(name = "TimeUnit")]
#[qapi(since = "8.2")]
pub enum TimeUnit {
    /// value is in seconds
    #[qapi(name = "second")]
    Second,
    /// value is in milliseconds
    #[qapi(name = "millisecond")]
    Millisecond,
}
/// Information about measured dirty page rate.
#[qapi(name = "DirtyRateInfo")]
#[qapi(since = "5.2")]
pub struct DirtyRateInfo {
    /// an estimate of the dirty page rate of the VM in units
    /// of MiB/s.  Value is present only when @status is 'measured'.
    #[qapi(name = "dirty-rate")]
    pub dirty_rate: Option<i64>,
    /// current status of dirty page rate measurements
    #[qapi(name = "status")]
    pub status: DirtyRateStatus,
    /// start time in units of second for calculation
    #[qapi(name = "start-time")]
    pub start_time: i64,
    /// time period for which dirty page rate was measured,
    /// expressed and rounded down to @calc-time-unit.
    #[qapi(name = "calc-time")]
    pub calc_time: i64,
    /// time unit of @calc-time  (Since 8.2)
    #[qapi(name = "calc-time-unit")]
    pub calc_time_unit: TimeUnit,
    /// number of sampled pages per GiB of guest memory.
    /// Valid only in page-sampling mode (Since 6.1)
    #[qapi(name = "sample-pages")]
    pub sample_pages: u64,
    /// mode that was used to measure dirty page rate (Since 6.2)
    #[qapi(name = "mode")]
    pub mode: DirtyRateMeasureMode,
    /// dirty rate for each vCPU if dirty-ring mode was
    /// specified (Since 6.2)
    #[qapi(name = "vcpu-dirty-rate")]
    pub vcpu_dirty_rate: Option<Vec<DirtyRateVcpu>>,
}
/// Start measuring dirty page rate of the VM.  Results can be retrieved
/// with @query-dirty-rate after measurements are completed.
///
/// Dirty page rate is the number of pages changed in a given time
/// period expressed in MiB/s.  The following methods of calculation are
/// available:
///
/// 1. In page sampling mode, a random subset of pages are selected and
/// hashed twice: once at the beginning of measurement time period,
/// and once again at the end.  If two hashes for some page are
/// different, the page is counted as changed.  Since this method
/// relies on sampling and hashing, calculated dirty page rate is
/// only an estimate of its true value.  Increasing @sample-pages
/// improves estimation quality at the cost of higher computational
/// overhead.
///
/// 2. Dirty bitmap mode captures writes to memory (for example by
/// temporarily revoking write access to all pages) and counting page
/// faults.  Information about modified pages is collected into a
/// bitmap, where each bit corresponds to one guest page.  This mode
/// requires that KVM accelerator property "dirty-ring-size" is *not*
/// set.
///
/// 3. Dirty ring mode is similar to dirty bitmap mode, but the
/// information about modified pages is collected into ring buffer.
/// This mode tracks page modification per each vCPU separately.  It
/// requires that KVM accelerator property "dirty-ring-size" is set.
#[qapi(name = "calc-dirty-rate")]
#[qapi(since = "5.2")]
#[qapi(returns = "()")]
pub struct CalcDirtyRate {
    /// time period for which dirty page rate is calculated.  By
    /// default it is specified in seconds, but the unit can be set
    /// explicitly with @calc-time-unit.  Note that larger @calc-time
    /// values will typically result in smaller dirty page rates because
    /// page dirtying is a one-time event.  Once some page is counted as
    /// dirty during @calc-time period, further writes to this page will
    /// not increase dirty page rate anymore.
    #[qapi(name = "calc-time")]
    pub calc_time: i64,
    /// time unit in which @calc-time is specified.  By
    /// default it is seconds.  (Since 8.2)
    #[qapi(name = "calc-time-unit")]
    pub calc_time_unit: Option<TimeUnit>,
    /// number of sampled pages per each GiB of guest memory.
    /// Default value is 512.  For 4KiB guest pages this corresponds to
    /// sampling ratio of 0.2%.  This argument is used only in page
    /// sampling mode.  (Since 6.1)
    #[qapi(name = "sample-pages")]
    pub sample_pages: Option<i64>,
    /// mechanism for tracking dirty pages.  Default value is
    /// 'page-sampling'.  Others are 'dirty-bitmap' and 'dirty-ring'.
    /// (Since 6.1)
    #[qapi(name = "mode")]
    pub mode: Option<DirtyRateMeasureMode>,
}
/// Query results of the most recent invocation of @calc-dirty-rate.
#[qapi(name = "query-dirty-rate")]
#[qapi(since = "5.2")]
#[qapi(returns = "DirtyRateInfo")]
pub struct QueryDirtyRate {
    /// time unit in which to report calculation time.
    /// By default it is reported in seconds.  (Since 8.2)
    #[qapi(name = "calc-time-unit")]
    pub calc_time_unit: Option<TimeUnit>,
}
/// Dirty page rate limit information of a virtual CPU.
#[qapi(name = "DirtyLimitInfo")]
#[qapi(since = "7.1")]
pub struct DirtyLimitInfo {
    /// index of a virtual CPU.
    #[qapi(name = "cpu-index")]
    pub cpu_index: i64,
    /// upper limit of dirty page rate (MB/s) for a virtual
    /// CPU, 0 means unlimited.
    #[qapi(name = "limit-rate")]
    pub limit_rate: u64,
    /// current dirty page rate (MB/s) for a virtual CPU.
    #[qapi(name = "current-rate")]
    pub current_rate: u64,
}
/// Set the upper limit of dirty page rate for virtual CPUs.
///
/// Requires KVM with accelerator property "dirty-ring-size" set.  A
/// virtual CPU's dirty page rate is a measure of its memory load.  To
/// observe dirty page rates, use @calc-dirty-rate.
#[qapi(name = "set-vcpu-dirty-limit")]
#[qapi(since = "7.1")]
#[qapi(returns = "()")]
pub struct SetVcpuDirtyLimit {
    /// index of a virtual CPU, default is all.
    #[qapi(name = "cpu-index")]
    pub cpu_index: Option<i64>,
    /// upper limit of dirty page rate (MB/s) for virtual CPUs.
    #[qapi(name = "dirty-rate")]
    pub dirty_rate: u64,
}
/// Cancel the upper limit of dirty page rate for virtual CPUs.
///
/// Cancel the dirty page limit for the vCPU which has been set with
/// set-vcpu-dirty-limit command.  Note that this command requires
/// support from dirty ring, same as the "set-vcpu-dirty-limit".
#[qapi(name = "cancel-vcpu-dirty-limit")]
#[qapi(since = "7.1")]
#[qapi(returns = "()")]
pub struct CancelVcpuDirtyLimit {
    /// index of a virtual CPU, default is all.
    #[qapi(name = "cpu-index")]
    pub cpu_index: Option<i64>,
}
/// Returns information about virtual CPU dirty page rate limits, if
/// any.
#[qapi(name = "query-vcpu-dirty-limit")]
#[qapi(since = "7.1")]
#[qapi(returns = "Vec<DirtyLimitInfo>")]
pub struct QueryVcpuDirtyLimit {}
/// Information about migrationthreads
#[qapi(name = "MigrationThreadInfo")]
#[qapi(since = "7.2")]
pub struct MigrationThreadInfo {
    /// the name of migration thread
    #[qapi(name = "name")]
    pub name: String,
    /// ID of the underlying host thread
    #[qapi(name = "thread-id")]
    pub thread_id: i64,
}
/// Returns information of migration threads
#[qapi(name = "query-migrationthreads")]
#[qapi(since = "7.2")]
#[qapi(returns = "Vec<MigrationThreadInfo>")]
pub struct QueryMigrationthreads {}
/// Save a VM snapshot
#[qapi(name = "snapshot-save")]
#[qapi(since = "6.0")]
#[qapi(returns = "()")]
pub struct SnapshotSave {
    /// identifier for the newly created job
    #[qapi(name = "job-id")]
    pub job_id: String,
    /// name of the snapshot to create
    #[qapi(name = "tag")]
    pub tag: String,
    /// block device node name to save vmstate to
    #[qapi(name = "vmstate")]
    pub vmstate: String,
    /// list of block device node names to save a snapshot to
    ///
    /// Applications should not assume that the snapshot save is complete
    /// when this command returns.  The job commands / events must be used
    /// to determine completion and to fetch details of any errors that
    /// arise.
    ///
    /// Note that execution of the guest CPUs may be stopped during the time
    /// it takes to save the snapshot.  A future version of QEMU may ensure
    /// CPUs are executing continuously.
    ///
    /// It is strongly recommended that @devices contain all writable block
    /// device nodes if a consistent snapshot is required.
    ///
    /// If @tag already exists, an error will be reported
    #[qapi(name = "devices")]
    pub devices: Vec<String>,
}
/// Load a VM snapshot
#[qapi(name = "snapshot-load")]
#[qapi(since = "6.0")]
#[qapi(returns = "()")]
pub struct SnapshotLoad {
    /// identifier for the newly created job
    #[qapi(name = "job-id")]
    pub job_id: String,
    /// name of the snapshot to load.
    #[qapi(name = "tag")]
    pub tag: String,
    /// block device node name to load vmstate from
    #[qapi(name = "vmstate")]
    pub vmstate: String,
    /// list of block device node names to load a snapshot from
    ///
    /// Applications should not assume that the snapshot load is complete
    /// when this command returns.  The job commands / events must be used
    /// to determine completion and to fetch details of any errors that
    /// arise.
    ///
    /// Note that execution of the guest CPUs will be stopped during the
    /// time it takes to load the snapshot.
    ///
    /// It is strongly recommended that @devices contain all writable block
    /// device nodes that can have changed since the original @snapshot-save
    /// command execution.
    #[qapi(name = "devices")]
    pub devices: Vec<String>,
}
/// Delete a VM snapshot
#[qapi(name = "snapshot-delete")]
#[qapi(since = "6.0")]
#[qapi(returns = "()")]
pub struct SnapshotDelete {
    /// identifier for the newly created job
    #[qapi(name = "job-id")]
    pub job_id: String,
    /// name of the snapshot to delete.
    #[qapi(name = "tag")]
    pub tag: String,
    /// list of block device node names to delete a snapshot from
    ///
    /// Applications should not assume that the snapshot delete is complete
    /// when this command returns.  The job commands / events must be used
    /// to determine completion and to fetch details of any errors that
    /// arise.
    #[qapi(name = "devices")]
    pub devices: Vec<String>,
}
// path end:	qapi/migration.json
// path begin:	qapi/transaction.json
/// This action can be used to test transaction failure.
#[qapi(name = "Abort")]
#[qapi(since = "1.6")]
pub struct Abort {}
/// An enumeration of Transactional completion modes.
#[qapi(name = "ActionCompletionMode")]
#[qapi(since = "2.5")]
pub enum ActionCompletionMode {
    /// Do not attempt to cancel any other Actions if any
    /// Actions fail after the Transaction request succeeds.  All
    /// Actions that can complete successfully will do so without
    /// waiting on others.  This is the default.
    #[qapi(name = "individual")]
    Individual,
    /// If any Action fails after the Transaction succeeds, cancel
    /// all Actions.  Actions do not complete until all Actions are
    /// ready to complete.  May be rejected by Actions that do not
    /// support this completion mode.
    #[qapi(name = "grouped")]
    Grouped,
}
#[qapi(name = "TransactionActionKind")]
#[qapi(since = "1.1")]
pub enum TransactionActionKind {
    /// Since 1.6
    #[qapi(name = "abort")]
    Abort,
    /// Since 2.5
    #[qapi(name = "block-dirty-bitmap-add")]
    BlockDirtyBitmapAdd,
    /// Since 4.2
    #[qapi(name = "block-dirty-bitmap-remove")]
    BlockDirtyBitmapRemove,
    /// Since 2.5
    #[qapi(name = "block-dirty-bitmap-clear")]
    BlockDirtyBitmapClear,
    /// Since 4.0
    #[qapi(name = "block-dirty-bitmap-enable")]
    BlockDirtyBitmapEnable,
    /// Since 4.0
    #[qapi(name = "block-dirty-bitmap-disable")]
    BlockDirtyBitmapDisable,
    /// Since 4.0
    #[qapi(name = "block-dirty-bitmap-merge")]
    BlockDirtyBitmapMerge,
    /// Since 2.3
    #[qapi(name = "blockdev-backup")]
    BlockdevBackup,
    /// Since 2.5
    #[qapi(name = "blockdev-snapshot")]
    BlockdevSnapshot,
    /// Since 1.7
    #[qapi(name = "blockdev-snapshot-internal-sync")]
    BlockdevSnapshotInternalSync,
    /// since 1.1
    #[qapi(name = "blockdev-snapshot-sync")]
    BlockdevSnapshotSync,
    /// Since 1.6
    #[qapi(name = "drive-backup")]
    #[qapi(feature = "deprecated")]
    DriveBackup,
}
#[qapi(name = "AbortWrapper")]
#[qapi(since = "1.6")]
pub struct AbortWrapper {
    #[qapi(name = "data")]
    pub data: Abort,
}
#[qapi(name = "BlockDirtyBitmapAddWrapper")]
#[qapi(since = "2.5")]
pub struct BlockDirtyBitmapAddWrapper {
    #[qapi(name = "data")]
    pub data: BlockDirtyBitmapAdd,
}
#[qapi(name = "BlockDirtyBitmapWrapper")]
#[qapi(since = "2.5")]
pub struct BlockDirtyBitmapWrapper {
    #[qapi(name = "data")]
    pub data: BlockDirtyBitmap,
}
#[qapi(name = "BlockDirtyBitmapMergeWrapper")]
#[qapi(since = "4.0")]
pub struct BlockDirtyBitmapMergeWrapper {
    #[qapi(name = "data")]
    pub data: BlockDirtyBitmapMerge,
}
#[qapi(name = "BlockdevBackupWrapper")]
#[qapi(since = "2.3")]
pub struct BlockdevBackupWrapper {
    #[qapi(name = "data")]
    pub data: BlockdevBackup,
}
#[qapi(name = "BlockdevSnapshotWrapper")]
#[qapi(since = "2.5")]
pub struct BlockdevSnapshotWrapper {
    #[qapi(name = "data")]
    pub data: BlockdevSnapshot,
}
#[qapi(name = "BlockdevSnapshotInternalWrapper")]
#[qapi(since = "1.7")]
pub struct BlockdevSnapshotInternalWrapper {
    #[qapi(name = "data")]
    pub data: BlockdevSnapshotInternal,
}
#[qapi(name = "BlockdevSnapshotSyncWrapper")]
#[qapi(since = "1.1")]
pub struct BlockdevSnapshotSyncWrapper {
    #[qapi(name = "data")]
    pub data: BlockdevSnapshotSync,
}
#[qapi(name = "DriveBackupWrapper")]
#[qapi(since = "1.6")]
pub struct DriveBackupWrapper {
    #[qapi(name = "data")]
    pub data: DriveBackup,
}
pub enum TransactionActionBranch {
    #[qapi(name = "abort")]
    Abort(AbortWrapper),
    #[qapi(name = "block-dirty-bitmap-add")]
    BlockDirtyBitmapAdd(BlockDirtyBitmapAddWrapper),
    #[qapi(name = "block-dirty-bitmap-remove")]
    BlockDirtyBitmapRemove(BlockDirtyBitmapWrapper),
    #[qapi(name = "block-dirty-bitmap-clear")]
    BlockDirtyBitmapClear(BlockDirtyBitmapWrapper),
    #[qapi(name = "block-dirty-bitmap-enable")]
    BlockDirtyBitmapEnable(BlockDirtyBitmapWrapper),
    #[qapi(name = "block-dirty-bitmap-disable")]
    BlockDirtyBitmapDisable(BlockDirtyBitmapWrapper),
    #[qapi(name = "block-dirty-bitmap-merge")]
    BlockDirtyBitmapMerge(BlockDirtyBitmapMergeWrapper),
    #[qapi(name = "blockdev-backup")]
    BlockdevBackup(BlockdevBackupWrapper),
    #[qapi(name = "blockdev-snapshot")]
    BlockdevSnapshot(BlockdevSnapshotWrapper),
    #[qapi(name = "blockdev-snapshot-internal-sync")]
    BlockdevSnapshotInternalSync(BlockdevSnapshotInternalWrapper),
    #[qapi(name = "blockdev-snapshot-sync")]
    BlockdevSnapshotSync(BlockdevSnapshotSyncWrapper),
    #[qapi(name = "drive-backup")]
    DriveBackup(DriveBackupWrapper),
}
/// A discriminated record of operations that can be performed with
/// @transaction.
#[qapi(name = "TransactionAction")]
#[qapi(since = "1.1")]
pub struct TransactionAction {
    /// the operation to be performed
    #[qapi(name = "type")]
    #[qapi(discriminator)]
    pub r#type: TransactionActionKind,
    #[qapi(union)]
    pub u: Option<TransactionActionBranch>,
}
/// Optional arguments to modify the behavior of a Transaction.
#[qapi(name = "TransactionProperties")]
#[qapi(since = "2.5")]
pub struct TransactionProperties {
    /// Controls how jobs launched asynchronously by
    /// Actions will complete or fail as a group.  See
    /// @ActionCompletionMode for details.
    #[qapi(name = "completion-mode")]
    pub completion_mode: Option<ActionCompletionMode>,
}
/// Executes a number of transactionable QMP commands atomically.  If
/// any operation fails, then the entire set of actions will be
/// abandoned and the appropriate error returned.
///
/// For external snapshots, the dictionary contains the device, the file
/// to use for the new snapshot, and the format.  The default format, if
/// not specified, is qcow2.
///
/// Each new snapshot defaults to being created by QEMU (wiping any
/// contents if the file already exists), but it is also possible to
/// reuse an externally-created file.  In the latter case, you should
/// ensure that the new image file has the same contents as the current
/// one; QEMU cannot perform any meaningful check.  Typically this is
/// achieved by using the current image file as the backing file for the
/// new image.
///
/// On failure, the original disks pre-snapshot attempt will be used.
///
/// For internal snapshots, the dictionary contains the device and the
/// snapshot's name.  If an internal snapshot matching name already
/// exists, the request will be rejected.  Only some image formats
/// support it, for example, qcow2, and rbd,
///
/// On failure, qemu will try delete the newly created internal snapshot
/// in the transaction.  When an I/O error occurs during deletion, the
/// user needs to fix it later with qemu-img or other command.
#[qapi(name = "transaction")]
#[qapi(since = "1.1")]
#[qapi(returns = "()")]
pub struct Transaction {
    /// List of @TransactionAction; information needed for the
    /// respective operations.
    #[qapi(name = "actions")]
    pub actions: Vec<TransactionAction>,
    /// structure of additional options to control the
    /// execution of the transaction.  See @TransactionProperties for
    /// additional detail.
    #[qapi(name = "properties")]
    pub properties: Option<TransactionProperties>,
}
// path end:	qapi/transaction.json
// path begin:	qapi/trace.json
/// State of a tracing event.
#[qapi(name = "TraceEventState")]
#[qapi(since = "2.2")]
pub enum TraceEventState {
    /// The event is statically disabled.
    #[qapi(name = "unavailable")]
    Unavailable,
    /// The event is dynamically disabled.
    #[qapi(name = "disabled")]
    Disabled,
    /// The event is dynamically enabled.
    #[qapi(name = "enabled")]
    Enabled,
}
/// Information of a tracing event.
#[qapi(name = "TraceEventInfo")]
#[qapi(since = "2.2")]
pub struct TraceEventInfo {
    /// Event name.
    #[qapi(name = "name")]
    pub name: String,
    /// Tracing state.
    #[qapi(name = "state")]
    pub state: TraceEventState,
}
/// Query the state of events.
#[qapi(name = "trace-event-get-state")]
#[qapi(since = "2.2")]
#[qapi(returns = "Vec<TraceEventInfo>")]
pub struct TraceEventGetState {
    /// Event name pattern (case-sensitive glob).
    #[qapi(name = "name")]
    pub name: String,
}
/// Set the dynamic tracing state of events.
#[qapi(name = "trace-event-set-state")]
#[qapi(since = "2.2")]
#[qapi(returns = "()")]
pub struct TraceEventSetState {
    /// Event name pattern (case-sensitive glob).
    #[qapi(name = "name")]
    pub name: String,
    /// Whether to enable tracing.
    #[qapi(name = "enable")]
    pub enable: bool,
    /// Do not match unavailable events with @name.
    #[qapi(name = "ignore-unavailable")]
    pub ignore_unavailable: Option<bool>,
}
// path end:	qapi/trace.json
// path begin:	qapi/compat.json
/// Policy for handling "funny" input.
#[qapi(name = "CompatPolicyInput")]
#[qapi(since = "6.0")]
pub enum CompatPolicyInput {
    /// Accept silently
    #[qapi(name = "accept")]
    Accept,
    /// Reject with an error
    #[qapi(name = "reject")]
    Reject,
    /// abort() the process
    #[qapi(name = "crash")]
    Crash,
}
/// Policy for handling "funny" output.
#[qapi(name = "CompatPolicyOutput")]
#[qapi(since = "6.0")]
pub enum CompatPolicyOutput {
    /// Pass on unchanged
    #[qapi(name = "accept")]
    Accept,
    /// Filter out
    #[qapi(name = "hide")]
    Hide,
}
/// Policy for handling deprecated management interfaces.
///
/// This is intended for testing users of the management interfaces.
///
/// Limitation: covers only syntactic aspects of QMP, i.e. stuff tagged
/// with feature 'deprecated' or 'unstable'.  We may want to extend it
/// to cover semantic aspects and CLI.
///
/// Limitation: deprecated-output policy @hide is not implemented for
/// enumeration values.  They behave the same as with policy @accept.
#[qapi(name = "CompatPolicy")]
#[qapi(since = "6.0")]
pub struct CompatPolicy {
    /// how to handle deprecated input (default 'accept')
    #[qapi(name = "deprecated-input")]
    pub deprecated_input: Option<CompatPolicyInput>,
    /// how to handle deprecated output (default
    /// 'accept')
    #[qapi(name = "deprecated-output")]
    pub deprecated_output: Option<CompatPolicyOutput>,
    /// how to handle unstable input (default 'accept')
    /// (since 6.2)
    #[qapi(name = "unstable-input")]
    pub unstable_input: Option<CompatPolicyInput>,
    /// how to handle unstable output (default 'accept')
    /// (since 6.2)
    #[qapi(name = "unstable-output")]
    pub unstable_output: Option<CompatPolicyOutput>,
}
// path end:	qapi/compat.json
// path begin:	qapi/control.json
/// Enable QMP capabilities.
#[qapi(name = "qmp_capabilities")]
#[qapi(since = "0.13")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct QmpCapabilities {
    /// An optional list of QMPCapability values to enable.  The
    /// client must not enable any capability that is not mentioned in
    /// the QMP greeting message.  If the field is not provided, it
    /// means no QMP capabilities will be enabled.  (since 2.12)
    #[qapi(name = "enable")]
    pub enable: Option<Vec<QmpCapability>>,
}
/// Enumeration of capabilities to be advertised during initial client
/// connection, used for agreeing on particular QMP extension behaviors.
#[qapi(name = "QMPCapability")]
#[qapi(since = "2.12")]
pub enum QmpCapability {
    /// QMP ability to support out-of-band requests.  (Please refer to
    /// qmp-spec.rst for more information on OOB)
    #[qapi(name = "oob")]
    Oob,
}
/// A three-part version number.
#[qapi(name = "VersionTriple")]
#[qapi(since = "2.4")]
pub struct VersionTriple {
    /// The major version number.
    #[qapi(name = "major")]
    pub major: i64,
    /// The minor version number.
    #[qapi(name = "minor")]
    pub minor: i64,
    /// The micro version number.
    #[qapi(name = "micro")]
    pub micro: i64,
}
/// A description of QEMU's version.
#[qapi(name = "VersionInfo")]
#[qapi(since = "0.14")]
pub struct VersionInfo {
    /// The version of QEMU.  By current convention, a micro version
    /// of 50 signifies a development branch.  A micro version greater
    /// than or equal to 90 signifies a release candidate for the next
    /// minor version.  A micro version of less than 50 signifies a
    /// stable release.
    #[qapi(name = "qemu")]
    pub qemu: VersionTriple,
    /// QEMU will always set this field to an empty string.
    /// Downstream versions of QEMU should set this to a non-empty
    /// string.  The exact format depends on the downstream however it
    /// highly recommended that a unique name is used.
    #[qapi(name = "package")]
    pub package: String,
}
/// Returns the current version of QEMU.
#[qapi(name = "query-version")]
#[qapi(since = "0.14")]
#[qapi(returns = "VersionInfo")]
#[qapi(allow_preconfig)]
pub struct QueryVersion {}
/// Information about a QMP command
#[qapi(name = "CommandInfo")]
#[qapi(since = "0.14")]
pub struct CommandInfo {
    /// The command name
    #[qapi(name = "name")]
    pub name: String,
}
/// Return a list of supported QMP commands by this server
#[qapi(name = "query-commands")]
#[qapi(since = "0.14")]
#[qapi(returns = "Vec<CommandInfo>")]
#[qapi(allow_preconfig)]
pub struct QueryCommands {}
/// This command will cause the QEMU process to exit gracefully.  While
/// every attempt is made to send the QMP response before terminating,
/// this is not guaranteed.  When using this interface, a premature EOF
/// would not be unexpected.
#[qapi(name = "quit")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct Quit {}
/// An enumeration of monitor modes.
#[qapi(name = "MonitorMode")]
#[qapi(since = "5.0")]
pub enum MonitorMode {
    /// HMP monitor (human-oriented command line interface)
    #[qapi(name = "readline")]
    Readline,
    /// QMP monitor (JSON-based machine interface)
    #[qapi(name = "control")]
    Control,
}
/// Options to be used for adding a new monitor.
#[qapi(name = "MonitorOptions")]
#[qapi(since = "5.0")]
pub struct MonitorOptions {
    /// Name of the monitor
    #[qapi(name = "id")]
    pub id: Option<String>,
    /// Selects the monitor mode (default: readline in the system
    /// emulator, control in qemu-storage-daemon)
    #[qapi(name = "mode")]
    pub mode: Option<MonitorMode>,
    /// Enables pretty printing (QMP only)
    #[qapi(name = "pretty")]
    pub pretty: Option<bool>,
    /// Name of a character device to expose the monitor on
    #[qapi(name = "chardev")]
    pub chardev: String,
}
// path end:	qapi/control.json
// path begin:	qapi/introspect.json
/// Command query-qmp-schema exposes the QMP wire ABI as an array of
/// SchemaInfo.  This lets QMP clients figure out what commands and
/// events are available in this QEMU, and their parameters and results.
///
/// However, the SchemaInfo can't reflect all the rules and restrictions
/// that apply to QMP.  It's interface introspection (figuring out
/// what's there), not interface specification.  The specification is in
/// the QAPI schema.
///
/// Furthermore, while we strive to keep the QMP wire format
/// backwards-compatible across qemu versions, the introspection output
/// is not guaranteed to have the same stability.  For example, one
/// version of qemu may list an object member as an optional
/// non-variant, while another lists the same member only through the
/// object's variants; or the type of a member may change from a generic
/// string into a specific enum or from one specific type into an
/// alternate that includes the original type alongside something else.
#[qapi(name = "query-qmp-schema")]
#[qapi(since = "2.5")]
#[qapi(returns = "Vec<SchemaInfo>")]
#[qapi(allow_preconfig)]
pub struct QueryQmpSchema {}
/// This is a @SchemaInfo's meta type, i.e. the kind of entity it
/// describes.
#[qapi(name = "SchemaMetaType")]
#[qapi(since = "2.5")]
pub enum SchemaMetaType {
    /// a predefined type such as 'int' or 'bool'.
    #[qapi(name = "builtin")]
    Builtin,
    /// an enumeration type
    #[qapi(name = "enum")]
    Enum,
    /// an array type
    #[qapi(name = "array")]
    Array,
    /// an object type (struct or union)
    #[qapi(name = "object")]
    Object,
    /// an alternate type
    #[qapi(name = "alternate")]
    Alternate,
    /// a QMP command
    #[qapi(name = "command")]
    Command,
    /// a QMP event
    #[qapi(name = "event")]
    Event,
}
pub enum SchemaInfoBranch {
    #[qapi(name = "builtin")]
    Builtin(SchemaInfoBuiltin),
    #[qapi(name = "enum")]
    Enum(SchemaInfoEnum),
    #[qapi(name = "array")]
    Array(SchemaInfoArray),
    #[qapi(name = "object")]
    Object(SchemaInfoObject),
    #[qapi(name = "alternate")]
    Alternate(SchemaInfoAlternate),
    #[qapi(name = "command")]
    Command(SchemaInfoCommand),
    #[qapi(name = "event")]
    Event(SchemaInfoEvent),
}
#[qapi(name = "SchemaInfo")]
#[qapi(since = "2.5")]
pub struct SchemaInfo {
    /// the entity's name, inherited from @base.  The SchemaInfo is
    /// always referenced by this name.  Commands and events have the
    /// name defined in the QAPI schema.  Unlike command and event
    /// names, type names are not part of the wire ABI.  Consequently,
    /// type names are meaningless strings here, although they are still
    /// guaranteed unique regardless of @meta-type.
    #[qapi(name = "name")]
    pub name: String,
    /// the entity's meta type, inherited from @base.
    #[qapi(name = "meta-type")]
    #[qapi(discriminator)]
    pub meta_type: SchemaMetaType,
    /// names of features associated with the entity, in no
    /// particular order.  (since 4.1 for object types, 4.2 for
    /// commands, 5.0 for the rest)
    #[qapi(name = "features")]
    pub features: Option<Vec<String>>,
    #[qapi(union)]
    pub u: Option<SchemaInfoBranch>,
}
/// Additional SchemaInfo members for meta-type 'builtin'.
#[qapi(name = "SchemaInfoBuiltin")]
#[qapi(since = "2.5")]
pub struct SchemaInfoBuiltin {
    /// the JSON type used for this type on the wire.
    #[qapi(name = "json-type")]
    pub json_type: JsonType,
}
/// The four primitive and two structured types according to RFC 8259
/// section 1, plus 'int' (split off 'number'), plus the obvious top
/// type 'value'.
#[qapi(name = "JSONType")]
#[qapi(since = "2.5")]
pub enum JsonType {
    #[qapi(name = "string")]
    String,
    #[qapi(name = "number")]
    f64,
    #[qapi(name = "int")]
    i64,
    #[qapi(name = "boolean")]
    Boolean,
    #[qapi(name = "null")]
    Null,
    #[qapi(name = "object")]
    Object,
    #[qapi(name = "array")]
    Array,
    #[qapi(name = "value")]
    Value,
}
/// Additional SchemaInfo members for meta-type 'enum'.
#[qapi(name = "SchemaInfoEnum")]
#[qapi(since = "2.5")]
pub struct SchemaInfoEnum {
    /// the enum type's members, in no particular order (since
    /// 6.2).
    #[qapi(name = "members")]
    pub members: Vec<SchemaInfoEnumMember>,
    /// the enumeration type's member names, in no particular
    /// order.  Redundant with @members.  Just for backward
    /// compatibility.
    #[qapi(name = "values")]
    #[qapi(feature = "deprecated")]
    pub values: Vec<String>,
}
/// An object member.
#[qapi(name = "SchemaInfoEnumMember")]
#[qapi(since = "6.2")]
pub struct SchemaInfoEnumMember {
    /// the member's name, as defined in the QAPI schema.
    #[qapi(name = "name")]
    pub name: String,
    /// names of features associated with the member, in no
    /// particular order.
    #[qapi(name = "features")]
    pub features: Option<Vec<String>>,
}
/// Additional SchemaInfo members for meta-type 'array'.
#[qapi(name = "SchemaInfoArray")]
#[qapi(since = "2.5")]
pub struct SchemaInfoArray {
    /// the array type's element type.
    ///
    /// Values of this type are JSON array on the wire.
    #[qapi(name = "element-type")]
    pub element_type: String,
}
/// Additional SchemaInfo members for meta-type 'object'.
#[qapi(name = "SchemaInfoObject")]
#[qapi(since = "2.5")]
pub struct SchemaInfoObject {
    /// the object type's (non-variant) members, in no particular
    /// order.
    #[qapi(name = "members")]
    pub members: Vec<SchemaInfoObjectMember>,
    /// the name of the member serving as type tag.  An element of
    /// @members with this name must exist.
    #[qapi(name = "tag")]
    pub tag: Option<String>,
    /// variant members, i.e. additional members that depend on
    /// the type tag's value.  Present exactly when @tag is present.
    /// The variants are in no particular order, and may even differ
    /// from the order of the values of the enum type of the @tag.
    ///
    /// Values of this type are JSON object on the wire.
    #[qapi(name = "variants")]
    pub variants: Option<Vec<SchemaInfoObjectVariant>>,
}
/// An object member.
#[qapi(name = "SchemaInfoObjectMember")]
#[qapi(since = "2.5")]
pub struct SchemaInfoObjectMember {
    /// the member's name, as defined in the QAPI schema.
    #[qapi(name = "name")]
    pub name: String,
    /// the name of the member's type.
    #[qapi(name = "type")]
    pub r#type: String,
    /// default when used as command parameter.  If absent, the
    /// parameter is mandatory.  If present, the value must be null.
    /// The parameter is optional, and behavior when it's missing is not
    /// specified here.  Future extension: if present and non-null, the
    /// parameter is optional, and defaults to this value.
    #[qapi(name = "default")]
    pub default: Option<serde_json::Value>,
    /// names of features associated with the member, in no
    /// particular order.  (since 5.0)
    #[qapi(name = "features")]
    pub features: Option<Vec<String>>,
}
/// The variant members for a value of the type tag.
#[qapi(name = "SchemaInfoObjectVariant")]
#[qapi(since = "2.5")]
pub struct SchemaInfoObjectVariant {
    /// a value of the type tag.
    #[qapi(name = "case")]
    pub case: String,
    /// the name of the object type that provides the variant members
    /// when the type tag has value @case.
    #[qapi(name = "type")]
    pub r#type: String,
}
/// Additional SchemaInfo members for meta-type 'alternate'.
#[qapi(name = "SchemaInfoAlternate")]
#[qapi(since = "2.5")]
pub struct SchemaInfoAlternate {
    /// the alternate type's members, in no particular order.  The
    /// members' wire encoding is distinct, see
    /// :doc:`/devel/qapi-code-gen` section Alternate types.
    ///
    /// On the wire, this can be any of the members.
    #[qapi(name = "members")]
    pub members: Vec<SchemaInfoAlternateMember>,
}
/// An alternate member.
#[qapi(name = "SchemaInfoAlternateMember")]
#[qapi(since = "2.5")]
pub struct SchemaInfoAlternateMember {
    /// the name of the member's type.
    #[qapi(name = "type")]
    pub r#type: String,
}
/// Additional SchemaInfo members for meta-type 'command'.
#[qapi(name = "SchemaInfoCommand")]
#[qapi(since = "2.5")]
pub struct SchemaInfoCommand {
    /// the name of the object type that provides the command's
    /// parameters.
    #[qapi(name = "arg-type")]
    pub arg_type: String,
    /// the name of the command's result type.
    #[qapi(name = "ret-type")]
    pub ret_type: String,
    /// whether the command allows out-of-band execution,
    /// defaults to false (Since: 2.12)
    ///
    /// TODO: @success-response (currently irrelevant, because it's QGA, not
    /// QMP)
    #[qapi(name = "allow-oob")]
    pub allow_oob: Option<bool>,
}
/// Additional SchemaInfo members for meta-type 'event'.
#[qapi(name = "SchemaInfoEvent")]
#[qapi(since = "2.5")]
pub struct SchemaInfoEvent {
    /// the name of the object type that provides the event's
    /// parameters.
    #[qapi(name = "arg-type")]
    pub arg_type: String,
}
// path end:	qapi/introspect.json
// path begin:	qapi/qom.json
#[qapi(name = "ObjectPropertyInfo")]
#[qapi(since = "1.2")]
pub struct ObjectPropertyInfo {
    /// the name of the property
    #[qapi(name = "name")]
    pub name: String,
    /// the type of the property.  This will typically come in one of
    /// four forms:
    ///
    /// 1) A primitive type such as 'u8', 'u16', 'bool', 'str', or
    /// 'double'.  These types are mapped to the appropriate JSON
    /// type.
    ///
    /// 2) A child type in the form 'child<subtype>' where subtype is a
    /// qdev device type name.  Child properties create the
    /// composition tree.
    ///
    /// 3) A link type in the form 'link<subtype>' where subtype is a
    /// qdev device type name.  Link properties form the device model
    /// graph.
    #[qapi(name = "type")]
    pub r#type: String,
    /// if specified, the description of the property.
    #[qapi(name = "description")]
    pub description: Option<String>,
    /// the default value, if any (since 5.0)
    #[qapi(name = "default-value")]
    pub default_value: Option<serde_json::Value>,
}
/// This command will list any properties of a object given a path in
/// the object model.
#[qapi(name = "qom-list")]
#[qapi(since = "1.2")]
#[qapi(returns = "Vec<ObjectPropertyInfo>")]
#[qapi(allow_preconfig)]
pub struct QomList {
    /// the path within the object model.  See @qom-get for a
    /// description of this parameter.
    #[qapi(name = "path")]
    pub path: String,
}
/// This command will get a property from a object model path and return
/// the value.
#[qapi(name = "qom-get")]
#[qapi(since = "1.2")]
#[qapi(returns = "any")]
#[qapi(allow_preconfig)]
pub struct QomGet {
    /// The path within the object model.  There are two forms of
    /// supported paths--absolute and partial paths.
    ///
    /// Absolute paths are derived from the root object and can follow
    /// child<> or link<> properties.  Since they can follow link<>
    /// properties, they can be arbitrarily long.  Absolute paths look
    /// like absolute filenames and are prefixed  with a leading slash.
    ///
    /// Partial paths look like relative filenames.  They do not begin
    /// with a prefix.  The matching rules for partial paths are subtle
    /// but designed to make specifying objects easy.  At each level of
    /// the composition tree, the partial path is matched as an absolute
    /// path.  The first match is not returned.  At least two matches
    /// are searched for.  A successful result is only returned if only
    /// one match is found.  If more than one match is found, a flag is
    /// return to indicate that the match was ambiguous.
    #[qapi(name = "path")]
    pub path: String,
    /// The property name to read
    #[qapi(name = "property")]
    pub property: String,
}
/// This command will set a property from a object model path.
#[qapi(name = "qom-set")]
#[qapi(since = "1.2")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct QomSet {
    /// see @qom-get for a description of this parameter
    #[qapi(name = "path")]
    pub path: String,
    /// the property name to set
    #[qapi(name = "property")]
    pub property: String,
    /// a value who's type is appropriate for the property type.
    /// See @qom-get for a description of type mapping.
    #[qapi(name = "value")]
    pub value: serde_json::Value,
}
/// This structure describes a search result from @qom-list-types
#[qapi(name = "ObjectTypeInfo")]
#[qapi(since = "1.1")]
pub struct ObjectTypeInfo {
    /// the type name found in the search
    #[qapi(name = "name")]
    pub name: String,
    /// the type is abstract and can't be directly instantiated.
    /// Omitted if false.  (since 2.10)
    #[qapi(name = "abstract")]
    pub r#abstract: Option<bool>,
    /// Name of parent type, if any (since 2.10)
    #[qapi(name = "parent")]
    pub parent: Option<String>,
}
/// This command will return a list of types given search parameters
#[qapi(name = "qom-list-types")]
#[qapi(since = "1.1")]
#[qapi(returns = "Vec<ObjectTypeInfo>")]
#[qapi(allow_preconfig)]
pub struct QomListTypes {
    /// if specified, only return types that implement this
    /// type name
    #[qapi(name = "implements")]
    pub implements: Option<String>,
    /// if true, include abstract types in the results
    #[qapi(name = "abstract")]
    pub r#abstract: Option<bool>,
}
/// List properties associated with a QOM object.
#[qapi(name = "qom-list-properties")]
#[qapi(since = "2.12")]
#[qapi(returns = "Vec<ObjectPropertyInfo>")]
#[qapi(allow_preconfig)]
pub struct QomListProperties {
    /// the type name of an object
    #[qapi(name = "typename")]
    pub typename: String,
}
/// Properties for can-host-socketcan objects.
#[qapi(name = "CanHostSocketcanProperties")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(since = "2.12")]
pub struct CanHostSocketcanProperties {
    /// interface name of the host system CAN bus to connect to
    #[qapi(name = "if")]
    pub r#if: String,
    /// object ID of the can-bus object to connect to the host
    /// interface
    #[qapi(name = "canbus")]
    pub canbus: String,
}
/// Properties for colo-compare objects.
#[qapi(name = "ColoCompareProperties")]
#[qapi(since = "2.8")]
pub struct ColoCompareProperties {
    /// name of the character device backend to use for the
    /// primary input (incoming packets are redirected to @outdev)
    #[qapi(name = "primary_in")]
    pub primary_in: String,
    /// name of the character device backend to use for
    /// secondary input (incoming packets are only compared to the input
    /// on @primary_in and then dropped)
    #[qapi(name = "secondary_in")]
    pub secondary_in: String,
    /// name of the character device backend to use for output
    #[qapi(name = "outdev")]
    pub outdev: String,
    /// name of the iothread to run in
    #[qapi(name = "iothread")]
    pub iothread: String,
    /// name of the character device backend to be used to
    /// communicate with the remote colo-frame (only for Xen COLO)
    #[qapi(name = "notify_dev")]
    pub notify_dev: Option<String>,
    /// the maximum time to hold a packet from @primary_in
    /// for comparison with an incoming packet on @secondary_in in
    /// milliseconds (default: 3000)
    #[qapi(name = "compare_timeout")]
    pub compare_timeout: Option<u64>,
    /// the interval at which colo-compare checks
    /// whether packets from @primary have timed out, in milliseconds
    /// (default: 3000)
    #[qapi(name = "expired_scan_cycle")]
    pub expired_scan_cycle: Option<u32>,
    /// the maximum number of packets to keep in the queue
    /// for comparing with incoming packets from @secondary_in.  If the
    /// queue is full and additional packets are received, the
    /// additional packets are dropped.  (default: 1024)
    #[qapi(name = "max_queue_size")]
    pub max_queue_size: Option<u32>,
    /// if true, vnet header support is enabled
    /// (default: false)
    #[qapi(name = "vnet_hdr_support")]
    pub vnet_hdr_support: Option<bool>,
}
/// Properties for cryptodev-backend and cryptodev-backend-builtin
/// objects.
#[qapi(name = "CryptodevBackendProperties")]
#[qapi(since = "2.8")]
pub struct CryptodevBackendProperties {
    /// the number of queues for the cryptodev backend.  Ignored
    /// for cryptodev-backend and must be 1 for
    /// cryptodev-backend-builtin.  (default: 1)
    #[qapi(name = "queues")]
    pub queues: Option<u32>,
    /// limit total bytes per second (Since 8.0)
    #[qapi(name = "throttle-bps")]
    pub throttle_bps: Option<u64>,
    /// limit total operations per second (Since 8.0)
    #[qapi(name = "throttle-ops")]
    pub throttle_ops: Option<u64>,
}
/// Properties for cryptodev-vhost-user objects.
#[qapi(name = "CryptodevVhostUserProperties")]
#[qapi(condition = "CONFIG_VHOST_CRYPTO")]
#[qapi(since = "2.12")]
pub struct CryptodevVhostUserProperties {
    /// the number of queues for the cryptodev backend.  Ignored
    /// for cryptodev-backend and must be 1 for
    /// cryptodev-backend-builtin.  (default: 1)
    #[qapi(name = "queues")]
    pub queues: Option<u32>,
    /// limit total bytes per second (Since 8.0)
    #[qapi(name = "throttle-bps")]
    pub throttle_bps: Option<u64>,
    /// limit total operations per second (Since 8.0)
    #[qapi(name = "throttle-ops")]
    pub throttle_ops: Option<u64>,
    /// the name of a Unix domain socket character device that
    /// connects to the vhost-user server
    #[qapi(name = "chardev")]
    pub chardev: String,
}
/// Properties for dbus-vmstate objects.
#[qapi(name = "DBusVMStateProperties")]
#[qapi(since = "5.0")]
pub struct DBusVmStateProperties {
    /// the name of the DBus bus to connect to
    #[qapi(name = "addr")]
    pub addr: String,
    /// a comma separated list of DBus IDs of helpers whose data
    /// should be included in the VM state on migration
    #[qapi(name = "id-list")]
    pub id_list: Option<String>,
}
/// Indicates where to insert a netfilter relative to a given other
/// filter.
#[qapi(name = "NetfilterInsert")]
#[qapi(since = "5.0")]
pub enum NetfilterInsert {
    /// insert before the specified filter
    #[qapi(name = "before")]
    Before,
    /// insert behind the specified filter
    #[qapi(name = "behind")]
    Behind,
}
/// Properties for objects of classes derived from netfilter.
#[qapi(name = "NetfilterProperties")]
#[qapi(since = "2.5")]
pub struct NetfilterProperties {
    /// id of the network device backend to filter
    #[qapi(name = "netdev")]
    pub netdev: String,
    /// indicates which queue(s) to filter (default: all)
    #[qapi(name = "queue")]
    pub queue: Option<NetFilterDirection>,
    /// indicates whether the filter is enabled ("on") or disabled
    /// ("off") (default: "on")
    #[qapi(name = "status")]
    pub status: Option<String>,
    /// specifies where the filter should be inserted in the
    /// filter list.  "head" means the filter is inserted at the head of
    /// the filter list, before any existing filters.  "tail" means the
    /// filter is inserted at the tail of the filter list, behind any
    /// existing filters (default).  "id=<id>" means the filter is
    /// inserted before or behind the filter specified by <id>,
    /// depending on the @insert property.  (default: "tail")
    #[qapi(name = "position")]
    pub position: Option<String>,
    /// where to insert the filter relative to the filter given in
    /// @position.  Ignored if @position is "head" or "tail".
    /// (default: behind)
    #[qapi(name = "insert")]
    pub insert: Option<NetfilterInsert>,
}
/// Properties for filter-buffer objects.
#[qapi(name = "FilterBufferProperties")]
#[qapi(since = "2.5")]
pub struct FilterBufferProperties {
    /// id of the network device backend to filter
    #[qapi(name = "netdev")]
    pub netdev: String,
    /// indicates which queue(s) to filter (default: all)
    #[qapi(name = "queue")]
    pub queue: Option<NetFilterDirection>,
    /// indicates whether the filter is enabled ("on") or disabled
    /// ("off") (default: "on")
    #[qapi(name = "status")]
    pub status: Option<String>,
    /// specifies where the filter should be inserted in the
    /// filter list.  "head" means the filter is inserted at the head of
    /// the filter list, before any existing filters.  "tail" means the
    /// filter is inserted at the tail of the filter list, behind any
    /// existing filters (default).  "id=<id>" means the filter is
    /// inserted before or behind the filter specified by <id>,
    /// depending on the @insert property.  (default: "tail")
    #[qapi(name = "position")]
    pub position: Option<String>,
    /// where to insert the filter relative to the filter given in
    /// @position.  Ignored if @position is "head" or "tail".
    /// (default: behind)
    #[qapi(name = "insert")]
    pub insert: Option<NetfilterInsert>,
    /// a non-zero interval in microseconds.  All packets
    /// arriving in the given interval are delayed until the end of the
    /// interval.
    #[qapi(name = "interval")]
    pub interval: u32,
}
/// Properties for filter-dump objects.
#[qapi(name = "FilterDumpProperties")]
#[qapi(since = "2.5")]
pub struct FilterDumpProperties {
    /// id of the network device backend to filter
    #[qapi(name = "netdev")]
    pub netdev: String,
    /// indicates which queue(s) to filter (default: all)
    #[qapi(name = "queue")]
    pub queue: Option<NetFilterDirection>,
    /// indicates whether the filter is enabled ("on") or disabled
    /// ("off") (default: "on")
    #[qapi(name = "status")]
    pub status: Option<String>,
    /// specifies where the filter should be inserted in the
    /// filter list.  "head" means the filter is inserted at the head of
    /// the filter list, before any existing filters.  "tail" means the
    /// filter is inserted at the tail of the filter list, behind any
    /// existing filters (default).  "id=<id>" means the filter is
    /// inserted before or behind the filter specified by <id>,
    /// depending on the @insert property.  (default: "tail")
    #[qapi(name = "position")]
    pub position: Option<String>,
    /// where to insert the filter relative to the filter given in
    /// @position.  Ignored if @position is "head" or "tail".
    /// (default: behind)
    #[qapi(name = "insert")]
    pub insert: Option<NetfilterInsert>,
    /// the filename where the dumped packets should be stored
    #[qapi(name = "file")]
    pub file: String,
    /// maximum number of bytes in a packet that are stored
    /// (default: 65536)
    #[qapi(name = "maxlen")]
    pub maxlen: Option<u32>,
}
/// Properties for filter-mirror objects.
#[qapi(name = "FilterMirrorProperties")]
#[qapi(since = "2.6")]
pub struct FilterMirrorProperties {
    /// id of the network device backend to filter
    #[qapi(name = "netdev")]
    pub netdev: String,
    /// indicates which queue(s) to filter (default: all)
    #[qapi(name = "queue")]
    pub queue: Option<NetFilterDirection>,
    /// indicates whether the filter is enabled ("on") or disabled
    /// ("off") (default: "on")
    #[qapi(name = "status")]
    pub status: Option<String>,
    /// specifies where the filter should be inserted in the
    /// filter list.  "head" means the filter is inserted at the head of
    /// the filter list, before any existing filters.  "tail" means the
    /// filter is inserted at the tail of the filter list, behind any
    /// existing filters (default).  "id=<id>" means the filter is
    /// inserted before or behind the filter specified by <id>,
    /// depending on the @insert property.  (default: "tail")
    #[qapi(name = "position")]
    pub position: Option<String>,
    /// where to insert the filter relative to the filter given in
    /// @position.  Ignored if @position is "head" or "tail".
    /// (default: behind)
    #[qapi(name = "insert")]
    pub insert: Option<NetfilterInsert>,
    /// the name of a character device backend to which all
    /// incoming packets are mirrored
    #[qapi(name = "outdev")]
    pub outdev: String,
    /// if true, vnet header support is enabled
    /// (default: false)
    #[qapi(name = "vnet_hdr_support")]
    pub vnet_hdr_support: Option<bool>,
}
/// Properties for filter-redirector objects.
///
/// At least one of @indev or @outdev must be present.  If both are
/// present, they must not refer to the same character device backend.
#[qapi(name = "FilterRedirectorProperties")]
#[qapi(since = "2.6")]
pub struct FilterRedirectorProperties {
    /// id of the network device backend to filter
    #[qapi(name = "netdev")]
    pub netdev: String,
    /// indicates which queue(s) to filter (default: all)
    #[qapi(name = "queue")]
    pub queue: Option<NetFilterDirection>,
    /// indicates whether the filter is enabled ("on") or disabled
    /// ("off") (default: "on")
    #[qapi(name = "status")]
    pub status: Option<String>,
    /// specifies where the filter should be inserted in the
    /// filter list.  "head" means the filter is inserted at the head of
    /// the filter list, before any existing filters.  "tail" means the
    /// filter is inserted at the tail of the filter list, behind any
    /// existing filters (default).  "id=<id>" means the filter is
    /// inserted before or behind the filter specified by <id>,
    /// depending on the @insert property.  (default: "tail")
    #[qapi(name = "position")]
    pub position: Option<String>,
    /// where to insert the filter relative to the filter given in
    /// @position.  Ignored if @position is "head" or "tail".
    /// (default: behind)
    #[qapi(name = "insert")]
    pub insert: Option<NetfilterInsert>,
    /// the name of a character device backend from which packets
    /// are received and redirected to the filtered network device
    #[qapi(name = "indev")]
    pub indev: Option<String>,
    /// the name of a character device backend to which all
    /// incoming packets are redirected
    #[qapi(name = "outdev")]
    pub outdev: Option<String>,
    /// if true, vnet header support is enabled
    /// (default: false)
    #[qapi(name = "vnet_hdr_support")]
    pub vnet_hdr_support: Option<bool>,
}
/// Properties for filter-rewriter objects.
#[qapi(name = "FilterRewriterProperties")]
#[qapi(since = "2.8")]
pub struct FilterRewriterProperties {
    /// id of the network device backend to filter
    #[qapi(name = "netdev")]
    pub netdev: String,
    /// indicates which queue(s) to filter (default: all)
    #[qapi(name = "queue")]
    pub queue: Option<NetFilterDirection>,
    /// indicates whether the filter is enabled ("on") or disabled
    /// ("off") (default: "on")
    #[qapi(name = "status")]
    pub status: Option<String>,
    /// specifies where the filter should be inserted in the
    /// filter list.  "head" means the filter is inserted at the head of
    /// the filter list, before any existing filters.  "tail" means the
    /// filter is inserted at the tail of the filter list, behind any
    /// existing filters (default).  "id=<id>" means the filter is
    /// inserted before or behind the filter specified by <id>,
    /// depending on the @insert property.  (default: "tail")
    #[qapi(name = "position")]
    pub position: Option<String>,
    /// where to insert the filter relative to the filter given in
    /// @position.  Ignored if @position is "head" or "tail".
    /// (default: behind)
    #[qapi(name = "insert")]
    pub insert: Option<NetfilterInsert>,
    /// if true, vnet header support is enabled
    /// (default: false)
    #[qapi(name = "vnet_hdr_support")]
    pub vnet_hdr_support: Option<bool>,
}
/// Properties for input-barrier objects.
#[qapi(name = "InputBarrierProperties")]
#[qapi(since = "4.2")]
pub struct InputBarrierProperties {
    /// the screen name as declared in the screens section of
    /// barrier.conf
    #[qapi(name = "name")]
    pub name: String,
    /// hostname of the Barrier server (default: "localhost")
    #[qapi(name = "server")]
    pub server: Option<String>,
    /// TCP port of the Barrier server (default: "24800")
    #[qapi(name = "port")]
    pub port: Option<String>,
    /// x coordinate of the leftmost pixel on the guest screen
    /// (default: "0")
    #[qapi(name = "x-origin")]
    pub x_origin: Option<String>,
    /// y coordinate of the topmost pixel on the guest screen
    /// (default: "0")
    #[qapi(name = "y-origin")]
    pub y_origin: Option<String>,
    /// the width of secondary screen in pixels (default: "1920")
    #[qapi(name = "width")]
    pub width: Option<String>,
    /// the height of secondary screen in pixels (default: "1080")
    #[qapi(name = "height")]
    pub height: Option<String>,
}
/// Properties for input-linux objects.
#[qapi(name = "InputLinuxProperties")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(since = "2.6")]
pub struct InputLinuxProperties {
    /// the path of the host evdev device to use
    #[qapi(name = "evdev")]
    pub evdev: String,
    /// if true, grab is toggled for all devices (e.g. both
    /// keyboard and mouse) instead of just one device (default: false)
    #[qapi(name = "grab_all")]
    pub grab_all: Option<bool>,
    /// enables auto-repeat events (default: false)
    #[qapi(name = "repeat")]
    pub repeat: Option<bool>,
    /// the key or key combination that toggles device grab
    /// (default: ctrl-ctrl)
    #[qapi(name = "grab-toggle")]
    pub grab_toggle: Option<GrabToggleKeys>,
}
/// Common properties for event loops
#[qapi(name = "EventLoopBaseProperties")]
#[qapi(since = "7.1")]
pub struct EventLoopBaseProperties {
    /// maximum number of requests in a batch for the AIO
    /// engine, 0 means that the engine will use its default.
    /// (default: 0)
    #[qapi(name = "aio-max-batch")]
    pub aio_max_batch: Option<i64>,
    /// minimum number of threads reserved in the thread
    /// pool (default:0)
    #[qapi(name = "thread-pool-min")]
    pub thread_pool_min: Option<i64>,
    /// maximum number of threads the thread pool can
    /// contain (default:64)
    #[qapi(name = "thread-pool-max")]
    pub thread_pool_max: Option<i64>,
}
/// Properties for iothread objects.
#[qapi(name = "IothreadProperties")]
#[qapi(since = "2.0")]
pub struct IothreadProperties {
    /// maximum number of requests in a batch for the AIO
    /// engine, 0 means that the engine will use its default.
    /// (default: 0)
    #[qapi(name = "aio-max-batch")]
    pub aio_max_batch: Option<i64>,
    /// minimum number of threads reserved in the thread
    /// pool (default:0)
    #[qapi(name = "thread-pool-min")]
    pub thread_pool_min: Option<i64>,
    /// maximum number of threads the thread pool can
    /// contain (default:64)
    #[qapi(name = "thread-pool-max")]
    pub thread_pool_max: Option<i64>,
    /// the maximum number of nanoseconds to busy wait for
    /// events.  0 means polling is disabled (default: 32768 on POSIX
    /// hosts, 0 otherwise)
    #[qapi(name = "poll-max-ns")]
    pub poll_max_ns: Option<i64>,
    /// the multiplier used to increase the polling time when
    /// the algorithm detects it is missing events due to not polling
    /// long enough.  0 selects a default behaviour (default: 0)
    #[qapi(name = "poll-grow")]
    pub poll_grow: Option<i64>,
    /// the divisor used to decrease the polling time when the
    /// algorithm detects it is spending too long polling without
    /// encountering events.  0 selects a default behaviour (default: 0)
    ///
    /// The @aio-max-batch option is available since 6.1.
    #[qapi(name = "poll-shrink")]
    pub poll_shrink: Option<i64>,
}
/// Properties for the main-loop object.
#[qapi(name = "MainLoopProperties")]
#[qapi(since = "7.1")]
pub struct MainLoopProperties {
    /// maximum number of requests in a batch for the AIO
    /// engine, 0 means that the engine will use its default.
    /// (default: 0)
    #[qapi(name = "aio-max-batch")]
    pub aio_max_batch: Option<i64>,
    /// minimum number of threads reserved in the thread
    /// pool (default:0)
    #[qapi(name = "thread-pool-min")]
    pub thread_pool_min: Option<i64>,
    /// maximum number of threads the thread pool can
    /// contain (default:64)
    #[qapi(name = "thread-pool-max")]
    pub thread_pool_max: Option<i64>,
}
/// Properties for objects of classes derived from memory-backend.
#[qapi(name = "MemoryBackendProperties")]
#[qapi(since = "2.1")]
pub struct MemoryBackendProperties {
    /// if true, include the memory in core dumps (default depends on
    /// the machine type)
    #[qapi(name = "dump")]
    pub dump: Option<bool>,
    /// the list of NUMA host nodes to bind the memory to
    #[qapi(name = "host-nodes")]
    pub host_nodes: Option<Vec<u16>>,
    /// if true, mark the memory as mergeable (default depends on
    /// the machine type)
    #[qapi(name = "merge")]
    pub merge: Option<bool>,
    /// the NUMA policy (default: 'default')
    #[qapi(name = "policy")]
    pub policy: Option<HostMemPolicy>,
    /// if true, preallocate memory (default: false)
    #[qapi(name = "prealloc")]
    pub prealloc: Option<bool>,
    /// number of CPU threads to use for prealloc
    /// (default: 1)
    #[qapi(name = "prealloc-threads")]
    pub prealloc_threads: Option<u32>,
    /// thread context to use for creation of
    /// preallocation threads (default: none) (since 7.2)
    #[qapi(name = "prealloc-context")]
    pub prealloc_context: Option<String>,
    /// if false, the memory is private to QEMU; if true, it is
    /// shared (default false for backends memory-backend-file and
    /// memory-backend-ram, true for backends memory-backend-epc,
    /// memory-backend-memfd, and memory-backend-shm)
    #[qapi(name = "share")]
    pub share: Option<bool>,
    /// if true, reserve swap space (or huge pages) if applicable
    /// (default: true) (since 6.1)
    #[qapi(name = "reserve")]
    pub reserve: Option<bool>,
    /// size of the memory region in bytes
    #[qapi(name = "size")]
    pub size: u64,
    /// if true, the canonical path
    /// is used for ramblock-id.  Disable this for 4.0 machine types or
    /// older to allow migration with newer QEMU versions.
    /// (default: false generally, but true for machine types <= 4.0)
    #[qapi(name = "x-use-canonical-path-for-ramblock-id")]
    pub x_use_canonical_path_for_ramblock_id: Option<bool>,
}
/// Properties for memory-backend-file objects.
#[qapi(name = "MemoryBackendFileProperties")]
#[qapi(since = "2.1")]
pub struct MemoryBackendFileProperties {
    /// if true, include the memory in core dumps (default depends on
    /// the machine type)
    #[qapi(name = "dump")]
    pub dump: Option<bool>,
    /// the list of NUMA host nodes to bind the memory to
    #[qapi(name = "host-nodes")]
    pub host_nodes: Option<Vec<u16>>,
    /// if true, mark the memory as mergeable (default depends on
    /// the machine type)
    #[qapi(name = "merge")]
    pub merge: Option<bool>,
    /// the NUMA policy (default: 'default')
    #[qapi(name = "policy")]
    pub policy: Option<HostMemPolicy>,
    /// if true, preallocate memory (default: false)
    #[qapi(name = "prealloc")]
    pub prealloc: Option<bool>,
    /// number of CPU threads to use for prealloc
    /// (default: 1)
    #[qapi(name = "prealloc-threads")]
    pub prealloc_threads: Option<u32>,
    /// thread context to use for creation of
    /// preallocation threads (default: none) (since 7.2)
    #[qapi(name = "prealloc-context")]
    pub prealloc_context: Option<String>,
    /// if false, the memory is private to QEMU; if true, it is
    /// shared (default false for backends memory-backend-file and
    /// memory-backend-ram, true for backends memory-backend-epc,
    /// memory-backend-memfd, and memory-backend-shm)
    #[qapi(name = "share")]
    pub share: Option<bool>,
    /// if true, reserve swap space (or huge pages) if applicable
    /// (default: true) (since 6.1)
    #[qapi(name = "reserve")]
    pub reserve: Option<bool>,
    /// size of the memory region in bytes
    #[qapi(name = "size")]
    pub size: u64,
    /// if true, the canonical path
    /// is used for ramblock-id.  Disable this for 4.0 machine types or
    /// older to allow migration with newer QEMU versions.
    /// (default: false generally, but true for machine types <= 4.0)
    #[qapi(name = "x-use-canonical-path-for-ramblock-id")]
    pub x_use_canonical_path_for_ramblock_id: Option<bool>,
    /// the base address alignment when QEMU mmap(2)s @mem-path.
    /// Some backend stores specified by @mem-path require an alignment
    /// different than the default one used by QEMU, e.g. the device DAX
    /// /dev/dax0.0 requires 2M alignment rather than 4K.  In such
    /// cases, users can specify the required alignment via this option.
    /// 0 selects a default alignment (currently the page size).
    /// (default: 0)
    #[qapi(name = "align")]
    pub align: Option<u64>,
    /// the offset into the target file that the region starts at.
    /// You can use this option to back multiple regions with a single
    /// file.  Must be a multiple of the page size.
    /// (default: 0) (since 8.1)
    #[qapi(name = "offset")]
    pub offset: Option<u64>,
    /// if true, the file contents can be destroyed when QEMU
    /// exits, to avoid unnecessarily flushing data to the backing file.
    /// Note that @discard-data is only an optimization, and QEMU might
    /// not discard file contents if it aborts unexpectedly or is
    /// terminated using SIGKILL.  (default: false)
    #[qapi(name = "discard-data")]
    pub discard_data: Option<bool>,
    /// the path to either a shared memory or huge page
    /// filesystem mount
    #[qapi(name = "mem-path")]
    pub mem_path: String,
    /// specifies whether the backing file specified by @mem-path is
    /// in host persistent memory that can be accessed using the SNIA
    /// NVM programming model (e.g. Intel NVDIMM).
    #[qapi(name = "pmem")]
    #[qapi(condition = "CONFIG_LIBPMEM")]
    pub pmem: Option<bool>,
    /// if true, the backing file is opened read-only; if false,
    /// it is opened read-write.  (default: false)
    #[qapi(name = "readonly")]
    pub readonly: Option<bool>,
    /// whether to create Read Only Memory (ROM) that cannot be
    /// modified by the VM.  Any write attempts to such ROM will be
    /// denied.  Most use cases want writable RAM instead of ROM.
    /// However, selected use cases, like R/O NVDIMMs, can benefit from
    /// ROM.  If set to 'on', create ROM; if set to 'off', create
    /// writable RAM; if set to 'auto', the value of the @readonly
    /// property is used.  This property is primarily helpful when we
    /// want to have proper RAM in configurations that would
    /// traditionally create ROM before this property was introduced: VM
    /// templating, where we want to open a file readonly (@readonly set
    /// to true) and mark the memory to be private for QEMU (@share set
    /// to false).  For this use case, we need writable RAM instead of
    /// ROM, and want to set this property to 'off'.  (default: auto,
    /// since 8.2)
    #[qapi(name = "rom")]
    pub rom: Option<OnOffAuto>,
}
/// Properties for memory-backend-memfd objects.
#[qapi(name = "MemoryBackendMemfdProperties")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(since = "2.12")]
pub struct MemoryBackendMemfdProperties {
    /// if true, include the memory in core dumps (default depends on
    /// the machine type)
    #[qapi(name = "dump")]
    pub dump: Option<bool>,
    /// the list of NUMA host nodes to bind the memory to
    #[qapi(name = "host-nodes")]
    pub host_nodes: Option<Vec<u16>>,
    /// if true, mark the memory as mergeable (default depends on
    /// the machine type)
    #[qapi(name = "merge")]
    pub merge: Option<bool>,
    /// the NUMA policy (default: 'default')
    #[qapi(name = "policy")]
    pub policy: Option<HostMemPolicy>,
    /// if true, preallocate memory (default: false)
    #[qapi(name = "prealloc")]
    pub prealloc: Option<bool>,
    /// number of CPU threads to use for prealloc
    /// (default: 1)
    #[qapi(name = "prealloc-threads")]
    pub prealloc_threads: Option<u32>,
    /// thread context to use for creation of
    /// preallocation threads (default: none) (since 7.2)
    #[qapi(name = "prealloc-context")]
    pub prealloc_context: Option<String>,
    /// if false, the memory is private to QEMU; if true, it is
    /// shared (default false for backends memory-backend-file and
    /// memory-backend-ram, true for backends memory-backend-epc,
    /// memory-backend-memfd, and memory-backend-shm)
    #[qapi(name = "share")]
    pub share: Option<bool>,
    /// if true, reserve swap space (or huge pages) if applicable
    /// (default: true) (since 6.1)
    #[qapi(name = "reserve")]
    pub reserve: Option<bool>,
    /// size of the memory region in bytes
    #[qapi(name = "size")]
    pub size: u64,
    /// if true, the canonical path
    /// is used for ramblock-id.  Disable this for 4.0 machine types or
    /// older to allow migration with newer QEMU versions.
    /// (default: false generally, but true for machine types <= 4.0)
    #[qapi(name = "x-use-canonical-path-for-ramblock-id")]
    pub x_use_canonical_path_for_ramblock_id: Option<bool>,
    /// if true, the file to be created resides in the hugetlbfs
    /// filesystem (default: false)
    #[qapi(name = "hugetlb")]
    pub hugetlb: Option<bool>,
    /// the hugetlb page size on systems that support multiple
    /// hugetlb page sizes (it must be a power of 2 value supported by
    /// the system).  0 selects a default page size.  This option is
    /// ignored if @hugetlb is false.  (default: 0)
    #[qapi(name = "hugetlbsize")]
    pub hugetlbsize: Option<u64>,
    /// if true, create a sealed-file, which will block further
    /// resizing of the memory (default: true)
    #[qapi(name = "seal")]
    pub seal: Option<bool>,
}
/// Properties for memory-backend-shm objects.
///
/// This memory backend supports only shared memory, which is the
/// default.
#[qapi(name = "MemoryBackendShmProperties")]
#[qapi(condition = "CONFIG_POSIX")]
#[qapi(since = "9.1")]
pub struct MemoryBackendShmProperties {
    /// if true, include the memory in core dumps (default depends on
    /// the machine type)
    #[qapi(name = "dump")]
    pub dump: Option<bool>,
    /// the list of NUMA host nodes to bind the memory to
    #[qapi(name = "host-nodes")]
    pub host_nodes: Option<Vec<u16>>,
    /// if true, mark the memory as mergeable (default depends on
    /// the machine type)
    #[qapi(name = "merge")]
    pub merge: Option<bool>,
    /// the NUMA policy (default: 'default')
    #[qapi(name = "policy")]
    pub policy: Option<HostMemPolicy>,
    /// if true, preallocate memory (default: false)
    #[qapi(name = "prealloc")]
    pub prealloc: Option<bool>,
    /// number of CPU threads to use for prealloc
    /// (default: 1)
    #[qapi(name = "prealloc-threads")]
    pub prealloc_threads: Option<u32>,
    /// thread context to use for creation of
    /// preallocation threads (default: none) (since 7.2)
    #[qapi(name = "prealloc-context")]
    pub prealloc_context: Option<String>,
    /// if false, the memory is private to QEMU; if true, it is
    /// shared (default false for backends memory-backend-file and
    /// memory-backend-ram, true for backends memory-backend-epc,
    /// memory-backend-memfd, and memory-backend-shm)
    #[qapi(name = "share")]
    pub share: Option<bool>,
    /// if true, reserve swap space (or huge pages) if applicable
    /// (default: true) (since 6.1)
    #[qapi(name = "reserve")]
    pub reserve: Option<bool>,
    /// size of the memory region in bytes
    #[qapi(name = "size")]
    pub size: u64,
    /// if true, the canonical path
    /// is used for ramblock-id.  Disable this for 4.0 machine types or
    /// older to allow migration with newer QEMU versions.
    /// (default: false generally, but true for machine types <= 4.0)
    #[qapi(name = "x-use-canonical-path-for-ramblock-id")]
    pub x_use_canonical_path_for_ramblock_id: Option<bool>,
}
/// Properties for memory-backend-epc objects.
///
/// The @merge boolean option is false by default with epc
///
/// The @dump boolean option is false by default with epc
#[qapi(name = "MemoryBackendEpcProperties")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(since = "6.2")]
pub struct MemoryBackendEpcProperties {
    /// if true, include the memory in core dumps (default depends on
    /// the machine type)
    #[qapi(name = "dump")]
    pub dump: Option<bool>,
    /// the list of NUMA host nodes to bind the memory to
    #[qapi(name = "host-nodes")]
    pub host_nodes: Option<Vec<u16>>,
    /// if true, mark the memory as mergeable (default depends on
    /// the machine type)
    #[qapi(name = "merge")]
    pub merge: Option<bool>,
    /// the NUMA policy (default: 'default')
    #[qapi(name = "policy")]
    pub policy: Option<HostMemPolicy>,
    /// if true, preallocate memory (default: false)
    #[qapi(name = "prealloc")]
    pub prealloc: Option<bool>,
    /// number of CPU threads to use for prealloc
    /// (default: 1)
    #[qapi(name = "prealloc-threads")]
    pub prealloc_threads: Option<u32>,
    /// thread context to use for creation of
    /// preallocation threads (default: none) (since 7.2)
    #[qapi(name = "prealloc-context")]
    pub prealloc_context: Option<String>,
    /// if false, the memory is private to QEMU; if true, it is
    /// shared (default false for backends memory-backend-file and
    /// memory-backend-ram, true for backends memory-backend-epc,
    /// memory-backend-memfd, and memory-backend-shm)
    #[qapi(name = "share")]
    pub share: Option<bool>,
    /// if true, reserve swap space (or huge pages) if applicable
    /// (default: true) (since 6.1)
    #[qapi(name = "reserve")]
    pub reserve: Option<bool>,
    /// size of the memory region in bytes
    #[qapi(name = "size")]
    pub size: u64,
    /// if true, the canonical path
    /// is used for ramblock-id.  Disable this for 4.0 machine types or
    /// older to allow migration with newer QEMU versions.
    /// (default: false generally, but true for machine types <= 4.0)
    #[qapi(name = "x-use-canonical-path-for-ramblock-id")]
    pub x_use_canonical_path_for_ramblock_id: Option<bool>,
}
/// Properties for pr-manager-helper objects.
#[qapi(name = "PrManagerHelperProperties")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(since = "2.11")]
pub struct PrManagerHelperProperties {
    /// the path to a Unix domain socket for connecting to the
    /// external helper
    #[qapi(name = "path")]
    pub path: String,
}
/// Properties for qtest objects.
#[qapi(name = "QtestProperties")]
#[qapi(since = "6.0")]
pub struct QtestProperties {
    /// the chardev to be used to receive qtest commands on.
    #[qapi(name = "chardev")]
    pub chardev: String,
    /// the path to a log file
    #[qapi(name = "log")]
    pub log: Option<String>,
}
/// Properties for x-remote-object objects.
#[qapi(name = "RemoteObjectProperties")]
#[qapi(since = "6.0")]
pub struct RemoteObjectProperties {
    /// file descriptor name previously passed via 'getfd' command
    #[qapi(name = "fd")]
    pub fd: String,
    /// the id of the device to be associated with the file
    /// descriptor
    #[qapi(name = "devid")]
    pub devid: String,
}
/// Properties for x-vfio-user-server objects.
#[qapi(name = "VfioUserServerProperties")]
#[qapi(since = "7.1")]
pub struct VfioUserServerProperties {
    /// socket to be used by the libvfio-user library
    #[qapi(name = "socket")]
    pub socket: SocketAddress,
    /// the ID of the device to be emulated at the server
    #[qapi(name = "device")]
    pub device: String,
}
/// Properties for iommufd objects.
#[qapi(name = "IOMMUFDProperties")]
#[qapi(since = "9.0")]
pub struct IommufdProperties {
    /// file descriptor name previously passed via 'getfd' command,
    /// which represents a pre-opened /dev/iommu.  This allows the
    /// iommufd object to be shared across several subsystems (VFIO,
    /// VDPA, ...), and the file descriptor to be shared with other
    /// process, e.g. DPDK.  (default: QEMU opens /dev/iommu by itself)
    #[qapi(name = "fd")]
    pub fd: Option<String>,
}
/// Properties for acpi-generic-initiator objects.
#[qapi(name = "AcpiGenericInitiatorProperties")]
#[qapi(since = "9.0")]
pub struct AcpiGenericInitiatorProperties {
    /// PCI device ID to be associated with the node
    #[qapi(name = "pci-dev")]
    pub pci_dev: String,
    /// NUMA node associated with the PCI device
    #[qapi(name = "node")]
    pub node: u32,
}
/// Properties for objects of classes derived from rng.
#[qapi(name = "RngProperties")]
#[qapi(since = "1.3")]
pub struct RngProperties {
    /// if true, the device is opened immediately when applying
    /// this option and will probably fail when processing the next
    /// option.  Don't use; only provided for compatibility.
    /// (default: false)
    #[qapi(name = "opened")]
    #[qapi(feature = "deprecated")]
    pub opened: Option<bool>,
}
/// Properties for rng-egd objects.
#[qapi(name = "RngEgdProperties")]
#[qapi(since = "1.3")]
pub struct RngEgdProperties {
    /// if true, the device is opened immediately when applying
    /// this option and will probably fail when processing the next
    /// option.  Don't use; only provided for compatibility.
    /// (default: false)
    #[qapi(name = "opened")]
    #[qapi(feature = "deprecated")]
    pub opened: Option<bool>,
    /// the name of a character device backend that provides the
    /// connection to the RNG daemon
    #[qapi(name = "chardev")]
    pub chardev: String,
}
/// Properties for rng-random objects.
#[qapi(name = "RngRandomProperties")]
#[qapi(condition = "CONFIG_POSIX")]
#[qapi(since = "1.3")]
pub struct RngRandomProperties {
    /// if true, the device is opened immediately when applying
    /// this option and will probably fail when processing the next
    /// option.  Don't use; only provided for compatibility.
    /// (default: false)
    #[qapi(name = "opened")]
    #[qapi(feature = "deprecated")]
    pub opened: Option<bool>,
    /// the filename of the device on the host to obtain entropy
    /// from (default: "/dev/urandom")
    #[qapi(name = "filename")]
    pub filename: Option<String>,
}
/// Properties common to objects that are derivatives of sev-common.
#[qapi(name = "SevCommonProperties")]
#[qapi(since = "9.1")]
pub struct SevCommonProperties {
    /// SEV device to use (default: "/dev/sev")
    #[qapi(name = "sev-device")]
    pub sev_device: Option<String>,
    /// C-bit location in page table entry (default: 0)
    #[qapi(name = "cbitpos")]
    pub cbitpos: Option<u32>,
    /// number of bits in physical addresses that become
    /// unavailable when SEV is enabled
    #[qapi(name = "reduced-phys-bits")]
    pub reduced_phys_bits: u32,
    /// if true, add hashes of kernel/initrd/cmdline to a
    /// designated guest firmware page for measured boot with -kernel
    /// (default: false) (since 6.2)
    #[qapi(name = "kernel-hashes")]
    pub kernel_hashes: Option<bool>,
}
/// Properties for sev-guest objects.
#[qapi(name = "SevGuestProperties")]
#[qapi(since = "2.12")]
pub struct SevGuestProperties {
    /// SEV device to use (default: "/dev/sev")
    #[qapi(name = "sev-device")]
    pub sev_device: Option<String>,
    /// C-bit location in page table entry (default: 0)
    #[qapi(name = "cbitpos")]
    pub cbitpos: Option<u32>,
    /// number of bits in physical addresses that become
    /// unavailable when SEV is enabled
    #[qapi(name = "reduced-phys-bits")]
    pub reduced_phys_bits: u32,
    /// if true, add hashes of kernel/initrd/cmdline to a
    /// designated guest firmware page for measured boot with -kernel
    /// (default: false) (since 6.2)
    #[qapi(name = "kernel-hashes")]
    pub kernel_hashes: Option<bool>,
    /// guest owners DH certificate (encoded with base64)
    #[qapi(name = "dh-cert-file")]
    pub dh_cert_file: Option<String>,
    /// guest owners session parameters (encoded with base64)
    #[qapi(name = "session-file")]
    pub session_file: Option<String>,
    /// SEV policy value (default: 0x1)
    #[qapi(name = "policy")]
    pub policy: Option<u32>,
    /// SEV firmware handle (default: 0)
    #[qapi(name = "handle")]
    pub handle: Option<u32>,
    /// Use legacy KVM_SEV_INIT KVM interface for creating
    /// the VM.  The newer KVM_SEV_INIT2 interface, from Linux >= 6.10,
    /// syncs additional vCPU state when initializing the VMSA
    /// structures, which will result in a different guest measurement.
    /// Set this to 'on' to force compatibility with older QEMU or kernel
    /// versions that rely on legacy KVM_SEV_INIT behavior.  'auto' will
    /// behave identically to 'on', but will automatically switch to
    /// using KVM_SEV_INIT2 if the user specifies any additional options
    /// that require it.  If set to 'off', QEMU will require
    /// KVM_SEV_INIT2 unconditionally.
    /// (default: off) (since 9.1)
    #[qapi(name = "legacy-vm-type")]
    pub legacy_vm_type: Option<OnOffAuto>,
}
/// Properties for sev-snp-guest objects.  Most of these are direct
/// arguments for the KVM_SNP_* interfaces documented in the Linux
/// kernel source under
/// Documentation/arch/x86/amd-memory-encryption.rst, which are in turn
/// closely coupled with the SNP_INIT/SNP_LAUNCH_* firmware commands
/// documented in the SEV-SNP Firmware ABI Specification (Rev 0.9).
///
/// More usage information is also available in the QEMU source tree
/// under docs/amd-memory-encryption.
#[qapi(name = "SevSnpGuestProperties")]
#[qapi(since = "9.1")]
pub struct SevSnpGuestProperties {
    /// SEV device to use (default: "/dev/sev")
    #[qapi(name = "sev-device")]
    pub sev_device: Option<String>,
    /// C-bit location in page table entry (default: 0)
    #[qapi(name = "cbitpos")]
    pub cbitpos: Option<u32>,
    /// number of bits in physical addresses that become
    /// unavailable when SEV is enabled
    #[qapi(name = "reduced-phys-bits")]
    pub reduced_phys_bits: u32,
    /// if true, add hashes of kernel/initrd/cmdline to a
    /// designated guest firmware page for measured boot with -kernel
    /// (default: false) (since 6.2)
    #[qapi(name = "kernel-hashes")]
    pub kernel_hashes: Option<bool>,
    /// the 'POLICY' parameter to the SNP_LAUNCH_START command, as
    /// defined in the SEV-SNP firmware ABI (default: 0x30000)
    #[qapi(name = "policy")]
    pub policy: Option<u64>,
    /// 16-byte, base64-encoded blob to report
    /// hypervisor-defined workarounds, corresponding to the 'GOSVW'
    /// parameter of the SNP_LAUNCH_START command defined in the SEV-SNP
    /// firmware ABI (default: all-zero)
    #[qapi(name = "guest-visible-workarounds")]
    pub guest_visible_workarounds: Option<String>,
    /// 96-byte, base64-encoded blob to provide the 'ID Block'
    /// structure for the SNP_LAUNCH_FINISH command defined in the
    /// SEV-SNP firmware ABI (default: all-zero)
    #[qapi(name = "id-block")]
    pub id_block: Option<String>,
    /// 4096-byte, base64-encoded blob to provide the 'ID
    /// Authentication Information Structure' for the SNP_LAUNCH_FINISH
    /// command defined in the SEV-SNP firmware ABI (default: all-zero)
    #[qapi(name = "id-auth")]
    pub id_auth: Option<String>,
    /// true if 'id-auth' blob contains the 'AUTHOR_KEY'
    /// field defined SEV-SNP firmware ABI (default: false)
    #[qapi(name = "author-key-enabled")]
    pub author_key_enabled: Option<bool>,
    /// 32-byte, base64-encoded, user-defined blob to provide to
    /// the guest, as documented for the 'HOST_DATA' parameter of the
    /// SNP_LAUNCH_FINISH command in the SEV-SNP firmware ABI (default:
    /// all-zero)
    #[qapi(name = "host-data")]
    pub host_data: Option<String>,
    /// Guests are by default allowed to choose between VLEK
    /// (Versioned Loaded Endorsement Key) or VCEK (Versioned Chip
    /// Endorsement Key) when requesting attestation reports from
    /// firmware.  Set this to true to disable the use of VCEK.
    /// (default: false) (since: 9.1)
    #[qapi(name = "vcek-disabled")]
    pub vcek_disabled: Option<bool>,
}
/// Properties for thread context objects.
#[qapi(name = "ThreadContextProperties")]
#[qapi(since = "7.2")]
pub struct ThreadContextProperties {
    /// the list of host CPU numbers used as CPU affinity for
    /// all threads created in the thread context (default: QEMU main
    /// thread CPU affinity)
    #[qapi(name = "cpu-affinity")]
    pub cpu_affinity: Option<Vec<u16>>,
    /// the list of host node numbers that will be resolved
    /// to a list of host CPU numbers used as CPU affinity.  This is a
    /// shortcut for specifying the list of host CPU numbers belonging
    /// to the host nodes manually by setting @cpu-affinity.
    /// (default: QEMU main thread affinity)
    #[qapi(name = "node-affinity")]
    pub node_affinity: Option<Vec<u16>>,
}
#[qapi(name = "ObjectType")]
#[qapi(since = "6.0")]
pub enum ObjectType {
    #[qapi(name = "acpi-generic-initiator")]
    AcpiGenericInitiator,
    #[qapi(name = "authz-list")]
    AuthzList,
    #[qapi(name = "authz-listfile")]
    AuthzListfile,
    #[qapi(name = "authz-pam")]
    AuthzPam,
    #[qapi(name = "authz-simple")]
    AuthzSimple,
    #[qapi(name = "can-bus")]
    CanBus,
    #[qapi(name = "can-host-socketcan")]
    #[qapi(condition = "CONFIG_LINUX")]
    CanHostSocketcan,
    #[qapi(name = "colo-compare")]
    ColoCompare,
    #[qapi(name = "cryptodev-backend")]
    CryptodevBackend,
    #[qapi(name = "cryptodev-backend-builtin")]
    CryptodevBackendBuiltin,
    #[qapi(name = "cryptodev-backend-lkcf")]
    CryptodevBackendLkcf,
    #[qapi(name = "cryptodev-vhost-user")]
    #[qapi(condition = "CONFIG_VHOST_CRYPTO")]
    CryptodevVhostUser,
    #[qapi(name = "dbus-vmstate")]
    DbusVmstate,
    #[qapi(name = "filter-buffer")]
    FilterBuffer,
    #[qapi(name = "filter-dump")]
    FilterDump,
    #[qapi(name = "filter-mirror")]
    FilterMirror,
    #[qapi(name = "filter-redirector")]
    FilterRedirector,
    #[qapi(name = "filter-replay")]
    FilterReplay,
    #[qapi(name = "filter-rewriter")]
    FilterRewriter,
    #[qapi(name = "input-barrier")]
    InputBarrier,
    #[qapi(name = "input-linux")]
    #[qapi(condition = "CONFIG_LINUX")]
    InputLinux,
    #[qapi(name = "iommufd")]
    Iommufd,
    #[qapi(name = "iothread")]
    Iothread,
    #[qapi(name = "main-loop")]
    MainLoop,
    #[qapi(name = "memory-backend-epc")]
    #[qapi(condition = "CONFIG_LINUX")]
    MemoryBackendEpc,
    #[qapi(name = "memory-backend-file")]
    MemoryBackendFile,
    #[qapi(name = "memory-backend-memfd")]
    #[qapi(condition = "CONFIG_LINUX")]
    MemoryBackendMemfd,
    #[qapi(name = "memory-backend-ram")]
    MemoryBackendRam,
    #[qapi(name = "memory-backend-shm")]
    #[qapi(condition = "CONFIG_POSIX")]
    MemoryBackendShm,
    #[qapi(name = "pef-guest")]
    PefGuest,
    #[qapi(name = "pr-manager-helper")]
    #[qapi(condition = "CONFIG_LINUX")]
    PrManagerHelper,
    #[qapi(name = "qtest")]
    Qtest,
    #[qapi(name = "rng-builtin")]
    RngBuiltin,
    #[qapi(name = "rng-egd")]
    RngEgd,
    #[qapi(name = "rng-random")]
    #[qapi(condition = "CONFIG_POSIX")]
    RngRandom,
    #[qapi(name = "secret")]
    Secret,
    #[qapi(name = "secret_keyring")]
    #[qapi(condition = "CONFIG_SECRET_KEYRING")]
    SecretKeyring,
    #[qapi(name = "sev-guest")]
    SevGuest,
    #[qapi(name = "sev-snp-guest")]
    SevSnpGuest,
    #[qapi(name = "thread-context")]
    ThreadContext,
    #[qapi(name = "s390-pv-guest")]
    S390PvGuest,
    #[qapi(name = "throttle-group")]
    ThrottleGroup,
    #[qapi(name = "tls-creds-anon")]
    TlsCredsAnon,
    #[qapi(name = "tls-creds-psk")]
    TlsCredsPsk,
    #[qapi(name = "tls-creds-x509")]
    TlsCredsX509,
    #[qapi(name = "tls-cipher-suites")]
    TlsCipherSuites,
    #[qapi(name = "x-remote-object")]
    #[qapi(feature = "unstable")]
    XRemoteObject,
    #[qapi(name = "x-vfio-user-server")]
    #[qapi(feature = "unstable")]
    XVfioUserServer,
}
pub enum ObjectOptionsBranch {
    #[qapi(name = "acpi-generic-initiator")]
    AcpiGenericInitiator(AcpiGenericInitiatorProperties),
    #[qapi(name = "authz-list")]
    AuthzList(AuthZListProperties),
    #[qapi(name = "authz-listfile")]
    AuthzListfile(AuthZListFileProperties),
    #[qapi(name = "authz-pam")]
    AuthzPam(AuthZpamProperties),
    #[qapi(name = "authz-simple")]
    AuthzSimple(AuthZSimpleProperties),
    #[qapi(name = "can-host-socketcan")]
    #[qapi(condition = "CONFIG_LINUX")]
    CanHostSocketcan(CanHostSocketcanProperties),
    #[qapi(name = "colo-compare")]
    ColoCompare(ColoCompareProperties),
    #[qapi(name = "cryptodev-backend")]
    CryptodevBackend(CryptodevBackendProperties),
    #[qapi(name = "cryptodev-backend-builtin")]
    CryptodevBackendBuiltin(CryptodevBackendProperties),
    #[qapi(name = "cryptodev-backend-lkcf")]
    CryptodevBackendLkcf(CryptodevBackendProperties),
    #[qapi(name = "cryptodev-vhost-user")]
    #[qapi(condition = "CONFIG_VHOST_CRYPTO")]
    CryptodevVhostUser(CryptodevVhostUserProperties),
    #[qapi(name = "dbus-vmstate")]
    DbusVmstate(DBusVmStateProperties),
    #[qapi(name = "filter-buffer")]
    FilterBuffer(FilterBufferProperties),
    #[qapi(name = "filter-dump")]
    FilterDump(FilterDumpProperties),
    #[qapi(name = "filter-mirror")]
    FilterMirror(FilterMirrorProperties),
    #[qapi(name = "filter-redirector")]
    FilterRedirector(FilterRedirectorProperties),
    #[qapi(name = "filter-replay")]
    FilterReplay(NetfilterProperties),
    #[qapi(name = "filter-rewriter")]
    FilterRewriter(FilterRewriterProperties),
    #[qapi(name = "input-barrier")]
    InputBarrier(InputBarrierProperties),
    #[qapi(name = "input-linux")]
    #[qapi(condition = "CONFIG_LINUX")]
    InputLinux(InputLinuxProperties),
    #[qapi(name = "iommufd")]
    Iommufd(IommufdProperties),
    #[qapi(name = "iothread")]
    Iothread(IothreadProperties),
    #[qapi(name = "main-loop")]
    MainLoop(MainLoopProperties),
    #[qapi(name = "memory-backend-epc")]
    #[qapi(condition = "CONFIG_LINUX")]
    MemoryBackendEpc(MemoryBackendEpcProperties),
    #[qapi(name = "memory-backend-file")]
    MemoryBackendFile(MemoryBackendFileProperties),
    #[qapi(name = "memory-backend-memfd")]
    #[qapi(condition = "CONFIG_LINUX")]
    MemoryBackendMemfd(MemoryBackendMemfdProperties),
    #[qapi(name = "memory-backend-ram")]
    MemoryBackendRam(MemoryBackendProperties),
    #[qapi(name = "memory-backend-shm")]
    #[qapi(condition = "CONFIG_POSIX")]
    MemoryBackendShm(MemoryBackendShmProperties),
    #[qapi(name = "pr-manager-helper")]
    #[qapi(condition = "CONFIG_LINUX")]
    PrManagerHelper(PrManagerHelperProperties),
    #[qapi(name = "qtest")]
    Qtest(QtestProperties),
    #[qapi(name = "rng-builtin")]
    RngBuiltin(RngProperties),
    #[qapi(name = "rng-egd")]
    RngEgd(RngEgdProperties),
    #[qapi(name = "rng-random")]
    #[qapi(condition = "CONFIG_POSIX")]
    RngRandom(RngRandomProperties),
    #[qapi(name = "secret")]
    Secret(SecretProperties),
    #[qapi(name = "secret_keyring")]
    #[qapi(condition = "CONFIG_SECRET_KEYRING")]
    SecretKeyring(SecretKeyringProperties),
    #[qapi(name = "sev-guest")]
    SevGuest(SevGuestProperties),
    #[qapi(name = "sev-snp-guest")]
    SevSnpGuest(SevSnpGuestProperties),
    #[qapi(name = "thread-context")]
    ThreadContext(ThreadContextProperties),
    #[qapi(name = "throttle-group")]
    ThrottleGroup(ThrottleGroupProperties),
    #[qapi(name = "tls-creds-anon")]
    TlsCredsAnon(TlsCredsAnonProperties),
    #[qapi(name = "tls-creds-psk")]
    TlsCredsPsk(TlsCredsPskProperties),
    #[qapi(name = "tls-creds-x509")]
    TlsCredsX509(TlsCredsX509Properties),
    #[qapi(name = "tls-cipher-suites")]
    TlsCipherSuites(TlsCredsProperties),
    #[qapi(name = "x-remote-object")]
    XRemoteObject(RemoteObjectProperties),
    #[qapi(name = "x-vfio-user-server")]
    XVfioUserServer(VfioUserServerProperties),
}
/// Describes the options of a user creatable QOM object.
#[qapi(name = "ObjectOptions")]
#[qapi(since = "6.0")]
pub struct ObjectOptions {
    /// the class name for the object to be created
    #[qapi(name = "qom-type")]
    #[qapi(discriminator)]
    pub qom_type: ObjectType,
    /// the name of the new object
    #[qapi(name = "id")]
    pub id: String,
    #[qapi(union)]
    pub u: Option<ObjectOptionsBranch>,
}
/// Create a QOM object.
#[qapi(name = "object-add")]
#[qapi(since = "2.0")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct ObjectAdd {
    #[qapi(flatten)]
    pub data: ObjectOptions,
}
/// Remove a QOM object.
#[qapi(name = "object-del")]
#[qapi(since = "2.0")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct ObjectDel {
    /// the name of the QOM object to remove
    #[qapi(name = "id")]
    pub id: String,
}
// path end:	qapi/qom.json
// path begin:	qapi/qdev.json
/// List properties associated with a device.
#[qapi(name = "device-list-properties")]
#[qapi(since = "1.2")]
#[qapi(returns = "Vec<ObjectPropertyInfo>")]
pub struct DeviceListProperties {
    /// the type name of a device
    #[qapi(name = "typename")]
    pub typename: String,
}
/// Add a device.
#[qapi(name = "device_add")]
#[qapi(feature = "json-cli")]
#[qapi(feature = "json-cli-hotplug")]
#[qapi(since = "0.13")]
#[qapi(returns = "()")]
pub struct DeviceAdd {
    /// the name of the new device's driver
    #[qapi(name = "driver")]
    pub driver: String,
    /// the device's parent bus (device tree path)
    #[qapi(name = "bus")]
    pub bus: Option<String>,
    /// the device's ID, must be unique
    #[qapi(name = "id")]
    pub id: Option<String>,
}
/// Remove a device from a guest
#[qapi(name = "device_del")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct DeviceDel {
    /// the device's ID or QOM path
    #[qapi(name = "id")]
    pub id: String,
}
/// Emitted whenever the device removal completion is acknowledged by
/// the guest.  At this point, it's safe to reuse the specified device
/// ID.  Device removal can be initiated by the guest or by HMP/QMP
/// commands.
#[qapi(name = "DEVICE_DELETED")]
#[qapi(since = "1.5")]
pub struct DeviceDeleted {
    /// the device's ID if it has one
    #[qapi(name = "device")]
    pub device: Option<String>,
    /// the device's QOM path
    #[qapi(name = "path")]
    pub path: String,
}
/// Emitted when a device hot unplug fails due to a guest reported
/// error.
#[qapi(name = "DEVICE_UNPLUG_GUEST_ERROR")]
#[qapi(since = "6.2")]
pub struct DeviceUnplugGuestError {
    /// the device's ID if it has one
    #[qapi(name = "device")]
    pub device: Option<String>,
    /// the device's QOM path
    #[qapi(name = "path")]
    pub path: String,
}
// path end:	qapi/qdev.json
// path begin:	qapi/machine-common.json
/// An enumeration of CPU entitlements that can be assumed by a virtual
/// S390 CPU
#[qapi(name = "CpuS390Entitlement")]
#[qapi(since = "8.2")]
pub enum CpuS390Entitlement {
    #[qapi(name = "auto")]
    Auto,
    #[qapi(name = "low")]
    Low,
    #[qapi(name = "medium")]
    Medium,
    #[qapi(name = "high")]
    High,
}
// path end:	qapi/machine-common.json
// path begin:	qapi/machine.json
/// The comprehensive enumeration of QEMU system emulation ("softmmu")
/// targets.  Run "./configure --help" in the project root directory,
/// and look for the \*-softmmu targets near the "--target-list" option.
/// The individual target constants are not documented here, for the
/// time being.
#[qapi(name = "SysEmuTarget")]
#[qapi(since = "3.0")]
pub enum SysEmuTarget {
    #[qapi(name = "aarch64")]
    Aarch64,
    #[qapi(name = "alpha")]
    Alpha,
    #[qapi(name = "arm")]
    Arm,
    /// since 5.1
    #[qapi(name = "avr")]
    Avr,
    #[qapi(name = "cris")]
    Cris,
    #[qapi(name = "hppa")]
    Hppa,
    #[qapi(name = "i386")]
    I386,
    /// since 7.1
    #[qapi(name = "loongarch64")]
    Loongarch64,
    #[qapi(name = "m68k")]
    M68k,
    #[qapi(name = "microblaze")]
    Microblaze,
    #[qapi(name = "microblazeel")]
    Microblazeel,
    #[qapi(name = "mips")]
    Mips,
    #[qapi(name = "mips64")]
    Mips64,
    #[qapi(name = "mips64el")]
    Mips64el,
    #[qapi(name = "mipsel")]
    Mipsel,
    #[qapi(name = "or1k")]
    Or1k,
    #[qapi(name = "ppc")]
    Ppc,
    #[qapi(name = "ppc64")]
    Ppc64,
    #[qapi(name = "riscv32")]
    Riscv32,
    #[qapi(name = "riscv64")]
    Riscv64,
    /// since 5.0
    #[qapi(name = "rx")]
    Rx,
    #[qapi(name = "s390x")]
    S390x,
    #[qapi(name = "sh4")]
    Sh4,
    #[qapi(name = "sh4eb")]
    Sh4eb,
    #[qapi(name = "sparc")]
    Sparc,
    #[qapi(name = "sparc64")]
    Sparc64,
    #[qapi(name = "tricore")]
    Tricore,
    #[qapi(name = "x86_64")]
    X8664,
    #[qapi(name = "xtensa")]
    Xtensa,
    #[qapi(name = "xtensaeb")]
    Xtensaeb,
}
/// An enumeration of cpu states that can be assumed by a virtual S390
/// CPU
#[qapi(name = "CpuS390State")]
#[qapi(since = "2.12")]
pub enum CpuS390State {
    #[qapi(name = "uninitialized")]
    Uninitialized,
    #[qapi(name = "stopped")]
    Stopped,
    #[qapi(name = "check-stop")]
    CheckStop,
    #[qapi(name = "operating")]
    Operating,
    #[qapi(name = "load")]
    Load,
}
/// Additional information about a virtual S390 CPU
#[qapi(name = "CpuInfoS390")]
#[qapi(since = "2.12")]
pub struct CpuInfoS390 {
    /// the virtual CPU's state
    #[qapi(name = "cpu-state")]
    pub cpu_state: CpuS390State,
    /// the virtual CPU's dedication (since 8.2)
    #[qapi(name = "dedicated")]
    pub dedicated: Option<bool>,
    /// the virtual CPU's entitlement (since 8.2)
    #[qapi(name = "entitlement")]
    pub entitlement: Option<CpuS390Entitlement>,
}
pub enum CpuInfoFastBranch {
    #[qapi(name = "s390x")]
    S390x(CpuInfoS390),
}
/// Information about a virtual CPU
#[qapi(name = "CpuInfoFast")]
#[qapi(since = "2.12")]
pub struct CpuInfoFast {
    /// index of the virtual CPU
    #[qapi(name = "cpu-index")]
    pub cpu_index: i64,
    /// path to the CPU object in the QOM tree
    #[qapi(name = "qom-path")]
    pub qom_path: String,
    /// ID of the underlying host thread
    #[qapi(name = "thread-id")]
    pub thread_id: i64,
    /// properties associated with a virtual CPU, e.g. the socket id
    #[qapi(name = "props")]
    pub props: Option<CpuInstanceProperties>,
    /// the QEMU system emulation target, which determines which
    /// additional fields will be listed (since 3.0)
    #[qapi(name = "target")]
    #[qapi(discriminator)]
    pub target: SysEmuTarget,
    #[qapi(union)]
    pub u: Option<CpuInfoFastBranch>,
}
/// Returns information about all virtual CPUs.
#[qapi(name = "query-cpus-fast")]
#[qapi(since = "2.12")]
#[qapi(returns = "Vec<CpuInfoFast>")]
pub struct QueryCpusFast {}
/// Property default values specific to a machine type, for use by
/// scripts/compare-machine-types.
#[qapi(name = "CompatProperty")]
#[qapi(since = "9.1")]
pub struct CompatProperty {
    /// name of the QOM type to which the default applies
    #[qapi(name = "qom-type")]
    pub qom_type: String,
    /// name of its property to which the default applies
    #[qapi(name = "property")]
    pub property: String,
    /// the default value (machine-specific default can overwrite
    /// the "default" default, to avoid this use -machine none)
    #[qapi(name = "value")]
    pub value: String,
}
/// Information describing a machine.
#[qapi(name = "MachineInfo")]
#[qapi(since = "1.2")]
pub struct MachineInfo {
    /// the name of the machine
    #[qapi(name = "name")]
    pub name: String,
    /// an alias for the machine name
    #[qapi(name = "alias")]
    pub alias: Option<String>,
    /// whether the machine is default
    #[qapi(name = "is-default")]
    pub is_default: Option<bool>,
    /// maximum number of CPUs supported by the machine type
    /// (since 1.5)
    #[qapi(name = "cpu-max")]
    pub cpu_max: i64,
    /// cpu hotplug via -device is supported (since 2.7)
    #[qapi(name = "hotpluggable-cpus")]
    pub hotpluggable_cpus: bool,
    /// true if '-numa node,mem' option is supported by
    /// the machine type and false otherwise (since 4.1)
    #[qapi(name = "numa-mem-supported")]
    pub numa_mem_supported: bool,
    /// if true, the machine type is deprecated and may be
    /// removed in future versions of QEMU according to the QEMU
    /// deprecation policy (since 4.1)
    #[qapi(name = "deprecated")]
    pub deprecated: bool,
    /// default CPU model typename if none is requested
    /// via the -cpu argument.  (since 4.2)
    #[qapi(name = "default-cpu-type")]
    pub default_cpu_type: Option<String>,
    /// the default ID of initial RAM memory backend (since
    /// 5.2)
    #[qapi(name = "default-ram-id")]
    pub default_ram_id: Option<String>,
    /// machine type supports ACPI (since 8.0)
    #[qapi(name = "acpi")]
    pub acpi: bool,
    /// The machine type's compatibility properties.  Only
    /// present when query-machines argument @compat-props is true.
    /// (since 9.1)
    #[qapi(name = "compat-props")]
    #[qapi(feature = "unstable")]
    pub compat_props: Option<Vec<CompatProperty>>,
}
/// Return a list of supported machines
#[qapi(name = "query-machines")]
#[qapi(since = "1.2")]
#[qapi(returns = "Vec<MachineInfo>")]
pub struct QueryMachines {
    /// if true, also return compatibility properties.
    /// (default: false) (since 9.1)
    #[qapi(name = "compat-props")]
    #[qapi(feature = "unstable")]
    pub compat_props: Option<bool>,
}
/// Information describing the running machine parameters.
#[qapi(name = "CurrentMachineParams")]
#[qapi(since = "4.0")]
pub struct CurrentMachineParams {
    /// true if the machine supports wake up from
    /// suspend
    #[qapi(name = "wakeup-suspend-support")]
    pub wakeup_suspend_support: bool,
}
/// Return information on the current virtual machine.
#[qapi(name = "query-current-machine")]
#[qapi(since = "4.0")]
#[qapi(returns = "CurrentMachineParams")]
pub struct QueryCurrentMachine {}
/// Information describing the QEMU target.
#[qapi(name = "TargetInfo")]
#[qapi(since = "1.2")]
pub struct TargetInfo {
    /// the target architecture
    #[qapi(name = "arch")]
    pub arch: SysEmuTarget,
}
/// Return information about the target for this QEMU
#[qapi(name = "query-target")]
#[qapi(since = "1.2")]
#[qapi(returns = "TargetInfo")]
pub struct QueryTarget {}
/// Guest UUID information (Universally Unique Identifier).
#[qapi(name = "UuidInfo")]
#[qapi(since = "0.14")]
pub struct UuidInfo {
    /// the UUID of the guest
    #[qapi(name = "UUID")]
    pub uuid: String,
}
/// Query the guest UUID information.
#[qapi(name = "query-uuid")]
#[qapi(since = "0.14")]
#[qapi(returns = "UuidInfo")]
#[qapi(allow_preconfig)]
pub struct QueryUuid {}
/// GUID information.
#[qapi(name = "GuidInfo")]
#[qapi(since = "2.9")]
pub struct GuidInfo {
    /// the globally unique identifier
    #[qapi(name = "guid")]
    pub guid: String,
}
/// Show Virtual Machine Generation ID
#[qapi(name = "query-vm-generation-id")]
#[qapi(since = "2.9")]
#[qapi(returns = "GuidInfo")]
pub struct QueryVmGenerationId {}
/// Performs a hard reset of a guest.
#[qapi(name = "system_reset")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct SystemReset {}
/// Requests that a guest perform a powerdown operation.
#[qapi(name = "system_powerdown")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct SystemPowerdown {}
/// Wake up guest from suspend.  If the guest has wake-up from suspend
/// support enabled (wakeup-suspend-support flag from
/// query-current-machine), wake-up guest from suspend if the guest is
/// in SUSPENDED state.  Return an error otherwise.
#[qapi(name = "system_wakeup")]
#[qapi(since = "1.1")]
#[qapi(returns = "()")]
pub struct SystemWakeup {}
/// Policy for handling lost ticks in timer devices.  Ticks end up
/// getting lost when, for example, the guest is paused.
#[qapi(name = "LostTickPolicy")]
#[qapi(since = "2.0")]
pub enum LostTickPolicy {
    /// throw away the missed ticks and continue with future
    /// injection normally.  The guest OS will see the timer jump ahead
    /// by a potentially quite significant amount all at once, as if the
    /// intervening chunk of time had simply not existed; needless to
    /// say, such a sudden jump can easily confuse a guest OS which is
    /// not specifically prepared to deal with it.  Assuming the guest
    /// OS can deal correctly with the time jump, the time in the guest
    /// and in the host should now match.
    #[qapi(name = "discard")]
    Discard,
    /// continue to deliver ticks at the normal rate.  The guest OS
    /// will not notice anything is amiss, as from its point of view
    /// time will have continued to flow normally.  The time in the
    /// guest should now be behind the time in the host by exactly the
    /// amount of time during which ticks have been missed.
    #[qapi(name = "delay")]
    Delay,
    /// deliver ticks at a higher rate to catch up with the missed
    /// ticks.  The guest OS will not notice anything is amiss, as from
    /// its point of view time will have continued to flow normally.
    /// Once the timer has managed to catch up with all the missing
    /// ticks, the time in the guest and in the host should match.
    #[qapi(name = "slew")]
    Slew,
}
/// Injects a Non-Maskable Interrupt into the default CPU (x86/s390) or
/// all CPUs (ppc64).  The command fails when the guest doesn't support
/// injecting.
#[qapi(name = "inject-nmi")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct InjectNmi {}
/// Information about support for KVM acceleration
#[qapi(name = "KvmInfo")]
#[qapi(since = "0.14")]
pub struct KvmInfo {
    /// true if KVM acceleration is active
    #[qapi(name = "enabled")]
    pub enabled: bool,
    /// true if KVM acceleration is built into this executable
    #[qapi(name = "present")]
    pub present: bool,
}
/// Returns information about KVM acceleration
#[qapi(name = "query-kvm")]
#[qapi(since = "0.14")]
#[qapi(returns = "KvmInfo")]
pub struct QueryKvm {}
#[qapi(name = "NumaOptionsType")]
#[qapi(since = "2.1")]
pub enum NumaOptionsType {
    /// NUMA nodes configuration
    #[qapi(name = "node")]
    Node,
    /// NUMA distance configuration (since 2.10)
    #[qapi(name = "dist")]
    Dist,
    /// property based CPU(s) to node mapping (Since: 2.10)
    #[qapi(name = "cpu")]
    Cpu,
    /// memory latency and bandwidth information (Since: 5.0)
    #[qapi(name = "hmat-lb")]
    HmatLb,
    /// memory side cache information (Since: 5.0)
    #[qapi(name = "hmat-cache")]
    HmatCache,
}
pub enum NumaOptionsBranch {
    #[qapi(name = "node")]
    Node(NumaNodeOptions),
    #[qapi(name = "dist")]
    Dist(NumaDistOptions),
    #[qapi(name = "cpu")]
    Cpu(NumaCpuOptions),
    #[qapi(name = "hmat-lb")]
    HmatLb(NumaHmatLbOptions),
    #[qapi(name = "hmat-cache")]
    HmatCache(NumaHmatCacheOptions),
}
/// A discriminated record of NUMA options.  (for OptsVisitor)
#[qapi(name = "NumaOptions")]
#[qapi(since = "2.1")]
pub struct NumaOptions {
    /// NUMA option type
    #[qapi(name = "type")]
    #[qapi(discriminator)]
    pub r#type: NumaOptionsType,
    #[qapi(union)]
    pub u: Option<NumaOptionsBranch>,
}
/// Create a guest NUMA node.  (for OptsVisitor)
#[qapi(name = "NumaNodeOptions")]
#[qapi(since = "2.1")]
pub struct NumaNodeOptions {
    /// NUMA node ID (increase by 1 from 0 if omitted)
    #[qapi(name = "nodeid")]
    pub nodeid: Option<u16>,
    /// VCPUs belonging to this node (assign VCPUS round-robin if
    /// omitted)
    #[qapi(name = "cpus")]
    pub cpus: Option<Vec<u16>>,
    /// memory size of this node; mutually exclusive with @memdev.
    /// Equally divide total memory among nodes if both @mem and @memdev
    /// are omitted.
    #[qapi(name = "mem")]
    pub mem: Option<u64>,
    /// memory backend object.  If specified for one node, it must
    /// be specified for all nodes.
    #[qapi(name = "memdev")]
    pub memdev: Option<String>,
    /// defined in ACPI 6.3 Chapter 5.2.27.3 Table 5-145, points
    /// to the nodeid which has the memory controller responsible for
    /// this NUMA node.  This field provides additional information as
    /// to the initiator node that is closest (as in directly attached)
    /// to this node, and therefore has the best performance (since 5.0)
    #[qapi(name = "initiator")]
    pub initiator: Option<u16>,
}
/// Set the distance between 2 NUMA nodes.
#[qapi(name = "NumaDistOptions")]
#[qapi(since = "2.10")]
pub struct NumaDistOptions {
    /// source NUMA node.
    #[qapi(name = "src")]
    pub src: u16,
    /// destination NUMA node.
    #[qapi(name = "dst")]
    pub dst: u16,
    /// NUMA distance from source node to destination node.  When a
    /// node is unreachable from another node, set the distance between
    /// them to 255.
    #[qapi(name = "val")]
    pub val: u8,
}
/// Create a CXL Fixed Memory Window
#[qapi(name = "CXLFixedMemoryWindowOptions")]
#[qapi(since = "7.1")]
pub struct CxlFixedMemoryWindowOptions {
    /// Size of the Fixed Memory Window in bytes.  Must be a multiple
    /// of 256MiB.
    #[qapi(name = "size")]
    pub size: u64,
    /// Number of contiguous bytes for which
    /// accesses will go to a given interleave target.  Accepted values
    /// [256, 512, 1k, 2k, 4k, 8k, 16k]
    #[qapi(name = "interleave-granularity")]
    pub interleave_granularity: Option<u64>,
    /// Target root bridge IDs from -device ...,id=<ID> for each
    /// root bridge.
    #[qapi(name = "targets")]
    pub targets: Vec<String>,
}
/// List of CXL Fixed Memory Windows.
#[qapi(name = "CXLFMWProperties")]
#[qapi(since = "7.1")]
pub struct CxlfmwProperties {
    /// List of CXLFixedMemoryWindowOptions
    #[qapi(name = "cxl-fmw")]
    pub cxl_fmw: Vec<CxlFixedMemoryWindowOptions>,
}
/// A X86 32-bit register
#[qapi(name = "X86CPURegister32")]
#[qapi(since = "1.5")]
pub enum X86cpuRegister32 {
    #[qapi(name = "EAX")]
    Eax,
    #[qapi(name = "EBX")]
    Ebx,
    #[qapi(name = "ECX")]
    Ecx,
    #[qapi(name = "EDX")]
    Edx,
    #[qapi(name = "ESP")]
    Esp,
    #[qapi(name = "EBP")]
    Ebp,
    #[qapi(name = "ESI")]
    Esi,
    #[qapi(name = "EDI")]
    Edi,
}
/// Information about a X86 CPU feature word
#[qapi(name = "X86CPUFeatureWordInfo")]
#[qapi(since = "1.5")]
pub struct X86cpuFeatureWordInfo {
    /// Input EAX value for CPUID instruction for that
    /// feature word
    #[qapi(name = "cpuid-input-eax")]
    pub cpuid_input_eax: i64,
    /// Input ECX value for CPUID instruction for that
    /// feature word
    #[qapi(name = "cpuid-input-ecx")]
    pub cpuid_input_ecx: Option<i64>,
    /// Output register containing the feature bits
    #[qapi(name = "cpuid-register")]
    pub cpuid_register: X86cpuRegister32,
    /// value of output register, containing the feature bits
    #[qapi(name = "features")]
    pub features: i64,
}
/// Not used by QMP; hack to let us use X86CPUFeatureWordInfoList
/// internally
#[qapi(name = "DummyForceArrays")]
#[qapi(since = "2.5")]
pub struct DummyForceArrays {
    #[qapi(name = "unused")]
    pub unused: Vec<X86cpuFeatureWordInfo>,
}
/// Option "-numa cpu" overrides default cpu to node mapping.  It
/// accepts the same set of cpu properties as returned by
/// query-hotpluggable-cpus[].props, where node-id could be used to
/// override default node mapping.
#[qapi(name = "NumaCpuOptions")]
#[qapi(since = "2.10")]
pub struct NumaCpuOptions {
    /// NUMA node ID the CPU belongs to
    #[qapi(name = "node-id")]
    pub node_id: Option<i64>,
    /// drawer number within CPU topology the CPU belongs to
    /// (since 8.2)
    #[qapi(name = "drawer-id")]
    pub drawer_id: Option<i64>,
    /// book number within parent container the CPU belongs to
    /// (since 8.2)
    #[qapi(name = "book-id")]
    pub book_id: Option<i64>,
    /// socket number within parent container the CPU belongs to
    #[qapi(name = "socket-id")]
    pub socket_id: Option<i64>,
    /// die number within the parent container the CPU belongs to
    /// (since 4.1)
    #[qapi(name = "die-id")]
    pub die_id: Option<i64>,
    /// cluster number within the parent container the CPU
    /// belongs to (since 7.1)
    #[qapi(name = "cluster-id")]
    pub cluster_id: Option<i64>,
    /// module number within the parent container the CPU
    /// belongs to (since 9.1)
    #[qapi(name = "module-id")]
    pub module_id: Option<i64>,
    /// core number within the parent container the CPU belongs to
    #[qapi(name = "core-id")]
    pub core_id: Option<i64>,
    /// thread number within the core the CPU  belongs to
    #[qapi(name = "thread-id")]
    pub thread_id: Option<i64>,
}
/// The memory hierarchy in the System Locality Latency and Bandwidth
/// Information Structure of HMAT (Heterogeneous Memory Attribute Table)
///
/// For more information about @HmatLBMemoryHierarchy, see chapter
/// 5.2.27.4: Table 5-146: Field "Flags" of ACPI 6.3 spec.
#[qapi(name = "HmatLBMemoryHierarchy")]
#[qapi(since = "5.0")]
pub enum HmatLbMemoryHierarchy {
    /// the structure represents the memory performance
    #[qapi(name = "memory")]
    Memory,
    /// first level of memory side cache
    #[qapi(name = "first-level")]
    FirstLevel,
    /// second level of memory side cache
    #[qapi(name = "second-level")]
    SecondLevel,
    /// third level of memory side cache
    #[qapi(name = "third-level")]
    ThirdLevel,
}
/// Data type in the System Locality Latency and Bandwidth Information
/// Structure of HMAT (Heterogeneous Memory Attribute Table)
///
/// For more information about @HmatLBDataType, see chapter 5.2.27.4:
/// Table 5-146:  Field "Data Type" of ACPI 6.3 spec.
#[qapi(name = "HmatLBDataType")]
#[qapi(since = "5.0")]
pub enum HmatLbDataType {
    /// access latency (nanoseconds)
    #[qapi(name = "access-latency")]
    AccessLatency,
    /// read latency (nanoseconds)
    #[qapi(name = "read-latency")]
    ReadLatency,
    /// write latency (nanoseconds)
    #[qapi(name = "write-latency")]
    WriteLatency,
    /// access bandwidth (Bytes per second)
    #[qapi(name = "access-bandwidth")]
    AccessBandwidth,
    /// read bandwidth (Bytes per second)
    #[qapi(name = "read-bandwidth")]
    ReadBandwidth,
    /// write bandwidth (Bytes per second)
    #[qapi(name = "write-bandwidth")]
    WriteBandwidth,
}
/// Set the system locality latency and bandwidth information between
/// Initiator and Target proximity Domains.
///
/// For more information about @NumaHmatLBOptions, see chapter 5.2.27.4:
/// Table 5-146 of ACPI 6.3 spec.
#[qapi(name = "NumaHmatLBOptions")]
#[qapi(since = "5.0")]
pub struct NumaHmatLbOptions {
    /// the Initiator Proximity Domain.
    #[qapi(name = "initiator")]
    pub initiator: u16,
    /// the Target Proximity Domain.
    #[qapi(name = "target")]
    pub target: u16,
    /// the Memory Hierarchy.  Indicates the performance of
    /// memory or side cache.
    #[qapi(name = "hierarchy")]
    pub hierarchy: HmatLbMemoryHierarchy,
    /// presents the type of data, access/read/write latency or
    /// hit latency.
    #[qapi(name = "data-type")]
    pub data_type: HmatLbDataType,
    /// the value of latency from @initiator to @target proximity
    /// domain, the latency unit is "ns(nanosecond)".
    #[qapi(name = "latency")]
    pub latency: Option<u64>,
    /// the value of bandwidth between @initiator and @target
    /// proximity domain, the bandwidth unit is "Bytes per second".
    #[qapi(name = "bandwidth")]
    pub bandwidth: Option<u64>,
}
/// Cache associativity in the Memory Side Cache Information Structure
/// of HMAT
///
/// For more information of @HmatCacheAssociativity, see chapter
/// 5.2.27.5: Table 5-147 of ACPI 6.3 spec.
#[qapi(name = "HmatCacheAssociativity")]
#[qapi(since = "5.0")]
pub enum HmatCacheAssociativity {
    /// None (no memory side cache in this proximity domain, or cache
    /// associativity unknown)
    #[qapi(name = "none")]
    None,
    /// Direct Mapped
    #[qapi(name = "direct")]
    Direct,
    /// Complex Cache Indexing (implementation specific)
    #[qapi(name = "complex")]
    Complex,
}
/// Cache write policy in the Memory Side Cache Information Structure of
/// HMAT
///
/// For more information of @HmatCacheWritePolicy, see chapter 5.2.27.5:
/// Table 5-147: Field "Cache Attributes" of ACPI 6.3 spec.
#[qapi(name = "HmatCacheWritePolicy")]
#[qapi(since = "5.0")]
pub enum HmatCacheWritePolicy {
    /// None (no memory side cache in this proximity domain, or cache
    /// write policy unknown)
    #[qapi(name = "none")]
    None,
    /// Write Back (WB)
    #[qapi(name = "write-back")]
    WriteBack,
    /// Write Through (WT)
    #[qapi(name = "write-through")]
    WriteThrough,
}
/// Set the memory side cache information for a given memory domain.
///
/// For more information of @NumaHmatCacheOptions, see chapter 5.2.27.5:
/// Table 5-147: Field "Cache Attributes" of ACPI 6.3 spec.
#[qapi(name = "NumaHmatCacheOptions")]
#[qapi(since = "5.0")]
pub struct NumaHmatCacheOptions {
    /// the memory proximity domain to which the memory belongs.
    #[qapi(name = "node-id")]
    pub node_id: u32,
    /// the size of memory side cache in bytes.
    #[qapi(name = "size")]
    pub size: u64,
    /// the cache level described in this structure.
    #[qapi(name = "level")]
    pub level: u8,
    /// the cache associativity,
    /// none/direct-mapped/complex(complex cache indexing).
    #[qapi(name = "associativity")]
    pub associativity: HmatCacheAssociativity,
    /// the write policy, none/write-back/write-through.
    #[qapi(name = "policy")]
    pub policy: HmatCacheWritePolicy,
    /// the cache Line size in bytes.
    #[qapi(name = "line")]
    pub line: u16,
}
/// Save a portion of guest memory to a file.
#[qapi(name = "memsave")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct Memsave {
    /// the virtual address of the guest to start from
    #[qapi(name = "val")]
    pub val: u64,
    /// the size of memory region to save
    #[qapi(name = "size")]
    pub size: u64,
    /// the file to save the memory to as binary data
    #[qapi(name = "filename")]
    pub filename: String,
    /// the index of the virtual CPU to use for translating the
    /// virtual address (defaults to CPU 0)
    #[qapi(name = "cpu-index")]
    pub cpu_index: Option<i64>,
}
/// Save a portion of guest physical memory to a file.
#[qapi(name = "pmemsave")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct Pmemsave {
    /// the physical address of the guest to start from
    #[qapi(name = "val")]
    pub val: u64,
    /// the size of memory region to save
    #[qapi(name = "size")]
    pub size: u64,
    /// the file to save the memory to as binary data
    #[qapi(name = "filename")]
    pub filename: String,
}
/// Information about memory backend
#[qapi(name = "Memdev")]
#[qapi(since = "2.1")]
pub struct Memdev {
    /// backend's ID if backend has 'id' property (since 2.9)
    #[qapi(name = "id")]
    pub id: Option<String>,
    /// memory backend size
    #[qapi(name = "size")]
    pub size: u64,
    /// whether memory merge support is enabled
    #[qapi(name = "merge")]
    pub merge: bool,
    /// whether memory backend's memory is included in a core dump
    #[qapi(name = "dump")]
    pub dump: bool,
    /// whether memory was preallocated
    #[qapi(name = "prealloc")]
    pub prealloc: bool,
    /// whether memory is private to QEMU or shared (since 6.1)
    #[qapi(name = "share")]
    pub share: bool,
    /// whether swap space (or huge pages) was reserved if
    /// applicable.  This corresponds to the user configuration and not
    /// the actual behavior implemented in the OS to perform the
    /// reservation.  For example, Linux will never reserve swap space
    /// for shared file mappings.  (since 6.1)
    #[qapi(name = "reserve")]
    pub reserve: Option<bool>,
    /// host nodes for its memory policy
    #[qapi(name = "host-nodes")]
    pub host_nodes: Vec<u16>,
    /// memory policy of memory backend
    #[qapi(name = "policy")]
    pub policy: HostMemPolicy,
}
/// Returns information for all memory backends.
#[qapi(name = "query-memdev")]
#[qapi(since = "2.1")]
#[qapi(returns = "Vec<Memdev>")]
#[qapi(allow_preconfig)]
pub struct QueryMemdev {}
/// Properties identifying a CPU.
///
/// Which members are optional and which mandatory depends on the
/// architecture and board.
///
/// For s390x see :ref:`cpu-topology-s390x`.
///
/// The ids other than the node-id specify the position of the CPU
/// within the CPU topology (as defined by the machine property "smp",
/// thus see also type @SMPConfiguration)
#[qapi(name = "CpuInstanceProperties")]
#[qapi(since = "2.7")]
pub struct CpuInstanceProperties {
    /// NUMA node ID the CPU belongs to
    #[qapi(name = "node-id")]
    pub node_id: Option<i64>,
    /// drawer number within CPU topology the CPU belongs to
    /// (since 8.2)
    #[qapi(name = "drawer-id")]
    pub drawer_id: Option<i64>,
    /// book number within parent container the CPU belongs to
    /// (since 8.2)
    #[qapi(name = "book-id")]
    pub book_id: Option<i64>,
    /// socket number within parent container the CPU belongs to
    #[qapi(name = "socket-id")]
    pub socket_id: Option<i64>,
    /// die number within the parent container the CPU belongs to
    /// (since 4.1)
    #[qapi(name = "die-id")]
    pub die_id: Option<i64>,
    /// cluster number within the parent container the CPU
    /// belongs to (since 7.1)
    #[qapi(name = "cluster-id")]
    pub cluster_id: Option<i64>,
    /// module number within the parent container the CPU
    /// belongs to (since 9.1)
    #[qapi(name = "module-id")]
    pub module_id: Option<i64>,
    /// core number within the parent container the CPU belongs to
    #[qapi(name = "core-id")]
    pub core_id: Option<i64>,
    /// thread number within the core the CPU  belongs to
    #[qapi(name = "thread-id")]
    pub thread_id: Option<i64>,
}
#[qapi(name = "HotpluggableCPU")]
#[qapi(since = "2.7")]
pub struct HotpluggableCpu {
    /// CPU object type for usage with device_add command
    #[qapi(name = "type")]
    pub r#type: String,
    /// number of logical VCPU threads @HotpluggableCPU
    /// provides
    #[qapi(name = "vcpus-count")]
    pub vcpus_count: i64,
    /// list of properties to pass for hotplugging a CPU with
    /// device_add
    #[qapi(name = "props")]
    pub props: CpuInstanceProperties,
    /// link to existing CPU object if CPU is present or omitted
    /// if CPU is not present.
    #[qapi(name = "qom-path")]
    pub qom_path: Option<String>,
}
/// TODO: Better documentation; currently there is none.
#[qapi(name = "query-hotpluggable-cpus")]
#[qapi(since = "2.7")]
#[qapi(returns = "Vec<HotpluggableCPU>")]
#[qapi(allow_preconfig)]
pub struct QueryHotpluggableCpus {}
/// Runtime equivalent of '-numa' CLI option, available at preconfigure
/// stage to configure numa mapping before initializing machine.
#[qapi(name = "set-numa-node")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct SetNumaNode {
    #[qapi(flatten)]
    pub data: NumaOptions,
}
/// Request the balloon driver to change its balloon size.
#[qapi(name = "balloon")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct Balloon {
    /// the target logical size of the VM in bytes.  We can deduce
    /// the size of the balloon using this formula:
    ///
    /// logical_vm_size = vm_ram_size - balloon_size
    ///
    /// From it we have: balloon_size = vm_ram_size - @value
    #[qapi(name = "value")]
    pub value: i64,
}
/// Information about the guest balloon device.
#[qapi(name = "BalloonInfo")]
#[qapi(since = "0.14")]
pub struct BalloonInfo {
    /// the logical size of the VM in bytes Formula used:
    /// logical_vm_size = vm_ram_size - balloon_size
    #[qapi(name = "actual")]
    pub actual: i64,
}
/// Return information about the balloon device.
#[qapi(name = "query-balloon")]
#[qapi(since = "0.14")]
#[qapi(returns = "BalloonInfo")]
pub struct QueryBalloon {}
/// Emitted when the guest changes the actual BALLOON level.  This value
/// is equivalent to the @actual field return by the 'query-balloon'
/// command
#[qapi(name = "BALLOON_CHANGE")]
#[qapi(since = "1.2")]
pub struct BalloonChange {
    /// the logical size of the VM in bytes Formula used:
    /// logical_vm_size = vm_ram_size - balloon_size
    #[qapi(name = "actual")]
    pub actual: i64,
}
/// hv-balloon guest-provided memory status information.
#[qapi(name = "HvBalloonInfo")]
#[qapi(since = "8.2")]
pub struct HvBalloonInfo {
    /// the amount of memory in use inside the guest plus the
    /// amount of the memory unusable inside the guest (ballooned out,
    /// offline, etc.)
    #[qapi(name = "committed")]
    pub committed: u64,
    /// the amount of the memory inside the guest available for
    /// new allocations ("free")
    #[qapi(name = "available")]
    pub available: u64,
}
/// Returns the hv-balloon driver data contained in the last received
/// "STATUS" message from the guest.
#[qapi(name = "query-hv-balloon-status-report")]
#[qapi(since = "8.2")]
#[qapi(returns = "HvBalloonInfo")]
pub struct QueryHvBalloonStatusReport {}
/// Emitted when the hv-balloon driver receives a "STATUS" message from
/// the guest.
#[qapi(name = "HV_BALLOON_STATUS_REPORT")]
#[qapi(since = "8.2")]
pub struct HvBalloonStatusReport {
    /// the amount of memory in use inside the guest plus the
    /// amount of the memory unusable inside the guest (ballooned out,
    /// offline, etc.)
    #[qapi(name = "committed")]
    pub committed: u64,
    /// the amount of the memory inside the guest available for
    /// new allocations ("free")
    #[qapi(name = "available")]
    pub available: u64,
}
/// Actual memory information in bytes.
#[qapi(name = "MemoryInfo")]
#[qapi(since = "2.11")]
pub struct MemoryInfo {
    /// size of "base" memory specified with command line
    /// option -m.
    #[qapi(name = "base-memory")]
    pub base_memory: u64,
    /// size of memory that can be hot-unplugged.  This
    /// field is omitted if target doesn't support memory hotplug (i.e.
    /// CONFIG_MEM_DEVICE not defined at build time).
    #[qapi(name = "plugged-memory")]
    pub plugged_memory: Option<u64>,
}
/// Return the amount of initially allocated and present hotpluggable
/// (if enabled) memory in bytes.
#[qapi(name = "query-memory-size-summary")]
#[qapi(since = "2.11")]
#[qapi(returns = "MemoryInfo")]
pub struct QueryMemorySizeSummary {}
/// PCDIMMDevice state information
#[qapi(name = "PCDIMMDeviceInfo")]
#[qapi(since = "2.1")]
pub struct PcdimmDeviceInfo {
    /// device's ID
    #[qapi(name = "id")]
    pub id: Option<String>,
    /// physical address, where device is mapped
    #[qapi(name = "addr")]
    pub addr: i64,
    /// size of memory that the device provides
    #[qapi(name = "size")]
    pub size: i64,
    /// slot number at which device is plugged in
    #[qapi(name = "slot")]
    pub slot: i64,
    /// NUMA node number where device is plugged in
    #[qapi(name = "node")]
    pub node: i64,
    /// memory backend linked with device
    #[qapi(name = "memdev")]
    pub memdev: String,
    /// true if device was hotplugged
    #[qapi(name = "hotplugged")]
    pub hotplugged: bool,
    /// true if device if could be added/removed while
    /// machine is running
    #[qapi(name = "hotpluggable")]
    pub hotpluggable: bool,
}
/// VirtioPMEM state information
#[qapi(name = "VirtioPMEMDeviceInfo")]
#[qapi(since = "4.1")]
pub struct VirtioPmemDeviceInfo {
    /// device's ID
    #[qapi(name = "id")]
    pub id: Option<String>,
    /// physical address in memory, where device is mapped
    #[qapi(name = "memaddr")]
    pub memaddr: u64,
    /// size of memory that the device provides
    #[qapi(name = "size")]
    pub size: u64,
    /// memory backend linked with device
    #[qapi(name = "memdev")]
    pub memdev: String,
}
/// VirtioMEMDevice state information
#[qapi(name = "VirtioMEMDeviceInfo")]
#[qapi(since = "5.1")]
pub struct VirtioMemDeviceInfo {
    /// device's ID
    #[qapi(name = "id")]
    pub id: Option<String>,
    /// physical address in memory, where device is mapped
    #[qapi(name = "memaddr")]
    pub memaddr: u64,
    /// the user requested size of the device
    #[qapi(name = "requested-size")]
    pub requested_size: u64,
    /// the (current) size of memory that the device provides
    #[qapi(name = "size")]
    pub size: u64,
    /// the maximum size of memory that the device can provide
    #[qapi(name = "max-size")]
    pub max_size: u64,
    /// the block size of memory that the device provides
    #[qapi(name = "block-size")]
    pub block_size: u64,
    /// NUMA node number where device is assigned to
    #[qapi(name = "node")]
    pub node: i64,
    /// memory backend linked with the region
    #[qapi(name = "memdev")]
    pub memdev: String,
}
/// Sgx EPC state information
#[qapi(name = "SgxEPCDeviceInfo")]
#[qapi(since = "6.2")]
pub struct SgxEpcDeviceInfo {
    /// device's ID
    #[qapi(name = "id")]
    pub id: Option<String>,
    /// physical address in memory, where device is mapped
    #[qapi(name = "memaddr")]
    pub memaddr: u64,
    /// size of memory that the device provides
    #[qapi(name = "size")]
    pub size: u64,
    /// the numa node (Since: 7.0)
    #[qapi(name = "node")]
    pub node: i64,
    /// memory backend linked with device
    #[qapi(name = "memdev")]
    pub memdev: String,
}
/// hv-balloon provided memory state information
#[qapi(name = "HvBalloonDeviceInfo")]
#[qapi(since = "8.2")]
pub struct HvBalloonDeviceInfo {
    /// device's ID
    #[qapi(name = "id")]
    pub id: Option<String>,
    /// physical address in memory, where device is mapped
    #[qapi(name = "memaddr")]
    pub memaddr: Option<u64>,
    /// the maximum size of memory that the device can provide
    #[qapi(name = "max-size")]
    pub max_size: u64,
    /// memory backend linked with device
    #[qapi(name = "memdev")]
    pub memdev: Option<String>,
}
#[qapi(name = "MemoryDeviceInfoKind")]
#[qapi(since = "2.1")]
pub enum MemoryDeviceInfoKind {
    #[qapi(name = "dimm")]
    Dimm,
    /// since 2.12
    #[qapi(name = "nvdimm")]
    Nvdimm,
    /// since 4.1
    #[qapi(name = "virtio-pmem")]
    VirtioPmem,
    /// since 5.1
    #[qapi(name = "virtio-mem")]
    VirtioMem,
    /// since 6.2.
    #[qapi(name = "sgx-epc")]
    SgxEpc,
    /// since 8.2.
    #[qapi(name = "hv-balloon")]
    HvBalloon,
}
#[qapi(name = "PCDIMMDeviceInfoWrapper")]
#[qapi(since = "2.1")]
pub struct PcdimmDeviceInfoWrapper {
    /// PCDIMMDevice state information
    #[qapi(name = "data")]
    pub data: PcdimmDeviceInfo,
}
#[qapi(name = "VirtioPMEMDeviceInfoWrapper")]
#[qapi(since = "2.1")]
pub struct VirtioPmemDeviceInfoWrapper {
    /// VirtioPMEM state information
    #[qapi(name = "data")]
    pub data: VirtioPmemDeviceInfo,
}
#[qapi(name = "VirtioMEMDeviceInfoWrapper")]
#[qapi(since = "2.1")]
pub struct VirtioMemDeviceInfoWrapper {
    /// VirtioMEMDevice state information
    #[qapi(name = "data")]
    pub data: VirtioMemDeviceInfo,
}
#[qapi(name = "SgxEPCDeviceInfoWrapper")]
#[qapi(since = "6.2")]
pub struct SgxEpcDeviceInfoWrapper {
    /// Sgx EPC state information
    #[qapi(name = "data")]
    pub data: SgxEpcDeviceInfo,
}
#[qapi(name = "HvBalloonDeviceInfoWrapper")]
#[qapi(since = "8.2")]
pub struct HvBalloonDeviceInfoWrapper {
    /// hv-balloon provided memory state information
    #[qapi(name = "data")]
    pub data: HvBalloonDeviceInfo,
}
pub enum MemoryDeviceInfoBranch {
    #[qapi(name = "dimm")]
    Dimm(PcdimmDeviceInfoWrapper),
    #[qapi(name = "nvdimm")]
    Nvdimm(PcdimmDeviceInfoWrapper),
    #[qapi(name = "virtio-pmem")]
    VirtioPmem(VirtioPmemDeviceInfoWrapper),
    #[qapi(name = "virtio-mem")]
    VirtioMem(VirtioMemDeviceInfoWrapper),
    #[qapi(name = "sgx-epc")]
    SgxEpc(SgxEpcDeviceInfoWrapper),
    #[qapi(name = "hv-balloon")]
    HvBalloon(HvBalloonDeviceInfoWrapper),
}
/// Union containing information about a memory device
#[qapi(name = "MemoryDeviceInfo")]
#[qapi(since = "2.1")]
pub struct MemoryDeviceInfo {
    /// memory device type
    #[qapi(name = "type")]
    #[qapi(discriminator)]
    pub r#type: MemoryDeviceInfoKind,
    #[qapi(union)]
    pub u: Option<MemoryDeviceInfoBranch>,
}
/// Sgx EPC cmdline information
#[qapi(name = "SgxEPC")]
#[qapi(since = "6.2")]
pub struct SgxEpc {
    /// memory backend linked with device
    #[qapi(name = "memdev")]
    pub memdev: String,
    /// the numa node (Since: 7.0)
    #[qapi(name = "node")]
    pub node: i64,
}
/// SGX properties of machine types.
#[qapi(name = "SgxEPCProperties")]
#[qapi(since = "6.2")]
pub struct SgxEpcProperties {
    /// list of ids of memory-backend-epc objects.
    #[qapi(name = "sgx-epc")]
    pub sgx_epc: Vec<SgxEpc>,
}
/// Lists available memory devices and their state
#[qapi(name = "query-memory-devices")]
#[qapi(since = "2.1")]
#[qapi(returns = "Vec<MemoryDeviceInfo>")]
pub struct QueryMemoryDevices {}
/// Emitted when the size of a memory device changes.  Only emitted for
/// memory devices that can actually change the size (e.g., virtio-mem
/// due to guest action).
#[qapi(name = "MEMORY_DEVICE_SIZE_CHANGE")]
#[qapi(since = "5.1")]
pub struct MemoryDeviceSizeChange {
    /// device's ID
    #[qapi(name = "id")]
    pub id: Option<String>,
    /// the new size of memory that the device provides
    #[qapi(name = "size")]
    pub size: u64,
    /// path to the device object in the QOM tree (since 6.2)
    #[qapi(name = "qom-path")]
    pub qom_path: String,
}
/// Schema for virtual machine boot configuration.
#[qapi(name = "BootConfiguration")]
#[qapi(since = "7.1")]
pub struct BootConfiguration {
    /// Boot order (a=floppy, c=hard disk, d=CD-ROM, n=network)
    #[qapi(name = "order")]
    pub order: Option<String>,
    /// Boot order to apply on first boot
    #[qapi(name = "once")]
    pub once: Option<String>,
    /// Whether to show a boot menu
    #[qapi(name = "menu")]
    pub menu: Option<bool>,
    /// The name of the file to be passed to the firmware as logo
    /// picture, if @menu is true.
    #[qapi(name = "splash")]
    pub splash: Option<String>,
    /// How long to show the logo picture, in milliseconds
    #[qapi(name = "splash-time")]
    pub splash_time: Option<i64>,
    /// Timeout before guest reboots after boot fails
    #[qapi(name = "reboot-timeout")]
    pub reboot_timeout: Option<i64>,
    /// Whether to attempt booting from devices not included in the
    /// boot order
    #[qapi(name = "strict")]
    pub strict: Option<bool>,
}
/// Schema for CPU topology configuration.  A missing value lets QEMU
/// figure out a suitable value based on the ones that are provided.
///
/// The members other than @cpus and @maxcpus define a topology of
/// containers.
///
/// The ordering from highest/coarsest to lowest/finest is: @drawers,
/// @books, @sockets, @dies, @clusters, @cores, @threads.
///
/// Different architectures support different subsets of topology
/// containers.
///
/// For example, s390x does not have clusters and dies, and the socket
/// is the parent container of cores.
#[qapi(name = "SMPConfiguration")]
#[qapi(since = "6.1")]
pub struct SmpConfiguration {
    /// number of virtual CPUs in the virtual machine
    #[qapi(name = "cpus")]
    pub cpus: Option<i64>,
    /// number of drawers in the CPU topology (since 8.2)
    #[qapi(name = "drawers")]
    pub drawers: Option<i64>,
    /// number of books in the CPU topology (since 8.2)
    #[qapi(name = "books")]
    pub books: Option<i64>,
    /// number of sockets per parent container
    #[qapi(name = "sockets")]
    pub sockets: Option<i64>,
    /// number of dies per parent container
    #[qapi(name = "dies")]
    pub dies: Option<i64>,
    /// number of clusters per parent container (since 7.0)
    #[qapi(name = "clusters")]
    pub clusters: Option<i64>,
    /// number of modules per parent container (since 9.1)
    #[qapi(name = "modules")]
    pub modules: Option<i64>,
    /// number of cores per parent container
    #[qapi(name = "cores")]
    pub cores: Option<i64>,
    /// number of threads per core
    #[qapi(name = "threads")]
    pub threads: Option<i64>,
    /// maximum number of hotpluggable virtual CPUs in the virtual
    /// machine
    #[qapi(name = "maxcpus")]
    pub maxcpus: Option<i64>,
}
/// Query interrupt statistics
#[qapi(name = "x-query-irq")]
#[qapi(feature = "unstable")]
#[qapi(since = "6.2")]
#[qapi(returns = "HumanReadableText")]
pub struct XQueryIrq {}
/// Query TCG compiler statistics
#[qapi(name = "x-query-jit")]
#[qapi(condition = "CONFIG_TCG")]
#[qapi(feature = "unstable")]
#[qapi(since = "6.2")]
#[qapi(returns = "HumanReadableText")]
pub struct XQueryJit {}
/// Query NUMA topology information
#[qapi(name = "x-query-numa")]
#[qapi(feature = "unstable")]
#[qapi(since = "6.2")]
#[qapi(returns = "HumanReadableText")]
pub struct XQueryNuma {}
/// Query TCG opcode counters
#[qapi(name = "x-query-opcount")]
#[qapi(condition = "CONFIG_TCG")]
#[qapi(feature = "unstable")]
#[qapi(since = "6.2")]
#[qapi(returns = "HumanReadableText")]
pub struct XQueryOpcount {}
/// Query system ramblock information
#[qapi(name = "x-query-ramblock")]
#[qapi(feature = "unstable")]
#[qapi(since = "6.2")]
#[qapi(returns = "HumanReadableText")]
pub struct XQueryRamblock {}
/// Query information on the registered ROMS
#[qapi(name = "x-query-roms")]
#[qapi(feature = "unstable")]
#[qapi(since = "6.2")]
#[qapi(returns = "HumanReadableText")]
pub struct XQueryRoms {}
/// Query information on the USB devices
#[qapi(name = "x-query-usb")]
#[qapi(feature = "unstable")]
#[qapi(since = "6.2")]
#[qapi(returns = "HumanReadableText")]
pub struct XQueryUsb {}
#[qapi(name = "SmbiosEntryPointType")]
#[qapi(since = "7.0")]
pub enum SmbiosEntryPointType {
    /// SMBIOS version 2.1 (32-bit) Entry Point
    #[qapi(name = "32")]
    _32,
    /// SMBIOS version 3.0 (64-bit) Entry Point
    #[qapi(name = "64")]
    _64,
    /// Either 2.x or 3.x SMBIOS version, 2.x if configuration can be
    /// described by it and 3.x otherwise (since: 9.0)
    #[qapi(name = "auto")]
    Auto,
}
/// Schema for memory size configuration.
#[qapi(name = "MemorySizeConfiguration")]
#[qapi(since = "7.1")]
pub struct MemorySizeConfiguration {
    /// memory size in bytes
    #[qapi(name = "size")]
    pub size: Option<u64>,
    /// maximum hotpluggable memory size in bytes
    #[qapi(name = "max-size")]
    pub max_size: Option<u64>,
    /// number of available memory slots for hotplug
    #[qapi(name = "slots")]
    pub slots: Option<u64>,
}
/// Save the FDT in dtb format.
#[qapi(name = "dumpdtb")]
#[qapi(condition = "CONFIG_FDT")]
#[qapi(since = "7.2")]
#[qapi(returns = "()")]
pub struct Dumpdtb {
    /// name of the dtb file to be created
    #[qapi(name = "filename")]
    pub filename: String,
}
/// Query information on interrupt controller devices
#[qapi(name = "x-query-interrupt-controllers")]
#[qapi(feature = "unstable")]
#[qapi(since = "9.1")]
#[qapi(returns = "HumanReadableText")]
pub struct XQueryInterruptControllers {}
// path end:	qapi/machine.json
// path begin:	qapi/machine-target.json
/// Virtual CPU model.
///
/// A CPU model consists of the name of a CPU definition, to which delta
/// changes are applied (e.g. features added/removed).  Most magic
/// values that an architecture might require should be hidden behind
/// the name.  However, if required, architectures can expose relevant
/// properties.
#[qapi(name = "CpuModelInfo")]
#[qapi(since = "2.8")]
pub struct CpuModelInfo {
    /// the name of the CPU definition the model is based on
    #[qapi(name = "name")]
    pub name: String,
    /// a dictionary of QOM properties to be applied
    #[qapi(name = "props")]
    pub props: Option<serde_json::Value>,
}
/// An enumeration of CPU model expansion types.
#[qapi(name = "CpuModelExpansionType")]
#[qapi(since = "2.8")]
pub enum CpuModelExpansionType {
    /// Expand to a static CPU model, a combination of a static
    /// base model name and property delta changes.  As the static base
    /// model will never change, the expanded CPU model will be the
    /// same, independent of QEMU version, machine type, machine
    /// options, and accelerator options.  Therefore, the resulting
    /// model can be used by tooling without having to specify a
    /// compatibility machine - e.g. when displaying the "host" model.
    /// The @static CPU models are migration-safe.
    #[qapi(name = "static")]
    Static,
    /// Expand all properties.  The produced model is not guaranteed
    /// to be migration-safe, but allows tooling to get an insight and
    /// work with model details.
    #[qapi(name = "full")]
    Full,
}
/// An enumeration of CPU model comparison results.  The result is
/// usually calculated using e.g. CPU features or CPU generations.
#[qapi(name = "CpuModelCompareResult")]
#[qapi(since = "2.8")]
pub enum CpuModelCompareResult {
    /// If model A is incompatible to model B, model A is not
    /// guaranteed to run where model B runs and the other way around.
    #[qapi(name = "incompatible")]
    Incompatible,
    /// If model A is identical to model B, model A is
    /// guaranteed to run where model B runs and the other way around.
    #[qapi(name = "identical")]
    Identical,
    /// If model A is a superset of model B, model B is
    /// guaranteed to run where model A runs.  There are no guarantees
    /// about the other way.
    #[qapi(name = "superset")]
    Superset,
    /// If model A is a subset of model B, model A is guaranteed to
    /// run where model B runs.  There are no guarantees about the other
    /// way.
    #[qapi(name = "subset")]
    Subset,
}
/// The result of a CPU model baseline.
#[qapi(name = "CpuModelBaselineInfo")]
#[qapi(condition = "TARGET_S390X")]
#[qapi(since = "2.8")]
pub struct CpuModelBaselineInfo {
    /// the baselined CpuModelInfo.
    #[qapi(name = "model")]
    pub model: CpuModelInfo,
}
/// The result of a CPU model comparison.
#[qapi(name = "CpuModelCompareInfo")]
#[qapi(condition = "TARGET_S390X")]
#[qapi(since = "2.8")]
pub struct CpuModelCompareInfo {
    /// The result of the compare operation.
    #[qapi(name = "result")]
    pub result: CpuModelCompareResult,
    /// List of properties that led to the
    /// comparison result not being identical.
    ///
    /// @responsible-properties is a list of QOM property names that led to
    /// both CPUs not being detected as identical.  For identical models,
    /// this list is empty.  If a QOM property is read-only, that means
    /// there's no known way to make the CPU models identical.  If the
    /// special property name "type" is included, the models are by
    /// definition not identical and cannot be made identical.
    #[qapi(name = "responsible-properties")]
    pub responsible_properties: Vec<String>,
}
/// Compares two CPU models, @modela and @modelb, returning how they
/// compare in a specific configuration.  The results indicates how
/// both models compare regarding runnability.  This result can be
/// used by tooling to make decisions if a certain CPU model will
/// run in a certain configuration or if a compatible CPU model has
/// to be created by baselining.
///
/// Usually, a CPU model is compared against the maximum possible CPU
/// model of a certain configuration (e.g. the "host" model for KVM).
/// If that CPU model is identical or a subset, it will run in that
/// configuration.
///
/// The result returned by this command may be affected by:
///
/// QEMU version: CPU models may look different depending on the QEMU
/// version.  (Except for CPU models reported as "static" in
/// query-cpu-definitions.)
/// machine-type: CPU model may look different depending on the
/// machine-type.  (Except for CPU models reported as "static" in
/// query-cpu-definitions.)
/// machine options (including accelerator): in some architectures,
/// CPU models may look different depending on machine and accelerator
/// options.  (Except for CPU models reported as "static" in
/// query-cpu-definitions.)
/// "-cpu" arguments and global properties: arguments to the -cpu
/// option and global properties may affect expansion of CPU models.
/// Using query-cpu-model-expansion while using these is not advised.
///
/// Some architectures may not support comparing CPU models.  s390x
/// supports comparing CPU models.
#[qapi(name = "query-cpu-model-comparison")]
#[qapi(condition = "TARGET_S390X")]
#[qapi(since = "2.8")]
#[qapi(returns = "CpuModelCompareInfo")]
pub struct QueryCpuModelComparison {
    /// description of the first CPU model to compare, referred to
    /// as "model A" in CpuModelCompareResult
    #[qapi(name = "modela")]
    pub modela: CpuModelInfo,
    /// description of the second CPU model to compare, referred to
    /// as "model B" in CpuModelCompareResult
    #[qapi(name = "modelb")]
    pub modelb: CpuModelInfo,
}
/// Baseline two CPU models, @modela and @modelb, creating a compatible
/// third model.  The created model will always be a static,
/// migration-safe CPU model (see "static" CPU model expansion for
/// details).
///
/// This interface can be used by tooling to create a compatible CPU
/// model out two CPU models.  The created CPU model will be identical
/// to or a subset of both CPU models when comparing them.  Therefore,
/// the created CPU model is guaranteed to run where the given CPU
/// models run.
///
/// The result returned by this command may be affected by:
///
/// QEMU version: CPU models may look different depending on the QEMU
/// version.  (Except for CPU models reported as "static" in
/// query-cpu-definitions.)
/// machine-type: CPU model may look different depending on the
/// machine-type.  (Except for CPU models reported as "static" in
/// query-cpu-definitions.)
/// machine options (including accelerator): in some architectures,
/// CPU models may look different depending on machine and accelerator
/// options.  (Except for CPU models reported as "static" in
/// query-cpu-definitions.)
/// "-cpu" arguments and global properties: arguments to the -cpu
/// option and global properties may affect expansion of CPU models.
/// Using query-cpu-model-expansion while using these is not advised.
///
/// Some architectures may not support baselining CPU models.  s390x
/// supports baselining CPU models.
#[qapi(name = "query-cpu-model-baseline")]
#[qapi(condition = "TARGET_S390X")]
#[qapi(since = "2.8")]
#[qapi(returns = "CpuModelBaselineInfo")]
pub struct QueryCpuModelBaseline {
    /// description of the first CPU model to baseline
    #[qapi(name = "modela")]
    pub modela: CpuModelInfo,
    /// description of the second CPU model to baseline
    #[qapi(name = "modelb")]
    pub modelb: CpuModelInfo,
}
/// The result of a cpu model expansion.
#[qapi(name = "CpuModelExpansionInfo")]
#[qapi(
    condition = "(TARGET_S390X || TARGET_I386 || TARGET_ARM || TARGET_LOONGARCH64 || TARGET_RISCV)"
)]
#[qapi(since = "2.8")]
pub struct CpuModelExpansionInfo {
    /// the expanded CpuModelInfo.
    #[qapi(name = "model")]
    pub model: CpuModelInfo,
    /// a list of properties that are flagged as
    /// deprecated by the CPU vendor.  The list depends on the
    /// CpuModelExpansionType: "static" properties are a subset of the
    /// enabled-properties for the expanded model; "full" properties are
    /// a set of properties that are deprecated across all models for
    /// the architecture.  (since: 9.1).
    #[qapi(name = "deprecated-props")]
    #[qapi(condition = "TARGET_S390X")]
    pub deprecated_props: Vec<String>,
}
/// Expands a given CPU model, @model, (or a combination of CPU model +
/// additional options) to different granularities, specified by @type,
/// allowing tooling to get an understanding what a specific CPU model
/// looks like in QEMU under a certain configuration.
///
/// This interface can be used to query the "host" CPU model.
///
/// The data returned by this command may be affected by:
///
/// QEMU version: CPU models may look different depending on the QEMU
/// version.  (Except for CPU models reported as "static" in
/// query-cpu-definitions.)
/// machine-type: CPU model may look different depending on the
/// machine-type.  (Except for CPU models reported as "static" in
/// query-cpu-definitions.)
/// machine options (including accelerator): in some architectures,
/// CPU models may look different depending on machine and accelerator
/// options.  (Except for CPU models reported as "static" in
/// query-cpu-definitions.)
/// "-cpu" arguments and global properties: arguments to the -cpu
/// option and global properties may affect expansion of CPU models.
/// Using query-cpu-model-expansion while using these is not advised.
///
/// Some architectures may not support all expansion types.  s390x
/// supports "full" and "static".  Arm only supports "full".
#[qapi(name = "query-cpu-model-expansion")]
#[qapi(
    condition = "(TARGET_S390X || TARGET_I386 || TARGET_ARM || TARGET_LOONGARCH64 || TARGET_RISCV)"
)]
#[qapi(since = "2.8")]
#[qapi(returns = "CpuModelExpansionInfo")]
pub struct QueryCpuModelExpansion {
    /// expansion type, specifying how to expand the CPU model
    #[qapi(name = "type")]
    pub r#type: CpuModelExpansionType,
    /// description of the CPU model to expand
    #[qapi(name = "model")]
    pub model: CpuModelInfo,
}
/// Virtual CPU definition.
#[qapi(name = "CpuDefinitionInfo")]
#[qapi(
    condition = "(TARGET_PPC || TARGET_ARM || TARGET_I386 || TARGET_S390X || TARGET_MIPS || TARGET_LOONGARCH64 || TARGET_RISCV)"
)]
#[qapi(since = "1.2")]
pub struct CpuDefinitionInfo {
    /// the name of the CPU definition
    #[qapi(name = "name")]
    pub name: String,
    /// whether a CPU definition can be safely used for
    /// migration in combination with a QEMU compatibility machine when
    /// migrating between different QEMU versions and between hosts with
    /// different sets of (hardware or software) capabilities.  If not
    /// provided, information is not available and callers should not
    /// assume the CPU definition to be migration-safe.  (since 2.8)
    #[qapi(name = "migration-safe")]
    pub migration_safe: Option<bool>,
    /// whether a CPU definition is static and will not change
    /// depending on QEMU version, machine type, machine options and
    /// accelerator options.  A static model is always migration-safe.
    /// (since 2.8)
    #[qapi(name = "static")]
    pub r#static: bool,
    /// List of properties that prevent the CPU model
    /// from running in the current host.  (since 2.8)
    #[qapi(name = "unavailable-features")]
    pub unavailable_features: Option<Vec<String>>,
    /// Type name that can be used as argument to
    /// @device-list-properties, to introspect properties configurable
    /// using -cpu or -global.  (since 2.9)
    #[qapi(name = "typename")]
    pub typename: String,
    /// Name of CPU model this model is an alias for.  The target
    /// of the CPU model alias may change depending on the machine type.
    /// Management software is supposed to translate CPU model aliases
    /// in the VM configuration, because aliases may stop being
    /// migration-safe in the future (since 4.1)
    #[qapi(name = "alias-of")]
    pub alias_of: Option<String>,
    /// If true, this CPU model is deprecated and may be
    /// removed in in some future version of QEMU according to the QEMU
    /// deprecation policy.  (since 5.2)
    ///
    /// @unavailable-features is a list of QOM property names that represent
    /// CPU model attributes that prevent the CPU from running.  If the QOM
    /// property is read-only, that means there's no known way to make the
    /// CPU model run in the current host.  Implementations that choose not
    /// to provide specific information return the property name "type".  If
    /// the property is read-write, it means that it MAY be possible to run
    /// the CPU model in the current host if that property is changed.
    /// Management software can use it as hints to suggest or choose an
    /// alternative for the user, or just to generate meaningful error
    /// messages explaining why the CPU model can't be used.  If
    /// @unavailable-features is an empty list, the CPU model is runnable
    /// using the current host and machine-type.  If @unavailable-features
    /// is not present, runnability information for the CPU is not
    /// available.
    #[qapi(name = "deprecated")]
    pub deprecated: bool,
}
/// Return a list of supported virtual CPU definitions
#[qapi(name = "query-cpu-definitions")]
#[qapi(
    condition = "(TARGET_PPC || TARGET_ARM || TARGET_I386 || TARGET_S390X || TARGET_MIPS || TARGET_LOONGARCH64 || TARGET_RISCV)"
)]
#[qapi(since = "1.2")]
#[qapi(returns = "Vec<CpuDefinitionInfo>")]
pub struct QueryCpuDefinitions {}
/// An enumeration of CPU polarization that can be assumed by a virtual
/// S390 CPU
#[qapi(name = "CpuS390Polarization")]
#[qapi(condition = "TARGET_S390X")]
#[qapi(since = "8.2")]
pub enum CpuS390Polarization {
    #[qapi(name = "horizontal")]
    Horizontal,
    #[qapi(name = "vertical")]
    Vertical,
}
/// Modify the topology by moving the CPU inside the topology tree, or
/// by changing a modifier attribute of a CPU.  Absent values will not
/// be modified.
#[qapi(name = "set-cpu-topology")]
#[qapi(condition = "(TARGET_S390X && CONFIG_KVM)")]
#[qapi(feature = "unstable")]
#[qapi(since = "8.2")]
#[qapi(returns = "()")]
pub struct SetCpuTopology {
    /// the vCPU ID to be moved
    #[qapi(name = "core-id")]
    pub core_id: u16,
    /// destination socket to move the vCPU to
    #[qapi(name = "socket-id")]
    pub socket_id: Option<u16>,
    /// destination book to move the vCPU to
    #[qapi(name = "book-id")]
    pub book_id: Option<u16>,
    /// destination drawer to move the vCPU to
    #[qapi(name = "drawer-id")]
    pub drawer_id: Option<u16>,
    /// entitlement to set
    #[qapi(name = "entitlement")]
    pub entitlement: Option<CpuS390Entitlement>,
    /// whether the provisioning of real to virtual CPU is
    /// dedicated
    #[qapi(name = "dedicated")]
    pub dedicated: Option<bool>,
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
#[qapi(name = "CPU_POLARIZATION_CHANGE")]
#[qapi(condition = "(TARGET_S390X && CONFIG_KVM)")]
#[qapi(feature = "unstable")]
#[qapi(since = "8.2")]
pub struct CpuPolarizationChange {
    /// polarization specified by the guest
    #[qapi(name = "polarization")]
    pub polarization: CpuS390Polarization,
}
/// The result of a CPU polarization query.
#[qapi(name = "CpuPolarizationInfo")]
#[qapi(condition = "(TARGET_S390X && CONFIG_KVM)")]
#[qapi(since = "8.2")]
pub struct CpuPolarizationInfo {
    /// the CPU polarization
    #[qapi(name = "polarization")]
    pub polarization: CpuS390Polarization,
}
#[qapi(name = "query-s390x-cpu-polarization")]
#[qapi(condition = "(TARGET_S390X && CONFIG_KVM)")]
#[qapi(feature = "unstable")]
#[qapi(since = "8.2")]
#[qapi(returns = "CpuPolarizationInfo")]
pub struct QueryS390xCpuPolarization {}
// path end:	qapi/machine-target.json
// path begin:	qapi/replay.json
/// Mode of the replay subsystem.
#[qapi(name = "ReplayMode")]
#[qapi(since = "2.5")]
pub enum ReplayMode {
    /// normal execution mode.  Replay or record are not enabled.
    #[qapi(name = "none")]
    None,
    /// record mode.  All non-deterministic data is written into
    /// the replay log.
    #[qapi(name = "record")]
    Record,
    /// replay mode.  Non-deterministic data required for system
    /// execution is read from the log.
    #[qapi(name = "play")]
    Play,
}
/// Record/replay information.
#[qapi(name = "ReplayInfo")]
#[qapi(since = "5.2")]
pub struct ReplayInfo {
    /// current mode.
    #[qapi(name = "mode")]
    pub mode: ReplayMode,
    /// name of the record/replay log file.  It is present only
    /// in record or replay modes, when the log is recorded or replayed.
    #[qapi(name = "filename")]
    pub filename: Option<String>,
    /// current number of executed instructions.
    #[qapi(name = "icount")]
    pub icount: i64,
}
/// Retrieve the record/replay information.  It includes current
/// instruction count which may be used for @replay-break and
/// @replay-seek commands.
#[qapi(name = "query-replay")]
#[qapi(since = "5.2")]
#[qapi(returns = "ReplayInfo")]
pub struct QueryReplay {}
/// Set replay breakpoint at instruction count @icount.  Execution stops
/// when the specified instruction is reached.  There can be at most one
/// breakpoint.  When breakpoint is set, any prior one is removed.  The
/// breakpoint may be set only in replay mode and only "in the future",
/// i.e. at instruction counts greater than the current one.  The
/// current instruction count can be observed with @query-replay.
#[qapi(name = "replay-break")]
#[qapi(since = "5.2")]
#[qapi(returns = "()")]
pub struct ReplayBreak {
    /// instruction count to stop at
    #[qapi(name = "icount")]
    pub icount: i64,
}
/// Remove replay breakpoint which was set with @replay-break.  The
/// command is ignored when there are no replay breakpoints.
#[qapi(name = "replay-delete-break")]
#[qapi(since = "5.2")]
#[qapi(returns = "()")]
pub struct ReplayDeleteBreak {}
/// Automatically proceed to the instruction count @icount, when
/// replaying the execution.  The command automatically loads nearest
/// snapshot and replays the execution to find the desired instruction.
/// When there is no preceding snapshot or the execution is not
/// replayed, then the command fails.  Instruction count can be obtained
/// with the @query-replay command.
#[qapi(name = "replay-seek")]
#[qapi(since = "5.2")]
#[qapi(returns = "()")]
pub struct ReplaySeek {
    /// target instruction count
    #[qapi(name = "icount")]
    pub icount: i64,
}
// path end:	qapi/replay.json
// path begin:	qapi/yank.json
/// An enumeration of yank instance types.  See @YankInstance for more
/// information.
#[qapi(name = "YankInstanceType")]
#[qapi(since = "6.0")]
pub enum YankInstanceType {
    #[qapi(name = "block-node")]
    BlockNode,
    #[qapi(name = "chardev")]
    Chardev,
    #[qapi(name = "migration")]
    Migration,
}
/// Specifies which block graph node to yank.  See @YankInstance for
/// more information.
#[qapi(name = "YankInstanceBlockNode")]
#[qapi(since = "6.0")]
pub struct YankInstanceBlockNode {
    /// the name of the block graph node
    #[qapi(name = "node-name")]
    pub node_name: String,
}
/// Specifies which character device to yank.  See @YankInstance for
/// more information.
#[qapi(name = "YankInstanceChardev")]
#[qapi(since = "6.0")]
pub struct YankInstanceChardev {
    /// the chardev's ID
    #[qapi(name = "id")]
    pub id: String,
}
pub enum YankInstanceBranch {
    #[qapi(name = "block-node")]
    BlockNode(YankInstanceBlockNode),
    #[qapi(name = "chardev")]
    Chardev(YankInstanceChardev),
}
/// A yank instance can be yanked with the @yank qmp command to recover
/// from a hanging QEMU.
#[qapi(name = "YankInstance")]
#[qapi(since = "6.0")]
pub struct YankInstance {
    /// yank instance type
    ///
    /// Currently implemented yank instances:
    ///
    /// - nbd block device: Yanking it will shut down the connection to the
    /// nbd server without attempting to reconnect.
    /// - socket chardev: Yanking it will shut down the connected socket.
    /// - migration: Yanking it will shut down all migration connections.
    /// Unlike @migrate_cancel, it will not notify the migration process,
    /// so migration will go into @failed state, instead of @cancelled
    /// state.  @yank should be used to recover from hangs.
    #[qapi(name = "type")]
    #[qapi(discriminator)]
    pub r#type: YankInstanceType,
    #[qapi(union)]
    pub u: Option<YankInstanceBranch>,
}
/// Try to recover from hanging QEMU by yanking the specified instances.
/// See @YankInstance for more information.
#[qapi(name = "yank")]
#[qapi(since = "6.0")]
#[qapi(returns = "()")]
#[qapi(allow_oob)]
pub struct Yank {
    /// the instances to be yanked
    #[qapi(name = "instances")]
    pub instances: Vec<YankInstance>,
}
/// Query yank instances.  See @YankInstance for more information.
#[qapi(name = "query-yank")]
#[qapi(since = "6.0")]
#[qapi(returns = "Vec<YankInstance>")]
#[qapi(allow_oob)]
pub struct QueryYank {}
// path end:	qapi/yank.json
// path begin:	qapi/misc.json
/// Allow client connections for VNC, Spice and socket based character
/// devices to be passed in to QEMU via SCM_RIGHTS.
///
/// If the FD associated with @fdname is not a socket, the command will
/// fail and the FD will be closed.
#[qapi(name = "add_client")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct AddClient {
    /// protocol name.  Valid names are "vnc", "spice",
    /// "@dbus-display" or the name of a character device (e.g. from
    /// -chardev id=XXXX)
    #[qapi(name = "protocol")]
    pub protocol: String,
    /// file descriptor name previously passed via 'getfd' command
    #[qapi(name = "fdname")]
    pub fdname: String,
    /// whether to skip authentication.  Only applies to "vnc"
    /// and "spice" protocols
    #[qapi(name = "skipauth")]
    pub skipauth: Option<bool>,
    /// whether to perform TLS.  Only applies to the "spice" protocol
    #[qapi(name = "tls")]
    pub tls: Option<bool>,
}
/// Guest name information.
#[qapi(name = "NameInfo")]
#[qapi(since = "0.14")]
pub struct NameInfo {
    /// The name of the guest
    #[qapi(name = "name")]
    pub name: Option<String>,
}
/// Return the name information of a guest.
#[qapi(name = "query-name")]
#[qapi(since = "0.14")]
#[qapi(returns = "NameInfo")]
#[qapi(allow_preconfig)]
pub struct QueryName {}
/// Information about an iothread
#[qapi(name = "IOThreadInfo")]
#[qapi(since = "2.0")]
pub struct IoThreadInfo {
    /// the identifier of the iothread
    #[qapi(name = "id")]
    pub id: String,
    /// ID of the underlying host thread
    #[qapi(name = "thread-id")]
    pub thread_id: i64,
    /// maximum polling time in ns, 0 means polling is
    /// disabled (since 2.9)
    #[qapi(name = "poll-max-ns")]
    pub poll_max_ns: i64,
    /// how many ns will be added to polling time, 0 means that
    /// it's not configured (since 2.9)
    #[qapi(name = "poll-grow")]
    pub poll_grow: i64,
    /// how many ns will be removed from polling time, 0 means
    /// that it's not configured (since 2.9)
    #[qapi(name = "poll-shrink")]
    pub poll_shrink: i64,
    /// maximum number of requests in a batch for the AIO
    /// engine, 0 means that the engine will use its default (since 6.1)
    #[qapi(name = "aio-max-batch")]
    pub aio_max_batch: i64,
}
/// Returns a list of information about each iothread.
#[qapi(name = "query-iothreads")]
#[qapi(since = "2.0")]
#[qapi(returns = "Vec<IOThreadInfo>")]
#[qapi(allow_preconfig)]
pub struct QueryIothreads {}
/// Stop guest VM execution.
#[qapi(name = "stop")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct Stop {}
/// Resume guest VM execution.
#[qapi(name = "cont")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct Cont {}
/// Exit from "preconfig" state
///
/// This command makes QEMU exit the preconfig state and proceed with VM
/// initialization using configuration data provided on the command line
/// and via the QMP monitor during the preconfig state.  The command is
/// only available during the preconfig state (i.e. when the --preconfig
/// command line option was in use).
#[qapi(name = "x-exit-preconfig")]
#[qapi(feature = "unstable")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct XExitPreconfig {}
/// Execute a command on the human monitor and return the output.
#[qapi(name = "human-monitor-command")]
#[qapi(feature = "savevm-monitor-nodes")]
#[qapi(since = "0.14")]
#[qapi(returns = "str")]
pub struct HumanMonitorCommand {
    /// the command to execute in the human monitor
    #[qapi(name = "command-line")]
    pub command_line: String,
    /// The CPU to use for commands that require an implicit CPU
    #[qapi(name = "cpu-index")]
    pub cpu_index: Option<i64>,
}
/// Receive a file descriptor via SCM rights and assign it a name
#[qapi(name = "getfd")]
#[qapi(condition = "CONFIG_POSIX")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct Getfd {
    /// file descriptor name
    #[qapi(name = "fdname")]
    pub fdname: String,
}
/// Add a socket that was duplicated to QEMU process with
/// WSADuplicateSocketW() via WSASocket() & WSAPROTOCOL_INFOW structure
/// and assign it a name (the SOCKET is associated with a CRT file
/// descriptor)
#[qapi(name = "get-win32-socket")]
#[qapi(condition = "CONFIG_WIN32")]
#[qapi(since = "8.0")]
#[qapi(returns = "()")]
pub struct GetWin32Socket {
    /// the WSAPROTOCOL_INFOW structure (encoded in base64)
    #[qapi(name = "info")]
    pub info: String,
    /// file descriptor name
    #[qapi(name = "fdname")]
    pub fdname: String,
}
/// Close a file descriptor previously passed via SCM rights
#[qapi(name = "closefd")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct Closefd {
    /// file descriptor name
    #[qapi(name = "fdname")]
    pub fdname: String,
}
/// Information about a file descriptor that was added to an fd set.
#[qapi(name = "AddfdInfo")]
#[qapi(since = "1.2")]
pub struct AddfdInfo {
    /// The ID of the fd set that @fd was added to.
    #[qapi(name = "fdset-id")]
    pub fdset_id: i64,
    /// The file descriptor that was received via SCM rights and added
    /// to the fd set.
    #[qapi(name = "fd")]
    pub fd: i64,
}
/// Add a file descriptor, that was passed via SCM rights, to an fd set.
#[qapi(name = "add-fd")]
#[qapi(since = "1.2")]
#[qapi(returns = "AddfdInfo")]
pub struct AddFd {
    /// The ID of the fd set to add the file descriptor to.
    #[qapi(name = "fdset-id")]
    pub fdset_id: Option<i64>,
    /// A free-form string that can be used to describe the fd.
    #[qapi(name = "opaque")]
    pub opaque: Option<String>,
}
/// Remove a file descriptor from an fd set.
#[qapi(name = "remove-fd")]
#[qapi(since = "1.2")]
#[qapi(returns = "()")]
pub struct RemoveFd {
    /// The ID of the fd set that the file descriptor belongs to.
    #[qapi(name = "fdset-id")]
    pub fdset_id: i64,
    /// The file descriptor that is to be removed.
    #[qapi(name = "fd")]
    pub fd: Option<i64>,
}
/// Information about a file descriptor that belongs to an fd set.
#[qapi(name = "FdsetFdInfo")]
#[qapi(since = "1.2")]
pub struct FdsetFdInfo {
    /// The file descriptor value.
    #[qapi(name = "fd")]
    pub fd: i64,
    /// A free-form string that can be used to describe the fd.
    #[qapi(name = "opaque")]
    pub opaque: Option<String>,
}
/// Information about an fd set.
#[qapi(name = "FdsetInfo")]
#[qapi(since = "1.2")]
pub struct FdsetInfo {
    /// The ID of the fd set.
    #[qapi(name = "fdset-id")]
    pub fdset_id: i64,
    /// A list of file descriptors that belong to this fd set.
    #[qapi(name = "fds")]
    pub fds: Vec<FdsetFdInfo>,
}
/// Return information describing all fd sets.
#[qapi(name = "query-fdsets")]
#[qapi(since = "1.2")]
#[qapi(returns = "Vec<FdsetInfo>")]
pub struct QueryFdsets {}
/// Possible types for an option parameter.
#[qapi(name = "CommandLineParameterType")]
#[qapi(since = "1.5")]
pub enum CommandLineParameterType {
    /// accepts a character string
    #[qapi(name = "string")]
    String,
    /// accepts "on" or "off"
    #[qapi(name = "boolean")]
    Boolean,
    /// accepts a number
    #[qapi(name = "number")]
    f64,
    /// accepts a number followed by an optional suffix (K)ilo,
    /// (M)ega, (G)iga, (T)era
    #[qapi(name = "size")]
    u64,
}
/// Details about a single parameter of a command line option.
#[qapi(name = "CommandLineParameterInfo")]
#[qapi(since = "1.5")]
pub struct CommandLineParameterInfo {
    /// parameter name
    #[qapi(name = "name")]
    pub name: String,
    /// parameter @CommandLineParameterType
    #[qapi(name = "type")]
    pub r#type: CommandLineParameterType,
    /// human readable text string, not suitable for parsing.
    #[qapi(name = "help")]
    pub help: Option<String>,
    /// default value string (since 2.1)
    #[qapi(name = "default")]
    pub default: Option<String>,
}
/// Details about a command line option, including its list of parameter
/// details
#[qapi(name = "CommandLineOptionInfo")]
#[qapi(since = "1.5")]
pub struct CommandLineOptionInfo {
    /// option name
    #[qapi(name = "option")]
    pub option: String,
    /// an array of @CommandLineParameterInfo
    #[qapi(name = "parameters")]
    pub parameters: Vec<CommandLineParameterInfo>,
}
/// Query command line option schema.
#[qapi(name = "query-command-line-options")]
#[qapi(since = "1.5")]
#[qapi(returns = "Vec<CommandLineOptionInfo>")]
#[qapi(allow_preconfig)]
pub struct QueryCommandLineOptions {
    /// option name
    #[qapi(name = "option")]
    pub option: Option<String>,
}
/// Emitted when the guest changes the RTC time.
#[qapi(name = "RTC_CHANGE")]
#[qapi(since = "0.13")]
pub struct RtcChange {
    /// offset in seconds between base RTC clock (as specified by
    /// -rtc base), and new RTC clock value
    #[qapi(name = "offset")]
    pub offset: i64,
    /// path to the RTC object in the QOM tree
    #[qapi(name = "qom-path")]
    pub qom_path: String,
}
/// Emitted when the client of a TYPE_VFIO_USER_SERVER closes the
/// communication channel
#[qapi(name = "VFU_CLIENT_HANGUP")]
#[qapi(since = "7.1")]
pub struct VfuClientHangup {
    /// ID of the TYPE_VFIO_USER_SERVER object.  It is the last
    /// component of @vfu-qom-path referenced below
    #[qapi(name = "vfu-id")]
    pub vfu_id: String,
    /// path to the TYPE_VFIO_USER_SERVER object in the QOM
    /// tree
    #[qapi(name = "vfu-qom-path")]
    pub vfu_qom_path: String,
    /// ID of attached PCI device
    #[qapi(name = "dev-id")]
    pub dev_id: String,
    /// path to attached PCI device in the QOM tree
    #[qapi(name = "dev-qom-path")]
    pub dev_qom_path: String,
}
// path end:	qapi/misc.json
// path begin:	qapi/misc-target.json
/// This command will reset the RTC interrupt reinjection backlog.  Can
/// be used if another mechanism to synchronize guest time is in effect,
/// for example QEMU guest agent's guest-set-time command.
#[qapi(name = "rtc-reset-reinjection")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "2.1")]
#[qapi(returns = "()")]
pub struct RtcResetReinjection {}
/// An enumeration of SEV state information used during @query-sev.
#[qapi(name = "SevState")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "2.12")]
pub enum SevState {
    /// The guest is uninitialized.
    #[qapi(name = "uninit")]
    Uninit,
    /// The guest is currently being launched; plaintext
    /// data and register state is being imported.
    #[qapi(name = "launch-update")]
    LaunchUpdate,
    /// The guest is currently being launched; ciphertext
    /// data is being imported.
    #[qapi(name = "launch-secret")]
    LaunchSecret,
    /// The guest is fully launched or migrated in.
    #[qapi(name = "running")]
    Running,
    /// The guest is currently being migrated out to another
    /// machine.
    #[qapi(name = "send-update")]
    SendUpdate,
    /// The guest is currently being migrated from another
    /// machine.
    #[qapi(name = "receive-update")]
    ReceiveUpdate,
}
/// An enumeration indicating the type of SEV guest being run.
#[qapi(name = "SevGuestType")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "6.2")]
pub enum SevGuestType {
    /// The guest is a legacy SEV or SEV-ES guest.
    #[qapi(name = "sev")]
    Sev,
    /// The guest is an SEV-SNP guest.
    #[qapi(name = "sev-snp")]
    SevSnp,
}
/// Information specific to legacy SEV/SEV-ES guests.
#[qapi(name = "SevGuestInfo")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "2.12")]
pub struct SevGuestInfo {
    /// SEV policy value
    #[qapi(name = "policy")]
    pub policy: u32,
    /// SEV firmware handle
    #[qapi(name = "handle")]
    pub handle: u32,
}
/// Information specific to SEV-SNP guests.
#[qapi(name = "SevSnpGuestInfo")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "9.1")]
pub struct SevSnpGuestInfo {
    /// SEV-SNP policy value
    #[qapi(name = "snp-policy")]
    pub snp_policy: u64,
}
pub enum SevInfoBranch {
    #[qapi(name = "sev")]
    Sev(SevGuestInfo),
    #[qapi(name = "sev-snp")]
    SevSnp(SevSnpGuestInfo),
}
/// Information about Secure Encrypted Virtualization (SEV) support
#[qapi(name = "SevInfo")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "2.12")]
pub struct SevInfo {
    /// true if SEV is active
    #[qapi(name = "enabled")]
    pub enabled: bool,
    /// SEV API major version
    #[qapi(name = "api-major")]
    pub api_major: u8,
    /// SEV API minor version
    #[qapi(name = "api-minor")]
    pub api_minor: u8,
    /// SEV FW build id
    #[qapi(name = "build-id")]
    pub build_id: u8,
    /// SEV guest state
    #[qapi(name = "state")]
    pub state: SevState,
    /// Type of SEV guest being run
    #[qapi(name = "sev-type")]
    #[qapi(discriminator)]
    pub sev_type: SevGuestType,
    #[qapi(union)]
    pub u: Option<SevInfoBranch>,
}
/// Returns information about SEV
#[qapi(name = "query-sev")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "2.12")]
#[qapi(returns = "SevInfo")]
pub struct QuerySev {}
/// SEV Guest Launch measurement information
#[qapi(name = "SevLaunchMeasureInfo")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "2.12")]
pub struct SevLaunchMeasureInfo {
    /// the measurement value encoded in base64
    #[qapi(name = "data")]
    pub data: String,
}
/// Query the SEV guest launch information.
#[qapi(name = "query-sev-launch-measure")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "2.12")]
#[qapi(returns = "SevLaunchMeasureInfo")]
pub struct QuerySevLaunchMeasure {}
/// The struct describes capability for a Secure Encrypted
/// Virtualization feature.
#[qapi(name = "SevCapability")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "2.12")]
pub struct SevCapability {
    /// Platform Diffie-Hellman key (base64 encoded)
    #[qapi(name = "pdh")]
    pub pdh: String,
    /// PDH certificate chain (base64 encoded)
    #[qapi(name = "cert-chain")]
    pub cert_chain: String,
    /// Unique ID of CPU0 (base64 encoded) (since 7.1)
    #[qapi(name = "cpu0-id")]
    pub cpu0_id: String,
    /// C-bit location in page table entry
    #[qapi(name = "cbitpos")]
    pub cbitpos: i64,
    /// Number of physical Address bit reduction when
    /// SEV is enabled
    #[qapi(name = "reduced-phys-bits")]
    pub reduced_phys_bits: i64,
}
/// This command is used to get the SEV capabilities, and is supported
/// on AMD X86 platforms only.
#[qapi(name = "query-sev-capabilities")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "2.12")]
#[qapi(returns = "SevCapability")]
pub struct QuerySevCapabilities {}
/// This command injects a secret blob into memory of SEV guest.
#[qapi(name = "sev-inject-launch-secret")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "6.0")]
#[qapi(returns = "()")]
pub struct SevInjectLaunchSecret {
    /// the launch secret packet header encoded in base64
    #[qapi(name = "packet-header")]
    pub packet_header: String,
    /// the launch secret data to be injected encoded in base64
    #[qapi(name = "secret")]
    pub secret: String,
    /// the guest physical address where secret will be injected.
    #[qapi(name = "gpa")]
    pub gpa: Option<u64>,
}
/// The struct describes attestation report for a Secure Encrypted
/// Virtualization feature.
#[qapi(name = "SevAttestationReport")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "6.1")]
pub struct SevAttestationReport {
    /// guest attestation report (base64 encoded)
    #[qapi(name = "data")]
    pub data: String,
}
/// This command is used to get the SEV attestation report, and is
/// supported on AMD X86 platforms only.
#[qapi(name = "query-sev-attestation-report")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "6.1")]
#[qapi(returns = "SevAttestationReport")]
pub struct QuerySevAttestationReport {
    /// a random 16 bytes value encoded in base64 (it will be
    /// included in report)
    #[qapi(name = "mnonce")]
    pub mnonce: String,
}
/// Dump guest's storage keys
#[qapi(name = "dump-skeys")]
#[qapi(condition = "TARGET_S390X")]
#[qapi(since = "2.5")]
#[qapi(returns = "()")]
pub struct DumpSkeys {
    /// the path to the file to dump to
    #[qapi(name = "filename")]
    pub filename: String,
}
/// The struct describes capability for a specific GIC (Generic
/// Interrupt Controller) version.  These bits are not only decided by
/// QEMU/KVM software version, but also decided by the hardware that the
/// program is running upon.
#[qapi(name = "GICCapability")]
#[qapi(condition = "TARGET_ARM")]
#[qapi(since = "2.6")]
pub struct GicCapability {
    /// version of GIC to be described.  Currently, only 2 and 3
    /// are supported.
    #[qapi(name = "version")]
    pub version: i64,
    /// whether current QEMU/hardware supports emulated GIC
    /// device in user space.
    #[qapi(name = "emulated")]
    pub emulated: bool,
    /// whether current QEMU/hardware supports hardware accelerated
    /// GIC device in kernel.
    #[qapi(name = "kernel")]
    pub kernel: bool,
}
/// This command is ARM-only.  It will return a list of GICCapability
/// objects that describe its capability bits.
#[qapi(name = "query-gic-capabilities")]
#[qapi(condition = "TARGET_ARM")]
#[qapi(since = "2.6")]
#[qapi(returns = "Vec<GICCapability>")]
pub struct QueryGicCapabilities {}
/// Information about intel SGX EPC section info
#[qapi(name = "SGXEPCSection")]
#[qapi(since = "7.0")]
pub struct SgxepcSection {
    /// the numa node
    #[qapi(name = "node")]
    pub node: i64,
    /// the size of EPC section
    #[qapi(name = "size")]
    pub size: u64,
}
/// Information about intel Safe Guard eXtension (SGX) support
#[qapi(name = "SGXInfo")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "6.2")]
pub struct SgxInfo {
    /// true if SGX is supported
    #[qapi(name = "sgx")]
    pub sgx: bool,
    /// true if SGX1 is supported
    #[qapi(name = "sgx1")]
    pub sgx1: bool,
    /// true if SGX2 is supported
    #[qapi(name = "sgx2")]
    pub sgx2: bool,
    /// true if FLC is supported
    #[qapi(name = "flc")]
    pub flc: bool,
    /// The EPC sections info for guest (Since: 7.0)
    #[qapi(name = "sections")]
    pub sections: Vec<SgxepcSection>,
}
/// Returns information about SGX
#[qapi(name = "query-sgx")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "6.2")]
#[qapi(returns = "SGXInfo")]
pub struct QuerySgx {}
/// Returns information from host SGX capabilities
#[qapi(name = "query-sgx-capabilities")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "6.2")]
#[qapi(returns = "SGXInfo")]
pub struct QuerySgxCapabilities {}
/// An enumeration of Xen event channel port types.
#[qapi(name = "EvtchnPortType")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "8.0")]
pub enum EvtchnPortType {
    /// The port is unused.
    #[qapi(name = "closed")]
    Closed,
    /// The port is allocated and ready to be bound.
    #[qapi(name = "unbound")]
    Unbound,
    /// The port is connected as an interdomain interrupt.
    #[qapi(name = "interdomain")]
    Interdomain,
    /// The port is bound to a physical IRQ (PIRQ).
    #[qapi(name = "pirq")]
    Pirq,
    /// The port is bound to a virtual IRQ (VIRQ).
    #[qapi(name = "virq")]
    Virq,
    /// The post is an inter-processor interrupt (IPI).
    #[qapi(name = "ipi")]
    Ipi,
}
/// Information about a Xen event channel port
#[qapi(name = "EvtchnInfo")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "8.0")]
pub struct EvtchnInfo {
    /// the port number
    #[qapi(name = "port")]
    pub port: u16,
    /// target vCPU for this port
    #[qapi(name = "vcpu")]
    pub vcpu: u32,
    /// the port type
    #[qapi(name = "type")]
    pub r#type: EvtchnPortType,
    /// remote domain for interdomain ports
    #[qapi(name = "remote-domain")]
    pub remote_domain: String,
    /// remote port ID, or virq/pirq number
    #[qapi(name = "target")]
    pub target: u16,
    /// port is currently active pending delivery
    #[qapi(name = "pending")]
    pub pending: bool,
    /// port is masked
    #[qapi(name = "masked")]
    pub masked: bool,
}
/// Query the Xen event channels opened by the guest.
#[qapi(name = "xen-event-list")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "8.0")]
#[qapi(returns = "Vec<EvtchnInfo>")]
pub struct XenEventList {}
/// Inject a Xen event channel port (interrupt) to the guest.
#[qapi(name = "xen-event-inject")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "8.0")]
#[qapi(returns = "()")]
pub struct XenEventInject {
    /// The port number
    #[qapi(name = "port")]
    pub port: u32,
}
// path end:	qapi/misc-target.json
// path begin:	qapi/audio.json
/// General audio backend options that are used for both playback and
/// recording.
#[qapi(name = "AudiodevPerDirectionOptions")]
#[qapi(since = "4.0")]
pub struct AudiodevPerDirectionOptions {
    /// use QEMU's mixing engine to mix all streams inside
    /// QEMU and convert audio formats when not supported by the
    /// backend.  When set to off, fixed-settings must be also off
    /// (default on, since 4.2)
    #[qapi(name = "mixing-engine")]
    pub mixing_engine: Option<bool>,
    /// use fixed settings for host input/output.  When
    /// off, frequency, channels and format must not be specified
    /// (default true)
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    /// frequency to use when using fixed settings (default
    /// 44100)
    #[qapi(name = "frequency")]
    pub frequency: Option<u32>,
    /// number of channels when using fixed settings (default 2)
    #[qapi(name = "channels")]
    pub channels: Option<u32>,
    /// number of voices to use (default 1)
    #[qapi(name = "voices")]
    pub voices: Option<u32>,
    /// sample format to use when using fixed settings (default
    /// s16)
    #[qapi(name = "format")]
    pub format: Option<AudioFormat>,
    /// the buffer length in microseconds
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
}
/// Generic driver-specific options.
#[qapi(name = "AudiodevGenericOptions")]
#[qapi(since = "4.0")]
pub struct AudiodevGenericOptions {
    /// options of the capture stream
    #[qapi(name = "in")]
    pub r#in: Option<AudiodevPerDirectionOptions>,
    /// options of the playback stream
    #[qapi(name = "out")]
    pub out: Option<AudiodevPerDirectionOptions>,
}
/// Options of the ALSA backend that are used for both playback and
/// recording.
#[qapi(name = "AudiodevAlsaPerDirectionOptions")]
#[qapi(since = "4.0")]
pub struct AudiodevAlsaPerDirectionOptions {
    /// use QEMU's mixing engine to mix all streams inside
    /// QEMU and convert audio formats when not supported by the
    /// backend.  When set to off, fixed-settings must be also off
    /// (default on, since 4.2)
    #[qapi(name = "mixing-engine")]
    pub mixing_engine: Option<bool>,
    /// use fixed settings for host input/output.  When
    /// off, frequency, channels and format must not be specified
    /// (default true)
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    /// frequency to use when using fixed settings (default
    /// 44100)
    #[qapi(name = "frequency")]
    pub frequency: Option<u32>,
    /// number of channels when using fixed settings (default 2)
    #[qapi(name = "channels")]
    pub channels: Option<u32>,
    /// number of voices to use (default 1)
    #[qapi(name = "voices")]
    pub voices: Option<u32>,
    /// sample format to use when using fixed settings (default
    /// s16)
    #[qapi(name = "format")]
    pub format: Option<AudioFormat>,
    /// the buffer length in microseconds
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    /// the name of the ALSA device to use (default 'default')
    #[qapi(name = "dev")]
    pub dev: Option<String>,
    /// the period length in microseconds
    #[qapi(name = "period-length")]
    pub period_length: Option<u32>,
    /// attempt to use poll mode, falling back to non-polling
    /// access on failure (default true)
    #[qapi(name = "try-poll")]
    pub try_poll: Option<bool>,
}
/// Options of the ALSA audio backend.
#[qapi(name = "AudiodevAlsaOptions")]
#[qapi(since = "4.0")]
pub struct AudiodevAlsaOptions {
    /// options of the capture stream
    #[qapi(name = "in")]
    pub r#in: Option<AudiodevAlsaPerDirectionOptions>,
    /// options of the playback stream
    #[qapi(name = "out")]
    pub out: Option<AudiodevAlsaPerDirectionOptions>,
    /// set the threshold (in microseconds) when playback starts
    #[qapi(name = "threshold")]
    pub threshold: Option<u32>,
}
/// Options of the sndio audio backend.
#[qapi(name = "AudiodevSndioOptions")]
#[qapi(since = "7.2")]
pub struct AudiodevSndioOptions {
    /// options of the capture stream
    #[qapi(name = "in")]
    pub r#in: Option<AudiodevPerDirectionOptions>,
    /// options of the playback stream
    #[qapi(name = "out")]
    pub out: Option<AudiodevPerDirectionOptions>,
    /// the name of the sndio device to use (default 'default')
    #[qapi(name = "dev")]
    pub dev: Option<String>,
    /// play buffer size (in microseconds)
    #[qapi(name = "latency")]
    pub latency: Option<u32>,
}
/// Options of the Core Audio backend that are used for both playback
/// and recording.
#[qapi(name = "AudiodevCoreaudioPerDirectionOptions")]
#[qapi(since = "4.0")]
pub struct AudiodevCoreaudioPerDirectionOptions {
    /// use QEMU's mixing engine to mix all streams inside
    /// QEMU and convert audio formats when not supported by the
    /// backend.  When set to off, fixed-settings must be also off
    /// (default on, since 4.2)
    #[qapi(name = "mixing-engine")]
    pub mixing_engine: Option<bool>,
    /// use fixed settings for host input/output.  When
    /// off, frequency, channels and format must not be specified
    /// (default true)
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    /// frequency to use when using fixed settings (default
    /// 44100)
    #[qapi(name = "frequency")]
    pub frequency: Option<u32>,
    /// number of channels when using fixed settings (default 2)
    #[qapi(name = "channels")]
    pub channels: Option<u32>,
    /// number of voices to use (default 1)
    #[qapi(name = "voices")]
    pub voices: Option<u32>,
    /// sample format to use when using fixed settings (default
    /// s16)
    #[qapi(name = "format")]
    pub format: Option<AudioFormat>,
    /// the buffer length in microseconds
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    /// number of buffers
    #[qapi(name = "buffer-count")]
    pub buffer_count: Option<u32>,
}
/// Options of the coreaudio audio backend.
#[qapi(name = "AudiodevCoreaudioOptions")]
#[qapi(since = "4.0")]
pub struct AudiodevCoreaudioOptions {
    /// options of the capture stream
    #[qapi(name = "in")]
    pub r#in: Option<AudiodevCoreaudioPerDirectionOptions>,
    /// options of the playback stream
    #[qapi(name = "out")]
    pub out: Option<AudiodevCoreaudioPerDirectionOptions>,
}
/// Options of the DirectSound audio backend.
#[qapi(name = "AudiodevDsoundOptions")]
#[qapi(since = "4.0")]
pub struct AudiodevDsoundOptions {
    /// options of the capture stream
    #[qapi(name = "in")]
    pub r#in: Option<AudiodevPerDirectionOptions>,
    /// options of the playback stream
    #[qapi(name = "out")]
    pub out: Option<AudiodevPerDirectionOptions>,
    /// add extra latency to playback in microseconds (default
    /// 10000)
    #[qapi(name = "latency")]
    pub latency: Option<u32>,
}
/// Options of the JACK backend that are used for both playback and
/// recording.
#[qapi(name = "AudiodevJackPerDirectionOptions")]
#[qapi(since = "5.1")]
pub struct AudiodevJackPerDirectionOptions {
    /// use QEMU's mixing engine to mix all streams inside
    /// QEMU and convert audio formats when not supported by the
    /// backend.  When set to off, fixed-settings must be also off
    /// (default on, since 4.2)
    #[qapi(name = "mixing-engine")]
    pub mixing_engine: Option<bool>,
    /// use fixed settings for host input/output.  When
    /// off, frequency, channels and format must not be specified
    /// (default true)
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    /// frequency to use when using fixed settings (default
    /// 44100)
    #[qapi(name = "frequency")]
    pub frequency: Option<u32>,
    /// number of channels when using fixed settings (default 2)
    #[qapi(name = "channels")]
    pub channels: Option<u32>,
    /// number of voices to use (default 1)
    #[qapi(name = "voices")]
    pub voices: Option<u32>,
    /// sample format to use when using fixed settings (default
    /// s16)
    #[qapi(name = "format")]
    pub format: Option<AudioFormat>,
    /// the buffer length in microseconds
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    /// select from among several possible concurrent server
    /// instances (default: environment variable $JACK_DEFAULT_SERVER if
    /// set, else "default")
    #[qapi(name = "server-name")]
    pub server_name: Option<String>,
    /// the client name to use.  The server will modify this
    /// name to create a unique variant, if needed unless @exact-name is
    /// true (default: the guest's name)
    #[qapi(name = "client-name")]
    pub client_name: Option<String>,
    /// if set, a regular expression of JACK client port
    /// name(s) to monitor for and automatically connect to
    #[qapi(name = "connect-ports")]
    pub connect_ports: Option<String>,
    /// start a jack server process if one is not already
    /// present (default: false)
    #[qapi(name = "start-server")]
    pub start_server: Option<bool>,
    /// use the exact name requested otherwise JACK
    /// automatically generates a unique one, if needed (default: false)
    #[qapi(name = "exact-name")]
    pub exact_name: Option<bool>,
}
/// Options of the JACK audio backend.
#[qapi(name = "AudiodevJackOptions")]
#[qapi(since = "5.1")]
pub struct AudiodevJackOptions {
    /// options of the capture stream
    #[qapi(name = "in")]
    pub r#in: Option<AudiodevJackPerDirectionOptions>,
    /// options of the playback stream
    #[qapi(name = "out")]
    pub out: Option<AudiodevJackPerDirectionOptions>,
}
/// Options of the OSS backend that are used for both playback and
/// recording.
#[qapi(name = "AudiodevOssPerDirectionOptions")]
#[qapi(since = "4.0")]
pub struct AudiodevOssPerDirectionOptions {
    /// use QEMU's mixing engine to mix all streams inside
    /// QEMU and convert audio formats when not supported by the
    /// backend.  When set to off, fixed-settings must be also off
    /// (default on, since 4.2)
    #[qapi(name = "mixing-engine")]
    pub mixing_engine: Option<bool>,
    /// use fixed settings for host input/output.  When
    /// off, frequency, channels and format must not be specified
    /// (default true)
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    /// frequency to use when using fixed settings (default
    /// 44100)
    #[qapi(name = "frequency")]
    pub frequency: Option<u32>,
    /// number of channels when using fixed settings (default 2)
    #[qapi(name = "channels")]
    pub channels: Option<u32>,
    /// number of voices to use (default 1)
    #[qapi(name = "voices")]
    pub voices: Option<u32>,
    /// sample format to use when using fixed settings (default
    /// s16)
    #[qapi(name = "format")]
    pub format: Option<AudioFormat>,
    /// the buffer length in microseconds
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    /// file name of the OSS device (default '/dev/dsp')
    #[qapi(name = "dev")]
    pub dev: Option<String>,
    /// number of buffers
    #[qapi(name = "buffer-count")]
    pub buffer_count: Option<u32>,
    /// attempt to use poll mode, falling back to non-polling
    /// access on failure (default true)
    #[qapi(name = "try-poll")]
    pub try_poll: Option<bool>,
}
/// Options of the OSS audio backend.
#[qapi(name = "AudiodevOssOptions")]
#[qapi(since = "4.0")]
pub struct AudiodevOssOptions {
    /// options of the capture stream
    #[qapi(name = "in")]
    pub r#in: Option<AudiodevOssPerDirectionOptions>,
    /// options of the playback stream
    #[qapi(name = "out")]
    pub out: Option<AudiodevOssPerDirectionOptions>,
    /// try using memory-mapped access, falling back to
    /// non-memory-mapped access on failure (default true)
    #[qapi(name = "try-mmap")]
    pub try_mmap: Option<bool>,
    /// open device in exclusive mode (vmix won't work) (default
    /// false)
    #[qapi(name = "exclusive")]
    pub exclusive: Option<bool>,
    /// set the timing policy of the device (between 0 and 10,
    /// where smaller number means smaller latency but higher CPU usage)
    /// or -1 to use fragment mode (option ignored on some platforms)
    /// (default 5)
    #[qapi(name = "dsp-policy")]
    pub dsp_policy: Option<u32>,
}
/// Options of the Pulseaudio backend that are used for both playback
/// and recording.
#[qapi(name = "AudiodevPaPerDirectionOptions")]
#[qapi(since = "4.0")]
pub struct AudiodevPaPerDirectionOptions {
    /// use QEMU's mixing engine to mix all streams inside
    /// QEMU and convert audio formats when not supported by the
    /// backend.  When set to off, fixed-settings must be also off
    /// (default on, since 4.2)
    #[qapi(name = "mixing-engine")]
    pub mixing_engine: Option<bool>,
    /// use fixed settings for host input/output.  When
    /// off, frequency, channels and format must not be specified
    /// (default true)
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    /// frequency to use when using fixed settings (default
    /// 44100)
    #[qapi(name = "frequency")]
    pub frequency: Option<u32>,
    /// number of channels when using fixed settings (default 2)
    #[qapi(name = "channels")]
    pub channels: Option<u32>,
    /// number of voices to use (default 1)
    #[qapi(name = "voices")]
    pub voices: Option<u32>,
    /// sample format to use when using fixed settings (default
    /// s16)
    #[qapi(name = "format")]
    pub format: Option<AudioFormat>,
    /// the buffer length in microseconds
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    /// name of the sink/source to use
    #[qapi(name = "name")]
    pub name: Option<String>,
    /// name of the PulseAudio stream created by qemu.  Can be
    /// used to identify the stream in PulseAudio when you create
    /// multiple PulseAudio devices or run multiple qemu instances
    /// (default: audiodev's id, since 4.2)
    #[qapi(name = "stream-name")]
    pub stream_name: Option<String>,
    /// latency you want PulseAudio to achieve in microseconds
    /// (default 15000)
    #[qapi(name = "latency")]
    pub latency: Option<u32>,
}
/// Options of the PulseAudio audio backend.
#[qapi(name = "AudiodevPaOptions")]
#[qapi(since = "4.0")]
pub struct AudiodevPaOptions {
    /// options of the capture stream
    #[qapi(name = "in")]
    pub r#in: Option<AudiodevPaPerDirectionOptions>,
    /// options of the playback stream
    #[qapi(name = "out")]
    pub out: Option<AudiodevPaPerDirectionOptions>,
    /// PulseAudio server address (default: let PulseAudio choose)
    #[qapi(name = "server")]
    pub server: Option<String>,
}
/// Options of the PipeWire backend that are used for both playback and
/// recording.
#[qapi(name = "AudiodevPipewirePerDirectionOptions")]
#[qapi(since = "8.1")]
pub struct AudiodevPipewirePerDirectionOptions {
    /// use QEMU's mixing engine to mix all streams inside
    /// QEMU and convert audio formats when not supported by the
    /// backend.  When set to off, fixed-settings must be also off
    /// (default on, since 4.2)
    #[qapi(name = "mixing-engine")]
    pub mixing_engine: Option<bool>,
    /// use fixed settings for host input/output.  When
    /// off, frequency, channels and format must not be specified
    /// (default true)
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    /// frequency to use when using fixed settings (default
    /// 44100)
    #[qapi(name = "frequency")]
    pub frequency: Option<u32>,
    /// number of channels when using fixed settings (default 2)
    #[qapi(name = "channels")]
    pub channels: Option<u32>,
    /// number of voices to use (default 1)
    #[qapi(name = "voices")]
    pub voices: Option<u32>,
    /// sample format to use when using fixed settings (default
    /// s16)
    #[qapi(name = "format")]
    pub format: Option<AudioFormat>,
    /// the buffer length in microseconds
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    /// name of the sink/source to use
    #[qapi(name = "name")]
    pub name: Option<String>,
    /// name of the PipeWire stream created by qemu.  Can be
    /// used to identify the stream in PipeWire when you create multiple
    /// PipeWire devices or run multiple qemu instances (default:
    /// audiodev's id)
    #[qapi(name = "stream-name")]
    pub stream_name: Option<String>,
    /// latency you want PipeWire to achieve in microseconds
    /// (default 46000)
    #[qapi(name = "latency")]
    pub latency: Option<u32>,
}
/// Options of the PipeWire audio backend.
#[qapi(name = "AudiodevPipewireOptions")]
#[qapi(since = "8.1")]
pub struct AudiodevPipewireOptions {
    /// options of the capture stream
    #[qapi(name = "in")]
    pub r#in: Option<AudiodevPipewirePerDirectionOptions>,
    /// options of the playback stream
    #[qapi(name = "out")]
    pub out: Option<AudiodevPipewirePerDirectionOptions>,
}
/// Options of the SDL audio backend that are used for both playback and
/// recording.
#[qapi(name = "AudiodevSdlPerDirectionOptions")]
#[qapi(since = "6.0")]
pub struct AudiodevSdlPerDirectionOptions {
    /// use QEMU's mixing engine to mix all streams inside
    /// QEMU and convert audio formats when not supported by the
    /// backend.  When set to off, fixed-settings must be also off
    /// (default on, since 4.2)
    #[qapi(name = "mixing-engine")]
    pub mixing_engine: Option<bool>,
    /// use fixed settings for host input/output.  When
    /// off, frequency, channels and format must not be specified
    /// (default true)
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    /// frequency to use when using fixed settings (default
    /// 44100)
    #[qapi(name = "frequency")]
    pub frequency: Option<u32>,
    /// number of channels when using fixed settings (default 2)
    #[qapi(name = "channels")]
    pub channels: Option<u32>,
    /// number of voices to use (default 1)
    #[qapi(name = "voices")]
    pub voices: Option<u32>,
    /// sample format to use when using fixed settings (default
    /// s16)
    #[qapi(name = "format")]
    pub format: Option<AudioFormat>,
    /// the buffer length in microseconds
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    /// number of buffers (default 4)
    #[qapi(name = "buffer-count")]
    pub buffer_count: Option<u32>,
}
/// Options of the SDL audio backend.
#[qapi(name = "AudiodevSdlOptions")]
#[qapi(since = "6.0")]
pub struct AudiodevSdlOptions {
    /// options of the recording stream
    #[qapi(name = "in")]
    pub r#in: Option<AudiodevSdlPerDirectionOptions>,
    /// options of the playback stream
    #[qapi(name = "out")]
    pub out: Option<AudiodevSdlPerDirectionOptions>,
}
/// Options of the wav audio backend.
#[qapi(name = "AudiodevWavOptions")]
#[qapi(since = "4.0")]
pub struct AudiodevWavOptions {
    /// options of the capture stream
    #[qapi(name = "in")]
    pub r#in: Option<AudiodevPerDirectionOptions>,
    /// options of the playback stream
    #[qapi(name = "out")]
    pub out: Option<AudiodevPerDirectionOptions>,
    /// name of the wav file to record (default 'qemu.wav')
    #[qapi(name = "path")]
    pub path: Option<String>,
}
/// An enumeration of possible audio formats.
#[qapi(name = "AudioFormat")]
#[qapi(since = "4.0")]
pub enum AudioFormat {
    /// unsigned 8 bit integer
    #[qapi(name = "u8")]
    U8,
    /// signed 8 bit integer
    #[qapi(name = "s8")]
    S8,
    /// unsigned 16 bit integer
    #[qapi(name = "u16")]
    U16,
    /// signed 16 bit integer
    #[qapi(name = "s16")]
    S16,
    /// unsigned 32 bit integer
    #[qapi(name = "u32")]
    U32,
    /// signed 32 bit integer
    #[qapi(name = "s32")]
    S32,
    /// single precision floating-point (since 5.0)
    #[qapi(name = "f32")]
    F32,
}
/// An enumeration of possible audio backend drivers.
#[qapi(name = "AudiodevDriver")]
#[qapi(since = "4.0")]
pub enum AudiodevDriver {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "alsa")]
    #[qapi(condition = "CONFIG_AUDIO_ALSA")]
    Alsa,
    #[qapi(name = "coreaudio")]
    #[qapi(condition = "CONFIG_AUDIO_COREAUDIO")]
    Coreaudio,
    #[qapi(name = "dbus")]
    #[qapi(condition = "CONFIG_DBUS_DISPLAY")]
    Dbus,
    #[qapi(name = "dsound")]
    #[qapi(condition = "CONFIG_AUDIO_DSOUND")]
    Dsound,
    /// JACK audio backend (since 5.1)
    #[qapi(name = "jack")]
    #[qapi(condition = "CONFIG_AUDIO_JACK")]
    Jack,
    #[qapi(name = "oss")]
    #[qapi(condition = "CONFIG_AUDIO_OSS")]
    Oss,
    #[qapi(name = "pa")]
    #[qapi(condition = "CONFIG_AUDIO_PA")]
    Pa,
    #[qapi(name = "pipewire")]
    #[qapi(condition = "CONFIG_AUDIO_PIPEWIRE")]
    Pipewire,
    #[qapi(name = "sdl")]
    #[qapi(condition = "CONFIG_AUDIO_SDL")]
    Sdl,
    #[qapi(name = "sndio")]
    #[qapi(condition = "CONFIG_AUDIO_SNDIO")]
    Sndio,
    #[qapi(name = "spice")]
    #[qapi(condition = "CONFIG_SPICE")]
    Spice,
    #[qapi(name = "wav")]
    Wav,
}
pub enum AudiodevBranch {
    #[qapi(name = "none")]
    None(AudiodevGenericOptions),
    #[qapi(name = "alsa")]
    #[qapi(condition = "CONFIG_AUDIO_ALSA")]
    Alsa(AudiodevAlsaOptions),
    #[qapi(name = "coreaudio")]
    #[qapi(condition = "CONFIG_AUDIO_COREAUDIO")]
    Coreaudio(AudiodevCoreaudioOptions),
    #[qapi(name = "dbus")]
    #[qapi(condition = "CONFIG_DBUS_DISPLAY")]
    Dbus(AudiodevGenericOptions),
    #[qapi(name = "dsound")]
    #[qapi(condition = "CONFIG_AUDIO_DSOUND")]
    Dsound(AudiodevDsoundOptions),
    #[qapi(name = "jack")]
    #[qapi(condition = "CONFIG_AUDIO_JACK")]
    Jack(AudiodevJackOptions),
    #[qapi(name = "oss")]
    #[qapi(condition = "CONFIG_AUDIO_OSS")]
    Oss(AudiodevOssOptions),
    #[qapi(name = "pa")]
    #[qapi(condition = "CONFIG_AUDIO_PA")]
    Pa(AudiodevPaOptions),
    #[qapi(name = "pipewire")]
    #[qapi(condition = "CONFIG_AUDIO_PIPEWIRE")]
    Pipewire(AudiodevPipewireOptions),
    #[qapi(name = "sdl")]
    #[qapi(condition = "CONFIG_AUDIO_SDL")]
    Sdl(AudiodevSdlOptions),
    #[qapi(name = "sndio")]
    #[qapi(condition = "CONFIG_AUDIO_SNDIO")]
    Sndio(AudiodevSndioOptions),
    #[qapi(name = "spice")]
    #[qapi(condition = "CONFIG_SPICE")]
    Spice(AudiodevGenericOptions),
    #[qapi(name = "wav")]
    Wav(AudiodevWavOptions),
}
/// Options of an audio backend.
#[qapi(name = "Audiodev")]
#[qapi(since = "4.0")]
pub struct Audiodev {
    /// identifier of the backend
    #[qapi(name = "id")]
    pub id: String,
    /// the backend driver to use
    #[qapi(name = "driver")]
    #[qapi(discriminator)]
    pub driver: AudiodevDriver,
    /// timer period (in microseconds, 0: use lowest
    /// possible)
    #[qapi(name = "timer-period")]
    pub timer_period: Option<u32>,
    #[qapi(union)]
    pub u: Option<AudiodevBranch>,
}
/// Returns information about audiodev configuration
#[qapi(name = "query-audiodevs")]
#[qapi(since = "8.0")]
#[qapi(returns = "Vec<Audiodev>")]
pub struct QueryAudiodevs {}
// path end:	qapi/audio.json
// path begin:	qapi/acpi.json
/// Specify an ACPI table on the command line to load.
///
/// At most one of @file and @data can be specified.  The list of files
/// specified by any one of them is loaded and concatenated in order.
/// If both are omitted, @data is implied.
///
/// Other fields / optargs can be used to override fields of the generic
/// ACPI table header; refer to the ACPI specification 5.0, section
/// 5.2.6 System Description Table Header.  If a header field is not
/// overridden, then the corresponding value from the concatenated blob
/// is used (in case of @file), or it is filled in with a hard-coded
/// value (in case of @data).
///
/// String fields are copied into the matching ACPI member from lowest
/// address upwards, and silently truncated / NUL-padded to length.
#[qapi(name = "AcpiTableOptions")]
#[qapi(since = "1.5")]
pub struct AcpiTableOptions {
    /// table signature / identifier (4 bytes)
    #[qapi(name = "sig")]
    pub sig: Option<String>,
    /// table revision number (dependent on signature, 1 byte)
    #[qapi(name = "rev")]
    pub rev: Option<u8>,
    /// OEM identifier (6 bytes)
    #[qapi(name = "oem_id")]
    pub oem_id: Option<String>,
    /// OEM table identifier (8 bytes)
    #[qapi(name = "oem_table_id")]
    pub oem_table_id: Option<String>,
    /// OEM-supplied revision number (4 bytes)
    #[qapi(name = "oem_rev")]
    pub oem_rev: Option<u32>,
    /// identifier of the utility that created the table
    /// (4 bytes)
    #[qapi(name = "asl_compiler_id")]
    pub asl_compiler_id: Option<String>,
    /// revision number of the utility that created the
    /// table (4 bytes)
    #[qapi(name = "asl_compiler_rev")]
    pub asl_compiler_rev: Option<u32>,
    /// colon (:) separated list of pathnames to load and concatenate
    /// as table data.  The resultant binary blob is expected to have an
    /// ACPI table header.  At least one file is required.  This field
    /// excludes @data.
    #[qapi(name = "file")]
    pub file: Option<String>,
    /// colon (:) separated list of pathnames to load and concatenate
    /// as table data.  The resultant binary blob must not have an ACPI
    /// table header.  At least one file is required.  This field
    /// excludes @file.
    #[qapi(name = "data")]
    pub data: Option<String>,
}
#[qapi(name = "ACPISlotType")]
pub enum AcpiSlotType {
    /// memory slot
    #[qapi(name = "DIMM")]
    Dimm,
    /// logical CPU slot (since 2.7)
    #[qapi(name = "CPU")]
    Cpu,
}
/// OSPM Status Indication for a device For description of possible
/// values of @source and @status fields see "_OST (OSPM Status
/// Indication)" chapter of ACPI5.0 spec.
#[qapi(name = "ACPIOSTInfo")]
#[qapi(since = "2.1")]
pub struct AcpiostInfo {
    /// device ID associated with slot
    #[qapi(name = "device")]
    pub device: Option<String>,
    /// slot ID, unique per slot of a given @slot-type
    #[qapi(name = "slot")]
    pub slot: String,
    /// type of the slot
    #[qapi(name = "slot-type")]
    pub slot_type: AcpiSlotType,
    /// an integer containing the source event
    #[qapi(name = "source")]
    pub source: i64,
    /// an integer containing the status code
    #[qapi(name = "status")]
    pub status: i64,
}
/// Return a list of ACPIOSTInfo for devices that support status
/// reporting via ACPI _OST method.
#[qapi(name = "query-acpi-ospm-status")]
#[qapi(since = "2.1")]
#[qapi(returns = "Vec<ACPIOSTInfo>")]
pub struct QueryAcpiOspmStatus {}
/// Emitted when guest executes ACPI _OST method.
#[qapi(name = "ACPI_DEVICE_OST")]
#[qapi(since = "2.1")]
pub struct AcpiDeviceOst {
    /// OSPM Status Indication
    #[qapi(name = "info")]
    pub info: AcpiostInfo,
}
// path end:	qapi/acpi.json
// path begin:	qapi/pci.json
/// A PCI device memory region
#[qapi(name = "PciMemoryRange")]
#[qapi(since = "0.14")]
pub struct PciMemoryRange {
    /// the starting address (guest physical)
    #[qapi(name = "base")]
    pub base: i64,
    /// the ending address (guest physical)
    #[qapi(name = "limit")]
    pub limit: i64,
}
/// Information about a PCI device I/O region.
#[qapi(name = "PciMemoryRegion")]
#[qapi(since = "0.14")]
pub struct PciMemoryRegion {
    /// the index of the Base Address Register for this region
    #[qapi(name = "bar")]
    pub bar: i64,
    /// - 'io' if the region is a PIO region
    /// - 'memory' if the region is a MMIO region
    #[qapi(name = "type")]
    pub r#type: String,
    #[qapi(name = "address")]
    pub address: i64,
    /// memory size
    #[qapi(name = "size")]
    pub size: i64,
    /// if @type is 'memory', true if the memory is prefetchable
    #[qapi(name = "prefetch")]
    pub prefetch: Option<bool>,
    /// if @type is 'memory', true if the BAR is 64-bit
    #[qapi(name = "mem_type_64")]
    pub mem_type_64: Option<bool>,
}
/// Information about a bus of a PCI Bridge device
#[qapi(name = "PciBusInfo")]
#[qapi(since = "2.4")]
pub struct PciBusInfo {
    /// primary bus interface number.  This should be the number of
    /// the bus the device resides on.
    #[qapi(name = "number")]
    pub number: i64,
    /// secondary bus interface number.  This is the number of
    /// the main bus for the bridge
    #[qapi(name = "secondary")]
    pub secondary: i64,
    /// This is the highest number bus that resides below the
    /// bridge.
    #[qapi(name = "subordinate")]
    pub subordinate: i64,
    /// The PIO range for all devices on this bridge
    #[qapi(name = "io_range")]
    pub io_range: PciMemoryRange,
    /// The MMIO range for all devices on this bridge
    #[qapi(name = "memory_range")]
    pub memory_range: PciMemoryRange,
    /// The range of prefetchable MMIO for all devices
    /// on this bridge
    #[qapi(name = "prefetchable_range")]
    pub prefetchable_range: PciMemoryRange,
}
/// Information about a PCI Bridge device
#[qapi(name = "PciBridgeInfo")]
#[qapi(since = "0.14")]
pub struct PciBridgeInfo {
    /// information about the bus the device resides on
    #[qapi(name = "bus")]
    pub bus: PciBusInfo,
    /// a list of @PciDeviceInfo for each device on this bridge
    #[qapi(name = "devices")]
    pub devices: Option<Vec<PciDeviceInfo>>,
}
/// Information about the Class of a PCI device
#[qapi(name = "PciDeviceClass")]
#[qapi(since = "2.4")]
pub struct PciDeviceClass {
    /// a string description of the device's class (not stable, and
    /// should only be treated as informational)
    #[qapi(name = "desc")]
    pub desc: Option<String>,
    /// the class code of the device
    #[qapi(name = "class")]
    pub class: i64,
}
/// Information about the Id of a PCI device
#[qapi(name = "PciDeviceId")]
#[qapi(since = "2.4")]
pub struct PciDeviceId {
    /// the PCI device id
    #[qapi(name = "device")]
    pub device: i64,
    /// the PCI vendor id
    #[qapi(name = "vendor")]
    pub vendor: i64,
    /// the PCI subsystem id (since 3.1)
    #[qapi(name = "subsystem")]
    pub subsystem: Option<i64>,
    /// the PCI subsystem vendor id (since 3.1)
    #[qapi(name = "subsystem-vendor")]
    pub subsystem_vendor: Option<i64>,
}
/// Information about a PCI device
#[qapi(name = "PciDeviceInfo")]
#[qapi(since = "0.14")]
pub struct PciDeviceInfo {
    /// the bus number of the device
    #[qapi(name = "bus")]
    pub bus: i64,
    /// the slot the device is located in
    #[qapi(name = "slot")]
    pub slot: i64,
    /// the function of the slot used by the device
    #[qapi(name = "function")]
    pub function: i64,
    /// the class of the device
    #[qapi(name = "class_info")]
    pub class_info: PciDeviceClass,
    /// the PCI device id
    #[qapi(name = "id")]
    pub id: PciDeviceId,
    /// if an IRQ is assigned to the device, the IRQ number
    #[qapi(name = "irq")]
    pub irq: Option<i64>,
    /// the IRQ pin, zero means no IRQ (since 5.1)
    #[qapi(name = "irq_pin")]
    pub irq_pin: i64,
    /// the device name of the PCI device
    #[qapi(name = "qdev_id")]
    pub qdev_id: String,
    /// if the device is a PCI bridge, the bridge information
    #[qapi(name = "pci_bridge")]
    pub pci_bridge: Option<PciBridgeInfo>,
    /// a list of the PCI I/O regions associated with the device
    #[qapi(name = "regions")]
    pub regions: Vec<PciMemoryRegion>,
}
/// Information about a PCI bus
#[qapi(name = "PciInfo")]
#[qapi(since = "0.14")]
pub struct PciInfo {
    /// the bus index
    #[qapi(name = "bus")]
    pub bus: i64,
    /// a list of devices on this bus
    #[qapi(name = "devices")]
    pub devices: Vec<PciDeviceInfo>,
}
/// Return information about the PCI bus topology of the guest.
#[qapi(name = "query-pci")]
#[qapi(since = "0.14")]
#[qapi(returns = "Vec<PciInfo>")]
pub struct QueryPci {}
// path end:	qapi/pci.json
// path begin:	qapi/stats.json
/// Enumeration of statistics types
#[qapi(name = "StatsType")]
#[qapi(since = "7.1")]
pub enum StatsType {
    /// stat is cumulative; value can only increase.
    #[qapi(name = "cumulative")]
    Cumulative,
    /// stat is instantaneous; value can increase or decrease.
    #[qapi(name = "instant")]
    Instant,
    /// stat is the peak value; value can only increase.
    #[qapi(name = "peak")]
    Peak,
    /// stat is a linear histogram.
    #[qapi(name = "linear-histogram")]
    LinearHistogram,
    /// stat is a logarithmic histogram, with one bucket
    /// for each power of two.
    #[qapi(name = "log2-histogram")]
    Log2Histogram,
}
/// Enumeration of unit of measurement for statistics
#[qapi(name = "StatsUnit")]
#[qapi(since = "7.1")]
pub enum StatsUnit {
    /// stat reported in bytes.
    #[qapi(name = "bytes")]
    Bytes,
    /// stat reported in seconds.
    #[qapi(name = "seconds")]
    Seconds,
    /// stat reported in clock cycles.
    #[qapi(name = "cycles")]
    Cycles,
    /// stat is a boolean value.
    #[qapi(name = "boolean")]
    Boolean,
}
/// Enumeration of statistics providers.
#[qapi(name = "StatsProvider")]
#[qapi(since = "7.1")]
pub enum StatsProvider {
    /// since 7.1
    #[qapi(name = "kvm")]
    Kvm,
    /// since 8.0
    #[qapi(name = "cryptodev")]
    Cryptodev,
}
/// The kinds of objects on which one can request statistics.
#[qapi(name = "StatsTarget")]
#[qapi(since = "7.1")]
pub enum StatsTarget {
    /// statistics that apply to the entire virtual machine or the
    /// entire QEMU process.
    #[qapi(name = "vm")]
    Vm,
    /// statistics that apply to a single virtual CPU.
    #[qapi(name = "vcpu")]
    Vcpu,
    /// statistics that apply to a crypto device (since 8.0)
    #[qapi(name = "cryptodev")]
    Cryptodev,
}
/// Indicates a set of statistics that should be returned by
/// query-stats.
#[qapi(name = "StatsRequest")]
#[qapi(since = "7.1")]
pub struct StatsRequest {
    /// provider for which to return statistics.
    #[qapi(name = "provider")]
    pub provider: StatsProvider,
    /// statistics to be returned (all if omitted).
    #[qapi(name = "names")]
    pub names: Option<Vec<String>>,
}
#[qapi(name = "StatsVCPUFilter")]
#[qapi(since = "7.1")]
pub struct StatsVcpuFilter {
    /// list of QOM paths for the desired vCPU objects.
    #[qapi(name = "vcpus")]
    pub vcpus: Option<Vec<String>>,
}
pub enum StatsFilterBranch {
    #[qapi(name = "vcpu")]
    Vcpu(StatsVcpuFilter),
}
/// The arguments to the query-stats command; specifies a target for
/// which to request statistics and optionally the required subset of
/// information for that target.
#[qapi(name = "StatsFilter")]
#[qapi(since = "7.1")]
pub struct StatsFilter {
    /// the kind of objects to query.  Note that each possible
    /// target may enable additional filtering options
    #[qapi(name = "target")]
    #[qapi(discriminator)]
    pub target: StatsTarget,
    /// which providers to request statistics from, and
    /// optionally which named values to return within each provider
    #[qapi(name = "providers")]
    pub providers: Option<Vec<StatsRequest>>,
    #[qapi(union)]
    pub u: Option<StatsFilterBranch>,
}
#[qapi(name = "StatsValue")]
#[qapi(since = "7.1")]
pub enum StatsValue {
    /// single unsigned 64-bit integers.
    #[qapi(name = "scalar")]
    Scalar(u64),
    /// single boolean value.
    #[qapi(name = "boolean")]
    Boolean(bool),
    /// list of unsigned 64-bit integers (used for histograms).
    #[qapi(name = "list")]
    List(u64),
}
#[qapi(name = "Stats")]
#[qapi(since = "7.1")]
pub struct Stats {
    /// name of stat.
    #[qapi(name = "name")]
    pub name: String,
    /// stat value.
    #[qapi(name = "value")]
    pub value: StatsValue,
}
#[qapi(name = "StatsResult")]
#[qapi(since = "7.1")]
pub struct StatsResult {
    /// provider for this set of statistics.
    #[qapi(name = "provider")]
    pub provider: StatsProvider,
    /// Path to the object for which the statistics are returned,
    /// if the object is exposed in the QOM tree
    #[qapi(name = "qom-path")]
    pub qom_path: Option<String>,
    /// list of statistics.
    #[qapi(name = "stats")]
    pub stats: Vec<Stats>,
}
/// Return runtime-collected statistics for objects such as the VM or
/// its vCPUs.
///
/// The arguments are a StatsFilter and specify the provider and objects
/// to return statistics about.
#[qapi(name = "query-stats")]
#[qapi(since = "7.1")]
#[qapi(returns = "Vec<StatsResult>")]
pub struct QueryStats {
    #[qapi(flatten)]
    pub data: StatsFilter,
}
/// Schema for a single statistic.
#[qapi(name = "StatsSchemaValue")]
#[qapi(since = "7.1")]
pub struct StatsSchemaValue {
    /// name of the statistic; each element of the schema is uniquely
    /// identified by a target, a provider (both available in
    /// @StatsSchema) and the name.
    #[qapi(name = "name")]
    pub name: String,
    /// kind of statistic.
    #[qapi(name = "type")]
    pub r#type: StatsType,
    /// basic unit of measure for the statistic; if missing, the
    /// statistic is a simple number or counter.
    #[qapi(name = "unit")]
    pub unit: Option<StatsUnit>,
    /// base for the multiple of @unit in which the statistic is
    /// measured.  Only present if @exponent is non-zero; @base and
    /// @exponent together form a SI prefix (e.g., _nano-_ for
    /// ``base=10`` and ``exponent=-9``) or IEC binary prefix (e.g.
    /// _kibi-_ for ``base=2`` and ``exponent=10``)
    #[qapi(name = "base")]
    pub base: Option<i8>,
    /// exponent for the multiple of @unit in which the statistic
    /// is expressed, or 0 for the basic unit
    #[qapi(name = "exponent")]
    pub exponent: i16,
    /// Present when @type is "linear-histogram", contains the
    /// width of each bucket of the histogram.
    #[qapi(name = "bucket-size")]
    pub bucket_size: Option<u32>,
}
/// Schema for all available statistics for a provider and target.
#[qapi(name = "StatsSchema")]
#[qapi(since = "7.1")]
pub struct StatsSchema {
    /// provider for this set of statistics.
    #[qapi(name = "provider")]
    pub provider: StatsProvider,
    /// the kind of object that can be queried through the
    /// provider.
    #[qapi(name = "target")]
    pub target: StatsTarget,
    /// list of statistics.
    #[qapi(name = "stats")]
    pub stats: Vec<StatsSchemaValue>,
}
/// Return the schema for all available runtime-collected statistics.
#[qapi(name = "query-stats-schemas")]
#[qapi(since = "7.1")]
#[qapi(returns = "Vec<StatsSchema>")]
pub struct QueryStatsSchemas {
    /// a provider to restrict the query to.
    #[qapi(name = "provider")]
    pub provider: Option<StatsProvider>,
}
// path end:	qapi/stats.json
// path begin:	qapi/virtio.json
/// Basic information about a given VirtIODevice
#[qapi(name = "VirtioInfo")]
#[qapi(since = "7.2")]
pub struct VirtioInfo {
    /// The VirtIODevice's canonical QOM path
    #[qapi(name = "path")]
    pub path: String,
    /// Name of the VirtIODevice
    #[qapi(name = "name")]
    pub name: String,
}
/// Returns a list of all realized VirtIODevices
#[qapi(name = "x-query-virtio")]
#[qapi(feature = "unstable")]
#[qapi(since = "7.2")]
#[qapi(returns = "Vec<VirtioInfo>")]
pub struct XQueryVirtio {}
/// Information about a vhost device.  This information will only be
/// displayed if the vhost device is active.
#[qapi(name = "VhostStatus")]
#[qapi(since = "7.2")]
pub struct VhostStatus {
    /// vhost_dev n_mem_sections
    #[qapi(name = "n-mem-sections")]
    pub n_mem_sections: i64,
    /// vhost_dev n_tmp_sections
    #[qapi(name = "n-tmp-sections")]
    pub n_tmp_sections: i64,
    /// vhost_dev nvqs (number of virtqueues being used)
    #[qapi(name = "nvqs")]
    pub nvqs: u32,
    /// vhost_dev vq_index
    #[qapi(name = "vq-index")]
    pub vq_index: i64,
    /// vhost_dev features
    #[qapi(name = "features")]
    pub features: VirtioDeviceFeatures,
    /// vhost_dev acked_features
    #[qapi(name = "acked-features")]
    pub acked_features: VirtioDeviceFeatures,
    /// vhost_dev backend_features
    #[qapi(name = "backend-features")]
    pub backend_features: VirtioDeviceFeatures,
    /// vhost_dev protocol_features
    #[qapi(name = "protocol-features")]
    pub protocol_features: VhostDeviceProtocols,
    /// vhost_dev max_queues
    #[qapi(name = "max-queues")]
    pub max_queues: u64,
    /// vhost_dev backend_cap
    #[qapi(name = "backend-cap")]
    pub backend_cap: u64,
    /// vhost_dev log_enabled flag
    #[qapi(name = "log-enabled")]
    pub log_enabled: bool,
    /// vhost_dev log_size
    #[qapi(name = "log-size")]
    pub log_size: u64,
}
/// Full status of the virtio device with most VirtIODevice members.
/// Also includes the full status of the corresponding vhost device if
/// the vhost device is active.
#[qapi(name = "VirtioStatus")]
#[qapi(since = "7.2")]
pub struct VirtioStatus {
    /// VirtIODevice name
    #[qapi(name = "name")]
    pub name: String,
    /// VirtIODevice ID
    #[qapi(name = "device-id")]
    pub device_id: u16,
    /// VirtIODevice vhost_started flag
    #[qapi(name = "vhost-started")]
    pub vhost_started: bool,
    /// VirtIODevice device_endian
    #[qapi(name = "device-endian")]
    pub device_endian: String,
    /// VirtIODevice guest_features
    #[qapi(name = "guest-features")]
    pub guest_features: VirtioDeviceFeatures,
    /// VirtIODevice host_features
    #[qapi(name = "host-features")]
    pub host_features: VirtioDeviceFeatures,
    /// VirtIODevice backend_features
    #[qapi(name = "backend-features")]
    pub backend_features: VirtioDeviceFeatures,
    /// VirtIODevice virtqueue count.  This is the number of
    /// active virtqueues being used by the VirtIODevice.
    #[qapi(name = "num-vqs")]
    pub num_vqs: i64,
    /// VirtIODevice configuration status (VirtioDeviceStatus)
    #[qapi(name = "status")]
    pub status: VirtioDeviceStatus,
    /// VirtIODevice ISR
    #[qapi(name = "isr")]
    pub isr: u8,
    /// VirtIODevice queue_sel
    #[qapi(name = "queue-sel")]
    pub queue_sel: u16,
    /// VirtIODevice vm_running flag
    #[qapi(name = "vm-running")]
    pub vm_running: bool,
    /// VirtIODevice broken flag
    #[qapi(name = "broken")]
    pub broken: bool,
    /// VirtIODevice disabled flag
    #[qapi(name = "disabled")]
    pub disabled: bool,
    /// VirtIODevice use_started flag
    #[qapi(name = "use-started")]
    pub use_started: bool,
    /// VirtIODevice started flag
    #[qapi(name = "started")]
    pub started: bool,
    /// VirtIODevice start_on_kick flag
    #[qapi(name = "start-on-kick")]
    pub start_on_kick: bool,
    /// VirtIODevice disabled_legacy_check flag
    #[qapi(name = "disable-legacy-check")]
    pub disable_legacy_check: bool,
    /// VirtIODevice bus_name
    #[qapi(name = "bus-name")]
    pub bus_name: String,
    /// VirtIODevice use_guest_notifier_mask flag
    #[qapi(name = "use-guest-notifier-mask")]
    pub use_guest_notifier_mask: bool,
    /// Corresponding vhost device info for a given
    /// VirtIODevice.  Present if the given VirtIODevice has an active
    /// vhost device.
    #[qapi(name = "vhost-dev")]
    pub vhost_dev: Option<VhostStatus>,
}
/// Poll for a comprehensive status of a given virtio device
#[qapi(name = "x-query-virtio-status")]
#[qapi(feature = "unstable")]
#[qapi(since = "7.2")]
#[qapi(returns = "VirtioStatus")]
pub struct XQueryVirtioStatus {
    /// Canonical QOM path of the VirtIODevice
    #[qapi(name = "path")]
    pub path: String,
}
/// A structure defined to list the configuration statuses of a virtio
/// device
#[qapi(name = "VirtioDeviceStatus")]
#[qapi(since = "7.2")]
pub struct VirtioDeviceStatus {
    /// List of decoded configuration statuses of the virtio
    /// device
    #[qapi(name = "statuses")]
    pub statuses: Vec<String>,
    /// Virtio device statuses bitmap that have not been
    /// decoded
    #[qapi(name = "unknown-statuses")]
    pub unknown_statuses: Option<u8>,
}
/// A structure defined to list the vhost user protocol features of a
/// Vhost User device
#[qapi(name = "VhostDeviceProtocols")]
#[qapi(since = "7.2")]
pub struct VhostDeviceProtocols {
    /// List of decoded vhost user protocol features of a vhost
    /// user device
    #[qapi(name = "protocols")]
    pub protocols: Vec<String>,
    /// Vhost user device protocol features bitmap that
    /// have not been decoded
    #[qapi(name = "unknown-protocols")]
    pub unknown_protocols: Option<u64>,
}
/// The common fields that apply to most Virtio devices.  Some devices
/// may not have their own device-specific features (e.g. virtio-rng).
#[qapi(name = "VirtioDeviceFeatures")]
#[qapi(since = "7.2")]
pub struct VirtioDeviceFeatures {
    /// List of transport features of the virtio device
    #[qapi(name = "transports")]
    pub transports: Vec<String>,
    /// List of device-specific features (if the device has
    /// unique features)
    #[qapi(name = "dev-features")]
    pub dev_features: Option<Vec<String>>,
    /// Virtio device features bitmap that have not
    /// been decoded
    #[qapi(name = "unknown-dev-features")]
    pub unknown_dev_features: Option<u64>,
}
/// Information of a VirtIODevice VirtQueue, including most members of
/// the VirtQueue data structure.
#[qapi(name = "VirtQueueStatus")]
#[qapi(since = "7.2")]
pub struct VirtQueueStatus {
    /// Name of the VirtIODevice that uses this VirtQueue
    #[qapi(name = "name")]
    pub name: String,
    /// VirtQueue queue_index
    #[qapi(name = "queue-index")]
    pub queue_index: u16,
    /// VirtQueue inuse
    #[qapi(name = "inuse")]
    pub inuse: u32,
    /// VirtQueue vring.num
    #[qapi(name = "vring-num")]
    pub vring_num: u32,
    /// VirtQueue vring.num_default
    #[qapi(name = "vring-num-default")]
    pub vring_num_default: u32,
    /// VirtQueue vring.align
    #[qapi(name = "vring-align")]
    pub vring_align: u32,
    /// VirtQueue vring.desc (descriptor area)
    #[qapi(name = "vring-desc")]
    pub vring_desc: u64,
    /// VirtQueue vring.avail (driver area)
    #[qapi(name = "vring-avail")]
    pub vring_avail: u64,
    /// VirtQueue vring.used (device area)
    #[qapi(name = "vring-used")]
    pub vring_used: u64,
    /// VirtQueue last_avail_idx or return of vhost_dev
    /// vhost_get_vring_base (if vhost active)
    #[qapi(name = "last-avail-idx")]
    pub last_avail_idx: Option<u16>,
    /// VirtQueue shadow_avail_idx
    #[qapi(name = "shadow-avail-idx")]
    pub shadow_avail_idx: Option<u16>,
    /// VirtQueue used_idx
    #[qapi(name = "used-idx")]
    pub used_idx: u16,
    /// VirtQueue signalled_used
    #[qapi(name = "signalled-used")]
    pub signalled_used: u16,
    /// VirtQueue signalled_used_valid flag
    #[qapi(name = "signalled-used-valid")]
    pub signalled_used_valid: bool,
}
/// Return the status of a given VirtIODevice's VirtQueue
#[qapi(name = "x-query-virtio-queue-status")]
#[qapi(feature = "unstable")]
#[qapi(since = "7.2")]
#[qapi(returns = "VirtQueueStatus")]
pub struct XQueryVirtioQueueStatus {
    /// VirtIODevice canonical QOM path
    #[qapi(name = "path")]
    pub path: String,
    /// VirtQueue index to examine
    #[qapi(name = "queue")]
    pub queue: u16,
}
/// Information of a vhost device's vhost_virtqueue, including most
/// members of the vhost_dev vhost_virtqueue data structure.
#[qapi(name = "VirtVhostQueueStatus")]
#[qapi(since = "7.2")]
pub struct VirtVhostQueueStatus {
    /// Name of the VirtIODevice that uses this vhost_virtqueue
    #[qapi(name = "name")]
    pub name: String,
    /// vhost_virtqueue kick
    #[qapi(name = "kick")]
    pub kick: i64,
    /// vhost_virtqueue call
    #[qapi(name = "call")]
    pub call: i64,
    /// vhost_virtqueue desc
    #[qapi(name = "desc")]
    pub desc: u64,
    /// vhost_virtqueue avail
    #[qapi(name = "avail")]
    pub avail: u64,
    /// vhost_virtqueue used
    #[qapi(name = "used")]
    pub used: u64,
    /// vhost_virtqueue num
    #[qapi(name = "num")]
    pub num: i64,
    /// vhost_virtqueue desc_phys (descriptor area physical
    /// address)
    #[qapi(name = "desc-phys")]
    pub desc_phys: u64,
    /// vhost_virtqueue desc_size
    #[qapi(name = "desc-size")]
    pub desc_size: u32,
    /// vhost_virtqueue avail_phys (driver area physical
    /// address)
    #[qapi(name = "avail-phys")]
    pub avail_phys: u64,
    /// vhost_virtqueue avail_size
    #[qapi(name = "avail-size")]
    pub avail_size: u32,
    /// vhost_virtqueue used_phys (device area physical address)
    #[qapi(name = "used-phys")]
    pub used_phys: u64,
    /// vhost_virtqueue used_size
    #[qapi(name = "used-size")]
    pub used_size: u32,
}
/// Return information of a given vhost device's vhost_virtqueue
#[qapi(name = "x-query-virtio-vhost-queue-status")]
#[qapi(feature = "unstable")]
#[qapi(since = "7.2")]
#[qapi(returns = "VirtVhostQueueStatus")]
pub struct XQueryVirtioVhostQueueStatus {
    /// VirtIODevice canonical QOM path
    #[qapi(name = "path")]
    pub path: String,
    /// vhost_virtqueue index to examine
    #[qapi(name = "queue")]
    pub queue: u16,
}
/// Information regarding the vring descriptor area
#[qapi(name = "VirtioRingDesc")]
#[qapi(since = "7.2")]
pub struct VirtioRingDesc {
    /// Guest physical address of the descriptor area
    #[qapi(name = "addr")]
    pub addr: u64,
    /// Length of the descriptor area
    #[qapi(name = "len")]
    pub len: u32,
    /// List of descriptor flags
    #[qapi(name = "flags")]
    pub flags: Vec<String>,
}
/// Information regarding the avail vring (a.k.a. driver area)
#[qapi(name = "VirtioRingAvail")]
#[qapi(since = "7.2")]
pub struct VirtioRingAvail {
    /// VRingAvail flags
    #[qapi(name = "flags")]
    pub flags: u16,
    /// VRingAvail index
    #[qapi(name = "idx")]
    pub idx: u16,
    /// VRingAvail ring[] entry at provided index
    #[qapi(name = "ring")]
    pub ring: u16,
}
/// Information regarding the used vring (a.k.a. device area)
#[qapi(name = "VirtioRingUsed")]
#[qapi(since = "7.2")]
pub struct VirtioRingUsed {
    /// VRingUsed flags
    #[qapi(name = "flags")]
    pub flags: u16,
    /// VRingUsed index
    #[qapi(name = "idx")]
    pub idx: u16,
}
/// Information regarding a VirtQueue's VirtQueueElement including
/// descriptor, driver, and device areas
#[qapi(name = "VirtioQueueElement")]
#[qapi(since = "7.2")]
pub struct VirtioQueueElement {
    /// Name of the VirtIODevice that uses this VirtQueue
    #[qapi(name = "name")]
    pub name: String,
    /// Index of the element in the queue
    #[qapi(name = "index")]
    pub index: u32,
    /// List of descriptors (VirtioRingDesc)
    #[qapi(name = "descs")]
    pub descs: Vec<VirtioRingDesc>,
    /// VRingAvail info
    #[qapi(name = "avail")]
    pub avail: VirtioRingAvail,
    /// VRingUsed info
    #[qapi(name = "used")]
    pub used: VirtioRingUsed,
}
/// Return the information about a VirtQueue's VirtQueueElement
#[qapi(name = "x-query-virtio-queue-element")]
#[qapi(feature = "unstable")]
#[qapi(since = "7.2")]
#[qapi(returns = "VirtioQueueElement")]
pub struct XQueryVirtioQueueElement {
    /// VirtIODevice canonical QOM path
    #[qapi(name = "path")]
    pub path: String,
    /// VirtQueue index to examine
    #[qapi(name = "queue")]
    pub queue: u16,
    /// Index of the element in the queue (default: head of the
    /// queue)
    #[qapi(name = "index")]
    pub index: Option<u16>,
}
/// Describes the subset of virtqueues assigned to an IOThread.
#[qapi(name = "IOThreadVirtQueueMapping")]
#[qapi(since = "9.0")]
pub struct IoThreadVirtQueueMapping {
    /// the id of IOThread object
    #[qapi(name = "iothread")]
    pub iothread: String,
    /// an optional array of virtqueue indices that will be handled by
    /// this IOThread.  When absent, virtqueues are assigned round-robin
    /// across all IOThreadVirtQueueMappings provided.  Either all
    /// IOThreadVirtQueueMappings must have @vqs or none of them must
    /// have it.
    #[qapi(name = "vqs")]
    pub vqs: Option<Vec<u16>>,
}
/// Not used by QMP; hack to let us use IOThreadVirtQueueMappingList
/// internally
#[qapi(name = "DummyVirtioForceArrays")]
#[qapi(since = "9.0")]
pub struct DummyVirtioForceArrays {
    #[qapi(name = "unused-iothread-vq-mapping")]
    pub unused_iothread_vq_mapping: Vec<IoThreadVirtQueueMapping>,
}
#[qapi(name = "GranuleMode")]
#[qapi(since = "9.0")]
pub enum GranuleMode {
    /// granule page size of 4KiB
    #[qapi(name = "4k")]
    _4k,
    /// granule page size of 8KiB
    #[qapi(name = "8k")]
    _8k,
    /// granule page size of 16KiB
    #[qapi(name = "16k")]
    _16k,
    /// granule page size of 64KiB
    #[qapi(name = "64k")]
    _64k,
    /// granule matches the host page size
    #[qapi(name = "host")]
    Host,
}
// path end:	qapi/virtio.json
// path begin:	qapi/vfio.json
/// An enumeration of the VFIO device migration states.
#[qapi(name = "VfioMigrationState")]
#[qapi(since = "9.1")]
pub enum VfioMigrationState {
    /// The device is stopped.
    #[qapi(name = "stop")]
    Stop,
    /// The device is running.
    #[qapi(name = "running")]
    Running,
    /// The device is stopped and its internal state is
    /// available for reading.
    #[qapi(name = "stop-copy")]
    StopCopy,
    /// The device is stopped and its internal state is available
    /// for writing.
    #[qapi(name = "resuming")]
    Resuming,
    /// The device is running in the P2P quiescent state.
    #[qapi(name = "running-p2p")]
    RunningP2p,
    /// The device is running, tracking its internal state and
    /// its internal state is available for reading.
    #[qapi(name = "pre-copy")]
    PreCopy,
    /// The device is running in the P2P quiescent state,
    /// tracking its internal state and its internal state is available
    /// for reading.
    #[qapi(name = "pre-copy-p2p")]
    PreCopyP2p,
}
/// This event is emitted when a VFIO device migration state is changed.
#[qapi(name = "VFIO_MIGRATION")]
#[qapi(since = "9.1")]
pub struct VfioMigration {
    /// The device's id, if it has one.
    #[qapi(name = "device-id")]
    pub device_id: String,
    /// The device's QOM path.
    #[qapi(name = "qom-path")]
    pub qom_path: String,
    /// The new changed device migration state.
    #[qapi(name = "device-state")]
    pub device_state: VfioMigrationState,
}
// path end:	qapi/vfio.json
// path begin:	qapi/cryptodev.json
/// The supported algorithm types of a crypto device.
#[qapi(name = "QCryptodevBackendAlgType")]
#[qapi(since = "8.0")]
pub enum QCryptodevBackendAlgType {
    /// symmetric encryption
    #[qapi(name = "sym")]
    Sym,
    /// asymmetric Encryption
    #[qapi(name = "asym")]
    Asym,
}
/// The supported service types of a crypto device.
#[qapi(name = "QCryptodevBackendServiceType")]
#[qapi(since = "8.0")]
pub enum QCryptodevBackendServiceType {
    #[qapi(name = "cipher")]
    Cipher,
    #[qapi(name = "hash")]
    Hash,
    #[qapi(name = "mac")]
    Mac,
    #[qapi(name = "aead")]
    Aead,
    #[qapi(name = "akcipher")]
    Akcipher,
}
/// The crypto device backend type
#[qapi(name = "QCryptodevBackendType")]
#[qapi(since = "8.0")]
pub enum QCryptodevBackendType {
    /// the QEMU builtin support
    #[qapi(name = "builtin")]
    Builtin,
    /// vhost-user
    #[qapi(name = "vhost-user")]
    VhostUser,
    /// Linux kernel cryptographic framework
    #[qapi(name = "lkcf")]
    Lkcf,
}
/// Information about a queue of crypto device.
#[qapi(name = "QCryptodevBackendClient")]
#[qapi(since = "8.0")]
pub struct QCryptodevBackendClient {
    /// the queue index of the crypto device
    #[qapi(name = "queue")]
    pub queue: u32,
    /// the type of the crypto device
    #[qapi(name = "type")]
    pub r#type: QCryptodevBackendType,
}
/// Information about a crypto device.
#[qapi(name = "QCryptodevInfo")]
#[qapi(since = "8.0")]
pub struct QCryptodevInfo {
    /// the id of the crypto device
    #[qapi(name = "id")]
    pub id: String,
    /// supported service types of a crypto device
    #[qapi(name = "service")]
    pub service: Vec<QCryptodevBackendServiceType>,
    /// the additional information of the crypto device
    #[qapi(name = "client")]
    pub client: Vec<QCryptodevBackendClient>,
}
/// Returns information about current crypto devices.
#[qapi(name = "query-cryptodev")]
#[qapi(since = "8.0")]
#[qapi(returns = "Vec<QCryptodevInfo>")]
pub struct QueryCryptodev {}
// path end:	qapi/cryptodev.json
// path begin:	qapi/cxl.json
/// CXL has a number of separate event logs for different types of
/// events.  Each such event log is handled and signaled independently.
#[qapi(name = "CxlEventLog")]
#[qapi(since = "8.1")]
pub enum CxlEventLog {
    /// Information Event Log
    #[qapi(name = "informational")]
    Informational,
    /// Warning Event Log
    #[qapi(name = "warning")]
    Warning,
    /// Failure Event Log
    #[qapi(name = "failure")]
    Failure,
    /// Fatal Event Log
    #[qapi(name = "fatal")]
    Fatal,
}
/// Inject an event record for a General Media Event (CXL r3.0
/// 8.2.9.2.1.1).  This event type is reported via one of the event logs
/// specified via the log parameter.
#[qapi(name = "cxl-inject-general-media-event")]
#[qapi(since = "8.1")]
#[qapi(returns = "()")]
pub struct CxlInjectGeneralMediaEvent {
    /// CXL type 3 device canonical QOM path
    #[qapi(name = "path")]
    pub path: String,
    /// event log to add the event to
    #[qapi(name = "log")]
    pub log: CxlEventLog,
    /// Event Record Flags.  See CXL r3.0 Table 8-42 Common Event
    /// Record Format, Event Record Flags for subfield definitions.
    #[qapi(name = "flags")]
    pub flags: u8,
    /// Device Physical Address (relative to @path device).  Note
    /// lower bits include some flags.  See CXL r3.0 Table 8-43 General
    /// Media Event Record, Physical Address.
    #[qapi(name = "dpa")]
    pub dpa: u64,
    /// Memory Event Descriptor with additional memory event
    /// information.  See CXL r3.0 Table 8-43 General Media Event
    /// Record, Memory Event Descriptor for bit definitions.
    #[qapi(name = "descriptor")]
    pub descriptor: u8,
    /// Type of memory event that occurred.  See CXL r3.0 Table 8-43
    /// General Media Event Record, Memory Event Type for possible
    /// values.
    #[qapi(name = "type")]
    pub r#type: u8,
    /// Type of first transaction that caused the event
    /// to occur.  See CXL r3.0 Table 8-43 General Media Event Record,
    /// Transaction Type for possible values.
    #[qapi(name = "transaction-type")]
    pub transaction_type: u8,
    /// The channel of the memory event location.  A channel is an
    /// interface that can be independently accessed for a transaction.
    #[qapi(name = "channel")]
    pub channel: Option<u8>,
    /// The rank of the memory event location.  A rank is a set of
    /// memory devices on a channel that together execute a transaction.
    #[qapi(name = "rank")]
    pub rank: Option<u8>,
    /// Bitmask that represents all devices in the rank associated
    /// with the memory event location.
    #[qapi(name = "device")]
    pub device: Option<u32>,
    /// Device specific component identifier for the event.
    /// May describe a field replaceable sub-component of the device.
    #[qapi(name = "component-id")]
    pub component_id: Option<String>,
}
/// Inject an event record for a DRAM Event (CXL r3.0 8.2.9.2.1.2).
/// This event type is reported via one of the event logs specified via
/// the log parameter.
#[qapi(name = "cxl-inject-dram-event")]
#[qapi(since = "8.1")]
#[qapi(returns = "()")]
pub struct CxlInjectDramEvent {
    /// CXL type 3 device canonical QOM path
    #[qapi(name = "path")]
    pub path: String,
    /// Event log to add the event to
    #[qapi(name = "log")]
    pub log: CxlEventLog,
    /// Event Record Flags.  See CXL r3.0 Table 8-42 Common Event
    /// Record Format, Event Record Flags for subfield definitions.
    #[qapi(name = "flags")]
    pub flags: u8,
    /// Device Physical Address (relative to @path device).  Note
    /// lower bits include some flags.  See CXL r3.0 Table 8-44 DRAM
    /// Event Record, Physical Address.
    #[qapi(name = "dpa")]
    pub dpa: u64,
    /// Memory Event Descriptor with additional memory event
    /// information.  See CXL r3.0 Table 8-44 DRAM Event Record, Memory
    /// Event Descriptor for bit definitions.
    #[qapi(name = "descriptor")]
    pub descriptor: u8,
    /// Type of memory event that occurred.  See CXL r3.0 Table 8-44
    /// DRAM Event Record, Memory Event Type for possible values.
    #[qapi(name = "type")]
    pub r#type: u8,
    /// Type of first transaction that caused the event
    /// to occur.  See CXL r3.0 Table 8-44 DRAM Event Record,
    /// Transaction Type for possible values.
    #[qapi(name = "transaction-type")]
    pub transaction_type: u8,
    /// The channel of the memory event location.  A channel is an
    /// interface that can be independently accessed for a transaction.
    #[qapi(name = "channel")]
    pub channel: Option<u8>,
    /// The rank of the memory event location.  A rank is a set of
    /// memory devices on a channel that together execute a transaction.
    #[qapi(name = "rank")]
    pub rank: Option<u8>,
    /// Identifies one or more nibbles that the error affects
    #[qapi(name = "nibble-mask")]
    pub nibble_mask: Option<u32>,
    /// Bank group of the memory event location, incorporating
    /// a number of Banks.
    #[qapi(name = "bank-group")]
    pub bank_group: Option<u8>,
    /// Bank of the memory event location.  A single bank is accessed
    /// per read or write of the memory.
    #[qapi(name = "bank")]
    pub bank: Option<u8>,
    /// Row address within the DRAM.
    #[qapi(name = "row")]
    pub row: Option<u32>,
    /// Column address within the DRAM.
    #[qapi(name = "column")]
    pub column: Option<u16>,
    /// Bits within each nibble.  Used in order of bits
    /// set in the nibble-mask.  Up to 4 nibbles may be covered.
    #[qapi(name = "correction-mask")]
    pub correction_mask: Option<Vec<u64>>,
}
/// Inject an event record for a Memory Module Event (CXL r3.0
/// 8.2.9.2.1.3).  This event includes a copy of the Device Health info
/// at the time of the event.
#[qapi(name = "cxl-inject-memory-module-event")]
#[qapi(since = "8.1")]
#[qapi(returns = "()")]
pub struct CxlInjectMemoryModuleEvent {
    /// CXL type 3 device canonical QOM path
    #[qapi(name = "path")]
    pub path: String,
    /// Event Log to add the event to
    #[qapi(name = "log")]
    pub log: CxlEventLog,
    /// Event Record Flags.  See CXL r3.0 Table 8-42 Common Event
    /// Record Format, Event Record Flags for subfield definitions.
    #[qapi(name = "flags")]
    pub flags: u8,
    /// Device Event Type.  See CXL r3.0 Table 8-45 Memory Module
    /// Event Record for bit definitions for bit definiions.
    #[qapi(name = "type")]
    pub r#type: u8,
    /// Overall health summary bitmap.  See CXL r3.0 Table
    /// 8-100 Get Health Info Output Payload, Health Status for bit
    /// definitions.
    #[qapi(name = "health-status")]
    pub health_status: u8,
    /// Overall media health summary.  See CXL r3.0 Table
    /// 8-100 Get Health Info Output Payload, Media Status for bit
    /// definitions.
    #[qapi(name = "media-status")]
    pub media_status: u8,
    /// See CXL r3.0 Table 8-100 Get Health Info Output
    /// Payload, Additional Status for subfield definitions.
    #[qapi(name = "additional-status")]
    pub additional_status: u8,
    /// Percentage (0-100) of factory expected life span.
    #[qapi(name = "life-used")]
    pub life_used: u8,
    /// Device temperature in degrees Celsius.
    #[qapi(name = "temperature")]
    pub temperature: i16,
    /// Number of times the device has been unable to
    /// determine whether data loss may have occurred.
    #[qapi(name = "dirty-shutdown-count")]
    pub dirty_shutdown_count: u32,
    /// Total number of correctable errors
    /// in volatile memory.
    #[qapi(name = "corrected-volatile-error-count")]
    pub corrected_volatile_error_count: u32,
    /// Total number of correctable
    /// errors in persistent memory
    #[qapi(name = "corrected-persistent-error-count")]
    pub corrected_persistent_error_count: u32,
}
/// Poison records indicate that a CXL memory device knows that a
/// particular memory region may be corrupted.  This may be because of
/// locally detected errors (e.g. ECC failure) or poisoned writes
/// received from other components in the system.  This injection
/// mechanism enables testing of the OS handling of poison records which
/// may be queried via the CXL mailbox.
#[qapi(name = "cxl-inject-poison")]
#[qapi(since = "8.1")]
#[qapi(returns = "()")]
pub struct CxlInjectPoison {
    /// CXL type 3 device canonical QOM path
    #[qapi(name = "path")]
    pub path: String,
    /// Start address; must be 64 byte aligned.
    #[qapi(name = "start")]
    pub start: u64,
    /// Length of poison to inject; must be a multiple of 64 bytes.
    #[qapi(name = "length")]
    pub length: u64,
}
/// Type of uncorrectable CXL error to inject.  These errors are
/// reported via an AER uncorrectable internal error with additional
/// information logged at the CXL device.
#[qapi(name = "CxlUncorErrorType")]
#[qapi(since = "8.0")]
pub enum CxlUncorErrorType {
    /// Data error such as data parity or data ECC error
    /// CXL.cache
    #[qapi(name = "cache-data-parity")]
    CacheDataParity,
    /// Address parity or other errors associated
    /// with the address field on CXL.cache
    #[qapi(name = "cache-address-parity")]
    CacheAddressParity,
    /// Byte enable parity or other byte enable errors on
    /// CXL.cache
    #[qapi(name = "cache-be-parity")]
    CacheBeParity,
    /// ECC error on CXL.cache
    #[qapi(name = "cache-data-ecc")]
    CacheDataEcc,
    /// Data error such as data parity or data ECC error
    /// on CXL.mem
    #[qapi(name = "mem-data-parity")]
    MemDataParity,
    /// Address parity or other errors associated with
    /// the address field on CXL.mem
    #[qapi(name = "mem-address-parity")]
    MemAddressParity,
    /// Byte enable parity or other byte enable errors on
    /// CXL.mem.
    #[qapi(name = "mem-be-parity")]
    MemBeParity,
    /// Data ECC error on CXL.mem.
    #[qapi(name = "mem-data-ecc")]
    MemDataEcc,
    /// REINIT threshold hit.
    #[qapi(name = "reinit-threshold")]
    ReinitThreshold,
    /// Received unrecognized encoding.
    #[qapi(name = "rsvd-encoding")]
    RsvdEncoding,
    /// Received poison from the peer.
    #[qapi(name = "poison-received")]
    PoisonReceived,
    /// Buffer overflows (first 3 bits of header log
    /// indicate which)
    #[qapi(name = "receiver-overflow")]
    ReceiverOverflow,
    /// Component specific error
    #[qapi(name = "internal")]
    Internal,
    /// Integrity and data encryption tx error.
    #[qapi(name = "cxl-ide-tx")]
    CxlIdeTx,
    /// Integrity and data encryption rx error.
    #[qapi(name = "cxl-ide-rx")]
    CxlIdeRx,
}
/// Record of a single error including header log.
#[qapi(name = "CXLUncorErrorRecord")]
#[qapi(since = "8.0")]
pub struct CxlUncorErrorRecord {
    /// Type of error
    #[qapi(name = "type")]
    pub r#type: CxlUncorErrorType,
    /// 16 DWORD of header.
    #[qapi(name = "header")]
    pub header: Vec<u32>,
}
/// Command to allow injection of multiple errors in one go.  This
/// allows testing of multiple header log handling in the OS.
#[qapi(name = "cxl-inject-uncorrectable-errors")]
#[qapi(since = "8.0")]
#[qapi(returns = "()")]
pub struct CxlInjectUncorrectableErrors {
    /// CXL Type 3 device canonical QOM path
    #[qapi(name = "path")]
    pub path: String,
    /// Errors to inject
    #[qapi(name = "errors")]
    pub errors: Vec<CxlUncorErrorRecord>,
}
/// Type of CXL correctable error to inject
#[qapi(name = "CxlCorErrorType")]
#[qapi(since = "8.0")]
pub enum CxlCorErrorType {
    /// Data ECC error on CXL.cache
    #[qapi(name = "cache-data-ecc")]
    CacheDataEcc,
    /// Data ECC error on CXL.mem
    #[qapi(name = "mem-data-ecc")]
    MemDataEcc,
    /// Component specific and applicable to 68 byte Flit
    /// mode only.
    #[qapi(name = "crc-threshold")]
    CrcThreshold,
    #[qapi(name = "retry-threshold")]
    RetryThreshold,
    /// Received poison from a peer on CXL.cache.
    #[qapi(name = "cache-poison-received")]
    CachePoisonReceived,
    /// Received poison from a peer on CXL.mem
    #[qapi(name = "mem-poison-received")]
    MemPoisonReceived,
    /// Received error indication from the physical layer.
    #[qapi(name = "physical")]
    Physical,
}
/// Command to inject a single correctable error.  Multiple error
/// injection of this error type is not interesting as there is no
/// associated header log.  These errors are reported via AER as a
/// correctable internal error, with additional detail available from
/// the CXL device.
#[qapi(name = "cxl-inject-correctable-error")]
#[qapi(since = "8.0")]
#[qapi(returns = "()")]
pub struct CxlInjectCorrectableError {
    /// CXL Type 3 device canonical QOM path
    #[qapi(name = "path")]
    pub path: String,
    /// Type of error.
    #[qapi(name = "type")]
    pub r#type: CxlCorErrorType,
}
/// A single dynamic capacity extent.  This is a contiguous allocation
/// of memory by Device Physical Address within a single Dynamic
/// Capacity Region on a CXL Type 3 Device.
#[qapi(name = "CxlDynamicCapacityExtent")]
#[qapi(since = "9.1")]
pub struct CxlDynamicCapacityExtent {
    /// The offset (in bytes) to the start of the region where the
    /// extent belongs to.
    #[qapi(name = "offset")]
    pub offset: u64,
    /// The length of the extent in bytes.
    #[qapi(name = "len")]
    pub len: u64,
}
/// The policy to use for selecting which extents comprise the added
/// capacity, as defined in Compute Express Link (CXL) Specification,
/// Revision 3.1, Table 7-70.
#[qapi(name = "CxlExtentSelectionPolicy")]
#[qapi(since = "9.1")]
pub enum CxlExtentSelectionPolicy {
    /// Device is responsible for allocating the requested memory
    /// capacity and is free to do this using any combination of
    /// supported extents.
    #[qapi(name = "free")]
    Free,
    /// Device is responsible for allocating the requested
    /// memory capacity but must do so as a single contiguous
    /// extent.
    #[qapi(name = "contiguous")]
    Contiguous,
    /// The precise set of extents to be allocated is
    /// specified by the command.  Thus allocation is being managed
    /// by the issuer of the allocation command, not the device.
    #[qapi(name = "prescriptive")]
    Prescriptive,
    /// Capacity has already been allocated to a
    /// different host using free, contiguous or prescriptive policy
    /// with a known tag.  This policy then instructs the device to make
    /// the capacity with the specified tag available to an additional
    /// host.  Capacity is implicit as it matches that already
    /// associated with the tag.  Note that the extent list (and hence
    /// Device Physical Addresses) used are per host, so a device may
    /// use different representations on each host.  The ordering of the
    /// extents provided to each host is indicated to the host using per
    /// extent sequence numbers generated by the device.  Has a similar
    /// meaning for temporal sharing, but in that case there may be only
    /// one host involved.
    #[qapi(name = "enable-shared-access")]
    EnableSharedAccess,
}
/// Initiate adding dynamic capacity extents to a host.  This simulates
/// operations defined in Compute Express Link (CXL) Specification,
/// Revision 3.1, Section 7.6.7.6.5.  Note that, currently, establishing
/// success or failure of the full Add Dynamic Capacity flow requires
/// out of band communication with the OS of the CXL host.
#[qapi(name = "cxl-add-dynamic-capacity")]
#[qapi(feature = "unstable")]
#[qapi(since = "9.1")]
#[qapi(returns = "()")]
pub struct CxlAddDynamicCapacity {
    /// path to the CXL Dynamic Capacity Device in the QOM tree.
    #[qapi(name = "path")]
    pub path: String,
    /// The "Host ID" field as defined in Compute Express Link
    /// (CXL) Specification, Revision 3.1, Table 7-70.
    #[qapi(name = "host-id")]
    pub host_id: u16,
    /// The "Selection Policy" bits as defined in
    /// Compute Express Link (CXL) Specification, Revision 3.1,
    /// Table 7-70.  It specifies the policy to use for selecting
    /// which extents comprise the added capacity.
    #[qapi(name = "selection-policy")]
    pub selection_policy: CxlExtentSelectionPolicy,
    /// The "Region Number" field as defined in Compute Express
    /// Link (CXL) Specification, Revision 3.1, Table 7-70.  Valid
    /// range is from 0-7.
    #[qapi(name = "region")]
    pub region: u8,
    /// The "Tag" field as defined in Compute Express Link (CXL)
    /// Specification, Revision 3.1, Table 7-70.
    #[qapi(name = "tag")]
    pub tag: Option<String>,
    /// The "Extent List" field as defined in Compute Express Link
    /// (CXL) Specification, Revision 3.1, Table 7-70.
    #[qapi(name = "extents")]
    pub extents: Vec<CxlDynamicCapacityExtent>,
}
/// The policy to use for selecting which extents comprise the released
/// capacity, defined in the "Flags" field in Compute Express Link (CXL)
/// Specification, Revision 3.1, Table 7-71.
#[qapi(name = "CxlExtentRemovalPolicy")]
#[qapi(since = "9.1")]
pub enum CxlExtentRemovalPolicy {
    /// Extents are selected by the device based on tag, with
    /// no requirement for contiguous extents.
    #[qapi(name = "tag-based")]
    TagBased,
    /// Extent list of capacity to release is included in
    /// the request payload.
    #[qapi(name = "prescriptive")]
    Prescriptive,
}
/// Initiate release of dynamic capacity extents from a host.  This
/// simulates operations defined in Compute Express Link (CXL)
/// Specification, Revision 3.1, Section 7.6.7.6.6.  Note that,
/// currently, success or failure of the full Release Dynamic Capacity
/// flow requires out of band communication with the OS of the CXL host.
#[qapi(name = "cxl-release-dynamic-capacity")]
#[qapi(feature = "unstable")]
#[qapi(since = "9.1")]
#[qapi(returns = "()")]
pub struct CxlReleaseDynamicCapacity {
    /// path to the CXL Dynamic Capacity Device in the QOM tree.
    #[qapi(name = "path")]
    pub path: String,
    /// The "Host ID" field as defined in Compute Express Link
    /// (CXL) Specification, Revision 3.1, Table 7-71.
    #[qapi(name = "host-id")]
    pub host_id: u16,
    /// Bit[3:0] of the "Flags" field as defined in
    /// Compute Express Link (CXL) Specification, Revision 3.1,
    /// Table 7-71.
    #[qapi(name = "removal-policy")]
    pub removal_policy: CxlExtentRemovalPolicy,
    /// Bit[4] of the "Flags" field in Compute Express
    /// Link (CXL) Specification, Revision 3.1, Table 7-71.  When set,
    /// the device does not wait for a Release Dynamic Capacity command
    /// from the host.  Instead, the host immediately looses access to
    /// the released capacity.
    #[qapi(name = "forced-removal")]
    pub forced_removal: Option<bool>,
    /// Bit[5] of the "Flags" field in Compute Express
    /// Link (CXL) Specification, Revision 3.1, Table 7-71.  When set,
    /// the device should sanitize all released capacity as a result of
    /// this request.  This ensures that all user data and metadata is
    /// made permanently unavailable by whatever means is appropriate
    /// for the media type.  Note that changing encryption keys is not
    /// sufficient.
    #[qapi(name = "sanitize-on-release")]
    pub sanitize_on_release: Option<bool>,
    /// The "Region Number" field as defined in Compute Express
    /// Link Specification, Revision 3.1, Table 7-71.  Valid range
    /// is from 0-7.
    #[qapi(name = "region")]
    pub region: u8,
    /// The "Tag" field as defined in Compute Express Link (CXL)
    /// Specification, Revision 3.1, Table 7-71.
    #[qapi(name = "tag")]
    pub tag: Option<String>,
    /// The "Extent List" field as defined in Compute Express
    /// Link (CXL) Specification, Revision 3.1, Table 7-71.
    #[qapi(name = "extents")]
    pub extents: Vec<CxlDynamicCapacityExtent>,
}
// path end:	qapi/cxl.json
// path begin:	qapi/qapi-schema.json
// path end:	qapi/qapi-schema.json

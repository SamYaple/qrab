#![allow(unused_imports, non_camel_case_types, dead_code)]
use crate as qapi;
use qapi_macros::qapi;
use std::collections::HashMap;
// path begin:	qapi/pragma.json
// path end:	qapi/pragma.json
// path begin:	qapi/error.json
/**QEMU error classes
*/
#[qapi(since = "1.2")]
pub enum QapiErrorClass {
    /**this is used for errors that don't require a specific
error class.  This should be the default case for most errors
*/
    GenericError,
    /**the requested command has not been found
*/
    CommandNotFound,
    /**a device has failed to be become active
*/
    DeviceNotActive,
    /**the requested device has not been found
*/
    DeviceNotFound,
    /**the requested operation can't be fulfilled because a
required KVM capability is missing
*/
    #[qapi(name = "KVMMissingCap")]
    KvmMissingCap,
}
// path end:	qapi/error.json
// path begin:	qapi/common.json
/**An enumeration of the I/O operation types
*/
#[qapi(since = "2.1")]
pub enum IoOperationType {
    /**read operation
*/
    #[qapi(name = "read")]
    Read,
    /**write operation
*/
    #[qapi(name = "write")]
    Write,
}
/**An enumeration of three options: on, off, and auto
*/
#[qapi(since = "2.2")]
pub enum OnOffAuto {
    /**QEMU selects the value between on and off
*/
    #[qapi(name = "auto")]
    Auto,
    /**Enabled
*/
    #[qapi(name = "on")]
    On,
    /**Disabled
*/
    #[qapi(name = "off")]
    Off,
}
/**An enumeration of three values: on, off, and split
*/
#[qapi(since = "2.6")]
pub enum OnOffSplit {
    /**Enabled
*/
    #[qapi(name = "on")]
    On,
    /**Disabled
*/
    #[qapi(name = "off")]
    Off,
    /**Mixed
*/
    #[qapi(name = "split")]
    Split,
}
/**This is a string value or the explicit lack of a string (null
pointer in C).  Intended for cases when 'optional absent' already
has a different meaning.
*/
#[qapi(since = "2.10")]
pub enum StrOrNull {
    /**the string value
*/
    #[qapi(name = "s")]
    S(String),
    /**no string value
*/
    #[qapi(name = "n")]
    N(()),
}
/**An enumeration of options for specifying a PCI BAR
*/
#[qapi(name = "OffAutoPCIBAR")]
#[qapi(since = "2.12")]
pub enum OffAutoPcibar {
    /**The specified feature is disabled
*/
    #[qapi(name = "off")]
    Off,
    /**The PCI BAR for the feature is automatically selected
*/
    #[qapi(name = "auto")]
    Auto,
    /**PCI BAR0 is used for the feature
*/
    #[qapi(name = "bar0")]
    Bar0,
    /**PCI BAR1 is used for the feature
*/
    #[qapi(name = "bar1")]
    Bar1,
    /**PCI BAR2 is used for the feature
*/
    #[qapi(name = "bar2")]
    Bar2,
    /**PCI BAR3 is used for the feature
*/
    #[qapi(name = "bar3")]
    Bar3,
    /**PCI BAR4 is used for the feature
*/
    #[qapi(name = "bar4")]
    Bar4,
    /**PCI BAR5 is used for the feature
*/
    #[qapi(name = "bar5")]
    Bar5,
}
/**An enumeration of PCIe link speeds in units of GT/s
*/
#[qapi(name = "PCIELinkSpeed")]
#[qapi(since = "4.0")]
pub enum PcieLinkSpeed {
    /**2.5GT/s
*/
    #[qapi(name = "2_5")]
    _25,
    /**5.0GT/s
*/
    #[qapi(name = "5")]
    _5,
    /**8.0GT/s
*/
    #[qapi(name = "8")]
    _8,
    /**16.0GT/s
*/
    #[qapi(name = "16")]
    _16,
    /**32.0GT/s (since 9.0)
*/
    #[qapi(name = "32")]
    #[qapi(since = "9.0")]
    _32,
    /**64.0GT/s (since 9.0)
*/
    #[qapi(name = "64")]
    #[qapi(since = "9.0")]
    _64,
}
/**An enumeration of PCIe link width
*/
#[qapi(name = "PCIELinkWidth")]
#[qapi(since = "4.0")]
pub enum PcieLinkWidth {
    /**x1
*/
    #[qapi(name = "1")]
    _1,
    /**x2
*/
    #[qapi(name = "2")]
    _2,
    /**x4
*/
    #[qapi(name = "4")]
    _4,
    /**x8
*/
    #[qapi(name = "8")]
    _8,
    /**x12
*/
    #[qapi(name = "12")]
    _12,
    /**x16
*/
    #[qapi(name = "16")]
    _16,
    /**x32
*/
    #[qapi(name = "32")]
    _32,
}
/**Host memory policy types
*/
#[qapi(since = "2.1")]
pub enum HostMemPolicy {
    /**restore default policy, remove any nondefault policy
*/
    #[qapi(name = "default")]
    Default,
    /**set the preferred host nodes for allocation
*/
    #[qapi(name = "preferred")]
    Preferred,
    /**a strict policy that restricts memory allocation to the host
nodes specified
*/
    #[qapi(name = "bind")]
    Bind,
    /**memory allocations are interleaved across the set of
host nodes specified
*/
    #[qapi(name = "interleave")]
    Interleave,
}
/**Indicates whether a netfilter is attached to a netdev's transmit
queue or receive queue or both.
*/
#[qapi(since = "2.5")]
pub enum NetFilterDirection {
    /**the filter is attached both to the receive and the transmit
queue of the netdev (default).
*/
    #[qapi(name = "all")]
    All,
    /**the filter is attached to the receive queue of the netdev,
where it will receive packets sent to the netdev.
*/
    #[qapi(name = "rx")]
    Rx,
    /**the filter is attached to the transmit queue of the netdev,
where it will receive packets sent by the netdev.
*/
    #[qapi(name = "tx")]
    Tx,
}
/**Keys to toggle input-linux between host and guest.
*/
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
#[qapi(since = "6.2")]
pub struct HumanReadableText {
    /**Formatted output intended for humans.
*/
    #[qapi(name = "human-readable-text")]
    pub human_readable_text: String,
}
// path end:	qapi/common.json
// path begin:	qapi/run-state.json
/**An enumeration of VM run states.
*/
#[qapi]
pub enum RunState {
    /**QEMU is running on a debugger
*/
    #[qapi(name = "debug")]
    Debug,
    /**guest is paused waiting for an incoming migration.  Note
that this state does not tell whether the machine will start at
the end of the migration.  This depends on the command-line -S
option and any invocation of 'stop' or 'cont' that has happened
since QEMU was started.
*/
    #[qapi(name = "inmigrate")]
    Inmigrate,
    /**An internal error that prevents further guest
execution has occurred
*/
    #[qapi(name = "internal-error")]
    InternalError,
    /**the last IOP has failed and the device is configured to
pause on I/O errors
*/
    #[qapi(name = "io-error")]
    IoError,
    /**guest has been paused via the 'stop' command
*/
    #[qapi(name = "paused")]
    Paused,
    /**guest is paused following a successful 'migrate'
*/
    #[qapi(name = "postmigrate")]
    Postmigrate,
    /**QEMU was started with -S and guest has not started
*/
    #[qapi(name = "prelaunch")]
    Prelaunch,
    /**guest is paused to finish the migration process
*/
    #[qapi(name = "finish-migrate")]
    FinishMigrate,
    /**guest is paused to restore VM state
*/
    #[qapi(name = "restore-vm")]
    RestoreVm,
    /**guest is actively running
*/
    #[qapi(name = "running")]
    Running,
    /**guest is paused to save the VM state
*/
    #[qapi(name = "save-vm")]
    SaveVm,
    /**guest is shut down (and -no-shutdown is in use)
*/
    #[qapi(name = "shutdown")]
    Shutdown,
    /**guest is suspended (ACPI S3)
*/
    #[qapi(name = "suspended")]
    Suspended,
    /**the watchdog action is configured to pause and has been
triggered
*/
    #[qapi(name = "watchdog")]
    Watchdog,
    /**guest has been panicked as a result of guest OS
panic
*/
    #[qapi(name = "guest-panicked")]
    GuestPanicked,
    /**guest is paused to save/restore VM state under colo
checkpoint, VM can not get into this state unless colo
capability is enabled for migration.  (since 2.8)*/
    #[qapi(name = "colo")]
    #[qapi(since = "2.8")]
    Colo,
}
/**An enumeration of reasons for a Shutdown.
*/
#[qapi]
pub enum ShutdownCause {
    /**No shutdown request pending
*/
    #[qapi(name = "none")]
    None,
    /**An error prevents further use of guest
*/
    #[qapi(name = "host-error")]
    HostError,
    /**Reaction to the QMP command 'quit'
*/
    #[qapi(name = "host-qmp-quit")]
    HostQmpQuit,
    /**Reaction to the QMP command 'system_reset'
*/
    #[qapi(name = "host-qmp-system-reset")]
    HostQmpSystemReset,
    /**Reaction to a signal, such as SIGINT
*/
    #[qapi(name = "host-signal")]
    HostSignal,
    /**Reaction to a UI event, like window close
*/
    #[qapi(name = "host-ui")]
    HostUi,
    /**Guest shutdown/suspend request, via ACPI or other
hardware-specific means
*/
    #[qapi(name = "guest-shutdown")]
    GuestShutdown,
    /**Guest reset request, and command line turns that into
a shutdown
*/
    #[qapi(name = "guest-reset")]
    GuestReset,
    /**Guest panicked, and command line turns that into a
shutdown
*/
    #[qapi(name = "guest-panic")]
    GuestPanic,
    /**Partial guest reset that does not trigger QMP
events and ignores --no-reboot.  This is useful for sanitizing
hypercalls on s390 that are used during kexec/kdump/boot
*/
    #[qapi(name = "subsystem-reset")]
    SubsystemReset,
    /**A snapshot is being loaded by the record & replay
subsystem.  This value is used only within QEMU.  It doesn't
occur in QMP.  (since 7.2)*/
    #[qapi(name = "snapshot-load")]
    #[qapi(since = "7.2")]
    SnapshotLoad,
}
/**Information about VM run state
*/
#[qapi(since = "0.14")]
pub struct StatusInfo {
    /**true if all VCPUs are runnable, false if not runnable
*/
    pub running: bool,
    /**the virtual machine @RunState
*/
    pub status: RunState,
}
/**Query the run status of the VM
*/
#[qapi(command)]
#[qapi(name = "query-status")]
#[qapi(since = "0.14")]
#[qapi(returns = "StatusInfo")]
#[qapi(allow_preconfig)]
pub struct QueryStatus {}
/**Emitted when the virtual machine has shut down, indicating that qemu
is about to exit.
*/
#[qapi(event)]
#[qapi(name = "SHUTDOWN")]
#[qapi(since = "0.12")]
pub struct ShutdownEvent {
    /**If true, the shutdown was triggered by a guest request (such
as a guest-initiated ACPI shutdown request or other
hardware-specific action) rather than a host request (such as
sending qemu a SIGINT).  (since 2.10)
*/
    #[qapi(since = "2.10")]
    pub guest: bool,
    /**The @ShutdownCause which resulted in the SHUTDOWN.
(since 4.0)
*/
    #[qapi(since = "4.0")]
    pub reason: ShutdownCause,
}
/**Emitted when the virtual machine is powered down through the power
control system, such as via ACPI.
*/
#[qapi(event)]
#[qapi(name = "POWERDOWN")]
#[qapi(since = "0.12")]
pub struct PowerdownEvent {}
/**Emitted when the virtual machine is reset
*/
#[qapi(event)]
#[qapi(name = "RESET")]
#[qapi(since = "0.12")]
pub struct ResetEvent {
    /**If true, the reset was triggered by a guest request (such as
a guest-initiated ACPI reboot request or other hardware-specific
action) rather than a host request (such as the QMP command
system_reset).  (since 2.10)
*/
    #[qapi(since = "2.10")]
    pub guest: bool,
    /**The @ShutdownCause of the RESET.  (since 4.0)
*/
    #[qapi(since = "4.0")]
    pub reason: ShutdownCause,
}
/**Emitted when the virtual machine is stopped
*/
#[qapi(event)]
#[qapi(name = "STOP")]
#[qapi(since = "0.12")]
pub struct StopEvent {}
/**Emitted when the virtual machine resumes execution
*/
#[qapi(event)]
#[qapi(name = "RESUME")]
#[qapi(since = "0.12")]
pub struct ResumeEvent {}
/**Emitted when guest enters a hardware suspension state, for example,
S3 state, which is sometimes called standby state
*/
#[qapi(event)]
#[qapi(name = "SUSPEND")]
#[qapi(since = "1.1")]
pub struct SuspendEvent {}
/**Emitted when guest enters a hardware suspension state with data
saved on disk, for example, S4 state, which is sometimes called
hibernate state
*/
#[qapi(event)]
#[qapi(name = "SUSPEND_DISK")]
#[qapi(since = "1.2")]
pub struct SuspendDiskEvent {}
/**Emitted when the guest has woken up from suspend state and is
running
*/
#[qapi(event)]
#[qapi(name = "WAKEUP")]
#[qapi(since = "1.1")]
pub struct WakeupEvent {}
/**Emitted when the watchdog device's timer is expired
*/
#[qapi(event)]
#[qapi(name = "WATCHDOG")]
#[qapi(since = "0.13")]
pub struct WatchdogEvent {
    /**action that has been taken
*/
    pub action: WatchdogAction,
}
/**An enumeration of the actions taken when the watchdog device's timer
is expired
*/
#[qapi(since = "2.1")]
pub enum WatchdogAction {
    /**system resets
*/
    #[qapi(name = "reset")]
    Reset,
    /**system shutdown, note that it is similar to @powerdown,
which tries to set to system status and notify guest
*/
    #[qapi(name = "shutdown")]
    Shutdown,
    /**system poweroff, the emulator program exits
*/
    #[qapi(name = "poweroff")]
    Poweroff,
    /**system pauses, similar to @stop
*/
    #[qapi(name = "pause")]
    Pause,
    /**system enters debug state
*/
    #[qapi(name = "debug")]
    Debug,
    /**nothing is done
*/
    #[qapi(name = "none")]
    None,
    /**a non-maskable interrupt is injected into the first
VCPU (all VCPUS on x86) (since 2.4)
*/
    #[qapi(name = "inject-nmi")]
    #[qapi(since = "2.4")]
    InjectNmi,
}
/**Possible QEMU actions upon guest reboot
*/
#[qapi(since = "6.0")]
pub enum RebootAction {
    /**Reset the VM
*/
    #[qapi(name = "reset")]
    Reset,
    /**Shutdown the VM and exit, according to the shutdown
action
*/
    #[qapi(name = "shutdown")]
    Shutdown,
}
/**Possible QEMU actions upon guest shutdown
*/
#[qapi(since = "6.0")]
pub enum ShutdownAction {
    /**Shutdown the VM and exit
*/
    #[qapi(name = "poweroff")]
    Poweroff,
    /**pause the VM
*/
    #[qapi(name = "pause")]
    Pause,
}
#[qapi(since = "6.0")]
pub enum PanicAction {
    /**Pause the VM
*/
    #[qapi(name = "pause")]
    Pause,
    /**Shutdown the VM and exit, according to the shutdown
action
*/
    #[qapi(name = "shutdown")]
    Shutdown,
    /**Shutdown the VM and exit with nonzero status (since
7.1)
*/
    #[qapi(name = "exit-failure")]
    #[qapi(since = "7.1")]
    ExitFailure,
    /**Continue VM execution
*/
    #[qapi(name = "none")]
    None,
}
/**Set watchdog action.
*/
#[qapi(command)]
#[qapi(name = "watchdog-set-action")]
#[qapi(since = "2.11")]
#[qapi(returns = "()")]
pub struct WatchdogSetAction {
    /**@WatchdogAction action taken when watchdog timer expires.
*/
    pub action: WatchdogAction,
}
/**Set the actions that will be taken by the emulator in response to
guest events.
*/
#[qapi(command)]
#[qapi(name = "set-action")]
#[qapi(since = "6.0")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct SetAction {
    /**@RebootAction action taken on guest reboot.
*/
    pub reboot: Option<RebootAction>,
    /**@ShutdownAction action taken on guest shutdown.
*/
    pub shutdown: Option<ShutdownAction>,
    /**@PanicAction action taken on guest panic.
*/
    pub panic: Option<PanicAction>,
    /**@WatchdogAction action taken when watchdog timer expires.
*/
    pub watchdog: Option<WatchdogAction>,
}
/**Emitted when guest OS panic is detected
*/
#[qapi(event)]
#[qapi(name = "GUEST_PANICKED")]
#[qapi(since = "1.5")]
pub struct GuestPanickedEvent {
    /**action that has been taken, currently always "pause"
*/
    pub action: GuestPanicAction,
    /**information about a panic (since 2.9)
*/
    #[qapi(since = "2.9")]
    pub info: Option<GuestPanicInformation>,
}
/**Emitted when guest OS crash loaded is detected
*/
#[qapi(event)]
#[qapi(name = "GUEST_CRASHLOADED")]
#[qapi(since = "5.0")]
pub struct GuestCrashloadedEvent {
    /**action that has been taken, currently always "run"
*/
    pub action: GuestPanicAction,
    /**information about a panic
*/
    pub info: Option<GuestPanicInformation>,
}
/**Emitted when guest submits a shutdown request via pvpanic interface
*/
#[qapi(event)]
#[qapi(name = "GUEST_PVSHUTDOWN")]
#[qapi(since = "9.1")]
pub struct GuestPvshutdownEvent {}
/**An enumeration of the actions taken when guest OS panic is detected
*/
#[qapi(since = "2.1")]
pub enum GuestPanicAction {
    /**system pauses
*/
    #[qapi(name = "pause")]
    Pause,
    /**system powers off (since 2.8)
*/
    #[qapi(name = "poweroff")]
    #[qapi(since = "2.8")]
    Poweroff,
    /**system continues to run (since 5.0)
*/
    #[qapi(name = "run")]
    #[qapi(since = "5.0")]
    Run,
}
/**An enumeration of the guest panic information types
*/
#[qapi(since = "2.9")]
pub enum GuestPanicInformationType {
    /**hyper-v guest panic information type
*/
    #[qapi(name = "hyper-v")]
    HyperV,
    /**s390 guest panic information type (Since: 2.12)
*/
    #[qapi(name = "s390")]
    #[qapi(since = "2.12")]
    S390,
}
#[qapi]
pub enum GuestPanicInformationBranch {
    #[qapi(name = "hyper-v")]
    HyperV(GuestPanicInformationHyperV),
    #[qapi(name = "s390")]
    S390(GuestPanicInformationS390),
}
/**Information about a guest panic
*/
#[qapi(since = "2.9")]
pub struct GuestPanicInformation {
    /**Crash type that defines the hypervisor specific information
*/
    #[qapi(discriminator)]
    pub r#type: GuestPanicInformationType,
    #[qapi(union)]
    pub u: Option<GuestPanicInformationBranch>,
}
/**Hyper-V specific guest panic information (HV crash MSRs)
*/
#[qapi(since = "2.9")]
pub struct GuestPanicInformationHyperV {
    /**for Windows, STOP code for the guest crash.  For Linux,
an error code.
*/
    pub arg1: u64,
    /**for Windows, first argument of the STOP.  For Linux, the
guest OS ID, which has the kernel version in bits 16-47 and
0x8100 in bits 48-63.
*/
    pub arg2: u64,
    /**for Windows, second argument of the STOP.  For Linux, the
program counter of the guest.
*/
    pub arg3: u64,
    /**for Windows, third argument of the STOP.  For Linux, the
RAX register (x86) or the stack pointer (aarch64) of the guest.
*/
    pub arg4: u64,
    /**for Windows, fourth argument of the STOP.  For x86 Linux, the
stack pointer of the guest.
*/
    pub arg5: u64,
}
/**Reason why the CPU is in a crashed state.
*/
#[qapi(since = "2.12")]
pub enum S390CrashReason {
    /**no crash reason was set
*/
    #[qapi(name = "unknown")]
    Unknown,
    /**the CPU has entered a disabled wait state
*/
    #[qapi(name = "disabled-wait")]
    DisabledWait,
    /**clock comparator or cpu timer interrupt with new PSW
enabled for external interrupts
*/
    #[qapi(name = "extint-loop")]
    ExtintLoop,
    /**program interrupt with BAD new PSW
*/
    #[qapi(name = "pgmint-loop")]
    PgmintLoop,
    /**operation exception interrupt with invalid code at the
program interrupt new PSW
*/
    #[qapi(name = "opint-loop")]
    OpintLoop,
}
/**S390 specific guest panic information (PSW)
*/
#[qapi(since = "2.12")]
pub struct GuestPanicInformationS390 {
    /**core id of the CPU that crashed
*/
    pub core: u32,
    /**control fields of guest PSW
*/
    #[qapi(name = "psw-mask")]
    pub psw_mask: u64,
    /**guest instruction address
*/
    #[qapi(name = "psw-addr")]
    pub psw_addr: u64,
    /**guest crash reason
*/
    pub reason: S390CrashReason,
}
/**Emitted when a memory failure occurs on host side.
*/
#[qapi(event)]
#[qapi(name = "MEMORY_FAILURE")]
#[qapi(since = "5.2")]
pub struct MemoryFailureEvent {
    /**recipient is defined as @MemoryFailureRecipient.
*/
    pub recipient: MemoryFailureRecipient,
    /**action that has been taken.
*/
    pub action: MemoryFailureAction,
    /**flags for MemoryFailureAction.
*/
    pub flags: MemoryFailureFlags,
}
/**Hardware memory failure occurs, handled by recipient.
*/
#[qapi(since = "5.2")]
pub enum MemoryFailureRecipient {
    /**memory failure at QEMU process address space.  (none
guest memory, but used by QEMU itself).
*/
    #[qapi(name = "hypervisor")]
    Hypervisor,
    /**memory failure at guest memory,
*/
    #[qapi(name = "guest")]
    Guest,
}
/**Actions taken by QEMU in response to a hardware memory failure.
*/
#[qapi(since = "5.2")]
pub enum MemoryFailureAction {
    /**the memory failure could be ignored.  This will only be the
case for action-optional failures.
*/
    #[qapi(name = "ignore")]
    Ignore,
    /**memory failure occurred in guest memory, the guest enabled
MCE handling mechanism, and QEMU could inject the MCE into the
guest successfully.
*/
    #[qapi(name = "inject")]
    Inject,
    /**the failure is unrecoverable.  This occurs for
action-required failures if the recipient is the hypervisor;
QEMU will exit.
*/
    #[qapi(name = "fatal")]
    Fatal,
    /**the failure is unrecoverable but confined to the guest.
This occurs if the recipient is a guest guest which is not ready
to handle memory failures.
*/
    #[qapi(name = "reset")]
    Reset,
}
/**Additional information on memory failures.
*/
#[qapi(since = "5.2")]
pub struct MemoryFailureFlags {
    /**whether a memory failure event is action-required
or action-optional (e.g. a failure during memory scrub).
*/
    #[qapi(name = "action-required")]
    pub action_required: bool,
    /**whether the failure occurred while the previous failure
was still in progress.
*/
    pub recursive: bool,
}
/**An enumeration of the options specified when enabling notify VM exit
*/
#[qapi(since = "7.2")]
pub enum NotifyVmexitOption {
    /**enable the feature, do nothing and continue if the notify VM
exit happens.
*/
    #[qapi(name = "run")]
    Run,
    /**enable the feature, raise a internal error if the
notify VM exit happens.
*/
    #[qapi(name = "internal-error")]
    InternalError,
    /**disable the feature.
*/
    #[qapi(name = "disable")]
    Disable,
}
// path end:	qapi/run-state.json
// path begin:	qapi/control.json
/**Enable QMP capabilities.
*/
#[qapi(command)]
#[qapi(name = "qmp_capabilities")]
#[qapi(since = "0.13")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct QmpCapabilities {
    /**An optional list of QMPCapability values to enable.  The
client must not enable any capability that is not mentioned in
the QMP greeting message.  If the field is not provided, it
means no QMP capabilities will be enabled.  (since 2.12)
*/
    #[qapi(since = "2.12")]
    pub enable: Option<Vec<QmpCapability>>,
}
/**Enumeration of capabilities to be advertised during initial client
connection, used for agreeing on particular QMP extension behaviors.
*/
#[qapi(name = "QMPCapability")]
#[qapi(since = "2.12")]
pub enum QmpCapability {
    /**QMP ability to support out-of-band requests.  (Please refer to
qmp-spec.rst for more information on OOB)
*/
    #[qapi(name = "oob")]
    Oob,
}
/**A three-part version number.
*/
#[qapi(since = "2.4")]
pub struct VersionTriple {
    /**The major version number.
*/
    pub major: i64,
    /**The minor version number.
*/
    pub minor: i64,
    /**The micro version number.
*/
    pub micro: i64,
}
/**A description of QEMU's version.
*/
#[qapi(since = "0.14")]
pub struct VersionInfo {
    /**The version of QEMU.  By current convention, a micro version
of 50 signifies a development branch.  A micro version greater
than or equal to 90 signifies a release candidate for the next
minor version.  A micro version of less than 50 signifies a
stable release.
*/
    pub qemu: VersionTriple,
    /**QEMU will always set this field to an empty string.
Downstream versions of QEMU should set this to a non-empty
string.  The exact format depends on the downstream however it
highly recommended that a unique name is used.
*/
    pub package: String,
}
/**Returns the current version of QEMU.
*/
#[qapi(command)]
#[qapi(name = "query-version")]
#[qapi(since = "0.14")]
#[qapi(returns = "VersionInfo")]
#[qapi(allow_preconfig)]
pub struct QueryVersion {}
/**Information about a QMP command
*/
#[qapi(since = "0.14")]
pub struct CommandInfo {
    /**The command name
*/
    pub name: String,
}
/**Return a list of supported QMP commands by this server
*/
#[qapi(command)]
#[qapi(name = "query-commands")]
#[qapi(since = "0.14")]
#[qapi(returns = "Vec<CommandInfo>")]
#[qapi(allow_preconfig)]
pub struct QueryCommands {}
/**This command will cause the QEMU process to exit gracefully.  While
every attempt is made to send the QMP response before terminating,
this is not guaranteed.  When using this interface, a premature EOF
would not be unexpected.
*/
#[qapi(command)]
#[qapi(name = "quit")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct Quit {}
/**An enumeration of monitor modes.
*/
#[qapi(since = "5.0")]
pub enum MonitorMode {
    /**HMP monitor (human-oriented command line interface)
*/
    #[qapi(name = "readline")]
    Readline,
    /**QMP monitor (JSON-based machine interface)
*/
    #[qapi(name = "control")]
    Control,
}
/**Options to be used for adding a new monitor.
*/
#[qapi(since = "5.0")]
pub struct MonitorOptions {
    /**Name of the monitor
*/
    pub id: Option<String>,
    /**Selects the monitor mode (default: readline in the system
emulator, control in qemu-storage-daemon)
*/
    pub mode: Option<MonitorMode>,
    /**Enables pretty printing (QMP only)
*/
    pub pretty: Option<bool>,
    /**Name of a character device to expose the monitor on
*/
    pub chardev: String,
}
// path end:	qapi/control.json
// path begin:	qapi/yank.json
/**An enumeration of yank instance types.  See @YankInstance for more
information.
*/
#[qapi(since = "6.0")]
pub enum YankInstanceType {
    #[qapi(name = "block-node")]
    BlockNode,
    #[qapi(name = "chardev")]
    Chardev,
    #[qapi(name = "migration")]
    Migration,
}
/**Specifies which block graph node to yank.  See @YankInstance for
more information.
*/
#[qapi(since = "6.0")]
pub struct YankInstanceBlockNode {
    /**the name of the block graph node
*/
    #[qapi(name = "node-name")]
    pub node_name: String,
}
/**Specifies which character device to yank.  See @YankInstance for
more information.
*/
#[qapi(since = "6.0")]
pub struct YankInstanceChardev {
    /**the chardev's ID
*/
    pub id: String,
}
#[qapi]
pub enum YankInstanceBranch {
    #[qapi(name = "block-node")]
    BlockNode(YankInstanceBlockNode),
    #[qapi(name = "chardev")]
    Chardev(YankInstanceChardev),
}
/**A yank instance can be yanked with the @yank qmp command to recover
from a hanging QEMU.
*/
#[qapi(since = "6.0")]
pub struct YankInstance {
    /**yank instance type

Currently implemented yank instances:

- nbd block device: Yanking it will shut down the connection to the
nbd server without attempting to reconnect.
- socket chardev: Yanking it will shut down the connected socket.
- migration: Yanking it will shut down all migration connections.
Unlike @migrate_cancel, it will not notify the migration process,
so migration will go into @failed state, instead of @cancelled
state.  @yank should be used to recover from hangs.
*/
    #[qapi(discriminator)]
    pub r#type: YankInstanceType,
    #[qapi(union)]
    pub u: Option<YankInstanceBranch>,
}
/**Try to recover from hanging QEMU by yanking the specified instances.
See @YankInstance for more information.
*/
#[qapi(command)]
#[qapi(name = "yank")]
#[qapi(since = "6.0")]
#[qapi(returns = "()")]
#[qapi(allow_oob)]
pub struct Yank {
    /**the instances to be yanked
*/
    pub instances: Vec<YankInstance>,
}
/**Query yank instances.  See @YankInstance for more information.
*/
#[qapi(command)]
#[qapi(name = "query-yank")]
#[qapi(since = "6.0")]
#[qapi(returns = "Vec<YankInstance>")]
#[qapi(allow_oob)]
pub struct QueryYank {}
// path end:	qapi/yank.json
// path begin:	qapi/misc.json
/**Allow client connections for VNC, Spice and socket based character
devices to be passed in to QEMU via SCM_RIGHTS.

If the FD associated with @fdname is not a socket, the command will
fail and the FD will be closed.
*/
#[qapi(command)]
#[qapi(name = "add_client")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct AddClient {
    /**protocol name.  Valid names are "vnc", "spice",
"@dbus-display" or the name of a character device (e.g. from
-chardev id=XXXX)
*/
    pub protocol: String,
    /**file descriptor name previously passed via 'getfd' command
*/
    pub fdname: String,
    /**whether to skip authentication.  Only applies to "vnc"
and "spice" protocols
*/
    pub skipauth: Option<bool>,
    /**whether to perform TLS.  Only applies to the "spice" protocol
*/
    pub tls: Option<bool>,
}
/**Guest name information.
*/
#[qapi(since = "0.14")]
pub struct NameInfo {
    /**The name of the guest
*/
    pub name: Option<String>,
}
/**Return the name information of a guest.
*/
#[qapi(command)]
#[qapi(name = "query-name")]
#[qapi(since = "0.14")]
#[qapi(returns = "NameInfo")]
#[qapi(allow_preconfig)]
pub struct QueryName {}
/**Information about an iothread
*/
#[qapi(name = "IOThreadInfo")]
#[qapi(since = "2.0")]
pub struct IoThreadInfo {
    /**the identifier of the iothread
*/
    pub id: String,
    /**ID of the underlying host thread
*/
    #[qapi(name = "thread-id")]
    pub thread_id: i64,
    /**maximum polling time in ns, 0 means polling is
disabled (since 2.9)
*/
    #[qapi(name = "poll-max-ns")]
    #[qapi(since = "2.9")]
    pub poll_max_ns: i64,
    /**how many ns will be added to polling time, 0 means that
it's not configured (since 2.9)
*/
    #[qapi(name = "poll-grow")]
    #[qapi(since = "2.9")]
    pub poll_grow: i64,
    /**how many ns will be removed from polling time, 0 means
that it's not configured (since 2.9)
*/
    #[qapi(name = "poll-shrink")]
    #[qapi(since = "2.9")]
    pub poll_shrink: i64,
    /**maximum number of requests in a batch for the AIO
engine, 0 means that the engine will use its default (since 6.1)
*/
    #[qapi(name = "aio-max-batch")]
    #[qapi(since = "6.1")]
    pub aio_max_batch: i64,
}
/**Returns a list of information about each iothread.
*/
#[qapi(command)]
#[qapi(name = "query-iothreads")]
#[qapi(since = "2.0")]
#[qapi(returns = "Vec<IoThreadInfo>")]
#[qapi(allow_preconfig)]
pub struct QueryIothreads {}
/**Stop guest VM execution.
*/
#[qapi(command)]
#[qapi(name = "stop")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct Stop {}
/**Resume guest VM execution.
*/
#[qapi(command)]
#[qapi(name = "cont")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct Cont {}
/**Exit from "preconfig" state

This command makes QEMU exit the preconfig state and proceed with VM
initialization using configuration data provided on the command line
and via the QMP monitor during the preconfig state.  The command is
only available during the preconfig state (i.e. when the --preconfig
command line option was in use).
*/
#[qapi(command)]
#[qapi(name = "x-exit-preconfig")]
#[qapi(feature = "unstable")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct XExitPreconfig {}
/**Execute a command on the human monitor and return the output.
*/
#[qapi(command)]
#[qapi(name = "human-monitor-command")]
#[qapi(feature = "savevm-monitor-nodes")]
#[qapi(since = "0.14")]
#[qapi(returns = "String")]
pub struct HumanMonitorCommand {
    /**the command to execute in the human monitor
*/
    #[qapi(name = "command-line")]
    pub command_line: String,
    /**The CPU to use for commands that require an implicit CPU
*/
    #[qapi(name = "cpu-index")]
    pub cpu_index: Option<i64>,
}
/**Receive a file descriptor via SCM rights and assign it a name
*/
#[qapi(command)]
#[qapi(name = "getfd")]
#[qapi(condition = "CONFIG_POSIX")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct Getfd {
    /**file descriptor name
*/
    pub fdname: String,
}
/**Add a socket that was duplicated to QEMU process with
WSADuplicateSocketW() via WSASocket() & WSAPROTOCOL_INFOW structure
and assign it a name (the SOCKET is associated with a CRT file
descriptor)
*/
#[qapi(command)]
#[qapi(name = "get-win32-socket")]
#[qapi(condition = "CONFIG_WIN32")]
#[qapi(since = "8.0")]
#[qapi(returns = "()")]
pub struct GetWin32Socket {
    /**the WSAPROTOCOL_INFOW structure (encoded in base64)
*/
    pub info: String,
    /**file descriptor name
*/
    pub fdname: String,
}
/**Close a file descriptor previously passed via SCM rights
*/
#[qapi(command)]
#[qapi(name = "closefd")]
#[qapi(since = "0.14")]
#[qapi(returns = "()")]
pub struct Closefd {
    /**file descriptor name
*/
    pub fdname: String,
}
/**Information about a file descriptor that was added to an fd set.
*/
#[qapi(since = "1.2")]
pub struct AddfdInfo {
    /**The ID of the fd set that @fd was added to.
*/
    #[qapi(name = "fdset-id")]
    pub fdset_id: i64,
    /**The file descriptor that was received via SCM rights and added
to the fd set.
*/
    pub fd: i64,
}
/**Add a file descriptor, that was passed via SCM rights, to an fd set.
*/
#[qapi(command)]
#[qapi(name = "add-fd")]
#[qapi(since = "1.2")]
#[qapi(returns = "AddfdInfo")]
pub struct AddFd {
    /**The ID of the fd set to add the file descriptor to.
*/
    #[qapi(name = "fdset-id")]
    pub fdset_id: Option<i64>,
    /**A free-form string that can be used to describe the fd.
*/
    pub opaque: Option<String>,
}
/**Remove a file descriptor from an fd set.
*/
#[qapi(command)]
#[qapi(name = "remove-fd")]
#[qapi(since = "1.2")]
#[qapi(returns = "()")]
pub struct RemoveFd {
    /**The ID of the fd set that the file descriptor belongs to.
*/
    #[qapi(name = "fdset-id")]
    pub fdset_id: i64,
    /**The file descriptor that is to be removed.
*/
    pub fd: Option<i64>,
}
/**Information about a file descriptor that belongs to an fd set.
*/
#[qapi(since = "1.2")]
pub struct FdsetFdInfo {
    /**The file descriptor value.
*/
    pub fd: i64,
    /**A free-form string that can be used to describe the fd.
*/
    pub opaque: Option<String>,
}
/**Information about an fd set.
*/
#[qapi(since = "1.2")]
pub struct FdsetInfo {
    /**The ID of the fd set.
*/
    #[qapi(name = "fdset-id")]
    pub fdset_id: i64,
    /**A list of file descriptors that belong to this fd set.
*/
    pub fds: Vec<FdsetFdInfo>,
}
/**Return information describing all fd sets.
*/
#[qapi(command)]
#[qapi(name = "query-fdsets")]
#[qapi(since = "1.2")]
#[qapi(returns = "Vec<FdsetInfo>")]
pub struct QueryFdsets {}
/**Possible types for an option parameter.
*/
#[qapi(since = "1.5")]
pub enum CommandLineParameterType {
    /**accepts a character string
*/
    #[qapi(name = "string")]
    String,
    /**accepts "on" or "off"
*/
    #[qapi(name = "boolean")]
    Boolean,
    /**accepts a number
*/
    #[qapi(name = "number")]
    Number,
    /**accepts a number followed by an optional suffix (K)ilo,
(M)ega, (G)iga, (T)era
*/
    #[qapi(name = "size")]
    Size,
}
/**Details about a single parameter of a command line option.
*/
#[qapi(since = "1.5")]
pub struct CommandLineParameterInfo {
    /**parameter name
*/
    pub name: String,
    /**parameter @CommandLineParameterType
*/
    pub r#type: CommandLineParameterType,
    /**human readable text string, not suitable for parsing.
*/
    pub help: Option<String>,
    /**default value string (since 2.1)
*/
    #[qapi(since = "2.1")]
    pub default: Option<String>,
}
/**Details about a command line option, including its list of parameter
details
*/
#[qapi(since = "1.5")]
pub struct CommandLineOptionInfo {
    /**option name
*/
    pub option: String,
    /**an array of @CommandLineParameterInfo
*/
    pub parameters: Vec<CommandLineParameterInfo>,
}
/**Query command line option schema.
*/
#[qapi(command)]
#[qapi(name = "query-command-line-options")]
#[qapi(since = "1.5")]
#[qapi(returns = "Vec<CommandLineOptionInfo>")]
#[qapi(allow_preconfig)]
pub struct QueryCommandLineOptions {
    /**option name
*/
    pub option: Option<String>,
}
/**Emitted when the guest changes the RTC time.
*/
#[qapi(event)]
#[qapi(name = "RTC_CHANGE")]
#[qapi(since = "0.13")]
pub struct RtcChangeEvent {
    /**offset in seconds between base RTC clock (as specified by
-rtc base), and new RTC clock value
*/
    pub offset: i64,
    /**path to the RTC object in the QOM tree
*/
    #[qapi(name = "qom-path")]
    pub qom_path: String,
}
/**Emitted when the client of a TYPE_VFIO_USER_SERVER closes the
communication channel
*/
#[qapi(event)]
#[qapi(name = "VFU_CLIENT_HANGUP")]
#[qapi(since = "7.1")]
pub struct VfuClientHangupEvent {
    /**ID of the TYPE_VFIO_USER_SERVER object.  It is the last
component of @vfu-qom-path referenced below
*/
    #[qapi(name = "vfu-id")]
    pub vfu_id: String,
    /**path to the TYPE_VFIO_USER_SERVER object in the QOM
tree
*/
    #[qapi(name = "vfu-qom-path")]
    pub vfu_qom_path: String,
    /**ID of attached PCI device
*/
    #[qapi(name = "dev-id")]
    pub dev_id: String,
    /**path to attached PCI device in the QOM tree
*/
    #[qapi(name = "dev-qom-path")]
    pub dev_qom_path: String,
}
// path end:	qapi/misc.json
// path begin:	qapi/test-schema.json
// path end:	qapi/test-schema.json

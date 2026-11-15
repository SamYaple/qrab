#![allow(unused_imports, non_camel_case_types, dead_code)]
use crate as qapi;
use qapi_macros::qapi;
use std::collections::HashMap;
use super::*;
/// Execute a command in the guest
///
/// Returns: PID
#[qapi(command)]
#[qapi(returns = "GuestExec")]
#[qapi(name = "guest-exec")]
#[qapi(added = "2.5.0")]
pub struct GuestExecCommand {
    /// argument list to pass to executable
    pub arg: Option<Vec<String>>,
    /// bool flag to enable capture of stdout/stderr of
    /// running process.  Defaults to false.
    #[qapi(name = "capture-output")]
    #[qapi(added = "8.1.0")]
    pub capture_output: Option<GuestExecCaptureOutput>,
    /// bool flag to enable capture of
    /// stdout/stderr of running process. defaults to false.
    #[qapi(name = "capture-output")]
    #[qapi(removed = "8.1.0")]
    pub capture_output_legacy: Option<bool>,
    /// environment variables to pass to executable
    pub env: Option<Vec<String>>,
    /// data to be passed to process stdin (base64 encoded)
    #[qapi(name = "input-data")]
    pub input_data: Option<String>,
    /// path or executable name to execute
    pub path: String,
}

/// Check status of process associated with PID retrieved via
/// `guest-exec`.  Reap the process and associated metadata if it has
/// exited.
#[qapi(command)]
#[qapi(returns = "GuestExecStatus")]
#[qapi(name = "guest-exec-status")]
#[qapi(added = "2.5.0")]
pub struct GuestExecStatusCommand {
    /// pid returned from `guest-exec`
    pub pid: i64,
}

/// Close an open file in the guest
#[qapi(command)]
#[qapi(name = "guest-file-close")]
#[qapi(returns = "()")]
#[qapi(added = "1.4.0")]
pub struct GuestFileClose {
    /// filehandle returned by `guest-file-open`
    pub handle: i64,
}

/// Write file changes buffered in userspace to disk/kernel buffers
#[qapi(command)]
#[qapi(name = "guest-file-flush")]
#[qapi(returns = "()")]
#[qapi(added = "1.4.0")]
pub struct GuestFileFlush {
    /// filehandle returned by `guest-file-open`
    pub handle: i64,
}

/// Open a file in the guest and retrieve a file handle for it
///
/// Returns: Guest file handle
#[qapi(command)]
#[qapi(name = "guest-file-open")]
#[qapi(returns = "i64")]
#[qapi(added = "1.4.0")]
pub struct GuestFileOpen {
    /// open mode, as per fopen(), "r" is the default.
    pub mode: Option<String>,
    /// Full path to the file in the guest to open.
    pub path: String,
}

/// Read from an open file in the guest.  Data will be base64-encoded.
/// As this command is just for limited, ad-hoc debugging, such as log
/// file access, the number of bytes to read is limited to 48 MB.
#[qapi(command)]
#[qapi(returns = "GuestFileRead")]
#[qapi(name = "guest-file-read")]
#[qapi(added = "1.4.0")]
pub struct GuestFileReadCommand {
    /// maximum number of bytes to read (default is 4KB, maximum is
    /// 48MB)
    pub count: Option<i64>,
    /// filehandle returned by `guest-file-open`
    pub handle: i64,
}

/// Seek to a position in the file, as with fseek(), and return the
/// current file position afterward.  Also encapsulates ftell()'s
/// functionality, with offset=0 and whence=1.
#[qapi(command)]
#[qapi(returns = "GuestFileSeek")]
#[qapi(name = "guest-file-seek")]
#[qapi(added = "1.4.0")]
pub struct GuestFileSeekCommand {
    /// filehandle returned by `guest-file-open`
    pub handle: i64,
    /// bytes to skip over in the file stream
    pub offset: i64,
    /// Symbolic or numeric code for interpreting offset
    #[qapi(added = "2.6.0")]
    #[qapi(required)]
    pub whence: Option<GuestFileWhence>,
    /// 0 for SEEK_SET, 1 for SEEK_CUR, or 2 for SEEK_END
    #[qapi(name = "whence")]
    #[qapi(removed = "2.6.0")]
    #[qapi(required)]
    pub whence_legacy: Option<i64>,
}

/// Write to an open file in the guest.
#[qapi(command)]
#[qapi(returns = "GuestFileWrite")]
#[qapi(name = "guest-file-write")]
#[qapi(added = "1.4.0")]
pub struct GuestFileWriteCommand {
    /// base64-encoded string representing data to be written
    #[qapi(name = "buf-b64")]
    pub buf_b64: String,
    /// bytes to write (actual bytes, after base64-decode), default
    /// is all content in buf-b64 buffer after base64 decoding
    pub count: Option<i64>,
    /// filehandle returned by `guest-file-open`
    pub handle: i64,
}

/// Sync and freeze all freezable, local guest filesystems.  If this
/// command succeeded, you may call `guest-fsfreeze-thaw` later to
/// unfreeze.
///
/// On error, all filesystems will be thawed.  If no filesystems are
/// frozen as a result of this call, then `guest-fsfreeze-status` will
/// remain "thawed" and calling `guest-fsfreeze-thaw` is not necessary.
///
/// Returns: Number of file systems currently frozen.
///
/// .. note:: On Windows, the command is implemented with the help of a
///    Volume Shadow-copy Service DLL helper.  The frozen state is
///    limited for up to 10 seconds by VSS.
#[qapi(command)]
#[qapi(name = "guest-fsfreeze-freeze")]
#[qapi(returns = "i64")]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_FSFREEZE)")]
#[qapi(added = "1.4.0")]
pub struct GuestFsfreezeFreeze {}

/// Sync and freeze specified guest filesystems.  See also
/// `guest-fsfreeze-freeze`.
///
/// On error, all filesystems will be thawed.
///
/// Returns: Number of file systems currently frozen.
#[qapi(command)]
#[qapi(name = "guest-fsfreeze-freeze-list")]
#[qapi(returns = "i64")]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_FSFREEZE)")]
#[qapi(added = "2.2.0")]
pub struct GuestFsfreezeFreezeList {
    /// an array of mountpoints of filesystems to be frozen.
    /// If omitted, every mounted filesystem is frozen.  Invalid mount
    /// points are ignored.
    pub mountpoints: Option<Vec<String>>,
}

/// Get guest fsfreeze state.
///
/// .. note:: This may fail to properly report the current state as a
///    result of some other guest processes having issued an fs
///    freeze/thaw.
#[qapi(command)]
#[qapi(returns = "GuestFsfreezeStatus")]
#[qapi(name = "guest-fsfreeze-status")]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_FSFREEZE)")]
#[qapi(added = "1.4.0")]
pub struct GuestFsfreezeStatusCommand {}

/// Unfreeze all frozen guest filesystems
///
/// Returns: Number of file systems thawed by this call
///
/// .. note:: If the return value does not match the previous call to
///    `guest-fsfreeze-freeze`, this likely means some freezable
///    filesystems were unfrozen before this call, and that the
///    filesystem state may have changed before issuing this command.
#[qapi(command)]
#[qapi(name = "guest-fsfreeze-thaw")]
#[qapi(returns = "i64")]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_FSFREEZE)")]
#[qapi(added = "1.4.0")]
pub struct GuestFsfreezeThaw {}

/// Discard (or "trim") blocks which are not in use by the filesystem.
///
/// Returns: status of all trimmed paths.  (since 2.4)
#[qapi(command)]
#[qapi(name = "guest-fstrim")]
#[qapi(returns = "GuestFilesystemTrimResponse")]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_FSTRIM)")]
#[qapi(added = "1.4.0")]
#[qapi(returns(added = "1.4.0", value = "()"))]
#[qapi(returns(added = "2.4.0", value = "GuestFilesystemTrimResponse"))]
pub struct GuestFstrim {
    /// Minimum contiguous free range to discard, in bytes.  Free
    /// ranges smaller than this may be ignored (this is a hint and the
    /// guest may not respect it).  By increasing this value, the fstrim
    /// operation will complete more quickly for filesystems with badly
    /// fragmented free space, although not all blocks will be
    /// discarded.  The default value is zero, meaning "discard every
    /// free block".
    pub minimum: Option<i64>,
}

/// Retrieve information about CPU stats.
///
/// Returns: List of CPU stats of guest.
#[qapi(command)]
#[qapi(name = "guest-get-cpustats")]
#[qapi(returns = "Vec<GuestCpuStats>")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "7.1.0")]
pub struct GuestGetCpustats {}

/// Retrieve information about device drivers in Windows guest
#[qapi(command)]
#[qapi(name = "guest-get-devices")]
#[qapi(returns = "Vec<GuestDeviceInfo>")]
#[qapi(condition = "CONFIG_WIN32")]
#[qapi(added = "5.2.0")]
pub struct GuestGetDevices {}

/// Returns: The list of disks in the guest.  For Windows these are only
/// ```text
///     the physical disks.  On Linux these are all root block devices
///     of non-zero size including e.g. removable devices, loop devices,
///     NBD, etc.
/// ```
#[qapi(command)]
#[qapi(name = "guest-get-disks")]
#[qapi(returns = "Vec<GuestDiskInfo>")]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_LIBUDEV)")]
#[qapi(added = "5.2.0")]
pub struct GuestGetDisks {}

/// Retrieve information about disk stats.
///
/// Returns: List of disk stats of guest.
#[qapi(command)]
#[qapi(name = "guest-get-diskstats")]
#[qapi(returns = "Vec<GuestDiskStatsInfo>")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "7.1.0")]
pub struct GuestGetDiskstats {}

/// Returns: The list of filesystems information mounted in the guest.
/// ```text
///     The returned mountpoints may be specified to
///     `guest-fsfreeze-freeze-list`.  Network filesystems (such as CIFS
///     and NFS) are not listed.
/// ```
#[qapi(command)]
#[qapi(name = "guest-get-fsinfo")]
#[qapi(returns = "Vec<GuestFilesystemInfo>")]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_LINUX)")]
#[qapi(added = "2.2.0")]
pub struct GuestGetFsinfo {}

/// Return a name for the machine.
///
/// The returned name is not necessarily a fully-qualified domain name,
/// or even present in DNS or some other name service at all.  It need
/// not even be unique on your local network or site, but usually it is.
///
/// Returns: the host name of the machine
#[qapi(command)]
#[qapi(name = "guest-get-host-name")]
#[qapi(returns = "GuestHostName")]
#[qapi(added = "2.10.0")]
pub struct GuestGetHostName {}

/// Retrieve CPU process load information
///
/// .. note:: Windows does not have load average API, so QGA emulates it
///    by calculating the average CPU usage in the last 1, 5, 15 minutes
///    similar as Linux does this.  Calculation starts from the first
///    time this command is called.
///
/// Returns: load information
#[qapi(command)]
#[qapi(name = "guest-get-load")]
#[qapi(condition = "CONFIG_GETLOADAVG")]
#[qapi(returns = "GuestLoadAverage")]
#[qapi(added = "10.0.0")]
pub struct GuestGetLoad {}

/// Get information relating to guest memory blocks.
#[qapi(command)]
#[qapi(name = "guest-get-memory-block-info")]
#[qapi(returns = "GuestMemoryBlockInfo")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "2.3.0")]
pub struct GuestGetMemoryBlockInfo {}

/// Retrieve the list of the guest's memory blocks.
///
/// This is a read-only operation.
///
/// Returns: The list of all memory blocks the guest knows about.  Each
/// ```text
///     memory block is put on the list exactly once, but their order is
///     unspecified.
/// ```
#[qapi(command)]
#[qapi(name = "guest-get-memory-blocks")]
#[qapi(returns = "Vec<GuestMemoryBlock>")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "2.3.0")]
pub struct GuestGetMemoryBlocks {}

/// Retrieve guest operating system information
#[qapi(command)]
#[qapi(name = "guest-get-osinfo")]
#[qapi(returns = "GuestOsInfo")]
#[qapi(added = "2.10.0")]
pub struct GuestGetOsinfo {}

/// Get the information about guest's System Time relative to the Epoch
/// of 1970-01-01 in UTC.
///
/// Returns: Time in nanoseconds.
#[qapi(command)]
#[qapi(name = "guest-get-time")]
#[qapi(returns = "i64")]
#[qapi(added = "1.5.0")]
pub struct GuestGetTime {}

/// Retrieves the timezone information from the guest.
#[qapi(command)]
#[qapi(name = "guest-get-timezone")]
#[qapi(returns = "GuestTimezone")]
#[qapi(added = "2.10.0")]
pub struct GuestGetTimezone {}

/// Retrieves a list of currently active users on the VM.
///
/// Returns: A unique list of users.
#[qapi(command)]
#[qapi(name = "guest-get-users")]
#[qapi(returns = "Vec<GuestUser>")]
#[qapi(condition = "(CONFIG_WIN32 || HAVE_UTMPX)")]
#[qapi(added = "2.10.0")]
pub struct GuestGetUsers {}

/// Retrieve the list of the guest's logical processors.
///
/// This is a read-only operation.
///
/// Returns: The list of all VCPUs the guest knows about.  Each VCPU is
/// ```text
///     put on the list exactly once, but their order is unspecified.
/// ```
#[qapi(command)]
#[qapi(name = "guest-get-vcpus")]
#[qapi(returns = "Vec<GuestLogicalProcessor>")]
#[qapi(condition = "(CONFIG_LINUX || CONFIG_WIN32)")]
#[qapi(added = "1.5.0")]
pub struct GuestGetVcpus {}

/// Get some information about the guest agent.
#[qapi(command)]
#[qapi(name = "guest-info")]
#[qapi(returns = "GuestAgentInfo")]
#[qapi(added = "1.4.0")]
pub struct GuestInfo {}

/// Get list of guest IP addresses, MAC addresses and netmasks.
#[qapi(command)]
#[qapi(name = "guest-network-get-interfaces")]
#[qapi(returns = "Vec<GuestNetworkInterface>")]
#[qapi(condition = "(CONFIG_WIN32 || HAVE_GETIFADDRS)")]
#[qapi(added = "1.4.0")]
pub struct GuestNetworkGetInterfaces {}

/// Retrieve information about route of network.
///
/// Returns: List of route info of guest.
#[qapi(command)]
#[qapi(name = "guest-network-get-route")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(returns = "Vec<GuestNetworkRoute>")]
#[qapi(added = "9.1.0")]
pub struct GuestNetworkGetRoute {}

/// Ping the guest agent, a non-error return implies success
#[qapi(command)]
#[qapi(name = "guest-ping")]
#[qapi(returns = "()")]
#[qapi(added = "1.4.0")]
pub struct GuestPing {}

/// Attempt to reconfigure (currently: enable/disable) state of memory
/// blocks inside the guest.
///
/// Returns: The operation results, it is a list of
/// ```text
///     `GuestMemoryBlockResponse`, which is corresponding to the input
///     list.
/// ```
///
/// ```text
///     Note: it will return an empty list if the @mem-blks list was
///     empty on input, or there is an error, and in this case, guest
///     state will not be changed.
/// ```
#[qapi(command)]
#[qapi(name = "guest-set-memory-blocks")]
#[qapi(returns = "Vec<GuestMemoryBlockResponse>")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "2.3.0")]
pub struct GuestSetMemoryBlocks {
    /// The memory blocks to be reconfigured.  This list is
    /// processed node by node in order.  In each node @phys-index is
    /// used to look up the guest MEMORY BLOCK, for which @online
    /// specifies the requested state.  The set of distinct
    /// @phys-index's is only required to be a subset of the
    /// guest-supported identifiers.  There's no restriction on list
    /// length or on repeating the same @phys-index (with possibly
    /// different @online field).  Preferably the input list should
    /// describe a modified subset of `guest-get-memory-blocks`' return
    /// value.
    #[qapi(name = "mem-blks")]
    pub mem_blks: Vec<GuestMemoryBlock>,
}

/// Set guest time.
///
/// When a guest is paused or migrated to a file then loaded from that
/// file, the guest OS has no idea that there was a big gap in the time.
/// Depending on how long the gap was, NTP might not be able to
/// resynchronize the guest.
///
/// This command tries to set guest's System Time to the given value,
/// then sets the Hardware Clock (RTC) to the current System Time.  This
/// will make it easier for a guest to resynchronize without waiting for
/// NTP.  If no @time is specified, then the time to set is read from
/// RTC.  However, this may not be supported on all platforms (i.e.
/// Windows).  If that's the case users are advised to always pass a
/// value.
#[qapi(command)]
#[qapi(name = "guest-set-time")]
#[qapi(returns = "()")]
#[qapi(added = "1.5.0")]
pub struct GuestSetTime {
    /// time of nanoseconds, relative to the Epoch of 1970-01-01 in
    /// UTC.
    pub time: Option<i64>,
}

/// If the @crypted flag is true, it is the caller's responsibility to
/// ensure the correct crypt() encryption scheme is used.  This command
/// does not attempt to interpret or report on the encryption scheme.
/// Refer to the documentation of the guest operating system in question
/// to determine what is supported.
///
/// Not all guest operating systems will support use of the @crypted
/// flag, as they may require the clear-text password
///
/// The @password parameter must always be base64 encoded before
/// transmission, even if already crypt()d, to ensure it is 8-bit safe
/// when passed as JSON.
#[qapi(command)]
#[qapi(name = "guest-set-user-password")]
#[qapi(returns = "()")]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_LINUX || CONFIG_FREEBSD)")]
#[qapi(added = "2.3.0")]
pub struct GuestSetUserPassword {
    /// true if password is already crypt()d, false if raw
    pub crypted: bool,
    /// the new password entry string, base64 encoded
    pub password: String,
    /// the user account whose password to change
    pub username: String,
}

/// Attempt to reconfigure (currently: enable/disable) logical
/// processors inside the guest.
///
/// Returns: The length of the initial sublist that has been
/// ```text
///     successfully processed.  The guest agent maximizes this value.
///     Possible cases:
/// ```
///
/// ```text
///     - 0:
///       if the @vcpus list was empty on input.  Guest state has not
///       been changed.  Otherwise,
///     - < length(@vcpus):
///       more than zero initial nodes have been processed, but not the
///       entire @vcpus list.  Guest state has changed accordingly.  To
///       retrieve the error (assuming it persists), repeat the call
///       with the successfully processed initial sublist removed.
///       Otherwise,
///     - length(@vcpus):
///       call successful.
/// ```
///
/// Errors:
/// ```text
///     - If the reconfiguration of the first node in @vcpus failed.
///       Guest state has not been changed.
/// ```
#[qapi(command)]
#[qapi(name = "guest-set-vcpus")]
#[qapi(returns = "i64")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "1.5.0")]
pub struct GuestSetVcpus {
    /// The logical processors to be reconfigured.  This list is
    /// processed node by node in order.  In each node @logical-id is
    /// used to look up the guest VCPU, for which @online specifies the
    /// requested state.  The set of distinct @logical-id's is only
    /// required to be a subset of the guest-supported identifiers.
    /// There's no restriction on list length or on repeating the same
    /// @logical-id (with possibly different @online field).  Preferably
    /// the input list should describe a modified subset of
    /// `guest-get-vcpus`' return value.
    pub vcpus: Vec<GuestLogicalProcessor>,
}

/// Initiate guest-activated shutdown.  Note: this is an asynchronous
/// shutdown request, with no guarantee of successful shutdown.
///
/// This command does NOT return a response on success.  Success
/// condition is indicated by the VM exiting with a zero exit status or,
/// when running with --no-shutdown, by issuing the `query-status` QMP
/// command to confirm the VM status is "shutdown".
#[qapi(command)]
#[qapi(name = "guest-shutdown")]
#[qapi(returns = "()")]
#[qapi(added = "1.4.0")]
#[qapi(no_success_response(added = "2.4.0"))]
pub struct GuestShutdown {
    /// "halt", "powerdown" (default), or "reboot"
    pub mode: Option<String>,
}

/// Append public keys to user .ssh/authorized_keys on Unix systems (not
/// implemented for other systems).
#[qapi(command)]
#[qapi(name = "guest-ssh-add-authorized-keys")]
#[qapi(condition = "defined(CONFIG_POSIX)")]
#[qapi(returns = "()")]
#[qapi(added = "5.2.0")]
pub struct GuestSshAddAuthorizedKeys {
    /// the public keys to add (in OpenSSH/sshd(8) authorized_keys
    /// format)
    pub keys: Vec<String>,
    /// ignore the existing content, set it with the given keys only
    pub reset: Option<bool>,
    /// the user account to add the authorized keys
    pub username: String,
}

/// Return the public keys from user .ssh/authorized_keys on Unix
/// systems (not implemented for other systems).
#[qapi(command)]
#[qapi(name = "guest-ssh-get-authorized-keys")]
#[qapi(condition = "defined(CONFIG_POSIX)")]
#[qapi(returns = "GuestAuthorizedKeys")]
#[qapi(added = "5.2.0")]
pub struct GuestSshGetAuthorizedKeys {
    /// the user account to add the authorized keys
    pub username: String,
}

/// Remove public keys from the user .ssh/authorized_keys on Unix
/// systems (not implemented for other systems).  It's not an error if
/// the key is already missing.
#[qapi(command)]
#[qapi(name = "guest-ssh-remove-authorized-keys")]
#[qapi(condition = "defined(CONFIG_POSIX)")]
#[qapi(returns = "()")]
#[qapi(added = "5.2.0")]
pub struct GuestSshRemoveAuthorizedKeys {
    /// the public keys to remove (in OpenSSH/sshd(8) authorized_keys
    /// format)
    pub keys: Vec<String>,
    /// the user account to remove the authorized keys
    pub username: String,
}

/// Suspend guest to disk.
///
/// This command attempts to suspend the guest using three strategies,
/// in this order:
///
/// - systemd hibernate
/// - pm-utils (via pm-hibernate)
/// - manual write into sysfs
///
/// This command does NOT return a response on success.  There is a high
/// chance the command succeeded if the VM exits with a zero exit status
/// or, when running with --no-shutdown, by issuing the `query-status`
/// QMP command to to confirm the VM status is "shutdown".  However, the
/// VM could also exit (or set its status to "shutdown") due to other
/// reasons.
///
/// Errors:
/// ```text
///     - If suspend to disk is not supported, Unsupported
/// ```
///
/// .. note:: It's strongly recommended to issue the `guest-sync`
///    command before sending commands when the guest resumes.
#[qapi(command)]
#[qapi(name = "guest-suspend-disk")]
#[qapi(returns = "()")]
#[qapi(condition = "(CONFIG_LINUX || CONFIG_WIN32)")]
#[qapi(added = "1.4.0")]
#[qapi(no_success_response(added = "2.4.0"))]
pub struct GuestSuspendDisk {}

/// Save guest state to disk and suspend to ram.
///
/// This command attempts to suspend the guest by executing, in this
/// order:
///
/// - systemd hybrid-sleep
/// - pm-utils (via pm-suspend-hybrid)
///
/// IMPORTANT: `guest-suspend-hybrid` requires working wakeup support in
/// QEMU.  You should check QMP command `query-current-machine` returns
/// wakeup-suspend-support: true before issuing this command.  Failure
/// in doing so can result in a suspended guest that QEMU will not be
/// able to awaken, forcing the user to power cycle the guest to bring
/// it back.
///
/// This command does NOT return a response on success.  There are two
/// options to check for success:
///
/// 1. Wait for the `SUSPEND` QMP event from QEMU
/// 2. Issue the `query-status` QMP command to confirm the VM status is
///    "suspended"
///
/// Errors:
/// ```text
///     - If hybrid suspend is not supported, Unsupported
/// ```
///
/// .. note:: It's strongly recommended to issue the `guest-sync`
///    command before sending commands when the guest resumes.
#[qapi(command)]
#[qapi(name = "guest-suspend-hybrid")]
#[qapi(returns = "()")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "1.4.0")]
#[qapi(no_success_response(added = "2.4.0"))]
pub struct GuestSuspendHybrid {}

/// Suspend guest to ram.
///
/// This command attempts to suspend the guest using three strategies,
/// in this order:
///
/// - systemd hibernate
/// - pm-utils (via pm-hibernate)
/// - manual write into sysfs
///
/// IMPORTANT: `guest-suspend-ram` requires working wakeup support in
/// QEMU.  You should check QMP command `query-current-machine` returns
/// wakeup-suspend-support: true before issuing this command.  Failure
/// in doing so can result in a suspended guest that QEMU will not be
/// able to awaken, forcing the user to power cycle the guest to bring
/// it back.
///
/// This command does NOT return a response on success.  There are two
/// options to check for success:
///
/// 1. Wait for the `SUSPEND` QMP event from QEMU
/// 2. Issue the `query-status` QMP command to confirm the VM status is
///    "suspended"
///
/// Errors:
/// ```text
///     - If suspend to ram is not supported, Unsupported
/// ```
///
/// .. note:: It's strongly recommended to issue the `guest-sync`
///    command before sending commands when the guest resumes.
#[qapi(command)]
#[qapi(name = "guest-suspend-ram")]
#[qapi(returns = "()")]
#[qapi(condition = "(CONFIG_LINUX || CONFIG_WIN32)")]
#[qapi(added = "1.4.0")]
#[qapi(no_success_response(added = "2.4.0"))]
pub struct GuestSuspendRam {}

/// Echo back a unique integer value
///
/// This is used by clients talking to the guest agent over the wire to
/// ensure the stream is in sync and doesn't contain stale data from
/// previous client.  All guest agent responses should be ignored until
/// the provided unique integer value is returned, and it is up to the
/// client to handle stale whole or partially-delivered JSON text in
/// such a way that this response can be obtained.
///
/// In cases where a partial stale response was previously received by
/// the client, this cannot always be done reliably.  One particular
/// scenario being if qemu-ga responses are fed character-by-character
/// into a JSON parser.  In these situations, using
/// `guest-sync-delimited` may be optimal.
///
/// For clients that fetch responses line by line and convert them to
/// JSON objects, `guest-sync` should be sufficient, but note that in
/// cases where the channel is dirty some attempts at parsing the
/// response may result in a parser error.
///
/// Such clients should also precede this command with a 0xFF byte to
/// make sure the guest agent flushes any partially read JSON data from
/// a previous session.
///
/// Returns: The unique integer id passed in by the client
#[qapi(command)]
#[qapi(name = "guest-sync")]
#[qapi(returns = "i64")]
#[qapi(added = "1.4.0")]
pub struct GuestSync {
    /// randomly generated 64-bit integer
    pub id: i64,
}

/// Echo back a unique integer value, and prepend to response a leading
/// sentinel byte (0xFF) the client can check scan for.
///
/// This is used by clients talking to the guest agent over the wire to
/// ensure the stream is in sync and doesn't contain stale data from
/// previous client.  It must be issued upon initial connection, and
/// after any client-side timeouts (including timeouts on receiving a
/// response to this command).
///
/// After issuing this request, all guest agent responses should be
/// ignored until the response containing the unique integer value the
/// client passed in is returned.  Receival of the 0xFF sentinel byte
/// must be handled as an indication that the client's
/// lexer/tokenizer/parser state should be flushed/reset in preparation
/// for reliably receiving the subsequent response.  As an optimization,
/// clients may opt to ignore all data until a sentinel value is
/// receiving to avoid unnecessary processing of stale data.
///
/// Similarly, clients should also precede this *request* with a 0xFF
/// byte to make sure the guest agent flushes any partially read JSON
/// data from a previous client connection.
///
/// Returns: The unique integer id passed in by the client
#[qapi(command)]
#[qapi(name = "guest-sync-delimited")]
#[qapi(returns = "i64")]
#[qapi(added = "1.4.0")]
pub struct GuestSyncDelimited {
    /// randomly generated 64-bit integer
    pub id: i64,
}

#![allow(unused_imports, non_camel_case_types, dead_code)]
use crate as qapi;
use qapi_macros::qapi;
use std::collections::HashMap;
use super::*;
#[qapi(command)]
#[qapi(name = "guest-exec")]
#[qapi(returns = "GuestExec")]
#[qapi(added = "2.5.0")]
pub struct GuestExec {
    pub arg: Option<Vec<String>>,
    #[qapi(name = "capture-output")]
    pub capture_output: Option<GuestExecCaptureOutput>,
    pub env: Option<Vec<String>>,
    #[qapi(name = "input-data")]
    pub input_data: Option<String>,
    pub path: String,
}

/// Check status of process associated with PID retrieved via
/// `guest-exec`.  Reap the process and associated metadata if it has
/// exited.
///
#[qapi(command)]
#[qapi(name = "guest-exec-status")]
#[qapi(returns = "GuestExecStatus")]
#[qapi(added = "2.5.0")]
pub struct GuestExecStatus {
    /// pid returned from `guest-exec`
    pub pid: i64,
}

/// Close an open file in the guest
///
#[qapi(command)]
#[qapi(name = "guest-file-close")]
#[qapi(returns = "()")]
#[qapi(added = "1.4.0")]
pub struct GuestFileClose {
    /// filehandle returned by `guest-file-open`
    pub handle: i64,
}

/// Write file changes buffered in userspace to disk/kernel buffers
///
#[qapi(command)]
#[qapi(name = "guest-file-flush")]
#[qapi(returns = "()")]
#[qapi(added = "1.4.0")]
pub struct GuestFileFlush {
    /// filehandle returned by `guest-file-open`
    pub handle: i64,
}

#[qapi(command)]
#[qapi(name = "guest-file-open")]
#[qapi(returns = "i64")]
#[qapi(added = "1.4.0")]
pub struct GuestFileOpen {
    pub mode: Option<String>,
    pub path: String,
}

/// Read from an open file in the guest.  Data will be base64-encoded.
/// As this command is just for limited, ad-hoc debugging, such as log
/// file access, the number of bytes to read is limited to 48 MB.
///
#[qapi(command)]
#[qapi(name = "guest-file-read")]
#[qapi(returns = "GuestFileRead")]
#[qapi(added = "1.4.0")]
pub struct GuestFileRead {
    /// maximum number of bytes to read (default is 4KB, maximum is
    /// 48MB)
    pub count: Option<i64>,
    /// filehandle returned by `guest-file-open`
    pub handle: i64,
}

/// Seek to a position in the file, as with fseek(), and return the
/// current file position afterward.  Also encapsulates ftell()'s
/// functionality, with offset=0 and whence=1.
///
#[qapi(command)]
#[qapi(name = "guest-file-seek")]
#[qapi(returns = "GuestFileSeek")]
#[qapi(added = "1.4.0")]
pub struct GuestFileSeek {
    /// filehandle returned by `guest-file-open`
    pub handle: i64,
    /// bytes to skip over in the file stream
    pub offset: i64,
    /// Symbolic or numeric code for interpreting offset
    pub whence: GuestFileWhence,
}

/// Write to an open file in the guest.
///
#[qapi(command)]
#[qapi(name = "guest-file-write")]
#[qapi(returns = "GuestFileWrite")]
#[qapi(added = "1.4.0")]
pub struct GuestFileWrite {
    /// base64-encoded string representing data to be written
    #[qapi(name = "buf-b64")]
    pub buf_b64: String,
    /// bytes to write (actual bytes, after base64-decode), default
    /// is all content in buf-b64 buffer after base64 decoding
    pub count: Option<i64>,
    /// filehandle returned by `guest-file-open`
    pub handle: i64,
}

#[qapi(command)]
#[qapi(name = "guest-fsfreeze-freeze")]
#[qapi(returns = "i64")]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_FSFREEZE)")]
#[qapi(added = "1.4.0")]
pub struct GuestFsfreezeFreeze {}

#[qapi(command)]
#[qapi(name = "guest-fsfreeze-freeze-list")]
#[qapi(returns = "i64")]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_FSFREEZE)")]
#[qapi(added = "2.2.0")]
pub struct GuestFsfreezeFreezeList {
    pub mountpoints: Option<Vec<String>>,
}

/// Get guest fsfreeze state.
///
/// .. note:: This may fail to properly report the current state as a
///    result of some other guest processes having issued an fs
///    freeze/thaw.
///
#[qapi(command)]
#[qapi(name = "guest-fsfreeze-status")]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_FSFREEZE)")]
#[qapi(returns = "GuestFsfreezeStatus")]
#[qapi(added = "10.2.0")]
pub struct GuestFsfreezeStatus {}

#[qapi(command)]
#[qapi(name = "guest-fsfreeze-thaw")]
#[qapi(returns = "i64")]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_FSFREEZE)")]
#[qapi(added = "1.4.0")]
pub struct GuestFsfreezeThaw {}

#[qapi(command)]
#[qapi(name = "guest-fstrim")]
#[qapi(returns = "()")]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_FSTRIM)")]
#[qapi(added = "1.4.0")]
pub struct GuestFstrim {
    pub minimum: Option<i64>,
}

#[qapi(command)]
#[qapi(name = "guest-get-cpustats")]
#[qapi(returns = "Vec<GuestCpuStats>")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "7.1.0")]
pub struct GuestGetCpustats {}

/// Retrieve information about device drivers in Windows guest
///
#[qapi(command)]
#[qapi(name = "guest-get-devices")]
#[qapi(returns = "Vec<GuestDeviceInfo>")]
#[qapi(condition = "CONFIG_WIN32")]
#[qapi(added = "5.2.0")]
pub struct GuestGetDevices {}

#[qapi(command)]
#[qapi(name = "guest-get-disks")]
#[qapi(returns = "Vec<GuestDiskInfo>")]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_LIBUDEV)")]
#[qapi(added = "5.2.0")]
pub struct GuestGetDisks {}

#[qapi(command)]
#[qapi(name = "guest-get-diskstats")]
#[qapi(returns = "Vec<GuestDiskStatsInfo>")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "7.1.0")]
pub struct GuestGetDiskstats {}

#[qapi(command)]
#[qapi(name = "guest-get-fsinfo")]
#[qapi(returns = "Vec<GuestFilesystemInfo>")]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_LINUX)")]
#[qapi(added = "2.2.0")]
pub struct GuestGetFsinfo {}

#[qapi(command)]
#[qapi(name = "guest-get-host-name")]
#[qapi(returns = "GuestHostName")]
#[qapi(added = "2.10.0")]
pub struct GuestGetHostName {}

#[qapi(command)]
#[qapi(name = "guest-get-load")]
#[qapi(condition = "CONFIG_GETLOADAVG")]
#[qapi(returns = "GuestLoadAverage")]
#[qapi(added = "10.0.0")]
pub struct GuestGetLoad {}

/// Get information relating to guest memory blocks.
///
#[qapi(command)]
#[qapi(name = "guest-get-memory-block-info")]
#[qapi(returns = "GuestMemoryBlockInfo")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "2.3.0")]
pub struct GuestGetMemoryBlockInfo {}

#[qapi(command)]
#[qapi(name = "guest-get-memory-blocks")]
#[qapi(returns = "Vec<GuestMemoryBlock>")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "2.3.0")]
pub struct GuestGetMemoryBlocks {}

/// Retrieve guest operating system information
///
#[qapi(command)]
#[qapi(name = "guest-get-osinfo")]
#[qapi(returns = "GuestOsInfo")]
#[qapi(added = "2.10.0")]
pub struct GuestGetOsinfo {}

#[qapi(command)]
#[qapi(name = "guest-get-time")]
#[qapi(returns = "i64")]
#[qapi(added = "1.5.0")]
pub struct GuestGetTime {}

/// Retrieves the timezone information from the guest.
///
#[qapi(command)]
#[qapi(name = "guest-get-timezone")]
#[qapi(returns = "GuestTimezone")]
#[qapi(added = "2.10.0")]
pub struct GuestGetTimezone {}

#[qapi(command)]
#[qapi(name = "guest-get-users")]
#[qapi(returns = "Vec<GuestUser>")]
#[qapi(condition = "(CONFIG_WIN32 || HAVE_UTMPX)")]
#[qapi(added = "2.10.0")]
pub struct GuestGetUsers {}

#[qapi(command)]
#[qapi(name = "guest-get-vcpus")]
#[qapi(returns = "Vec<GuestLogicalProcessor>")]
#[qapi(condition = "(CONFIG_LINUX || CONFIG_WIN32)")]
#[qapi(added = "1.5.0")]
pub struct GuestGetVcpus {}

/// Get some information about the guest agent.
///
#[qapi(command)]
#[qapi(name = "guest-info")]
#[qapi(returns = "GuestAgentInfo")]
#[qapi(added = "1.4.0")]
pub struct GuestInfo {}

/// Get list of guest IP addresses, MAC addresses and netmasks.
///
#[qapi(command)]
#[qapi(name = "guest-network-get-interfaces")]
#[qapi(returns = "Vec<GuestNetworkInterface>")]
#[qapi(condition = "(CONFIG_WIN32 || HAVE_GETIFADDRS)")]
#[qapi(added = "1.4.0")]
pub struct GuestNetworkGetInterfaces {}

#[qapi(command)]
#[qapi(name = "guest-network-get-route")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(returns = "Vec<GuestNetworkRoute>")]
#[qapi(added = "9.1.0")]
pub struct GuestNetworkGetRoute {}

/// Ping the guest agent, a non-error return implies success
///
#[qapi(command)]
#[qapi(name = "guest-ping")]
#[qapi(returns = "()")]
#[qapi(added = "1.4.0")]
pub struct GuestPing {}

#[qapi(command)]
#[qapi(name = "guest-set-memory-blocks")]
#[qapi(returns = "Vec<GuestMemoryBlockResponse>")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "2.3.0")]
pub struct GuestSetMemoryBlocks {
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
///
#[qapi(command)]
#[qapi(name = "guest-set-time")]
#[qapi(returns = "()")]
#[qapi(added = "1.5.0")]
pub struct GuestSetTime {
    /// time of nanoseconds, relative to the Epoch of 1970-01-01 in
    /// UTC.
    pub time: Option<i64>,
}

#[qapi(command)]
#[qapi(name = "guest-set-user-password")]
#[qapi(returns = "()")]
#[qapi(condition = "(CONFIG_WIN32 || CONFIG_LINUX || CONFIG_FREEBSD)")]
#[qapi(added = "2.3.0")]
pub struct GuestSetUserPassword {
    /// true if password is already crypt()d, false if raw
    ///
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
    pub crypted: bool,
    /// the new password entry string, base64 encoded
    pub password: String,
    /// the user account whose password to change
    pub username: String,
}

#[qapi(command)]
#[qapi(name = "guest-set-vcpus")]
#[qapi(returns = "i64")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "1.5.0")]
pub struct GuestSetVcpus {
    pub vcpus: Vec<GuestLogicalProcessor>,
}

/// Initiate guest-activated shutdown.  Note: this is an asynchronous
/// shutdown request, with no guarantee of successful shutdown.
///
#[qapi(command)]
#[qapi(name = "guest-shutdown")]
#[qapi(returns = "()")]
#[qapi(added = "1.4.0")]
#[qapi(no_success_response(added = "2.4.0"))]
pub struct GuestShutdown {
    /// "halt", "powerdown" (default), or "reboot"
    ///
    /// This command does NOT return a response on success.  Success
    /// condition is indicated by the VM exiting with a zero exit status or,
    /// when running with --no-shutdown, by issuing the `query-status` QMP
    /// command to confirm the VM status is "shutdown".
    pub mode: Option<String>,
}

/// Append public keys to user .ssh/authorized_keys on Unix systems (not
/// implemented for other systems).
///
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
///
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
///
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

#[qapi(command)]
#[qapi(name = "guest-suspend-disk")]
#[qapi(returns = "()")]
#[qapi(condition = "(CONFIG_LINUX || CONFIG_WIN32)")]
#[qapi(added = "1.4.0")]
#[qapi(no_success_response(added = "2.4.0"))]
pub struct GuestSuspendDisk {}

#[qapi(command)]
#[qapi(name = "guest-suspend-hybrid")]
#[qapi(returns = "()")]
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "1.4.0")]
#[qapi(no_success_response(added = "2.4.0"))]
pub struct GuestSuspendHybrid {}

#[qapi(command)]
#[qapi(name = "guest-suspend-ram")]
#[qapi(returns = "()")]
#[qapi(condition = "(CONFIG_LINUX || CONFIG_WIN32)")]
#[qapi(added = "1.4.0")]
#[qapi(no_success_response(added = "2.4.0"))]
pub struct GuestSuspendRam {}

#[qapi(command)]
#[qapi(name = "guest-sync")]
#[qapi(returns = "i64")]
#[qapi(added = "1.4.0")]
pub struct GuestSync {
    pub id: i64,
}

#[qapi(command)]
#[qapi(name = "guest-sync-delimited")]
#[qapi(returns = "i64")]
#[qapi(added = "1.4.0")]
pub struct GuestSyncDelimited {
    pub id: i64,
}

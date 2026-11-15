#![allow(unused_imports, non_camel_case_types, dead_code)]
use crate as qapi;
use qapi_macros::qapi;
use std::collections::HashMap;
use super::*;
/// Allow client connections for VNC, Spice and socket based character
/// devices to be passed in to QEMU via SCM_RIGHTS.
///
/// If the FD associated with @fdname is not a socket, the command will
/// fail and the FD will be closed.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "add_client", "arguments": { "protocol": "vnc",
///                                                  "fdname": "myclient" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "add_client")]
#[qapi(returns = "()")]
#[qapi(added = "1.3.0")]
pub struct AddClient {
    /// file descriptor name previously passed via `getfd` command
    pub fdname: String,
    /// protocol name.  Valid names are "vnc", "spice",
    /// "@dbus-display" or the name of a character device (e.g. from
    /// -chardev id=XXXX)
    pub protocol: String,
    /// whether to skip authentication.  Only applies to "vnc"
    /// and "spice" protocols
    pub skipauth: Option<bool>,
    /// whether to perform TLS.  Only applies to the "spice" protocol
    pub tls: Option<bool>,
}

/// Add a file descriptor, that was passed via SCM rights, to an fd set.
///
/// Errors:
/// ```text
///     - If file descriptor was not received, GenericError
///     - If @fdset-id is a negative value, GenericError
/// ```
///
/// .. note:: The list of fd sets is shared by all monitor connections.
///
/// .. note:: If @fdset-id is not specified, a new fd set will be
/// ```text
///    created.
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "add-fd", "arguments": { "fdset-id": 1 } }
///     <- { "return": { "fdset-id": 1, "fd": 3 } }
/// ```
#[qapi(command)]
#[qapi(name = "add-fd")]
#[qapi(returns = "AddfdInfo")]
#[qapi(added = "1.2.0")]
pub struct AddFd {
    /// The ID of the fd set to add the file descriptor to.
    #[qapi(name = "fdset-id")]
    pub fdset_id: Option<i64>,
    /// A free-form string that can be used to describe the fd.
    pub opaque: Option<String>,
}

/// Trigger generation of broadcast RARP frames to update network
/// switches.  This can be useful when network bonds fail-over the
/// active slave.
///
/// TODO: This line is a hack to separate the example from the body
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "announce-self",
///          "arguments": {
///              "initial": 50, "max": 550, "rounds": 10, "step": 50,
///              "interfaces": ["vn2", "vn3"], "id": "bob" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "announce-self")]
#[qapi(returns = "()")]
#[qapi(added = "4.0.0")]
pub struct AnnounceSelf {
    /// A name to be used to identify an instance of announce-timers
    /// and to allow it to modified later.  Not for use as part of the
    /// migration parameters.  (Since 4.1)
    #[qapi(added = "4.1.0")]
    pub id: Option<String>,
    /// Initial delay (in ms) before sending the first GARP/RARP
    /// announcement
    pub initial: i64,
    /// An optional list of interface names, which restricts
    /// the announcement to the listed interfaces.  (Since 4.1)
    #[qapi(added = "4.1.0")]
    pub interfaces: Option<Vec<String>>,
    /// Maximum delay (in ms) between GARP/RARP announcement packets
    pub max: i64,
    /// Number of self-announcement attempts
    pub rounds: i64,
    /// Delay increase (in ms) after each self-announcement attempt
    pub step: i64,
}

/// Request the balloon driver to change its balloon size.
///
/// Errors:
/// ```text
///     - If the balloon driver is enabled but not functional because
///       the KVM kernel module cannot support it, KVMMissingCap
///     - If no balloon device is present, DeviceNotActive
/// ```
///
/// .. note:: This command just issues a request to the guest.  When it
/// ```text
///    returns, the balloon size may not have changed.  A guest can
///    change the balloon size independent of this command.
/// ```
///
///
/// .. qmp-example::
/// ```text
///    :annotated:
/// ```
///
/// ```text
///    ::
/// ```
///
/// ```text
///      -> { "execute": "balloon", "arguments": { "value": 536870912 } }
///      <- { "return": {} }
/// ```
///
/// ```text
///    With a 2.5GiB guest this command inflated the ballon to 3GiB.
/// ```
#[qapi(command)]
#[qapi(name = "balloon")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct Balloon {
    /// the target logical size of the VM in bytes.  We can deduce
    /// the size of the balloon using this formula:
    ///
    /// ```text
    ///     logical_vm_size = vm_ram_size - balloon_size
    /// ```
    ///
    /// From it we have: balloon_size = vm_ram_size - @value
    pub value: i64,
}

/// Live commit of data from overlay image nodes into backing nodes -
/// i.e., writes data between 'top' and 'base' into 'base'.
///
/// If top == base, that is an error.  If top has no overlays on top of
/// it, or if it is in use by a writer, the job will not be completed by
/// itself.  The user needs to complete the job with the `job-complete`
/// command after getting the ready event.  (Since 2.0)
///
/// If the base image is smaller than top, then the base image will be
/// resized to be the same size as top.  If top is smaller than the base
/// image, the base will not be truncated.  If you want the base image
/// size to match the size of the smaller top, you can safely truncate
/// it yourself once the commit operation successfully completes.
///
/// Features:
///
/// @deprecated: Members @base and @top are deprecated.  Use @base-node
/// ```text
///     and @top-node instead.
/// ```
///
/// Errors:
/// ```text
///     - If @device does not exist, DeviceNotFound
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "block-commit",
///          "arguments": { "device": "virtio0",
///                         "top": "/tmp/snap1.qcow2" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "block-commit")]
#[qapi(returns = "()")]
#[qapi(added = "1.3.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockCommit {
    /// When false, this job will wait in a CONCLUDED state
    /// after it has completely ceased all work, and awaits
    /// `job-dismiss`.  When true, this job will automatically disappear
    /// without user intervention.  Defaults to true.  (Since 3.1)
    #[qapi(name = "auto-dismiss")]
    #[qapi(added = "3.1.0")]
    pub auto_dismiss: Option<bool>,
    /// When false, this job will wait in a PENDING state
    /// after it has finished its work, waiting for `job-finalize`
    /// before making any block graph changes.  When true, this job will
    /// automatically perform its abort or commit actions.  Defaults to
    /// true.  (Since 3.1)
    #[qapi(name = "auto-finalize")]
    #[qapi(added = "3.1.0")]
    pub auto_finalize: Option<bool>,
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
    #[qapi(added = "2.1.0")]
    pub backing_file: Option<String>,
    /// If true, replace any protocol mentioned in
    /// the 'backing file format' with 'raw', rather than storing the
    /// protocol name as the backing format.  Can be used even when no
    /// image header will be updated (default false; since 9.0).
    #[qapi(name = "backing-mask-protocol")]
    #[qapi(added = "9.0.0")]
    pub backing_mask_protocol: Option<bool>,
    /// Same as @base-node, except that it is a file name rather than
    /// a node name.  This must be the exact filename string that was
    /// used to open the node; other strings, even if addressing the
    /// same file, are not accepted
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "5.0.0"))]
    pub base: Option<String>,
    /// The node name of the backing image to write data into.
    /// If not specified, this is the deepest backing image.
    /// (since: 3.1)
    #[qapi(name = "base-node")]
    #[qapi(added = "3.1.0")]
    pub base_node: Option<String>,
    /// the device name or node-name of a root node
    pub device: String,
    /// the node name that should be assigned to the
    /// filter driver that the commit job inserts into the graph above
    /// @top.  If this option is not given, a node name is
    /// autogenerated.  (Since: 2.9)
    #[qapi(name = "filter-node-name")]
    #[qapi(added = "2.9.0")]
    pub filter_node_name: Option<String>,
    /// identifier for the newly-created block job.  If omitted,
    /// the device name will be used.  (Since 2.7)
    #[qapi(name = "job-id")]
    #[qapi(added = "2.7.0")]
    pub job_id: Option<String>,
    /// the action to take on an error.  'ignore' means that the
    /// request should be retried.  (default: report; since: 5.0)
    #[qapi(name = "on-error")]
    #[qapi(added = "5.0.0")]
    pub on_error: Option<BlockdevOnError>,
    /// the maximum speed, in bytes per second
    pub speed: Option<i64>,
    /// Same as @top-node, except that it is a file name rather than a
    /// node name.  This must be the exact filename string that was used
    /// to open the node; other strings, even if addressing the same
    /// file, are not accepted
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "5.0.0"))]
    pub top: Option<String>,
    /// The node name of the backing image within the image chain
    /// which contains the topmost data to be committed down.  If not
    /// specified, this is the active layer.  (since: 3.1)
    #[qapi(name = "top-node")]
    #[qapi(added = "3.1.0")]
    pub top_node: Option<String>,
}

/// Creates a new block device.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "blockdev-add",
///          "arguments": {
///               "driver": "qcow2",
///               "node-name": "test1",
///               "file": {
///                   "driver": "file",
///                   "filename": "test.qcow2"
///                }
///           }
///         }
///     <- { "return": {} }
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "blockdev-add",
///          "arguments": {
///               "driver": "qcow2",
///               "node-name": "node0",
///               "discard": "unmap",
///               "cache": {
///                  "direct": true
///                },
///                "file": {
///                  "driver": "file",
///                  "filename": "/tmp/test.qcow2"
///                },
///                "backing": {
///                   "driver": "raw",
///                   "file": {
///                      "driver": "file",
///                      "filename": "/dev/fdset/4"
///                    }
///                }
///            }
///          }
/// ```
///
/// ```text
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "blockdev-add")]
#[qapi(returns = "()")]
#[qapi(added = "1.7.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockdevAdd {
    /// whether the block node should be activated (default: true).
    /// Having inactive block nodes is useful primarily for migration
    /// because it allows opening an image on the destination while the
    /// source is still holding locks for it.  (Since 10.0)
    #[qapi(added = "10.0.0")]
    pub active: Option<bool>,
    /// if true and @read-only is false, QEMU may
    /// automatically decide not to open the image read-write as
    /// requested, but fall back to read-only instead (and switch
    /// between the modes later), e.g. depending on whether the image
    /// file is writable or whether a writing user is attached to the
    /// node (default: false, since 3.1)
    #[qapi(name = "auto-read-only")]
    #[qapi(added = "3.1.0")]
    pub auto_read_only: Option<bool>,
    /// cache-related options
    #[qapi(added = "2.8.0")]
    pub cache: Option<BlockdevCacheOptions>,
    /// detect and optimize zero writes (Since 2.1)
    /// (default: off)
    #[qapi(name = "detect-zeroes")]
    #[qapi(added = "2.8.0")]
    pub detect_zeroes: Option<BlockdevDetectZeroesOptions>,
    /// discard-related options (default: ignore)
    #[qapi(added = "2.8.0")]
    pub discard: Option<BlockdevDiscardOptions>,
    /// block driver name
    #[qapi(discriminator)]
    #[qapi(added = "2.8.0")]
    #[qapi(required)]
    pub driver: Option<BlockdevDriver>,
    /// force share all permission on added nodes.  Requires
    /// read-only=true.  (Since 2.10)
    #[qapi(name = "force-share")]
    #[qapi(added = "2.10.0")]
    pub force_share: Option<bool>,
    /// the node name of the new node.  This option is required
    /// on the top level of `blockdev-add`.  Valid node names start with
    /// an alphabetic character and may contain only alphanumeric
    /// characters, '-', '.' and '_'.  Their maximum length is 31
    /// characters.  (Since 2.0)
    #[qapi(name = "node-name")]
    #[qapi(added = "2.8.0")]
    pub node_name: Option<String>,
    /// block device options for the new device
    #[qapi(removed = "2.8.0")]
    #[qapi(required)]
    pub options: Option<BlockdevOptions>,
    /// whether the block device should be read-only (default:
    /// false).  Note that some block drivers support only read-only
    /// access, either generally or in certain configurations.  In this
    /// case, the default value does not work and the option must be
    /// specified explicitly.
    #[qapi(name = "read-only")]
    #[qapi(added = "2.8.0")]
    pub read_only: Option<bool>,
    #[qapi(union)]
    #[qapi(flatten)]
    #[qapi(added = "2.8.0")]
    pub u: Option<BlockdevOptionsBranch>,
}

/// Changes the medium inserted into a block device by ejecting the
/// current medium and loading a new image file which is inserted as the
/// new medium (this command combines `blockdev-open-tray`,
/// `blockdev-remove-medium`, `blockdev-insert-medium` and
/// `blockdev-close-tray`).
///
/// Features:
///
/// @deprecated: Member @device is deprecated.  Use @id instead.
///
///
/// .. qmp-example::
/// ```text
///    :title: Change a removable medium
/// ```
///
/// ```text
///     -> { "execute": "blockdev-change-medium",
///          "arguments": { "id": "ide0-1-0",
///                         "filename": "/srv/images/Fedora-12-x86_64-DVD.iso",
///                         "format": "raw" } }
///     <- { "return": {} }
/// ```
///
/// .. qmp-example::
/// ```text
///    :title: Load a read-only medium into a writable drive
/// ```
///
/// ```text
///     -> { "execute": "blockdev-change-medium",
///          "arguments": { "id": "floppyA",
///                         "filename": "/srv/images/ro.img",
///                         "format": "raw",
///                         "read-only-mode": "retain" } }
/// ```
///
/// ```text
///     <- { "error":
///          { "class": "GenericError",
///            "desc": "Could not open '/srv/images/ro.img': Permission denied" } }
/// ```
///
/// ```text
///     -> { "execute": "blockdev-change-medium",
///          "arguments": { "id": "floppyA",
///                         "filename": "/srv/images/ro.img",
///                         "format": "raw",
///                         "read-only-mode": "read-only" } }
/// ```
///
/// ```text
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "blockdev-change-medium")]
#[qapi(returns = "()")]
#[qapi(added = "2.5.0")]
pub struct BlockdevChangeMedium {
    /// Block device name
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "5.0.0"))]
    pub device: Option<String>,
    /// filename of the new image to be loaded
    pub filename: String,
    /// if false (the default), an eject request through
    /// `blockdev-open-tray` will be sent to the guest if it has locked
    /// the tray (and the tray will not be opened immediately); if true,
    /// the tray will be opened regardless of whether it is locked.
    /// (since 7.1)
    #[qapi(added = "7.1.0")]
    pub force: Option<bool>,
    /// format to open the new image with (defaults to the probed
    /// format)
    pub format: Option<String>,
    /// The name or QOM path of the guest device (since: 2.8)
    #[qapi(added = "2.8.0")]
    pub id: Option<String>,
    /// change the read-only mode of the device; defaults
    /// to 'retain'
    #[qapi(name = "read-only-mode")]
    pub read_only_mode: Option<BlockdevChangeReadOnlyMode>,
}

/// Closes a block device's tray.  If there is a block driver state tree
/// associated with the block device (which is currently ejected), that
/// tree will be loaded as the medium.
///
/// If the tray was already closed before, this will be a no-op.
///
/// Features:
///
/// @deprecated: Member @device is deprecated.  Use @id instead.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "blockdev-close-tray",
///          "arguments": { "id": "ide0-1-0" } }
/// ```
///
/// ```text
///     <- { "timestamp": { "seconds": 1418751345,
///                         "microseconds": 272147 },
///          "event": "DEVICE_TRAY_MOVED",
///          "data": { "device": "ide1-cd0",
///                    "id": "ide0-1-0",
///                    "tray-open": false } }
/// ```
///
/// ```text
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "blockdev-close-tray")]
#[qapi(returns = "()")]
#[qapi(added = "2.5.0")]
pub struct BlockdevCloseTray {
    /// Block device name
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "5.0.0"))]
    pub device: Option<String>,
    /// The name or QOM path of the guest device (since: 2.8)
    #[qapi(added = "2.8.0")]
    pub id: Option<String>,
}

/// Starts a job to create an image format on a given node.  The job is
/// automatically finalized, but a manual `job-dismiss` is required.
#[qapi(command)]
#[qapi(name = "blockdev-create")]
#[qapi(returns = "()")]
#[qapi(added = "3.0.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockdevCreate {
    /// Identifier for the newly created job.
    #[qapi(name = "job-id")]
    pub job_id: String,
    /// Options for the image creation.
    pub options: BlockdevCreateOptions,
}

/// Deletes a block device that has been added using `blockdev-add`.
/// The command will fail if the node is attached to a device or is
/// otherwise being used.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "blockdev-add",
///          "arguments": {
///               "driver": "qcow2",
///               "node-name": "node0",
///               "file": {
///                   "driver": "file",
///                   "filename": "test.qcow2"
///               }
///          }
///        }
///     <- { "return": {} }
/// ```
///
/// ```text
///     -> { "execute": "blockdev-del",
///          "arguments": { "node-name": "node0" }
///        }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "blockdev-del")]
#[qapi(returns = "()")]
#[qapi(added = "2.9.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockdevDel {
    /// Name of the graph node to delete.
    #[qapi(name = "node-name")]
    pub node_name: String,
}

/// Inserts a medium (a block driver state tree) into a block device.
/// That block device's tray must currently be open (unless there is no
/// attached guest device) and there must be no medium inserted already.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "blockdev-add",
///          "arguments": {
///              "node-name": "node0",
///              "driver": "raw",
///              "file": { "driver": "file",
///                        "filename": "fedora.iso" } } }
///     <- { "return": {} }
/// ```
///
/// ```text
///     -> { "execute": "blockdev-insert-medium",
///          "arguments": { "id": "ide0-1-0",
///                         "node-name": "node0" } }
/// ```
///
/// ```text
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "blockdev-insert-medium")]
#[qapi(returns = "()")]
#[qapi(added = "2.12.0")]
pub struct BlockdevInsertMedium {
    /// The name or QOM path of the guest device
    pub id: String,
    /// name of a node in the block driver state graph
    #[qapi(name = "node-name")]
    pub node_name: String,
}

/// Start mirroring a block device's writes to a new destination.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "blockdev-mirror",
///          "arguments": { "device": "ide-hd0",
///                         "target": "target0",
///                         "sync": "full" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "blockdev-mirror")]
#[qapi(returns = "()")]
#[qapi(added = "2.6.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockdevMirror {
    /// When false, this job will wait in a CONCLUDED state
    /// after it has completely ceased all work, and awaits
    /// `job-dismiss`.  When true, this job will automatically disappear
    /// without user intervention.  Defaults to true.  (Since 3.1)
    #[qapi(name = "auto-dismiss")]
    #[qapi(added = "3.1.0")]
    pub auto_dismiss: Option<bool>,
    /// When false, this job will wait in a PENDING state
    /// after it has finished its work, waiting for `job-finalize`
    /// before making any block graph changes.  When true, this job will
    /// automatically perform its abort or commit actions.  Defaults to
    /// true.  (Since 3.1)
    #[qapi(name = "auto-finalize")]
    #[qapi(added = "3.1.0")]
    pub auto_finalize: Option<bool>,
    /// maximum amount of data in flight from source to target
    #[qapi(name = "buf-size")]
    pub buf_size: Option<i64>,
    /// when to copy data to the destination; defaults to
    /// 'background' (Since: 3.0)
    #[qapi(name = "copy-mode")]
    #[qapi(added = "3.0.0")]
    pub copy_mode: Option<MirrorCopyMode>,
    /// The device name or node-name of a root node whose writes
    /// should be mirrored.
    pub device: String,
    /// the node name that should be assigned to the
    /// filter driver that the mirror job inserts into the graph above
    /// @device.  If this option is not given, a node name is
    /// autogenerated.  (Since: 2.9)
    #[qapi(name = "filter-node-name")]
    #[qapi(added = "2.9.0")]
    pub filter_node_name: Option<String>,
    /// granularity of the dirty bitmap, default is 64K if the
    /// image format doesn't have clusters, 4K if the clusters are
    /// smaller than that, else the cluster size.  Must be a power of 2
    /// between 512 and 64M
    pub granularity: Option<u32>,
    /// identifier for the newly-created block job.  If omitted,
    /// the device name will be used.  (Since 2.7)
    #[qapi(name = "job-id")]
    #[qapi(added = "2.7.0")]
    pub job_id: Option<String>,
    /// the action to take on an error on the source,
    /// default 'report'.  'stop' and 'enospc' can only be used if the
    /// block device supports io-status (see `BlockInfo`).
    #[qapi(name = "on-source-error")]
    pub on_source_error: Option<BlockdevOnError>,
    /// the action to take on an error on the target,
    /// default 'report' (no limitations, since this applies to a
    /// different block device than @device).
    #[qapi(name = "on-target-error")]
    pub on_target_error: Option<BlockdevOnError>,
    /// with sync=full graph node name to be replaced by the new
    /// image when a whole image copy is done.  This can be used to
    /// repair broken Quorum files.  By default, @device is replaced,
    /// although implicitly created filters on it are kept.
    pub replaces: Option<String>,
    /// the maximum speed, in bytes per second
    pub speed: Option<i64>,
    /// what parts of the disk image should be copied to the
    /// destination (all the disk, only the sectors allocated in the
    /// topmost image, or only new I/O).
    pub sync: MirrorSyncMode,
    /// the id or node-name of the block device to mirror to.  This
    /// mustn't be attached to guest.
    pub target: String,
    /// Assume the destination reads as all zeroes before
    /// the mirror started.  Setting this to true can speed up the
    /// mirror.  Setting this to true when the destination is not
    /// actually all zero can corrupt the destination.  (Since 10.1)
    #[qapi(name = "target-is-zero")]
    #[qapi(added = "10.1.0")]
    pub target_is_zero: Option<bool>,
}

/// Opens a block device's tray.  If there is a block driver state tree
/// inserted as a medium, it will become inaccessible to the guest (but
/// it will remain associated to the block device, so closing the tray
/// will make it accessible again).
///
/// If the tray was already open before, this will be a no-op.
///
/// Once the tray opens, a `DEVICE_TRAY_MOVED` event is emitted.  There
/// are cases in which no such event will be generated, these include:
///
/// - if the guest has locked the tray, @force is false and the guest
///   does not respond to the eject request
/// - if the BlockBackend denoted by @device does not have a guest
///   device attached to it
/// - if the guest device does not have an actual tray
///
/// Features:
///
/// @deprecated: Member @device is deprecated.  Use @id instead.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "blockdev-open-tray",
///          "arguments": { "id": "ide0-1-0" } }
/// ```
///
/// ```text
///     <- { "timestamp": { "seconds": 1418751016,
///                         "microseconds": 716996 },
///          "event": "DEVICE_TRAY_MOVED",
///          "data": { "device": "ide1-cd0",
///                    "id": "ide0-1-0",
///                    "tray-open": true } }
/// ```
///
/// ```text
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "blockdev-open-tray")]
#[qapi(returns = "()")]
#[qapi(added = "2.5.0")]
pub struct BlockdevOpenTray {
    /// Block device name
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "5.0.0"))]
    pub device: Option<String>,
    /// if false (the default), an eject request will be sent to the
    /// guest if it has locked the tray (and the tray will not be opened
    /// immediately); if true, the tray will be opened regardless of
    /// whether it is locked
    pub force: Option<bool>,
    /// The name or QOM path of the guest device (since: 2.8)
    #[qapi(added = "2.8.0")]
    pub id: Option<String>,
}

/// Removes a medium (a block driver state tree) from a block device.
/// That block device's tray must currently be open (unless there is no
/// attached guest device).
///
/// If the tray is open and there is no medium inserted, this will be a
/// no-op.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "blockdev-remove-medium",
///          "arguments": { "id": "ide0-1-0" } }
/// ```
///
/// ```text
///     <- { "error": { "class": "GenericError",
///                     "desc": "Tray of device 'ide0-1-0' is not open" } }
/// ```
///
/// ```text
///     -> { "execute": "blockdev-open-tray",
///          "arguments": { "id": "ide0-1-0" } }
/// ```
///
/// ```text
///     <- { "timestamp": { "seconds": 1418751627,
///                         "microseconds": 549958 },
///          "event": "DEVICE_TRAY_MOVED",
///          "data": { "device": "ide1-cd0",
///                    "id": "ide0-1-0",
///                    "tray-open": true } }
/// ```
///
/// ```text
///     <- { "return": {} }
/// ```
///
/// ```text
///     -> { "execute": "blockdev-remove-medium",
///          "arguments": { "id": "ide0-1-0" } }
/// ```
///
/// ```text
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "blockdev-remove-medium")]
#[qapi(returns = "()")]
#[qapi(added = "2.12.0")]
pub struct BlockdevRemoveMedium {
    /// The name or QOM path of the guest device
    pub id: String,
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
/// one of them, the top-level @node-name option (from
/// `BlockdevOptions`) must be specified and is used to select the block
/// device to be reopened.  Other @node-name options must be either
/// omitted or set to the current name of the appropriate node.  This
/// command won't change any node name and any attempt to do it will
/// result in an error.
///
/// In the case of options that refer to child nodes, the behavior of
/// this command depends on the value:
///
///  1) A set of options (`BlockdevOptions`): the child is reopened with
/// ```text
///     the specified set of options.
/// ```
///
///  2) A reference to the current child: the child is reopened using
/// ```text
///     its existing set of options.
/// ```
///
///  3) A reference to a different node: the current child is replaced
/// ```text
///     with the specified one.
/// ```
///
///  4) null: the current child (if any) is detached.
///
/// Options (1) and (2) are supported in all cases.  Option (3) is
/// supported for @file and @backing, and option (4) for @backing only.
///
/// Unlike with `blockdev-add`, the @backing option must always be
/// present unless the node being reopened does not have a backing file
/// and its image does not have a default backing file name as part of
/// its metadata.
#[qapi(command)]
#[qapi(name = "blockdev-reopen")]
#[qapi(returns = "()")]
#[qapi(added = "6.1.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockdevReopen {
    pub options: Vec<BlockdevOptions>,
}

/// Activate or deactivate a block device.  Use this to manage the
/// handover of block devices on migration with qemu-storage-daemon.
///
/// Activating a node automatically activates all of its child nodes
/// first.  Deactivating a node automatically deactivates any of its
/// child nodes that are not in use by a still active node.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "blockdev-set-active",
///          "arguments": {
///               "node-name": "node0",
///               "active": false
///          }
///        }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "blockdev-set-active")]
#[qapi(returns = "()")]
#[qapi(added = "10.0.0")]
#[qapi(allow_preconfig)]
pub struct BlockdevSetActive {
    /// true if the nodes should be active when the command returns
    /// success, false if they should be inactive.
    pub active: bool,
    /// Name of the graph node to activate or deactivate.  By
    /// default, all nodes are affected by the operation.
    #[qapi(name = "node-name")]
    pub node_name: Option<String>,
}

/// Synchronously delete an internal snapshot of a block device, when
/// the format of the image used support it.  The snapshot is identified
/// by name or id or both.  One of the name or id is required.  Return
/// `SnapshotInfo` for the successfully deleted snapshot.
///
/// Errors:
/// ```text
///     - If @device is not a valid block device, GenericError
///     - If snapshot not found, GenericError
///     - If the format of the image used does not support it,
///       GenericError
///     - If @id and @name are both not specified, GenericError
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "blockdev-snapshot-delete-internal-sync",
///          "arguments": { "device": "ide-hd0",
///                         "name": "snapshot0" }
///        }
///     <- { "return": {
///                        "id": "1",
///                        "name": "snapshot0",
///                        "vm-state-size": 0,
///                        "date-sec": 1000012,
///                        "date-nsec": 10,
///                        "vm-clock-sec": 100,
///                        "vm-clock-nsec": 20,
///                        "icount": 220414
///          }
///        }
/// ```
#[qapi(command)]
#[qapi(name = "blockdev-snapshot-delete-internal-sync")]
#[qapi(returns = "SnapshotInfo")]
#[qapi(added = "1.7.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockdevSnapshotDeleteInternalSync {
    /// the device name or node-name of a root node to delete the
    /// snapshot from
    pub device: String,
    /// optional the snapshot's ID to be deleted
    pub id: Option<String>,
    /// optional the snapshot's name to be deleted
    pub name: Option<String>,
}

/// Synchronously take an internal snapshot of a block device, when the
/// format of the image used supports it.  If the name is an empty
/// string, or a snapshot with name already exists, the operation will
/// fail.
///
/// Errors:
/// ```text
///     - If @device is not a valid block device, GenericError
///     - If any snapshot matching @name exists, or @name is empty,
///       GenericError
///     - If the format of the image used does not support it,
///       GenericError
/// ```
///
/// .. note:: Only some image formats such as qcow2 and rbd support
/// ```text
///    internal snapshots.
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "blockdev-snapshot-internal-sync",
///          "arguments": { "device": "ide-hd0",
///                         "name": "snapshot0" }
///        }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "blockdev-snapshot-internal-sync")]
#[qapi(returns = "()")]
#[qapi(added = "1.7.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockdevSnapshotInternalSync {
    /// the device name or node-name of a root node to generate the
    /// snapshot from
    pub device: String,
    /// the name of the internal snapshot to be created
    pub name: String,
}

/// Clear (reset) a dirty bitmap on the device, so that an incremental
/// backup from this point in time forward will only backup clusters
/// modified after this clear operation.
///
/// Errors:
/// ```text
///     - If @node is not a valid block device, DeviceNotFound
///     - If @name is not found, GenericError
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "block-dirty-bitmap-clear",
///          "arguments": { "node": "drive0", "name": "bitmap0" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "block-dirty-bitmap-clear")]
#[qapi(returns = "()")]
#[qapi(added = "2.4.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockDirtyBitmapClear {
    /// name of the dirty bitmap
    pub name: String,
    /// name of device/node which the bitmap is tracking
    pub node: String,
}

/// Disables a dirty bitmap so that it will stop tracking disk changes.
///
/// Errors:
/// ```text
///     - If @node is not a valid block device, DeviceNotFound
///     - If @name is not found, GenericError
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "block-dirty-bitmap-disable",
///          "arguments": { "node": "drive0", "name": "bitmap0" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "block-dirty-bitmap-disable")]
#[qapi(returns = "()")]
#[qapi(added = "4.0.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockDirtyBitmapDisable {
    /// name of the dirty bitmap
    pub name: String,
    /// name of device/node which the bitmap is tracking
    pub node: String,
}

/// Enables a dirty bitmap so that it will begin tracking disk changes.
///
/// Errors:
/// ```text
///     - If @node is not a valid block device, DeviceNotFound
///     - If @name is not found, GenericError
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "block-dirty-bitmap-enable",
///          "arguments": { "node": "drive0", "name": "bitmap0" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "block-dirty-bitmap-enable")]
#[qapi(returns = "()")]
#[qapi(added = "4.0.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockDirtyBitmapEnable {
    /// name of the dirty bitmap
    pub name: String,
    /// name of device/node which the bitmap is tracking
    pub node: String,
}

/// Stop write tracking and remove the dirty bitmap that was created
/// with `block-dirty-bitmap-add`.  If the bitmap is persistent, remove
/// it from its storage too.
///
/// Errors:
/// ```text
///     - If @node is not a valid block device or node, DeviceNotFound
///     - If @name is not found, GenericError
///     - if @name is frozen by an operation, GenericError
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "block-dirty-bitmap-remove",
///          "arguments": { "node": "drive0", "name": "bitmap0" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "block-dirty-bitmap-remove")]
#[qapi(returns = "()")]
#[qapi(added = "2.4.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockDirtyBitmapRemove {
    /// name of the dirty bitmap
    pub name: String,
    /// name of device/node which the bitmap is tracking
    pub node: String,
}

/// Creates a new block export.
#[qapi(command)]
#[qapi(name = "block-export-add")]
#[qapi(returns = "()")]
#[qapi(added = "5.2.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockExportAdd {
    /// If true, the export allows the exported node to be
    /// inactive.  If it is created for an inactive block node, the node
    /// remains inactive.  If the export type doesn't support running on
    /// an inactive node, an error is returned.  If false, inactive
    /// block nodes are automatically activated before creating the
    /// export and trying to inactivate them later fails.
    /// (since: 10.0; default: false)
    #[qapi(name = "allow-inactive")]
    #[qapi(added = "10.0.0")]
    pub allow_inactive: Option<bool>,
    /// True prevents the block node from being moved to
    /// another thread while the export is active.  If true and
    /// @iothread is given, export creation fails if the block node
    /// cannot be moved to the iothread.  Must not be true when giving
    /// multiple iothreads for @iothread.  The default is false.
    /// (since: 5.2)
    #[qapi(name = "fixed-iothread")]
    pub fixed_iothread: Option<bool>,
    /// A unique identifier for the block export (across all export
    /// types)
    pub id: String,
    /// The name(s) of one or more iothread object(s) where the
    /// export will run.  The default is to use the thread currently
    /// associated with the block node.  (since: 5.2; multi-threading
    /// since 10.1)
    #[qapi(added = "11.0.0")]
    pub iothread: Option<BlockExportIothreads>,
    /// The name of the iothread object where the export will
    /// run.  The default is to use the thread currently associated with
    /// the block node.  (since: 5.2)
    #[qapi(name = "iothread")]
    #[qapi(removed = "11.0.0")]
    pub iothread_legacy: Option<String>,
    /// The node name of the block node to be exported
    /// (since: 5.2)
    #[qapi(name = "node-name")]
    pub node_name: String,
    /// Block export type
    #[qapi(discriminator)]
    pub r#type: BlockExportType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<BlockExportOptionsBranch>,
    /// True if clients should be able to write to the export
    /// (default false)
    pub writable: Option<bool>,
    /// If true, caches are flushed after every write request
    /// to the export before completion is signalled.  (since: 5.2;
    /// default: false)
    pub writethrough: Option<bool>,
}

/// Request to remove a block export.  This drops the user's reference
/// to the export, but the export may still stay around after this
/// command returns until the shutdown of the export has completed.
///
/// Errors:
/// ```text
///     - if the export is not found
///     - if @mode is 'safe' and the export is still in use (e.g. by
///       existing client connections)
/// ```
#[qapi(command)]
#[qapi(name = "block-export-del")]
#[qapi(returns = "()")]
#[qapi(added = "5.2.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockExportDel {
    /// Block export id.
    pub id: String,
    /// Mode of command operation.  See `BlockExportRemoveMode`
    /// description.  Default is 'safe'.
    pub mode: Option<BlockExportRemoveMode>,
}

/// Stop an active background block operation.
///
/// This command returns immediately after marking the active background
/// block operation for cancellation.  It is an error to call this
/// command if no operation is in progress.
///
/// The operation will cancel as soon as possible and then emit the
/// `BLOCK_JOB_CANCELLED` event.  Before that happens the job is still
/// visible when enumerated using `query-block-jobs`.
///
/// Note that if you issue `block-job-cancel` after `drive-mirror` has
/// indicated (via the event `BLOCK_JOB_READY`) that the source and
/// destination are synchronized, then the event triggered by this
/// command changes to `BLOCK_JOB_COMPLETED`, to indicate that the
/// mirroring has ended and the destination now has a point-in-time copy
/// tied to the time of the cancellation.
///
/// For streaming, the image file retains its backing file unless the
/// streaming operation happens to complete just as it is being
/// cancelled.  A new streaming operation can be started at a later time
/// to finish copying all data from the backing file.
///
/// Errors:
/// ```text
///     - If no background operation is active on this device,
///       DeviceNotActive
/// ```
#[qapi(command)]
#[qapi(name = "block-job-cancel")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockJobCancel {
    /// The job identifier.  This used to be a device name (hence
    /// the name of the parameter), but since QEMU 2.7 it can have other
    /// values.
    pub device: String,
    /// If true, and the job has already emitted the event
    /// `BLOCK_JOB_READY`, abandon the job immediately (even if it is
    /// paused) instead of waiting for the destination to complete its
    /// final synchronization (since 1.3)
    #[qapi(added = "1.3.0")]
    pub force: Option<bool>,
}

/// Change the block job's options.
#[qapi(command)]
#[qapi(name = "block-job-change")]
#[qapi(returns = "()")]
#[qapi(added = "8.2.0")]
pub struct BlockJobChange {
    /// The job identifier
    pub id: String,
    /// The job type
    #[qapi(discriminator)]
    pub r#type: JobType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<BlockJobChangeOptionsBranch>,
}

/// Manually trigger completion of an active job in the READY or STANDBY
/// state.  Completing the job in any other state is an error.
///
/// This is supported only for drive mirroring, where it also switches
/// the device to write to the target path only.  Note that drive
/// mirroring includes `drive-mirror`, `blockdev-mirror` and
/// `block-commit` job (only in case of "active commit", when the node
/// being committed is used by the guest).  The ability to complete is
/// signaled with a `BLOCK_JOB_READY` event.
///
/// This command completes an active background block operation
/// synchronously.  The ordering of this command's return with the
/// `BLOCK_JOB_COMPLETED` event is not defined.  Note that if an I/O
/// error occurs during the processing of this command: 1) the command
/// itself will fail; 2) the error will be processed according to the
/// rerror/werror arguments that were specified when starting the
/// operation.
///
/// Features:
///
/// @deprecated: This command is deprecated.  Use `job-complete`
/// ```text
///     instead.
/// ```
///
/// Errors:
/// ```text
///     - If no background operation is active on this device,
///       DeviceNotActive
/// ```
#[qapi(command)]
#[qapi(name = "block-job-complete")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.3.0")]
#[qapi(deprecated(added = "10.1.0"))]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockJobComplete {
    /// The job identifier.  This used to be a device name (hence
    /// the name of the parameter), but since QEMU 2.7 it can have other
    /// values.
    pub device: String,
}

/// Deletes a job that is in the CONCLUDED state.  This command only
/// needs to be run explicitly for jobs that don't have automatic
/// dismiss enabled.  In turn, automatic dismiss may be enabled only for
/// jobs that have @auto-dismiss option, which are `drive-backup`,
/// `blockdev-backup`, `drive-mirror`, `blockdev-mirror`, `block-commit`
/// and `block-stream`.  @auto-dismiss is enabled by default for these
/// jobs.
///
/// This command will refuse to operate on any job that has not yet
/// reached its terminal state, CONCLUDED.  For jobs that make use of
/// the `BLOCK_JOB_READY` event, `job-cancel`, `block-job-cancel` or
/// `job-complete` will still need to be used as appropriate.
///
/// Features:
///
/// @deprecated: This command is deprecated.  Use `job-dismiss`
/// ```text
///     instead.
/// ```
#[qapi(command)]
#[qapi(name = "block-job-dismiss")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "2.12.0")]
#[qapi(deprecated(added = "10.1.0"))]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockJobDismiss {
    /// The job identifier.
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
///
/// The command is applicable only to jobs which have @auto-finalize
/// option and only when this option is set to false.
///
/// Features:
///
/// @deprecated: This command is deprecated.  Use `job-finalize`
/// ```text
///     instead.
/// ```
#[qapi(command)]
#[qapi(name = "block-job-finalize")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "2.12.0")]
#[qapi(deprecated(added = "10.1.0"))]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockJobFinalize {
    /// The job identifier.
    pub id: String,
}

/// Pause an active background block operation.
///
/// This command returns immediately after marking the active job for
/// pausing.  Pausing an already paused job is an error.
///
/// The job will pause as soon as possible, which means transitioning
/// into the PAUSED state if it was RUNNING, or into STANDBY if it was
/// READY.  The corresponding `JOB_STATUS_CHANGE` event will be emitted.
///
/// Cancelling a paused job automatically resumes it.
///
/// Features:
///
/// @deprecated: This command is deprecated.  Use `job-pause`
/// ```text
///     instead.
/// ```
///
/// Errors:
/// ```text
///     - If no background operation is active on this device,
///       DeviceNotActive
/// ```
#[qapi(command)]
#[qapi(name = "block-job-pause")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.3.0")]
#[qapi(deprecated(added = "10.1.0"))]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockJobPause {
    /// The job identifier.  This used to be a device name (hence
    /// the name of the parameter), but since QEMU 2.7 it can have other
    /// values.
    pub device: String,
}

/// Resume an active background block operation.
///
/// This command returns immediately after resuming a paused job.
/// Resuming an already running job is an error.
///
/// This command also clears the error status of the job.
///
/// Features:
///
/// @deprecated: This command is deprecated.  Use `job-resume`
/// ```text
///     instead.
/// ```
///
/// Errors:
/// ```text
///     - If no background operation is active on this device,
///       DeviceNotActive
/// ```
#[qapi(command)]
#[qapi(name = "block-job-resume")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.3.0")]
#[qapi(deprecated(added = "10.1.0"))]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockJobResume {
    /// The job identifier.  This used to be a device name (hence
    /// the name of the parameter), but since QEMU 2.7 it can have other
    /// values.
    pub device: String,
}

/// Set maximum speed for a background block operation.
///
/// This command can only be issued when there is an active block job.
///
/// Throttling can be disabled by setting the speed to 0.
///
/// Errors:
/// ```text
///     - If no background operation is active on this device,
///       DeviceNotActive
/// ```
#[qapi(command)]
#[qapi(name = "block-job-set-speed")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockJobSetSpeed {
    /// The job identifier.  This used to be a device name (hence
    /// the name of the parameter), but since QEMU 2.7 it can have other
    /// values.
    pub device: String,
    /// the maximum speed, in bytes per second, or 0 for unlimited.
    /// Defaults to 0.
    pub speed: i64,
}

/// Manage read, write and flush latency histograms for the device.
///
/// If only @id parameter is specified, remove all present latency
/// histograms for the device.  Otherwise, add/reset some of (or all)
/// latency histograms.
///
/// Errors:
/// ```text
///     - if device is not found or any boundary arrays are invalid.
/// ```
///
///
/// .. qmp-example::
/// ```text
///    :annotated:
/// ```
///
/// ```text
///    Set new histograms for all io types with intervals
///    [0, 10), [10, 50), [50, 100), [100, +inf)::
/// ```
///
/// ```text
///     -> { "execute": "block-latency-histogram-set",
///          "arguments": { "id": "drive0",
///                         "boundaries": [10, 50, 100] } }
///     <- { "return": {} }
/// ```
///
/// .. qmp-example::
/// ```text
///    :annotated:
/// ```
///
/// ```text
///    Set new histogram only for write, other histograms will remain
///    not changed (or not created)::
/// ```
///
/// ```text
///     -> { "execute": "block-latency-histogram-set",
///          "arguments": { "id": "drive0",
///                         "boundaries-write": [10, 50, 100] } }
///     <- { "return": {} }
/// ```
///
/// .. qmp-example::
/// ```text
///    :annotated:
/// ```
///
/// ```text
///    Set new histograms with the following intervals:
/// ```
///
/// ```text
///    - read, flush: [0, 10), [10, 50), [50, 100), [100, +inf)
///    - write: [0, 1000), [1000, 5000), [5000, +inf)
/// ```
///
/// ```text
///    ::
/// ```
///
/// ```text
///     -> { "execute": "block-latency-histogram-set",
///          "arguments": { "id": "drive0",
///                         "boundaries": [10, 50, 100],
///                         "boundaries-write": [1000, 5000] } }
///     <- { "return": {} }
/// ```
///
/// .. qmp-example::
/// ```text
///    :annotated:
/// ```
///
/// ```text
///    Remove all latency histograms::
/// ```
///
/// ```text
///     -> { "execute": "block-latency-histogram-set",
///          "arguments": { "id": "drive0" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "block-latency-histogram-set")]
#[qapi(returns = "()")]
#[qapi(added = "4.0.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockLatencyHistogramSet {
    /// list of interval boundary values (see description in
    /// `BlockLatencyHistogramInfo` definition).  If specified, all
    /// latency histograms are removed, and empty ones created for all
    /// io types with intervals corresponding to @boundaries (except for
    /// io types, for which specific boundaries are set through the
    /// following parameters).
    pub boundaries: Option<Vec<u64>>,
    /// list of interval boundary values for flush
    /// latency histogram.
    #[qapi(name = "boundaries-flush")]
    pub boundaries_flush: Option<Vec<u64>>,
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
    #[qapi(added = "8.1.0")]
    pub boundaries_zap: Option<Vec<u64>>,
    /// The name or QOM path of the guest device.
    pub id: String,
}

/// This command sets the password of a block device that has not been open
/// with a password and requires one.
///
/// This command is now obsolete and will always return an error since 2.10
#[qapi(command)]
#[qapi(name = "block_passwd")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
#[qapi(removed = "6.0.0")]
pub struct BlockPasswd {
    /// the name of the block backend device to set the password on
    pub device: Option<String>,
    /// graph node name to set the password on (Since 2.0)
    #[qapi(name = "node-name")]
    #[qapi(added = "2.0.0")]
    pub node_name: Option<String>,
    /// the password to use for the device
    pub password: String,
}

/// Resize a block image while a guest is running.
///
/// Either @device or @node-name must be set but not both.
///
/// Errors:
/// ```text
///     - If @device is not a valid block device, DeviceNotFound
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "block_resize",
///          "arguments": { "device": "scratch", "size": 1073741824 } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "block_resize")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockResize {
    /// the name of the device to get the image resized
    pub device: Option<String>,
    /// graph node name to get the image resized (Since 2.0)
    #[qapi(name = "node-name")]
    #[qapi(added = "2.0.0")]
    pub node_name: Option<String>,
    /// new image size in bytes
    pub size: i64,
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
///
/// Errors:
/// ```text
///     - If @device is not a valid block device, DeviceNotFound
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "block_set_io_throttle",
///          "arguments": { "id": "virtio-blk-pci0/virtio-backend",
///                         "bps": 0,
///                         "bps_rd": 0,
///                         "bps_wr": 0,
///                         "iops": 512,
///                         "iops_rd": 0,
///                         "iops_wr": 0,
///                         "bps_max": 0,
///                         "bps_rd_max": 0,
///                         "bps_wr_max": 0,
///                         "iops_max": 0,
///                         "iops_rd_max": 0,
///                         "iops_wr_max": 0,
///                         "bps_max_length": 0,
///                         "iops_size": 0 } }
///     <- { "return": {} }
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "block_set_io_throttle",
///          "arguments": { "id": "ide0-1-0",
///                         "bps": 1000000,
///                         "bps_rd": 0,
///                         "bps_wr": 0,
///                         "iops": 0,
///                         "iops_rd": 0,
///                         "iops_wr": 0,
///                         "bps_max": 8000000,
///                         "bps_rd_max": 0,
///                         "bps_wr_max": 0,
///                         "iops_max": 0,
///                         "iops_rd_max": 0,
///                         "iops_wr_max": 0,
///                         "bps_max_length": 60,
///                         "iops_size": 0 } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "block_set_io_throttle")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockSetIoThrottle {
    /// total throughput limit in bytes per second
    pub bps: i64,
    /// total throughput limit during bursts, in bytes (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub bps_max: Option<i64>,
    /// maximum length of the @bps_max burst period, in
    /// seconds.  It must only be set if @bps_max is set as well.
    /// Defaults to 1.  (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub bps_max_length: Option<i64>,
    /// read throughput limit in bytes per second
    pub bps_rd: i64,
    /// read throughput limit during bursts, in bytes
    /// (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub bps_rd_max: Option<i64>,
    /// maximum length of the @bps_rd_max burst period,
    /// in seconds.  It must only be set if @bps_rd_max is set as well.
    /// Defaults to 1.  (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub bps_rd_max_length: Option<i64>,
    /// write throughput limit in bytes per second
    pub bps_wr: i64,
    /// write throughput limit during bursts, in bytes
    /// (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub bps_wr_max: Option<i64>,
    /// maximum length of the @bps_wr_max burst period,
    /// in seconds.  It must only be set if @bps_wr_max is set as well.
    /// Defaults to 1.  (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub bps_wr_max_length: Option<i64>,
    /// Block device name
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "5.0.0"))]
    pub device: Option<String>,
    /// throttle group name (Since 2.4)
    #[qapi(added = "2.4.0")]
    pub group: Option<String>,
    /// The name or QOM path of the guest device (since: 2.8)
    #[qapi(added = "2.8.0")]
    pub id: Option<String>,
    /// total I/O operations per second
    pub iops: i64,
    /// total I/O operations per second during bursts, in bytes
    /// (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub iops_max: Option<i64>,
    /// maximum length of the @iops burst period, in
    /// seconds.  It must only be set if @iops_max is set as well.
    /// Defaults to 1.  (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub iops_max_length: Option<i64>,
    /// read I/O operations per second
    pub iops_rd: i64,
    /// read I/O operations per second during bursts, in bytes
    /// (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub iops_rd_max: Option<i64>,
    /// maximum length of the @iops_rd_max burst
    /// period, in seconds.  It must only be set if @iops_rd_max is set
    /// as well.  Defaults to 1.  (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub iops_rd_max_length: Option<i64>,
    /// an I/O size in bytes (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub iops_size: Option<i64>,
    /// write I/O operations per second
    pub iops_wr: i64,
    /// write I/O operations per second during bursts, in
    /// bytes (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub iops_wr_max: Option<i64>,
    /// maximum length of the @iops_wr_max burst
    /// period, in seconds.  It must only be set if @iops_wr_max is set
    /// as well.  Defaults to 1.  (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub iops_wr_max_length: Option<i64>,
}

/// Change the write threshold for a block drive.  An event will be
/// delivered if a write to this block drive crosses the configured
/// threshold.  The threshold is an offset, thus must be non-negative.
/// Default is no write threshold.  Setting the threshold to zero
/// disables it.
///
/// This is useful to transparently resize thin-provisioned drives
/// without the guest OS noticing.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "block-set-write-threshold",
///          "arguments": { "node-name": "mydev",
///                         "write-threshold": 17179869184 } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "block-set-write-threshold")]
#[qapi(returns = "()")]
#[qapi(added = "2.3.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockSetWriteThreshold {
    /// graph node name on which the threshold must be set.
    #[qapi(name = "node-name")]
    pub node_name: String,
    /// configured threshold for the block device, bytes.
    /// Use 0 to disable the threshold.
    #[qapi(name = "write-threshold")]
    pub write_threshold: u64,
}

/// Copy data from a backing file into a block device.
///
/// The block streaming operation is performed in the background until
/// the entire backing file has been copied.  This command returns
/// immediately once streaming has started.  The status of ongoing block
/// streaming operations can be checked with `query-block-jobs`.  The
/// operation can be stopped before it has completed using the
/// `job-cancel` or `block-job-cancel` command.
///
/// The node that receives the data is called the top image, can be
/// located in any part of the chain (but always above the base image;
/// see below) and can be specified using its device or node name.
/// Earlier QEMU versions only allowed 'device' to name the top level
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
/// backing file and the `BLOCK_JOB_COMPLETED` event is emitted.
///
/// In case @device is a filter node, `block-stream` modifies the first
/// non-filter overlay node below it to point to the new backing node
/// instead of modifying @device itself.
///
/// Errors:
/// ```text
///     - If @device does not exist, DeviceNotFound.
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "block-stream",
///          "arguments": { "device": "virtio0",
///                         "base": "/tmp/master.qcow2" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "block-stream")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockStream {
    /// When false, this job will wait in a CONCLUDED state
    /// after it has completely ceased all work, and awaits
    /// `job-dismiss`.  When true, this job will automatically disappear
    /// without user intervention.  Defaults to true.  (Since 3.1)
    #[qapi(name = "auto-dismiss")]
    #[qapi(added = "3.1.0")]
    pub auto_dismiss: Option<bool>,
    /// When false, this job will wait in a PENDING state
    /// after it has finished its work, waiting for `job-finalize`
    /// before making any block graph changes.  When true, this job will
    /// automatically perform its abort or commit actions.  Defaults to
    /// true.  (Since 3.1)
    #[qapi(name = "auto-finalize")]
    #[qapi(added = "3.1.0")]
    pub auto_finalize: Option<bool>,
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
    #[qapi(added = "2.1.0")]
    pub backing_file: Option<String>,
    /// If true, replace any protocol mentioned in
    /// the 'backing file format' with 'raw', rather than storing the
    /// protocol name as the backing format.  Can be used even when no
    /// image header will be updated (default false; since 9.0).
    #[qapi(name = "backing-mask-protocol")]
    #[qapi(added = "9.0.0")]
    pub backing_mask_protocol: Option<bool>,
    /// the common backing file name.  It cannot be set if @base-node
    /// or @bottom is also set.
    pub base: Option<String>,
    /// the node name of the backing file.  It cannot be set if
    /// @base or @bottom is also set.  (Since 2.8)
    #[qapi(name = "base-node")]
    #[qapi(added = "2.8.0")]
    pub base_node: Option<String>,
    /// the last node in the chain that should be streamed into
    /// top.  It cannot be set if @base or @base-node is also set.  It
    /// cannot be filter node.  (Since 6.0)
    #[qapi(added = "6.0.0")]
    pub bottom: Option<String>,
    /// the device or node name of the top image
    pub device: String,
    /// the node name that should be assigned to the
    /// filter driver that the stream job inserts into the graph above
    /// @device.  If this option is not given, a node name is
    /// autogenerated.  (Since: 6.0)
    #[qapi(name = "filter-node-name")]
    #[qapi(added = "6.0.0")]
    pub filter_node_name: Option<String>,
    /// identifier for the newly-created block job.  If omitted,
    /// the device name will be used.  (Since 2.7)
    #[qapi(name = "job-id")]
    #[qapi(added = "2.7.0")]
    pub job_id: Option<String>,
    /// the action to take on an error (default report).  'stop'
    /// and 'enospc' can only be used if the block device supports
    /// io-status (see `BlockInfo`).  (Since 1.3)
    #[qapi(name = "on-error")]
    #[qapi(added = "1.3.0")]
    pub on_error: Option<BlockdevOnError>,
    /// the maximum speed, in bytes per second
    pub speed: Option<i64>,
}

/// Start measuring dirty page rate of the VM.  Results can be retrieved
/// with `query-dirty-rate` after measurements are completed.
///
/// Dirty page rate is the number of pages changed in a given time
/// period expressed in MiB/s.  The following methods of calculation are
/// available:
///
/// 1. In page sampling mode, a random subset of pages are selected and
/// ```text
///    hashed twice: once at the beginning of measurement time period,
///    and once again at the end.  If two hashes for some page are
///    different, the page is counted as changed.  Since this method
///    relies on sampling and hashing, calculated dirty page rate is
///    only an estimate of its true value.  Increasing @sample-pages
///    improves estimation quality at the cost of higher computational
///    overhead.
/// ```
///
/// 2. Dirty bitmap mode captures writes to memory (for example by
/// ```text
///    temporarily revoking write access to all pages) and counting page
///    faults.  Information about modified pages is collected into a
///    bitmap, where each bit corresponds to one guest page.  This mode
///    requires that KVM accelerator property "dirty-ring-size" is *not*
///    set.
/// ```
///
/// 3. Dirty ring mode is similar to dirty bitmap mode, but the
/// ```text
///    information about modified pages is collected into ring buffer.
///    This mode tracks page modification per each vCPU separately.  It
///    requires that KVM accelerator property "dirty-ring-size" is set.
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> {"execute": "calc-dirty-rate", "arguments": {"calc-time": 1,
///                                                     "sample-pages": 512} }
///     <- { "return": {} }
/// ```
///
/// .. qmp-example::
/// ```text
///    :annotated:
/// ```
///
/// ```text
///    Measure dirty rate using dirty bitmap for 500 milliseconds::
/// ```
///
/// ```text
///     -> {"execute": "calc-dirty-rate", "arguments": {"calc-time": 500,
///         "calc-time-unit": "millisecond", "mode": "dirty-bitmap"} }
/// ```
///
/// ```text
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "calc-dirty-rate")]
#[qapi(returns = "()")]
#[qapi(added = "5.2.0")]
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
    #[qapi(added = "8.2.0")]
    pub calc_time_unit: Option<TimeUnit>,
    /// mechanism for tracking dirty pages.  Default value is
    /// 'page-sampling'.  Others are 'dirty-bitmap' and 'dirty-ring'.
    /// (Since 6.1)
    #[qapi(added = "6.2.0")]
    pub mode: Option<DirtyRateMeasureMode>,
    /// number of sampled pages per each GiB of guest memory.
    /// Default value is 512.  For 4KiB guest pages this corresponds to
    /// sampling ratio of 0.2%.  This argument is used only in page
    /// sampling mode.  (Since 6.1)
    #[qapi(name = "sample-pages")]
    #[qapi(added = "6.1.0")]
    pub sample_pages: Option<i64>,
}

/// Cancel the upper limit of dirty page rate for virtual CPUs.
///
/// Cancel the dirty page limit for the vCPU which has been set with
/// `set-vcpu-dirty-limit` command.  Note that this command requires
/// support from dirty ring, same as the `set-vcpu-dirty-limit`.
///
///
/// .. qmp-example::
///
/// ```text
///     -> {"execute": "cancel-vcpu-dirty-limit"},
///         "arguments": { "cpu-index": 1 } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "cancel-vcpu-dirty-limit")]
#[qapi(returns = "()")]
#[qapi(added = "7.1.0")]
pub struct CancelVcpuDirtyLimit {
    /// index of a virtual CPU, default is all.
    #[qapi(name = "cpu-index")]
    pub cpu_index: Option<i64>,
}

/// This command is multiple commands multiplexed together.
///
/// Features:
/// @deprecated: This command is deprecated.  For changing block
/// ```text
///              devices, use 'blockdev-change-medium' instead; for changing VNC
///              parameters, use 'change-vnc-password' instead.
/// ```
///
/// Returns: - Nothing on success.
/// ```text
///          - If @device is not a valid block device, DeviceNotFound
/// ```
///
///
/// Example:
///
/// 1. Change a removable medium
///
/// -> { "execute": "change",
/// ```text
///      "arguments": { "device": "ide1-cd0",
///                     "target": "/srv/images/Fedora-12-x86_64-DVD.iso" } }
/// ```
/// <- { "return": {} }
///
/// 2. Change VNC password
///
/// -> { "execute": "change",
/// ```text
///      "arguments": { "device": "vnc", "target": "password",
///                     "arg": "foobar1" } }
/// ```
/// <- { "return": {} }
#[qapi(command)]
#[qapi(name = "change")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.1.0")]
#[qapi(deprecated(added = "5.0.0"))]
#[qapi(removed = "6.0.0")]
pub struct Change {
    /// If @device is a block device, then this is an optional format to open
    /// the device with.
    /// If @device is 'vnc' and @target is 'password', this is the new VNC
    /// password to set.  See change-vnc-password for additional notes.
    pub arg: Option<String>,
    /// This is normally the name of a block device but it may also be 'vnc'.
    /// when it's 'vnc', then sub command depends on @target
    pub device: String,
    /// If @device is a block device, then this is the new filename.
    /// If @device is 'vnc', then if the value 'password' selects the vnc
    /// change password command.   Otherwise, this specifies a new server URI
    /// address to listen to for VNC connections.
    pub target: String,
}

/// Change the backing file in the image file metadata.  This does not
/// cause QEMU to reopen the image file to reparse the backing filename
/// (it may, however, perform a reopen to change permissions from r/o ->
/// r/w -> r/o, if needed).  The new backing file string is written into
/// the image file metadata, and the QEMU internal strings are updated.
///
/// Errors:
/// ```text
///     - If "device" does not exist or cannot be determined,
///       DeviceNotFound
/// ```
#[qapi(command)]
#[qapi(name = "change-backing-file")]
#[qapi(returns = "()")]
#[qapi(added = "2.1.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct ChangeBackingFile {
    /// The string to write as the backing file.  This string
    /// is not validated, so care should be taken when specifying the
    /// string or the image chain may not be able to be reopened again.
    #[qapi(name = "backing-file")]
    pub backing_file: String,
    /// The device name or node-name of the root node that owns
    /// image-node-name.
    pub device: String,
    /// The name of the block driver state node of the
    /// image to modify.  The "device" argument is used to verify
    /// "image-node-name" is in the chain described by "device".
    #[qapi(name = "image-node-name")]
    pub image_node_name: String,
}

/// Change the VNC server password.
///
///
/// .. note:: An empty password in this command will set the password to
/// ```text
///    the empty string.  Existing clients are unaffected by executing
///    this command.
/// ```
#[qapi(command)]
#[qapi(name = "change-vnc-password")]
#[qapi(returns = "()")]
#[qapi(condition = "defined(CONFIG_VNC)")]
#[qapi(added = "1.1.0")]
pub struct ChangeVncPassword {
    /// the new password to use with VNC authentication
    pub password: String,
}

/// Add a character device backend
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute" : "chardev-add",
///          "arguments" : { "id" : "foo",
///                          "backend" : { "type" : "null", "data" : {} } } }
///     <- { "return": {} }
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute" : "chardev-add",
///          "arguments" : { "id" : "bar",
///                          "backend" : { "type" : "file",
///                                        "data" : { "out" : "/tmp/bar.log" } } } }
///     <- { "return": {} }
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute" : "chardev-add",
///          "arguments" : { "id" : "baz",
///                          "backend" : { "type" : "pty", "data" : {} } } }
///     <- { "return": { "pty" : "/dev/pty/42" } }
/// ```
#[qapi(command)]
#[qapi(name = "chardev-add")]
#[qapi(returns = "ChardevReturn")]
#[qapi(added = "1.4.0")]
pub struct ChardevAdd {
    /// backend type and parameters
    pub backend: ChardevBackend,
    /// the chardev's ID, must be unique
    pub id: String,
}

/// Change a character device backend
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute" : "chardev-change",
///          "arguments" : { "id" : "baz",
///                          "backend" : { "type" : "pty", "data" : {} } } }
///     <- { "return": { "pty" : "/dev/pty/42" } }
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> {"execute" : "chardev-change",
///         "arguments" : {
///             "id" : "charchannel2",
///             "backend" : {
///                 "type" : "socket",
///                 "data" : {
///                     "addr" : {
///                         "type" : "unix" ,
///                         "data" : {
///                             "path" : "/tmp/charchannel2.socket"
///                         }
///                      },
///                      "server" : true,
///                      "wait" : false }}}}
///     <- {"return": {}}
/// ```
#[qapi(command)]
#[qapi(name = "chardev-change")]
#[qapi(returns = "ChardevReturn")]
#[qapi(added = "2.10.0")]
pub struct ChardevChange {
    /// new backend type and parameters
    pub backend: ChardevBackend,
    /// the chardev's ID
    pub id: String,
}

/// Remove a character device backend
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "chardev-remove", "arguments": { "id" : "foo" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "chardev-remove")]
#[qapi(returns = "()")]
#[qapi(added = "1.4.0")]
pub struct ChardevRemove {
    /// the chardev's ID, must not be in use
    pub id: String,
}

/// Send a break to a character device
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "chardev-send-break", "arguments": { "id" : "foo" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "chardev-send-break")]
#[qapi(returns = "()")]
#[qapi(added = "2.10.0")]
pub struct ChardevSendBreak {
    /// the chardev's ID
    pub id: String,
}

/// Set migration information for remote display.  This makes the server
/// ask the client to automatically reconnect using the new parameters
/// once migration finished successfully.  Only implemented for SPICE.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "client_migrate_info",
///          "arguments": { "protocol": "spice",
///                         "hostname": "virt42.lab.kraxel.org",
///                         "port": 1234 } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "client_migrate_info")]
#[qapi(returns = "()")]
#[qapi(added = "2.4.0")]
pub struct ClientMigrateInfo {
    /// server certificate subject
    #[qapi(name = "cert-subject")]
    pub cert_subject: Option<String>,
    /// migration target hostname
    pub hostname: String,
    /// spice tcp port for plaintext channels
    pub port: Option<i64>,
    /// must be "spice"
    pub protocol: String,
    /// spice tcp port for tls-secured channels
    #[qapi(name = "tls-port")]
    pub tls_port: Option<i64>,
}

/// Close a file descriptor previously passed via SCM rights
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "closefd", "arguments": { "fdname": "fd1" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "closefd")]
#[qapi(returns = "()")]
#[qapi(added = "1.2.0")]
pub struct Closefd {
    /// file descriptor name
    pub fdname: String,
}

/// Resume guest VM execution.
///
///
/// .. note:: This command will succeed if the guest is currently
/// ```text
///    running.  It will also succeed if the guest is in the "inmigrate"
///    state; in this case, the effect of the command is to make sure
///    the guest starts once migration finishes, removing the effect of
///    the ``-S`` command line option if it was passed.
/// ```
///
/// ```text
///    If the VM was previously suspended, and not been reset or woken,
///    this command will transition back to the "suspended" state.
///    (Since 9.0)
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "cont" }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "cont")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct Cont {}

/// This command is a nop that is only provided for the purposes of compatibility.
///
///
/// Notes: Do not use this command.
#[qapi(command)]
#[qapi(name = "cpu")]
#[qapi(returns = "()")]
#[qapi(added = "1.0.0")]
#[qapi(removed = "2.12.0")]
pub struct Cpu {
    pub index: i64,
}

/// Adds CPU with specified ID.
///
/// Features:
/// @deprecated: This command is deprecated.  Use `device_add` instead.
/// ```text
///     See the `query-hotpluggable-cpus` command for details.
/// ```
///
/// Returns: Nothing on success
///
///
/// Example:
///
/// -> { "execute": "cpu-add", "arguments": { "id": 2 } }
/// <- { "return": {} }
#[qapi(command)]
#[qapi(name = "cpu-add")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.5.0")]
#[qapi(deprecated(added = "5.0.0"))]
#[qapi(removed = "5.2.0")]
pub struct CpuAdd {
    /// ID of CPU to be created, valid values [0..max_cpus)
    pub id: i64,
}

/// Initiate adding dynamic capacity extents to a host.  This simulates
/// operations defined in Compute Express Link (CXL) Specification,
/// Revision 3.1, Section 7.6.7.6.5.  Note that, currently, establishing
/// success or failure of the full Add Dynamic Capacity flow requires
/// out of band communication with the OS of the CXL host.
///
/// Features:
///
/// @unstable: For now this command is subject to change.
#[qapi(command)]
#[qapi(name = "cxl-add-dynamic-capacity")]
#[qapi(feature = "unstable")]
#[qapi(returns = "()")]
#[qapi(added = "9.1.0")]
#[qapi(unstable(added = "9.1.0"))]
pub struct CxlAddDynamicCapacity {
    /// The "Extent List" field as defined in Compute Express Link
    /// (CXL) Specification, Revision 3.1, Table 7-70.
    pub extents: Vec<CxlDynamicCapacityExtent>,
    /// The "Host ID" field as defined in Compute Express Link
    /// (CXL) Specification, Revision 3.1, Table 7-70.
    #[qapi(name = "host-id")]
    pub host_id: u16,
    /// path to the CXL Dynamic Capacity Device in the QOM tree.
    pub path: String,
    /// The "Region Number" field as defined in Compute Express
    /// Link (CXL) Specification, Revision 3.1, Table 7-70.  Valid
    /// range is from 0-7.
    pub region: u8,
    /// The "Selection Policy" bits as defined in
    /// Compute Express Link (CXL) Specification, Revision 3.1,
    /// Table 7-70.  It specifies the policy to use for selecting
    /// which extents comprise the added capacity.
    #[qapi(name = "selection-policy")]
    pub selection_policy: CxlExtentSelectionPolicy,
    /// The "Tag" field as defined in Compute Express Link (CXL)
    /// Specification, Revision 3.1, Table 7-70.
    pub tag: Option<String>,
}

/// Command to inject a single correctable error.  Multiple error
/// injection of this error type is not interesting as there is no
/// associated header log.  These errors are reported via AER as a
/// correctable internal error, with additional detail available from
/// the CXL device.
#[qapi(command)]
#[qapi(name = "cxl-inject-correctable-error")]
#[qapi(returns = "()")]
#[qapi(added = "8.0.0")]
pub struct CxlInjectCorrectableError {
    /// CXL Type 3 device canonical QOM path
    pub path: String,
    /// Type of error.
    pub r#type: CxlCorErrorType,
}

/// Inject an event record for a DRAM Event (CXL r3.2 8.2.10.2.1.2).
/// This event type is reported via one of the event logs
/// specified via the log parameter.
#[qapi(command)]
#[qapi(name = "cxl-inject-dram-event")]
#[qapi(returns = "()")]
#[qapi(added = "8.1.0")]
pub struct CxlInjectDramEvent {
    /// Bank of the memory event location.  A single bank is accessed
    /// per read or write of the memory.
    pub bank: Option<u8>,
    /// Bank group of the memory event location, incorporating
    /// a number of banks.
    #[qapi(name = "bank-group")]
    pub bank_group: Option<u8>,
    /// The channel of the memory event location.  A channel is an
    /// interface that can be independently accessed for a transaction.
    pub channel: Option<u8>,
    /// Advanced programmable corrected memory error
    /// threshold event flags.
    #[qapi(name = "cme-ev-flags")]
    #[qapi(added = "11.0.0")]
    pub cme_ev_flags: Option<u8>,
    /// Column address within the DRAM.
    pub column: Option<u16>,
    /// Device specific component identifier for the event.
    /// May describe a field replaceable sub-component of the device.
    #[qapi(name = "component-id")]
    #[qapi(added = "11.0.0")]
    pub component_id: Option<String>,
    /// Bits within each nibble.  Used in order of bits
    /// set in the nibble-mask.  Up to 4 nibbles may be covered.
    #[qapi(name = "correction-mask")]
    pub correction_mask: Option<Vec<u64>>,
    /// Corrected volatile memory error count at event.
    #[qapi(name = "cvme-count")]
    #[qapi(added = "11.0.0")]
    pub cvme_count: Option<u32>,
    /// Memory Event Descriptor with additional memory event
    /// information.  See CXL r3.2 Table 8-58 DRAM Event Record, Memory
    /// Event Descriptor for bit definitions.
    pub descriptor: u8,
    /// Device Physical Address (relative to @path device).  Note
    /// lower bits include some flags.  See CXL r3.2 Table 8-58 DRAM
    /// Event Record, Physical Address.
    pub dpa: u64,
    /// Event Record Flags.  See CXL r3.2 Table 8-55 Common Event
    /// Record Format, Event Record Flags for subfield definitions.
    #[qapi(added = "11.0.0")]
    #[qapi(required)]
    pub flags: Option<u32>,
    /// Event Record Flags.  See CXL r3.0 Table 8-42 Common Event
    /// Record Format, Event Record Flags for subfield definitions.
    #[qapi(name = "flags")]
    #[qapi(removed = "11.0.0")]
    #[qapi(required)]
    pub flags_legacy: Option<u8>,
    /// ID of the device head from where the event originated.
    #[qapi(name = "head-id")]
    #[qapi(added = "11.0.0")]
    pub head_id: Option<u8>,
    /// This flag specifies whether the device-specific
    /// component identifier format follows PLDM.
    #[qapi(name = "is-comp-id-pldm")]
    #[qapi(added = "11.0.0")]
    pub is_comp_id_pldm: Option<bool>,
    /// Logical Device (LD) ID of LD from where the event
    /// originated.
    #[qapi(name = "ld-id")]
    #[qapi(added = "11.0.0")]
    pub ld_id: Option<u16>,
    /// event log to add the event to
    pub log: CxlEventLog,
    /// Maintenance operation class the device requests to
    /// initiate.
    #[qapi(name = "maint-op-class")]
    #[qapi(added = "11.0.0")]
    pub maint_op_class: Option<u8>,
    /// Maintenance operation subclass the device
    /// requests to initiate.
    #[qapi(name = "maint-op-subclass")]
    #[qapi(added = "11.0.0")]
    pub maint_op_subclass: Option<u8>,
    /// Identifies one or more nibbles that the error affects
    #[qapi(name = "nibble-mask")]
    pub nibble_mask: Option<u32>,
    /// CXL type 3 device canonical QOM path
    pub path: String,
    /// The rank of the memory event location.  A rank is a set of
    /// memory devices on a channel that together execute a transaction.
    pub rank: Option<u8>,
    /// Row address within the DRAM.
    pub row: Option<u32>,
    /// The sub-channel of the memory event location.
    #[qapi(name = "sub-channel")]
    #[qapi(added = "11.0.0")]
    pub sub_channel: Option<u8>,
    /// Memory event sub-type.
    #[qapi(name = "sub-type")]
    #[qapi(added = "11.0.0")]
    #[qapi(required)]
    pub sub_type: Option<u8>,
    /// Type of first transaction that caused the event
    /// to occur.  See CXL r3.2 Table 8-58 DRAM Event Record,
    /// Transaction Type for possible values.
    #[qapi(name = "transaction-type")]
    pub transaction_type: u8,
    /// Type of memory event that occurred.  See CXL r3.2 Table 8-58
    /// DRAM Event Record, Memory Event Type for possible values.
    pub r#type: u8,
}

/// Inject an event record for a General Media Event (CXL r3.2
/// 8.2.10.2.1.1).  This event type is reported via one of the event
/// logs specified via the log parameter.
#[qapi(command)]
#[qapi(name = "cxl-inject-general-media-event")]
#[qapi(returns = "()")]
#[qapi(added = "8.1.0")]
pub struct CxlInjectGeneralMediaEvent {
    /// The channel of the memory event location.  A channel is an
    /// interface that can be independently accessed for a transaction.
    pub channel: Option<u8>,
    /// Corrected memory error count at event.
    #[qapi(name = "cme-count")]
    #[qapi(added = "11.0.0")]
    pub cme_count: Option<u32>,
    /// Advanced programmable corrected memory error
    /// threshold event flags.
    #[qapi(name = "cme-ev-flags")]
    #[qapi(added = "11.0.0")]
    pub cme_ev_flags: Option<u8>,
    /// Device specific component identifier for the event.
    /// May describe a field replaceable sub-component of the device.
    #[qapi(name = "component-id")]
    pub component_id: Option<String>,
    /// Memory Event Descriptor with additional memory event
    /// information.  See CXL r3.2 Table 8-57 General Media Event
    /// Record, Memory Event Descriptor for bit definitions.
    pub descriptor: u8,
    /// Bitmask that represents all devices in the rank associated
    /// with the memory event location.
    pub device: Option<u32>,
    /// Device Physical Address (relative to @path device).  Note
    /// lower bits include some flags.  See CXL r3.2 Table 8-57 General
    /// Media Event Record, Physical Address.
    pub dpa: u64,
    /// Event Record Flags.  See CXL r3.2 Table 8-55 Common Event
    /// Record Format, Event Record Flags for subfield definitions.
    #[qapi(added = "11.0.0")]
    #[qapi(required)]
    pub flags: Option<u32>,
    /// Event Record Flags.  See CXL r3.0 Table 8-42 Common Event
    /// Record Format, Event Record Flags for subfield definitions.
    #[qapi(name = "flags")]
    #[qapi(removed = "11.0.0")]
    #[qapi(required)]
    pub flags_legacy: Option<u8>,
    /// ID of the device head from where the event originated.
    #[qapi(name = "head-id")]
    #[qapi(added = "11.0.0")]
    pub head_id: Option<u8>,
    /// This flag specifies whether the device-specific
    /// component identifier format follows PLDM.
    #[qapi(name = "is-comp-id-pldm")]
    #[qapi(added = "11.0.0")]
    pub is_comp_id_pldm: Option<bool>,
    /// Logical Device (LD) ID of LD from where the event
    /// originated.
    #[qapi(name = "ld-id")]
    #[qapi(added = "11.0.0")]
    pub ld_id: Option<u16>,
    /// event log to add the event to
    pub log: CxlEventLog,
    /// Maintenance operation class the device requests to
    /// initiate.
    #[qapi(name = "maint-op-class")]
    #[qapi(added = "11.0.0")]
    pub maint_op_class: Option<u8>,
    /// Maintenance operation subclass the device
    /// requests to initiate.
    #[qapi(name = "maint-op-subclass")]
    #[qapi(added = "11.0.0")]
    pub maint_op_subclass: Option<u8>,
    /// CXL type 3 device canonical QOM path
    pub path: String,
    /// The rank of the memory event location.  A rank is a set of
    /// memory devices on a channel that together execute a transaction.
    pub rank: Option<u8>,
    /// Memory event sub-type.
    #[qapi(name = "sub-type")]
    #[qapi(added = "11.0.0")]
    #[qapi(required)]
    pub sub_type: Option<u8>,
    /// Type of first transaction that caused the event
    /// to occur.  See CXL r3-2 Table 8-57 General Media Event Record,
    /// Transaction Type for possible values.
    #[qapi(name = "transaction-type")]
    pub transaction_type: u8,
    /// Type of memory event that occurred.  See CXL r3.2 Table 8-57
    /// General Media Event Record, Memory Event Type for possible
    /// values.
    pub r#type: u8,
}

/// Inject an event record for a Memory Module Event (CXL r3.2
/// 8.2.10.2.1.3).  This event includes a copy of the Device Health info
/// at the time of the event.
#[qapi(command)]
#[qapi(name = "cxl-inject-memory-module-event")]
#[qapi(returns = "()")]
#[qapi(added = "8.1.0")]
pub struct CxlInjectMemoryModuleEvent {
    /// See CXL r3.2 Table 8-148 Get Health Info Output
    /// Payload, Additional Status for subfield definitions.
    #[qapi(name = "additional-status")]
    pub additional_status: u8,
    /// Device specific component identifier for the event.
    /// May describe a field replaceable sub-component of the device.
    #[qapi(name = "component-id")]
    #[qapi(added = "11.0.0")]
    pub component_id: Option<String>,
    /// Total number of correctable
    /// errors in persistent memory
    #[qapi(name = "corrected-persistent-error-count")]
    pub corrected_persistent_error_count: u32,
    /// Total number of correctable errors
    /// in volatile memory.
    #[qapi(name = "corrected-volatile-error-count")]
    pub corrected_volatile_error_count: u32,
    /// Number of times the device has been unable to
    /// determine whether data loss may have occurred.
    #[qapi(name = "dirty-shutdown-count")]
    pub dirty_shutdown_count: u32,
    /// Event Record Flags.  See CXL r3.2 Table 8-55 Common Event
    /// Record Format, Event Record Flags for subfield definitions.
    #[qapi(added = "11.0.0")]
    #[qapi(required)]
    pub flags: Option<u32>,
    /// Event Record Flags.  See CXL r3.0 Table 8-42 Common Event
    /// Record Format, Event Record Flags for subfield definitions.
    #[qapi(name = "flags")]
    #[qapi(removed = "11.0.0")]
    #[qapi(required)]
    pub flags_legacy: Option<u8>,
    /// ID of the device head from where the event originated.
    #[qapi(name = "head-id")]
    #[qapi(added = "11.0.0")]
    pub head_id: Option<u8>,
    /// Overall health summary bitmap.  See CXL r3.2 Table
    /// 8-148 Get Health Info Output Payload, Health Status for bit
    /// definitions.
    #[qapi(name = "health-status")]
    pub health_status: u8,
    /// This flag specifies whether the device-specific
    /// component identifier format follows PLDM.
    #[qapi(name = "is-comp-id-pldm")]
    #[qapi(added = "11.0.0")]
    pub is_comp_id_pldm: Option<bool>,
    /// Logical Device (LD) ID of LD from where the event
    /// originated.
    #[qapi(name = "ld-id")]
    #[qapi(added = "11.0.0")]
    pub ld_id: Option<u16>,
    /// Percentage (0-100) of factory expected life span.
    #[qapi(name = "life-used")]
    pub life_used: u8,
    /// event log to add the event to
    pub log: CxlEventLog,
    /// Maintenance operation class the device requests to
    /// initiate.
    #[qapi(name = "maint-op-class")]
    #[qapi(added = "11.0.0")]
    pub maint_op_class: Option<u8>,
    /// Maintenance operation subclass the device
    /// requests to initiate.
    #[qapi(name = "maint-op-subclass")]
    #[qapi(added = "11.0.0")]
    pub maint_op_subclass: Option<u8>,
    /// Overall media health summary.  See CXL r3.2 Table
    /// 8-148 Get Health Info Output Payload, Media Status for bit
    /// definitions.
    #[qapi(name = "media-status")]
    pub media_status: u8,
    /// CXL type 3 device canonical QOM path
    pub path: String,
    /// Device event sub-type.
    #[qapi(name = "sub-type")]
    #[qapi(added = "11.0.0")]
    #[qapi(required)]
    pub sub_type: Option<u8>,
    /// Device temperature in degrees Celsius.
    pub temperature: i16,
    /// Device Event Type.  See CXL r3.2 Table 8-59 Memory Module
    /// Event Record for bit definitions for bit definiions.
    pub r#type: u8,
}

/// Poison records indicate that a CXL memory device knows that a
/// particular memory region may be corrupted.  This may be because of
/// locally detected errors (e.g. ECC failure) or poisoned writes
/// received from other components in the system.  This injection
/// mechanism enables testing of the OS handling of poison records which
/// may be queried via the CXL mailbox.
#[qapi(command)]
#[qapi(name = "cxl-inject-poison")]
#[qapi(returns = "()")]
#[qapi(added = "8.1.0")]
pub struct CxlInjectPoison {
    /// Length of poison to inject; must be a multiple of 64 bytes.
    pub length: u64,
    /// CXL type 3 device canonical QOM path
    pub path: String,
    /// Start address; must be 64 byte aligned.
    pub start: u64,
}

/// Command to allow injection of multiple errors in one go.  This
/// allows testing of multiple header log handling in the OS.
#[qapi(command)]
#[qapi(name = "cxl-inject-uncorrectable-errors")]
#[qapi(returns = "()")]
#[qapi(added = "8.0.0")]
pub struct CxlInjectUncorrectableErrors {
    /// Errors to inject
    pub errors: Vec<CxlUncorErrorRecord>,
    /// CXL Type 3 device canonical QOM path
    pub path: String,
}

/// Initiate release of dynamic capacity extents from a host.  This
/// simulates operations defined in Compute Express Link (CXL)
/// Specification, Revision 3.1, Section 7.6.7.6.6.  Note that,
/// currently, success or failure of the full Release Dynamic Capacity
/// flow requires out of band communication with the OS of the CXL host.
///
/// Features:
///
/// @unstable: For now this command is subject to change.
#[qapi(command)]
#[qapi(name = "cxl-release-dynamic-capacity")]
#[qapi(feature = "unstable")]
#[qapi(returns = "()")]
#[qapi(added = "9.1.0")]
#[qapi(unstable(added = "9.1.0"))]
pub struct CxlReleaseDynamicCapacity {
    /// The "Extent List" field as defined in Compute Express
    /// Link (CXL) Specification, Revision 3.1, Table 7-71.
    pub extents: Vec<CxlDynamicCapacityExtent>,
    /// Bit[4] of the "Flags" field in Compute Express
    /// Link (CXL) Specification, Revision 3.1, Table 7-71.  When set,
    /// the device does not wait for a Release Dynamic Capacity command
    /// from the host.  Instead, the host immediately looses access to
    /// the released capacity.
    #[qapi(name = "forced-removal")]
    pub forced_removal: Option<bool>,
    /// The "Host ID" field as defined in Compute Express Link
    /// (CXL) Specification, Revision 3.1, Table 7-71.
    #[qapi(name = "host-id")]
    pub host_id: u16,
    /// path to the CXL Dynamic Capacity Device in the QOM tree.
    pub path: String,
    /// The "Region Number" field as defined in Compute Express
    /// Link Specification, Revision 3.1, Table 7-71.  Valid range
    /// is from 0-7.
    pub region: u8,
    /// Bit[3:0] of the "Flags" field as defined in
    /// Compute Express Link (CXL) Specification, Revision 3.1,
    /// Table 7-71.
    #[qapi(name = "removal-policy")]
    pub removal_policy: CxlExtentRemovalPolicy,
    /// Bit[5] of the "Flags" field in Compute Express
    /// Link (CXL) Specification, Revision 3.1, Table 7-71.  When set,
    /// the device should sanitize all released capacity as a result of
    /// this request.  This ensures that all user data and metadata is
    /// made permanently unavailable by whatever means is appropriate
    /// for the media type.  Note that changing encryption keys is not
    /// sufficient.
    #[qapi(name = "sanitize-on-release")]
    pub sanitize_on_release: Option<bool>,
    /// The "Tag" field as defined in Compute Express Link (CXL)
    /// Specification, Revision 3.1, Table 7-71.
    pub tag: Option<String>,
}

/// Add a device.
///
/// Features:
///
/// @json-cli: If present, the "-device" command line option supports
/// ```text
///     JSON syntax with a structure identical to the arguments of this
///     command.
/// ```
///
/// @json-cli-hotplug: If present, the "-device" command line option
/// ```text
///     supports JSON syntax without the reference counting leak that
///     broke hot-unplug
/// ```
///
/// .. admonition:: Notes
///
/// ```text
///     1. Additional arguments depend on the type.
/// ```
///
/// ```text
///     2. For detailed information about this command, please refer to
///        the 'docs/qdev-device-use.txt' file.
/// ```
///
/// ```text
///     3. It's possible to list device properties by running QEMU with
///        the ``-device DEVICE,help`` command-line argument, where
///        DEVICE is the device's name.
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "device_add",
///          "arguments": { "driver": "e1000", "id": "net1",
///                         "bus": "pci.0",
///                         "mac": "52:54:00:12:34:56" } }
///     <- { "return": {} }
/// ```
///
/// TODO: This command effectively bypasses QAPI completely due to its
/// ```text
///     "additional arguments" business.  It shouldn't have been added
///     to the schema in this form.  It should be qapified properly, or
///     replaced by a properly qapified command.
/// ```
#[qapi(command)]
#[qapi(name = "device_add")]
#[qapi(returns = "()")]
#[qapi(feature = "json-cli")]
#[qapi(feature = "json-cli-hotplug")]
#[qapi(added = "2.8.0")]
pub struct DeviceAdd {
    /// Additional command arguments not modeled by this schema.
    #[qapi(flatten)]
    pub arguments: Option<HashMap<String, serde_json::Value>>,
    /// the device's parent bus (device tree path)
    #[qapi(added = "2.9.0")]
    pub bus: Option<String>,
    /// the name of the new device's driver
    pub driver: String,
    /// the device's ID, must be unique
    pub id: Option<String>,
}

/// Remove a device from a guest
///
/// Errors:
/// ```text
///     - If @id is not a valid device, DeviceNotFound
/// ```
///
/// .. note:: When this command completes, the device may not be removed
/// ```text
///    from the guest.  Hot removal is an operation that requires guest
///    cooperation.  This command merely requests that the guest begin
///    the hot removal process.  Completion of the device removal
///    process is signaled with a `DEVICE_DELETED` event.  Guest reset
///    will automatically complete removal for all devices.  If a
///    guest-side error in the hot removal process is detected, the
///    device will not be removed and a `DEVICE_UNPLUG_GUEST_ERROR`
///    event is sent.  Some errors cannot be detected.
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "device_del",
///          "arguments": { "id": "net1" } }
///     <- { "return": {} }
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "device_del",
///          "arguments": { "id": "/machine/peripheral-anon/device[0]" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "device_del")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct DeviceDel {
    /// the device's ID or QOM path
    pub id: String,
}

/// List properties associated with a device.
///
/// Returns: a list describing a devices properties
///
/// .. note:: Objects can create properties at runtime, for example to
/// ```text
///    describe links between different devices and/or objects.  These
///    properties are not included in the output of this command.
/// ```
#[qapi(command)]
#[qapi(name = "device-list-properties")]
#[qapi(returns = "Vec<ObjectPropertyInfo>")]
#[qapi(added = "1.2.0")]
#[qapi(returns(added = "1.2.0", value = "Vec<DevicePropertyInfo>"))]
#[qapi(returns(added = "2.12.0", value = "Vec<ObjectPropertyInfo>"))]
pub struct DeviceListProperties {
    /// the type name of a device
    pub typename: String,
}

/// Synchronize device configuration from host to guest part.  First,
/// copy the configuration from the host part (backend) to the guest
/// part (frontend).  Then notify guest software that device
/// configuration changed.
///
/// The command may be used to notify the guest about block device
/// capacity change.  Currently only vhost-user-blk device supports
/// this.
///
/// Features:
///
/// @unstable: The command is experimental.
#[qapi(command)]
#[qapi(name = "device-sync-config")]
#[qapi(feature = "unstable")]
#[qapi(returns = "()")]
#[qapi(added = "9.2.0")]
#[qapi(unstable(added = "9.2.0"))]
pub struct DeviceSyncConfig {
    /// the device's ID or QOM path
    pub id: String,
}

/// Reload display configuration.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "display-reload",
///          "arguments": { "type": "vnc", "tls-certs": true  } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "display-reload")]
#[qapi(returns = "()")]
#[qapi(added = "6.0.0")]
pub struct DisplayReload {
    /// Specify the display type.
    #[qapi(discriminator)]
    pub r#type: DisplayReloadType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<DisplayReloadOptionsBranch>,
}

/// Update display configuration.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "display-update",
///          "arguments": { "type": "vnc", "addresses":
///                         [ { "type": "inet", "host": "0.0.0.0",
///                             "port": "5901" } ] } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "display-update")]
#[qapi(returns = "()")]
#[qapi(added = "7.1.0")]
pub struct DisplayUpdate {
    /// Specify the display type.
    #[qapi(discriminator)]
    pub r#type: DisplayUpdateType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<DisplayUpdateOptionsBranch>,
}

/// Start mirroring a block device's writes to a new destination.
/// target specifies the target of the new image.  If the file exists,
/// or if it is a device, it will be used as the new destination for
/// writes.  If it does not exist, a new file will be created.  @format
/// specifies the format of the mirror image, default is to probe if
/// mode='existing', else the format of the source.
///
/// Errors:
/// ```text
///     - If @device is not a valid block device, GenericError
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "drive-mirror",
///          "arguments": { "device": "ide-hd0",
///                         "target": "/some/place/my-image",
///                         "sync": "full",
///                         "format": "qcow2" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "drive-mirror")]
#[qapi(returns = "()")]
#[qapi(added = "1.3.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct DriveMirror {
    /// When false, this job will wait in a CONCLUDED state
    /// after it has completely ceased all work, and awaits
    /// `job-dismiss`.  When true, this job will automatically disappear
    /// without user intervention.  Defaults to true.  (Since 3.1)
    #[qapi(name = "auto-dismiss")]
    #[qapi(added = "3.1.0")]
    pub auto_dismiss: Option<bool>,
    /// When false, this job will wait in a PENDING state
    /// after it has finished its work, waiting for `job-finalize`
    /// before making any block graph changes.  When true, this job will
    /// automatically perform its abort or commit actions.  Defaults to
    /// true.  (Since 3.1)
    #[qapi(name = "auto-finalize")]
    #[qapi(added = "3.1.0")]
    pub auto_finalize: Option<bool>,
    /// maximum amount of data in flight from source to target
    /// (since 1.4).
    #[qapi(name = "buf-size")]
    #[qapi(added = "1.4.0")]
    pub buf_size: Option<i64>,
    /// when to copy data to the destination; defaults to
    /// 'background' (Since: 3.0)
    #[qapi(name = "copy-mode")]
    #[qapi(added = "3.0.0")]
    pub copy_mode: Option<MirrorCopyMode>,
    /// the device name or node-name of a root node whose writes
    /// should be mirrored.
    pub device: String,
    /// the format of the new destination, default is to probe if
    /// @mode is 'existing', else the format of the source
    pub format: Option<String>,
    /// granularity of the dirty bitmap, default is 64K if the
    /// image format doesn't have clusters, 4K if the clusters are
    /// smaller than that, else the cluster size.  Must be a power of 2
    /// between 512 and 64M (since 1.4).
    #[qapi(added = "1.4.0")]
    pub granularity: Option<u32>,
    /// identifier for the newly-created block job.  If omitted,
    /// the device name will be used.  (Since 2.7)
    #[qapi(name = "job-id")]
    #[qapi(added = "2.7.0")]
    pub job_id: Option<String>,
    /// whether and how QEMU should create a new image, default is
    /// 'absolute-paths'.
    pub mode: Option<NewImageMode>,
    /// the new block driver state node name in the graph
    /// (Since 2.1)
    #[qapi(name = "node-name")]
    #[qapi(added = "2.1.0")]
    pub node_name: Option<String>,
    /// the action to take on an error on the source,
    /// default 'report'.  'stop' and 'enospc' can only be used if the
    /// block device supports io-status (see `BlockInfo`).
    #[qapi(name = "on-source-error")]
    pub on_source_error: Option<BlockdevOnError>,
    /// the action to take on an error on the target,
    /// default 'report' (no limitations, since this applies to a
    /// different block device than @device).
    #[qapi(name = "on-target-error")]
    pub on_target_error: Option<BlockdevOnError>,
    /// with sync=full graph node name to be replaced by the new
    /// image when a whole image copy is done.  This can be used to
    /// repair broken Quorum files.  By default, @device is replaced,
    /// although implicitly created filters on it are kept.  (Since 2.1)
    #[qapi(added = "2.1.0")]
    pub replaces: Option<String>,
    /// the maximum speed, in bytes per second
    pub speed: Option<i64>,
    /// what parts of the disk image should be copied to the
    /// destination (all the disk, only the sectors allocated in the
    /// topmost image, or only new I/O).
    pub sync: MirrorSyncMode,
    /// the target of the new image.  If the file exists, or if it
    /// is a device, the existing file/device will be used as the new
    /// destination.  If it does not exist, a new file will be created.
    pub target: String,
    /// Whether to try to unmap target sectors where source has only
    /// zero.  If true, and target unallocated sectors will read as
    /// zero, target image sectors will be unmapped; otherwise, zeroes
    /// will be written.  Both will result in identical contents.
    /// Default is true.  (Since 2.4)
    #[qapi(added = "2.3.1")]
    pub unmap: Option<bool>,
}

/// Save the FDT in dtb format.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "dumpdtb" }
///          "arguments": { "filename": "fdt.dtb" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "dumpdtb")]
#[qapi(condition = "CONFIG_FDT")]
#[qapi(returns = "()")]
#[qapi(added = "7.2.0")]
pub struct Dumpdtb {
    /// name of the dtb file to be created
    pub filename: String,
}

/// Dump guest's memory to vmcore.  It is a synchronous operation that
/// can take very long depending on the amount of guest memory.
///
/// .. note:: All boolean arguments default to false.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "dump-guest-memory",
///          "arguments": { "paging": false, "protocol": "fd:dump" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "dump-guest-memory")]
#[qapi(returns = "()")]
#[qapi(added = "1.2.0")]
pub struct DumpGuestMemory {
    /// if specified, the starting physical address.
    pub begin: Option<i64>,
    /// if true, QMP will return immediately rather than waiting
    /// for the dump to finish.  The user can track progress using
    /// `query-dump`.  (since 2.6).
    #[qapi(added = "2.6.0")]
    pub detach: Option<bool>,
    /// if specified, the format of guest memory dump.  But non-elf
    /// format is conflict with paging and filter, ie.  @paging, @begin
    /// and @length is not allowed to be specified with non-elf @format
    /// at the same time (since 2.0)
    #[qapi(added = "2.0.0")]
    pub format: Option<DumpGuestMemoryFormat>,
    /// if specified, the memory size, in bytes.  If you don't want
    /// to dump all guest's memory, please specify the start @begin and
    /// @length
    pub length: Option<i64>,
    /// if true, do paging to get guest's memory mapping.  This
    /// allows using gdb to process the core file.
    ///
    /// **Important**: this option can make QEMU allocate several
    /// gigabytes of RAM.  This can happen for a large guest, or a
    /// malicious guest pretending to be large.
    ///
    /// Also, paging=true has the following limitations:
    ///
    /// 1. The guest may be in a catastrophic state or can have
    /// ```text
    ///    corrupted memory, which cannot be trusted
    /// ```
    /// 2. The guest can be in real-mode even if paging is enabled.  For
    /// ```text
    ///    example, the guest uses ACPI to sleep, and ACPI sleep state
    ///    goes in real-mode
    /// ```
    /// 3. Currently only supported on i386 and x86_64.
    pub paging: bool,
    /// the filename or file descriptor of the vmcore.  The
    /// supported protocols are:
    ///
    /// 1. file: the protocol starts with "file:", and the following
    /// ```text
    ///    string is the file's path.
    /// ```
    /// 2. fd: the protocol starts with "fd:", and the following string
    /// ```text
    ///    is the fd's name.
    /// ```
    pub protocol: String,
}

/// Dump the storage keys for an s390x guest
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "dump-skeys",
///          "arguments": { "filename": "/tmp/skeys" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "dump-skeys")]
#[qapi(returns = "()")]
#[qapi(condition = "defined(TARGET_S390X)")]
#[qapi(added = "2.5.0")]
pub struct DumpSkeys {
    /// the path to the file to dump to
    pub filename: String,
}

/// Ejects the medium from a removable drive.
///
/// Features:
///
/// @deprecated: Member @device is deprecated.  Use @id instead.
///
/// Errors:
/// ```text
///     - If @device is not a valid block device, DeviceNotFound
/// ```
///
/// .. note:: Ejecting a device with no media results in success.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "eject", "arguments": { "id": "ide1-0-1" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "eject")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct Eject {
    /// Block device name
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "5.0.0"))]
    pub device: Option<String>,
    /// If true, eject regardless of whether the drive is locked.
    /// If not specified, the default value is false.
    pub force: Option<bool>,
    /// The name or QOM path of the guest device (since: 2.8)
    #[qapi(added = "2.8.0")]
    pub id: Option<String>,
}

/// Expire the password of a remote display server.
///
/// Errors:
/// ```text
///     - If @protocol is 'spice' and Spice is not active,
///       DeviceNotFound
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "expire_password", "arguments": { "protocol": "vnc",
///                                                       "time": "+60" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "expire_password")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct ExpirePassword {
    /// - 'vnc' to modify the VNC server expiration
    /// - 'spice' to modify the Spice server expiration
    #[qapi(discriminator)]
    #[qapi(added = "7.0.0")]
    #[qapi(required)]
    pub protocol: Option<DisplayProtocol>,
    /// the name of the remote display protocol 'vnc' or 'spice'
    #[qapi(name = "protocol")]
    #[qapi(removed = "7.0.0")]
    #[qapi(required)]
    pub protocol_legacy: Option<String>,
    /// when to expire the password.
    ///
    /// - 'now' to expire the password immediately
    /// - 'never' to cancel password expiration
    /// - '+INT' where INT is the number of seconds from now (integer)
    /// - 'INT' where INT is the absolute time in seconds
    pub time: String,
    #[qapi(union)]
    #[qapi(flatten)]
    #[qapi(added = "7.0.0")]
    pub u: Option<ExpirePasswordOptionsBranch>,
}

/// Receive a file descriptor via SCM rights and assign it a name
///
///
/// .. note:: If @fdname already exists, the file descriptor assigned to
/// ```text
///    it will be closed and replaced by the received file descriptor.
/// ```
///
/// ```text
///    The `closefd` command can be used to explicitly close the file
///    descriptor when it is no longer needed.
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "getfd", "arguments": { "fdname": "fd1" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "getfd")]
#[qapi(returns = "()")]
#[qapi(condition = "CONFIG_POSIX")]
#[qapi(added = "1.2.0")]
pub struct Getfd {
    /// file descriptor name
    pub fdname: String,
}

/// Add a socket that was duplicated to QEMU process with
/// WSADuplicateSocketW() via WSASocket() & WSAPROTOCOL_INFOW structure
/// and assign it a name (the SOCKET is associated with a CRT file
/// descriptor)
///
///
/// .. note:: If @fdname already exists, the file descriptor assigned to
/// ```text
///    it will be closed and replaced by the received file descriptor.
/// ```
///
/// ```text
///    The `closefd` command can be used to explicitly close the file
///    descriptor when it is no longer needed.
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "get-win32-socket",
///          "arguments": { "info": "abcd123..", "fdname": "skclient" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "get-win32-socket")]
#[qapi(condition = "CONFIG_WIN32")]
#[qapi(returns = "()")]
#[qapi(added = "8.0.0")]
pub struct GetWin32Socket {
    /// file descriptor name
    pub fdname: String,
    /// the WSAPROTOCOL_INFOW structure (encoded in base64)
    pub info: String,
}

/// Execute a command on the human monitor and return the output.
///
/// Features:
///
/// @savevm-monitor-nodes: If present, HMP command savevm only snapshots
/// ```text
///     monitor-owned nodes if they have no parents.  This allows the
///     use of 'savevm' with -blockdev.  (since 4.2)
/// ```
///
/// Returns: the output of the command as a string
///
///
/// .. note:: This command only exists as a stop-gap.  Its use is highly
/// ```text
///    discouraged.  The semantics of this command are not guaranteed:
///    this means that command names, arguments and responses can change
///    or be removed at **any** time.  Applications that rely on long
///    term stability guarantees should **not** use this command.
/// ```
///
/// ```text
///    Known limitations:
/// ```
///
/// ```text
///    * This command is stateless, this means that commands that depend
///      on state information (such as `getfd`) might not work.
/// ```
///
/// ```text
///    * Commands that prompt the user for data don't currently work.
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "human-monitor-command",
///          "arguments": { "command-line": "info kvm" } }
///     <- { "return": "kvm support: enabled\r\n" }
/// ```
#[qapi(command)]
#[qapi(name = "human-monitor-command")]
#[qapi(returns = "String")]
#[qapi(feature = "savevm-monitor-nodes")]
#[qapi(added = "1.1.0")]
pub struct HumanMonitorCommand {
    /// the command to execute in the human monitor
    #[qapi(name = "command-line")]
    pub command_line: String,
    /// The CPU to use for commands that require an implicit CPU
    #[qapi(name = "cpu-index")]
    pub cpu_index: Option<i64>,
}

/// Inject an error with additional ACPI 6.1 GHESv2 error information
///
/// Features:
///
/// @unstable: This command is experimental.
#[qapi(command)]
#[qapi(name = "inject-ghes-v2-error")]
#[qapi(feature = "unstable")]
#[qapi(returns = "()")]
#[qapi(added = "10.2.0")]
#[qapi(unstable(added = "10.2.0"))]
pub struct InjectGhesV2Error {
    /// contains a base64 encoded string with raw data for a single
    /// CPER record with Generic Error Status Block, Generic Error Data
    /// Entry and generic error data payload, as described at
    /// https://uefi.org/specs/UEFI/2.10/Apx_N_Common_Platform_Error_Record.html#format
    pub cper: String,
}

/// Injects a Non-Maskable Interrupt into the default CPU (x86/s390) or
/// all CPUs (ppc64).  The command fails when the guest doesn't support
/// injecting.
///
///
/// .. note:: Prior to 2.1, this command was only supported for x86 and
/// ```text
///    s390 VMs.
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "inject-nmi" }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "inject-nmi")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct InjectNmi {}

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
///
///
/// .. note:: The consoles are visible in the qom tree, under
/// ```text
///    ``/backend/console[$index]``.  They have a device link and head
///    property, so it is possible to map which console belongs to which
///    device and display.
/// ```
///
/// .. qmp-example::
/// ```text
///    :title: Press left mouse button
/// ```
///
/// ```text
///     -> { "execute": "input-send-event",
///         "arguments": { "device": "video0",
///                        "events": [ { "type": "btn",
///                        "data" : { "down": true, "button": "left" } } ] } }
///     <- { "return": {} }
/// ```
///
/// ```text
///     -> { "execute": "input-send-event",
///         "arguments": { "device": "video0",
///                        "events": [ { "type": "btn",
///                        "data" : { "down": false, "button": "left" } } ] } }
///     <- { "return": {} }
/// ```
///
/// .. qmp-example::
/// ```text
///    :title: Press ctrl-alt-del
/// ```
///
/// ```text
///     -> { "execute": "input-send-event",
///          "arguments": { "events": [
///             { "type": "key", "data" : { "down": true,
///               "key": {"type": "qcode", "data": "ctrl" } } },
///             { "type": "key", "data" : { "down": true,
///               "key": {"type": "qcode", "data": "alt" } } },
///             { "type": "key", "data" : { "down": true,
///               "key": {"type": "qcode", "data": "delete" } } } ] } }
///     <- { "return": {} }
/// ```
///
/// .. qmp-example::
/// ```text
///    :title: Move mouse pointer to absolute coordinates
/// ```
///
/// ```text
///     -> { "execute": "input-send-event" ,
///       "arguments": { "events": [
///                    { "type": "abs", "data" : { "axis": "x", "value" : 20000 } },
///                    { "type": "abs", "data" : { "axis": "y", "value" : 400 } } ] } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "input-send-event")]
#[qapi(returns = "()")]
#[qapi(added = "2.6.0")]
pub struct InputSendEvent {
    /// display device to send event(s) to.
    pub device: Option<String>,
    /// List of `InputEvent` union.
    pub events: Vec<InputEvent>,
    /// head to send event(s) to, in case the display device supports
    /// multiple scanouts.
    pub head: Option<i64>,
}

/// Instruct an active background job to cancel at the next opportunity.
/// This command returns immediately after marking the active job for
/// cancellation.
///
/// The job will cancel as soon as possible and then emit a
/// `JOB_STATUS_CHANGE` event.  Usually, the status will change to
/// ABORTING, but it is possible that a job successfully completes (e.g.
/// because it was almost done and there was no opportunity to cancel
/// earlier than completing the job) and transitions to PENDING instead.
#[qapi(command)]
#[qapi(name = "job-cancel")]
#[qapi(returns = "()")]
#[qapi(added = "3.0.0")]
pub struct JobCancel {
    /// The job identifier.
    pub id: String,
}

/// Manually trigger completion of an active job in the READY or STANDBY
/// state.  Completing the job in any other state is an error.
///
/// This is supported only for drive mirroring, where it also switches
/// the device to write to the target path only.  Note that drive
/// mirroring includes `drive-mirror`, `blockdev-mirror` and
/// `block-commit` job (only in case of "active commit", when the node
/// being committed is used by the guest).  The ability to complete is
/// signaled with a `BLOCK_JOB_READY` event.
///
/// This command completes an active background block operation
/// synchronously.  The ordering of this command's return with the
/// `BLOCK_JOB_COMPLETED` event is not defined.  Note that if an I/O
/// error occurs during the processing of this command: 1) the command
/// itself will fail; 2) the error will be processed according to the
/// rerror/werror arguments that were specified when starting the
/// operation.
#[qapi(command)]
#[qapi(name = "job-complete")]
#[qapi(returns = "()")]
#[qapi(added = "3.0.0")]
pub struct JobComplete {
    /// The job identifier.
    pub id: String,
}

/// Deletes a job that is in the CONCLUDED state.  This command only
/// needs to be run explicitly for jobs that don't have automatic
/// dismiss enabled.  In turn, automatic dismiss may be enabled only for
/// jobs that have @auto-dismiss option, which are `drive-backup`,
/// `blockdev-backup`, `drive-mirror`, `blockdev-mirror`, `block-commit`
/// and `block-stream`.  @auto-dismiss is enabled by default for these
/// jobs.
///
/// This command will refuse to operate on any job that has not yet
/// reached its terminal state, CONCLUDED.  For jobs that make use of
/// the JOB_READY event, `job-cancel` or `job-complete` will still need
/// to be used as appropriate.
#[qapi(command)]
#[qapi(name = "job-dismiss")]
#[qapi(returns = "()")]
#[qapi(added = "3.0.0")]
pub struct JobDismiss {
    /// The job identifier.
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
///
/// The command is applicable only to jobs which have @auto-finalize
/// option and only when this option is set to false.
#[qapi(command)]
#[qapi(name = "job-finalize")]
#[qapi(returns = "()")]
#[qapi(added = "3.0.0")]
pub struct JobFinalize {
    /// The identifier of any job in the transaction, or of a job that
    /// is not part of any transaction.
    pub id: String,
}

/// Pause an active job.
///
/// This command returns immediately after marking the active job for
/// pausing.  Pausing an already paused job is an error.
///
/// The job will pause as soon as possible, which means transitioning
/// into the PAUSED state if it was RUNNING, or into STANDBY if it was
/// READY.  The corresponding `JOB_STATUS_CHANGE` event will be emitted.
///
/// Cancelling a paused job automatically resumes it.
#[qapi(command)]
#[qapi(name = "job-pause")]
#[qapi(returns = "()")]
#[qapi(added = "3.0.0")]
pub struct JobPause {
    /// The job identifier.
    pub id: String,
}

/// Resume a paused job.
///
/// This command returns immediately after resuming a paused job.
/// Resuming an already running job is an error.
///
/// This command also clears the error status for block-jobs (stream,
/// commit, mirror, backup).
#[qapi(command)]
#[qapi(name = "job-resume")]
#[qapi(returns = "()")]
#[qapi(added = "3.0.0")]
pub struct JobResume {
    /// The job identifier.
    pub id: String,
}

/// Save a portion of guest memory to a file.
///
///
/// .. caution:: Errors were not reliably returned until 1.1.
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "memsave",
///          "arguments": { "val": 10,
///                         "size": 100,
///                         "filename": "/tmp/virtual-mem-dump" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "memsave")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct Memsave {
    /// the index of the virtual CPU to use for translating the
    /// virtual address (defaults to CPU 0)
    #[qapi(name = "cpu-index")]
    pub cpu_index: Option<i64>,
    /// the file to save the memory to as binary data
    pub filename: String,
    /// the size of memory region to save
    #[qapi(added = "9.1.0")]
    #[qapi(required)]
    pub size: Option<u64>,
    /// the size of memory region to save
    #[qapi(name = "size")]
    #[qapi(removed = "9.1.0")]
    #[qapi(required)]
    pub size_legacy: Option<i64>,
    /// the virtual address of the guest to start from
    #[qapi(added = "9.1.0")]
    #[qapi(required)]
    pub val: Option<u64>,
    /// the virtual address of the guest to start from
    #[qapi(name = "val")]
    #[qapi(removed = "9.1.0")]
    #[qapi(required)]
    pub val_legacy: Option<i64>,
}

/// Migrates the current running guest to another Virtual Machine.
///
///
/// .. admonition:: Notes
///
/// ```text
///     1. The `query-migrate` command should be used to check
///        migration's progress and final result (this information is
///        provided by the 'status' member).
/// ```
///
/// ```text
///     2. The uri argument should have the Uniform Resource Identifier
///        of default destination VM.  This connection will be bound to
///        default network.
/// ```
///
/// ```text
///     3. For now, number of migration streams is restricted to one,
///        i.e. number of items in 'channels' list is just 1.
/// ```
///
/// ```text
///     4. The 'uri' and 'channels' arguments are mutually exclusive;
///        exactly one of the two should be present.
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "migrate", "arguments": { "uri": "tcp:0:4446" } }
///     <- { "return": {} }
/// ```
///
/// ```text
///     -> { "execute": "migrate",
///          "arguments": {
///              "channels": [ { "channel-type": "main",
///                              "addr": { "transport": "socket",
///                                        "type": "inet",
///                                        "host": "10.12.34.9",
///                                        "port": "1050" } } ] } }
///     <- { "return": {} }
/// ```
///
/// ```text
///     -> { "execute": "migrate",
///          "arguments": {
///              "channels": [ { "channel-type": "main",
///                              "addr": { "transport": "exec",
///                                        "args": [ "/bin/nc", "-p", "6000",
///                                                  "/some/sock" ] } } ] } }
///     <- { "return": {} }
/// ```
///
/// ```text
///     -> { "execute": "migrate",
///          "arguments": {
///              "channels": [ { "channel-type": "main",
///                              "addr": { "transport": "rdma",
///                                        "host": "10.12.34.9",
///                                        "port": "1050" } } ] } }
///     <- { "return": {} }
/// ```
///
/// ```text
///     -> { "execute": "migrate",
///          "arguments": {
///              "channels": [ { "channel-type": "main",
///                              "addr": { "transport": "file",
///                                        "filename": "/tmp/migfile",
///                                        "offset": "0x1000" } } ] } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "migrate")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct Migrate {
    /// do block migration (full disk copy)
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub blk: Option<bool>,
    /// list of migration stream channels with each stream in the
    /// list connected to a destination interface endpoint.
    #[qapi(added = "8.2.0")]
    pub channels: Option<Vec<MigrationChannel>>,
    /// this argument exists only for compatibility reasons and is
    /// ignored by QEMU
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "10.1.0"))]
    #[qapi(removed = "11.0.0")]
    pub detach: Option<bool>,
    /// incremental disk copy migration
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub inc: Option<bool>,
    /// when set, use the new uri/channels specified to resume
    /// paused postcopy migration.  This flag should only be used if
    /// the previous postcopy migration was interrupted.  The command
    /// will fail unless migration is in "postcopy-paused" state.
    /// (default: false, since 3.0)
    #[qapi(added = "3.0.0")]
    pub resume: Option<bool>,
    /// the Uniform Resource Identifier of the destination VM
    pub uri: Option<String>,
}

/// Cancel the currently executing migration process.  Allows a new
/// migration to be started right after.  When postcopy-ram is in use,
/// cancelling is not allowed after the postcopy phase has started.
///
/// .. note:: This command succeeds even if there is no migration
/// ```text
///    process running.
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "migrate_cancel" }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "migrate_cancel")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct MigrateCancel {}

/// Continue migration when it's in a paused state.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "migrate-continue" , "arguments":
///          { "state": "pre-switchover" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "migrate-continue")]
#[qapi(returns = "()")]
#[qapi(added = "2.11.0")]
pub struct MigrateContinue {
    /// The state the migration is currently expected to be in
    pub state: MigrationStatus,
}

/// Start an incoming migration.  QEMU must have been started with
/// -incoming defer.
///
///
/// .. admonition:: Notes
///
/// ```text
///     1. It's a bad idea to use a string for the uri, but it needs to
///        stay compatible with -incoming and the format of the uri is
///        already exposed above libvirt.
/// ```
///
/// ```text
///     2. QEMU must be started with -incoming defer to allow
///        `migrate-incoming` to be used.
/// ```
///
/// ```text
///     3. The uri format is the same as for -incoming
/// ```
///
/// ```text
///     4. For now, number of migration streams is restricted to one,
///        i.e. number of items in 'channels' list is just 1.
/// ```
///
/// ```text
///     5. The 'uri' and 'channels' arguments are mutually exclusive;
///        exactly one of the two should be present.
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "migrate-incoming",
///          "arguments": { "uri": "tcp:0:4446" } }
///     <- { "return": {} }
/// ```
///
/// ```text
///     -> { "execute": "migrate-incoming",
///          "arguments": {
///              "channels": [ { "channel-type": "main",
///                              "addr": { "transport": "socket",
///                                        "type": "inet",
///                                        "host": "10.12.34.9",
///                                        "port": "1050" } } ] } }
///     <- { "return": {} }
/// ```
///
/// ```text
///     -> { "execute": "migrate-incoming",
///          "arguments": {
///              "channels": [ { "channel-type": "main",
///                              "addr": { "transport": "exec",
///                                        "args": [ "/bin/nc", "-p", "6000",
///                                                  "/some/sock" ] } } ] } }
///     <- { "return": {} }
/// ```
///
/// ```text
///     -> { "execute": "migrate-incoming",
///          "arguments": {
///              "channels": [ { "channel-type": "main",
///                              "addr": { "transport": "rdma",
///                                        "host": "10.12.34.9",
///                                        "port": "1050" } } ] } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "migrate-incoming")]
#[qapi(returns = "()")]
#[qapi(added = "2.3.0")]
pub struct MigrateIncoming {
    /// list of migration stream channels with each stream in the
    /// list connected to a destination interface endpoint.
    #[qapi(added = "8.2.0")]
    pub channels: Option<Vec<MigrationChannel>>,
    /// Exit on incoming migration failure.  Default true.
    /// When set to false, the failure triggers a
    /// :qapi:event:`MIGRATION` event, and error details could be
    /// retrieved with `query-migrate`.  (since 9.1)
    #[qapi(name = "exit-on-error")]
    #[qapi(added = "9.1.0")]
    pub exit_on_error: Option<bool>,
    /// The Uniform Resource Identifier identifying the source or
    /// address to listen on
    pub uri: Option<String>,
}

/// Pause a migration.  Currently it only supports postcopy.
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "migrate-pause" }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "migrate-pause")]
#[qapi(returns = "()")]
#[qapi(added = "3.0.0")]
#[qapi(allow_oob)]
pub struct MigratePause {}

/// Provide a recovery migration stream URI.
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "migrate-recover",
///          "arguments": { "uri": "tcp:192.168.1.200:12345" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "migrate-recover")]
#[qapi(returns = "()")]
#[qapi(added = "3.0.0")]
#[qapi(allow_oob)]
pub struct MigrateRecover {
    /// the URI to be used for the recovery of migration stream.
    pub uri: String,
}

/// Set cache size to be used by XBZRLE migration
///
/// Features:
/// @deprecated: This command is deprecated.  Use
/// ```text
///              'migrate-set-parameters' instead.
/// ```
///
/// The size will be rounded down to the nearest power of 2.
/// The cache size can be modified before and during ongoing migration
///
/// Returns: nothing on success
///
///
/// Example:
///
/// -> { "execute": "migrate-set-cache-size",
/// ```text
///      "arguments": { "value": 536870912 } }
/// ```
/// <- { "return": {} }
#[qapi(command)]
#[qapi(name = "migrate-set-cache-size")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.2.0")]
#[qapi(deprecated(added = "5.0.0"))]
#[qapi(removed = "6.0.0")]
pub struct MigrateSetCacheSize {
    /// cache size in bytes
    pub value: i64,
}

/// Enable/Disable the following migration capabilities (like xbzrle)
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "migrate-set-capabilities" , "arguments":
///          { "capabilities": [ { "capability": "xbzrle", "state": true } ] } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "migrate-set-capabilities")]
#[qapi(returns = "()")]
#[qapi(added = "1.2.0")]
pub struct MigrateSetCapabilities {
    /// json array of capability modifications to make
    pub capabilities: Vec<MigrationCapabilityStatus>,
}

/// Set maximum tolerated downtime for migration.
///
/// Features:
/// @deprecated: This command is deprecated.  Use
/// ```text
///              'migrate-set-parameters' instead.
/// ```
///
/// Returns: nothing on success
///
///
/// Example:
///
/// -> { "execute": "migrate_set_downtime", "arguments": { "value": 0.1 } }
/// <- { "return": {} }
#[qapi(command)]
#[qapi(name = "migrate_set_downtime")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.1.0")]
#[qapi(deprecated(added = "5.0.0"))]
#[qapi(removed = "6.0.0")]
pub struct MigrateSetDowntime {
    /// maximum downtime in seconds
    pub value: f64,
}

/// Set migration parameters.  All arguments are optional.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "migrate-set-parameters" ,
///          "arguments": { "multifd-channels": 5 } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "migrate-set-parameters")]
#[qapi(returns = "()")]
#[qapi(added = "2.4.0")]
pub struct MigrateSetParameters {
    /// Initial delay (in milliseconds) before sending
    /// the first announce (Since 4.0)
    #[qapi(name = "announce-initial")]
    #[qapi(added = "4.0.0")]
    pub announce_initial: Option<u64>,
    /// Maximum delay (in milliseconds) between packets in
    /// the announcement (Since 4.0)
    #[qapi(name = "announce-max")]
    #[qapi(added = "4.0.0")]
    pub announce_max: Option<u64>,
    /// Number of self-announce packets sent after
    /// migration (Since 4.0)
    #[qapi(name = "announce-rounds")]
    #[qapi(added = "4.0.0")]
    pub announce_rounds: Option<u64>,
    /// Increase in delay (in milliseconds) between
    /// subsequent packets in the announcement (Since 4.0)
    #[qapi(name = "announce-step")]
    #[qapi(added = "4.0.0")]
    pub announce_step: Option<u64>,
    /// to set the available bandwidth that
    /// migration can use during switchover phase, in bytes per
    /// second.  **Note:** this does not limit the bandwidth during
    /// switchover, but only for calculations when making decisions to
    /// switch over.  By default, this value is zero, which means QEMU
    /// will estimate the bandwidth automatically.  This can be set
    /// when the estimated value is not accurate, while the user is
    /// able to guarantee such bandwidth is available when switching
    /// over.  When specified correctly, this can make the switchover
    /// decision much more accurate.  (Since 8.2)
    #[qapi(name = "avail-switchover-bandwidth")]
    #[qapi(added = "8.2.0")]
    pub avail_switchover_bandwidth: Option<u64>,
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
    #[qapi(added = "5.2.0")]
    pub block_bitmap_mapping: Option<Vec<BitmapMigrationNodeAlias>>,
    /// Affects how much storage is migrated when the
    /// block migration capability is enabled.  When false, the entire
    /// storage backing chain is migrated into a flattened image at the
    /// destination; when true, only the active qcow2 layer is migrated
    /// and the destination must already have access to the same backing
    /// chain as was used on the source.  (since 2.10)
    #[qapi(name = "block-incremental")]
    #[qapi(feature = "deprecated")]
    #[qapi(added = "2.10.0")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub block_incremental: Option<bool>,
    /// Set the compression level to be used in live
    /// migration, the compression level is an integer between 0 and 9,
    /// where 0 means no compression, 1 means the best compression
    /// speed, and 9 means best compression ratio which will consume
    /// more CPU.
    #[qapi(name = "compress-level")]
    #[qapi(feature = "deprecated")]
    #[qapi(added = "6.0.0")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub compress_level: Option<u8>,
    /// Set compression thread count to be used in live
    /// migration, the compression thread count is an integer between 1
    /// and 255.
    #[qapi(name = "compress-threads")]
    #[qapi(feature = "deprecated")]
    #[qapi(added = "6.0.0")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub compress_threads: Option<u8>,
    /// Controls behavior when all compression
    /// threads are currently busy.  If true (default), wait for a free
    /// compression thread to become available; otherwise, send the page
    /// uncompressed.  (Since 3.1)
    #[qapi(name = "compress-wait-thread")]
    #[qapi(feature = "deprecated")]
    #[qapi(added = "3.1.0")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub compress_wait_thread: Option<bool>,
    /// compression level
    #[qapi(name = "compress-level")]
    #[qapi(removed = "6.0.0")]
    pub compress_level_legacy: Option<i64>,
    /// compression thread count
    #[qapi(name = "compress-threads")]
    #[qapi(removed = "6.0.0")]
    pub compress_threads_legacy: Option<i64>,
    /// Command to start the new QEMU process when @mode
    /// is @cpr-exec.  The first list element is the program's filename,
    /// the remainder its arguments.  (Since 10.2)
    #[qapi(name = "cpr-exec-command")]
    #[qapi(added = "10.2.0")]
    pub cpr_exec_command: Option<Vec<String>>,
    /// throttle percentage increase each time
    /// auto-converge detects that migration is not making progress.
    /// The default value is 10.  (Since 2.7)
    #[qapi(name = "cpu-throttle-increment")]
    #[qapi(added = "6.0.0")]
    pub cpu_throttle_increment: Option<u8>,
    /// Initial percentage of time guest cpus are
    /// throttled when migration auto-converge is activated.  The
    /// default value is 20.  (Since 2.7)
    #[qapi(name = "cpu-throttle-initial")]
    #[qapi(added = "6.0.0")]
    pub cpu_throttle_initial: Option<u8>,
    /// Make CPU throttling slower at tail stage.
    /// At the tail stage of throttling, the Guest is very sensitive to
    /// CPU percentage while the @cpu-throttle -increment is excessive
    /// usually at tail stage.  If this parameter is true, we will
    /// compute the ideal CPU percentage used by the Guest, which may
    /// exactly make the dirty rate match the dirty rate threshold.
    /// Then we will choose a smaller throttle increment between the one
    /// specified by @cpu-throttle-increment and the one generated by
    /// ideal CPU percentage.  Therefore, it is compatible to
    /// traditional throttling, meanwhile the throttle increment won't
    /// be excessive at tail stage.  The default value is false.
    /// (Since 5.1)
    #[qapi(name = "cpu-throttle-tailslow")]
    #[qapi(added = "5.1.0")]
    pub cpu_throttle_tailslow: Option<bool>,
    /// throttle percentage increase each time
    /// auto-converge detects that migration is not making
    /// progress. The default value is 10. (Since 2.7)
    #[qapi(name = "cpu-throttle-increment")]
    #[qapi(added = "2.7.0")]
    #[qapi(removed = "6.0.0")]
    pub cpu_throttle_increment_legacy: Option<i64>,
    /// Initial percentage of time guest cpus are
    /// throttled when migration auto-converge is activated.
    /// The default value is 20. (Since 2.7)
    #[qapi(name = "cpu-throttle-initial")]
    #[qapi(added = "2.7.0")]
    #[qapi(removed = "6.0.0")]
    pub cpu_throttle_initial_legacy: Option<i64>,
    /// Set decompression thread count to be used in
    /// live migration, the decompression thread count is an integer
    /// between 1 and 255. Usually, decompression is at least 4 times as
    /// fast as compression, so set the decompress-threads to the number
    /// about 1/4 of compress-threads is adequate.
    #[qapi(name = "decompress-threads")]
    #[qapi(feature = "deprecated")]
    #[qapi(added = "6.0.0")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub decompress_threads: Option<u8>,
    /// decompression thread count
    #[qapi(name = "decompress-threads")]
    #[qapi(removed = "6.0.0")]
    pub decompress_threads_legacy: Option<i64>,
    /// Open migration files with O_DIRECT when possible.  This
    /// only has effect if the @mapped-ram capability is enabled.
    /// (Since 9.1)
    #[qapi(name = "direct-io")]
    #[qapi(added = "9.1.0")]
    pub direct_io: Option<bool>,
    /// set maximum tolerated downtime for migration.
    /// maximum downtime in milliseconds (Since 2.8)
    #[qapi(name = "downtime-limit")]
    #[qapi(added = "6.0.0")]
    pub downtime_limit: Option<u64>,
    /// set maximum tolerated downtime for migration. maximum
    /// downtime in milliseconds (Since 2.8)
    #[qapi(name = "downtime-limit")]
    #[qapi(added = "2.8.0")]
    #[qapi(removed = "6.0.0")]
    pub downtime_limit_legacy: Option<i64>,
    /// maximum speed for migration, in bytes per second.
    /// (Since 2.8)
    #[qapi(name = "max-bandwidth")]
    #[qapi(added = "6.0.0")]
    pub max_bandwidth: Option<u64>,
    /// maximum cpu throttle percentage.  Defaults to 99.
    /// (Since 3.1)
    #[qapi(name = "max-cpu-throttle")]
    #[qapi(added = "6.0.0")]
    pub max_cpu_throttle: Option<u8>,
    /// Background transfer bandwidth during
    /// postcopy.  Defaults to 0 (unlimited).  In bytes per second.
    /// (Since 3.0)
    #[qapi(name = "max-postcopy-bandwidth")]
    #[qapi(added = "3.0.0")]
    pub max_postcopy_bandwidth: Option<u64>,
    /// to set maximum speed for migration. maximum speed in
    /// bytes per second. (Since 2.8)
    #[qapi(name = "max-bandwidth")]
    #[qapi(added = "2.8.0")]
    #[qapi(removed = "6.0.0")]
    pub max_bandwidth_legacy: Option<i64>,
    /// maximum cpu throttle percentage.
    /// The default value is 99. (Since 3.1)
    #[qapi(name = "max-cpu-throttle")]
    #[qapi(added = "3.1.0")]
    #[qapi(removed = "6.0.0")]
    pub max_cpu_throttle_legacy: Option<i64>,
    /// Migration mode.  See description in `MigMode`.  Default is
    /// 'normal'.  (Since 8.2)
    #[qapi(added = "8.2.0")]
    pub mode: Option<MigMode>,
    /// Number of channels used to migrate data in
    /// parallel.  This is the same number that the number of sockets
    /// used for migration.  The default value is 2 (since 4.0)
    #[qapi(name = "multifd-channels")]
    #[qapi(added = "6.0.0")]
    pub multifd_channels: Option<u8>,
    /// Which compression method to use.  Defaults to
    /// none.  (Since 5.0)
    #[qapi(name = "multifd-compression")]
    #[qapi(added = "5.0.0")]
    pub multifd_compression: Option<MultiFdCompression>,
    /// Set the compression level to be used in live
    /// migration.  The level is an integer between 1 and 9, where 1
    /// means the best compression speed, and 9 means the best
    /// compression ratio which will consume more CPU.  Defaults to 1.
    /// (Since 9.2)
    #[qapi(name = "multifd-qatzip-level")]
    #[qapi(added = "9.2.0")]
    pub multifd_qatzip_level: Option<u8>,
    /// Set the compression level to be used in live
    /// migration, the compression level is an integer between 0 and 9,
    /// where 0 means no compression, 1 means the best compression
    /// speed, and 9 means best compression ratio which will consume
    /// more CPU.  Defaults to 1.  (Since 5.0)
    #[qapi(name = "multifd-zlib-level")]
    #[qapi(added = "6.0.0")]
    pub multifd_zlib_level: Option<u8>,
    /// Set the compression level to be used in live
    /// migration, the compression level is an integer between 0 and 20,
    /// where 0 means no compression, 1 means the best compression
    /// speed, and 20 means best compression ratio which will consume
    /// more CPU.  Defaults to 1.  (Since 5.0)
    #[qapi(name = "multifd-zstd-level")]
    #[qapi(added = "6.0.0")]
    pub multifd_zstd_level: Option<u8>,
    /// Number of channels used to migrate data in
    /// parallel. This is the same number that the
    /// number of sockets used for migration.  The
    /// default value is 2 (since 4.0)
    #[qapi(name = "multifd-channels")]
    #[qapi(added = "4.0.0")]
    #[qapi(removed = "6.0.0")]
    pub multifd_channels_legacy: Option<i64>,
    /// Set the compression level to be used in live
    /// migration, the compression level is an integer between 0
    /// and 9, where 0 means no compression, 1 means the best
    /// compression speed, and 9 means best compression ratio which
    /// will consume more CPU.
    /// Defaults to 1. (Since 5.0)
    #[qapi(name = "multifd-zlib-level")]
    #[qapi(added = "5.0.0")]
    #[qapi(removed = "6.0.0")]
    pub multifd_zlib_level_legacy: Option<i64>,
    /// Set the compression level to be used in live
    /// migration, the compression level is an integer between 0
    /// and 20, where 0 means no compression, 1 means the best
    /// compression speed, and 20 means best compression ratio which
    /// will consume more CPU.
    /// Defaults to 1. (Since 5.0)
    #[qapi(name = "multifd-zstd-level")]
    #[qapi(added = "5.0.0")]
    #[qapi(removed = "6.0.0")]
    pub multifd_zstd_level_legacy: Option<i64>,
    /// The ratio of bytes_dirty_period and
    /// bytes_xfer_period to trigger throttling.  It is expressed as
    /// percentage.  The default value is 50.  (Since 5.0)
    #[qapi(name = "throttle-trigger-threshold")]
    #[qapi(added = "6.0.0")]
    pub throttle_trigger_threshold: Option<u8>,
    /// The ratio of bytes_dirty_period and bytes_xfer_period
    /// to trigger throttling. It is expressed as percentage.
    /// The default value is 50. (Since 5.0)
    #[qapi(name = "throttle-trigger-threshold")]
    #[qapi(added = "5.0.0")]
    #[qapi(removed = "6.0.0")]
    pub throttle_trigger_threshold_legacy: Option<i64>,
    /// ID of the 'authz' object subclass that provides access
    /// control checking of the TLS x509 certificate distinguished name.
    /// This object is only resolved at time of use, so can be deleted
    /// and recreated on the fly while the migration server is active.
    /// If missing, it will default to denying access (Since 4.0)
    #[qapi(name = "tls-authz")]
    #[qapi(added = "4.0.0")]
    pub tls_authz: Option<StrOrNull>,
    /// ID of the 'tls-creds' object that provides credentials
    /// for establishing a TLS connection over the migration data
    /// channel.  On the outgoing side of the migration, the credentials
    /// must be for a 'client' endpoint, while for the incoming side the
    /// credentials must be for a 'server' endpoint.  Setting this to a
    /// non-empty string enables TLS for all migrations.  An empty
    /// string means that QEMU will use plain text mode for migration,
    /// rather than TLS.  This is the default.  (Since 2.7)
    #[qapi(name = "tls-creds")]
    #[qapi(added = "2.10.0")]
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
    #[qapi(added = "2.10.0")]
    pub tls_hostname: Option<StrOrNull>,
    /// ID of the 'tls-creds' object that provides credentials
    /// for establishing a TLS connection over the migration data
    /// channel. On the outgoing side of the migration, the credentials
    /// must be for a 'client' endpoint, while for the incoming side the
    /// credentials must be for a 'server' endpoint. Setting this
    /// will enable TLS for all migrations. The default is unset,
    /// resulting in unsecured migration at the QEMU level. (Since 2.7)
    /// An empty string means that QEMU will use plain text mode for
    /// migration, rather than TLS (Since 2.9)
    #[qapi(name = "tls-creds")]
    #[qapi(added = "2.7.0")]
    #[qapi(removed = "2.10.0")]
    pub tls_creds_legacy: Option<String>,
    /// hostname of the target host for the migration. This
    /// is required when using x509 based TLS credentials and the
    /// migration URI does not already include a hostname. For
    /// example if using fd: or exec: based migration, the
    /// hostname must be provided so that the server's x509
    /// certificate identity can be validated. (Since 2.7)
    /// An empty string means that QEMU will use the hostname
    /// associated with the migration URI, if any. (Since 2.9)
    #[qapi(name = "tls-hostname")]
    #[qapi(added = "2.7.0")]
    #[qapi(removed = "2.10.0")]
    pub tls_hostname_legacy: Option<String>,
    /// Dirtyrate limit (MB/s) during live migration.
    /// Defaults to 1.  (Since 8.1)
    #[qapi(name = "vcpu-dirty-limit")]
    #[qapi(added = "8.1.0")]
    pub vcpu_dirty_limit: Option<u64>,
    /// The delay time (in ms) between two COLO
    /// checkpoints in periodic mode.  (Since 2.8)
    #[qapi(name = "x-checkpoint-delay")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "6.0.0")]
    pub x_checkpoint_delay: Option<u32>,
    #[qapi(name = "x-cpu-throttle-increment")]
    #[qapi(added = "2.5.0")]
    #[qapi(removed = "2.7.0")]
    pub x_cpu_throttle_increment: Option<i64>,
    #[qapi(name = "x-cpu-throttle-initial")]
    #[qapi(added = "2.5.0")]
    #[qapi(removed = "2.7.0")]
    pub x_cpu_throttle_initial: Option<i64>,
    /// Number of channels used to migrate data in
    /// parallel. This is the same number that the
    /// number of sockets used for migration.  The
    /// default value is 2 (since 2.11)
    #[qapi(name = "x-multifd-channels")]
    #[qapi(added = "2.11.0")]
    #[qapi(removed = "4.0.0")]
    pub x_multifd_channels: Option<i64>,
    /// Number of pages sent together to a thread.
    /// The default value is 16 (since 2.11)
    #[qapi(name = "x-multifd-page-count")]
    #[qapi(added = "2.11.0")]
    #[qapi(removed = "4.0.0")]
    pub x_multifd_page_count: Option<i64>,
    /// RDMA memory registration chunk size in bytes.
    /// Default is 1MiB.  Must be a power of 2 in the range
    /// [1MiB, 1024MiB].  Only applies when migrating via RDMA.
    /// Must be set to the same value on both source and destination
    /// before migration starts.  (Since 11.1)
    #[qapi(name = "x-rdma-chunk-size")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "11.1.0")]
    pub x_rdma_chunk_size: Option<u64>,
    /// Periodic time (in milliseconds) of dirty
    /// limit during live migration.  Should be in the range 1 to
    /// 1000ms.  Defaults to 1000ms.  (Since 8.1)
    #[qapi(name = "x-vcpu-dirty-limit-period")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "8.1.0")]
    pub x_vcpu_dirty_limit_period: Option<u64>,
    /// the delay time between two COLO checkpoints. (Since 2.8)
    #[qapi(name = "x-checkpoint-delay")]
    #[qapi(added = "2.8.0")]
    #[qapi(removed = "6.0.0")]
    pub x_checkpoint_delay_legacy: Option<i64>,
    /// cache size to be used by XBZRLE migration.  It
    /// needs to be a multiple of the target page size and a power of 2
    /// (Since 2.11)
    #[qapi(name = "xbzrle-cache-size")]
    #[qapi(added = "2.11.0")]
    pub xbzrle_cache_size: Option<u64>,
    /// Whether and how to detect zero pages.  See
    /// description in `ZeroPageDetection`.  Default is 'multifd'.
    /// (since 9.0)
    #[qapi(name = "zero-page-detection")]
    #[qapi(added = "9.0.0")]
    pub zero_page_detection: Option<ZeroPageDetection>,
}

/// Set maximum speed for migration.
///
/// Features:
/// @deprecated: This command is deprecated.  Use
/// ```text
///              'migrate-set-parameters' instead.
/// ```
///
/// Returns: nothing on success
///
///
/// Example:
///
/// -> { "execute": "migrate_set_speed", "arguments": { "value": 1024 } }
/// <- { "return": {} }
#[qapi(command)]
#[qapi(name = "migrate_set_speed")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.1.0")]
#[qapi(deprecated(added = "5.0.0"))]
#[qapi(removed = "6.0.0")]
pub struct MigrateSetSpeed {
    /// maximum speed in bytes per second.
    pub value: i64,
}

/// Followup to a migration command to switch the migration to postcopy
/// mode.  The postcopy-ram capability must be set on both source and
/// destination before the original migration command.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "migrate-start-postcopy" }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "migrate-start-postcopy")]
#[qapi(returns = "()")]
#[qapi(added = "2.5.0")]
pub struct MigrateStartPostcopy {}

/// Export a block node to QEMU's embedded NBD server.
///
/// The export name will be used as the id for the resulting block
/// export.
///
/// Features:
///
/// @deprecated: This command is deprecated.  Use `block-export-add`
/// ```text
///     instead.
/// ```
///
/// Errors:
/// ```text
///     - if the server is not running
///     - if an export with the same name already exists
/// ```
#[qapi(command)]
#[qapi(name = "nbd-server-add")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.3.0")]
#[qapi(deprecated(added = "5.2.0"))]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct NbdServerAdd {
    /// Also export a single dirty bitmap reachable from @device,
    /// so the NBD client can use NBD_OPT_SET_META_CONTEXT with the
    /// metadata context name "qemu:dirty-bitmap:BITMAP" to inspect the
    /// bitmap (since 4.0).
    #[qapi(added = "4.0.0")]
    pub bitmap: Option<String>,
    /// Free-form description of the export, up to 4096 bytes.
    /// (Since 5.0)
    #[qapi(added = "5.0.0")]
    pub description: Option<String>,
    /// The device name or node name of the node to be exported
    pub device: String,
    /// Export name.  If unspecified, the @device parameter is used
    /// as the export name.  (Since 2.12)
    #[qapi(added = "2.12.0")]
    pub name: Option<String>,
    /// Whether clients should be able to write to the device via
    /// the NBD connection (default false).
    pub writable: Option<bool>,
}

/// Remove NBD export by name.
///
/// Features:
///
/// @deprecated: This command is deprecated.  Use `block-export-del`
/// ```text
///     instead.
/// ```
///
/// Errors:
/// ```text
///     - if the server is not running
///     - if export is not found
///     - if mode is 'safe' and there are existing connections
/// ```
#[qapi(command)]
#[qapi(name = "nbd-server-remove")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "2.12.0")]
#[qapi(deprecated(added = "5.2.0"))]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct NbdServerRemove {
    /// Mode of command operation.  See `BlockExportRemoveMode`
    /// description.  Default is 'safe'.
    #[qapi(added = "5.2.0")]
    pub mode: Option<BlockExportRemoveMode>,
    /// Mode of command operation. See @NbdServerRemoveMode description.
    /// Default is 'safe'.
    #[qapi(name = "mode")]
    #[qapi(removed = "5.2.0")]
    pub mode_legacy: Option<NbdServerRemoveMode>,
    /// Block export id.
    pub name: String,
}

/// Start an NBD server listening on the given host and port.  Block
/// devices can then be exported using `nbd-server-add`.  The NBD server
/// will present them as named exports; for example, another QEMU
/// instance could refer to them as "nbd:HOST:PORT:exportname=NAME".
///
/// Errors:
/// ```text
///     - if the server is already running
/// ```
#[qapi(command)]
#[qapi(name = "nbd-server-start")]
#[qapi(returns = "()")]
#[qapi(added = "1.3.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct NbdServerStart {
    /// Address on which to listen (since 1.3).
    #[qapi(added = "2.10.0")]
    #[qapi(required)]
    pub addr: Option<SocketAddressLegacy>,
    /// Address on which to listen.
    #[qapi(name = "addr")]
    #[qapi(removed = "2.10.0")]
    #[qapi(required)]
    pub addr_legacy: Option<SocketAddress>,
    /// Time limit, in seconds, at which a client
    /// that has not completed the negotiation handshake will be
    /// disconnected, or 0 for no limit (since 10.0; default: 10).
    #[qapi(name = "handshake-max-seconds")]
    #[qapi(added = "10.0.0")]
    pub handshake_max_seconds: Option<u32>,
    /// The maximum number of connections to allow at the
    /// same time, 0 for unlimited.  Setting this to 1 also stops the
    /// server from advertising multiple client support (since 5.2;
    /// default: 100).
    #[qapi(name = "max-connections")]
    #[qapi(added = "5.2.0")]
    pub max_connections: Option<u32>,
    /// ID of the QAuthZ authorization object used to validate
    /// the client's x509 distinguished name.  This object is is only
    /// resolved at time of use, so can be deleted and recreated on the
    /// fly while the NBD server is active.  If missing, it will default
    /// to denying access (since 4.0).
    #[qapi(name = "tls-authz")]
    #[qapi(added = "4.0.0")]
    pub tls_authz: Option<String>,
    /// ID of the TLS credentials object (since 2.6).
    #[qapi(name = "tls-creds")]
    #[qapi(added = "2.6.0")]
    pub tls_creds: Option<String>,
}

/// Stop QEMU's embedded NBD server, and unregister all devices
/// previously added via `nbd-server-add`.
#[qapi(command)]
#[qapi(name = "nbd-server-stop")]
#[qapi(returns = "()")]
#[qapi(added = "1.3.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct NbdServerStop {}

/// Add a network backend.
///
/// Additional arguments depend on the type.
///
///
/// Errors:
/// ```text
///     - If @type is not a valid network backend, DeviceNotFound
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "netdev_add",
///          "arguments": { "type": "user", "id": "netdev1",
///                         "dnssearch": [ { "str": "example.org" } ] } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "netdev_add")]
#[qapi(returns = "()")]
#[qapi(added = "1.2.0")]
#[qapi(allow_preconfig(added = "6.1.0"))]
pub struct NetdevAdd {
    /// Additional command arguments not modeled by this schema.
    #[qapi(flatten)]
    #[qapi(removed = "5.0.0")]
    pub arguments: Option<HashMap<String, serde_json::Value>>,
    /// identifier for monitor commands.
    pub id: String,
    /// #optional a list of properties to be passed to the backend in
    /// the format 'name=value', like 'ifname=tap0,script=no'
    #[qapi(removed = "2.5.0")]
    pub props: Option<serde_json::Value>,
    /// Specify the driver used for interpreting remaining arguments.
    #[qapi(discriminator)]
    #[qapi(added = "5.0.0")]
    #[qapi(required)]
    pub r#type: Option<NetClientDriver>,
    /// the type of network backend. Possible values are listed in
    /// NetClientDriver (excluding 'none' and 'nic')
    #[qapi(name = "type")]
    #[qapi(removed = "5.0.0")]
    #[qapi(required)]
    pub type_legacy: Option<String>,
    #[qapi(union)]
    #[qapi(flatten)]
    #[qapi(added = "5.0.0")]
    pub u: Option<NetdevBranch>,
}

/// Remove a network backend.
///
/// Errors:
/// ```text
///     - If @id is not a valid network backend, DeviceNotFound
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "netdev_del", "arguments": { "id": "netdev1" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "netdev_del")]
#[qapi(returns = "()")]
#[qapi(added = "1.2.0")]
#[qapi(allow_preconfig(added = "6.1.0"))]
pub struct NetdevDel {
    /// the name of the network backend to remove
    pub id: String,
}

/// Create a QOM object.
///
/// Errors:
/// ```text
///     - If @qom-type is not a valid class name
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "object-add",
///          "arguments": { "qom-type": "rng-random", "id": "rng1",
///                         "filename": "/dev/hwrng" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "object-add")]
#[qapi(returns = "()")]
#[qapi(added = "2.0.0")]
#[qapi(allow_preconfig(added = "6.1.0"))]
pub struct ObjectAdd {
    /// Additional command arguments not modeled by this schema.
    #[qapi(flatten)]
    #[qapi(removed = "6.0.0")]
    pub arguments: Option<HashMap<String, serde_json::Value>>,
    /// the name of the new object
    pub id: String,
    /// a dictionary of properties to be passed to the backend. Deprecated
    /// since 5.0, specify the properties on the top level instead. It is an
    /// error to specify the same option both on the top level and in @props.
    #[qapi(removed = "6.0.0")]
    pub props: Option<serde_json::Value>,
    /// the class name for the object to be created
    #[qapi(name = "qom-type")]
    #[qapi(discriminator)]
    #[qapi(added = "6.0.0")]
    #[qapi(required)]
    pub qom_type: Option<ObjectType>,
    /// the class name for the object to be created
    #[qapi(name = "qom-type")]
    #[qapi(removed = "6.0.0")]
    #[qapi(required)]
    pub qom_type_legacy: Option<String>,
    #[qapi(union)]
    #[qapi(flatten)]
    #[qapi(added = "6.0.0")]
    pub u: Option<ObjectOptionsBranch>,
}

/// Remove a QOM object.
///
/// Errors:
/// ```text
///     - If @id is not a valid id for a QOM object
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "object-del", "arguments": { "id": "rng1" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "object-del")]
#[qapi(returns = "()")]
#[qapi(added = "2.0.0")]
#[qapi(allow_preconfig(added = "6.1.0"))]
pub struct ObjectDel {
    /// the name of the QOM object to remove
    pub id: String,
}

/// Save a portion of guest physical memory to a file.
///
///
/// .. caution:: Errors were not reliably returned until 1.1.
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "pmemsave",
///          "arguments": { "val": 10,
///                         "size": 100,
///                         "filename": "/tmp/physical-mem-dump" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "pmemsave")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct Pmemsave {
    /// the file to save the memory to as binary data
    pub filename: String,
    /// the size of memory region to save
    #[qapi(added = "9.1.0")]
    #[qapi(required)]
    pub size: Option<u64>,
    /// the size of memory region to save
    #[qapi(name = "size")]
    #[qapi(removed = "9.1.0")]
    #[qapi(required)]
    pub size_legacy: Option<i64>,
    /// the physical address of the guest to start from
    #[qapi(added = "9.1.0")]
    #[qapi(required)]
    pub val: Option<u64>,
    /// the physical address of the guest to start from
    #[qapi(name = "val")]
    #[qapi(removed = "9.1.0")]
    #[qapi(required)]
    pub val_legacy: Option<i64>,
}

/// Enable QMP capabilities.
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "qmp_capabilities",
///          "arguments": { "enable": [ "oob" ] } }
///     <- { "return": {} }
/// ```
///
/// .. note:: This command is valid exactly when first connecting: it
/// ```text
///    must be issued before any other command will be accepted, and
///    will fail once the monitor is accepting other commands.  (see
///    :doc:`/interop/qmp-spec`)
/// ```
///
/// .. note:: The QMP client needs to explicitly enable QMP
/// ```text
///    capabilities, otherwise all the QMP capabilities will be turned
///    off by default.
/// ```
#[qapi(command)]
#[qapi(name = "qmp_capabilities")]
#[qapi(returns = "()")]
#[qapi(added = "2.8.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QmpCapabilities {
    /// An optional list of `QMPCapability` values to enable.  The
    /// client must not enable any capability that is not mentioned in
    /// the QMP greeting message.  If the field is not provided, it
    /// means no QMP capabilities will be enabled.  (since 2.12)
    #[qapi(added = "2.12.0")]
    pub enable: Option<Vec<QmpCapability>>,
}

/// Get a property value.
///
/// Returns: The property value.  The type depends on the property type.
/// ```text
///     child<> and link<> properties are returned as #str pathnames.
///     All integer property types (u8, u16, etc) are returned as #int.
/// ```
///
///
/// .. qmp-example::
/// ```text
///    :title: Use absolute path
/// ```
///
/// ```text
///     -> { "execute": "qom-get",
///          "arguments": { "path": "/machine/unattached/device[0]",
///                         "property": "hotplugged" } }
///     <- { "return": false }
/// ```
///
/// .. qmp-example::
/// ```text
///    :title: Use partial path
/// ```
///
/// ```text
///     -> { "execute": "qom-get",
///          "arguments": { "path": "unattached/sysbus",
///                         "property": "type" } }
///     <- { "return": "System" }
/// ```
#[qapi(command)]
#[qapi(name = "qom-get")]
#[qapi(returns = "serde_json::Value")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QomGet {
    /// Additional command arguments not modeled by this schema.
    #[qapi(flatten)]
    #[qapi(removed = "2.5.0")]
    pub arguments: Option<HashMap<String, serde_json::Value>>,
    /// The path within the object model.  There are two forms of
    /// supported paths--absolute and partial paths.
    ///
    /// Absolute paths are derived from the root object and can follow
    /// child<> or link<> properties.  Since they can follow link<>
    /// properties, they can be arbitrarily long.  Absolute paths look
    /// like absolute filenames and are prefixed with a leading slash.
    ///
    /// Partial paths look like relative filenames.  They do not begin
    /// with a prefix.  The matching rules for partial paths are subtle
    /// but designed to make specifying objects easy.  At each level of
    /// the composition tree, the partial path is matched as an absolute
    /// path.  The first match is not returned.  At least two matches
    /// are searched for.  A successful result is only returned if only
    /// one match is found.  If more than one match is found, a flag is
    /// return to indicate that the match was ambiguous.
    pub path: String,
    /// The property name to read
    pub property: String,
}

/// List properties of a object given a path in the object model.
///
/// Returns: a list that describe the properties of the object.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "qom-list",
///          "arguments": { "path": "/chardevs" } }
///     <- { "return": [ { "name": "type", "type": "string" },
///                      { "name": "parallel0", "type": "child<chardev-vc>" },
///                      { "name": "serial0", "type": "child<chardev-vc>" },
///                      { "name": "mon0", "type": "child<chardev-stdio>" } ] }
/// ```
#[qapi(command)]
#[qapi(name = "qom-list")]
#[qapi(returns = "Vec<ObjectPropertyInfo>")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QomList {
    /// the path within the object model.  See `qom-get` for a
    /// description of this parameter.
    pub path: String,
}

/// List properties and their values for each object path in the input
/// list.
///
/// Errors:
/// ```text
///     - If any path is not valid or is ambiguous
/// ```
///
/// Returns: A list where each element is the result for the
/// ```text
///     corresponding element of @paths.
/// ```
#[qapi(command)]
#[qapi(name = "qom-list-get")]
#[qapi(returns = "Vec<ObjectPropertiesValues>")]
#[qapi(added = "10.1.0")]
#[qapi(allow_preconfig)]
pub struct QomListGet {
    /// The absolute or partial path for each object, as described
    /// in `qom-get`.
    pub paths: Vec<String>,
}

/// List properties associated with a QOM object.
///
/// .. note:: Objects can create properties at runtime, for example to
/// ```text
///    describe links between different devices and/or objects.  These
///    properties are not included in the output of this command.
/// ```
///
/// Returns: a list describing object properties
#[qapi(command)]
#[qapi(name = "qom-list-properties")]
#[qapi(returns = "Vec<ObjectPropertyInfo>")]
#[qapi(added = "2.12.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QomListProperties {
    /// the type name of an object
    pub typename: String,
}

/// Return a list of types given search parameters.
///
/// Returns: a list of types, or an empty list if no results are found
#[qapi(command)]
#[qapi(name = "qom-list-types")]
#[qapi(returns = "Vec<ObjectTypeInfo>")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QomListTypes {
    /// if true, include abstract types in the results
    pub r#abstract: Option<bool>,
    /// if specified, only return types that implement this
    /// type name
    pub implements: Option<String>,
}

/// Set a property value.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "qom-set",
///          "arguments": { "path": "/machine",
///                         "property": "graphics",
///                         "value": false } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "qom-set")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QomSet {
    /// Additional command arguments not modeled by this schema.
    #[qapi(flatten)]
    #[qapi(removed = "2.5.0")]
    pub arguments: Option<HashMap<String, serde_json::Value>>,
    /// see `qom-get` for a description of this parameter
    pub path: String,
    /// the property name to set
    pub property: String,
    /// a value who's type is appropriate for the property type.
    /// See `qom-get` for a description of type mapping.
    pub value: serde_json::Value,
}

/// Return information about accelerators
///
/// Returns: @AcceleratorInfo
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-accelerators" }
///     <- { "return": { "enabled": "mshv", "present": ["kvm", "mshv", "qtest", "tcg"] } }
/// ```
#[qapi(command)]
#[qapi(name = "query-accelerators")]
#[qapi(returns = "AcceleratorInfo")]
#[qapi(added = "10.2.0")]
pub struct QueryAccelerators {}

/// Return a list of `ACPIOSTInfo` for devices that support status
/// reporting via ACPI _OST method.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-acpi-ospm-status" }
///     <- { "return": [ { "device": "d1", "slot": "0", "slot-type": "DIMM", "source": 1, "status": 0},
///                      { "slot": "1", "slot-type": "DIMM", "source": 0, "status": 0},
///                      { "slot": "2", "slot-type": "DIMM", "source": 0, "status": 0},
///                      { "slot": "3", "slot-type": "DIMM", "source": 0, "status": 0}
///        ]}
/// ```
#[qapi(command)]
#[qapi(name = "query-acpi-ospm-status")]
#[qapi(returns = "Vec<AcpiostInfo>")]
#[qapi(added = "2.1.0")]
pub struct QueryAcpiOspmStatus {}

/// Return information about audiodev configuration
#[qapi(command)]
#[qapi(name = "query-audiodevs")]
#[qapi(returns = "Vec<Audiodev>")]
#[qapi(added = "8.0.0")]
pub struct QueryAudiodevs {}

/// Return information about the balloon device.
///
/// Errors:
/// ```text
///     - If the balloon driver is enabled but not functional because
///       the KVM kernel module cannot support it, KVMMissingCap
///     - If no balloon device is present, DeviceNotActive
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-balloon" }
///     <- { "return": {
///              "actual": 1073741824
///           }
///        }
/// ```
#[qapi(command)]
#[qapi(name = "query-balloon")]
#[qapi(returns = "BalloonInfo")]
#[qapi(added = "1.0.0")]
pub struct QueryBalloon {}

/// Get a list of `BlockInfo` for all virtual block devices.
///
/// Returns: a list describing each virtual block device.  Filter nodes
/// ```text
///     that were created implicitly are skipped over.
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-block" }
///     <- {
///           "return":[
///              {
///                 "io-status": "ok",
///                 "device":"ide0-hd0",
///                 "locked":false,
///                 "removable":false,
///                 "inserted":{
///                    "ro":false,
///                    "drv":"qcow2",
///                    "encrypted":false,
///                    "file":"disks/test.qcow2",
///                    "backing_file_depth":1,
///                    "bps":1000000,
///                    "bps_rd":0,
///                    "bps_wr":0,
///                    "iops":1000000,
///                    "iops_rd":0,
///                    "iops_wr":0,
///                    "bps_max": 8000000,
///                    "bps_rd_max": 0,
///                    "bps_wr_max": 0,
///                    "iops_max": 0,
///                    "iops_rd_max": 0,
///                    "iops_wr_max": 0,
///                    "iops_size": 0,
///                    "detect_zeroes": "on",
///                    "write_threshold": 0,
///                    "image":{
///                       "filename":"disks/test.qcow2",
///                       "format":"qcow2",
///                       "virtual-size":2048000,
///                       "backing_file":"base.qcow2",
///                       "full-backing-filename":"disks/base.qcow2",
///                       "backing-filename-format":"qcow2",
///                       "snapshots":[
///                          {
///                             "id": "1",
///                             "name": "snapshot1",
///                             "vm-state-size": 0,
///                             "date-sec": 10000200,
///                             "date-nsec": 12,
///                             "vm-clock-sec": 206,
///                             "vm-clock-nsec": 30
///                          }
///                       ],
///                       "backing-image":{
///                           "filename":"disks/base.qcow2",
///                           "format":"qcow2",
///                           "virtual-size":2048000
///                       }
///                    }
///                 },
///                 "qdev": "ide_disk",
///                 "type":"unknown"
///              },
///              {
///                 "io-status": "ok",
///                 "device":"ide1-cd0",
///                 "locked":false,
///                 "removable":true,
///                 "qdev": "/machine/unattached/device[23]",
///                 "tray_open": false,
///                 "type":"unknown"
///              },
///              {
///                 "device":"floppy0",
///                 "locked":false,
///                 "removable":true,
///                 "qdev": "/machine/unattached/device[20]",
///                 "type":"unknown"
///              },
///              {
///                 "device":"sd0",
///                 "locked":false,
///                 "removable":true,
///                 "type":"unknown"
///              }
///           ]
///        }
/// ```
#[qapi(command)]
#[qapi(name = "query-block")]
#[qapi(returns = "Vec<BlockInfo>")]
#[qapi(added = "1.0.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct QueryBlock {
    /// Omit nested data about the backing image, i.e. `BlockInfo`
    /// member 'inserted.image.backing-image' will be absent.
    /// Default is false.  (Since 11.0)
    #[qapi(added = "11.0.0")]
    pub flat: Option<bool>,
}

/// Returns: A list describing all block exports
#[qapi(command)]
#[qapi(name = "query-block-exports")]
#[qapi(returns = "Vec<BlockExportInfo>")]
#[qapi(added = "5.2.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct QueryBlockExports {}

/// Return information about long-running block device operations.
///
/// Returns: a list of job info for each active block job
#[qapi(command)]
#[qapi(name = "query-block-jobs")]
#[qapi(returns = "Vec<BlockJobInfo>")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct QueryBlockJobs {}

/// Query the `BlockStats` for all virtual block devices.
///
/// Returns: A list of statistics for each virtual block device.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-blockstats" }
///     <- {
///           "return":[
///              {
///                 "device":"ide0-hd0",
///                 "parent":{
///                    "stats":{
///                       "wr_highest_offset":3686448128,
///                       "wr_bytes":9786368,
///                       "wr_operations":751,
///                       "rd_bytes":122567168,
///                       "rd_operations":36772
///                       "wr_total_times_ns":313253456
///                       "rd_total_times_ns":3465673657
///                       "flush_total_times_ns":49653
///                       "flush_operations":61,
///                       "rd_merged":0,
///                       "wr_merged":0,
///                       "idle_time_ns":2953431879,
///                       "account_invalid":true,
///                       "account_failed":false
///                    }
///                 },
///                 "stats":{
///                    "wr_highest_offset":2821110784,
///                    "wr_bytes":9786368,
///                    "wr_operations":692,
///                    "rd_bytes":122739200,
///                    "rd_operations":36604
///                    "flush_operations":51,
///                    "wr_total_times_ns":313253456
///                    "rd_total_times_ns":3465673657
///                    "flush_total_times_ns":49653,
///                    "rd_merged":0,
///                    "wr_merged":0,
///                    "idle_time_ns":2953431879,
///                    "account_invalid":true,
///                    "account_failed":false
///                 },
///                 "qdev": "/machine/unattached/device[23]"
///              },
///              {
///                 "device":"ide1-cd0",
///                 "stats":{
///                    "wr_highest_offset":0,
///                    "wr_bytes":0,
///                    "wr_operations":0,
///                    "rd_bytes":0,
///                    "rd_operations":0
///                    "flush_operations":0,
///                    "wr_total_times_ns":0
///                    "rd_total_times_ns":0
///                    "flush_total_times_ns":0,
///                    "rd_merged":0,
///                    "wr_merged":0,
///                    "account_invalid":false,
///                    "account_failed":false
///                 },
///                 "qdev": "/machine/unattached/device[24]"
///              },
///              {
///                 "device":"floppy0",
///                 "stats":{
///                    "wr_highest_offset":0,
///                    "wr_bytes":0,
///                    "wr_operations":0,
///                    "rd_bytes":0,
///                    "rd_operations":0
///                    "flush_operations":0,
///                    "wr_total_times_ns":0
///                    "rd_total_times_ns":0
///                    "flush_total_times_ns":0,
///                    "rd_merged":0,
///                    "wr_merged":0,
///                    "account_invalid":false,
///                    "account_failed":false
///                 },
///                 "qdev": "/machine/unattached/device[16]"
///              },
///              {
///                 "device":"sd0",
///                 "stats":{
///                    "wr_highest_offset":0,
///                    "wr_bytes":0,
///                    "wr_operations":0,
///                    "rd_bytes":0,
///                    "rd_operations":0
///                    "flush_operations":0,
///                    "wr_total_times_ns":0
///                    "rd_total_times_ns":0
///                    "flush_total_times_ns":0,
///                    "rd_merged":0,
///                    "wr_merged":0,
///                    "account_invalid":false,
///                    "account_failed":false
///                 }
///              }
///           ]
///        }
/// ```
#[qapi(command)]
#[qapi(name = "query-blockstats")]
#[qapi(returns = "Vec<BlockStats>")]
#[qapi(added = "1.0.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct QueryBlockstats {
    /// If true, the command will query all the block nodes
    /// that have a node name, in a list which will include "parent"
    /// information, but not "backing".  If false or omitted, the
    /// behavior is as before - query all the device backends,
    /// recursively including their "parent" and "backing".  Filter
    /// nodes that were created implicitly are skipped over in this
    /// mode.  (Since 2.3)
    #[qapi(name = "query-nodes")]
    #[qapi(added = "2.3.0")]
    pub query_nodes: Option<bool>,
}

/// Return information about current character devices.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-chardev" }
///     <- {
///           "return": [
///              {
///                 "label": "charchannel0",
///                 "filename": "unix:/var/lib/libvirt/qemu/seabios.rhel6.agent,server=on",
///                 "frontend-open": false
///              },
///              {
///                 "label": "charmonitor",
///                 "filename": "unix:/var/lib/libvirt/qemu/seabios.rhel6.monitor,server=on",
///                 "frontend-open": true
///              },
///              {
///                 "label": "charserial0",
///                 "filename": "pty:/dev/pts/2",
///                 "frontend-open": true
///              }
///           ]
///        }
/// ```
#[qapi(command)]
#[qapi(name = "query-chardev")]
#[qapi(returns = "Vec<ChardevInfo>")]
#[qapi(added = "1.0.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryChardev {}

/// Return information about character device backends.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-chardev-backends" }
///     <- {
///           "return":[
///              {
///                 "name":"udp"
///              },
///              {
///                 "name":"tcp"
///              },
///              {
///                 "name":"unix"
///              },
///              {
///                 "name":"spiceport"
///              }
///           ]
///        }
/// ```
#[qapi(command)]
#[qapi(name = "query-chardev-backends")]
#[qapi(returns = "Vec<ChardevBackendInfo>")]
#[qapi(added = "2.0.0")]
pub struct QueryChardevBackends {}

/// Query COLO status while the vm is running.
///
/// TODO: This line is a hack to separate the example from the body
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-colo-status" }
///     <- { "return": { "mode": "primary", "last-mode": "none", "reason": "request" } }
/// ```
#[qapi(command)]
#[qapi(name = "query-colo-status")]
#[qapi(returns = "ColoStatus")]
#[qapi(condition = "CONFIG_REPLICATION")]
#[qapi(added = "3.1.0")]
pub struct QueryColoStatus {}

/// Query command line option schema.
///
/// Returns: list of objects for all options (or for the given @option).
///
/// Errors:
/// ```text
///     - if the given @option doesn't exist
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-command-line-options",
///          "arguments": { "option": "option-rom" } }
///     <- { "return": [
///             {
///                 "parameters": [
///                     {
///                         "name": "romfile",
///                         "type": "string"
///                     },
///                     {
///                         "name": "bootindex",
///                         "type": "number"
///                     }
///                 ],
///                 "option": "option-rom"
///             }
///          ]
///        }
/// ```
#[qapi(command)]
#[qapi(name = "query-command-line-options")]
#[qapi(returns = "Vec<CommandLineOptionInfo>")]
#[qapi(added = "1.5.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryCommandLineOptions {
    /// option name
    pub option: Option<String>,
}

/// Return a list of supported QMP commands by this server
///
/// Returns: A list of all supported commands
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-commands" }
///     <- {
///          "return":[
///             {
///                "name":"query-balloon"
///             },
///             {
///                "name":"system_powerdown"
///             },
///             ...
///          ]
///        }
/// ```
///
/// This example has been shortened as the real response is too long.
#[qapi(command)]
#[qapi(name = "query-commands")]
#[qapi(returns = "Vec<CommandInfo>")]
#[qapi(added = "1.0.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryCommands {}

/// Return a list of supported virtual CPU definitions
#[qapi(command)]
#[qapi(name = "query-cpu-definitions")]
#[qapi(returns = "Vec<CpuDefinitionInfo>")]
#[qapi(
    condition = "defined(TARGET_PPC) || defined(TARGET_ARM) || defined(TARGET_I386) || defined(TARGET_S390X) || defined(TARGET_MIPS)"
)]
#[qapi(added = "1.2.0")]
pub struct QueryCpuDefinitions {}

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
/// * QEMU version: CPU models may look different depending on the QEMU
///   version.  (Except for CPU models reported as "static" in
///   `query-cpu-definitions`.)
/// * machine-type: CPU model may look different depending on the
///   machine-type.  (Except for CPU models reported as "static" in
///   `query-cpu-definitions`.)
/// * machine options (including accelerator): in some architectures,
///   CPU models may look different depending on machine and accelerator
///   options.  (Except for CPU models reported as "static" in
///   `query-cpu-definitions`.)
/// * "-cpu" arguments and global properties: arguments to the -cpu
///   option and global properties may affect expansion of CPU models.
///   Using `query-cpu-model-expansion` while using these is not
///   advised.
///
/// Some architectures may not support baselining CPU models.  s390x
/// supports baselining CPU models.
///
/// Returns: a `CpuModelBaselineInfo` describing the baselined CPU model
///
/// Errors:
/// ```text
///     - if baselining CPU models is not supported by the target
///     - if a model cannot be used
///     - if a model contains an unknown cpu definition name, unknown
///       properties or properties with wrong types.
/// ```
#[qapi(command)]
#[qapi(name = "query-cpu-model-baseline")]
#[qapi(returns = "CpuModelBaselineInfo")]
#[qapi(condition = "defined(TARGET_S390X)")]
#[qapi(added = "2.8.0")]
pub struct QueryCpuModelBaseline {
    /// description of the first CPU model to baseline
    pub modela: CpuModelInfo,
    /// description of the second CPU model to baseline
    pub modelb: CpuModelInfo,
}

/// Compares two CPU models, @modela and @modelb, returning how they
/// compare in a specific configuration.  The results indicates how both
/// models compare regarding runnability.  This result can be used by
/// tooling to make decisions if a certain CPU model will run in a
/// certain configuration or if a compatible CPU model has to be created
/// by baselining.
///
/// Usually, a CPU model is compared against the maximum possible CPU
/// model of a certain configuration (e.g. the "host" model for KVM).
/// If that CPU model is identical or a subset, it will run in that
/// configuration.
///
/// The result returned by this command may be affected by:
///
/// * QEMU version: CPU models may look different depending on the QEMU
///   version.  (Except for CPU models reported as "static" in
///   `query-cpu-definitions`.)
/// * machine-type: CPU model may look different depending on the
///   machine-type.  (Except for CPU models reported as "static" in
///   `query-cpu-definitions`.)
/// * machine options (including accelerator): in some architectures,
///   CPU models may look different depending on machine and accelerator
///   options.  (Except for CPU models reported as "static" in
///   `query-cpu-definitions`.)
/// * "-cpu" arguments and global properties: arguments to the -cpu
///   option and global properties may affect expansion of CPU models.
///   Using `query-cpu-model-expansion` while using these is not
///   advised.
///
/// Some architectures may not support comparing CPU models.  s390x
/// supports comparing CPU models.
///
/// Returns: a `CpuModelCompareInfo` describing how both CPU models
/// ```text
///     compare
/// ```
///
/// Errors:
/// ```text
///     - if comparing CPU models is not supported by the target
///     - if a model cannot be used
///     - if a model contains an unknown cpu definition name, unknown
///       properties or properties with wrong types.
/// ```
#[qapi(command)]
#[qapi(name = "query-cpu-model-comparison")]
#[qapi(returns = "CpuModelCompareInfo")]
#[qapi(condition = "defined(TARGET_S390X)")]
#[qapi(added = "2.8.0")]
pub struct QueryCpuModelComparison {
    /// description of the first CPU model to compare, referred to
    /// as "model A" in `CpuModelCompareResult`
    pub modela: CpuModelInfo,
    /// description of the second CPU model to compare, referred to
    /// as "model B" in `CpuModelCompareResult`
    pub modelb: CpuModelInfo,
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
/// * QEMU version: CPU models may look different depending on the QEMU
///   version.  (Except for CPU models reported as "static" in
///   `query-cpu-definitions`.)
/// * machine-type: CPU model may look different depending on the
///   machine-type.  (Except for CPU models reported as "static" in
///   `query-cpu-definitions`.)
/// * machine options (including accelerator): in some architectures,
///   CPU models may look different depending on machine and accelerator
///   options.  (Except for CPU models reported as "static" in
///   `query-cpu-definitions`.)
/// * "-cpu" arguments and global properties: arguments to the -cpu
///   option and global properties may affect expansion of CPU models.
///   Using `query-cpu-model-expansion` while using these is not
///   advised.
///
/// Some architectures may not support all expansion types.  s390x
/// supports "full" and "static".  Arm only supports "full".
///
/// Returns: a `CpuModelExpansionInfo` describing the expanded CPU model
///
/// Errors:
/// ```text
///     - if expanding CPU models is not supported
///     - if the model cannot be expanded
///     - if the model contains an unknown CPU definition name, unknown
///       properties or properties with a wrong type
///     - if an expansion type is not supported
/// ```
#[qapi(command)]
#[qapi(name = "query-cpu-model-expansion")]
#[qapi(returns = "CpuModelExpansionInfo")]
#[qapi(condition = "defined(TARGET_S390X) || defined(TARGET_I386)")]
#[qapi(added = "2.8.0")]
pub struct QueryCpuModelExpansion {
    /// description of the CPU model to expand
    pub model: CpuModelInfo,
    /// expansion type, specifying how to expand the CPU model
    pub r#type: CpuModelExpansionType,
}

/// Returns a list of information about each virtual CPU.
///
/// This command causes vCPU threads to exit to userspace, which causes
/// a small interruption to guest CPU execution. This will have a negative
/// impact on realtime guests and other latency sensitive guest workloads.
///
/// Features:
/// @deprecated: This command is deprecated, because it interferes with
/// ```text
///              the guest.  Use 'query-cpus-fast' instead to avoid the vCPU
///              interruption.
/// ```
///
/// Returns: a list of @CpuInfo for each virtual CPU
///
///
/// Example:
///
/// -> { "execute": "query-cpus" }
/// <- { "return": [
/// ```text
///          {
///             "CPU":0,
///             "current":true,
///             "halted":false,
///             "qom_path":"/machine/unattached/device[0]",
///             "arch":"x86",
///             "pc":3227107138,
///             "thread_id":3134
///          },
///          {
///             "CPU":1,
///             "current":false,
///             "halted":true,
///             "qom_path":"/machine/unattached/device[2]",
///             "arch":"x86",
///             "pc":7108165,
///             "thread_id":3135
///          }
///       ]
///    }
/// ```
#[qapi(command)]
#[qapi(name = "query-cpus")]
#[qapi(returns = "Vec<CpuInfo>")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.0.0")]
#[qapi(deprecated(added = "5.0.0"))]
#[qapi(removed = "6.0.0")]
pub struct QueryCpus {}

/// Return information about all virtual CPUs.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-cpus-fast" }
///     <- { "return": [
///             {
///                 "thread-id": 25627,
///                 "props": {
///                     "core-id": 0,
///                     "thread-id": 0,
///                     "socket-id": 0
///                 },
///                 "qom-path": "/machine/unattached/device[0]",
///                 "target":"x86_64",
///                 "cpu-index": 0
///             },
///             {
///                 "thread-id": 25628,
///                 "props": {
///                     "core-id": 0,
///                     "thread-id": 0,
///                     "socket-id": 1
///                 },
///                 "qom-path": "/machine/unattached/device[2]",
///                 "target":"x86_64",
///                 "cpu-index": 1
///             }
///         ]
///     }
/// ```
#[qapi(command)]
#[qapi(name = "query-cpus-fast")]
#[qapi(returns = "Vec<CpuInfoFast>")]
#[qapi(added = "2.12.0")]
pub struct QueryCpusFast {}

/// Return information about current crypto devices.
#[qapi(command)]
#[qapi(name = "query-cryptodev")]
#[qapi(returns = "Vec<QCryptodevInfo>")]
#[qapi(added = "8.0.0")]
pub struct QueryCryptodev {}

/// Return information on the current virtual machine.
#[qapi(command)]
#[qapi(name = "query-current-machine")]
#[qapi(returns = "CurrentMachineParams")]
#[qapi(added = "4.0.0")]
pub struct QueryCurrentMachine {}

/// Query results of the most recent invocation of `calc-dirty-rate`.
///
///
/// .. qmp-example::
/// ```text
///    :title: Measurement is in progress
/// ```
///
/// ```text
///     <- {"status": "measuring", "sample-pages": 512,
///         "mode": "page-sampling", "start-time": 1693900454, "calc-time": 10,
///         "calc-time-unit": "second"}
/// ```
///
/// .. qmp-example::
/// ```text
///    :title: Measurement has been completed
/// ```
///
/// ```text
///     <- {"status": "measured", "sample-pages": 512, "dirty-rate": 108,
///         "mode": "page-sampling", "start-time": 1693900454, "calc-time": 10,
///         "calc-time-unit": "second"}
/// ```
#[qapi(command)]
#[qapi(name = "query-dirty-rate")]
#[qapi(returns = "DirtyRateInfo")]
#[qapi(added = "5.2.0")]
pub struct QueryDirtyRate {
    /// time unit in which to report calculation time.
    /// By default it is reported in seconds.  (Since 8.2)
    #[qapi(name = "calc-time-unit")]
    #[qapi(added = "8.2.0")]
    pub calc_time_unit: Option<TimeUnit>,
}

/// Return information about display configuration
#[qapi(command)]
#[qapi(name = "query-display-options")]
#[qapi(returns = "DisplayOptions")]
#[qapi(added = "3.1.0")]
pub struct QueryDisplayOptions {}

/// Query latest dump status.
///
/// Returns: An object showing the dump status.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-dump" }
///     <- { "return": { "status": "active", "completed": 1024000,
///                      "total": 2048000 } }
/// ```
#[qapi(command)]
#[qapi(name = "query-dump")]
#[qapi(returns = "DumpQueryResult")]
#[qapi(added = "2.6.0")]
pub struct QueryDump {}

/// Return the available formats for `dump-guest-memory`
///
/// Returns: An object listing available formats for `dump-guest-memory`
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-dump-guest-memory-capability" }
///     <- { "return": { "formats":
///                      ["elf", "kdump-zlib", "kdump-lzo", "kdump-snappy"] } }
/// ```
#[qapi(command)]
#[qapi(name = "query-dump-guest-memory-capability")]
#[qapi(returns = "DumpGuestMemoryCapability")]
#[qapi(added = "2.0.0")]
pub struct QueryDumpGuestMemoryCapability {}

/// Return information on QMP events.
///
/// Features:
/// @deprecated: This command is deprecated, because its output doesn't
/// ```text
///              reflect compile-time configuration.  Use 'query-qmp-schema'
///              instead.
/// ```
///
/// Returns: A list of @EventInfo.
///
///
/// Example:
///
/// -> { "execute": "query-events" }
/// <- {
/// ```text
///      "return": [
///          {
///             "name":"SHUTDOWN"
///          },
///          {
///             "name":"RESET"
///          }
///       ]
///    }
/// ```
///
/// Note: This example has been shortened as the real response is too long.
#[qapi(command)]
#[qapi(name = "query-events")]
#[qapi(returns = "Vec<EventInfo>")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.2.0")]
#[qapi(deprecated(added = "5.0.0"))]
#[qapi(removed = "6.0.0")]
pub struct QueryEvents {}

/// Return information describing all fd sets.
///
///
/// .. note:: The list of fd sets is shared by all monitor connections.
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-fdsets" }
///     <- { "return": [
///            {
///              "fds": [
///                {
///                  "fd": 30,
///                  "opaque": "rdonly:/path/to/file"
///                },
///                {
///                  "fd": 24,
///                  "opaque": "rdwr:/path/to/file"
///                }
///              ],
///              "fdset-id": 1
///            },
///            {
///              "fds": [
///                {
///                  "fd": 28
///                },
///                {
///                  "fd": 29
///                }
///              ],
///              "fdset-id": 0
///            }
///          ]
///        }
/// ```
#[qapi(command)]
#[qapi(name = "query-fdsets")]
#[qapi(returns = "Vec<FdsetInfo>")]
#[qapi(added = "1.2.0")]
pub struct QueryFdsets {}

/// Find firmware memory log buffer in guest memory, return content.
#[qapi(command)]
#[qapi(name = "query-firmware-log")]
#[qapi(returns = "FirmwareLog")]
#[qapi(added = "10.2.0")]
pub struct QueryFirmwareLog {
    /// limit the amount of log data returned.  Up to 1 MiB of
    /// log data is allowed.  In case the amount of log data is larger
    /// than @max-size the tail of the log is returned.
    #[qapi(name = "max-size")]
    pub max_size: Option<u64>,
}

/// It will return a list of `GICCapability` objects that describe its
/// capability bits.
///
/// On non-ARM targets this command will report an error as the GIC
/// technology is not applicable.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-gic-capabilities" }
///     <- { "return": [{ "version": 2, "emulated": true, "kernel": false },
///                     { "version": 3, "emulated": false, "kernel": true } ] }
/// ```
#[qapi(command)]
#[qapi(name = "query-gic-capabilities")]
#[qapi(returns = "Vec<GicCapability>")]
#[qapi(condition = "defined(TARGET_ARM)")]
#[qapi(added = "2.6.0")]
pub struct QueryGicCapabilities {}

/// TODO: Better documentation; currently there is none.
///
///
/// .. qmp-example::
/// ```text
///    :annotated:
/// ```
///
/// ```text
///    For pseries machine type started with
///    ``-smp 2,cores=2,maxcpus=4 -cpu POWER8``::
/// ```
///
/// ```text
///     -> { "execute": "query-hotpluggable-cpus" }
///     <- {"return": [
///          { "props": { "core-id": 8 }, "type": "POWER8-spapr-cpu-core",
///            "vcpus-count": 1 },
///          { "props": { "core-id": 0 }, "type": "POWER8-spapr-cpu-core",
///            "vcpus-count": 1, "qom-path": "/machine/unattached/device[0]"}
///        ]}
/// ```
///
/// .. qmp-example::
/// ```text
///    :annotated:
/// ```
///
/// ```text
///    For pc machine type started with ``-smp 1,maxcpus=2``::
/// ```
///
/// ```text
///     -> { "execute": "query-hotpluggable-cpus" }
///     <- {"return": [
///          {
///             "type": "qemu64-x86_64-cpu", "vcpus-count": 1,
///             "props": {"core-id": 0, "socket-id": 1, "thread-id": 0}
///          },
///          {
///             "qom-path": "/machine/unattached/device[0]",
///             "type": "qemu64-x86_64-cpu", "vcpus-count": 1,
///             "props": {"core-id": 0, "socket-id": 0, "thread-id": 0}
///          }
///        ]}
/// ```
///
/// .. qmp-example::
/// ```text
///    :annotated:
/// ```
///
/// ```text
///    For s390x-virtio-ccw machine type started with
///    ``-smp 1,maxcpus=2 -cpu qemu``::
/// ```
///
/// ```text
///     -> { "execute": "query-hotpluggable-cpus" }
///     <- {"return": [
///          {
///             "type": "qemu-s390x-cpu", "vcpus-count": 1,
///             "props": { "core-id": 1 }
///          },
///          {
///             "qom-path": "/machine/unattached/device[0]",
///             "type": "qemu-s390x-cpu", "vcpus-count": 1,
///             "props": { "core-id": 0 }
///          }
///        ]}
/// ```
#[qapi(command)]
#[qapi(name = "query-hotpluggable-cpus")]
#[qapi(returns = "Vec<HotpluggableCpu>")]
#[qapi(added = "2.7.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryHotpluggableCpus {}

/// Return the hv-balloon driver data contained in the last received
/// "STATUS" message from the guest.
///
/// Errors:
/// ```text
///     - If no hv-balloon device is present, guest memory status
///       reporting is not enabled or no guest memory status report
///       received yet, GenericError
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-hv-balloon-status-report" }
///     <- { "return": {
///              "committed": 816640000,
///              "available": 3333054464
///           }
///        }
/// ```
#[qapi(command)]
#[qapi(name = "query-hv-balloon-status-report")]
#[qapi(returns = "HvBalloonInfo")]
#[qapi(added = "8.2.0")]
pub struct QueryHvBalloonStatusReport {}

/// Return a list of information about each iothread.
///
/// .. note:: This list excludes the QEMU main loop thread, which is not
/// ```text
///    declared using the ``-object iothread`` command-line option.  It
///    is always the main thread of the process.
/// ```
///
/// Returns: a list of info for each iothread
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-iothreads" }
///     <- { "return": [
///              {
///                 "id":"iothread0",
///                 "thread-id":3134,
///                 "poll-max-ns":32768,
///                 "poll-grow":0,
///                 "poll-shrink":0,
///                 "aio-max-batch":0
///              },
///              {
///                 "id":"iothread1",
///                 "thread-id":3135,
///                 "poll-max-ns":32768,
///                 "poll-grow":0,
///                 "poll-shrink":0,
///                 "aio-max-batch":0
///              }
///           ]
///        }
/// ```
#[qapi(command)]
#[qapi(name = "query-iothreads")]
#[qapi(returns = "Vec<IoThreadInfo>")]
#[qapi(added = "2.0.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryIothreads {}

/// Return information about jobs.
///
/// Returns: a list with info for each active job
#[qapi(command)]
#[qapi(name = "query-jobs")]
#[qapi(returns = "Vec<JobInfo>")]
#[qapi(added = "3.0.0")]
pub struct QueryJobs {}

/// Return information about KVM acceleration
///
/// Features:
///
/// @deprecated: This command is deprecated.  Use `query-accelerators`
/// ```text
///     instead.
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-kvm" }
///     <- { "return": { "enabled": true, "present": true } }
/// ```
#[qapi(command)]
#[qapi(name = "query-kvm")]
#[qapi(returns = "KvmInfo")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.0.0")]
#[qapi(deprecated(added = "11.1.0"))]
pub struct QueryKvm {}

/// Return a list of supported machines
///
/// Features:
///
/// @unstable: Argument @compat-props is experimental.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-machines", "arguments": { "compat-props": true } }
///     <- { "return": [
///               {
///                  "hotpluggable-cpus": true,
///                  "name": "pc-q35-6.2",
///                  "compat-props": [
///                       {
///                          "qom-type": "virtio-mem",
///                          "property": "unplugged-inaccessible",
///                          "value": "off"
///                       }
///                   ],
///                   "numa-mem-supported": false,
///                   "default-cpu-type": "qemu64-x86_64-cpu",
///                   "cpu-max": 288,
///                   "deprecated": false,
///                   "default-ram-id": "pc.ram"
///               },
///               ...
///        }
/// ```
#[qapi(command)]
#[qapi(name = "query-machines")]
#[qapi(returns = "Vec<MachineInfo>")]
#[qapi(added = "1.2.0")]
pub struct QueryMachines {
    /// if true, also return compatibility properties.
    /// (default: false) (since 9.1)
    #[qapi(name = "compat-props")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "9.1.0")]
    pub compat_props: Option<bool>,
}

/// Return information for all memory backends.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-memdev" }
///     <- { "return": [
///            {
///              "id": "mem1",
///              "size": 536870912,
///              "merge": false,
///              "dump": true,
///              "prealloc": false,
///              "share": false,
///              "host-nodes": [0, 1],
///              "policy": "bind"
///            },
///            {
///              "size": 536870912,
///              "merge": false,
///              "dump": true,
///              "prealloc": true,
///              "share": false,
///              "host-nodes": [2, 3],
///              "policy": "preferred"
///            }
///          ]
///        }
/// ```
#[qapi(command)]
#[qapi(name = "query-memdev")]
#[qapi(returns = "Vec<Memdev>")]
#[qapi(added = "2.1.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryMemdev {}

/// Lists available memory devices and their state
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-memory-devices" }
///     <- { "return": [ { "data":
///                           { "addr": 5368709120,
///                             "hotpluggable": true,
///                             "hotplugged": true,
///                             "id": "d1",
///                             "memdev": "/objects/memX",
///                             "node": 0,
///                             "size": 1073741824,
///                             "slot": 0},
///                        "type": "dimm"
///                      } ] }
/// ```
#[qapi(command)]
#[qapi(name = "query-memory-devices")]
#[qapi(returns = "Vec<MemoryDeviceInfo>")]
#[qapi(added = "2.1.0")]
pub struct QueryMemoryDevices {}

/// Return the amount of initially allocated and present hotpluggable
/// (if enabled) memory in bytes.
///
/// TODO: This line is a hack to separate the example from the body
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-memory-size-summary" }
///     <- { "return": { "base-memory": 4294967296, "plugged-memory": 0 } }
/// ```
#[qapi(command)]
#[qapi(name = "query-memory-size-summary")]
#[qapi(returns = "MemoryInfo")]
#[qapi(added = "2.11.0")]
pub struct QueryMemorySizeSummary {}

/// Return information about each active mouse device
///
/// Returns: a list of info for each device
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-mice" }
///     <- { "return": [
///              {
///                 "name":"QEMU Microsoft Mouse",
///                 "index":0,
///                 "current":false,
///                 "absolute":false
///              },
///              {
///                 "name":"QEMU PS/2 Mouse",
///                 "index":1,
///                 "current":true,
///                 "absolute":true
///              }
///           ]
///        }
/// ```
#[qapi(command)]
#[qapi(name = "query-mice")]
#[qapi(returns = "Vec<MouseInfo>")]
#[qapi(added = "1.0.0")]
pub struct QueryMice {}

/// Return information about current migration process.  If migration is
/// active there will be another json-object with RAM migration status.
///
///
/// .. qmp-example::
/// ```text
///    :title: Before the first migration
/// ```
///
/// ```text
///     -> { "execute": "query-migrate" }
///     <- { "return": {} }
/// ```
///
/// .. qmp-example::
/// ```text
///    :title: Migration is done and has succeeded
/// ```
///
/// ```text
///     -> { "execute": "query-migrate" }
///     <- { "return": {
///             "status": "completed",
///             "total-time":12345,
///             "setup-time":12345,
///             "downtime":12345,
///             "ram":{
///               "transferred":123,
///               "remaining":123,
///               "total":246,
///               "duplicate":123,
///               "normal":123,
///               "normal-bytes":123456,
///               "dirty-sync-count":15
///             }
///          }
///        }
/// ```
///
/// .. qmp-example::
/// ```text
///    :title: Migration is done and has failed
/// ```
///
/// ```text
///     -> { "execute": "query-migrate" }
///     <- { "return": { "status": "failed" } }
/// ```
///
/// .. qmp-example::
/// ```text
///    :title: Migration is being performed
/// ```
///
/// ```text
///     -> { "execute": "query-migrate" }
///     <- {
///           "return":{
///              "status":"active",
///              "total-time":12345,
///              "setup-time":12345,
///              "expected-downtime":12345,
///              "ram":{
///                 "transferred":123,
///                 "remaining":123,
///                 "total":246,
///                 "duplicate":123,
///                 "normal":123,
///                 "normal-bytes":123456,
///                 "dirty-sync-count":15
///              }
///           }
///        }
/// ```
///
/// .. qmp-example::
/// ```text
///    :title: Migration is being performed and XBZRLE is active
/// ```
///
/// ```text
///     -> { "execute": "query-migrate" }
///     <- {
///           "return":{
///              "status":"active",
///              "total-time":12345,
///              "setup-time":12345,
///              "expected-downtime":12345,
///              "ram":{
///                 "total":1057024,
///                 "remaining":1053304,
///                 "transferred":3720,
///                 "duplicate":10,
///                 "normal":3333,
///                 "normal-bytes":3412992,
///                 "dirty-sync-count":15
///              },
///              "xbzrle-cache":{
///                 "cache-size":67108864,
///                 "bytes":20971520,
///                 "pages":2444343,
///                 "cache-miss":2244,
///                 "cache-miss-rate":0.123,
///                 "encoding-rate":80.1,
///                 "overflow":34434
///              }
///           }
///        }
/// ```
#[qapi(command)]
#[qapi(name = "query-migrate")]
#[qapi(returns = "MigrationInfo")]
#[qapi(added = "1.0.0")]
pub struct QueryMigrate {}

/// Query migration XBZRLE cache size
///
/// Features:
/// @deprecated: This command is deprecated.  Use
/// ```text
///              'query-migrate-parameters' instead.
/// ```
///
/// Returns: XBZRLE cache size in bytes
///
///
/// Example:
///
/// -> { "execute": "query-migrate-cache-size" }
/// <- { "return": 67108864 }
#[qapi(command)]
#[qapi(name = "query-migrate-cache-size")]
#[qapi(returns = "i64")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.2.0")]
#[qapi(deprecated(added = "5.0.0"))]
#[qapi(removed = "6.0.0")]
pub struct QueryMigrateCacheSize {}

/// Return information about the current migration capabilities status
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-migrate-capabilities" }
///     <- { "return": [
///           {"state": false, "capability": "xbzrle"},
///           {"state": false, "capability": "rdma-pin-all"},
///           {"state": false, "capability": "auto-converge"},
///           {"state": true, "capability": "events"},
///           {"state": false, "capability": "postcopy-ram"},
///           {"state": false, "capability": "x-colo"}
///        ]}
/// ```
#[qapi(command)]
#[qapi(name = "query-migrate-capabilities")]
#[qapi(returns = "Vec<MigrationCapabilityStatus>")]
#[qapi(added = "1.2.0")]
pub struct QueryMigrateCapabilities {}

/// Return information about the current migration parameters.  Optional
/// members of the return value are always present, except
/// @block-bitmap-mapping, which is only present if it has been
/// previously set.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-migrate-parameters" }
///     <- { "return": {
///              "multifd-channels": 2,
///              "cpu-throttle-increment": 10,
///              "cpu-throttle-initial": 20,
///              "max-bandwidth": 33554432,
///              "downtime-limit": 300
///           }
///        }
/// ```
#[qapi(command)]
#[qapi(name = "query-migrate-parameters")]
#[qapi(returns = "MigrationParameters")]
#[qapi(added = "2.4.0")]
pub struct QueryMigrateParameters {}

/// Return information of migration threads
///
/// Features:
///
/// @deprecated: This command is deprecated with no replacement yet.
#[qapi(command)]
#[qapi(name = "query-migrationthreads")]
#[qapi(returns = "Vec<MigrationThreadInfo>")]
#[qapi(feature = "deprecated")]
#[qapi(added = "8.0.0")]
#[qapi(deprecated(added = "9.2.0"))]
#[qapi(removed = "11.0.0")]
pub struct QueryMigrationthreads {}

/// Return the name information of a guest.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-name" }
///     <- { "return": { "name": "qemu-name" } }
/// ```
#[qapi(command)]
#[qapi(name = "query-name")]
#[qapi(returns = "NameInfo")]
#[qapi(added = "1.0.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryName {}

/// Get the named block driver list
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-named-block-nodes" }
///     <- { "return": [ { "ro":false,
///                        "drv":"qcow2",
///                        "encrypted":false,
///                        "file":"disks/test.qcow2",
///                        "node-name": "my-node",
///                        "backing_file_depth":1,
///                        "detect_zeroes":"off",
///                        "bps":1000000,
///                        "bps_rd":0,
///                        "bps_wr":0,
///                        "iops":1000000,
///                        "iops_rd":0,
///                        "iops_wr":0,
///                        "bps_max": 8000000,
///                        "bps_rd_max": 0,
///                        "bps_wr_max": 0,
///                        "iops_max": 0,
///                        "iops_rd_max": 0,
///                        "iops_wr_max": 0,
///                        "iops_size": 0,
///                        "write_threshold": 0,
///                        "image":{
///                           "filename":"disks/test.qcow2",
///                           "format":"qcow2",
///                           "virtual-size":2048000,
///                           "backing_file":"base.qcow2",
///                           "full-backing-filename":"disks/base.qcow2",
///                           "backing-filename-format":"qcow2",
///                           "snapshots":[
///                              {
///                                 "id": "1",
///                                 "name": "snapshot1",
///                                 "vm-state-size": 0,
///                                 "date-sec": 10000200,
///                                 "date-nsec": 12,
///                                 "vm-clock-sec": 206,
///                                 "vm-clock-nsec": 30
///                              }
///                           ],
///                           "backing-image":{
///                               "filename":"disks/base.qcow2",
///                               "format":"qcow2",
///                               "virtual-size":2048000
///                           }
///                        } } ] }
/// ```
#[qapi(command)]
#[qapi(name = "query-named-block-nodes")]
#[qapi(returns = "Vec<BlockDeviceInfo>")]
#[qapi(added = "2.0.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct QueryNamedBlockNodes {
    /// Omit the nested data about backing image ("backing-image"
    /// key) if true.  Default is false (Since 5.0)
    #[qapi(added = "5.0.0")]
    pub flat: Option<bool>,
}

/// Return information about the PCI bus topology of the guest.
///
/// Returns: a list of info for each PCI bus.  Each bus is
/// ```text
///     represented by a json-object, which has a key with a json-array
///     of all PCI devices attached to it.  Each device is represented
///     by a json-object.
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-pci" }
///     <- { "return": [
///              {
///                 "bus": 0,
///                 "devices": [
///                    {
///                       "bus": 0,
///                       "qdev_id": "",
///                       "slot": 0,
///                       "class_info": {
///                          "class": 1536,
///                          "desc": "Host bridge"
///                       },
///                       "id": {
///                          "device": 32902,
///                          "vendor": 4663
///                       },
///                       "function": 0,
///                       "regions": [
///                       ]
///                    },
///                    {
///                       "bus": 0,
///                       "qdev_id": "",
///                       "slot": 1,
///                       "class_info": {
///                          "class": 1537,
///                          "desc": "ISA bridge"
///                       },
///                       "id": {
///                          "device": 32902,
///                          "vendor": 28672
///                       },
///                       "function": 0,
///                       "regions": [
///                       ]
///                    },
///                    {
///                       "bus": 0,
///                       "qdev_id": "",
///                       "slot": 1,
///                       "class_info": {
///                          "class": 257,
///                          "desc": "IDE controller"
///                       },
///                       "id": {
///                          "device": 32902,
///                          "vendor": 28688
///                       },
///                       "function": 1,
///                       "regions": [
///                          {
///                             "bar": 4,
///                             "size": 16,
///                             "address": 49152,
///                             "type": "io"
///                          }
///                       ]
///                    },
///                    {
///                       "bus": 0,
///                       "qdev_id": "",
///                       "slot": 2,
///                       "class_info": {
///                          "class": 768,
///                          "desc": "VGA controller"
///                       },
///                       "id": {
///                          "device": 4115,
///                          "vendor": 184
///                       },
///                       "function": 0,
///                       "regions": [
///                          {
///                             "prefetch": true,
///                             "mem_type_64": false,
///                             "bar": 0,
///                             "size": 33554432,
///                             "address": 4026531840,
///                             "type": "memory"
///                          },
///                          {
///                             "prefetch": false,
///                             "mem_type_64": false,
///                             "bar": 1,
///                             "size": 4096,
///                             "address": 4060086272,
///                             "type": "memory"
///                          },
///                          {
///                             "prefetch": false,
///                             "mem_type_64": false,
///                             "bar": 6,
///                             "size": 65536,
///                             "address": -1,
///                             "type": "memory"
///                          }
///                       ]
///                    },
///                    {
///                       "bus": 0,
///                       "qdev_id": "",
///                       "irq": 11,
///                       "slot": 4,
///                       "class_info": {
///                          "class": 1280,
///                          "desc": "RAM controller"
///                       },
///                       "id": {
///                          "device": 6900,
///                          "vendor": 4098
///                       },
///                       "function": 0,
///                       "regions": [
///                          {
///                             "bar": 0,
///                             "size": 32,
///                             "address": 49280,
///                             "type": "io"
///                          }
///                       ]
///                    }
///                 ]
///              }
///           ]
///        }
/// ```
///
/// This example has been shortened as the real response is too long.
#[qapi(command)]
#[qapi(name = "query-pci")]
#[qapi(returns = "Vec<PciInfo>")]
#[qapi(added = "1.0.0")]
pub struct QueryPci {}

/// Return a list of information about each persistent reservation
/// manager.
///
/// Returns: a list of manager info for each persistent reservation
/// ```text
///     manager
/// ```
#[qapi(command)]
#[qapi(name = "query-pr-managers")]
#[qapi(returns = "Vec<PrManagerInfo>")]
#[qapi(added = "3.0.0")]
#[qapi(allow_preconfig)]
pub struct QueryPrManagers {}

/// Command `query-qmp-schema` exposes the QMP wire ABI as an array of
/// `SchemaInfo`.  This lets QMP clients figure out what commands and
/// events are available in this QEMU, and their parameters and results.
///
/// However, the `SchemaInfo` can't reflect all the rules and
/// restrictions that apply to QMP.  It's interface introspection
/// (figuring out what's there), not interface specification.  The
/// specification is in the QAPI schema.
///
/// Furthermore, while we strive to keep the QMP wire format
/// backwards-compatible across QEMU versions, the introspection output
/// is not guaranteed to have the same stability.  For example, one
/// version of QEMU may list an object member as an optional
/// non-variant, while another lists the same member only through the
/// object's variants; or the type of a member may change from a generic
/// string into a specific enum or from one specific type into an
/// alternate that includes the original type alongside something else.
///
/// Returns: an array where each element describes an entity in the ABI:
/// ```text
///     command, event, type, ...
/// ```
///
/// ```text
///     The order of the various elements is unspecified; however, all
///     names are guaranteed to be unique (no name will be duplicated
///     with different meta-types).
/// ```
///
/// .. note:: The QAPI schema is also used to help define *internal*
/// ```text
///    interfaces, by defining QAPI types.  These are not part of the
///    QMP wire ABI, and therefore not returned by this command.
/// ```
#[qapi(command)]
#[qapi(name = "query-qmp-schema")]
#[qapi(returns = "Vec<SchemaInfo>")]
#[qapi(added = "2.5.0")]
#[qapi(allow_preconfig(added = "6.0.0"))]
pub struct QueryQmpSchema {
    /// Additional command arguments not modeled by this schema.
    #[qapi(flatten)]
    #[qapi(removed = "6.0.0")]
    pub arguments: Option<HashMap<String, serde_json::Value>>,
}

/// Retrieve the record/replay information.  It includes current
/// instruction count which may be used for `replay-break` and
/// `replay-seek` commands.
///
/// Returns: record/replay information.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-replay" }
///     <- { "return": { "mode": "play", "filename": "log.rr", "icount": 220414 } }
/// ```
#[qapi(command)]
#[qapi(name = "query-replay")]
#[qapi(returns = "ReplayInfo")]
#[qapi(added = "5.2.0")]
pub struct QueryReplay {}

/// Return rocker switch information.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-rocker", "arguments": { "name": "sw1" } }
///     <- { "return": {"name": "sw1", "ports": 2, "id": 1327446905938}}
/// ```
#[qapi(command)]
#[qapi(name = "query-rocker")]
#[qapi(returns = "RockerSwitch")]
#[qapi(added = "2.4.0")]
pub struct QueryRocker {
    /// switch name
    pub name: String,
}

/// Return rocker OF-DPA flow information.
///
/// Returns: rocker OF-DPA flow information
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-rocker-of-dpa-flows",
///          "arguments": { "name": "sw1" } }
///     <- { "return": [ {"key": {"in-pport": 0, "priority": 1, "tbl-id": 0},
///                       "hits": 138,
///                       "cookie": 0,
///                       "action": {"goto-tbl": 10},
///                       "mask": {"in-pport": 4294901760}
///                      },
///                      ...
///        ]}
/// ```
#[qapi(command)]
#[qapi(name = "query-rocker-of-dpa-flows")]
#[qapi(returns = "Vec<RockerOfDpaFlow>")]
#[qapi(added = "2.4.0")]
pub struct QueryRockerOfDpaFlows {
    /// switch name
    pub name: String,
    /// flow table ID.  If tbl-id is not specified, returns flow
    /// information for all tables.
    #[qapi(name = "tbl-id")]
    pub tbl_id: Option<u32>,
}

/// Return rocker OF-DPA group information.
///
/// Returns: rocker OF-DPA group information
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-rocker-of-dpa-groups",
///          "arguments": { "name": "sw1" } }
///     <- { "return": [ {"type": 0, "out-pport": 2,
///                       "pport": 2, "vlan-id": 3841,
///                       "pop-vlan": 1, "id": 251723778},
///                      {"type": 0, "out-pport": 0,
///                       "pport": 0, "vlan-id": 3841,
///                       "pop-vlan": 1, "id": 251723776},
///                      {"type": 0, "out-pport": 1,
///                       "pport": 1, "vlan-id": 3840,
///                       "pop-vlan": 1, "id": 251658241},
///                      {"type": 0, "out-pport": 0,
///                       "pport": 0, "vlan-id": 3840,
///                       "pop-vlan": 1, "id": 251658240}
///        ]}
/// ```
#[qapi(command)]
#[qapi(name = "query-rocker-of-dpa-groups")]
#[qapi(returns = "Vec<RockerOfDpaGroup>")]
#[qapi(added = "2.4.0")]
pub struct QueryRockerOfDpaGroups {
    /// switch name
    pub name: String,
    /// group type.  If type is not specified, returns group
    /// information for all group types.
    pub r#type: Option<u8>,
}

/// Return rocker switch port information.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-rocker-ports", "arguments": { "name": "sw1" } }
///     <- { "return": [ {"duplex": "full", "enabled": true, "name": "sw1.1",
///                       "autoneg": "off", "link-up": true, "speed": 10000},
///                      {"duplex": "full", "enabled": true, "name": "sw1.2",
///                       "autoneg": "off", "link-up": true, "speed": 10000}
///        ]}
/// ```
#[qapi(command)]
#[qapi(name = "query-rocker-ports")]
#[qapi(returns = "Vec<RockerPort>")]
#[qapi(added = "2.4.0")]
pub struct QueryRockerPorts {
    /// port name
    pub name: String,
}

/// Return rx-filter information for all NICs (or for the given NIC).
///
/// Returns: list of info for all NICs (or for the given NIC).
///
/// Errors:
/// ```text
///     - if the given @name doesn't exist
///     - if the given NIC doesn't support rx-filter querying
///     - if the given net client isn't a NIC
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-rx-filter", "arguments": { "name": "vnet0" } }
///     <- { "return": [
///             {
///                 "promiscuous": true,
///                 "name": "vnet0",
///                 "main-mac": "52:54:00:12:34:56",
///                 "unicast": "normal",
///                 "vlan": "normal",
///                 "vlan-table": [
///                     4,
///                     0
///                 ],
///                 "unicast-table": [
///                 ],
///                 "multicast": "normal",
///                 "multicast-overflow": false,
///                 "unicast-overflow": false,
///                 "multicast-table": [
///                     "01:00:5e:00:00:01",
///                     "33:33:00:00:00:01",
///                     "33:33:ff:12:34:56"
///                 ],
///                 "broadcast-allowed": false
///             }
///           ]
///        }
/// ```
#[qapi(command)]
#[qapi(name = "query-rx-filter")]
#[qapi(returns = "Vec<RxFilterInfo>")]
#[qapi(added = "1.6.0")]
pub struct QueryRxFilter {
    /// net client name
    pub name: Option<String>,
}

/// Features:
///
/// @unstable: This command is experimental.
///
/// Returns: the machine's CPU polarization
#[qapi(command)]
#[qapi(name = "query-s390x-cpu-polarization")]
#[qapi(condition = "(TARGET_S390X && CONFIG_KVM)")]
#[qapi(feature = "unstable")]
#[qapi(returns = "CpuPolarizationInfo")]
#[qapi(added = "8.2.0")]
#[qapi(unstable(added = "8.2.0"))]
pub struct QueryS390xCpuPolarization {}

/// Return information about SEV/SEV-ES/SEV-SNP.
///
/// If unavailable due to an incompatible configuration the returned
/// @enabled field is set to 'false' and the state of all other fields
/// is unspecified.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-sev" }
///     <- { "return": { "enabled": true, "api-major" : 0, "api-minor" : 0,
///                      "build-id" : 0, "policy" : 0, "state" : "running",
///                      "handle" : 1 } }
/// ```
#[qapi(command)]
#[qapi(name = "query-sev")]
#[qapi(returns = "SevInfo")]
#[qapi(condition = "defined(TARGET_I386)")]
#[qapi(added = "2.12.0")]
pub struct QuerySev {}

/// Get the SEV attestation report.
///
/// This is only valid on x86 machines configured with KVM and the
/// 'sev-guest' confidential virtualization object.  The attestation
/// report for SEV-SNP guests is only available within the guest.
///
/// Errors:
/// ```text
///     - If the attestation report is unavailable, either due to an
///       invalid guest configuration or because the guest has not
///       reached the required SEV state, GenericError
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute" : "query-sev-attestation-report",
///                      "arguments": { "mnonce": "aaaaaaa" } }
///     <- { "return" : { "data": "aaaaaaaabbbddddd"} }
/// ```
#[qapi(command)]
#[qapi(name = "query-sev-attestation-report")]
#[qapi(condition = "defined(TARGET_I386)")]
#[qapi(returns = "SevAttestationReport")]
#[qapi(added = "6.1.0")]
pub struct QuerySevAttestationReport {
    /// a random 16 bytes value encoded in base64 (it will be
    /// included in report)
    pub mnonce: String,
}

/// Get SEV capabilities.
///
/// This is only supported on AMD X86 platforms with KVM enabled.
///
/// Errors:
/// ```text
///     - If SEV is not available on the platform, GenericError
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-sev-capabilities" }
///     <- { "return": { "pdh": "8CCDD8DDD", "cert-chain": "888CCCDDDEE",
///                      "cpu0-id": "2lvmGwo+...61iEinw==",
///                      "cbitpos": 47, "reduced-phys-bits": 1}}
/// ```
#[qapi(command)]
#[qapi(name = "query-sev-capabilities")]
#[qapi(returns = "SevCapability")]
#[qapi(condition = "defined(TARGET_I386)")]
#[qapi(added = "2.12.0")]
pub struct QuerySevCapabilities {}

/// Query the SEV/SEV-ES guest launch information.
///
/// This is only valid on x86 machines configured with KVM and the
/// 'sev-guest' confidential virtualization object.  The launch
/// measurement for SEV-SNP guests is only available within the guest.
///
/// Returns: The guest's SEV guest launch measurement info
///
/// Errors:
/// ```text
///     - If the launch measurement is unavailable, either due to an
///       invalid guest configuration or if the guest has not reached
///       the required SEV state, GenericError
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-sev-launch-measure" }
///     <- { "return": { "data": "4l8LXeNlSPUDlXPJG5966/8%YZ" } }
/// ```
#[qapi(command)]
#[qapi(name = "query-sev-launch-measure")]
#[qapi(returns = "SevLaunchMeasureInfo")]
#[qapi(condition = "defined(TARGET_I386)")]
#[qapi(added = "2.12.0")]
pub struct QuerySevLaunchMeasure {}

/// Return information about configured SGX capabilities of guest
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-sgx" }
///     <- { "return": { "sgx": true, "sgx1" : true, "sgx2" : true,
///                      "flc": true,
///                      "sections": [{"node": 0, "size": 67108864},
///                      {"node": 1, "size": 29360128}]} }
/// ```
#[qapi(command)]
#[qapi(name = "query-sgx")]
#[qapi(condition = "TARGET_I386")]
#[qapi(returns = "SgxInfo")]
#[qapi(added = "6.2.0")]
pub struct QuerySgx {}

/// Return information about SGX capabilities of host
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-sgx-capabilities" }
///     <- { "return": { "sgx": true, "sgx1" : true, "sgx2" : true,
///                      "flc": true,
///                      "section" : [{"node": 0, "size": 67108864},
///                      {"node": 1, "size": 29360128}]} }
/// ```
#[qapi(command)]
#[qapi(name = "query-sgx-capabilities")]
#[qapi(condition = "TARGET_I386")]
#[qapi(returns = "SgxInfo")]
#[qapi(added = "6.2.0")]
pub struct QuerySgxCapabilities {}

/// Return information about the current SPICE server
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-spice" }
///     <- { "return": {
///              "enabled": true,
///              "auth": "spice",
///              "port": 5920,
///              "migrated":false,
///              "tls-port": 5921,
///              "host": "0.0.0.0",
///              "mouse-mode":"client",
///              "channels": [
///                 {
///                    "port": "54924",
///                    "family": "ipv4",
///                    "channel-type": 1,
///                    "connection-id": 1804289383,
///                    "host": "127.0.0.1",
///                    "channel-id": 0,
///                    "tls": true
///                 },
///                 {
///                    "port": "36710",
///                    "family": "ipv4",
///                    "channel-type": 4,
///                    "connection-id": 1804289383,
///                    "host": "127.0.0.1",
///                    "channel-id": 0,
///                    "tls": false
///                 },
///                 ...
///              ]
///           }
///        }
/// ```
#[qapi(command)]
#[qapi(name = "query-spice")]
#[qapi(returns = "SpiceInfo")]
#[qapi(condition = "defined(CONFIG_SPICE)")]
#[qapi(added = "1.0.0")]
pub struct QuerySpice {}

/// Return runtime-collected statistics for objects such as the VM or
/// its vCPUs.
///
/// The arguments are a `StatsFilter` and specify the provider and
/// objects to return statistics about.
///
/// Returns: a list of statistics, one for each provider and object
/// ```text
///     (e.g., for each vCPU).
/// ```
#[qapi(command)]
#[qapi(name = "query-stats")]
#[qapi(returns = "Vec<StatsResult>")]
#[qapi(added = "7.1.0")]
pub struct QueryStats {
    /// which providers to request statistics from, and
    /// optionally which named values to return within each provider
    pub providers: Option<Vec<StatsRequest>>,
    /// the kind of objects to query.  Note that each possible
    /// target may enable additional filtering options
    #[qapi(discriminator)]
    pub target: StatsTarget,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<StatsFilterBranch>,
}

/// Return the schema for all available runtime-collected statistics.
///
/// .. note:: Runtime-collected statistics and their names fall outside
/// ```text
///    QEMU's usual deprecation policies.  QEMU will try to keep the set
///    of available data stable, together with their names, but will not
///    guarantee stability at all costs; the same is true of providers
///    that source statistics externally, e.g. from Linux.  For example,
///    if the same value is being tracked with different names on
///    different architectures or by different providers, one of them
///    might be renamed.  A statistic might go away if an algorithm is
///    changed or some code is removed; changing a default might cause
///    previously useful statistics to always report 0.  Such changes,
///    however, are expected to be rare.
/// ```
#[qapi(command)]
#[qapi(name = "query-stats-schemas")]
#[qapi(returns = "Vec<StatsSchema>")]
#[qapi(added = "7.1.0")]
pub struct QueryStatsSchemas {
    /// a provider to restrict the query to.
    pub provider: Option<StatsProvider>,
}

/// Query the run status of the VM
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-status" }
///     <- { "return": { "running": true,
///                      "status": "running" } }
/// ```
#[qapi(command)]
#[qapi(name = "query-status")]
#[qapi(returns = "StatusInfo")]
#[qapi(added = "1.0.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryStatus {}

/// Return information about the target for this QEMU
#[qapi(command)]
#[qapi(name = "query-target")]
#[qapi(returns = "QemuTargetInfo")]
#[qapi(added = "1.2.0")]
#[qapi(returns(added = "1.2.0", value = "TargetInfo"))]
#[qapi(returns(added = "10.1.0", value = "QemuTargetInfo"))]
pub struct QueryTarget {}

/// Return information about the TPM device
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-tpm" }
///     <- { "return":
///          [
///            { "model": "tpm-tis",
///              "options":
///                { "type": "passthrough",
///                  "data":
///                    { "cancel-path": "/sys/class/misc/tpm0/device/cancel",
///                      "path": "/dev/tpm0"
///                    }
///                },
///              "id": "tpm0"
///            }
///          ]
///        }
/// ```
#[qapi(command)]
#[qapi(name = "query-tpm")]
#[qapi(returns = "Vec<TpmInfo>")]
#[qapi(condition = "defined(CONFIG_TPM)")]
#[qapi(added = "1.5.0")]
pub struct QueryTpm {}

/// Return a list of supported TPM models
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-tpm-models" }
///     <- { "return": [ "tpm-tis", "tpm-crb", "tpm-spapr" ] }
/// ```
#[qapi(command)]
#[qapi(name = "query-tpm-models")]
#[qapi(returns = "Vec<TpmModel>")]
#[qapi(condition = "defined(CONFIG_TPM)")]
#[qapi(added = "1.5.0")]
pub struct QueryTpmModels {}

/// Return a list of supported TPM types
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-tpm-types" }
///     <- { "return": [ "passthrough", "emulator" ] }
/// ```
#[qapi(command)]
#[qapi(name = "query-tpm-types")]
#[qapi(returns = "Vec<TpmType>")]
#[qapi(condition = "defined(CONFIG_TPM)")]
#[qapi(added = "1.5.0")]
pub struct QueryTpmTypes {}

/// Query the guest UUID information.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-uuid" }
///     <- { "return": { "UUID": "550e8400-e29b-41d4-a716-446655440000" } }
/// ```
#[qapi(command)]
#[qapi(name = "query-uuid")]
#[qapi(returns = "UuidInfo")]
#[qapi(added = "1.0.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryUuid {}

/// Return information about virtual CPU dirty page rate limits, if any.
///
///
/// .. qmp-example::
///
/// ```text
///     -> {"execute": "query-vcpu-dirty-limit"}
///     <- {"return": [
///            { "limit-rate": 60, "current-rate": 3, "cpu-index": 0},
///            { "limit-rate": 60, "current-rate": 3, "cpu-index": 1}]}
/// ```
#[qapi(command)]
#[qapi(name = "query-vcpu-dirty-limit")]
#[qapi(returns = "Vec<DirtyLimitInfo>")]
#[qapi(added = "7.1.0")]
pub struct QueryVcpuDirtyLimit {}

/// Return the current version of QEMU.
///
/// Returns: An object describing the current version of QEMU.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-version" }
///     <- {
///           "return":{
///              "qemu":{
///                 "major":0,
///                 "minor":11,
///                 "micro":5
///              },
///              "package":""
///           }
///        }
/// ```
#[qapi(command)]
#[qapi(name = "query-version")]
#[qapi(returns = "VersionInfo")]
#[qapi(added = "1.0.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryVersion {}

/// Show Virtual Machine Generation ID
#[qapi(command)]
#[qapi(name = "query-vm-generation-id")]
#[qapi(returns = "GuidInfo")]
#[qapi(added = "2.9.0")]
pub struct QueryVmGenerationId {}

/// Return information about the current VNC server
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-vnc" }
///     <- { "return": {
///              "enabled":true,
///              "host":"0.0.0.0",
///              "service":"50402",
///              "auth":"vnc",
///              "family":"ipv4",
///              "clients":[
///                 {
///                    "host":"127.0.0.1",
///                    "service":"50401",
///                    "family":"ipv4",
///                    "websocket":false
///                 }
///              ]
///           }
///        }
/// ```
#[qapi(command)]
#[qapi(name = "query-vnc")]
#[qapi(returns = "VncInfo")]
#[qapi(condition = "defined(CONFIG_VNC)")]
#[qapi(added = "1.0.0")]
pub struct QueryVnc {}

/// Return a list of vnc servers.  The list can be empty.
#[qapi(command)]
#[qapi(name = "query-vnc-servers")]
#[qapi(returns = "Vec<VncInfo2>")]
#[qapi(condition = "defined(CONFIG_VNC)")]
#[qapi(added = "2.3.0")]
pub struct QueryVncServers {}

/// Query replication status while the vm is running.
///
/// TODO: This line is a hack to separate the example from the body
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-xen-replication-status" }
///     <- { "return": { "error": false } }
/// ```
#[qapi(command)]
#[qapi(name = "query-xen-replication-status")]
#[qapi(returns = "ReplicationStatus")]
#[qapi(condition = "defined(CONFIG_REPLICATION)")]
#[qapi(added = "2.9.0")]
pub struct QueryXenReplicationStatus {}

/// Query yank instances.  See `YankInstance` for more information.
///
/// TODO: This line is a hack to separate the example from the body
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-yank" }
///     <- { "return": [
///              { "type": "block-node",
///                "node-name": "nbd0" }
///          ] }
/// ```
#[qapi(command)]
#[qapi(name = "query-yank")]
#[qapi(returns = "Vec<YankInstance>")]
#[qapi(added = "6.0.0")]
#[qapi(allow_oob)]
pub struct QueryYank {}

/// Request graceful QEMU process termination.
///
/// While every attempt is made to send the QMP response before
/// terminating, this is not guaranteed.  When using this interface, a
/// premature EOF would not be unexpected.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "quit" }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "quit")]
#[qapi(returns = "()")]
#[qapi(added = "1.0.0")]
#[qapi(allow_preconfig(added = "6.0.0"))]
pub struct Quit {}

/// Remove a file descriptor from an fd set.
///
/// Errors:
/// ```text
///     - If @fdset-id or @fd is not found, GenericError
/// ```
///
///
/// .. note:: The list of fd sets is shared by all monitor connections.
///
/// .. note:: If @fd is not specified, all file descriptors in @fdset-id
/// ```text
///    will be removed.
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "remove-fd", "arguments": { "fdset-id": 1, "fd": 3 } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "remove-fd")]
#[qapi(returns = "()")]
#[qapi(added = "1.2.0")]
pub struct RemoveFd {
    /// The file descriptor that is to be removed.
    pub fd: Option<i64>,
    /// The ID of the fd set that the file descriptor belongs to.
    #[qapi(name = "fdset-id")]
    pub fdset_id: i64,
}

/// Set replay breakpoint at instruction count @icount.  Execution stops
/// when the specified instruction is reached.  There can be at most one
/// breakpoint.  When breakpoint is set, any prior one is removed.  The
/// breakpoint may be set only in replay mode and only "in the future",
/// i.e. at instruction counts greater than the current one.  The
/// current instruction count can be observed with `query-replay`.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "replay-break", "arguments": { "icount": 220414 } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "replay-break")]
#[qapi(returns = "()")]
#[qapi(added = "5.2.0")]
pub struct ReplayBreak {
    /// instruction count to stop at
    pub icount: i64,
}

/// Remove replay breakpoint which was set with `replay-break`.  The
/// command is ignored when there are no replay breakpoints.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "replay-delete-break" }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "replay-delete-break")]
#[qapi(returns = "()")]
#[qapi(added = "5.2.0")]
pub struct ReplayDeleteBreak {}

/// Automatically proceed to the instruction count @icount, when
/// replaying the execution.  The command automatically loads nearest
/// snapshot and replays the execution to find the desired instruction.
/// When there is no preceding snapshot or the execution is not
/// replayed, then the command fails.  Instruction count can be obtained
/// with the `query-replay` command.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "replay-seek", "arguments": { "icount": 220414 } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "replay-seek")]
#[qapi(returns = "()")]
#[qapi(added = "5.2.0")]
pub struct ReplaySeek {
    /// target instruction count
    pub icount: i64,
}

/// Retrieve an eBPF object that can be loaded with libbpf.  Management
/// applications (e.g. libvirt) may load it and pass file descriptors to
/// QEMU, so they can run running QEMU without BPF capabilities.
///
/// Returns: eBPF object encoded in base64.
#[qapi(command)]
#[qapi(name = "request-ebpf")]
#[qapi(condition = "CONFIG_EBPF")]
#[qapi(returns = "EbpfObject")]
#[qapi(added = "9.0.0")]
pub struct RequestEbpf {
    /// The ID of the program to return.
    pub id: EbpfProgramId,
}

/// Read from a ring buffer character device.
///
/// Returns: data read from the device
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "ringbuf-read",
///          "arguments": { "device": "foo",
///                         "size": 1000,
///                         "format": "utf8" } }
///     <- { "return": "abcdefgh" }
/// ```
#[qapi(command)]
#[qapi(name = "ringbuf-read")]
#[qapi(returns = "String")]
#[qapi(added = "1.4.0")]
pub struct RingbufRead {
    /// the ring buffer character device name
    pub device: String,
    /// data encoding (default 'utf8').
    ///
    /// - base64: the data read is returned in base64 encoding.
    /// - utf8: the data read is interpreted as UTF-8.
    ///   Bug: can screw up when the buffer contains invalid UTF-8
    ///   sequences, NUL characters, after the ring buffer lost data,
    ///   and when reading stops because the size limit is reached.
    /// - The return value is always Unicode regardless of format, like
    ///   any other string.
    pub format: Option<DataFormat>,
    /// how many bytes to read at most
    pub size: i64,
}

/// Write to a ring buffer character device.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "ringbuf-write",
///          "arguments": { "device": "foo",
///                         "data": "abcdefgh",
///                         "format": "utf8" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "ringbuf-write")]
#[qapi(returns = "()")]
#[qapi(added = "1.4.0")]
pub struct RingbufWrite {
    /// data to write
    pub data: String,
    /// the ring buffer character device name
    pub device: String,
    /// data encoding (default 'utf8').
    ///
    /// - base64: data must be base64 encoded text.  Its binary decoding
    ///   gets written.
    /// - utf8: data's UTF-8 encoding is written
    /// - data itself is always Unicode regardless of format, like any
    ///   other string.
    pub format: Option<DataFormat>,
}

/// Reset the RTC interrupt reinjection backlog.  Can be used if another
/// mechanism to synchronize guest time is in effect, for example QEMU
/// guest agent's `guest-set-time` command.
///
/// Use of this command is only applicable for x86 machines with an RTC,
/// and on other machines will silently return without performing any
/// action.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "rtc-reset-reinjection" }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "rtc-reset-reinjection")]
#[qapi(returns = "()")]
#[qapi(condition = "defined(TARGET_I386)")]
#[qapi(added = "2.1.0")]
pub struct RtcResetReinjection {}

/// Capture the contents of a screen and write it to a file.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "screendump",
///          "arguments": { "filename": "/tmp/image" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "screendump")]
#[qapi(returns = "()")]
#[qapi(condition = "CONFIG_PIXMAN")]
#[qapi(added = "1.3.0")]
pub struct Screendump {
    /// ID of the display device that should be dumped.  If this
    /// parameter is missing, the primary display will be used.
    /// (Since 2.12)
    #[qapi(added = "2.12.0")]
    pub device: Option<String>,
    /// the path of a new file to store the image
    pub filename: String,
    /// image format for `screendump`.  (default: ppm) (Since 7.1)
    #[qapi(added = "7.1.0")]
    pub format: Option<ImageFormat>,
    /// head to use in case the device supports multiple heads.  If
    /// this parameter is missing, head #0 will be used.  Also note that
    /// the head can only be specified in conjunction with the device
    /// ID.  (Since 2.12)
    #[qapi(added = "2.12.0")]
    pub head: Option<i64>,
}

/// Send keys to guest.
///
/// Errors:
/// ```text
///     - If key is unknown or redundant, GenericError
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "send-key",
///          "arguments": { "keys": [ { "type": "qcode", "data": "ctrl" },
///                                   { "type": "qcode", "data": "alt" },
///                                   { "type": "qcode", "data": "delete" } ] } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "send-key")]
#[qapi(returns = "()")]
#[qapi(added = "1.3.0")]
pub struct SendKey {
    /// time to delay key up events, milliseconds.  Defaults to
    /// 100
    #[qapi(name = "hold-time")]
    pub hold_time: Option<i64>,
    /// An array of `KeyValue` elements.  All @KeyValues in this
    /// array are simultaneously sent to the guest.  A `KeyValue`.number
    /// value is sent directly to the guest, while `KeyValue`.qcode must
    /// be a valid `QKeyCode` value
    pub keys: Vec<KeyValue>,
}

/// Set the actions that will be taken by the emulator in response to
/// guest events.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "set-action",
///          "arguments": { "reboot": "shutdown",
///                         "shutdown" : "pause",
///                         "panic": "pause",
///                         "watchdog": "inject-nmi" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "set-action")]
#[qapi(returns = "()")]
#[qapi(added = "6.0.0")]
#[qapi(allow_preconfig)]
pub struct SetAction {
    /// `PanicAction` action taken on guest panic.
    pub panic: Option<PanicAction>,
    /// `RebootAction` action taken on guest reboot.
    pub reboot: Option<RebootAction>,
    /// `ShutdownAction` action taken on guest shutdown.
    pub shutdown: Option<ShutdownAction>,
    /// `WatchdogAction` action taken when watchdog timer
    /// expires.
    pub watchdog: Option<WatchdogAction>,
}

/// Modify the topology by moving the CPU inside the topology tree, or
/// by changing a modifier attribute of a CPU.  Absent values will not
/// be modified.
///
/// Features:
///
/// @unstable: This command is experimental.
#[qapi(command)]
#[qapi(name = "set-cpu-topology")]
#[qapi(condition = "(TARGET_S390X && CONFIG_KVM)")]
#[qapi(feature = "unstable")]
#[qapi(returns = "()")]
#[qapi(added = "8.2.0")]
#[qapi(unstable(added = "8.2.0"))]
pub struct SetCpuTopology {
    /// destination book to move the vCPU to
    #[qapi(name = "book-id")]
    pub book_id: Option<u16>,
    /// the vCPU ID to be moved
    #[qapi(name = "core-id")]
    pub core_id: u16,
    /// whether the provisioning of real to virtual CPU is
    /// dedicated
    pub dedicated: Option<bool>,
    /// destination drawer to move the vCPU to
    #[qapi(name = "drawer-id")]
    pub drawer_id: Option<u16>,
    /// entitlement to set
    #[qapi(added = "9.2.0")]
    pub entitlement: Option<S390CpuEntitlement>,
    /// entitlement to set
    #[qapi(name = "entitlement")]
    #[qapi(removed = "9.2.0")]
    pub entitlement_legacy: Option<CpuS390Entitlement>,
    /// destination socket to move the vCPU to
    #[qapi(name = "socket-id")]
    pub socket_id: Option<u16>,
}

/// Sets the link status of a virtual network adapter.
///
/// Errors:
/// ```text
///     - If @name is not a valid network device, DeviceNotFound
/// ```
///
///
/// .. note:: Not all network adapters support setting link status.
/// ```text
///    This command will succeed even if the network adapter does not
///    support link status notification.
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "set_link",
///          "arguments": { "name": "e1000.0", "up": false } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "set_link")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct SetLink {
    /// the device name of the virtual network adapter
    pub name: String,
    /// true to set the link status to be up
    pub up: bool,
}

/// Runtime equivalent of '-numa' CLI option, available at preconfigure
/// stage to configure numa mapping before initializing machine.
#[qapi(command)]
#[qapi(name = "set-numa-node")]
#[qapi(returns = "()")]
#[qapi(added = "3.0.0")]
#[qapi(allow_preconfig)]
pub struct SetNumaNode {
    /// NUMA option type
    #[qapi(discriminator)]
    pub r#type: NumaOptionsType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<NumaOptionsBranch>,
}

/// Set the password of a remote display server.
///
/// Errors:
/// ```text
///     - If Spice is not enabled, DeviceNotFound
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "set_password", "arguments": { "protocol": "vnc",
///                                                    "password": "secret" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "set_password")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct SetPassword {
    /// How to handle existing clients when changing the
    /// password.  If nothing is specified, defaults to 'keep'.  For
    /// VNC, only 'keep' is currently implemented.
    #[qapi(added = "7.0.0")]
    pub connected: Option<SetPasswordAction>,
    /// how to handle existing clients when changing the
    /// password.  If nothing is specified, defaults to 'keep'
    /// 'fail' to fail the command if clients are connected
    /// 'disconnect' to disconnect existing clients
    /// 'keep' to maintain existing clients
    #[qapi(name = "connected")]
    #[qapi(removed = "7.0.0")]
    pub connected_legacy: Option<String>,
    /// the new password
    pub password: String,
    /// - 'vnc' to modify the VNC server password
    /// - 'spice' to modify the Spice server password
    #[qapi(discriminator)]
    #[qapi(added = "7.0.0")]
    #[qapi(required)]
    pub protocol: Option<DisplayProtocol>,
    /// - 'vnc' to modify the VNC server password
    /// - 'spice' to modify the Spice server password
    #[qapi(name = "protocol")]
    #[qapi(removed = "7.0.0")]
    #[qapi(required)]
    pub protocol_legacy: Option<String>,
    #[qapi(union)]
    #[qapi(flatten)]
    #[qapi(added = "7.0.0")]
    pub u: Option<SetPasswordOptionsBranch>,
}

/// Set the upper limit of dirty page rate for virtual CPUs.
///
/// Requires KVM with accelerator property "dirty-ring-size" set.  A
/// virtual CPU's dirty page rate is a measure of its memory load.  To
/// observe dirty page rates, use `calc-dirty-rate`.
///
///
/// .. qmp-example::
///
/// ```text
///     -> {"execute": "set-vcpu-dirty-limit"}
///         "arguments": { "dirty-rate": 200,
///                        "cpu-index": 1 } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "set-vcpu-dirty-limit")]
#[qapi(returns = "()")]
#[qapi(added = "7.1.0")]
pub struct SetVcpuDirtyLimit {
    /// index of a virtual CPU, default is all.
    #[qapi(name = "cpu-index")]
    pub cpu_index: Option<i64>,
    /// upper limit of dirty page rate (MB/s) for virtual CPUs.
    #[qapi(name = "dirty-rate")]
    pub dirty_rate: u64,
}

/// Inject a secret blob into a SEV/SEV-ES guest's memory.
///
/// This is only valid on x86 machines configured with KVM and the
/// 'sev-guest' confidential virtualization object.  SEV-SNP guests do
/// not support launch secret injection.
///
/// Errors:
/// ```text
///     - If launch secret injection is not possible, either due to
///       an invalid guest configuration, or if the guest has not
///       reached the required SEV state, GenericError
/// ```
#[qapi(command)]
#[qapi(name = "sev-inject-launch-secret")]
#[qapi(condition = "defined(TARGET_I386)")]
#[qapi(returns = "()")]
#[qapi(added = "6.0.0")]
pub struct SevInjectLaunchSecret {
    /// the guest physical address where secret will be injected.
    pub gpa: Option<u64>,
    /// the launch secret packet header encoded in base64
    #[qapi(name = "packet-header")]
    pub packet_header: String,
    /// the launch secret data to be injected encoded in base64
    pub secret: String,
}

/// Delete a VM snapshot
///
/// Applications should not assume that the snapshot delete is complete
/// when this command returns.  The job commands / events must be used
/// to determine completion and to fetch details of any errors that
/// arise.
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "snapshot-delete",
///          "arguments": {
///             "job-id": "snapdelete0",
///             "tag": "my-snap",
///             "devices": ["disk0", "disk1"]
///          }
///        }
///     <- { "return": { } }
///     <- {"event": "JOB_STATUS_CHANGE",
///         "timestamp": {"seconds": 1442124172, "microseconds": 744001},
///         "data": {"status": "created", "id": "snapdelete0"}}
///     <- {"event": "JOB_STATUS_CHANGE",
///         "timestamp": {"seconds": 1442125172, "microseconds": 744001},
///         "data": {"status": "running", "id": "snapdelete0"}}
///     <- {"event": "JOB_STATUS_CHANGE",
///         "timestamp": {"seconds": 1442126172, "microseconds": 744001},
///         "data": {"status": "waiting", "id": "snapdelete0"}}
///     <- {"event": "JOB_STATUS_CHANGE",
///         "timestamp": {"seconds": 1442127172, "microseconds": 744001},
///         "data": {"status": "pending", "id": "snapdelete0"}}
///     <- {"event": "JOB_STATUS_CHANGE",
///         "timestamp": {"seconds": 1442128172, "microseconds": 744001},
///         "data": {"status": "concluded", "id": "snapdelete0"}}
///     -> {"execute": "query-jobs"}
///     <- {"return": [{"current-progress": 1,
///                     "status": "concluded",
///                     "total-progress": 1,
///                     "type": "snapshot-delete",
///                     "id": "snapdelete0"}]}
/// ```
#[qapi(command)]
#[qapi(name = "snapshot-delete")]
#[qapi(returns = "()")]
#[qapi(added = "6.0.0")]
pub struct SnapshotDelete {
    /// list of block device node names to delete a snapshot from
    pub devices: Vec<String>,
    /// identifier for the newly created job
    #[qapi(name = "job-id")]
    pub job_id: String,
    /// name of the snapshot to delete.
    pub tag: String,
}

/// Load a VM snapshot
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
/// device nodes that can have changed since the original
/// `snapshot-save` command execution.
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "snapshot-load",
///          "arguments": {
///             "job-id": "snapload0",
///             "tag": "my-snap",
///             "vmstate": "disk0",
///             "devices": ["disk0", "disk1"]
///          }
///        }
///     <- { "return": { } }
///     <- {"event": "JOB_STATUS_CHANGE",
///         "timestamp": {"seconds": 1472124172, "microseconds": 744001},
///         "data": {"status": "created", "id": "snapload0"}}
///     <- {"event": "JOB_STATUS_CHANGE",
///         "timestamp": {"seconds": 1472125172, "microseconds": 744001},
///         "data": {"status": "running", "id": "snapload0"}}
///     <- {"event": "STOP",
///         "timestamp": {"seconds": 1472125472, "microseconds": 744001} }
///     <- {"event": "RESUME",
///         "timestamp": {"seconds": 1472125872, "microseconds": 744001} }
///     <- {"event": "JOB_STATUS_CHANGE",
///         "timestamp": {"seconds": 1472126172, "microseconds": 744001},
///         "data": {"status": "waiting", "id": "snapload0"}}
///     <- {"event": "JOB_STATUS_CHANGE",
///         "timestamp": {"seconds": 1472127172, "microseconds": 744001},
///         "data": {"status": "pending", "id": "snapload0"}}
///     <- {"event": "JOB_STATUS_CHANGE",
///         "timestamp": {"seconds": 1472128172, "microseconds": 744001},
///         "data": {"status": "concluded", "id": "snapload0"}}
///     -> {"execute": "query-jobs"}
///     <- {"return": [{"current-progress": 1,
///                     "status": "concluded",
///                     "total-progress": 1,
///                     "type": "snapshot-load",
///                     "id": "snapload0"}]}
/// ```
#[qapi(command)]
#[qapi(name = "snapshot-load")]
#[qapi(returns = "()")]
#[qapi(added = "6.0.0")]
pub struct SnapshotLoad {
    /// list of block device node names to load a snapshot from
    pub devices: Vec<String>,
    /// identifier for the newly created job
    #[qapi(name = "job-id")]
    pub job_id: String,
    /// name of the snapshot to load.
    pub tag: String,
    /// block device node name to load vmstate from
    pub vmstate: String,
}

/// Save a VM snapshot
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
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "snapshot-save",
///          "arguments": {
///             "job-id": "snapsave0",
///             "tag": "my-snap",
///             "vmstate": "disk0",
///             "devices": ["disk0", "disk1"]
///          }
///        }
///     <- { "return": { } }
///     <- {"event": "JOB_STATUS_CHANGE",
///         "timestamp": {"seconds": 1432121972, "microseconds": 744001},
///         "data": {"status": "created", "id": "snapsave0"}}
///     <- {"event": "JOB_STATUS_CHANGE",
///         "timestamp": {"seconds": 1432122172, "microseconds": 744001},
///         "data": {"status": "running", "id": "snapsave0"}}
///     <- {"event": "STOP",
///         "timestamp": {"seconds": 1432122372, "microseconds": 744001} }
///     <- {"event": "RESUME",
///         "timestamp": {"seconds": 1432122572, "microseconds": 744001} }
///     <- {"event": "JOB_STATUS_CHANGE",
///         "timestamp": {"seconds": 1432122772, "microseconds": 744001},
///         "data": {"status": "waiting", "id": "snapsave0"}}
///     <- {"event": "JOB_STATUS_CHANGE",
///         "timestamp": {"seconds": 1432122972, "microseconds": 744001},
///         "data": {"status": "pending", "id": "snapsave0"}}
///     <- {"event": "JOB_STATUS_CHANGE",
///         "timestamp": {"seconds": 1432123172, "microseconds": 744001},
///         "data": {"status": "concluded", "id": "snapsave0"}}
///     -> {"execute": "query-jobs"}
///     <- {"return": [{"current-progress": 1,
///                     "status": "concluded",
///                     "total-progress": 1,
///                     "type": "snapshot-save",
///                     "id": "snapsave0"}]}
/// ```
#[qapi(command)]
#[qapi(name = "snapshot-save")]
#[qapi(returns = "()")]
#[qapi(added = "6.0.0")]
pub struct SnapshotSave {
    /// list of block device node names to save a snapshot to
    pub devices: Vec<String>,
    /// identifier for the newly created job
    #[qapi(name = "job-id")]
    pub job_id: String,
    /// name of the snapshot to create
    pub tag: String,
    /// block device node name to save vmstate to
    pub vmstate: String,
}

/// Stop guest VM execution.
///
///
/// .. note:: This function will succeed even if the guest is already in
/// ```text
///    the stopped state.  In "inmigrate" state, it will ensure that the
///    guest remains paused once migration finishes, as if the ``-S``
///    option was passed on the command line.
/// ```
///
/// ```text
///    In the "suspended" state, it will completely stop the VM and
///    cause a transition to the "paused" state.  (Since 9.0)
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "stop" }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "stop")]
#[qapi(returns = "()")]
#[qapi(added = "1.0.0")]
pub struct Stop {}

/// Requests that a guest perform a powerdown operation.
///
///
/// .. note:: A guest may or may not respond to this command.  This
/// ```text
///    command returning does not indicate that a guest has accepted the
///    request or that it has shut down.  Many guests will respond to
///    this command by prompting the user in some way.
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "system_powerdown" }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "system_powerdown")]
#[qapi(returns = "()")]
#[qapi(added = "1.0.0")]
pub struct SystemPowerdown {}

/// Performs a hard reset of a guest.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "system_reset" }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "system_reset")]
#[qapi(returns = "()")]
#[qapi(added = "1.0.0")]
pub struct SystemReset {}

/// Wake up guest from suspend.  If the guest has wake-up from suspend
/// support enabled (wakeup-suspend-support flag from
/// `query-current-machine`), wake-up guest from suspend if the guest is
/// in SUSPENDED state.  Return an error otherwise.
///
///
/// .. note:: Prior to 4.0, this command does nothing in case the guest
/// ```text
///    isn't suspended.
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "system_wakeup" }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "system_wakeup")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct SystemWakeup {}

/// Query the state of events.
///
/// Returns: a list of info for the matching events
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "trace-event-get-state",
///          "arguments": { "name": "qemu_memalign" } }
///     <- { "return": [ { "name": "qemu_memalign", "state": "disabled", "vcpu": false } ] }
/// ```
#[qapi(command)]
#[qapi(name = "trace-event-get-state")]
#[qapi(returns = "Vec<TraceEventInfo>")]
#[qapi(added = "2.2.0")]
pub struct TraceEventGetState {
    /// Event name pattern (case-sensitive glob).
    pub name: String,
    /// The vCPU to query (since 2.7).
    #[qapi(feature = "deprecated")]
    #[qapi(added = "2.7.0")]
    #[qapi(deprecated(added = "8.1.0"))]
    #[qapi(removed = "9.1.0")]
    pub vcpu: Option<i64>,
}

/// Set the dynamic tracing state of events.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "trace-event-set-state",
///          "arguments": { "name": "qemu_memalign", "enable": true } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "trace-event-set-state")]
#[qapi(returns = "()")]
#[qapi(added = "2.2.0")]
pub struct TraceEventSetState {
    /// Whether to enable tracing.
    pub enable: bool,
    /// Do not match unavailable events with @name.
    #[qapi(name = "ignore-unavailable")]
    pub ignore_unavailable: Option<bool>,
    /// Event name pattern (case-sensitive glob).
    pub name: String,
    /// The vCPU to act upon (all by default; since 2.7).
    #[qapi(feature = "deprecated")]
    #[qapi(added = "2.7.0")]
    #[qapi(deprecated(added = "8.1.0"))]
    #[qapi(removed = "9.1.0")]
    pub vcpu: Option<i64>,
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
/// On failure, QEMU will try delete the newly created internal snapshot
/// in the transaction.  When an I/O error occurs during deletion, the
/// user needs to fix it later with qemu-img or other command.
///
/// Errors:
/// ```text
///     - Any errors from commands in the transaction
/// ```
///
/// .. note:: The transaction aborts on the first failure.  Therefore,
/// ```text
///    there will be information on only one failed operation returned
///    in an error condition, and subsequent actions will not have been
///    attempted.
/// ```
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "transaction",
///          "arguments": { "actions": [
///              { "type": "blockdev-snapshot-sync", "data" : { "device": "ide-hd0",
///                                          "snapshot-file": "/some/place/my-image",
///                                          "format": "qcow2" } },
///              { "type": "blockdev-snapshot-sync", "data" : { "node-name": "myfile",
///                                          "snapshot-file": "/some/place/my-image2",
///                                          "snapshot-node-name": "node3432",
///                                          "mode": "existing",
///                                          "format": "qcow2" } },
///              { "type": "blockdev-snapshot-sync", "data" : { "device": "ide-hd1",
///                                          "snapshot-file": "/some/place/my-image2",
///                                          "mode": "existing",
///                                          "format": "qcow2" } },
///              { "type": "blockdev-snapshot-internal-sync", "data" : {
///                                          "device": "ide-hd2",
///                                          "name": "snapshot0" } } ] } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "transaction")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct Transaction {
    /// List of `TransactionAction`; information needed for the
    /// respective operations.
    #[qapi(added = "1.6.0")]
    #[qapi(required)]
    pub actions: Option<Vec<TransactionAction>>,
    #[qapi(name = "actions")]
    #[qapi(removed = "1.6.0")]
    #[qapi(required)]
    pub actions_legacy: Option<Vec<BlockdevAction>>,
    /// structure of additional options to control the
    /// execution of the transaction.  See `TransactionProperties` for
    /// additional detail.
    #[qapi(added = "2.5.0")]
    pub properties: Option<TransactionProperties>,
}

/// Set watchdog action.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "watchdog-set-action",
///          "arguments": { "action": "inject-nmi" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "watchdog-set-action")]
#[qapi(returns = "()")]
#[qapi(added = "2.11.0")]
pub struct WatchdogSetAction {
    /// `WatchdogAction` action taken when watchdog timer expires.
    pub action: WatchdogAction,
}

/// Query accelerator statistics
///
/// Features:
///
/// @unstable: This command is meant for debugging.
///
/// Returns: accelerator statistics
#[qapi(command)]
#[qapi(name = "x-accel-stats")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "10.1.0")]
#[qapi(unstable(added = "10.1.0"))]
pub struct XAccelStats {}

/// Starts a job to amend format specific options of an existing open
/// block device.  The job is automatically finalized, but a manual
/// `job-dismiss` is required.
///
/// Features:
///
/// @unstable: This command is experimental.
#[qapi(command)]
#[qapi(name = "x-blockdev-amend")]
#[qapi(returns = "()")]
#[qapi(feature = "unstable")]
#[qapi(added = "5.1.0")]
#[qapi(unstable(added = "6.2.0"))]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct XBlockdevAmend {
    /// Allow unsafe operations, format specific.  For luks that
    /// allows erase of the last active keyslot (permanent loss of
    /// data), and replacement of an active keyslot (possible loss of
    /// data if IO error happens)
    pub force: Option<bool>,
    /// Identifier for the newly created job.
    #[qapi(name = "job-id")]
    pub job_id: String,
    /// Name of the block node to work on
    #[qapi(name = "node-name")]
    pub node_name: String,
    /// Options (driver specific)
    pub options: BlockdevAmendOptions,
}

/// Dynamically reconfigure the block driver state graph.
///
/// Currently only supports adding and deleting quorum children.  A
/// child will be added at the end of the list of children.  Its
/// contents *must* be consistent with the other childrens' contents.
/// Deleting a child that is not last in the list of children is
/// problematic, because it "renumbers" the children following it.
///
/// Features:
///
/// @unstable: This command is experimental.
///
/// TODO: Removing children from a quorum node means introducing
/// ```text
///     gaps in the child indices.  This cannot be represented in the
///     'children' list of `BlockdevOptionsQuorum`, as returned by
///     .bdrv_refresh_filename().
/// ```
///
///
/// .. qmp-example::
/// ```text
///    :title: Add a new node to a quorum
/// ```
///
/// ```text
///     -> { "execute": "blockdev-add",
///          "arguments": {
///              "driver": "raw",
///              "node-name": "new_node",
///              "file": { "driver": "file",
///                        "filename": "test.raw" } } }
///     <- { "return": {} }
///     -> { "execute": "x-blockdev-change",
///          "arguments": { "parent": "disk1",
///                         "node": "new_node" } }
///     <- { "return": {} }
/// ```
///
/// .. qmp-example::
/// ```text
///    :title: Delete a quorum's node
/// ```
///
/// ```text
///     -> { "execute": "x-blockdev-change",
///          "arguments": { "parent": "disk1",
///                         "child": "children.1" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "x-blockdev-change")]
#[qapi(returns = "()")]
#[qapi(feature = "unstable")]
#[qapi(added = "2.7.0")]
#[qapi(unstable(added = "6.2.0"))]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct XBlockdevChange {
    /// the name of a child to be deleted.  Mutually exclusive with
    /// @node.
    pub child: Option<String>,
    /// the name of the node to be added.  Mutually exclusive with
    /// @child.
    pub node: Option<String>,
    /// the id or name of the parent node.
    pub parent: String,
}

/// Create an image format on a given node.
/// TODO Replace with something asynchronous (block job?)
#[qapi(command)]
#[qapi(name = "x-blockdev-create")]
#[qapi(returns = "()")]
#[qapi(added = "2.12.0")]
#[qapi(removed = "3.0.0")]
pub struct XBlockdevCreate {
    #[qapi(discriminator)]
    pub driver: BlockdevDriver,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<BlockdevCreateOptionsBranch>,
}

/// Deletes a block device that has been added using blockdev-add.
/// The command will fail if the node is attached to a device or is
/// otherwise being used.
///
/// This command is still a work in progress and is considered
/// experimental. Stay away from it unless you want to help with its
/// development.
#[qapi(command)]
#[qapi(name = "x-blockdev-del")]
#[qapi(returns = "()")]
#[qapi(added = "2.5.0")]
#[qapi(removed = "2.9.0")]
pub struct XBlockdevDel {
    /// #optional Name of the block backend device to delete.
    #[qapi(removed = "2.8.0")]
    pub id: Option<String>,
    /// Name of the graph node to delete.
    #[qapi(name = "node-name")]
    pub node_name: String,
}

/// Inserts a medium (a block driver state tree) into a block device. That block
/// device's tray must currently be open (unless there is no attached guest
/// device) and there must be no medium inserted already.
///
/// Note: This command is still a work in progress and is considered experimental.
/// Stay away from it unless you want to help with its development.
///
///
/// Example:
///
/// -> { "execute": "blockdev-add",
/// ```text
///      "arguments": {
///          "node-name": "node0",
///          "driver": "raw",
///          "file": { "driver": "file",
///                    "filename": "fedora.iso" } } }
/// ```
/// <- { "return": {} }
///
/// -> { "execute": "x-blockdev-insert-medium",
/// ```text
///      "arguments": { "id": "ide0-1-0",
///                     "node-name": "node0" } }
/// ```
///
/// <- { "return": {} }
#[qapi(command)]
#[qapi(name = "x-blockdev-insert-medium")]
#[qapi(returns = "()")]
#[qapi(added = "2.5.0")]
#[qapi(removed = "2.12.0")]
pub struct XBlockdevInsertMedium {
    /// Block device name (deprecated, use @id instead)
    pub device: Option<String>,
    /// The name or QOM path of the guest device (since: 2.8)
    #[qapi(added = "2.8.0")]
    pub id: Option<String>,
    /// name of a node in the block driver state graph
    #[qapi(name = "node-name")]
    pub node_name: String,
}

/// Removes a medium (a block driver state tree) from a block device. That block
/// device's tray must currently be open (unless there is no attached guest
/// device).
///
/// If the tray is open and there is no medium inserted, this will be a no-op.
///
/// Note: This command is still a work in progress and is considered experimental.
/// Stay away from it unless you want to help with its development.
///
///
/// Example:
///
/// -> { "execute": "x-blockdev-remove-medium",
/// ```text
///      "arguments": { "id": "ide0-1-0" } }
/// ```
///
/// <- { "error": { "class": "GenericError",
/// ```text
///                 "desc": "Tray of device 'ide0-1-0' is not open" } }
/// ```
///
/// -> { "execute": "blockdev-open-tray",
/// ```text
///      "arguments": { "id": "ide0-1-0" } }
/// ```
///
/// <- { "timestamp": { "seconds": 1418751627,
/// ```text
///                     "microseconds": 549958 },
///      "event": "DEVICE_TRAY_MOVED",
///      "data": { "device": "ide1-cd0",
///                "id": "ide0-1-0",
///                "tray-open": true } }
/// ```
///
/// <- { "return": {} }
///
/// -> { "execute": "x-blockdev-remove-medium",
/// ```text
///      "arguments": { "id": "ide0-1-0" } }
/// ```
///
/// <- { "return": {} }
#[qapi(command)]
#[qapi(name = "x-blockdev-remove-medium")]
#[qapi(returns = "()")]
#[qapi(added = "2.5.0")]
#[qapi(removed = "2.12.0")]
pub struct XBlockdevRemoveMedium {
    /// Block device name (deprecated, use @id instead)
    pub device: Option<String>,
    /// The name or QOM path of the guest device (since: 2.8)
    #[qapi(added = "2.8.0")]
    pub id: Option<String>,
}

/// Reopens a block device using the given set of options. Any option
/// not specified will be reset to its default value regardless of its
/// previous status. If an option cannot be changed or a particular
/// driver does not support reopening then the command will return an
/// error.
///
/// The top-level @node-name option (from BlockdevOptions) must be
/// specified and is used to select the block device to be reopened.
/// Other @node-name options must be either omitted or set to the
/// current name of the appropriate node. This command won't change any
/// node name and any attempt to do it will result in an error.
///
/// In the case of options that refer to child nodes, the behavior of
/// this command depends on the value:
///
///  1) A set of options (BlockdevOptions): the child is reopened with
/// ```text
///     the specified set of options.
/// ```
///
///  2) A reference to the current child: the child is reopened using
/// ```text
///     its existing set of options.
/// ```
///
///  3) A reference to a different node: the current child is replaced
/// ```text
///     with the specified one.
/// ```
///
///  4) NULL: the current child (if any) is detached.
///
/// Options (1) and (2) are supported in all cases, but at the moment
/// only @backing allows replacing or detaching an existing child.
///
/// Unlike with blockdev-add, the @backing option must always be present
/// unless the node being reopened does not have a backing file and its
/// image does not have a default backing file name as part of its
/// metadata.
#[qapi(command)]
#[qapi(name = "x-blockdev-reopen")]
#[qapi(returns = "()")]
#[qapi(added = "4.0.0")]
#[qapi(removed = "6.1.0")]
pub struct XBlockdevReopen {
    /// if true and @read-only is false, QEMU may automatically
    /// decide not to open the image read-write as requested, but
    /// fall back to read-only instead (and switch between the modes
    /// later), e.g. depending on whether the image file is writable
    /// or whether a writing user is attached to the node
    /// (default: false, since 3.1)
    #[qapi(name = "auto-read-only")]
    pub auto_read_only: Option<bool>,
    /// cache-related options
    pub cache: Option<BlockdevCacheOptions>,
    /// detect and optimize zero writes (Since 2.1)
    /// (default: off)
    #[qapi(name = "detect-zeroes")]
    pub detect_zeroes: Option<BlockdevDetectZeroesOptions>,
    /// discard-related options (default: ignore)
    pub discard: Option<BlockdevDiscardOptions>,
    /// block driver name
    #[qapi(discriminator)]
    pub driver: BlockdevDriver,
    /// force share all permission on added nodes.
    /// Requires read-only=true. (Since 2.10)
    #[qapi(name = "force-share")]
    pub force_share: Option<bool>,
    /// the node name of the new node (Since 2.0).
    /// This option is required on the top level of blockdev-add.
    /// Valid node names start with an alphabetic character and may
    /// contain only alphanumeric characters, '-', '.' and '_'. Their
    /// maximum length is 31 characters.
    #[qapi(name = "node-name")]
    pub node_name: Option<String>,
    /// whether the block device should be read-only (default: false).
    /// Note that some block drivers support only read-only access,
    /// either generally or in certain configurations. In this case,
    /// the default value does not work and the option must be
    /// specified explicitly.
    #[qapi(name = "read-only")]
    pub read_only: Option<bool>,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<BlockdevOptionsBranch>,
}

/// Move @node and its children into the @iothread.  If @iothread is
/// null then move @node and its children into the main loop.
///
/// The node must not be attached to a BlockBackend.
///
/// Features:
///
/// @unstable: This command is experimental and intended for test cases
/// ```text
///     that need control over IOThreads only.
/// ```
///
///
/// .. qmp-example::
/// ```text
///    :title: Move a node into an IOThread
/// ```
///
/// ```text
///     -> { "execute": "x-blockdev-set-iothread",
///          "arguments": { "node-name": "disk1",
///                         "iothread": "iothread0" } }
///     <- { "return": {} }
/// ```
///
/// .. qmp-example::
/// ```text
///    :title: Move a node into the main loop
/// ```
///
/// ```text
///     -> { "execute": "x-blockdev-set-iothread",
///          "arguments": { "node-name": "disk1",
///                         "iothread": null } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "x-blockdev-set-iothread")]
#[qapi(returns = "()")]
#[qapi(feature = "unstable")]
#[qapi(added = "2.12.0")]
#[qapi(unstable(added = "6.2.0"))]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct XBlockdevSetIothread {
    /// true if the node and its children should be moved when a
    /// BlockBackend is already attached
    pub force: Option<bool>,
    /// the name of the IOThread object or null for the main loop
    pub iothread: StrOrNull,
    /// the name of the block driver node
    #[qapi(name = "node-name")]
    pub node_name: String,
}

/// Disables a dirty bitmap so that it will stop tracking disk changes.
///
/// Returns: nothing on success
/// ```text
///          If @node is not a valid block device, DeviceNotFound
///          If @name is not found, GenericError with an explanation
/// ```
///
///
/// Example:
///
/// -> { "execute": "x-block-dirty-bitmap-disable",
/// ```text
///      "arguments": { "node": "drive0", "name": "bitmap0" } }
/// ```
/// <- { "return": {} }
#[qapi(command)]
#[qapi(name = "x-block-dirty-bitmap-disable")]
#[qapi(returns = "()")]
#[qapi(added = "3.0.0")]
#[qapi(removed = "4.0.0")]
pub struct XBlockDirtyBitmapDisable {
    /// name of the dirty bitmap
    pub name: String,
    /// name of device/node which the bitmap is tracking
    pub node: String,
}

/// Enables a dirty bitmap so that it will begin tracking disk changes.
///
/// Returns: nothing on success
/// ```text
///          If @node is not a valid block device, DeviceNotFound
///          If @name is not found, GenericError with an explanation
/// ```
///
///
/// Example:
///
/// -> { "execute": "x-block-dirty-bitmap-enable",
/// ```text
///      "arguments": { "node": "drive0", "name": "bitmap0" } }
/// ```
/// <- { "return": {} }
#[qapi(command)]
#[qapi(name = "x-block-dirty-bitmap-enable")]
#[qapi(returns = "()")]
#[qapi(added = "3.0.0")]
#[qapi(removed = "4.0.0")]
pub struct XBlockDirtyBitmapEnable {
    /// name of the dirty bitmap
    pub name: String,
    /// name of device/node which the bitmap is tracking
    pub node: String,
}

/// FIXME: Rename @src_name and @dst_name to src-name and dst-name.
///
/// Merge @src_name dirty bitmap to @dst_name dirty bitmap. @src_name dirty
/// bitmap is unchanged. On error, @dst_name is unchanged.
///
/// Returns: nothing on success
/// ```text
///          If @node is not a valid block device, DeviceNotFound
///          If @dst_name or @src_name is not found, GenericError
///          If bitmaps has different sizes or granularities, GenericError
/// ```
///
///
/// Example:
///
/// -> { "execute": "x-block-dirty-bitmap-merge",
/// ```text
///      "arguments": { "node": "drive0", "dst_name": "bitmap0",
///                     "src_name": "bitmap1" } }
/// ```
/// <- { "return": {} }
#[qapi(command)]
#[qapi(name = "x-block-dirty-bitmap-merge")]
#[qapi(returns = "()")]
#[qapi(added = "3.0.0")]
#[qapi(removed = "4.0.0")]
pub struct XBlockDirtyBitmapMerge {
    /// name of the destination dirty bitmap
    pub dst_name: String,
    /// name of device/node which the bitmap is tracking
    pub node: String,
    /// name of the source dirty bitmap
    pub src_name: String,
}

/// Manage read, write and flush latency histograms for the device.
///
/// If only @device parameter is specified, remove all present latency histograms
/// for the device. Otherwise, add/reset some of (or all) latency histograms.
///
/// Returns: error if device is not found or any boundary arrays are invalid.
///
///
/// Example: set new histograms for all io types with intervals
/// [0, 10), [10, 50), [50, 100), [100, +inf):
///
/// -> { "execute": "block-latency-histogram-set",
/// ```text
///      "arguments": { "device": "drive0",
///                     "boundaries": [10, 50, 100] } }
/// ```
/// <- { "return": {} }
///
/// Example: set new histogram only for write, other histograms will remain
/// not changed (or not created):
///
/// -> { "execute": "block-latency-histogram-set",
/// ```text
///      "arguments": { "device": "drive0",
///                     "boundaries-write": [10, 50, 100] } }
/// ```
/// <- { "return": {} }
///
/// Example: set new histograms with the following intervals:
///   read, flush: [0, 10), [10, 50), [50, 100), [100, +inf)
///   write: [0, 1000), [1000, 5000), [5000, +inf)
///
/// -> { "execute": "block-latency-histogram-set",
/// ```text
///      "arguments": { "device": "drive0",
///                     "boundaries": [10, 50, 100],
///                     "boundaries-write": [1000, 5000] } }
/// ```
/// <- { "return": {} }
///
/// Example: remove all latency histograms:
///
/// -> { "execute": "block-latency-histogram-set",
/// ```text
///      "arguments": { "device": "drive0" } }
/// ```
/// <- { "return": {} }
#[qapi(command)]
#[qapi(name = "x-block-latency-histogram-set")]
#[qapi(returns = "()")]
#[qapi(added = "2.12.0")]
#[qapi(removed = "4.0.0")]
pub struct XBlockLatencyHistogramSet {
    /// list of interval boundary values (see description in
    /// BlockLatencyHistogramInfo definition). If specified, all
    /// latency histograms are removed, and empty ones created for all
    /// io types with intervals corresponding to @boundaries (except for
    /// io types, for which specific boundaries are set through the
    /// following parameters).
    pub boundaries: Option<Vec<u64>>,
    /// list of interval boundary values for flush latency
    /// histogram.
    #[qapi(name = "boundaries-flush")]
    pub boundaries_flush: Option<Vec<u64>>,
    /// list of interval boundary values for read latency
    /// histogram. If specified, old read latency histogram is
    /// removed, and empty one created with intervals
    /// corresponding to @boundaries-read. The parameter has higher
    /// priority then @boundaries.
    #[qapi(name = "boundaries-read")]
    pub boundaries_read: Option<Vec<u64>>,
    /// list of interval boundary values for write latency
    /// histogram.
    #[qapi(name = "boundaries-write")]
    pub boundaries_write: Option<Vec<u64>>,
    /// device name to set latency histogram for.
    pub device: String,
}

/// Tell QEMU that heartbeat is lost, request it to do takeover
/// procedures.  If this command is sent to the PVM, the Primary side
/// will exit COLO mode.  If sent to the Secondary, the Secondary side
/// will run failover work, then takes over server operation to become
/// the service VM.
///
/// Features:
///
/// @unstable: This command is experimental.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "x-colo-lost-heartbeat" }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "x-colo-lost-heartbeat")]
#[qapi(returns = "()")]
#[qapi(feature = "unstable")]
#[qapi(condition = "CONFIG_REPLICATION")]
#[qapi(added = "2.8.0")]
#[qapi(unstable(added = "6.2.0"))]
pub struct XColoLostHeartbeat {}

/// Get bitmap SHA256.
///
/// Features:
///
/// @unstable: This command is meant for debugging.
///
/// Errors:
/// ```text
///     - If @node is not a valid block device, DeviceNotFound
///     - If @name is not found or if hashing has failed, GenericError
/// ```
#[qapi(command)]
#[qapi(name = "x-debug-block-dirty-bitmap-sha256")]
#[qapi(returns = "BlockDirtyBitmapSha256")]
#[qapi(feature = "unstable")]
#[qapi(added = "2.10.0")]
#[qapi(unstable(added = "6.2.0"))]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct XDebugBlockDirtyBitmapSha256 {
    /// name of the dirty bitmap
    pub name: String,
    /// name of device/node which the bitmap is tracking
    pub node: String,
}

/// Get the block graph.
///
/// Features:
///
/// @unstable: This command is meant for debugging.
#[qapi(command)]
#[qapi(name = "x-debug-query-block-graph")]
#[qapi(returns = "XDbgBlockGraph")]
#[qapi(feature = "unstable")]
#[qapi(added = "4.0.0")]
#[qapi(unstable(added = "6.2.0"))]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct XDebugQueryBlockGraph {}

/// Xen uses this command to notify replication to trigger a checkpoint.
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "xen-colo-do-checkpoint" }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "xen-colo-do-checkpoint")]
#[qapi(returns = "()")]
#[qapi(condition = "defined(CONFIG_REPLICATION)")]
#[qapi(added = "2.9.0")]
pub struct XenColoDoCheckpoint {}

/// Inject a Xen event channel port (interrupt) to the guest.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "xen-event-inject", "arguments": { "port": 1 } }
///     <- { "return": { } }
/// ```
#[qapi(command)]
#[qapi(name = "xen-event-inject")]
#[qapi(condition = "TARGET_I386")]
#[qapi(returns = "()")]
#[qapi(added = "8.0.0")]
pub struct XenEventInject {
    /// The port number
    pub port: u32,
}

/// Query the Xen event channels opened by the guest.
///
/// Returns: list of open event channel ports.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "xen-event-list" }
///     <- { "return": [
///             {
///                 "pending": false,
///                 "port": 1,
///                 "vcpu": 1,
///                 "remote-domain": "qemu",
///                 "masked": false,
///                 "type": "interdomain",
///                 "target": 1
///             },
///             {
///                 "pending": false,
///                 "port": 2,
///                 "vcpu": 0,
///                 "remote-domain": "",
///                 "masked": false,
///                 "type": "virq",
///                 "target": 0
///             }
///          ]
///        }
/// ```
#[qapi(command)]
#[qapi(name = "xen-event-list")]
#[qapi(condition = "TARGET_I386")]
#[qapi(returns = "Vec<EvtchnInfo>")]
#[qapi(added = "8.0.0")]
pub struct XenEventList {}

/// Load the state of all devices from file.  The RAM and the block
/// devices of the VM are not loaded by this command.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "xen-load-devices-state",
///          "arguments": { "filename": "/tmp/resume" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "xen-load-devices-state")]
#[qapi(returns = "()")]
#[qapi(added = "2.7.0")]
pub struct XenLoadDevicesState {
    /// the file to load the state of the devices from as binary
    /// data.  See `xen-save-devices-state`.txt for a description of the
    /// binary format.
    pub filename: String,
}

/// Save the state of all devices to file.  The RAM and the block
/// devices of the VM are not saved by this command.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "xen-save-devices-state",
///          "arguments": { "filename": "/tmp/save" } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "xen-save-devices-state")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct XenSaveDevicesState {
    /// the file to save the state of the devices to as binary
    /// data.  See `xen-save-devices-state`.txt for a description of the
    /// binary format.
    pub filename: String,
    /// Optional argument to ask QEMU to treat this command as part
    /// of a live migration.  Default to true.  (since 2.11)
    #[qapi(added = "2.11.0")]
    pub live: Option<bool>,
}

/// Enable or disable the global dirty log mode.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "xen-set-global-dirty-log",
///          "arguments": { "enable": true } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "xen-set-global-dirty-log")]
#[qapi(returns = "()")]
#[qapi(added = "1.3.0")]
pub struct XenSetGlobalDirtyLog {
    /// true to enable, false to disable.
    pub enable: bool,
}

/// Enable or disable replication.
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "xen-set-replication",
///          "arguments": {"enable": true, "primary": false} }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "xen-set-replication")]
#[qapi(returns = "()")]
#[qapi(condition = "defined(CONFIG_REPLICATION)")]
#[qapi(added = "2.9.0")]
pub struct XenSetReplication {
    /// true to enable, false to disable.
    pub enable: bool,
    /// true to do failover, false to stop.  Cannot be specified
    /// if 'enable' is true.  Default value is false.
    pub failover: Option<bool>,
    /// true for primary or false for secondary.
    pub primary: bool,
}

/// Exit from "preconfig" state
///
/// This command makes QEMU exit the preconfig state and proceed with VM
/// initialization using configuration data provided on the command line
/// and via the QMP monitor during the preconfig state.  The command is
/// only available during the preconfig state (i.e. when the --preconfig
/// command line option was in use).
///
/// Features:
///
/// @unstable: This command is experimental.
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "x-exit-preconfig" }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "x-exit-preconfig")]
#[qapi(returns = "()")]
#[qapi(feature = "unstable")]
#[qapi(added = "3.0.0")]
#[qapi(unstable(added = "6.2.0"))]
#[qapi(allow_preconfig)]
pub struct XExitPreconfig {}

/// Send input event(s) to guest.
///
/// Returns: Nothing on success.
///
///
/// Note: this command is experimental, and not a stable API.  Things that
/// might change before it becomes stable include the spelling of enum
/// values for InputButton and InputAxis, and the notion of how to designate
/// which console will receive the event.
#[qapi(command)]
#[qapi(name = "x-input-send-event")]
#[qapi(returns = "()")]
#[qapi(added = "2.2.0")]
#[qapi(removed = "2.6.0")]
pub struct XInputSendEvent {
    /// #optional console to send event(s) to.
    /// This parameter can be used to send the input event to
    /// specific input devices in case (a) multiple input devices
    /// of the same kind are added to the virtual machine and (b)
    /// you have configured input routing (see docs/multiseat.txt)
    /// for those input devices.  If input routing is not
    /// configured this parameter has no effect.
    /// If @console is missing, only devices that aren't associated
    /// with a console are admissible.
    /// If @console is specified, it must exist, and both devices
    /// associated with that console and devices not associated with a
    /// console are admissible, but the former take precedence.
    pub console: Option<i64>,
    /// List of InputEvent union.
    pub events: Vec<InputEvent>,
}

/// Expose a dirty bitmap associated with the selected export. The bitmap search
/// starts at the device attached to the export, and includes all backing files.
/// The exported bitmap is then locked until the NBD export is removed.
///
/// Note: the client must use NBD_OPT_SET_META_CONTEXT with a query of
/// "qemu:dirty-bitmap:NAME" (where NAME matches @bitmap-export-name) to access
/// the exposed bitmap.
#[qapi(command)]
#[qapi(name = "x-nbd-server-add-bitmap")]
#[qapi(returns = "()")]
#[qapi(added = "3.0.0")]
#[qapi(removed = "4.0.0")]
pub struct XNbdServerAddBitmap {
    /// Bitmap name to search for.
    pub bitmap: String,
    /// How the bitmap will be seen by nbd clients
    /// (default @bitmap)
    #[qapi(name = "bitmap-export-name")]
    pub bitmap_export_name: Option<String>,
    /// Export name.
    pub name: String,
}

/// Test OOB functionality.  When sending this command with lock=true,
/// it'll try to hang the dispatcher.  When sending it with lock=false,
/// it'll try to notify the locked thread to continue.  Note: it should
/// only be used by QMP test program rather than anything else.
///
///
/// Example:
///
/// { "execute": "x-oob-test",
///   "arguments": { "lock": true } }
#[qapi(command)]
#[qapi(name = "x-oob-test")]
#[qapi(returns = "()")]
#[qapi(added = "2.12.0")]
#[qapi(removed = "3.0.0")]
#[qapi(allow_oob)]
pub struct XOobTest {
    pub lock: bool,
}

/// Query information on interrupt controller devices
///
/// Features:
///
/// @unstable: This command is meant for debugging.
///
/// Returns: Interrupt controller devices information
#[qapi(command)]
#[qapi(name = "x-query-interrupt-controllers")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "9.1.0")]
#[qapi(unstable(added = "9.1.0"))]
pub struct XQueryInterruptControllers {}

/// Query interrupt statistics
///
/// Features:
///
/// @unstable: This command is meant for debugging.
///
/// Returns: interrupt statistics
#[qapi(command)]
#[qapi(name = "x-query-irq")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "6.2.0")]
#[qapi(unstable(added = "6.2.0"))]
pub struct XQueryIrq {}

/// Query TCG compiler statistics
///
/// Features:
///
/// @unstable: This command is meant for debugging.
///
/// Returns: TCG compiler statistics
#[qapi(command)]
#[qapi(name = "x-query-jit")]
#[qapi(condition = "CONFIG_TCG")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "6.2.0")]
#[qapi(unstable(added = "6.2.0"))]
pub struct XQueryJit {}

/// Query NUMA topology information
///
/// Features:
///
/// @unstable: This command is meant for debugging.
///
/// Returns: topology information
#[qapi(command)]
#[qapi(name = "x-query-numa")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "6.2.0")]
#[qapi(unstable(added = "6.2.0"))]
pub struct XQueryNuma {}

/// Query TCG opcode counters
///
/// Features:
///
/// @unstable: This command is meant for debugging.
///
/// Returns: TCG opcode counters
#[qapi(command)]
#[qapi(name = "x-query-opcount")]
#[qapi(condition = "CONFIG_TCG")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "6.2.0")]
#[qapi(unstable(added = "6.2.0"))]
#[qapi(removed = "10.1.0")]
pub struct XQueryOpcount {}

/// Query TCG profiling information
///
/// Features:
/// @unstable: This command is meant for debugging.
///
/// Returns: profile information
#[qapi(command)]
#[qapi(name = "x-query-profile")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(condition = "CONFIG_TCG")]
#[qapi(added = "6.2.0")]
#[qapi(unstable(added = "6.2.0"))]
#[qapi(removed = "8.1.0")]
pub struct XQueryProfile {}

/// Query system ramblock information
///
/// Features:
///
/// @unstable: This command is meant for debugging.
///
/// Returns: system ramblock information
#[qapi(command)]
#[qapi(name = "x-query-ramblock")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "6.2.0")]
#[qapi(unstable(added = "6.2.0"))]
pub struct XQueryRamblock {}

/// Query RDMA state
///
/// Features:
///
/// @unstable: This command is meant for debugging.
///
/// Returns: RDMA state
#[qapi(command)]
#[qapi(name = "x-query-rdma")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "6.2.0")]
#[qapi(unstable(added = "6.2.0"))]
#[qapi(removed = "9.1.0")]
pub struct XQueryRdma {}

/// Query information on the registered ROMS
///
/// Features:
///
/// @unstable: This command is meant for debugging.
///
/// Returns: registered ROMs
#[qapi(command)]
#[qapi(name = "x-query-roms")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "6.2.0")]
#[qapi(unstable(added = "6.2.0"))]
pub struct XQueryRoms {}

/// Query information on the USB devices
///
/// Features:
///
/// @unstable: This command is meant for debugging.
///
/// Returns: USB device information
#[qapi(command)]
#[qapi(name = "x-query-usb")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "6.2.0")]
#[qapi(unstable(added = "6.2.0"))]
pub struct XQueryUsb {}

/// Return a list of all realized VirtIODevices
///
/// Features:
///
/// @unstable: This command is meant for debugging.
///
/// Returns: List of gathered VirtIODevices
///
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "x-query-virtio" }
///     <- { "return": [
///              {
///                  "name": "virtio-input",
///                  "path": "/machine/peripheral-anon/device[4]/virtio-backend"
///              },
///              {
///                  "name": "virtio-crypto",
///                  "path": "/machine/peripheral/crypto0/virtio-backend"
///              },
///              {
///                  "name": "virtio-scsi",
///                  "path": "/machine/peripheral-anon/device[2]/virtio-backend"
///              },
///              {
///                  "name": "virtio-net",
///                  "path": "/machine/peripheral-anon/device[1]/virtio-backend"
///              },
///              {
///                  "name": "virtio-serial",
///                  "path": "/machine/peripheral-anon/device[0]/virtio-backend"
///              }
///          ]
///        }
/// ```
#[qapi(command)]
#[qapi(name = "x-query-virtio")]
#[qapi(feature = "unstable")]
#[qapi(returns = "Vec<VirtioInfo>")]
#[qapi(added = "7.2.0")]
#[qapi(unstable(added = "7.2.0"))]
pub struct XQueryVirtio {}

/// Return the information about a VirtQueue's VirtQueueElement
///
/// Features:
///
/// @unstable: This command is meant for debugging.
///
///
/// .. qmp-example::
/// ```text
///    :title: Introspect on virtio-net's VirtQueue 0 at index 5
/// ```
///
/// ```text
///     -> { "execute": "x-query-virtio-queue-element",
///          "arguments": { "path": "/machine/peripheral-anon/device[1]/virtio-backend",
///                         "queue": 0,
///                         "index": 5 }
///        }
///     <- { "return": {
///              "index": 5,
///              "name": "virtio-net",
///              "descs": [
///                  {
///                      "flags": ["write"],
///                      "len": 1536,
///                      "addr": 5257305600
///                  }
///              ],
///              "avail": {
///                  "idx": 256,
///                  "flags": 0,
///                  "ring": 5
///              },
///              "used": {
///                  "idx": 13,
///                  "flags": 0
///              }
///          }
///        }
/// ```
///
/// .. qmp-example::
/// ```text
///    :title: Introspect on virtio-crypto's VirtQueue 1 at head
/// ```
///
/// ```text
///     -> { "execute": "x-query-virtio-queue-element",
///          "arguments": { "path": "/machine/peripheral/crypto0/virtio-backend",
///                         "queue": 1 }
///        }
///     <- { "return": {
///              "index": 0,
///              "name": "virtio-crypto",
///              "descs": [
///                  {
///                      "flags": [],
///                      "len": 0,
///                      "addr": 8080268923184214134
///                  }
///              ],
///              "avail": {
///                  "idx": 280,
///                  "flags": 0,
///                  "ring": 0
///              },
///              "used": {
///                  "idx": 280,
///                  "flags": 0
///              }
///          }
///        }
/// ```
///
/// .. qmp-example::
/// ```text
///    :title: Introspect on virtio-scsi's VirtQueue 2 at head
/// ```
///
/// ```text
///     -> { "execute": "x-query-virtio-queue-element",
///          "arguments": { "path": "/machine/peripheral-anon/device[2]/virtio-backend",
///                         "queue": 2 }
///        }
///     <- { "return": {
///              "index": 19,
///              "name": "virtio-scsi",
///              "descs": [
///                  {
///                      "flags": ["used", "indirect", "write"],
///                      "len": 4099327944,
///                      "addr": 12055409292258155293
///                  }
///              ],
///              "avail": {
///                  "idx": 1147,
///                  "flags": 0,
///                  "ring": 19
///              },
///              "used": {
///                  "idx": 280,
///                  "flags": 0
///              }
///          }
///        }
/// ```
#[qapi(command)]
#[qapi(name = "x-query-virtio-queue-element")]
#[qapi(feature = "unstable")]
#[qapi(returns = "VirtioQueueElement")]
#[qapi(added = "7.2.0")]
#[qapi(unstable(added = "7.2.0"))]
pub struct XQueryVirtioQueueElement {
    /// Index of the element in the queue (default: head of the
    /// queue)
    pub index: Option<u16>,
    /// VirtIODevice canonical QOM path
    pub path: String,
    /// VirtQueue index to examine
    pub queue: u16,
}

/// Return the status of a given VirtIODevice's VirtQueue
///
/// Features:
///
/// @unstable: This command is meant for debugging.
///
/// Returns: Status of the queue
///
/// .. note:: last_avail_idx will not be displayed in the case where the
/// ```text
///    selected VirtIODevice has a running vhost device and the
///    VirtIODevice VirtQueue index (queue) does not exist for the
///    corresponding vhost device vhost_virtqueue.  Also,
///    shadow_avail_idx will not be displayed in the case where the
///    selected VirtIODevice has a running vhost device.
/// ```
///
///
/// .. qmp-example::
/// ```text
///    :annotated:
/// ```
///
/// ```text
///    Get `VirtQueueStatus` for virtio-vsock (vhost-vsock running)
///    ::
/// ```
///
/// ```text
///     -> { "execute": "x-query-virtio-queue-status",
///          "arguments": { "path": "/machine/peripheral/vsock0/virtio-backend",
///                         "queue": 1 }
///        }
///     <- { "return": {
///              "signalled-used": 0,
///              "inuse": 0,
///              "name": "vhost-vsock",
///              "vring-align": 4096,
///              "vring-desc": 5217370112,
///              "signalled-used-valid": false,
///              "vring-num-default": 128,
///              "vring-avail": 5217372160,
///              "queue-index": 1,
///              "last-avail-idx": 0,
///              "vring-used": 5217372480,
///              "used-idx": 0,
///              "vring-num": 128
///          }
///        }
/// ```
///
/// .. qmp-example::
/// ```text
///    :annotated:
/// ```
///
/// ```text
///    Get `VirtQueueStatus` for virtio-serial (no vhost)
///    ::
/// ```
///
/// ```text
///     -> { "execute": "x-query-virtio-queue-status",
///          "arguments": { "path": "/machine/peripheral-anon/device[0]/virtio-backend",
///                         "queue": 20 }
///        }
///     <- { "return": {
///              "signalled-used": 0,
///              "inuse": 0,
///              "name": "virtio-serial",
///              "vring-align": 4096,
///              "vring-desc": 5182074880,
///              "signalled-used-valid": false,
///              "vring-num-default": 128,
///              "vring-avail": 5182076928,
///              "queue-index": 20,
///              "last-avail-idx": 0,
///              "vring-used": 5182077248,
///              "used-idx": 0,
///              "shadow-avail-idx": 0,
///              "vring-num": 128
///          }
///        }
/// ```
#[qapi(command)]
#[qapi(name = "x-query-virtio-queue-status")]
#[qapi(feature = "unstable")]
#[qapi(returns = "VirtQueueStatus")]
#[qapi(added = "7.2.0")]
#[qapi(unstable(added = "7.2.0"))]
pub struct XQueryVirtioQueueStatus {
    /// VirtIODevice canonical QOM path
    pub path: String,
    /// VirtQueue index to examine
    pub queue: u16,
}

/// Poll for a comprehensive status of a given virtio device
///
/// Features:
///
/// @unstable: This command is meant for debugging.
///
/// Returns: Status of the virtio device
///
///
/// .. qmp-example::
/// ```text
///    :annotated:
/// ```
///
/// ```text
///    Poll for the status of virtio-crypto (no vhost-crypto active)
///    ::
/// ```
///
/// ```text
///     -> { "execute": "x-query-virtio-status",
///          "arguments": { "path": "/machine/peripheral/crypto0/virtio-backend" }
///        }
///     <- { "return": {
///              "device-endian": "little",
///              "bus-name": "",
///              "disable-legacy-check": false,
///              "name": "virtio-crypto",
///              "started": true,
///              "device-id": 20,
///              "backend-features": {
///                  "transports": [],
///                  "dev-features": []
///              },
///              "start-on-kick": false,
///              "isr": 1,
///              "broken": false,
///              "status": {
///                  "statuses": [
///                      "VIRTIO_CONFIG_S_ACKNOWLEDGE: Valid virtio device found",
///                      "VIRTIO_CONFIG_S_DRIVER: Guest OS compatible with device",
///                      "VIRTIO_CONFIG_S_FEATURES_OK: Feature negotiation complete",
///                      "VIRTIO_CONFIG_S_DRIVER_OK: Driver setup and ready"
///                  ]
///              },
///              "num-vqs": 2,
///              "guest-features": {
///                  "dev-features": [],
///                  "transports": [
///                      "VIRTIO_RING_F_EVENT_IDX: Used & avail. event fields enabled",
///                      "VIRTIO_RING_F_INDIRECT_DESC: Indirect descriptors supported",
///                      "VIRTIO_F_VERSION_1: Device compliant for v1 spec (legacy)"
///                  ]
///              },
///              "host-features": {
///                  "unknown-dev-features": 1073741824,
///                  "unknown-dev-features2": 0,
///                  "dev-features": [],
///                  "transports": [
///                      "VIRTIO_RING_F_EVENT_IDX: Used & avail. event fields enabled",
///                      "VIRTIO_RING_F_INDIRECT_DESC: Indirect descriptors supported",
///                      "VIRTIO_F_VERSION_1: Device compliant for v1 spec (legacy)",
///                      "VIRTIO_F_ANY_LAYOUT: Device accepts arbitrary desc. layouts",
///                      "VIRTIO_F_NOTIFY_ON_EMPTY: Notify when device runs out of avail. descs. on VQ"
///                  ]
///              },
///              "use-guest-notifier-mask": true,
///              "vm-running": true,
///              "queue-sel": 1,
///              "disabled": false,
///              "vhost-started": false,
///              "use-started": true
///          }
///        }
/// ```
///
/// .. qmp-example::
/// ```text
///    :annotated:
/// ```
///
/// ```text
///    Poll for the status of virtio-net (vhost-net is active)
///    ::
/// ```
///
/// ```text
///     -> { "execute": "x-query-virtio-status",
///          "arguments": { "path": "/machine/peripheral-anon/device[1]/virtio-backend" }
///        }
///     <- { "return": {
///              "device-endian": "little",
///              "bus-name": "",
///              "disabled-legacy-check": false,
///              "name": "virtio-net",
///              "started": true,
///              "device-id": 1,
///              "vhost-dev": {
///                  "n-tmp-sections": 4,
///                  "n-mem-sections": 4,
///                  "max-queues": 1,
///                  "backend-cap": 2,
///                  "log-size": 0,
///                  "backend-features": {
///                      "dev-features": [],
///                      "transports": []
///                  },
///                  "nvqs": 2,
///                  "protocol-features": {
///                      "protocols": []
///                  },
///                  "vq-index": 0,
///                  "log-enabled": false,
///                  "acked-features": {
///                      "dev-features": [
///                          "VIRTIO_NET_F_MRG_RXBUF: Driver can merge receive buffers"
///                      ],
///                      "transports": [
///                          "VIRTIO_RING_F_EVENT_IDX: Used & avail. event fields enabled",
///                          "VIRTIO_RING_F_INDIRECT_DESC: Indirect descriptors supported",
///                          "VIRTIO_F_VERSION_1: Device compliant for v1 spec (legacy)"
///                      ]
///                  },
///                  "features": {
///                      "dev-features": [
///                          "VHOST_F_LOG_ALL: Logging write descriptors supported",
///                          "VIRTIO_NET_F_MRG_RXBUF: Driver can merge receive buffers"
///                      ],
///                      "transports": [
///                          "VIRTIO_RING_F_EVENT_IDX: Used & avail. event fields enabled",
///                          "VIRTIO_RING_F_INDIRECT_DESC: Indirect descriptors supported",
///                          "VIRTIO_F_IOMMU_PLATFORM: Device can be used on IOMMU platform",
///                          "VIRTIO_F_VERSION_1: Device compliant for v1 spec (legacy)",
///                          "VIRTIO_F_ANY_LAYOUT: Device accepts arbitrary desc. layouts",
///                          "VIRTIO_F_NOTIFY_ON_EMPTY: Notify when device runs out of avail. descs. on VQ"
///                      ]
///                  }
///              },
///              "backend-features": {
///                  "dev-features": [
///                      "VHOST_USER_F_PROTOCOL_FEATURES: Vhost-user protocol features negotiation supported",
///                      "VIRTIO_NET_F_GSO: Handling GSO-type packets supported",
///                      "VIRTIO_NET_F_CTRL_MAC_ADDR: MAC address set through control channel",
///                      "VIRTIO_NET_F_GUEST_ANNOUNCE: Driver sending gratuitous packets supported",
///                      "VIRTIO_NET_F_CTRL_RX_EXTRA: Extra RX mode control supported",
///                      "VIRTIO_NET_F_CTRL_VLAN: Control channel VLAN filtering supported",
///                      "VIRTIO_NET_F_CTRL_RX: Control channel RX mode supported",
///                      "VIRTIO_NET_F_CTRL_VQ: Control channel available",
///                      "VIRTIO_NET_F_STATUS: Configuration status field available",
///                      "VIRTIO_NET_F_MRG_RXBUF: Driver can merge receive buffers",
///                      "VIRTIO_NET_F_HOST_UFO: Device can receive UFO",
///                      "VIRTIO_NET_F_HOST_ECN: Device can receive TSO with ECN",
///                      "VIRTIO_NET_F_HOST_TSO6: Device can receive TSOv6",
///                      "VIRTIO_NET_F_HOST_TSO4: Device can receive TSOv4",
///                      "VIRTIO_NET_F_GUEST_UFO: Driver can receive UFO",
///                      "VIRTIO_NET_F_GUEST_ECN: Driver can receive TSO with ECN",
///                      "VIRTIO_NET_F_GUEST_TSO6: Driver can receive TSOv6",
///                      "VIRTIO_NET_F_GUEST_TSO4: Driver can receive TSOv4",
///                      "VIRTIO_NET_F_MAC: Device has given MAC address",
///                      "VIRTIO_NET_F_CTRL_GUEST_OFFLOADS: Control channel offloading reconfig. supported",
///                      "VIRTIO_NET_F_GUEST_CSUM: Driver handling packets with partial checksum supported",
///                      "VIRTIO_NET_F_CSUM: Device handling packets with partial checksum supported"
///                  ],
///                  "transports": [
///                      "VIRTIO_RING_F_EVENT_IDX: Used & avail. event fields enabled",
///                      "VIRTIO_RING_F_INDIRECT_DESC: Indirect descriptors supported",
///                      "VIRTIO_F_VERSION_1: Device compliant for v1 spec (legacy)",
///                      "VIRTIO_F_ANY_LAYOUT: Device accepts arbitrary desc. layouts",
///                      "VIRTIO_F_NOTIFY_ON_EMPTY: Notify when device runs out of avail. descs. on VQ"
///                  ]
///              },
///              "start-on-kick": false,
///              "isr": 1,
///              "broken": false,
///              "status": {
///                  "statuses": [
///                      "VIRTIO_CONFIG_S_ACKNOWLEDGE: Valid virtio device found",
///                      "VIRTIO_CONFIG_S_DRIVER: Guest OS compatible with device",
///                      "VIRTIO_CONFIG_S_FEATURES_OK: Feature negotiation complete",
///                      "VIRTIO_CONFIG_S_DRIVER_OK: Driver setup and ready"
///                  ]
///              },
///              "num-vqs": 3,
///              "guest-features": {
///                  "dev-features": [
///                      "VIRTIO_NET_F_CTRL_MAC_ADDR: MAC address set through control channel",
///                      "VIRTIO_NET_F_GUEST_ANNOUNCE: Driver sending gratuitous packets supported",
///                      "VIRTIO_NET_F_CTRL_VLAN: Control channel VLAN filtering supported",
///                      "VIRTIO_NET_F_CTRL_RX: Control channel RX mode supported",
///                      "VIRTIO_NET_F_CTRL_VQ: Control channel available",
///                      "VIRTIO_NET_F_STATUS: Configuration status field available",
///                      "VIRTIO_NET_F_MRG_RXBUF: Driver can merge receive buffers",
///                      "VIRTIO_NET_F_HOST_UFO: Device can receive UFO",
///                      "VIRTIO_NET_F_HOST_ECN: Device can receive TSO with ECN",
///                      "VIRTIO_NET_F_HOST_TSO6: Device can receive TSOv6",
///                      "VIRTIO_NET_F_HOST_TSO4: Device can receive TSOv4",
///                      "VIRTIO_NET_F_GUEST_UFO: Driver can receive UFO",
///                      "VIRTIO_NET_F_GUEST_ECN: Driver can receive TSO with ECN",
///                      "VIRTIO_NET_F_GUEST_TSO6: Driver can receive TSOv6",
///                      "VIRTIO_NET_F_GUEST_TSO4: Driver can receive TSOv4",
///                      "VIRTIO_NET_F_MAC: Device has given MAC address",
///                      "VIRTIO_NET_F_CTRL_GUEST_OFFLOADS: Control channel offloading reconfig. supported",
///                      "VIRTIO_NET_F_GUEST_CSUM: Driver handling packets with partial checksum supported",
///                      "VIRTIO_NET_F_CSUM: Device handling packets with partial checksum supported"
///                  ],
///                  "transports": [
///                      "VIRTIO_RING_F_EVENT_IDX: Used & avail. event fields enabled",
///                      "VIRTIO_RING_F_INDIRECT_DESC: Indirect descriptors supported",
///                      "VIRTIO_F_VERSION_1: Device compliant for v1 spec (legacy)"
///                 ]
///              },
///              "host-features": {
///                  "dev-features": [
///                      "VHOST_USER_F_PROTOCOL_FEATURES: Vhost-user protocol features negotiation supported",
///                      "VIRTIO_NET_F_GSO: Handling GSO-type packets supported",
///                      "VIRTIO_NET_F_CTRL_MAC_ADDR: MAC address set through control channel",
///                      "VIRTIO_NET_F_GUEST_ANNOUNCE: Driver sending gratuitous packets supported",
///                      "VIRTIO_NET_F_CTRL_RX_EXTRA: Extra RX mode control supported",
///                      "VIRTIO_NET_F_CTRL_VLAN: Control channel VLAN filtering supported",
///                      "VIRTIO_NET_F_CTRL_RX: Control channel RX mode supported",
///                      "VIRTIO_NET_F_CTRL_VQ: Control channel available",
///                      "VIRTIO_NET_F_STATUS: Configuration status field available",
///                      "VIRTIO_NET_F_MRG_RXBUF: Driver can merge receive buffers",
///                      "VIRTIO_NET_F_HOST_UFO: Device can receive UFO",
///                      "VIRTIO_NET_F_HOST_ECN: Device can receive TSO with ECN",
///                      "VIRTIO_NET_F_HOST_TSO6: Device can receive TSOv6",
///                      "VIRTIO_NET_F_HOST_TSO4: Device can receive TSOv4",
///                      "VIRTIO_NET_F_GUEST_UFO: Driver can receive UFO",
///                      "VIRTIO_NET_F_GUEST_ECN: Driver can receive TSO with ECN",
///                      "VIRTIO_NET_F_GUEST_TSO6: Driver can receive TSOv6",
///                      "VIRTIO_NET_F_GUEST_TSO4: Driver can receive TSOv4",
///                      "VIRTIO_NET_F_MAC: Device has given MAC address",
///                      "VIRTIO_NET_F_CTRL_GUEST_OFFLOADS: Control channel offloading reconfig. supported",
///                      "VIRTIO_NET_F_GUEST_CSUM: Driver handling packets with partial checksum supported",
///                      "VIRTIO_NET_F_CSUM: Device handling packets with partial checksum supported"
///                  ],
///                  "transports": [
///                      "VIRTIO_RING_F_EVENT_IDX: Used & avail. event fields enabled",
///                      "VIRTIO_RING_F_INDIRECT_DESC: Indirect descriptors supported",
///                      "VIRTIO_F_VERSION_1: Device compliant for v1 spec (legacy)",
///                      "VIRTIO_F_ANY_LAYOUT: Device accepts arbitrary desc. layouts",
///                      "VIRTIO_F_NOTIFY_ON_EMPTY: Notify when device runs out of avail. descs. on VQ"
///                 ]
///              },
///              "use-guest-notifier-mask": true,
///              "vm-running": true,
///              "queue-sel": 2,
///              "disabled": false,
///              "vhost-started": true,
///              "use-started": true
///          }
///        }
/// ```
#[qapi(command)]
#[qapi(name = "x-query-virtio-status")]
#[qapi(feature = "unstable")]
#[qapi(returns = "VirtioStatus")]
#[qapi(added = "7.2.0")]
#[qapi(unstable(added = "7.2.0"))]
pub struct XQueryVirtioStatus {
    /// Canonical QOM path of the VirtIODevice
    pub path: String,
}

/// Return information of a given vhost device's vhost_virtqueue
///
/// Features:
///
/// @unstable: This command is meant for debugging.
///
/// Returns: Status of the vhost_virtqueue
///
///
/// .. qmp-example::
/// ```text
///    :title: Get vhost_virtqueue status for vhost-crypto
/// ```
///
/// ```text
///     -> { "execute": "x-query-virtio-vhost-queue-status",
///          "arguments": { "path": "/machine/peripheral/crypto0/virtio-backend",
///                         "queue": 0 }
///        }
///     <- { "return": {
///              "avail-phys": 5216124928,
///              "name": "virtio-crypto",
///              "used-phys": 5216127040,
///              "avail-size": 2054,
///              "desc-size": 16384,
///              "used-size": 8198,
///              "num": 1024,
///              "call": 0,
///              "desc-phys": 5216108544,
///              "kick": 0
///          }
///        }
/// ```
///
/// .. qmp-example::
/// ```text
///    :title: Get vhost_virtqueue status for vhost-vsock
/// ```
///
/// ```text
///     -> { "execute": "x-query-virtio-vhost-queue-status",
///          "arguments": { "path": "/machine/peripheral/vsock0/virtio-backend",
///                         "queue": 0 }
///        }
///     <- { "return": {
///              "avail-phys": 5182261248,
///              "name": "vhost-vsock",
///              "used-phys": 5182261568,
///              "avail-size": 262,
///              "desc-size": 2048,
///              "used-size": 1030,
///              "num": 128,
///              "call": 0,
///              "desc-phys": 5182259200,
///              "kick": 0
///          }
///        }
/// ```
#[qapi(command)]
#[qapi(name = "x-query-virtio-vhost-queue-status")]
#[qapi(feature = "unstable")]
#[qapi(returns = "VirtVhostQueueStatus")]
#[qapi(added = "7.2.0")]
#[qapi(unstable(added = "7.2.0"))]
pub struct XQueryVirtioVhostQueueStatus {
    /// VirtIODevice canonical QOM path
    pub path: String,
    /// vhost_virtqueue index to examine
    pub queue: u16,
}

/// Try to recover from hanging QEMU by yanking the specified instances.
/// See `YankInstance` for more information.
///
/// Errors:
/// ```text
///     - If any of the YankInstances doesn't exist, DeviceNotFound
/// ```
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "yank",
///          "arguments": {
///              "instances": [
///                   { "type": "block-node",
///                     "node-name": "nbd0" }
///              ] } }
///     <- { "return": {} }
/// ```
#[qapi(command)]
#[qapi(name = "yank")]
#[qapi(returns = "()")]
#[qapi(added = "6.0.0")]
#[qapi(allow_oob)]
pub struct Yank {
    /// the instances to be yanked
    pub instances: Vec<YankInstance>,
}

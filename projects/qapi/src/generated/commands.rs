#![allow(unused_imports, non_camel_case_types, dead_code)]
use crate as qapi;
use qapi_macros::qapi;
use std::collections::HashMap;
use super::*;
#[qapi(command)]
#[qapi(name = "add_client")]
#[qapi(returns = "()")]
#[qapi(added = "1.3.0")]
pub struct AddClient {
    pub fdname: String,
    pub protocol: String,
    pub skipauth: Option<bool>,
    pub tls: Option<bool>,
}

#[qapi(command)]
#[qapi(name = "add-fd")]
#[qapi(returns = "AddfdInfo")]
#[qapi(added = "1.2.0")]
pub struct AddFd {
    #[qapi(name = "fdset-id")]
    pub fdset_id: Option<i64>,
    pub opaque: Option<String>,
}

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

#[qapi(command)]
#[qapi(name = "balloon")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct Balloon {
    pub value: i64,
}

#[qapi(command)]
#[qapi(name = "block-commit")]
#[qapi(returns = "()")]
#[qapi(added = "1.3.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockCommit {
    #[qapi(name = "auto-dismiss")]
    #[qapi(added = "3.1.0")]
    pub auto_dismiss: Option<bool>,
    #[qapi(name = "auto-finalize")]
    #[qapi(added = "3.1.0")]
    pub auto_finalize: Option<bool>,
    #[qapi(name = "backing-file")]
    #[qapi(added = "2.1.0")]
    pub backing_file: Option<String>,
    #[qapi(name = "backing-mask-protocol")]
    #[qapi(added = "9.0.0")]
    pub backing_mask_protocol: Option<bool>,
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "5.0.0"))]
    pub base: Option<String>,
    #[qapi(name = "base-node")]
    #[qapi(added = "3.1.0")]
    pub base_node: Option<String>,
    pub device: String,
    #[qapi(name = "filter-node-name")]
    #[qapi(added = "2.9.0")]
    pub filter_node_name: Option<String>,
    #[qapi(name = "job-id")]
    #[qapi(added = "2.7.0")]
    pub job_id: Option<String>,
    #[qapi(name = "on-error")]
    #[qapi(added = "5.0.0")]
    pub on_error: Option<BlockdevOnError>,
    pub speed: Option<i64>,
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "5.0.0"))]
    pub top: Option<String>,
    #[qapi(name = "top-node")]
    #[qapi(added = "3.1.0")]
    pub top_node: Option<String>,
}

/// Creates a new block device. If the @id option is given at the top level, a
/// BlockBackend will be created; otherwise, @node-name is mandatory at the top
/// level and no BlockBackend will be created.
///
/// This command is still a work in progress.  It doesn't support all
/// block drivers among other things.  Stay away from it unless you want
/// to help with its development.
///
/// For the arguments, see the documentation of BlockdevOptions.
///
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

/// Changes the medium inserted into a block device by ejecting the current medium
/// and loading a new image file which is inserted as the new medium (this command
/// combines blockdev-open-tray, x-blockdev-remove-medium,
/// x-blockdev-insert-medium and blockdev-close-tray).
///
#[qapi(command)]
#[qapi(name = "blockdev-change-medium")]
#[qapi(returns = "()")]
#[qapi(added = "2.5.0")]
pub struct BlockdevChangeMedium {
    /// optional Block device name (deprecated, use @id instead)
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "5.0.0"))]
    pub device: Option<String>,
    /// filename of the new image to be loaded
    pub filename: String,
    #[qapi(added = "7.1.0")]
    pub force: Option<bool>,
    /// optional, format to open the new image with (defaults to
    /// the probed format)
    pub format: Option<String>,
    /// optional The name or QOM path of the guest device
    /// (since: 2.8)
    #[qapi(added = "2.8.0")]
    pub id: Option<String>,
    /// optional, change the read-only mode of the device; defaults
    /// to 'retain'
    #[qapi(name = "read-only-mode")]
    pub read_only_mode: Option<BlockdevChangeReadOnlyMode>,
}

/// Closes a block device's tray. If there is a block driver state tree associated
/// with the block device (which is currently ejected), that tree will be loaded
/// as the medium.
///
/// If the tray was already closed before, this will be a no-op.
///
#[qapi(command)]
#[qapi(name = "blockdev-close-tray")]
#[qapi(returns = "()")]
#[qapi(added = "2.5.0")]
pub struct BlockdevCloseTray {
    /// optional Block device name (deprecated, use @id instead)
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "5.0.0"))]
    pub device: Option<String>,
    /// optional The name or QOM path of the guest device (since: 2.8)
    #[qapi(added = "2.8.0")]
    pub id: Option<String>,
}

/// Starts a job to create an image format on a given node.  The job is
/// automatically finalized, but a manual `job-dismiss` is required.
///
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

#[qapi(command)]
#[qapi(name = "blockdev-del")]
#[qapi(returns = "()")]
#[qapi(added = "2.9.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockdevDel {
    #[qapi(name = "node-name")]
    pub node_name: String,
}

#[qapi(command)]
#[qapi(name = "blockdev-insert-medium")]
#[qapi(returns = "()")]
#[qapi(added = "2.12.0")]
pub struct BlockdevInsertMedium {
    pub id: String,
    #[qapi(name = "node-name")]
    pub node_name: String,
}

#[qapi(command)]
#[qapi(name = "blockdev-mirror")]
#[qapi(returns = "()")]
#[qapi(added = "2.6.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockdevMirror {
    #[qapi(name = "auto-dismiss")]
    #[qapi(added = "3.1.0")]
    pub auto_dismiss: Option<bool>,
    #[qapi(name = "auto-finalize")]
    #[qapi(added = "3.1.0")]
    pub auto_finalize: Option<bool>,
    #[qapi(name = "buf-size")]
    pub buf_size: Option<i64>,
    #[qapi(name = "copy-mode")]
    #[qapi(added = "3.0.0")]
    pub copy_mode: Option<MirrorCopyMode>,
    pub device: String,
    #[qapi(name = "filter-node-name")]
    #[qapi(added = "2.9.0")]
    pub filter_node_name: Option<String>,
    pub granularity: Option<u32>,
    #[qapi(name = "job-id")]
    #[qapi(added = "2.7.0")]
    pub job_id: Option<String>,
    #[qapi(name = "on-source-error")]
    pub on_source_error: Option<BlockdevOnError>,
    #[qapi(name = "on-target-error")]
    pub on_target_error: Option<BlockdevOnError>,
    pub replaces: Option<String>,
    pub speed: Option<i64>,
    pub sync: MirrorSyncMode,
    pub target: String,
    #[qapi(name = "target-is-zero")]
    #[qapi(added = "10.1.0")]
    pub target_is_zero: Option<bool>,
}

/// Opens a block device's tray. If there is a block driver state tree inserted as
/// a medium, it will become inaccessible to the guest (but it will remain
/// associated to the block device, so closing the tray will make it accessible
/// again).
///
/// If the tray was already open before, this will be a no-op.
///
/// Once the tray opens, a DEVICE_TRAY_MOVED event is emitted. There are cases in
/// which no such event will be generated, these include:
/// - if the guest has locked the tray, @force is false and the guest does not
///   respond to the eject request
/// - if the BlockBackend denoted by @device does not have a guest device attached
///   to it
/// - if the guest device does not have an actual tray
///
#[qapi(command)]
#[qapi(name = "blockdev-open-tray")]
#[qapi(returns = "()")]
#[qapi(added = "2.5.0")]
pub struct BlockdevOpenTray {
    /// optional Block device name (deprecated, use @id instead)
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "5.0.0"))]
    pub device: Option<String>,
    /// optional if false (the default), an eject request will be sent to
    /// the guest if it has locked the tray (and the tray will not be opened
    /// immediately); if true, the tray will be opened regardless of whether
    /// it is locked
    pub force: Option<bool>,
    /// optional The name or QOM path of the guest device (since: 2.8)
    #[qapi(added = "2.8.0")]
    pub id: Option<String>,
}

#[qapi(command)]
#[qapi(name = "blockdev-remove-medium")]
#[qapi(returns = "()")]
#[qapi(added = "2.12.0")]
pub struct BlockdevRemoveMedium {
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
///
#[qapi(command)]
#[qapi(name = "blockdev-reopen")]
#[qapi(returns = "()")]
#[qapi(added = "6.1.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockdevReopen {
    pub options: Vec<BlockdevOptions>,
}

#[qapi(command)]
#[qapi(name = "blockdev-set-active")]
#[qapi(returns = "()")]
#[qapi(added = "10.0.0")]
#[qapi(allow_preconfig)]
pub struct BlockdevSetActive {
    pub active: bool,
    #[qapi(name = "node-name")]
    pub node_name: Option<String>,
}

#[qapi(command)]
#[qapi(name = "blockdev-snapshot-delete-internal-sync")]
#[qapi(returns = "SnapshotInfo")]
#[qapi(added = "1.7.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockdevSnapshotDeleteInternalSync {
    pub device: String,
    pub id: Option<String>,
    pub name: Option<String>,
}

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
///
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
    #[qapi(name = "fixed-iothread")]
    pub fixed_iothread: Option<bool>,
    /// A unique identifier for the block export (across all export
    /// types)
    pub id: String,
    /// is given, export creation fails if the block node
    /// cannot be moved to the iothread.  Must not be true when giving
    /// multiple iothreads for @iothread.  The default is false.
    /// (since: 5.2)
    pub iothread: Option<BlockExportIothreads>,
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

#[qapi(command)]
#[qapi(name = "block-export-del")]
#[qapi(returns = "()")]
#[qapi(added = "5.2.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockExportDel {
    pub id: String,
    pub mode: Option<BlockExportRemoveMode>,
}

#[qapi(command)]
#[qapi(name = "block-job-cancel")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockJobCancel {
    pub device: String,
    #[qapi(added = "1.3.0")]
    pub force: Option<bool>,
}

/// Change the block job's options.
///
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

#[qapi(command)]
#[qapi(name = "block-job-complete")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.3.0")]
#[qapi(deprecated(added = "10.1.0"))]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockJobComplete {
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
///
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

/// Once a job that has manual=true reaches the pending state, it can be
/// instructed to finalize any graph changes and do any necessary
/// cleanup via this command.  For jobs in a transaction, instructing
/// one job to finalize will force ALL jobs in the transaction to
/// finalize, so it is only necessary to instruct a single member job to
/// finalize.
///
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

#[qapi(command)]
#[qapi(name = "block-job-pause")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.3.0")]
#[qapi(deprecated(added = "10.1.0"))]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockJobPause {
    pub device: String,
}

#[qapi(command)]
#[qapi(name = "block-job-resume")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.3.0")]
#[qapi(deprecated(added = "10.1.0"))]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockJobResume {
    pub device: String,
}

#[qapi(command)]
#[qapi(name = "block-job-set-speed")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockJobSetSpeed {
    pub device: String,
    pub speed: i64,
}

#[qapi(command)]
#[qapi(name = "block-latency-histogram-set")]
#[qapi(returns = "()")]
#[qapi(added = "4.0.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockLatencyHistogramSet {
    pub boundaries: Option<Vec<u64>>,
    #[qapi(name = "boundaries-flush")]
    pub boundaries_flush: Option<Vec<u64>>,
    #[qapi(name = "boundaries-read")]
    pub boundaries_read: Option<Vec<u64>>,
    #[qapi(name = "boundaries-write")]
    pub boundaries_write: Option<Vec<u64>>,
    #[qapi(name = "boundaries-zap")]
    #[qapi(added = "8.1.0")]
    pub boundaries_zap: Option<Vec<u64>>,
    pub id: String,
}

#[qapi(command)]
#[qapi(name = "block_passwd")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
#[qapi(removed = "6.0.0")]
pub struct BlockPasswd {
    pub device: Option<String>,
    #[qapi(name = "node-name")]
    #[qapi(added = "2.0.0")]
    pub node_name: Option<String>,
    pub password: String,
}

#[qapi(command)]
#[qapi(name = "block_resize")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockResize {
    pub device: Option<String>,
    #[qapi(name = "node-name")]
    #[qapi(added = "2.0.0")]
    pub node_name: Option<String>,
    pub size: i64,
}

#[qapi(command)]
#[qapi(name = "block_set_io_throttle")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockSetIoThrottle {
    /// total throughput limit in bytes per second
    pub bps: i64,
    /// total throughput limit during bursts,
    /// in bytes (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub bps_max: Option<i64>,
    /// maximum length of the @bps_max burst
    /// period, in seconds. It must only
    /// be set if @bps_max is set as well.
    /// Defaults to 1. (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub bps_max_length: Option<i64>,
    /// read throughput limit in bytes per second
    pub bps_rd: i64,
    /// read throughput limit during bursts,
    /// in bytes (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub bps_rd_max: Option<i64>,
    /// maximum length of the @bps_rd_max
    /// burst period, in seconds. It must only
    /// be set if @bps_rd_max is set as well.
    /// Defaults to 1. (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub bps_rd_max_length: Option<i64>,
    /// write throughput limit in bytes per second
    pub bps_wr: i64,
    /// write throughput limit during bursts,
    /// in bytes (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub bps_wr_max: Option<i64>,
    /// maximum length of the @bps_wr_max
    /// burst period, in seconds. It must only
    /// be set if @bps_wr_max is set as well.
    /// Defaults to 1. (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub bps_wr_max_length: Option<i64>,
    /// Block device name (deprecated, use @id instead)
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
    /// total I/O operations per second during bursts,
    /// in bytes (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub iops_max: Option<i64>,
    /// maximum length of the @iops burst
    /// period, in seconds. It must only
    /// be set if @iops_max is set as well.
    /// Defaults to 1. (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub iops_max_length: Option<i64>,
    /// read I/O operations per second
    pub iops_rd: i64,
    /// read I/O operations per second during bursts,
    /// in bytes (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub iops_rd_max: Option<i64>,
    /// maximum length of the @iops_rd_max
    /// burst period, in seconds. It must only
    /// be set if @iops_rd_max is set as well.
    /// Defaults to 1. (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub iops_rd_max_length: Option<i64>,
    /// an I/O size in bytes (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub iops_size: Option<i64>,
    /// write I/O operations per second
    pub iops_wr: i64,
    /// write I/O operations per second during bursts,
    /// in bytes (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub iops_wr_max: Option<i64>,
    /// maximum length of the @iops_wr_max
    /// burst period, in seconds. It must only
    /// be set if @iops_wr_max is set as well.
    /// Defaults to 1. (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub iops_wr_max_length: Option<i64>,
}

/// Change the write threshold for a block drive. An event will be delivered
/// if a write to this block drive crosses the configured threshold.
/// This is useful to transparently resize thin-provisioned drives without
/// the guest OS noticing.
///
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
/// The block streaming operation is performed in the background until the entire
/// backing file has been copied.  This command returns immediately once streaming
/// has started.  The status of ongoing block streaming operations can be checked
/// with query-block-jobs.  The operation can be stopped before it has completed
/// using the block-job-cancel command.
///
/// The node that receives the data is called the top image, can be located in
/// any part of the chain (but always above the base image; see below) and can be
/// specified using its device or node name. Earlier qemu versions only allowed
/// 'device' to name the top level node; presence of the 'base-node' parameter
/// during introspection can be used as a witness of the enhanced semantics
/// of 'device'.
///
/// If a base file is specified then sectors are not copied from that base file and
/// its backing chain.  When streaming completes the image file will have the base
/// file as its backing file.  This can be used to stream a subset of the backing
/// file chain instead of flattening the entire image.
///
/// On successful completion the image file is updated to drop the backing file
/// and the BLOCK_JOB_COMPLETED event is emitted.
///
#[qapi(command)]
#[qapi(name = "block-stream")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockStream {
    #[qapi(name = "auto-dismiss")]
    #[qapi(added = "3.1.0")]
    pub auto_dismiss: Option<bool>,
    #[qapi(name = "auto-finalize")]
    #[qapi(added = "3.1.0")]
    pub auto_finalize: Option<bool>,
    /// optional The backing file string to write into the top
    /// image. This filename is not validated.
    ///
    /// If a pathname string is such that it cannot be
    /// resolved by QEMU, that means that subsequent QMP or
    /// HMP commands must use node-names for the image in
    /// question, as filename lookup methods will fail.
    ///
    /// If not specified, QEMU will automatically determine
    /// the backing file string to use, or error out if there
    /// is no obvious choice.  Care should be taken when
    /// specifying the string, to specify a valid filename or
    /// protocol.
    /// (Since 2.1)
    #[qapi(name = "backing-file")]
    #[qapi(added = "2.1.0")]
    pub backing_file: Option<String>,
    #[qapi(name = "backing-mask-protocol")]
    #[qapi(added = "9.0.0")]
    pub backing_mask_protocol: Option<bool>,
    /// optional the common backing file name.
    /// It cannot be set if @base-node is also set.
    pub base: Option<String>,
    /// optional the node name of the backing file.
    /// It cannot be set if @base is also set. (Since 2.8)
    #[qapi(name = "base-node")]
    #[qapi(added = "2.8.0")]
    pub base_node: Option<String>,
    #[qapi(added = "6.0.0")]
    pub bottom: Option<String>,
    /// the device or node name of the top image
    pub device: String,
    #[qapi(name = "filter-node-name")]
    #[qapi(added = "6.0.0")]
    pub filter_node_name: Option<String>,
    /// optional identifier for the newly-created block job. If
    /// omitted, the device name will be used. (Since 2.7)
    #[qapi(name = "job-id")]
    #[qapi(added = "2.7.0")]
    pub job_id: Option<String>,
    /// optional the action to take on an error (default report).
    /// 'stop' and 'enospc' can only be used if the block device
    /// supports io-status (see BlockInfo).  Since 1.3.
    #[qapi(name = "on-error")]
    #[qapi(added = "1.3.0")]
    pub on_error: Option<BlockdevOnError>,
    /// optional the maximum speed, in bytes per second
    pub speed: Option<i64>,
}

#[qapi(command)]
#[qapi(name = "calc-dirty-rate")]
#[qapi(returns = "()")]
#[qapi(added = "5.2.0")]
pub struct CalcDirtyRate {
    #[qapi(name = "calc-time")]
    pub calc_time: i64,
    #[qapi(name = "calc-time-unit")]
    #[qapi(added = "8.2.0")]
    pub calc_time_unit: Option<TimeUnit>,
    #[qapi(added = "6.2.0")]
    pub mode: Option<DirtyRateMeasureMode>,
    #[qapi(name = "sample-pages")]
    #[qapi(added = "6.1.0")]
    pub sample_pages: Option<i64>,
}

#[qapi(command)]
#[qapi(name = "cancel-vcpu-dirty-limit")]
#[qapi(returns = "()")]
#[qapi(added = "7.1.0")]
pub struct CancelVcpuDirtyLimit {
    #[qapi(name = "cpu-index")]
    pub cpu_index: Option<i64>,
}

#[qapi(command)]
#[qapi(name = "change")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.1.0")]
#[qapi(deprecated(added = "5.0.0"))]
#[qapi(removed = "6.0.0")]
pub struct Change {
    pub arg: Option<String>,
    pub device: String,
    pub target: String,
}

/// Change the backing file in the image file metadata.  This does not
/// cause QEMU to reopen the image file to reparse the backing filename
/// (it may, however, perform a reopen to change permissions from
/// r/o -> r/w -> r/o, if needed). The new backing file string is written
/// into the image file metadata, and the QEMU internal strings are
/// updated.
///
#[qapi(command)]
#[qapi(name = "change-backing-file")]
#[qapi(returns = "()")]
#[qapi(added = "2.1.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct ChangeBackingFile {
    /// The string to write as the backing file.  This
    /// string is not validated, so care should be taken
    /// when specifying the string or the image chain may
    /// not be able to be reopened again.
    #[qapi(name = "backing-file")]
    pub backing_file: String,
    /// The device name or node-name of the root node that owns
    /// image-node-name.
    pub device: String,
    /// The name of the block driver state node of the
    /// image to modify.
    #[qapi(name = "image-node-name")]
    pub image_node_name: String,
}

#[qapi(command)]
#[qapi(name = "change-vnc-password")]
#[qapi(returns = "()")]
#[qapi(condition = "defined(CONFIG_VNC)")]
#[qapi(added = "1.1.0")]
pub struct ChangeVncPassword {
    pub password: String,
}

#[qapi(command)]
#[qapi(name = "chardev-add")]
#[qapi(returns = "ChardevReturn")]
#[qapi(added = "1.4.0")]
pub struct ChardevAdd {
    pub backend: ChardevBackend,
    pub id: String,
}

#[qapi(command)]
#[qapi(name = "chardev-change")]
#[qapi(returns = "ChardevReturn")]
#[qapi(added = "2.10.0")]
pub struct ChardevChange {
    pub backend: ChardevBackend,
    pub id: String,
}

#[qapi(command)]
#[qapi(name = "chardev-remove")]
#[qapi(returns = "()")]
#[qapi(added = "1.4.0")]
pub struct ChardevRemove {
    pub id: String,
}

#[qapi(command)]
#[qapi(name = "chardev-send-break")]
#[qapi(returns = "()")]
#[qapi(added = "2.10.0")]
pub struct ChardevSendBreak {
    pub id: String,
}

/// Set migration information for remote display.  This makes the server
/// ask the client to automatically reconnect using the new parameters
/// once migration finished successfully.  Only implemented for SPICE.
///
#[qapi(command)]
#[qapi(name = "client_migrate_info")]
#[qapi(returns = "()")]
#[qapi(added = "2.4.0")]
pub struct ClientMigrateInfo {
    /// optional server certificate subject
    #[qapi(name = "cert-subject")]
    pub cert_subject: Option<String>,
    /// migration target hostname
    pub hostname: String,
    /// optional spice tcp port for plaintext channels
    pub port: Option<i64>,
    /// must be "spice"
    pub protocol: String,
    /// optional spice tcp port for tls-secured channels
    #[qapi(name = "tls-port")]
    pub tls_port: Option<i64>,
}

#[qapi(command)]
#[qapi(name = "closefd")]
#[qapi(returns = "()")]
#[qapi(added = "1.2.0")]
pub struct Closefd {
    pub fdname: String,
}

#[qapi(command)]
#[qapi(name = "cont")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct Cont {}

#[qapi(command)]
#[qapi(name = "cpu")]
#[qapi(returns = "()")]
#[qapi(added = "1.0.0")]
#[qapi(removed = "2.12.0")]
pub struct Cpu {
    pub index: i64,
}

#[qapi(command)]
#[qapi(name = "cpu-add")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.5.0")]
#[qapi(deprecated(added = "5.0.0"))]
#[qapi(removed = "5.2.0")]
pub struct CpuAdd {
    pub id: i64,
}

#[qapi(command)]
#[qapi(name = "cxl-add-dynamic-capacity")]
#[qapi(feature = "unstable")]
#[qapi(returns = "()")]
#[qapi(added = "9.1.0")]
#[qapi(unstable(added = "9.1.0"))]
pub struct CxlAddDynamicCapacity {
    pub extents: Vec<CxlDynamicCapacityExtent>,
    #[qapi(name = "host-id")]
    pub host_id: u16,
    pub path: String,
    pub region: u8,
    #[qapi(name = "selection-policy")]
    pub selection_policy: CxlExtentSelectionPolicy,
    pub tag: Option<String>,
}

/// Command to inject a single correctable error.  Multiple error
/// injection of this error type is not interesting as there is no
/// associated header log.  These errors are reported via AER as a
/// correctable internal error, with additional detail available from
/// the CXL device.
///
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
///
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
    pub flags: u32,
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
///
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
    pub flags: u32,
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
///
#[qapi(command)]
#[qapi(name = "cxl-inject-memory-module-event")]
#[qapi(returns = "()")]
#[qapi(added = "8.1.0")]
pub struct CxlInjectMemoryModuleEvent {
    #[qapi(name = "additional-status")]
    pub additional_status: u8,
    #[qapi(name = "component-id")]
    #[qapi(added = "11.0.0")]
    pub component_id: Option<String>,
    #[qapi(name = "corrected-persistent-error-count")]
    pub corrected_persistent_error_count: u32,
    #[qapi(name = "corrected-volatile-error-count")]
    pub corrected_volatile_error_count: u32,
    #[qapi(name = "dirty-shutdown-count")]
    pub dirty_shutdown_count: u32,
    /// Event Record Flags.  See CXL r3.2 Table 8-55 Common Event
    /// Record Format, Event Record Flags for subfield definitions.
    pub flags: u32,
    /// ID of the device head from where the event originated.
    #[qapi(name = "head-id")]
    #[qapi(added = "11.0.0")]
    pub head_id: Option<u8>,
    #[qapi(name = "health-status")]
    pub health_status: u8,
    #[qapi(name = "is-comp-id-pldm")]
    #[qapi(added = "11.0.0")]
    pub is_comp_id_pldm: Option<bool>,
    /// Logical Device (LD) ID of LD from where the event
    /// originated.
    #[qapi(name = "ld-id")]
    #[qapi(added = "11.0.0")]
    pub ld_id: Option<u16>,
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
    #[qapi(name = "media-status")]
    pub media_status: u8,
    /// CXL type 3 device canonical QOM path
    pub path: String,
    #[qapi(name = "sub-type")]
    #[qapi(added = "11.0.0")]
    pub sub_type: Option<u8>,
    pub temperature: i16,
    pub r#type: u8,
}

/// Poison records indicate that a CXL memory device knows that a
/// particular memory region may be corrupted.  This may be because of
/// locally detected errors (e.g. ECC failure) or poisoned writes
/// received from other components in the system.  This injection
/// mechanism enables testing of the OS handling of poison records which
/// may be queried via the CXL mailbox.
///
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
///
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

#[qapi(command)]
#[qapi(name = "cxl-release-dynamic-capacity")]
#[qapi(feature = "unstable")]
#[qapi(returns = "()")]
#[qapi(added = "9.1.0")]
#[qapi(unstable(added = "9.1.0"))]
pub struct CxlReleaseDynamicCapacity {
    pub extents: Vec<CxlDynamicCapacityExtent>,
    #[qapi(name = "forced-removal")]
    pub forced_removal: Option<bool>,
    #[qapi(name = "host-id")]
    pub host_id: u16,
    pub path: String,
    pub region: u8,
    #[qapi(name = "removal-policy")]
    pub removal_policy: CxlExtentRemovalPolicy,
    #[qapi(name = "sanitize-on-release")]
    pub sanitize_on_release: Option<bool>,
    pub tag: Option<String>,
}

#[qapi(command)]
#[qapi(name = "device_add")]
#[qapi(returns = "()")]
#[qapi(feature = "json-cli")]
#[qapi(feature = "json-cli-hotplug")]
#[qapi(added = "2.8.0")]
pub struct DeviceAdd {
    /// Additional device-specific properties.
    #[qapi(flatten)]
    pub arguments: Option<HashMap<String, serde_json::Value>>,
    #[qapi(added = "2.9.0")]
    pub bus: Option<String>,
    pub driver: String,
    pub id: Option<String>,
}

#[qapi(command)]
#[qapi(name = "device_del")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct DeviceDel {
    pub id: String,
}

#[qapi(command)]
#[qapi(name = "device-list-properties")]
#[qapi(returns = "Vec<DevicePropertyInfo>")]
#[qapi(added = "1.2.0")]
pub struct DeviceListProperties {
    pub typename: String,
}

#[qapi(command)]
#[qapi(name = "device-sync-config")]
#[qapi(feature = "unstable")]
#[qapi(returns = "()")]
#[qapi(added = "9.2.0")]
#[qapi(unstable(added = "9.2.0"))]
pub struct DeviceSyncConfig {
    pub id: String,
}

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

/// A set of parameters describing drive mirror setup.
///
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

#[qapi(command)]
#[qapi(name = "dumpdtb")]
#[qapi(condition = "CONFIG_FDT")]
#[qapi(returns = "()")]
#[qapi(added = "7.2.0")]
pub struct Dumpdtb {
    pub filename: String,
}

#[qapi(command)]
#[qapi(name = "dump-guest-memory")]
#[qapi(returns = "()")]
#[qapi(added = "1.2.0")]
pub struct DumpGuestMemory {
    pub begin: Option<i64>,
    #[qapi(added = "2.6.0")]
    pub detach: Option<bool>,
    #[qapi(added = "2.0.0")]
    pub format: Option<DumpGuestMemoryFormat>,
    pub length: Option<i64>,
    pub paging: bool,
    pub protocol: String,
}

/// Dump guest's storage keys
///
#[qapi(command)]
#[qapi(name = "dump-skeys")]
#[qapi(returns = "()")]
#[qapi(condition = "defined(TARGET_S390X)")]
#[qapi(added = "2.5.0")]
pub struct DumpSkeys {
    /// the path to the file to dump to
    ///
    /// This command is only supported on s390 architecture.
    pub filename: String,
}

#[qapi(command)]
#[qapi(name = "eject")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct Eject {
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "5.0.0"))]
    pub device: Option<String>,
    pub force: Option<bool>,
    #[qapi(added = "2.8.0")]
    pub id: Option<String>,
}

#[qapi(command)]
#[qapi(name = "expire_password")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct ExpirePassword {
    /// - 'vnc' to modify the VNC server expiration
    /// - 'spice' to modify the Spice server expiration
    #[qapi(discriminator)]
    pub protocol: DisplayProtocol,
    /// when to expire the password.
    ///
    /// - 'now' to expire the password immediately
    /// - 'never' to cancel password expiration
    /// - '+INT' where INT is the number of seconds from now (integer)
    /// - 'INT' where INT is the absolute time in seconds
    ///
    /// .. note:: Time is relative to the server and currently there is no
    /// way to coordinate server time with client time.  It is not
    /// recommended to use the absolute time version of the @time
    /// parameter unless you're sure you are on the same machine as the
    /// QEMU instance.
    pub time: String,
    #[qapi(union)]
    #[qapi(flatten)]
    #[qapi(added = "7.0.0")]
    pub u: Option<ExpirePasswordOptionsBranch>,
}

#[qapi(command)]
#[qapi(name = "getfd")]
#[qapi(returns = "()")]
#[qapi(condition = "CONFIG_POSIX")]
#[qapi(added = "1.2.0")]
pub struct Getfd {
    pub fdname: String,
}

#[qapi(command)]
#[qapi(name = "get-win32-socket")]
#[qapi(condition = "CONFIG_WIN32")]
#[qapi(returns = "()")]
#[qapi(added = "8.0.0")]
pub struct GetWin32Socket {
    pub fdname: String,
    pub info: String,
}

#[qapi(command)]
#[qapi(name = "human-monitor-command")]
#[qapi(returns = "String")]
#[qapi(feature = "savevm-monitor-nodes")]
#[qapi(added = "1.1.0")]
pub struct HumanMonitorCommand {
    #[qapi(name = "command-line")]
    pub command_line: String,
    #[qapi(name = "cpu-index")]
    pub cpu_index: Option<i64>,
}

#[qapi(command)]
#[qapi(name = "inject-ghes-v2-error")]
#[qapi(feature = "unstable")]
#[qapi(returns = "()")]
#[qapi(added = "10.2.0")]
#[qapi(unstable(added = "10.2.0"))]
pub struct InjectGhesV2Error {
    pub cper: String,
}

#[qapi(command)]
#[qapi(name = "inject-nmi")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct InjectNmi {}

#[qapi(command)]
#[qapi(name = "input-send-event")]
#[qapi(returns = "()")]
#[qapi(added = "2.6.0")]
pub struct InputSendEvent {
    pub device: Option<String>,
    pub events: Vec<InputEvent>,
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
///
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
///
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
///
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
///
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
///
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
///
#[qapi(command)]
#[qapi(name = "job-resume")]
#[qapi(returns = "()")]
#[qapi(added = "3.0.0")]
pub struct JobResume {
    /// The job identifier.
    pub id: String,
}

#[qapi(command)]
#[qapi(name = "memsave")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct Memsave {
    #[qapi(name = "cpu-index")]
    pub cpu_index: Option<i64>,
    pub filename: String,
    pub size: u64,
    pub val: u64,
}

#[qapi(command)]
#[qapi(name = "migrate")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct Migrate {
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub blk: Option<bool>,
    #[qapi(added = "8.2.0")]
    pub channels: Option<Vec<MigrationChannel>>,
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "10.1.0"))]
    #[qapi(removed = "11.0.0")]
    pub detach: Option<bool>,
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub inc: Option<bool>,
    #[qapi(added = "3.0.0")]
    pub resume: Option<bool>,
    pub uri: Option<String>,
}

#[qapi(command)]
#[qapi(name = "migrate_cancel")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct MigrateCancel {}

#[qapi(command)]
#[qapi(name = "migrate-continue")]
#[qapi(returns = "()")]
#[qapi(added = "2.11.0")]
pub struct MigrateContinue {
    pub state: MigrationStatus,
}

#[qapi(command)]
#[qapi(name = "migrate-incoming")]
#[qapi(returns = "()")]
#[qapi(added = "2.3.0")]
pub struct MigrateIncoming {
    #[qapi(added = "8.2.0")]
    pub channels: Option<Vec<MigrationChannel>>,
    #[qapi(name = "exit-on-error")]
    #[qapi(added = "9.1.0")]
    pub exit_on_error: Option<bool>,
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
///
#[qapi(command)]
#[qapi(name = "migrate-pause")]
#[qapi(returns = "()")]
#[qapi(added = "3.0.0")]
#[qapi(allow_oob)]
pub struct MigratePause {}

/// Provide a recovery migration stream URI.
///
#[qapi(command)]
#[qapi(name = "migrate-recover")]
#[qapi(returns = "()")]
#[qapi(added = "3.0.0")]
#[qapi(allow_oob)]
pub struct MigrateRecover {
    /// the URI to be used for the recovery of migration stream.
    ///
    /// .. qmp-example::
    ///
    /// -> { "execute": "migrate-recover",
    /// "arguments": { "uri": "tcp:192.168.1.200:12345" } }
    /// <- { "return": {} }
    pub uri: String,
}

#[qapi(command)]
#[qapi(name = "migrate-set-cache-size")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.2.0")]
#[qapi(deprecated(added = "5.0.0"))]
#[qapi(removed = "6.0.0")]
pub struct MigrateSetCacheSize {
    pub value: i64,
}

/// Enable/Disable the following migration capabilities (like xbzrle)
///
#[qapi(command)]
#[qapi(name = "migrate-set-capabilities")]
#[qapi(returns = "()")]
#[qapi(added = "1.2.0")]
pub struct MigrateSetCapabilities {
    /// json array of capability modifications to make
    pub capabilities: Vec<MigrationCapabilityStatus>,
}

#[qapi(command)]
#[qapi(name = "migrate_set_downtime")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.1.0")]
#[qapi(deprecated(added = "5.0.0"))]
#[qapi(removed = "6.0.0")]
pub struct MigrateSetDowntime {
    pub value: f64,
}

/// Set various migration parameters.  See MigrationParameters for details.
///
#[qapi(command)]
#[qapi(name = "migrate-set-parameters")]
#[qapi(returns = "()")]
#[qapi(added = "2.4.0")]
pub struct MigrateSetParameters {
    #[qapi(name = "announce-initial")]
    #[qapi(added = "4.0.0")]
    pub announce_initial: Option<u64>,
    #[qapi(name = "announce-max")]
    #[qapi(added = "4.0.0")]
    pub announce_max: Option<u64>,
    #[qapi(name = "announce-rounds")]
    #[qapi(added = "4.0.0")]
    pub announce_rounds: Option<u64>,
    #[qapi(name = "announce-step")]
    #[qapi(added = "4.0.0")]
    pub announce_step: Option<u64>,
    #[qapi(name = "avail-switchover-bandwidth")]
    #[qapi(added = "8.2.0")]
    pub avail_switchover_bandwidth: Option<u64>,
    #[qapi(name = "block-bitmap-mapping")]
    #[qapi(added = "5.2.0")]
    pub block_bitmap_mapping: Option<Vec<BitmapMigrationNodeAlias>>,
    #[qapi(name = "block-incremental")]
    #[qapi(feature = "deprecated")]
    #[qapi(added = "2.10.0")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub block_incremental: Option<bool>,
    #[qapi(name = "compress-level")]
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub compress_level: Option<u8>,
    #[qapi(name = "compress-threads")]
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub compress_threads: Option<u8>,
    #[qapi(name = "compress-wait-thread")]
    #[qapi(feature = "deprecated")]
    #[qapi(added = "3.1.0")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub compress_wait_thread: Option<bool>,
    #[qapi(name = "cpr-exec-command")]
    #[qapi(added = "10.2.0")]
    pub cpr_exec_command: Option<Vec<String>>,
    #[qapi(name = "cpu-throttle-increment")]
    #[qapi(added = "2.7.0")]
    pub cpu_throttle_increment: Option<u8>,
    #[qapi(name = "cpu-throttle-initial")]
    #[qapi(added = "2.7.0")]
    pub cpu_throttle_initial: Option<u8>,
    #[qapi(name = "cpu-throttle-tailslow")]
    #[qapi(added = "5.1.0")]
    pub cpu_throttle_tailslow: Option<bool>,
    #[qapi(name = "decompress-threads")]
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub decompress_threads: Option<u8>,
    #[qapi(name = "direct-io")]
    #[qapi(added = "9.1.0")]
    pub direct_io: Option<bool>,
    #[qapi(name = "downtime-limit")]
    #[qapi(added = "2.8.0")]
    pub downtime_limit: Option<u64>,
    #[qapi(name = "max-bandwidth")]
    #[qapi(added = "2.8.0")]
    pub max_bandwidth: Option<u64>,
    #[qapi(name = "max-cpu-throttle")]
    #[qapi(added = "3.1.0")]
    pub max_cpu_throttle: Option<u8>,
    #[qapi(name = "max-postcopy-bandwidth")]
    #[qapi(added = "3.0.0")]
    pub max_postcopy_bandwidth: Option<u64>,
    #[qapi(added = "8.2.0")]
    pub mode: Option<MigMode>,
    #[qapi(name = "multifd-channels")]
    #[qapi(added = "4.0.0")]
    pub multifd_channels: Option<u8>,
    #[qapi(name = "multifd-compression")]
    #[qapi(added = "5.0.0")]
    pub multifd_compression: Option<MultiFdCompression>,
    #[qapi(name = "multifd-qatzip-level")]
    #[qapi(added = "9.2.0")]
    pub multifd_qatzip_level: Option<u8>,
    #[qapi(name = "multifd-zlib-level")]
    #[qapi(added = "5.0.0")]
    pub multifd_zlib_level: Option<u8>,
    #[qapi(name = "multifd-zstd-level")]
    #[qapi(added = "5.0.0")]
    pub multifd_zstd_level: Option<u8>,
    #[qapi(name = "throttle-trigger-threshold")]
    #[qapi(added = "5.0.0")]
    pub throttle_trigger_threshold: Option<u8>,
    #[qapi(name = "tls-authz")]
    #[qapi(added = "4.0.0")]
    pub tls_authz: Option<StrOrNull>,
    #[qapi(name = "tls-creds")]
    #[qapi(added = "2.7.0")]
    pub tls_creds: Option<StrOrNull>,
    #[qapi(name = "tls-hostname")]
    #[qapi(added = "2.7.0")]
    pub tls_hostname: Option<StrOrNull>,
    #[qapi(name = "vcpu-dirty-limit")]
    #[qapi(added = "8.1.0")]
    pub vcpu_dirty_limit: Option<u64>,
    #[qapi(name = "x-checkpoint-delay")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "2.8.0")]
    pub x_checkpoint_delay: Option<u32>,
    #[qapi(name = "x-cpu-throttle-increment")]
    #[qapi(added = "2.5.0")]
    #[qapi(removed = "2.7.0")]
    pub x_cpu_throttle_increment: Option<i64>,
    #[qapi(name = "x-cpu-throttle-initial")]
    #[qapi(added = "2.5.0")]
    #[qapi(removed = "2.7.0")]
    pub x_cpu_throttle_initial: Option<i64>,
    #[qapi(name = "x-multifd-channels")]
    #[qapi(added = "2.11.0")]
    #[qapi(removed = "4.0.0")]
    pub x_multifd_channels: Option<i64>,
    #[qapi(name = "x-multifd-page-count")]
    #[qapi(added = "2.11.0")]
    #[qapi(removed = "4.0.0")]
    pub x_multifd_page_count: Option<i64>,
    #[qapi(name = "x-rdma-chunk-size")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "11.1.0")]
    pub x_rdma_chunk_size: Option<u64>,
    #[qapi(name = "x-vcpu-dirty-limit-period")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "8.1.0")]
    pub x_vcpu_dirty_limit_period: Option<u64>,
    #[qapi(name = "xbzrle-cache-size")]
    #[qapi(added = "2.11.0")]
    pub xbzrle_cache_size: Option<u64>,
    #[qapi(name = "zero-page-detection")]
    #[qapi(added = "9.0.0")]
    pub zero_page_detection: Option<ZeroPageDetection>,
}

#[qapi(command)]
#[qapi(name = "migrate_set_speed")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.1.0")]
#[qapi(deprecated(added = "5.0.0"))]
#[qapi(removed = "6.0.0")]
pub struct MigrateSetSpeed {
    pub value: i64,
}

/// Followup to a migration command to switch the migration to postcopy mode.
/// The postcopy-ram capability must be set before the original migration
/// command.
///
#[qapi(command)]
#[qapi(name = "migrate-start-postcopy")]
#[qapi(returns = "()")]
#[qapi(added = "2.5.0")]
pub struct MigrateStartPostcopy {}

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

#[qapi(command)]
#[qapi(name = "nbd-server-remove")]
#[qapi(returns = "()")]
#[qapi(feature = "deprecated")]
#[qapi(added = "2.12.0")]
#[qapi(deprecated(added = "5.2.0"))]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct NbdServerRemove {
    pub mode: Option<BlockExportRemoveMode>,
    pub name: String,
}

#[qapi(command)]
#[qapi(name = "nbd-server-start")]
#[qapi(returns = "()")]
#[qapi(added = "1.3.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct NbdServerStart {
    pub addr: SocketAddressLegacy,
    #[qapi(name = "handshake-max-seconds")]
    #[qapi(added = "10.0.0")]
    pub handshake_max_seconds: Option<u32>,
    #[qapi(name = "max-connections")]
    #[qapi(added = "5.2.0")]
    pub max_connections: Option<u32>,
    #[qapi(name = "tls-authz")]
    #[qapi(added = "4.0.0")]
    pub tls_authz: Option<String>,
    #[qapi(name = "tls-creds")]
    #[qapi(added = "2.6.0")]
    pub tls_creds: Option<String>,
}

/// Stop QEMU's embedded NBD server, and unregister all devices
/// previously added via `nbd-server-add`.
///
#[qapi(command)]
#[qapi(name = "nbd-server-stop")]
#[qapi(returns = "()")]
#[qapi(added = "1.3.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct NbdServerStop {}

#[qapi(command)]
#[qapi(name = "netdev_add")]
#[qapi(returns = "()")]
#[qapi(added = "1.2.0")]
#[qapi(allow_preconfig(added = "6.1.0"))]
pub struct NetdevAdd {
    /// Additional device-specific properties.
    #[qapi(flatten)]
    #[qapi(removed = "5.0.0")]
    pub arguments: Option<HashMap<String, serde_json::Value>>,
    /// identifier for monitor commands.
    pub id: String,
    #[qapi(removed = "2.5.0")]
    pub props: Option<serde_json::Value>,
    /// Specify the driver used for interpreting remaining arguments.
    #[qapi(discriminator)]
    pub r#type: NetClientDriver,
    #[qapi(union)]
    #[qapi(flatten)]
    #[qapi(added = "5.0.0")]
    pub u: Option<NetdevBranch>,
}

#[qapi(command)]
#[qapi(name = "netdev_del")]
#[qapi(returns = "()")]
#[qapi(added = "1.2.0")]
#[qapi(allow_preconfig(added = "6.1.0"))]
pub struct NetdevDel {
    pub id: String,
}

#[qapi(command)]
#[qapi(name = "object-add")]
#[qapi(returns = "()")]
#[qapi(added = "2.0.0")]
#[qapi(allow_preconfig(added = "6.1.0"))]
pub struct ObjectAdd {
    /// Additional device-specific properties.
    #[qapi(flatten)]
    #[qapi(removed = "6.0.0")]
    pub arguments: Option<HashMap<String, serde_json::Value>>,
    /// the name of the new object
    pub id: String,
    #[qapi(removed = "6.0.0")]
    pub props: Option<serde_json::Value>,
    /// the class name for the object to be created
    #[qapi(name = "qom-type")]
    #[qapi(discriminator)]
    pub qom_type: ObjectType,
    #[qapi(union)]
    #[qapi(flatten)]
    #[qapi(added = "6.0.0")]
    pub u: Option<ObjectOptionsBranch>,
}

#[qapi(command)]
#[qapi(name = "object-del")]
#[qapi(returns = "()")]
#[qapi(added = "2.0.0")]
#[qapi(allow_preconfig(added = "6.1.0"))]
pub struct ObjectDel {
    pub id: String,
}

#[qapi(command)]
#[qapi(name = "pmemsave")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct Pmemsave {
    pub filename: String,
    pub size: u64,
    pub val: u64,
}

/// Enable QMP capabilities.
///
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
    ///
    /// .. qmp-example::
    ///
    /// -> { "execute": "qmp_capabilities",
    /// "arguments": { "enable": [ "oob" ] } }
    /// <- { "return": {} }
    ///
    /// .. note:: This command is valid exactly when first connecting: it
    /// must be issued before any other command will be accepted, and
    /// will fail once the monitor is accepting other commands.  (see
    /// :doc:`/interop/qmp-spec`)
    ///
    /// .. note:: The QMP client needs to explicitly enable QMP
    /// capabilities, otherwise all the QMP capabilities will be turned
    /// off by default.
    #[qapi(added = "2.12.0")]
    pub enable: Option<Vec<QmpCapability>>,
}

#[qapi(command)]
#[qapi(name = "qom-get")]
#[qapi(returns = "serde_json::Value")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QomGet {
    /// Additional device-specific properties.
    #[qapi(flatten)]
    #[qapi(removed = "2.5.0")]
    pub arguments: Option<HashMap<String, serde_json::Value>>,
    pub path: String,
    pub property: String,
}

#[qapi(command)]
#[qapi(name = "qom-list")]
#[qapi(returns = "Vec<ObjectPropertyInfo>")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QomList {
    pub path: String,
}

#[qapi(command)]
#[qapi(name = "qom-list-get")]
#[qapi(returns = "Vec<ObjectPropertiesValues>")]
#[qapi(added = "10.1.0")]
#[qapi(allow_preconfig)]
pub struct QomListGet {
    pub paths: Vec<String>,
}

#[qapi(command)]
#[qapi(name = "qom-list-properties")]
#[qapi(returns = "Vec<ObjectPropertyInfo>")]
#[qapi(added = "2.12.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QomListProperties {
    pub typename: String,
}

#[qapi(command)]
#[qapi(name = "qom-list-types")]
#[qapi(returns = "Vec<ObjectTypeInfo>")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QomListTypes {
    pub r#abstract: Option<bool>,
    pub implements: Option<String>,
}

/// This command will set a property from a object model path.
///
#[qapi(command)]
#[qapi(name = "qom-set")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QomSet {
    /// Additional device-specific properties.
    #[qapi(flatten)]
    #[qapi(removed = "2.5.0")]
    pub arguments: Option<HashMap<String, serde_json::Value>>,
    /// see @qom-get for a description of this parameter
    pub path: String,
    /// the property name to set
    pub property: String,
    /// a value who's type is appropriate for the property type.  See @qom-get
    /// for a description of type mapping.
    pub value: serde_json::Value,
}

#[qapi(command)]
#[qapi(name = "query-accelerators")]
#[qapi(returns = "AcceleratorInfo")]
#[qapi(added = "10.2.0")]
pub struct QueryAccelerators {}

/// Lists ACPI OSPM status of ACPI device objects,
/// which might be reported via _OST method
///
#[qapi(command)]
#[qapi(name = "query-acpi-ospm-status")]
#[qapi(returns = "Vec<AcpiostInfo>")]
#[qapi(added = "2.1.0")]
pub struct QueryAcpiOspmStatus {}

/// Return information about audiodev configuration
///
#[qapi(command)]
#[qapi(name = "query-audiodevs")]
#[qapi(returns = "Vec<Audiodev>")]
#[qapi(added = "8.0.0")]
pub struct QueryAudiodevs {}

#[qapi(command)]
#[qapi(name = "query-balloon")]
#[qapi(returns = "BalloonInfo")]
#[qapi(added = "1.0.0")]
pub struct QueryBalloon {}

#[qapi(command)]
#[qapi(name = "query-block")]
#[qapi(returns = "Vec<BlockInfo>")]
#[qapi(added = "1.0.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct QueryBlock {
    #[qapi(added = "11.0.0")]
    pub flat: Option<bool>,
}

#[qapi(command)]
#[qapi(name = "query-block-exports")]
#[qapi(returns = "Vec<BlockExportInfo>")]
#[qapi(added = "5.2.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct QueryBlockExports {}

#[qapi(command)]
#[qapi(name = "query-block-jobs")]
#[qapi(returns = "Vec<BlockJobInfo>")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct QueryBlockJobs {}

#[qapi(command)]
#[qapi(name = "query-blockstats")]
#[qapi(returns = "Vec<BlockStats>")]
#[qapi(added = "1.0.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct QueryBlockstats {
    #[qapi(name = "query-nodes")]
    #[qapi(added = "2.3.0")]
    pub query_nodes: Option<bool>,
}

#[qapi(command)]
#[qapi(name = "query-chardev")]
#[qapi(returns = "Vec<ChardevInfo>")]
#[qapi(added = "1.0.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryChardev {}

#[qapi(command)]
#[qapi(name = "query-chardev-backends")]
#[qapi(returns = "Vec<ChardevBackendInfo>")]
#[qapi(added = "2.0.0")]
pub struct QueryChardevBackends {}

#[qapi(command)]
#[qapi(name = "query-colo-status")]
#[qapi(returns = "ColoStatus")]
#[qapi(condition = "CONFIG_REPLICATION")]
#[qapi(added = "3.1.0")]
pub struct QueryColoStatus {}

#[qapi(command)]
#[qapi(name = "query-command-line-options")]
#[qapi(returns = "Vec<CommandLineOptionInfo>")]
#[qapi(added = "1.5.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryCommandLineOptions {
    pub option: Option<String>,
}

#[qapi(command)]
#[qapi(name = "query-commands")]
#[qapi(returns = "Vec<CommandInfo>")]
#[qapi(added = "1.0.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryCommands {}

/// Return a list of supported virtual CPU definitions
///
#[qapi(command)]
#[qapi(name = "query-cpu-definitions")]
#[qapi(returns = "Vec<CpuDefinitionInfo>")]
#[qapi(
    condition = "defined(TARGET_PPC) || defined(TARGET_ARM) || defined(TARGET_I386) || defined(TARGET_S390X) || defined(TARGET_MIPS)"
)]
#[qapi(added = "1.2.0")]
pub struct QueryCpuDefinitions {}

#[qapi(command)]
#[qapi(name = "query-cpu-model-baseline")]
#[qapi(returns = "CpuModelBaselineInfo")]
#[qapi(condition = "defined(TARGET_S390X)")]
#[qapi(added = "2.8.0")]
pub struct QueryCpuModelBaseline {
    pub modela: CpuModelInfo,
    pub modelb: CpuModelInfo,
}

#[qapi(command)]
#[qapi(name = "query-cpu-model-comparison")]
#[qapi(returns = "CpuModelCompareInfo")]
#[qapi(condition = "defined(TARGET_S390X)")]
#[qapi(added = "2.8.0")]
pub struct QueryCpuModelComparison {
    pub modela: CpuModelInfo,
    pub modelb: CpuModelInfo,
}

#[qapi(command)]
#[qapi(name = "query-cpu-model-expansion")]
#[qapi(returns = "CpuModelExpansionInfo")]
#[qapi(condition = "defined(TARGET_S390X) || defined(TARGET_I386)")]
#[qapi(added = "2.8.0")]
pub struct QueryCpuModelExpansion {
    pub model: CpuModelInfo,
    pub r#type: CpuModelExpansionType,
}

#[qapi(command)]
#[qapi(name = "query-cpus")]
#[qapi(returns = "Vec<CpuInfo>")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.0.0")]
#[qapi(deprecated(added = "5.0.0"))]
#[qapi(removed = "6.0.0")]
pub struct QueryCpus {}

#[qapi(command)]
#[qapi(name = "query-cpus-fast")]
#[qapi(returns = "Vec<CpuInfoFast>")]
#[qapi(added = "2.12.0")]
pub struct QueryCpusFast {}

/// Return information about current crypto devices.
///
#[qapi(command)]
#[qapi(name = "query-cryptodev")]
#[qapi(returns = "Vec<QCryptodevInfo>")]
#[qapi(added = "8.0.0")]
pub struct QueryCryptodev {}

/// Return information on the current virtual machine.
///
#[qapi(command)]
#[qapi(name = "query-current-machine")]
#[qapi(returns = "CurrentMachineParams")]
#[qapi(added = "4.0.0")]
pub struct QueryCurrentMachine {}

/// query dirty page rate in units of MB/s for vm
///
#[qapi(command)]
#[qapi(name = "query-dirty-rate")]
#[qapi(returns = "DirtyRateInfo")]
#[qapi(added = "5.2.0")]
pub struct QueryDirtyRate {
    #[qapi(name = "calc-time-unit")]
    #[qapi(added = "8.2.0")]
    pub calc_time_unit: Option<TimeUnit>,
}

/// Return information about display configuration
///
#[qapi(command)]
#[qapi(name = "query-display-options")]
#[qapi(returns = "DisplayOptions")]
#[qapi(added = "3.1.0")]
pub struct QueryDisplayOptions {}

#[qapi(command)]
#[qapi(name = "query-dump")]
#[qapi(returns = "DumpQueryResult")]
#[qapi(added = "2.6.0")]
pub struct QueryDump {}

#[qapi(command)]
#[qapi(name = "query-dump-guest-memory-capability")]
#[qapi(returns = "DumpGuestMemoryCapability")]
#[qapi(added = "2.0.0")]
pub struct QueryDumpGuestMemoryCapability {}

#[qapi(command)]
#[qapi(name = "query-events")]
#[qapi(returns = "Vec<EventInfo>")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.2.0")]
#[qapi(deprecated(added = "5.0.0"))]
#[qapi(removed = "6.0.0")]
pub struct QueryEvents {}

#[qapi(command)]
#[qapi(name = "query-fdsets")]
#[qapi(returns = "Vec<FdsetInfo>")]
#[qapi(added = "1.2.0")]
pub struct QueryFdsets {}

/// Find firmware memory log buffer in guest memory, return content.
///
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

#[qapi(command)]
#[qapi(name = "query-gic-capabilities")]
#[qapi(returns = "Vec<GicCapability>")]
#[qapi(condition = "defined(TARGET_ARM)")]
#[qapi(added = "2.6.0")]
pub struct QueryGicCapabilities {}

#[qapi(command)]
#[qapi(name = "query-hotpluggable-cpus")]
#[qapi(returns = "Vec<HotpluggableCpu>")]
#[qapi(added = "2.7.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryHotpluggableCpus {}

#[qapi(command)]
#[qapi(name = "query-hv-balloon-status-report")]
#[qapi(returns = "HvBalloonInfo")]
#[qapi(added = "8.2.0")]
pub struct QueryHvBalloonStatusReport {}

#[qapi(command)]
#[qapi(name = "query-iothreads")]
#[qapi(returns = "Vec<IoThreadInfo>")]
#[qapi(added = "2.0.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryIothreads {}

#[qapi(command)]
#[qapi(name = "query-jobs")]
#[qapi(returns = "Vec<JobInfo>")]
#[qapi(added = "3.0.0")]
pub struct QueryJobs {}

#[qapi(command)]
#[qapi(name = "query-kvm")]
#[qapi(returns = "KvmInfo")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.0.0")]
#[qapi(deprecated(added = "11.1.0"))]
pub struct QueryKvm {}

#[qapi(command)]
#[qapi(name = "query-machines")]
#[qapi(returns = "Vec<MachineInfo>")]
#[qapi(added = "1.2.0")]
pub struct QueryMachines {
    #[qapi(name = "compat-props")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "9.1.0")]
    pub compat_props: Option<bool>,
}

#[qapi(command)]
#[qapi(name = "query-memdev")]
#[qapi(returns = "Vec<Memdev>")]
#[qapi(added = "2.1.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryMemdev {}

/// Lists available memory devices and their state
///
#[qapi(command)]
#[qapi(name = "query-memory-devices")]
#[qapi(returns = "Vec<MemoryDeviceInfo>")]
#[qapi(added = "2.1.0")]
pub struct QueryMemoryDevices {}

/// Return the amount of initially allocated and present hotpluggable
/// (if enabled) memory in bytes.
///
/// .. qmp-example::
///
/// ```text
///     -> { "execute": "query-memory-size-summary" }
///     <- { "return": { "base-memory": 4294967296, "plugged-memory": 0 } }
/// ```
///
#[qapi(command)]
#[qapi(name = "query-memory-size-summary")]
#[qapi(returns = "MemoryInfo")]
#[qapi(added = "2.11.0")]
pub struct QueryMemorySizeSummary {}

#[qapi(command)]
#[qapi(name = "query-mice")]
#[qapi(returns = "Vec<MouseInfo>")]
#[qapi(added = "1.0.0")]
pub struct QueryMice {}

#[qapi(command)]
#[qapi(name = "query-migrate")]
#[qapi(returns = "MigrationInfo")]
#[qapi(added = "1.0.0")]
pub struct QueryMigrate {}

#[qapi(command)]
#[qapi(name = "query-migrate-cache-size")]
#[qapi(returns = "i64")]
#[qapi(feature = "deprecated")]
#[qapi(added = "1.2.0")]
#[qapi(deprecated(added = "5.0.0"))]
#[qapi(removed = "6.0.0")]
pub struct QueryMigrateCacheSize {}

#[qapi(command)]
#[qapi(name = "query-migrate-capabilities")]
#[qapi(returns = "Vec<MigrationCapabilityStatus>")]
#[qapi(added = "1.2.0")]
pub struct QueryMigrateCapabilities {}

#[qapi(command)]
#[qapi(name = "query-migrate-parameters")]
#[qapi(returns = "MigrationParameters")]
#[qapi(added = "2.4.0")]
pub struct QueryMigrateParameters {}

#[qapi(command)]
#[qapi(name = "query-migrationthreads")]
#[qapi(returns = "Vec<MigrationThreadInfo>")]
#[qapi(feature = "deprecated")]
#[qapi(added = "8.0.0")]
#[qapi(deprecated(added = "9.2.0"))]
#[qapi(removed = "11.0.0")]
pub struct QueryMigrationthreads {}

#[qapi(command)]
#[qapi(name = "query-name")]
#[qapi(returns = "NameInfo")]
#[qapi(added = "1.0.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryName {}

#[qapi(command)]
#[qapi(name = "query-named-block-nodes")]
#[qapi(returns = "Vec<BlockDeviceInfo>")]
#[qapi(added = "2.0.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct QueryNamedBlockNodes {
    #[qapi(added = "5.0.0")]
    pub flat: Option<bool>,
}

#[qapi(command)]
#[qapi(name = "query-pci")]
#[qapi(returns = "Vec<PciInfo>")]
#[qapi(added = "1.0.0")]
pub struct QueryPci {}

#[qapi(command)]
#[qapi(name = "query-pr-managers")]
#[qapi(returns = "Vec<PrManagerInfo>")]
#[qapi(added = "3.0.0")]
#[qapi(allow_preconfig)]
pub struct QueryPrManagers {}

#[qapi(command)]
#[qapi(name = "query-qmp-schema")]
#[qapi(returns = "Vec<SchemaInfo>")]
#[qapi(added = "2.5.0")]
#[qapi(allow_preconfig(added = "6.0.0"))]
pub struct QueryQmpSchema {
    /// Additional device-specific properties.
    #[qapi(flatten)]
    #[qapi(removed = "6.0.0")]
    pub arguments: Option<HashMap<String, serde_json::Value>>,
}

#[qapi(command)]
#[qapi(name = "query-replay")]
#[qapi(returns = "ReplayInfo")]
#[qapi(added = "5.2.0")]
pub struct QueryReplay {}

#[qapi(command)]
#[qapi(name = "query-rocker")]
#[qapi(returns = "RockerSwitch")]
#[qapi(added = "2.4.0")]
pub struct QueryRocker {
    pub name: String,
}

#[qapi(command)]
#[qapi(name = "query-rocker-of-dpa-flows")]
#[qapi(returns = "Vec<RockerOfDpaFlow>")]
#[qapi(added = "2.4.0")]
pub struct QueryRockerOfDpaFlows {
    pub name: String,
    #[qapi(name = "tbl-id")]
    pub tbl_id: Option<u32>,
}

#[qapi(command)]
#[qapi(name = "query-rocker-of-dpa-groups")]
#[qapi(returns = "Vec<RockerOfDpaGroup>")]
#[qapi(added = "2.4.0")]
pub struct QueryRockerOfDpaGroups {
    pub name: String,
    pub r#type: Option<u8>,
}

#[qapi(command)]
#[qapi(name = "query-rocker-ports")]
#[qapi(returns = "Vec<RockerPort>")]
#[qapi(added = "2.4.0")]
pub struct QueryRockerPorts {
    pub name: String,
}

#[qapi(command)]
#[qapi(name = "query-rx-filter")]
#[qapi(returns = "Vec<RxFilterInfo>")]
#[qapi(added = "1.6.0")]
pub struct QueryRxFilter {
    pub name: Option<String>,
}

#[qapi(command)]
#[qapi(name = "query-s390x-cpu-polarization")]
#[qapi(condition = "(TARGET_S390X && CONFIG_KVM)")]
#[qapi(feature = "unstable")]
#[qapi(returns = "CpuPolarizationInfo")]
#[qapi(added = "8.2.0")]
#[qapi(unstable(added = "8.2.0"))]
pub struct QueryS390xCpuPolarization {}

#[qapi(command)]
#[qapi(name = "query-sev")]
#[qapi(returns = "SevInfo")]
#[qapi(condition = "defined(TARGET_I386)")]
#[qapi(added = "2.12.0")]
pub struct QuerySev {}

#[qapi(command)]
#[qapi(name = "query-sev-attestation-report")]
#[qapi(condition = "defined(TARGET_I386)")]
#[qapi(returns = "SevAttestationReport")]
#[qapi(added = "6.1.0")]
pub struct QuerySevAttestationReport {
    pub mnonce: String,
}

#[qapi(command)]
#[qapi(name = "query-sev-capabilities")]
#[qapi(returns = "SevCapability")]
#[qapi(condition = "defined(TARGET_I386)")]
#[qapi(added = "2.12.0")]
pub struct QuerySevCapabilities {}

#[qapi(command)]
#[qapi(name = "query-sev-launch-measure")]
#[qapi(returns = "SevLaunchMeasureInfo")]
#[qapi(condition = "defined(TARGET_I386)")]
#[qapi(added = "2.12.0")]
pub struct QuerySevLaunchMeasure {}

#[qapi(command)]
#[qapi(name = "query-sgx")]
#[qapi(condition = "TARGET_I386")]
#[qapi(returns = "SgxInfo")]
#[qapi(added = "6.2.0")]
pub struct QuerySgx {}

#[qapi(command)]
#[qapi(name = "query-sgx-capabilities")]
#[qapi(condition = "TARGET_I386")]
#[qapi(returns = "SgxInfo")]
#[qapi(added = "6.2.0")]
pub struct QuerySgxCapabilities {}

#[qapi(command)]
#[qapi(name = "query-spice")]
#[qapi(returns = "SpiceInfo")]
#[qapi(condition = "defined(CONFIG_SPICE)")]
#[qapi(added = "1.0.0")]
pub struct QuerySpice {}

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
#[qapi(command)]
#[qapi(name = "query-stats-schemas")]
#[qapi(returns = "Vec<StatsSchema>")]
#[qapi(added = "7.1.0")]
pub struct QueryStatsSchemas {
    /// a provider to restrict the query to.
    ///
    /// .. note:: Runtime-collected statistics and their names fall outside
    /// QEMU's usual deprecation policies.  QEMU will try to keep the set
    /// of available data stable, together with their names, but will not
    /// guarantee stability at all costs; the same is true of providers
    /// that source statistics externally, e.g. from Linux.  For example,
    /// if the same value is being tracked with different names on
    /// different architectures or by different providers, one of them
    /// might be renamed.  A statistic might go away if an algorithm is
    /// changed or some code is removed; changing a default might cause
    /// previously useful statistics to always report 0.  Such changes,
    /// however, are expected to be rare.
    pub provider: Option<StatsProvider>,
}

#[qapi(command)]
#[qapi(name = "query-status")]
#[qapi(returns = "StatusInfo")]
#[qapi(added = "1.0.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryStatus {}

/// Return information about the target for this QEMU
///
#[qapi(command)]
#[qapi(name = "query-target")]
#[qapi(returns = "TargetInfo")]
#[qapi(added = "1.2.0")]
pub struct QueryTarget {}

#[qapi(command)]
#[qapi(name = "query-tpm")]
#[qapi(returns = "Vec<TpmInfo>")]
#[qapi(condition = "defined(CONFIG_TPM)")]
#[qapi(added = "1.5.0")]
pub struct QueryTpm {}

#[qapi(command)]
#[qapi(name = "query-tpm-models")]
#[qapi(returns = "Vec<TpmModel>")]
#[qapi(condition = "defined(CONFIG_TPM)")]
#[qapi(added = "1.5.0")]
pub struct QueryTpmModels {}

#[qapi(command)]
#[qapi(name = "query-tpm-types")]
#[qapi(returns = "Vec<TpmType>")]
#[qapi(condition = "defined(CONFIG_TPM)")]
#[qapi(added = "1.5.0")]
pub struct QueryTpmTypes {}

#[qapi(command)]
#[qapi(name = "query-uuid")]
#[qapi(returns = "UuidInfo")]
#[qapi(added = "1.0.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryUuid {}

#[qapi(command)]
#[qapi(name = "query-vcpu-dirty-limit")]
#[qapi(returns = "Vec<DirtyLimitInfo>")]
#[qapi(added = "7.1.0")]
pub struct QueryVcpuDirtyLimit {}

#[qapi(command)]
#[qapi(name = "query-version")]
#[qapi(returns = "VersionInfo")]
#[qapi(added = "1.0.0")]
#[qapi(allow_preconfig(added = "3.0.0"))]
pub struct QueryVersion {}

/// Show Virtual Machine Generation ID
///
#[qapi(command)]
#[qapi(name = "query-vm-generation-id")]
#[qapi(returns = "GuidInfo")]
#[qapi(added = "2.9.0")]
pub struct QueryVmGenerationId {}

#[qapi(command)]
#[qapi(name = "query-vnc")]
#[qapi(returns = "VncInfo")]
#[qapi(condition = "defined(CONFIG_VNC)")]
#[qapi(added = "1.0.0")]
pub struct QueryVnc {}

/// Return a list of vnc servers.  The list can be empty.
///
#[qapi(command)]
#[qapi(name = "query-vnc-servers")]
#[qapi(returns = "Vec<VncInfo2>")]
#[qapi(condition = "defined(CONFIG_VNC)")]
#[qapi(added = "2.3.0")]
pub struct QueryVncServers {}

#[qapi(command)]
#[qapi(name = "query-xen-replication-status")]
#[qapi(returns = "ReplicationStatus")]
#[qapi(condition = "defined(CONFIG_REPLICATION)")]
#[qapi(added = "2.9.0")]
pub struct QueryXenReplicationStatus {}

#[qapi(command)]
#[qapi(name = "query-yank")]
#[qapi(returns = "Vec<YankInstance>")]
#[qapi(added = "6.0.0")]
#[qapi(allow_oob)]
pub struct QueryYank {}

/// This command will cause the QEMU process to exit gracefully.  While every
/// attempt is made to send the QMP response before terminating, this is not
/// guaranteed.  When using this interface, a premature EOF would not be
/// unexpected.
///
#[qapi(command)]
#[qapi(name = "quit")]
#[qapi(returns = "()")]
#[qapi(added = "1.0.0")]
#[qapi(allow_preconfig(added = "6.0.0"))]
pub struct Quit {}

#[qapi(command)]
#[qapi(name = "remove-fd")]
#[qapi(returns = "()")]
#[qapi(added = "1.2.0")]
pub struct RemoveFd {
    pub fd: Option<i64>,
    #[qapi(name = "fdset-id")]
    pub fdset_id: i64,
}

#[qapi(command)]
#[qapi(name = "replay-break")]
#[qapi(returns = "()")]
#[qapi(added = "5.2.0")]
pub struct ReplayBreak {
    pub icount: i64,
}

#[qapi(command)]
#[qapi(name = "replay-delete-break")]
#[qapi(returns = "()")]
#[qapi(added = "5.2.0")]
pub struct ReplayDeleteBreak {}

#[qapi(command)]
#[qapi(name = "replay-seek")]
#[qapi(returns = "()")]
#[qapi(added = "5.2.0")]
pub struct ReplaySeek {
    pub icount: i64,
}

#[qapi(command)]
#[qapi(name = "request-ebpf")]
#[qapi(condition = "CONFIG_EBPF")]
#[qapi(returns = "EbpfObject")]
#[qapi(added = "9.0.0")]
pub struct RequestEbpf {
    pub id: EbpfProgramId,
}

#[qapi(command)]
#[qapi(name = "ringbuf-read")]
#[qapi(returns = "String")]
#[qapi(added = "1.4.0")]
pub struct RingbufRead {
    pub device: String,
    pub format: Option<DataFormat>,
    pub size: i64,
}

#[qapi(command)]
#[qapi(name = "ringbuf-write")]
#[qapi(returns = "()")]
#[qapi(added = "1.4.0")]
pub struct RingbufWrite {
    pub data: String,
    pub device: String,
    pub format: Option<DataFormat>,
}

/// This command will reset the RTC interrupt reinjection backlog.
/// Can be used if another mechanism to synchronize guest time
/// is in effect, for example QEMU guest agent's guest-set-time
/// command.
///
#[qapi(command)]
#[qapi(name = "rtc-reset-reinjection")]
#[qapi(returns = "()")]
#[qapi(condition = "defined(TARGET_I386)")]
#[qapi(added = "2.1.0")]
pub struct RtcResetReinjection {}

#[qapi(command)]
#[qapi(name = "screendump")]
#[qapi(returns = "()")]
#[qapi(condition = "CONFIG_PIXMAN")]
#[qapi(added = "1.3.0")]
pub struct Screendump {
    #[qapi(added = "2.12.0")]
    pub device: Option<String>,
    pub filename: String,
    #[qapi(added = "7.1.0")]
    pub format: Option<ImageFormat>,
    #[qapi(added = "2.12.0")]
    pub head: Option<i64>,
}

#[qapi(command)]
#[qapi(name = "send-key")]
#[qapi(returns = "()")]
#[qapi(added = "1.3.0")]
pub struct SendKey {
    #[qapi(name = "hold-time")]
    pub hold_time: Option<i64>,
    pub keys: Vec<KeyValue>,
}

#[qapi(command)]
#[qapi(name = "set-action")]
#[qapi(returns = "()")]
#[qapi(added = "6.0.0")]
#[qapi(allow_preconfig)]
pub struct SetAction {
    pub panic: Option<PanicAction>,
    pub reboot: Option<RebootAction>,
    pub shutdown: Option<ShutdownAction>,
    pub watchdog: Option<WatchdogAction>,
}

#[qapi(command)]
#[qapi(name = "set-cpu-topology")]
#[qapi(condition = "(TARGET_S390X && CONFIG_KVM)")]
#[qapi(feature = "unstable")]
#[qapi(returns = "()")]
#[qapi(added = "8.2.0")]
#[qapi(unstable(added = "8.2.0"))]
pub struct SetCpuTopology {
    #[qapi(name = "book-id")]
    pub book_id: Option<u16>,
    #[qapi(name = "core-id")]
    pub core_id: u16,
    pub dedicated: Option<bool>,
    #[qapi(name = "drawer-id")]
    pub drawer_id: Option<u16>,
    pub entitlement: Option<S390CpuEntitlement>,
    #[qapi(name = "socket-id")]
    pub socket_id: Option<u16>,
}

#[qapi(command)]
#[qapi(name = "set_link")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct SetLink {
    pub name: String,
    pub up: bool,
}

/// Runtime equivalent of '-numa' CLI option, available at preconfigure
/// stage to configure numa mapping before initializing machine.
///
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

#[qapi(command)]
#[qapi(name = "set_password")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct SetPassword {
    /// How to handle existing clients when changing the
    /// password.  If nothing is specified, defaults to 'keep'.  For
    /// VNC, only 'keep' is currently implemented.
    pub connected: Option<SetPasswordAction>,
    /// the new password
    pub password: String,
    /// - 'vnc' to modify the VNC server password
    /// - 'spice' to modify the Spice server password
    #[qapi(discriminator)]
    pub protocol: DisplayProtocol,
    #[qapi(union)]
    #[qapi(flatten)]
    #[qapi(added = "7.0.0")]
    pub u: Option<SetPasswordOptionsBranch>,
}

#[qapi(command)]
#[qapi(name = "set-vcpu-dirty-limit")]
#[qapi(returns = "()")]
#[qapi(added = "7.1.0")]
pub struct SetVcpuDirtyLimit {
    #[qapi(name = "cpu-index")]
    pub cpu_index: Option<i64>,
    #[qapi(name = "dirty-rate")]
    pub dirty_rate: u64,
}

/// This command injects a secret blob into memory of SEV guest.
///
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
#[qapi(command)]
#[qapi(name = "snapshot-delete")]
#[qapi(returns = "()")]
#[qapi(added = "6.0.0")]
pub struct SnapshotDelete {
    /// list of block device node names to delete a snapshot from
    ///
    /// Applications should not assume that the snapshot delete is complete
    /// when this command returns.  The job commands / events must be used
    /// to determine completion and to fetch details of any errors that
    /// arise.
    ///
    /// .. qmp-example::
    ///
    /// -> { "execute": "snapshot-delete",
    /// "arguments": {
    /// "job-id": "snapdelete0",
    /// "tag": "my-snap",
    /// "devices": ["disk0", "disk1"]
    /// }
    /// }
    /// <- { "return": { } }
    /// <- {"event": "JOB_STATUS_CHANGE",
    /// "timestamp": {"seconds": 1442124172, "microseconds": 744001},
    /// "data": {"status": "created", "id": "snapdelete0"}}
    /// <- {"event": "JOB_STATUS_CHANGE",
    /// "timestamp": {"seconds": 1442125172, "microseconds": 744001},
    /// "data": {"status": "running", "id": "snapdelete0"}}
    /// <- {"event": "JOB_STATUS_CHANGE",
    /// "timestamp": {"seconds": 1442126172, "microseconds": 744001},
    /// "data": {"status": "waiting", "id": "snapdelete0"}}
    /// <- {"event": "JOB_STATUS_CHANGE",
    /// "timestamp": {"seconds": 1442127172, "microseconds": 744001},
    /// "data": {"status": "pending", "id": "snapdelete0"}}
    /// <- {"event": "JOB_STATUS_CHANGE",
    /// "timestamp": {"seconds": 1442128172, "microseconds": 744001},
    /// "data": {"status": "concluded", "id": "snapdelete0"}}
    /// -> {"execute": "query-jobs"}
    /// <- {"return": [{"current-progress": 1,
    /// "status": "concluded",
    /// "total-progress": 1,
    /// "type": "snapshot-delete",
    /// "id": "snapdelete0"}]}
    pub devices: Vec<String>,
    /// identifier for the newly created job
    #[qapi(name = "job-id")]
    pub job_id: String,
    /// name of the snapshot to delete.
    pub tag: String,
}

#[qapi(command)]
#[qapi(name = "snapshot-load")]
#[qapi(returns = "()")]
#[qapi(added = "6.0.0")]
pub struct SnapshotLoad {
    pub devices: Vec<String>,
    #[qapi(name = "job-id")]
    pub job_id: String,
    pub tag: String,
    pub vmstate: String,
}

#[qapi(command)]
#[qapi(name = "snapshot-save")]
#[qapi(returns = "()")]
#[qapi(added = "6.0.0")]
pub struct SnapshotSave {
    pub devices: Vec<String>,
    #[qapi(name = "job-id")]
    pub job_id: String,
    pub tag: String,
    pub vmstate: String,
}

#[qapi(command)]
#[qapi(name = "stop")]
#[qapi(returns = "()")]
#[qapi(added = "1.0.0")]
pub struct Stop {}

#[qapi(command)]
#[qapi(name = "system_powerdown")]
#[qapi(returns = "()")]
#[qapi(added = "1.0.0")]
pub struct SystemPowerdown {}

/// Performs a hard reset of a guest.
///
#[qapi(command)]
#[qapi(name = "system_reset")]
#[qapi(returns = "()")]
#[qapi(added = "1.0.0")]
pub struct SystemReset {}

#[qapi(command)]
#[qapi(name = "system_wakeup")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct SystemWakeup {}

#[qapi(command)]
#[qapi(name = "trace-event-get-state")]
#[qapi(returns = "Vec<TraceEventInfo>")]
#[qapi(added = "2.2.0")]
pub struct TraceEventGetState {
    pub name: String,
    #[qapi(feature = "deprecated")]
    #[qapi(added = "2.7.0")]
    #[qapi(deprecated(added = "8.1.0"))]
    #[qapi(removed = "9.1.0")]
    pub vcpu: Option<i64>,
}

/// Set the dynamic tracing state of events.
///
#[qapi(command)]
#[qapi(name = "trace-event-set-state")]
#[qapi(returns = "()")]
#[qapi(added = "2.2.0")]
pub struct TraceEventSetState {
    /// Whether to enable tracing.
    pub enable: bool,
    /// optional Do not match unavailable events with @name.
    #[qapi(name = "ignore-unavailable")]
    pub ignore_unavailable: Option<bool>,
    /// Event name pattern (case-sensitive glob).
    pub name: String,
    /// optional The vCPU to act upon (all by default; since 2.7).
    ///
    /// An event's state is modified if:
    /// - its name matches the @name pattern, and
    /// - if @vcpu is given, the event has the "vcpu" property.
    ///
    /// Therefore, if @vcpu is given, the operation will only match per-vCPU events,
    /// setting their state on the specified vCPU. Special case: if @name is an exact
    /// match, @vcpu is given and the event does not have the "vcpu" property, an
    /// error is returned.
    #[qapi(feature = "deprecated")]
    #[qapi(added = "2.7.0")]
    #[qapi(deprecated(added = "8.1.0"))]
    #[qapi(removed = "9.1.0")]
    pub vcpu: Option<i64>,
}

#[qapi(command)]
#[qapi(name = "transaction")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct Transaction {
    pub actions: Vec<TransactionAction>,
    #[qapi(added = "2.5.0")]
    pub properties: Option<TransactionProperties>,
}

/// Set watchdog action
///
#[qapi(command)]
#[qapi(name = "watchdog-set-action")]
#[qapi(returns = "()")]
#[qapi(added = "2.11.0")]
pub struct WatchdogSetAction {
    pub action: WatchdogAction,
}

#[qapi(command)]
#[qapi(name = "x-accel-stats")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "10.1.0")]
#[qapi(unstable(added = "10.1.0"))]
pub struct XAccelStats {}

/// Starts a job to amend format specific options of an existing open block device
/// The job is automatically finalized, but a manual job-dismiss is required.
///
#[qapi(command)]
#[qapi(name = "x-blockdev-amend")]
#[qapi(returns = "()")]
#[qapi(feature = "unstable")]
#[qapi(added = "5.1.0")]
#[qapi(unstable(added = "6.2.0"))]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct XBlockdevAmend {
    /// Allow unsafe operations, format specific
    /// For luks that allows erase of the last active keyslot
    /// (permanent loss of data),
    /// and replacement of an active keyslot
    /// (possible loss of data if IO error happens)
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

#[qapi(command)]
#[qapi(name = "x-blockdev-change")]
#[qapi(returns = "()")]
#[qapi(feature = "unstable")]
#[qapi(added = "2.7.0")]
#[qapi(unstable(added = "6.2.0"))]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct XBlockdevChange {
    pub child: Option<String>,
    pub node: Option<String>,
    pub parent: String,
}

#[qapi(command)]
#[qapi(name = "x-blockdev-create")]
#[qapi(returns = "()")]
#[qapi(added = "2.12.0")]
#[qapi(removed = "3.0.0")]
pub struct XBlockdevCreate {
    /// block driver to create the image format
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
///
#[qapi(command)]
#[qapi(name = "x-blockdev-del")]
#[qapi(returns = "()")]
#[qapi(added = "2.5.0")]
#[qapi(removed = "2.9.0")]
pub struct XBlockdevDel {
    /// optional Name of the block backend device to delete.
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
/// This command is still a work in progress and is considered experimental.
/// Stay away from it unless you want to help with its development.
///
#[qapi(command)]
#[qapi(name = "x-blockdev-insert-medium")]
#[qapi(returns = "()")]
#[qapi(added = "2.5.0")]
#[qapi(removed = "2.12.0")]
pub struct XBlockdevInsertMedium {
    /// optional Block device name (deprecated, use @id instead)
    pub device: Option<String>,
    /// optional The name or QOM path of the guest device (since: 2.8)
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
/// This command is still a work in progress and is considered experimental.
/// Stay away from it unless you want to help with its development.
///
#[qapi(command)]
#[qapi(name = "x-blockdev-remove-medium")]
#[qapi(returns = "()")]
#[qapi(added = "2.5.0")]
#[qapi(removed = "2.12.0")]
pub struct XBlockdevRemoveMedium {
    /// optional Block device name (deprecated, use @id instead)
    pub device: Option<String>,
    /// optional The name or QOM path of the guest device (since: 2.8)
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
///
#[qapi(command)]
#[qapi(name = "x-blockdev-reopen")]
#[qapi(returns = "()")]
#[qapi(added = "4.0.0")]
#[qapi(removed = "6.1.0")]
pub struct XBlockdevReopen {
    #[qapi(name = "auto-read-only")]
    pub auto_read_only: Option<bool>,
    pub cache: Option<BlockdevCacheOptions>,
    #[qapi(name = "detect-zeroes")]
    pub detect_zeroes: Option<BlockdevDetectZeroesOptions>,
    pub discard: Option<BlockdevDiscardOptions>,
    #[qapi(discriminator)]
    pub driver: BlockdevDriver,
    #[qapi(name = "force-share")]
    pub force_share: Option<bool>,
    #[qapi(name = "node-name")]
    pub node_name: Option<String>,
    #[qapi(name = "read-only")]
    pub read_only: Option<bool>,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<BlockdevOptionsBranch>,
}

#[qapi(command)]
#[qapi(name = "x-blockdev-set-iothread")]
#[qapi(returns = "()")]
#[qapi(feature = "unstable")]
#[qapi(added = "2.12.0")]
#[qapi(unstable(added = "6.2.0"))]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct XBlockdevSetIothread {
    pub force: Option<bool>,
    pub iothread: StrOrNull,
    #[qapi(name = "node-name")]
    pub node_name: String,
}

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

#[qapi(command)]
#[qapi(name = "x-block-latency-histogram-set")]
#[qapi(returns = "()")]
#[qapi(added = "2.12.0")]
#[qapi(removed = "4.0.0")]
pub struct XBlockLatencyHistogramSet {
    pub boundaries: Option<Vec<u64>>,
    #[qapi(name = "boundaries-flush")]
    pub boundaries_flush: Option<Vec<u64>>,
    #[qapi(name = "boundaries-read")]
    pub boundaries_read: Option<Vec<u64>>,
    #[qapi(name = "boundaries-write")]
    pub boundaries_write: Option<Vec<u64>>,
    pub device: String,
}

/// Tell qemu that heartbeat is lost, request it to do takeover procedures.
/// If this command is sent to the PVM, the Primary side will exit COLO mode.
/// If sent to the Secondary, the Secondary side will run failover work,
/// then takes over server operation to become the service VM.
///
#[qapi(command)]
#[qapi(name = "x-colo-lost-heartbeat")]
#[qapi(returns = "()")]
#[qapi(feature = "unstable")]
#[qapi(condition = "CONFIG_REPLICATION")]
#[qapi(added = "2.8.0")]
#[qapi(unstable(added = "6.2.0"))]
pub struct XColoLostHeartbeat {}

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
///
#[qapi(command)]
#[qapi(name = "xen-colo-do-checkpoint")]
#[qapi(returns = "()")]
#[qapi(condition = "defined(CONFIG_REPLICATION)")]
#[qapi(added = "2.9.0")]
pub struct XenColoDoCheckpoint {}

#[qapi(command)]
#[qapi(name = "xen-event-inject")]
#[qapi(condition = "TARGET_I386")]
#[qapi(returns = "()")]
#[qapi(added = "8.0.0")]
pub struct XenEventInject {
    pub port: u32,
}

#[qapi(command)]
#[qapi(name = "xen-event-list")]
#[qapi(condition = "TARGET_I386")]
#[qapi(returns = "Vec<EvtchnInfo>")]
#[qapi(added = "8.0.0")]
pub struct XenEventList {}

/// Load the state of all devices from file. The RAM and the block devices
/// of the VM are not loaded by this command.
///
#[qapi(command)]
#[qapi(name = "xen-load-devices-state")]
#[qapi(returns = "()")]
#[qapi(added = "2.7.0")]
pub struct XenLoadDevicesState {
    /// the file to load the state of the devices from as binary
    /// data. See xen-save-devices-state.txt for a description of the binary
    /// format.
    pub filename: String,
}

#[qapi(command)]
#[qapi(name = "xen-save-devices-state")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
pub struct XenSaveDevicesState {
    pub filename: String,
    #[qapi(added = "2.11.0")]
    pub live: Option<bool>,
}

#[qapi(command)]
#[qapi(name = "xen-set-global-dirty-log")]
#[qapi(returns = "()")]
#[qapi(added = "1.3.0")]
pub struct XenSetGlobalDirtyLog {
    pub enable: bool,
}

/// Enable or disable replication.
///
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
    ///
    /// .. qmp-example::
    ///
    /// -> { "execute": "xen-set-replication",
    /// "arguments": {"enable": true, "primary": false} }
    /// <- { "return": {} }
    pub failover: Option<bool>,
    /// true for primary or false for secondary.
    pub primary: bool,
}

#[qapi(command)]
#[qapi(name = "x-exit-preconfig")]
#[qapi(returns = "()")]
#[qapi(feature = "unstable")]
#[qapi(added = "3.0.0")]
#[qapi(unstable(added = "6.2.0"))]
#[qapi(allow_preconfig)]
pub struct XExitPreconfig {}

#[qapi(command)]
#[qapi(name = "x-input-send-event")]
#[qapi(returns = "()")]
#[qapi(added = "2.2.0")]
#[qapi(removed = "2.6.0")]
pub struct XInputSendEvent {
    pub console: Option<i64>,
    pub events: Vec<InputEvent>,
}

#[qapi(command)]
#[qapi(name = "x-nbd-server-add-bitmap")]
#[qapi(returns = "()")]
#[qapi(added = "3.0.0")]
#[qapi(removed = "4.0.0")]
pub struct XNbdServerAddBitmap {
    pub bitmap: String,
    #[qapi(name = "bitmap-export-name")]
    pub bitmap_export_name: Option<String>,
    pub name: String,
}

#[qapi(command)]
#[qapi(name = "x-oob-test")]
#[qapi(returns = "()")]
#[qapi(added = "2.12.0")]
#[qapi(removed = "3.0.0")]
#[qapi(allow_oob)]
pub struct XOobTest {
    pub lock: bool,
}

#[qapi(command)]
#[qapi(name = "x-query-interrupt-controllers")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "9.1.0")]
#[qapi(unstable(added = "9.1.0"))]
pub struct XQueryInterruptControllers {}

#[qapi(command)]
#[qapi(name = "x-query-irq")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "6.2.0")]
#[qapi(unstable(added = "6.2.0"))]
pub struct XQueryIrq {}

#[qapi(command)]
#[qapi(name = "x-query-jit")]
#[qapi(condition = "CONFIG_TCG")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "6.2.0")]
#[qapi(unstable(added = "6.2.0"))]
pub struct XQueryJit {}

#[qapi(command)]
#[qapi(name = "x-query-numa")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "6.2.0")]
#[qapi(unstable(added = "6.2.0"))]
pub struct XQueryNuma {}

#[qapi(command)]
#[qapi(name = "x-query-opcount")]
#[qapi(condition = "CONFIG_TCG")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "6.2.0")]
#[qapi(unstable(added = "6.2.0"))]
#[qapi(removed = "10.1.0")]
pub struct XQueryOpcount {}

#[qapi(command)]
#[qapi(name = "x-query-profile")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(condition = "CONFIG_TCG")]
#[qapi(added = "6.2.0")]
#[qapi(unstable(added = "6.2.0"))]
#[qapi(removed = "8.1.0")]
pub struct XQueryProfile {}

#[qapi(command)]
#[qapi(name = "x-query-ramblock")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "6.2.0")]
#[qapi(unstable(added = "6.2.0"))]
pub struct XQueryRamblock {}

#[qapi(command)]
#[qapi(name = "x-query-rdma")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "6.2.0")]
#[qapi(unstable(added = "6.2.0"))]
#[qapi(removed = "9.1.0")]
pub struct XQueryRdma {}

#[qapi(command)]
#[qapi(name = "x-query-roms")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "6.2.0")]
#[qapi(unstable(added = "6.2.0"))]
pub struct XQueryRoms {}

#[qapi(command)]
#[qapi(name = "x-query-usb")]
#[qapi(feature = "unstable")]
#[qapi(returns = "HumanReadableText")]
#[qapi(added = "6.2.0")]
#[qapi(unstable(added = "6.2.0"))]
pub struct XQueryUsb {}

#[qapi(command)]
#[qapi(name = "x-query-virtio")]
#[qapi(feature = "unstable")]
#[qapi(returns = "Vec<VirtioInfo>")]
#[qapi(added = "7.2.0")]
#[qapi(unstable(added = "7.2.0"))]
pub struct XQueryVirtio {}

#[qapi(command)]
#[qapi(name = "x-query-virtio-queue-element")]
#[qapi(feature = "unstable")]
#[qapi(returns = "VirtioQueueElement")]
#[qapi(added = "7.2.0")]
#[qapi(unstable(added = "7.2.0"))]
pub struct XQueryVirtioQueueElement {
    pub index: Option<u16>,
    pub path: String,
    pub queue: u16,
}

#[qapi(command)]
#[qapi(name = "x-query-virtio-queue-status")]
#[qapi(feature = "unstable")]
#[qapi(returns = "VirtQueueStatus")]
#[qapi(added = "7.2.0")]
#[qapi(unstable(added = "7.2.0"))]
pub struct XQueryVirtioQueueStatus {
    pub path: String,
    pub queue: u16,
}

#[qapi(command)]
#[qapi(name = "x-query-virtio-status")]
#[qapi(feature = "unstable")]
#[qapi(returns = "VirtioStatus")]
#[qapi(added = "7.2.0")]
#[qapi(unstable(added = "7.2.0"))]
pub struct XQueryVirtioStatus {
    pub path: String,
}

#[qapi(command)]
#[qapi(name = "x-query-virtio-vhost-queue-status")]
#[qapi(feature = "unstable")]
#[qapi(returns = "VirtVhostQueueStatus")]
#[qapi(added = "7.2.0")]
#[qapi(unstable(added = "7.2.0"))]
pub struct XQueryVirtioVhostQueueStatus {
    pub path: String,
    pub queue: u16,
}

#[qapi(command)]
#[qapi(name = "yank")]
#[qapi(returns = "()")]
#[qapi(added = "6.0.0")]
#[qapi(allow_oob)]
pub struct Yank {
    pub instances: Vec<YankInstance>,
}

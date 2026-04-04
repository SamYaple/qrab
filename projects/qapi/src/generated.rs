#![allow(unused_imports, non_camel_case_types, dead_code)]
use crate as qapi;
use qapi_macros::qapi;
use std::collections::HashMap;

#[qapi(name = "ACPISlotType")]
#[qapi(since = "2.8")]
pub enum AcpiSlotType {
    #[qapi(name = "DIMM")]
    Dimm,
    #[qapi(name = "CPU")]
    Cpu,
}
#[qapi(name = "AFXDPMode")]
#[qapi(condition = "CONFIG_AF_XDP")]
#[qapi(since = "8.2")]
pub enum AfxdpMode {
    #[qapi(name = "native")]
    Native,
    #[qapi(name = "skb")]
    Skb,
}
#[qapi(since = "10.2.0")]
pub enum Accelerator {
    #[qapi(name = "hvf")]
    Hvf,
    #[qapi(name = "kvm")]
    Kvm,
    #[qapi(name = "mshv")]
    Mshv,
    #[qapi(name = "nvmm")]
    Nvmm,
    #[qapi(name = "qtest")]
    Qtest,
    #[qapi(name = "tcg")]
    Tcg,
    #[qapi(name = "whpx")]
    Whpx,
    #[qapi(name = "xen")]
    Xen,
}
#[qapi(since = "2.5")]
pub enum ActionCompletionMode {
    #[qapi(name = "individual")]
    Individual,
    #[qapi(name = "grouped")]
    Grouped,
}
#[qapi(since = "4.0")]
pub enum AudioFormat {
    #[qapi(name = "u8")]
    U8,
    #[qapi(name = "s8")]
    S8,
    #[qapi(name = "u16")]
    U16,
    #[qapi(name = "s16")]
    S16,
    #[qapi(name = "u32")]
    U32,
    #[qapi(name = "s32")]
    S32,
    #[qapi(name = "f32")]
    #[qapi(since = "5.0")]
    F32,
}
#[qapi(since = "4.0")]
pub enum AudiodevBranch {
    #[qapi(name = "none")]
    None(AudiodevGenericOptions),
    #[qapi(name = "alsa")]
    Alsa(AudiodevAlsaOptions),
    #[qapi(name = "coreaudio")]
    Coreaudio(AudiodevCoreaudioOptions),
    #[qapi(name = "dsound")]
    Dsound(AudiodevDsoundOptions),
    #[qapi(name = "oss")]
    Oss(AudiodevOssOptions),
    #[qapi(name = "pa")]
    Pa(AudiodevPaOptions),
    #[qapi(name = "sdl")]
    Sdl(AudiodevGenericOptions),
    #[qapi(name = "spice")]
    Spice(AudiodevGenericOptions),
    #[qapi(name = "wav")]
    Wav(AudiodevWavOptions),
    #[qapi(name = "jack")]
    #[qapi(since = "5.1")]
    Jack(AudiodevJackOptions),
    #[qapi(name = "dbus")]
    #[qapi(since = "7.0")]
    Dbus(AudiodevGenericOptions),
    #[qapi(name = "sndio")]
    #[qapi(since = "7.2")]
    Sndio(AudiodevSndioOptions),
    #[qapi(name = "pipewire")]
    #[qapi(condition = "CONFIG_AUDIO_PIPEWIRE")]
    #[qapi(since = "8.1")]
    Pipewire(AudiodevPipewireOptions),
}
#[qapi(since = "4.0")]
pub enum AudiodevDriver {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "alsa")]
    Alsa,
    #[qapi(name = "coreaudio")]
    Coreaudio,
    #[qapi(name = "dsound")]
    Dsound,
    #[qapi(name = "oss")]
    Oss,
    #[qapi(name = "pa")]
    Pa,
    #[qapi(name = "sdl")]
    Sdl,
    #[qapi(name = "spice")]
    Spice,
    #[qapi(name = "wav")]
    Wav,
    #[qapi(name = "jack")]
    #[qapi(since = "5.1")]
    Jack,
    #[qapi(name = "dbus")]
    #[qapi(since = "7.0")]
    Dbus,
    #[qapi(name = "sndio")]
    #[qapi(since = "7.2")]
    Sndio,
    #[qapi(name = "pipewire")]
    #[qapi(condition = "CONFIG_AUDIO_PIPEWIRE")]
    #[qapi(since = "8.1")]
    Pipewire,
}
#[qapi(since = "2.0")]
pub enum BiosAtaTranslation {
    #[qapi(name = "auto")]
    Auto,
    #[qapi(name = "none")]
    None,
    #[qapi(name = "lba")]
    Lba,
    #[qapi(name = "large")]
    Large,
    #[qapi(name = "rechs")]
    Rechs,
}
#[qapi(since = "4.2")]
pub enum BitmapSyncMode {
    #[qapi(name = "on-success")]
    OnSuccess,
    #[qapi(name = "never")]
    Never,
    #[qapi(name = "always")]
    Always,
}
#[qapi(since = "2.0")]
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
    #[qapi(since = "2.2")]
    PwritevRmwHead,
    #[qapi(name = "pwritev_rmw_after_head")]
    #[qapi(since = "2.2")]
    PwritevRmwAfterHead,
    #[qapi(name = "pwritev_rmw_tail")]
    #[qapi(since = "2.2")]
    PwritevRmwTail,
    #[qapi(name = "pwritev_rmw_after_tail")]
    #[qapi(since = "2.2")]
    PwritevRmwAfterTail,
    #[qapi(name = "pwritev")]
    #[qapi(since = "2.2")]
    Pwritev,
    #[qapi(name = "pwritev_zero")]
    #[qapi(since = "2.2")]
    PwritevZero,
    #[qapi(name = "pwritev_done")]
    #[qapi(since = "2.2")]
    PwritevDone,
    #[qapi(name = "empty_image_prepare")]
    #[qapi(since = "2.2")]
    EmptyImagePrepare,
    #[qapi(name = "l1_shrink_write_table")]
    #[qapi(since = "2.11")]
    L1ShrinkWriteTable,
    #[qapi(name = "l1_shrink_free_l2_clusters")]
    #[qapi(since = "2.11")]
    L1ShrinkFreeL2Clusters,
    #[qapi(name = "cor_write")]
    #[qapi(since = "2.11")]
    CorWrite,
    #[qapi(name = "cluster_alloc_space")]
    #[qapi(since = "4.1")]
    ClusterAllocSpace,
    #[qapi(name = "none")]
    #[qapi(since = "4.1")]
    None,
}
#[qapi(name = "BlkdebugIOType")]
#[qapi(since = "4.1")]
pub enum BlkdebugIoType {
    #[qapi(name = "read")]
    Read,
    #[qapi(name = "write")]
    Write,
    #[qapi(name = "write-zeroes")]
    WriteZeroes,
    #[qapi(name = "discard")]
    Discard,
    #[qapi(name = "flush")]
    Flush,
    #[qapi(name = "block-status")]
    BlockStatus,
}
#[qapi(since = "1.0")]
pub enum BlockDeviceIoStatus {
    #[qapi(name = "ok")]
    Ok,
    #[qapi(name = "failed")]
    Failed,
    #[qapi(name = "nospace")]
    Nospace,
}
#[qapi(since = "4.1")]
#[qapi(until = "7.1")]
pub enum BlockDirtyBitmapMergeSource {
    #[qapi(name = "local")]
    Local(String),
    #[qapi(name = "external")]
    External(BlockDirtyBitmap),
}
#[qapi(since = "4.1")]
pub enum BlockDirtyBitmapOrStr {
    #[qapi(name = "local")]
    Local(String),
    #[qapi(name = "external")]
    External(BlockDirtyBitmap),
}
#[qapi(since = "2.1")]
pub enum BlockErrorAction {
    #[qapi(name = "ignore")]
    Ignore,
    #[qapi(name = "report")]
    Report,
    #[qapi(name = "stop")]
    Stop,
}
#[qapi(since = "5.0")]
#[qapi(until = "5.2")]
pub enum BlockExportBranch {
    #[qapi(name = "nbd")]
    Nbd(BlockExportNbd),
}
#[qapi(since = "5.2")]
pub enum BlockExportOptionsBranch {
    #[qapi(name = "nbd")]
    Nbd(BlockExportOptionsNbd),
    #[qapi(name = "vhost-user-blk")]
    VhostUserBlk(BlockExportOptionsVhostUserBlk),
    #[qapi(name = "fuse")]
    #[qapi(condition = "defined(CONFIG_FUSE)")]
    #[qapi(since = "6.0")]
    Fuse(BlockExportOptionsFuse),
    #[qapi(name = "vduse-blk")]
    #[qapi(condition = "CONFIG_VDUSE_BLK_EXPORT")]
    #[qapi(since = "7.1")]
    VduseBlk(BlockExportOptionsVduseBlk),
}
#[qapi(since = "2.12")]
pub enum BlockExportRemoveMode {
    #[qapi(name = "safe")]
    Safe,
    #[qapi(name = "hard")]
    Hard,
}
#[qapi(since = "4.2")]
pub enum BlockExportType {
    #[qapi(name = "nbd")]
    Nbd,
    #[qapi(name = "vhost-user-blk")]
    #[qapi(since = "5.2")]
    VhostUserBlk,
    #[qapi(name = "fuse")]
    #[qapi(condition = "defined(CONFIG_FUSE)")]
    #[qapi(since = "6.0")]
    Fuse,
    #[qapi(name = "vduse-blk")]
    #[qapi(condition = "CONFIG_VDUSE_BLK_EXPORT")]
    #[qapi(since = "7.1")]
    VduseBlk,
}
#[qapi(since = "8.2")]
pub enum BlockJobChangeOptionsBranch {
    #[qapi(name = "mirror")]
    Mirror(BlockJobChangeOptionsMirror),
}
#[qapi(since = "8.2")]
pub enum BlockJobInfoBranch {
    #[qapi(name = "mirror")]
    Mirror(BlockJobInfoMirror),
}
#[qapi(since = "2.12")]
#[qapi(until = "3.0")]
pub enum BlockJobStatus {
    #[qapi(name = "undefined")]
    Undefined,
    #[qapi(name = "created")]
    Created,
    #[qapi(name = "running")]
    Running,
    #[qapi(name = "paused")]
    Paused,
    #[qapi(name = "ready")]
    Ready,
    #[qapi(name = "standby")]
    Standby,
    #[qapi(name = "waiting")]
    Waiting,
    #[qapi(name = "pending")]
    Pending,
    #[qapi(name = "aborting")]
    Aborting,
    #[qapi(name = "concluded")]
    Concluded,
    #[qapi(name = "null")]
    Null,
}
#[qapi(since = "1.7")]
#[qapi(until = "3.0")]
pub enum BlockJobType {
    #[qapi(name = "commit")]
    Commit,
    #[qapi(name = "stream")]
    Stream,
    #[qapi(name = "mirror")]
    Mirror,
    #[qapi(name = "backup")]
    Backup,
}
#[qapi(since = "2.12")]
#[qapi(until = "3.0")]
pub enum BlockJobVerb {
    #[qapi(name = "cancel")]
    Cancel,
    #[qapi(name = "pause")]
    Pause,
    #[qapi(name = "resume")]
    Resume,
    #[qapi(name = "set-speed")]
    SetSpeed,
    #[qapi(name = "complete")]
    Complete,
    #[qapi(name = "dismiss")]
    Dismiss,
    #[qapi(name = "finalize")]
    Finalize,
}
#[qapi(since = "4.0")]
pub enum BlockPermission {
    #[qapi(name = "consistent-read")]
    ConsistentRead,
    #[qapi(name = "write")]
    Write,
    #[qapi(name = "write-unchanged")]
    WriteUnchanged,
    #[qapi(name = "resize")]
    Resize,
    #[qapi(name = "graph-mod")]
    #[qapi(until = "7.0")]
    GraphMod,
}
#[qapi(since = "4.2")]
pub enum BlockStatsSpecificBranch {
    #[qapi(name = "file")]
    File(BlockStatsSpecificFile),
    #[qapi(name = "host_device")]
    HostDevice(BlockStatsSpecificFile),
    #[qapi(name = "nvme")]
    #[qapi(since = "5.2")]
    Nvme(BlockStatsSpecificNvme),
}
#[qapi(since = "1.1")]
#[qapi(until = "1.6")]
pub enum BlockdevActionBranch {
    #[qapi(name = "blockdev-snapshot-sync")]
    BlockdevSnapshotSync(BlockdevSnapshot),
}
#[qapi(since = "1.1")]
#[qapi(until = "1.6")]
pub enum BlockdevActionKind {
    #[qapi(name = "blockdev-snapshot-sync")]
    BlockdevSnapshotSync,
}
#[qapi(since = "1.7")]
pub enum BlockdevAioOptions {
    #[qapi(name = "threads")]
    Threads,
    #[qapi(name = "native")]
    Native,
    #[qapi(name = "io_uring")]
    #[qapi(condition = "defined(CONFIG_LINUX_IO_URING)")]
    #[qapi(since = "5.0")]
    IoUring,
}
#[qapi(since = "5.1")]
pub enum BlockdevAmendOptionsBranch {
    #[qapi(name = "luks")]
    Luks(BlockdevAmendOptionsLuks),
    #[qapi(name = "qcow2")]
    Qcow2(BlockdevAmendOptionsQcow2),
}
#[qapi(since = "2.3")]
pub enum BlockdevChangeReadOnlyMode {
    #[qapi(name = "retain")]
    Retain,
    #[qapi(name = "read-only")]
    ReadOnly,
    #[qapi(name = "read-write")]
    ReadWrite,
}
#[qapi(since = "2.12")]
pub enum BlockdevCreateOptionsBranch {
    #[qapi(name = "blkdebug")]
    #[qapi(until = "3.0")]
    Blkdebug(BlockdevCreateNotSupported),
    #[qapi(name = "blkverify")]
    #[qapi(until = "3.0")]
    Blkverify(BlockdevCreateNotSupported),
    #[qapi(name = "bochs")]
    #[qapi(until = "3.0")]
    Bochs(BlockdevCreateNotSupported),
    #[qapi(name = "cloop")]
    #[qapi(until = "3.0")]
    Cloop(BlockdevCreateNotSupported),
    #[qapi(name = "dmg")]
    #[qapi(until = "3.0")]
    Dmg(BlockdevCreateNotSupported),
    #[qapi(name = "file")]
    File(BlockdevCreateOptionsFile),
    #[qapi(name = "ftp")]
    #[qapi(until = "3.0")]
    Ftp(BlockdevCreateNotSupported),
    #[qapi(name = "ftps")]
    #[qapi(until = "3.0")]
    Ftps(BlockdevCreateNotSupported),
    #[qapi(name = "gluster")]
    Gluster(BlockdevCreateOptionsGluster),
    #[qapi(name = "host_cdrom")]
    #[qapi(until = "3.0")]
    HostCdrom(BlockdevCreateNotSupported),
    #[qapi(name = "host_device")]
    #[qapi(until = "3.0")]
    HostDevice(BlockdevCreateNotSupported),
    #[qapi(name = "http")]
    #[qapi(until = "3.0")]
    Http(BlockdevCreateNotSupported),
    #[qapi(name = "https")]
    #[qapi(until = "3.0")]
    Https(BlockdevCreateNotSupported),
    #[qapi(name = "iscsi")]
    #[qapi(until = "3.0")]
    Iscsi(BlockdevCreateNotSupported),
    #[qapi(name = "luks")]
    Luks(BlockdevCreateOptionsLuks),
    #[qapi(name = "nbd")]
    #[qapi(until = "3.0")]
    Nbd(BlockdevCreateNotSupported),
    #[qapi(name = "nfs")]
    Nfs(BlockdevCreateOptionsNfs),
    #[qapi(name = "null-aio")]
    #[qapi(until = "3.0")]
    NullAio(BlockdevCreateNotSupported),
    #[qapi(name = "null-co")]
    #[qapi(until = "3.0")]
    NullCo(BlockdevCreateNotSupported),
    #[qapi(name = "nvme")]
    #[qapi(until = "3.0")]
    Nvme(BlockdevCreateNotSupported),
    #[qapi(name = "parallels")]
    Parallels(BlockdevCreateOptionsParallels),
    #[qapi(name = "qcow")]
    Qcow(BlockdevCreateOptionsQcow),
    #[qapi(name = "qcow2")]
    Qcow2(BlockdevCreateOptionsQcow2),
    #[qapi(name = "qed")]
    Qed(BlockdevCreateOptionsQed),
    #[qapi(name = "quorum")]
    #[qapi(until = "3.0")]
    Quorum(BlockdevCreateNotSupported),
    #[qapi(name = "raw")]
    #[qapi(until = "3.0")]
    Raw(BlockdevCreateNotSupported),
    #[qapi(name = "rbd")]
    Rbd(BlockdevCreateOptionsRbd),
    #[qapi(name = "replication")]
    #[qapi(until = "3.0")]
    Replication(BlockdevCreateNotSupported),
    #[qapi(name = "sheepdog")]
    #[qapi(until = "6.1")]
    Sheepdog(BlockdevCreateOptionsSheepdog),
    #[qapi(name = "ssh")]
    Ssh(BlockdevCreateOptionsSsh),
    #[qapi(name = "throttle")]
    #[qapi(until = "3.0")]
    Throttle(BlockdevCreateNotSupported),
    #[qapi(name = "vdi")]
    Vdi(BlockdevCreateOptionsVdi),
    #[qapi(name = "vhdx")]
    Vhdx(BlockdevCreateOptionsVhdx),
    #[qapi(name = "vmdk")]
    Vmdk(BlockdevCreateNotSupported),
    #[qapi(name = "vpc")]
    Vpc(BlockdevCreateOptionsVpc),
    #[qapi(name = "vvfat")]
    #[qapi(until = "3.0")]
    Vvfat(BlockdevCreateNotSupported),
    #[qapi(name = "vxhs")]
    #[qapi(until = "3.0")]
    Vxhs(BlockdevCreateNotSupported),
}
#[qapi(since = "2.1")]
pub enum BlockdevDetectZeroesOptions {
    #[qapi(name = "off")]
    Off,
    #[qapi(name = "on")]
    On,
    #[qapi(name = "unmap")]
    Unmap,
}
#[qapi(since = "1.7")]
pub enum BlockdevDiscardOptions {
    #[qapi(name = "ignore")]
    Ignore,
    #[qapi(name = "unmap")]
    Unmap,
}
#[qapi(since = "2.0")]
pub enum BlockdevDriver {
    #[qapi(name = "file")]
    File,
    #[qapi(name = "http")]
    Http,
    #[qapi(name = "https")]
    Https,
    #[qapi(name = "ftp")]
    Ftp,
    #[qapi(name = "ftps")]
    Ftps,
    #[qapi(name = "tftp")]
    #[qapi(until = "2.8")]
    Tftp,
    #[qapi(name = "vvfat")]
    Vvfat,
    #[qapi(name = "blkdebug")]
    Blkdebug,
    #[qapi(name = "blkverify")]
    Blkverify,
    #[qapi(name = "bochs")]
    Bochs,
    #[qapi(name = "cloop")]
    Cloop,
    #[qapi(name = "cow")]
    #[qapi(until = "2.2")]
    Cow,
    #[qapi(name = "dmg")]
    Dmg,
    #[qapi(name = "parallels")]
    Parallels,
    #[qapi(name = "qcow")]
    Qcow,
    #[qapi(name = "qcow2")]
    Qcow2,
    #[qapi(name = "qed")]
    Qed,
    #[qapi(name = "raw")]
    Raw,
    #[qapi(name = "vdi")]
    Vdi,
    #[qapi(name = "vhdx")]
    Vhdx,
    #[qapi(name = "vmdk")]
    Vmdk,
    #[qapi(name = "vpc")]
    Vpc,
    #[qapi(name = "quorum")]
    Quorum,
    #[qapi(name = "host_device")]
    #[qapi(since = "2.1")]
    HostDevice,
    #[qapi(name = "host_cdrom")]
    #[qapi(since = "2.1")]
    HostCdrom,
    #[qapi(name = "host_floppy")]
    #[qapi(since = "2.1")]
    #[qapi(until = "2.5")]
    HostFloppy,
    #[qapi(name = "archipelago")]
    #[qapi(since = "2.2")]
    #[qapi(until = "2.9")]
    Archipelago,
    #[qapi(name = "null-aio")]
    #[qapi(since = "2.2")]
    NullAio,
    #[qapi(name = "null-co")]
    #[qapi(since = "2.2")]
    NullCo,
    #[qapi(name = "luks")]
    #[qapi(since = "2.6")]
    Luks,
    #[qapi(name = "gluster")]
    #[qapi(since = "2.7")]
    Gluster,
    #[qapi(name = "nbd")]
    #[qapi(since = "2.8")]
    Nbd,
    #[qapi(name = "nfs")]
    #[qapi(since = "2.8")]
    Nfs,
    #[qapi(name = "replication")]
    #[qapi(since = "2.8")]
    Replication,
    #[qapi(name = "ssh")]
    #[qapi(since = "2.8")]
    Ssh,
    #[qapi(name = "iscsi")]
    #[qapi(since = "2.9")]
    Iscsi,
    #[qapi(name = "rbd")]
    #[qapi(since = "2.9")]
    Rbd,
    #[qapi(name = "sheepdog")]
    #[qapi(since = "2.9")]
    #[qapi(until = "6.1")]
    Sheepdog,
    #[qapi(name = "vxhs")]
    #[qapi(since = "2.10")]
    #[qapi(until = "5.1")]
    Vxhs,
    #[qapi(name = "throttle")]
    #[qapi(since = "2.11")]
    Throttle,
    #[qapi(name = "nvme")]
    #[qapi(since = "2.12")]
    Nvme,
    #[qapi(name = "blklogwrites")]
    #[qapi(since = "3.0")]
    Blklogwrites,
    #[qapi(name = "copy-on-read")]
    #[qapi(since = "3.0")]
    CopyOnRead,
    #[qapi(name = "blkreplay")]
    #[qapi(since = "4.2")]
    Blkreplay,
    #[qapi(name = "compress")]
    #[qapi(since = "5.0")]
    Compress,
    #[qapi(name = "preallocate")]
    #[qapi(since = "6.0")]
    Preallocate,
    #[qapi(name = "copy-before-write")]
    #[qapi(since = "6.2")]
    CopyBeforeWrite,
    #[qapi(name = "snapshot-access")]
    #[qapi(since = "7.0")]
    SnapshotAccess,
    #[qapi(name = "io_uring")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(since = "7.2")]
    IoUring,
    #[qapi(name = "nvme-io_uring")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(since = "7.2")]
    NvmeIoUring,
    #[qapi(name = "virtio-blk-vfio-pci")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(since = "7.2")]
    VirtioBlkVfioPci,
    #[qapi(name = "virtio-blk-vhost-user")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(since = "7.2")]
    VirtioBlkVhostUser,
    #[qapi(name = "virtio-blk-vhost-vdpa")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(since = "7.2")]
    VirtioBlkVhostVdpa,
}
#[qapi(since = "1.3")]
pub enum BlockdevOnError {
    #[qapi(name = "report")]
    Report,
    #[qapi(name = "ignore")]
    Ignore,
    #[qapi(name = "enospc")]
    Enospc,
    #[qapi(name = "stop")]
    Stop,
    #[qapi(name = "auto")]
    #[qapi(since = "2.7")]
    Auto,
}
#[qapi(since = "2.0")]
pub enum BlockdevOptionsBranch {
    #[qapi(name = "file")]
    File(BlockdevOptionsFile),
    #[qapi(name = "http")]
    Http(BlockdevOptionsFile),
    #[qapi(name = "https")]
    Https(BlockdevOptionsFile),
    #[qapi(name = "ftp")]
    Ftp(BlockdevOptionsFile),
    #[qapi(name = "ftps")]
    Ftps(BlockdevOptionsFile),
    #[qapi(name = "tftp")]
    #[qapi(until = "2.8")]
    Tftp(BlockdevOptionsFile),
    #[qapi(name = "vvfat")]
    Vvfat(BlockdevOptionsVvfat),
    #[qapi(name = "blkdebug")]
    Blkdebug(BlockdevOptionsBlkdebug),
    #[qapi(name = "blkverify")]
    Blkverify(BlockdevOptionsBlkverify),
    #[qapi(name = "bochs")]
    Bochs(BlockdevOptionsGenericFormat),
    #[qapi(name = "cloop")]
    Cloop(BlockdevOptionsGenericFormat),
    #[qapi(name = "cow")]
    #[qapi(until = "2.2")]
    Cow(BlockdevOptionsGenericCowFormat),
    #[qapi(name = "dmg")]
    Dmg(BlockdevOptionsGenericFormat),
    #[qapi(name = "parallels")]
    Parallels(BlockdevOptionsGenericFormat),
    #[qapi(name = "qcow")]
    Qcow(BlockdevOptionsGenericCowFormat),
    #[qapi(name = "qcow2")]
    Qcow2(BlockdevOptionsQcow2),
    #[qapi(name = "qed")]
    Qed(BlockdevOptionsGenericCowFormat),
    #[qapi(name = "raw")]
    Raw(BlockdevOptionsGenericFormat),
    #[qapi(name = "vdi")]
    Vdi(BlockdevOptionsGenericFormat),
    #[qapi(name = "vhdx")]
    Vhdx(BlockdevOptionsGenericFormat),
    #[qapi(name = "vmdk")]
    Vmdk(BlockdevOptionsGenericCowFormat),
    #[qapi(name = "vpc")]
    Vpc(BlockdevOptionsGenericFormat),
    #[qapi(name = "quorum")]
    Quorum(BlockdevOptionsQuorum),
    #[qapi(name = "host_device")]
    #[qapi(since = "2.1")]
    HostDevice(BlockdevOptionsFile),
    #[qapi(name = "host_cdrom")]
    #[qapi(since = "2.1")]
    HostCdrom(BlockdevOptionsFile),
    #[qapi(name = "host_floppy")]
    #[qapi(since = "2.1")]
    #[qapi(until = "2.5")]
    HostFloppy(BlockdevOptionsFile),
    #[qapi(name = "archipelago")]
    #[qapi(since = "2.2")]
    #[qapi(until = "2.9")]
    Archipelago(BlockdevOptionsArchipelago),
    #[qapi(name = "null-aio")]
    #[qapi(since = "2.2")]
    NullAio(BlockdevOptionsNull),
    #[qapi(name = "null-co")]
    #[qapi(since = "2.2")]
    NullCo(BlockdevOptionsNull),
    #[qapi(name = "luks")]
    #[qapi(since = "2.6")]
    Luks(BlockdevOptionsLuks),
    #[qapi(name = "gluster")]
    #[qapi(since = "2.7")]
    Gluster(BlockdevOptionsGluster),
    #[qapi(name = "nbd")]
    #[qapi(since = "2.8")]
    Nbd(BlockdevOptionsNbd),
    #[qapi(name = "nfs")]
    #[qapi(since = "2.8")]
    Nfs(BlockdevOptionsNfs),
    #[qapi(name = "replication")]
    #[qapi(since = "2.8")]
    Replication(BlockdevOptionsReplication),
    #[qapi(name = "ssh")]
    #[qapi(since = "2.8")]
    Ssh(BlockdevOptionsSsh),
    #[qapi(name = "iscsi")]
    #[qapi(since = "2.9")]
    Iscsi(BlockdevOptionsIscsi),
    #[qapi(name = "rbd")]
    #[qapi(since = "2.9")]
    Rbd(BlockdevOptionsRbd),
    #[qapi(name = "sheepdog")]
    #[qapi(since = "2.9")]
    #[qapi(until = "6.1")]
    Sheepdog(BlockdevOptionsSheepdog),
    #[qapi(name = "vxhs")]
    #[qapi(since = "2.10")]
    #[qapi(until = "5.1")]
    Vxhs(BlockdevOptionsVxHs),
    #[qapi(name = "throttle")]
    #[qapi(since = "2.11")]
    Throttle(BlockdevOptionsThrottle),
    #[qapi(name = "nvme")]
    #[qapi(since = "2.12")]
    Nvme(BlockdevOptionsNvMe),
    #[qapi(name = "blklogwrites")]
    #[qapi(since = "3.0")]
    Blklogwrites(BlockdevOptionsBlklogwrites),
    #[qapi(name = "copy-on-read")]
    #[qapi(since = "3.0")]
    CopyOnRead(BlockdevOptionsGenericFormat),
    #[qapi(name = "blkreplay")]
    #[qapi(since = "4.2")]
    Blkreplay(BlockdevOptionsBlkreplay),
    #[qapi(name = "compress")]
    #[qapi(since = "5.0")]
    Compress(BlockdevOptionsGenericFormat),
    #[qapi(name = "preallocate")]
    #[qapi(since = "6.0")]
    Preallocate(BlockdevOptionsPreallocate),
    #[qapi(name = "copy-before-write")]
    #[qapi(since = "6.2")]
    CopyBeforeWrite(BlockdevOptionsCbw),
    #[qapi(name = "snapshot-access")]
    #[qapi(since = "7.0")]
    SnapshotAccess(BlockdevOptionsGenericFormat),
    #[qapi(name = "io_uring")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(since = "7.2")]
    IoUring(BlockdevOptionsIoUring),
    #[qapi(name = "nvme-io_uring")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(since = "7.2")]
    NvmeIoUring(BlockdevOptionsNvmeIoUring),
    #[qapi(name = "virtio-blk-vfio-pci")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(since = "7.2")]
    VirtioBlkVfioPci(BlockdevOptionsVirtioBlkVfioPci),
    #[qapi(name = "virtio-blk-vhost-user")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(since = "7.2")]
    VirtioBlkVhostUser(BlockdevOptionsVirtioBlkVhostUser),
    #[qapi(name = "virtio-blk-vhost-vdpa")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(since = "7.2")]
    VirtioBlkVhostVdpa(BlockdevOptionsVirtioBlkVhostVdpa),
}
#[qapi(since = "2.10")]
pub enum BlockdevQcow2EncryptionBranch {
    #[qapi(name = "aes")]
    Aes(QCryptoBlockOptionsQCow),
    #[qapi(name = "luks")]
    Luks(QCryptoBlockOptionsLuks),
}
#[qapi(since = "2.10")]
pub enum BlockdevQcow2EncryptionFormat {
    #[qapi(name = "aes")]
    Aes,
    #[qapi(name = "luks")]
    Luks,
}
#[qapi(since = "2.12")]
pub enum BlockdevQcow2Version {
    #[qapi(name = "v2")]
    V2,
    #[qapi(name = "v3")]
    V3,
}
#[qapi(since = "2.10")]
pub enum BlockdevQcowEncryptionBranch {
    #[qapi(name = "aes")]
    Aes(QCryptoBlockOptionsQCow),
}
#[qapi(since = "2.10")]
pub enum BlockdevQcowEncryptionFormat {
    #[qapi(name = "aes")]
    Aes,
}
#[qapi(since = "1.7")]
pub enum BlockdevRef {
    #[qapi(name = "definition")]
    Definition(BlockdevOptions),
    #[qapi(name = "reference")]
    Reference(String),
}
#[qapi(since = "2.9")]
pub enum BlockdevRefOrNull {
    #[qapi(name = "definition")]
    Definition(BlockdevOptions),
    #[qapi(name = "reference")]
    Reference(String),
    #[qapi(name = "null")]
    Null(()),
}
#[qapi(since = "2.12")]
pub enum BlockdevVhdxSubformat {
    #[qapi(name = "dynamic")]
    Dynamic,
    #[qapi(name = "fixed")]
    Fixed,
}
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
#[qapi(since = "4.0")]
pub enum BlockdevVmdkSubformat {
    #[qapi(name = "monolithicSparse")]
    MonolithicSparse,
    #[qapi(name = "monolithicFlat")]
    MonolithicFlat,
    #[qapi(name = "twoGbMaxExtentSparse")]
    TwoGbMaxExtentSparse,
    #[qapi(name = "twoGbMaxExtentFlat")]
    TwoGbMaxExtentFlat,
    #[qapi(name = "streamOptimized")]
    StreamOptimized,
}
#[qapi(since = "2.12")]
pub enum BlockdevVpcSubformat {
    #[qapi(name = "dynamic")]
    Dynamic,
    #[qapi(name = "fixed")]
    Fixed,
}
#[qapi(name = "COLOExitReason")]
#[qapi(since = "3.1")]
pub enum ColoExitReason {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "request")]
    Request,
    #[qapi(name = "error")]
    Error,
    #[qapi(name = "processing")]
    #[qapi(since = "4.0")]
    Processing,
}
#[qapi(name = "COLOMessage")]
#[qapi(since = "2.8")]
pub enum ColoMessage {
    #[qapi(name = "checkpoint-ready")]
    CheckpointReady,
    #[qapi(name = "checkpoint-request")]
    CheckpointRequest,
    #[qapi(name = "checkpoint-reply")]
    CheckpointReply,
    #[qapi(name = "vmstate-send")]
    VmstateSend,
    #[qapi(name = "vmstate-size")]
    VmstateSize,
    #[qapi(name = "vmstate-received")]
    VmstateReceived,
    #[qapi(name = "vmstate-loaded")]
    VmstateLoaded,
}
#[qapi(name = "COLOMode")]
#[qapi(since = "2.8")]
pub enum ColoMode {
    #[qapi(name = "unknown")]
    #[qapi(until = "3.1")]
    Unknown,
    #[qapi(name = "primary")]
    Primary,
    #[qapi(name = "secondary")]
    Secondary,
    #[qapi(name = "none")]
    #[qapi(since = "3.1")]
    None,
}
#[qapi(since = "9.2")]
pub enum CacheLevelAndType {
    #[qapi(name = "l1d")]
    L1d,
    #[qapi(name = "l1i")]
    L1i,
    #[qapi(name = "l2")]
    L2,
    #[qapi(name = "l3")]
    L3,
}
#[qapi(since = "1.4")]
pub enum ChardevBackendBranch {
    #[qapi(name = "file")]
    File(ChardevFile),
    #[qapi(name = "serial")]
    Serial(ChardevHostdev),
    #[qapi(name = "parallel")]
    Parallel(ChardevHostdev),
    #[qapi(name = "socket")]
    Socket(ChardevSocket),
    #[qapi(name = "pty")]
    Pty(ChardevDummy),
    #[qapi(name = "null")]
    Null(ChardevDummy),
    #[qapi(name = "pipe")]
    #[qapi(since = "1.5")]
    Pipe(ChardevHostdev),
    #[qapi(name = "udp")]
    #[qapi(since = "1.5")]
    Udp(ChardevUdp),
    #[qapi(name = "mux")]
    #[qapi(since = "1.5")]
    Mux(ChardevMux),
    #[qapi(name = "msmouse")]
    #[qapi(since = "1.5")]
    Msmouse(ChardevDummy),
    #[qapi(name = "braille")]
    #[qapi(since = "1.5")]
    Braille(ChardevDummy),
    #[qapi(name = "stdio")]
    #[qapi(since = "1.5")]
    Stdio(ChardevStdio),
    #[qapi(name = "console")]
    #[qapi(since = "1.5")]
    Console(ChardevDummy),
    #[qapi(name = "spicevmc")]
    #[qapi(since = "1.5")]
    Spicevmc(ChardevSpiceChannel),
    #[qapi(name = "spiceport")]
    #[qapi(since = "1.5")]
    Spiceport(ChardevSpicePort),
    #[qapi(name = "vc")]
    #[qapi(since = "1.5")]
    Vc(ChardevVc),
    #[qapi(name = "memory")]
    #[qapi(since = "1.5")]
    Memory(ChardevRingbuf),
    #[qapi(name = "ringbuf")]
    #[qapi(since = "2.0")]
    Ringbuf(ChardevRingbuf),
    #[qapi(name = "testdev")]
    #[qapi(since = "2.2")]
    Testdev(ChardevDummy),
    #[qapi(name = "wctablet")]
    #[qapi(since = "2.9")]
    Wctablet(ChardevCommon),
    #[qapi(name = "qemu-vdagent")]
    #[qapi(condition = "defined(CONFIG_SPICE_PROTOCOL)")]
    #[qapi(since = "6.1")]
    QemuVdagent(ChardevQemuVdAgent),
    #[qapi(name = "dbus")]
    #[qapi(condition = "CONFIG_DBUS_DISPLAY")]
    #[qapi(since = "7.0")]
    Dbus(ChardevDBusWrapper),
    #[qapi(name = "hub")]
    #[qapi(since = "10.0")]
    Hub(ChardevHubWrapper),
}
#[qapi(since = "1.4")]
pub enum ChardevBackendKind {
    #[qapi(name = "file")]
    File,
    #[qapi(name = "serial")]
    Serial,
    #[qapi(name = "parallel")]
    Parallel,
    #[qapi(name = "socket")]
    Socket,
    #[qapi(name = "pty")]
    Pty,
    #[qapi(name = "null")]
    Null,
    #[qapi(name = "pipe")]
    #[qapi(since = "1.5")]
    Pipe,
    #[qapi(name = "udp")]
    #[qapi(since = "1.5")]
    Udp,
    #[qapi(name = "mux")]
    #[qapi(since = "1.5")]
    Mux,
    #[qapi(name = "msmouse")]
    #[qapi(since = "1.5")]
    Msmouse,
    #[qapi(name = "braille")]
    #[qapi(since = "1.5")]
    Braille,
    #[qapi(name = "stdio")]
    #[qapi(since = "1.5")]
    Stdio,
    #[qapi(name = "console")]
    #[qapi(since = "1.5")]
    Console,
    #[qapi(name = "spicevmc")]
    #[qapi(since = "1.5")]
    Spicevmc,
    #[qapi(name = "spiceport")]
    #[qapi(since = "1.5")]
    Spiceport,
    #[qapi(name = "vc")]
    #[qapi(since = "1.5")]
    Vc,
    #[qapi(name = "memory")]
    #[qapi(since = "1.5")]
    Memory,
    #[qapi(name = "ringbuf")]
    #[qapi(since = "2.0")]
    Ringbuf,
    #[qapi(name = "testdev")]
    #[qapi(since = "2.2")]
    Testdev,
    #[qapi(name = "wctablet")]
    #[qapi(since = "2.9")]
    Wctablet,
    #[qapi(name = "type")]
    #[qapi(since = "4.0")]
    #[qapi(until = "6.2")]
    Type,
    #[qapi(name = "if")]
    #[qapi(since = "4.0")]
    #[qapi(until = "6.2")]
    If,
    #[qapi(name = "qemu-vdagent")]
    #[qapi(since = "6.1")]
    QemuVdagent,
    #[qapi(name = "dbus")]
    #[qapi(condition = "CONFIG_DBUS_DISPLAY")]
    #[qapi(since = "7.0")]
    Dbus,
    #[qapi(name = "hub")]
    #[qapi(since = "10.0")]
    Hub,
}
#[qapi(since = "2.12")]
#[qapi(until = "4.0")]
pub enum CommandDropReason {
    #[qapi(name = "queue-full")]
    QueueFull,
}
#[qapi(since = "1.5")]
pub enum CommandLineParameterType {
    #[qapi(name = "string")]
    String,
    #[qapi(name = "boolean")]
    Boolean,
    #[qapi(name = "number")]
    Number,
    #[qapi(name = "size")]
    Size,
}
#[qapi(since = "6.0")]
pub enum CompatPolicyInput {
    #[qapi(name = "accept")]
    Accept,
    #[qapi(name = "reject")]
    Reject,
    #[qapi(name = "crash")]
    Crash,
}
#[qapi(since = "6.0")]
pub enum CompatPolicyOutput {
    #[qapi(name = "accept")]
    Accept,
    #[qapi(name = "hide")]
    Hide,
}
#[qapi(since = "2.6")]
#[qapi(until = "6.0")]
pub enum CpuInfoArch {
    #[qapi(name = "x86")]
    X86,
    #[qapi(name = "sparc")]
    Sparc,
    #[qapi(name = "ppc")]
    Ppc,
    #[qapi(name = "mips")]
    Mips,
    #[qapi(name = "tricore")]
    Tricore,
    #[qapi(name = "other")]
    Other,
    #[qapi(name = "s390")]
    #[qapi(since = "2.12")]
    S390,
    #[qapi(name = "riscv")]
    #[qapi(since = "2.12")]
    Riscv,
}
#[qapi(since = "2.6")]
#[qapi(until = "6.0")]
pub enum CpuInfoBranch {
    #[qapi(name = "x86")]
    X86(CpuInfoX86),
    #[qapi(name = "sparc")]
    Sparc(CpuInfoSparc),
    #[qapi(name = "ppc")]
    Ppc(CpuInfoPpc),
    #[qapi(name = "mips")]
    Mips(CpuInfoMips),
    #[qapi(name = "tricore")]
    Tricore(CpuInfoTricore),
    #[qapi(name = "other")]
    #[qapi(until = "3.0")]
    Other(CpuInfoOther),
    #[qapi(name = "s390")]
    #[qapi(since = "2.12")]
    S390(CpuInfoS390),
    #[qapi(name = "riscv")]
    #[qapi(since = "2.12")]
    Riscv(CpuInfoRiscv),
}
#[qapi(since = "2.12")]
pub enum CpuInfoFastBranch {
    #[qapi(name = "x86")]
    #[qapi(until = "3.0")]
    X86(CpuInfoOther),
    #[qapi(name = "sparc")]
    #[qapi(until = "3.0")]
    Sparc(CpuInfoOther),
    #[qapi(name = "ppc")]
    #[qapi(until = "3.0")]
    Ppc(CpuInfoOther),
    #[qapi(name = "mips")]
    #[qapi(until = "3.0")]
    Mips(CpuInfoOther),
    #[qapi(name = "tricore")]
    #[qapi(until = "3.0")]
    Tricore(CpuInfoOther),
    #[qapi(name = "s390")]
    #[qapi(until = "3.0")]
    S390(CpuInfoS390),
    #[qapi(name = "riscv")]
    #[qapi(until = "3.0")]
    Riscv(CpuInfoRiscv),
    #[qapi(name = "other")]
    #[qapi(until = "3.0")]
    Other(CpuInfoOther),
    #[qapi(name = "s390x")]
    #[qapi(since = "3.0")]
    S390x(CpuInfoS390),
}
#[qapi(since = "2.8.0")]
pub enum CpuModelCompareResult {
    #[qapi(name = "incompatible")]
    Incompatible,
    #[qapi(name = "identical")]
    Identical,
    #[qapi(name = "superset")]
    Superset,
    #[qapi(name = "subset")]
    Subset,
}
#[qapi(since = "2.8.0")]
pub enum CpuModelExpansionType {
    #[qapi(name = "static")]
    Static,
    #[qapi(name = "full")]
    Full,
}
#[qapi(since = "8.2")]
#[qapi(until = "9.2")]
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
#[qapi(condition = "TARGET_S390X")]
#[qapi(since = "8.2")]
#[qapi(until = "9.2")]
pub enum CpuS390Polarization {
    #[qapi(name = "horizontal")]
    Horizontal,
    #[qapi(name = "vertical")]
    Vertical,
}
#[qapi(since = "2.12")]
#[qapi(until = "9.2")]
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
#[qapi(since = "9.2")]
pub enum CpuTopologyLevel {
    #[qapi(name = "thread")]
    Thread,
    #[qapi(name = "core")]
    Core,
    #[qapi(name = "module")]
    Module,
    #[qapi(name = "cluster")]
    Cluster,
    #[qapi(name = "die")]
    Die,
    #[qapi(name = "socket")]
    Socket,
    #[qapi(name = "book")]
    Book,
    #[qapi(name = "drawer")]
    Drawer,
    #[qapi(name = "default")]
    Default,
}
#[qapi(since = "8.0")]
pub enum CxlCorErrorType {
    #[qapi(name = "cache-data-ecc")]
    CacheDataEcc,
    #[qapi(name = "mem-data-ecc")]
    MemDataEcc,
    #[qapi(name = "crc-threshold")]
    CrcThreshold,
    #[qapi(name = "retry-threshold")]
    RetryThreshold,
    #[qapi(name = "cache-poison-received")]
    CachePoisonReceived,
    #[qapi(name = "mem-poison-received")]
    MemPoisonReceived,
    #[qapi(name = "physical")]
    Physical,
}
#[qapi(since = "8.1")]
pub enum CxlEventLog {
    #[qapi(name = "informational")]
    Informational,
    #[qapi(name = "warning")]
    Warning,
    #[qapi(name = "failure")]
    Failure,
    #[qapi(name = "fatal")]
    Fatal,
}
#[qapi(since = "9.1")]
pub enum CxlExtentRemovalPolicy {
    #[qapi(name = "tag-based")]
    TagBased,
    #[qapi(name = "prescriptive")]
    Prescriptive,
}
#[qapi(since = "9.1")]
pub enum CxlExtentSelectionPolicy {
    #[qapi(name = "free")]
    Free,
    #[qapi(name = "contiguous")]
    Contiguous,
    #[qapi(name = "prescriptive")]
    Prescriptive,
    #[qapi(name = "enable-shared-access")]
    EnableSharedAccess,
}
#[qapi(since = "8.0")]
pub enum CxlUncorErrorType {
    #[qapi(name = "cache-data-parity")]
    CacheDataParity,
    #[qapi(name = "cache-address-parity")]
    CacheAddressParity,
    #[qapi(name = "cache-be-parity")]
    CacheBeParity,
    #[qapi(name = "cache-data-ecc")]
    CacheDataEcc,
    #[qapi(name = "mem-data-parity")]
    MemDataParity,
    #[qapi(name = "mem-address-parity")]
    MemAddressParity,
    #[qapi(name = "mem-be-parity")]
    MemBeParity,
    #[qapi(name = "mem-data-ecc")]
    MemDataEcc,
    #[qapi(name = "reinit-threshold")]
    ReinitThreshold,
    #[qapi(name = "rsvd-encoding")]
    RsvdEncoding,
    #[qapi(name = "poison-received")]
    PoisonReceived,
    #[qapi(name = "receiver-overflow")]
    ReceiverOverflow,
    #[qapi(name = "internal")]
    Internal,
    #[qapi(name = "cxl-ide-tx")]
    CxlIdeTx,
    #[qapi(name = "cxl-ide-rx")]
    CxlIdeRx,
}
#[qapi(since = "1.4")]
pub enum DataFormat {
    #[qapi(name = "utf8")]
    Utf8,
    #[qapi(name = "base64")]
    Base64,
}
#[qapi(since = "2.4")]
#[qapi(until = "6.0")]
pub enum DirtyBitmapStatus {
    #[qapi(name = "active")]
    Active,
    #[qapi(name = "disabled")]
    Disabled,
    #[qapi(name = "frozen")]
    Frozen,
    #[qapi(name = "locked")]
    #[qapi(since = "2.12")]
    Locked,
    #[qapi(name = "inconsistent")]
    #[qapi(since = "4.0")]
    Inconsistent,
}
#[qapi(since = "6.2")]
pub enum DirtyRateMeasureMode {
    #[qapi(name = "page-sampling")]
    PageSampling,
    #[qapi(name = "dirty-ring")]
    DirtyRing,
    #[qapi(name = "dirty-bitmap")]
    DirtyBitmap,
}
#[qapi(since = "5.2")]
pub enum DirtyRateStatus {
    #[qapi(name = "unstarted")]
    Unstarted,
    #[qapi(name = "measuring")]
    Measuring,
    #[qapi(name = "measured")]
    Measured,
}
#[qapi(name = "DisplayGLMode")]
#[qapi(since = "3.0")]
pub enum DisplayGlMode {
    #[qapi(name = "off")]
    Off,
    #[qapi(name = "on")]
    On,
    #[qapi(name = "core")]
    Core,
    #[qapi(name = "es")]
    Es,
}
#[qapi(since = "2.12")]
pub enum DisplayOptionsBranch {
    #[qapi(name = "default")]
    #[qapi(until = "3.0")]
    Default(DisplayNoOpts),
    #[qapi(name = "none")]
    #[qapi(until = "3.0")]
    None(DisplayNoOpts),
    #[qapi(name = "gtk")]
    Gtk(DisplayGtk),
    #[qapi(name = "sdl")]
    Sdl(DisplayNoOpts),
    #[qapi(name = "egl-headless")]
    EglHeadless(DisplayNoOpts),
    #[qapi(name = "curses")]
    Curses(DisplayNoOpts),
    #[qapi(name = "cocoa")]
    Cocoa(DisplayNoOpts),
    #[qapi(name = "dbus")]
    #[qapi(condition = "CONFIG_DBUS_DISPLAY")]
    #[qapi(since = "7.0")]
    Dbus(DisplayDBus),
}
#[qapi(since = "7.0")]
pub enum DisplayProtocol {
    #[qapi(name = "vnc")]
    Vnc,
    #[qapi(name = "spice")]
    Spice,
}
#[qapi(since = "6.0")]
pub enum DisplayReloadOptionsBranch {
    #[qapi(name = "vnc")]
    Vnc(DisplayReloadOptionsVnc),
}
#[qapi(since = "6.0")]
pub enum DisplayReloadType {
    #[qapi(name = "vnc")]
    Vnc,
}
#[qapi(since = "2.12")]
pub enum DisplayType {
    #[qapi(name = "default")]
    Default,
    #[qapi(name = "none")]
    None,
    #[qapi(name = "gtk")]
    Gtk,
    #[qapi(name = "sdl")]
    Sdl,
    #[qapi(name = "egl-headless")]
    EglHeadless,
    #[qapi(name = "curses")]
    Curses,
    #[qapi(name = "cocoa")]
    Cocoa,
    #[qapi(name = "spice-app")]
    #[qapi(since = "4.0")]
    SpiceApp,
    #[qapi(name = "dbus")]
    #[qapi(condition = "CONFIG_DBUS_DISPLAY")]
    #[qapi(since = "7.0")]
    Dbus,
}
#[qapi(since = "7.1")]
pub enum DisplayUpdateOptionsBranch {
    #[qapi(name = "vnc")]
    Vnc(DisplayUpdateOptionsVnc),
}
#[qapi(since = "7.1")]
pub enum DisplayUpdateType {
    #[qapi(name = "vnc")]
    Vnc,
}
#[qapi(since = "2.0")]
pub enum DumpGuestMemoryFormat {
    #[qapi(name = "elf")]
    Elf,
    #[qapi(name = "kdump-zlib")]
    KdumpZlib,
    #[qapi(name = "kdump-lzo")]
    KdumpLzo,
    #[qapi(name = "kdump-snappy")]
    KdumpSnappy,
    #[qapi(name = "win-dmp")]
    #[qapi(since = "3.0")]
    WinDmp,
    #[qapi(name = "kdump-raw-zlib")]
    #[qapi(since = "8.2")]
    KdumpRawZlib,
    #[qapi(name = "kdump-raw-lzo")]
    #[qapi(since = "8.2")]
    KdumpRawLzo,
    #[qapi(name = "kdump-raw-snappy")]
    #[qapi(since = "8.2")]
    KdumpRawSnappy,
}
#[qapi(since = "2.6")]
pub enum DumpStatus {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "active")]
    Active,
    #[qapi(name = "completed")]
    Completed,
    #[qapi(name = "failed")]
    Failed,
}
#[qapi(name = "EbpfProgramID")]
#[qapi(condition = "CONFIG_EBPF")]
#[qapi(since = "9.0")]
pub enum EbpfProgramId {
    #[qapi(name = "rss")]
    Rss,
}
#[qapi(since = "10.0")]
pub enum EndianMode {
    #[qapi(name = "unspecified")]
    Unspecified,
    #[qapi(name = "little")]
    Little,
    #[qapi(name = "big")]
    Big,
}
#[qapi(since = "1.2")]
#[qapi(until = "2.6")]
pub enum ErrorClass {
    GenericError,
    CommandNotFound,
    DeviceEncrypted,
    DeviceNotActive,
    DeviceNotFound,
    #[qapi(name = "KVMMissingCap")]
    KvmMissingCap,
    #[qapi(until = "1.3")]
    MigrationExpected,
}
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "8.0")]
pub enum EvtchnPortType {
    #[qapi(name = "closed")]
    Closed,
    #[qapi(name = "unbound")]
    Unbound,
    #[qapi(name = "interdomain")]
    Interdomain,
    #[qapi(name = "pirq")]
    Pirq,
    #[qapi(name = "virq")]
    Virq,
    #[qapi(name = "ipi")]
    Ipi,
}
#[qapi(since = "7.0")]
pub enum ExpirePasswordOptionsBranch {
    #[qapi(name = "vnc")]
    Vnc(ExpirePasswordOptionsVnc),
}
#[qapi(since = "2.8")]
pub enum FailoverStatus {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "require")]
    Require,
    #[qapi(name = "active")]
    Active,
    #[qapi(name = "completed")]
    Completed,
    #[qapi(name = "relaunch")]
    #[qapi(since = "2.9")]
    Relaunch,
}
#[qapi(since = "2.6")]
pub enum FloppyDriveType {
    #[qapi(name = "144")]
    _144,
    #[qapi(name = "288")]
    _288,
    #[qapi(name = "120")]
    _120,
    #[qapi(name = "none")]
    None,
    #[qapi(name = "auto")]
    Auto,
}
#[qapi(since = "6.1")]
pub enum FuseExportAllowOther {
    #[qapi(name = "off")]
    Off,
    #[qapi(name = "on")]
    On,
    #[qapi(name = "auto")]
    Auto,
}
#[qapi(since = "2.7")]
#[qapi(until = "2.9")]
pub enum GlusterServerBranch {
    #[qapi(name = "unix")]
    Unix(UnixSocketAddress),
    #[qapi(name = "tcp")]
    Tcp(InetSocketAddress),
}
#[qapi(since = "2.7")]
#[qapi(until = "2.9")]
pub enum GlusterTransport {
    #[qapi(name = "unix")]
    Unix,
    #[qapi(name = "tcp")]
    Tcp,
}
#[qapi(since = "4.0")]
pub enum GrabToggleKeys {
    #[qapi(name = "ctrl-ctrl")]
    CtrlCtrl,
    #[qapi(name = "alt-alt")]
    AltAlt,
    #[qapi(name = "meta-meta")]
    MetaMeta,
    #[qapi(name = "scrolllock")]
    Scrolllock,
    #[qapi(name = "ctrl-scrolllock")]
    CtrlScrolllock,
    #[qapi(name = "shift-shift")]
    #[qapi(since = "4.2")]
    ShiftShift,
}
#[qapi(since = "9.0")]
pub enum GranuleMode {
    #[qapi(name = "4k")]
    _4k,
    #[qapi(name = "8k")]
    _8k,
    #[qapi(name = "16k")]
    _16k,
    #[qapi(name = "64k")]
    _64k,
    #[qapi(name = "host")]
    Host,
}
#[qapi(since = "2.1")]
pub enum GuestPanicAction {
    #[qapi(name = "pause")]
    Pause,
    #[qapi(name = "poweroff")]
    #[qapi(since = "2.8")]
    Poweroff,
    #[qapi(name = "run")]
    #[qapi(since = "5.0")]
    Run,
}
#[qapi(since = "2.9")]
pub enum GuestPanicInformationBranch {
    #[qapi(name = "hyper-v")]
    HyperV(GuestPanicInformationHyperV),
    #[qapi(name = "s390")]
    #[qapi(since = "2.12")]
    S390(GuestPanicInformationS390),
    #[qapi(name = "tdx")]
    #[qapi(since = "10.1")]
    Tdx(GuestPanicInformationTdx),
}
#[qapi(since = "2.9")]
pub enum GuestPanicInformationType {
    #[qapi(name = "hyper-v")]
    HyperV,
    #[qapi(name = "s390")]
    #[qapi(since = "2.12")]
    S390,
    #[qapi(name = "tdx")]
    #[qapi(since = "10.1")]
    Tdx,
}
#[qapi(since = "5.0")]
pub enum HmatCacheAssociativity {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "direct")]
    Direct,
    #[qapi(name = "complex")]
    Complex,
}
#[qapi(since = "5.0")]
pub enum HmatCacheWritePolicy {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "write-back")]
    WriteBack,
    #[qapi(name = "write-through")]
    WriteThrough,
}
#[qapi(name = "HmatLBDataType")]
#[qapi(since = "5.0")]
pub enum HmatLbDataType {
    #[qapi(name = "access-latency")]
    AccessLatency,
    #[qapi(name = "read-latency")]
    ReadLatency,
    #[qapi(name = "write-latency")]
    WriteLatency,
    #[qapi(name = "access-bandwidth")]
    AccessBandwidth,
    #[qapi(name = "read-bandwidth")]
    ReadBandwidth,
    #[qapi(name = "write-bandwidth")]
    WriteBandwidth,
}
#[qapi(name = "HmatLBMemoryHierarchy")]
#[qapi(since = "5.0")]
pub enum HmatLbMemoryHierarchy {
    #[qapi(name = "memory")]
    Memory,
    #[qapi(name = "first-level")]
    FirstLevel,
    #[qapi(name = "second-level")]
    SecondLevel,
    #[qapi(name = "third-level")]
    ThirdLevel,
}
#[qapi(since = "2.1")]
pub enum HostMemPolicy {
    #[qapi(name = "default")]
    Default,
    #[qapi(name = "preferred")]
    Preferred,
    #[qapi(name = "bind")]
    Bind,
    #[qapi(name = "interleave")]
    Interleave,
}
#[qapi(since = "7.1")]
pub enum HotKeyMod {
    #[qapi(name = "lctrl-lalt")]
    LctrlLalt,
    #[qapi(name = "lshift-lctrl-lalt")]
    LshiftLctrlLalt,
    #[qapi(name = "rctrl")]
    Rctrl,
}
#[qapi(since = "7.1")]
pub enum ImageFormat {
    #[qapi(name = "ppm")]
    Ppm,
    #[qapi(name = "png")]
    Png,
}
#[qapi(since = "1.7")]
pub enum ImageInfoSpecificBranch {
    #[qapi(name = "qcow2")]
    Qcow2(ImageInfoSpecificQCow2),
    #[qapi(name = "vmdk")]
    Vmdk(ImageInfoSpecificVmdk),
    #[qapi(name = "luks")]
    #[qapi(since = "2.7")]
    Luks(QCryptoBlockInfoLuks),
    #[qapi(name = "rbd")]
    #[qapi(since = "6.1")]
    Rbd(ImageInfoSpecificRbd),
    #[qapi(name = "file")]
    #[qapi(since = "8.0")]
    File(ImageInfoSpecificFileWrapper),
}
#[qapi(since = "1.7")]
pub enum ImageInfoSpecificKind {
    #[qapi(name = "qcow2")]
    Qcow2,
    #[qapi(name = "vmdk")]
    Vmdk,
    #[qapi(name = "luks")]
    #[qapi(since = "2.7")]
    Luks,
    #[qapi(name = "rbd")]
    #[qapi(since = "6.1")]
    Rbd,
    #[qapi(name = "file")]
    #[qapi(since = "8.0")]
    File,
}
#[qapi(since = "2.10")]
pub enum ImageInfoSpecificQCow2EncryptionBranch {
    #[qapi(name = "aes")]
    #[qapi(until = "3.0")]
    Aes(QCryptoBlockInfoQCow),
    #[qapi(name = "luks")]
    Luks(QCryptoBlockInfoLuks),
}
#[qapi(since = "2.0")]
pub enum InputAxis {
    #[qapi(name = "x")]
    X,
    #[qapi(name = "y")]
    Y,
}
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
    #[qapi(name = "side")]
    #[qapi(since = "2.9")]
    Side,
    #[qapi(name = "extra")]
    #[qapi(since = "2.9")]
    Extra,
    #[qapi(name = "wheel-left")]
    #[qapi(since = "7.0")]
    WheelLeft,
    #[qapi(name = "wheel-right")]
    #[qapi(since = "7.0")]
    WheelRight,
    #[qapi(name = "touch")]
    #[qapi(since = "8.1")]
    Touch,
}
#[qapi(since = "2.0")]
pub enum InputEventBranch {
    #[qapi(name = "key")]
    Key(InputKeyEvent),
    #[qapi(name = "btn")]
    Btn(InputBtnEvent),
    #[qapi(name = "rel")]
    Rel(InputMoveEvent),
    #[qapi(name = "abs")]
    Abs(InputMoveEvent),
    #[qapi(name = "mtt")]
    #[qapi(since = "8.1")]
    Mtt(InputMultiTouchEventWrapper),
}
#[qapi(since = "2.0")]
pub enum InputEventKind {
    #[qapi(name = "key")]
    Key,
    #[qapi(name = "btn")]
    Btn,
    #[qapi(name = "rel")]
    Rel,
    #[qapi(name = "abs")]
    Abs,
    #[qapi(name = "mtt")]
    #[qapi(since = "8.1")]
    Mtt,
}
#[qapi(since = "8.1")]
pub enum InputMultiTouchType {
    #[qapi(name = "begin")]
    Begin,
    #[qapi(name = "update")]
    Update,
    #[qapi(name = "end")]
    End,
    #[qapi(name = "cancel")]
    Cancel,
    #[qapi(name = "data")]
    Data,
}
#[qapi(since = "2.1")]
pub enum IoOperationType {
    #[qapi(name = "read")]
    Read,
    #[qapi(name = "write")]
    Write,
}
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
#[qapi(since = "2.9")]
pub enum IscsiTransport {
    #[qapi(name = "tcp")]
    Tcp,
    #[qapi(name = "iser")]
    Iser,
}
#[qapi(name = "JSONType")]
#[qapi(since = "2.5")]
pub enum JsonType {
    #[qapi(name = "string")]
    String,
    #[qapi(name = "number")]
    Number,
    #[qapi(name = "int")]
    Int,
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
#[qapi(since = "2.12")]
pub enum JobStatus {
    #[qapi(name = "undefined")]
    Undefined,
    #[qapi(name = "created")]
    Created,
    #[qapi(name = "running")]
    Running,
    #[qapi(name = "paused")]
    Paused,
    #[qapi(name = "ready")]
    Ready,
    #[qapi(name = "standby")]
    Standby,
    #[qapi(name = "waiting")]
    Waiting,
    #[qapi(name = "pending")]
    Pending,
    #[qapi(name = "aborting")]
    Aborting,
    #[qapi(name = "concluded")]
    Concluded,
    #[qapi(name = "null")]
    Null,
}
#[qapi(since = "1.7")]
pub enum JobType {
    #[qapi(name = "commit")]
    Commit,
    #[qapi(name = "stream")]
    Stream,
    #[qapi(name = "mirror")]
    Mirror,
    #[qapi(name = "backup")]
    Backup,
    #[qapi(name = "create")]
    Create,
    #[qapi(name = "amend")]
    #[qapi(since = "5.1")]
    Amend,
    #[qapi(name = "snapshot-load")]
    #[qapi(since = "6.0")]
    SnapshotLoad,
    #[qapi(name = "snapshot-save")]
    #[qapi(since = "6.0")]
    SnapshotSave,
    #[qapi(name = "snapshot-delete")]
    #[qapi(since = "6.0")]
    SnapshotDelete,
}
#[qapi(since = "2.12")]
pub enum JobVerb {
    #[qapi(name = "cancel")]
    Cancel,
    #[qapi(name = "pause")]
    Pause,
    #[qapi(name = "resume")]
    Resume,
    #[qapi(name = "set-speed")]
    SetSpeed,
    #[qapi(name = "complete")]
    Complete,
    #[qapi(name = "dismiss")]
    Dismiss,
    #[qapi(name = "finalize")]
    Finalize,
    #[qapi(name = "change")]
    #[qapi(since = "8.2")]
    Change,
}
#[qapi(since = "1.3")]
pub enum KeyValueBranch {
    #[qapi(name = "number")]
    Number(i64),
    #[qapi(name = "qcode")]
    Qcode(QKeyCode),
}
#[qapi(since = "1.3.0")]
pub enum KeyValueKind {
    #[qapi(name = "number")]
    Number,
    #[qapi(name = "qcode")]
    Qcode,
}
#[qapi(since = "2.0")]
pub enum LostTickPolicy {
    #[qapi(name = "discard")]
    Discard,
    #[qapi(name = "delay")]
    Delay,
    #[qapi(name = "merge")]
    #[qapi(until = "4.1")]
    Merge,
    #[qapi(name = "slew")]
    Slew,
}
#[qapi(since = "2.1")]
pub enum MemoryDeviceInfoBranch {
    #[qapi(name = "dimm")]
    Dimm(PcdimmDeviceInfo),
    #[qapi(name = "nvdimm")]
    #[qapi(since = "2.12")]
    Nvdimm(PcdimmDeviceInfo),
    #[qapi(name = "virtio-pmem")]
    #[qapi(since = "4.1")]
    VirtioPmem(VirtioPmemDeviceInfo),
    #[qapi(name = "virtio-mem")]
    #[qapi(since = "5.1")]
    VirtioMem(VirtioMemDeviceInfo),
    #[qapi(name = "sgx-epc")]
    #[qapi(since = "6.2")]
    SgxEpc(SgxEpcDeviceInfoWrapper),
    #[qapi(name = "hv-balloon")]
    #[qapi(since = "8.2")]
    HvBalloon(HvBalloonDeviceInfoWrapper),
}
#[qapi(since = "2.1")]
pub enum MemoryDeviceInfoKind {
    #[qapi(name = "dimm")]
    Dimm,
    #[qapi(name = "nvdimm")]
    #[qapi(since = "2.12")]
    Nvdimm,
    #[qapi(name = "virtio-pmem")]
    #[qapi(since = "4.1")]
    VirtioPmem,
    #[qapi(name = "virtio-mem")]
    #[qapi(since = "5.1")]
    VirtioMem,
    #[qapi(name = "sgx-epc")]
    #[qapi(since = "6.2")]
    SgxEpc,
    #[qapi(name = "hv-balloon")]
    #[qapi(since = "8.2")]
    HvBalloon,
}
#[qapi(since = "5.2")]
pub enum MemoryFailureAction {
    #[qapi(name = "ignore")]
    Ignore,
    #[qapi(name = "inject")]
    Inject,
    #[qapi(name = "fatal")]
    Fatal,
    #[qapi(name = "reset")]
    Reset,
}
#[qapi(since = "5.2")]
pub enum MemoryFailureRecipient {
    #[qapi(name = "hypervisor")]
    Hypervisor,
    #[qapi(name = "guest")]
    Guest,
}
#[qapi(since = "8.2")]
pub enum MigMode {
    #[qapi(name = "normal")]
    Normal,
    #[qapi(name = "cpr-reboot")]
    CprReboot,
    #[qapi(name = "cpr-transfer")]
    #[qapi(since = "10.0")]
    CprTransfer,
    #[qapi(name = "cpr-exec")]
    #[qapi(since = "10.2")]
    CprExec,
}
#[qapi(since = "8.2")]
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
#[qapi(since = "8.2")]
pub enum MigrationAddressType {
    #[qapi(name = "socket")]
    Socket,
    #[qapi(name = "exec")]
    Exec,
    #[qapi(name = "rdma")]
    Rdma,
    #[qapi(name = "file")]
    File,
}
#[qapi(since = "1.2")]
pub enum MigrationCapability {
    #[qapi(name = "xbzrle")]
    Xbzrle,
    #[qapi(name = "x-rdma-pin-all")]
    #[qapi(since = "1.6")]
    #[qapi(until = "2.3")]
    XRdmaPinAll,
    #[qapi(name = "auto-converge")]
    #[qapi(since = "1.6")]
    AutoConverge,
    #[qapi(name = "zero-blocks")]
    #[qapi(since = "1.6")]
    ZeroBlocks,
    #[qapi(name = "rdma-pin-all")]
    #[qapi(since = "2.3")]
    RdmaPinAll,
    #[qapi(name = "compress")]
    #[qapi(since = "2.4")]
    #[qapi(until = "9.1")]
    Compress,
    #[qapi(name = "events")]
    #[qapi(since = "2.4")]
    Events,
    #[qapi(name = "x-postcopy-ram")]
    #[qapi(since = "2.5")]
    #[qapi(until = "2.6")]
    XPostcopyRam,
    #[qapi(name = "postcopy-ram")]
    #[qapi(since = "2.6")]
    PostcopyRam,
    #[qapi(name = "x-colo")]
    #[qapi(since = "2.8")]
    XColo,
    #[qapi(name = "release-ram")]
    #[qapi(since = "2.9")]
    ReleaseRam,
    #[qapi(name = "block")]
    #[qapi(since = "2.10")]
    #[qapi(until = "9.1")]
    Block,
    #[qapi(name = "return-path")]
    #[qapi(since = "2.10")]
    ReturnPath,
    #[qapi(name = "pause-before-switchover")]
    #[qapi(since = "2.11")]
    PauseBeforeSwitchover,
    #[qapi(name = "x-multifd")]
    #[qapi(since = "2.11")]
    #[qapi(until = "4.0")]
    XMultifd,
    #[qapi(name = "dirty-bitmaps")]
    #[qapi(since = "2.12")]
    DirtyBitmaps,
    #[qapi(name = "postcopy-blocktime")]
    #[qapi(since = "3.0")]
    PostcopyBlocktime,
    #[qapi(name = "late-block-activate")]
    #[qapi(since = "3.0")]
    LateBlockActivate,
    #[qapi(name = "multifd")]
    #[qapi(since = "4.0")]
    Multifd,
    #[qapi(name = "x-ignore-shared")]
    #[qapi(since = "4.0")]
    XIgnoreShared,
    #[qapi(name = "validate-uuid")]
    #[qapi(since = "4.2")]
    ValidateUuid,
    #[qapi(name = "background-snapshot")]
    #[qapi(since = "6.0")]
    BackgroundSnapshot,
    #[qapi(name = "zero-copy-send")]
    #[qapi(since = "7.1")]
    ZeroCopySend,
    #[qapi(name = "postcopy-preempt")]
    #[qapi(since = "7.1")]
    PostcopyPreempt,
    #[qapi(name = "switchover-ack")]
    #[qapi(since = "8.1")]
    SwitchoverAck,
    #[qapi(name = "dirty-limit")]
    #[qapi(since = "8.1")]
    DirtyLimit,
    #[qapi(name = "mapped-ram")]
    #[qapi(since = "9.0")]
    MappedRam,
}
#[qapi(since = "8.1")]
pub enum MigrationChannelType {
    #[qapi(name = "main")]
    Main,
    #[qapi(name = "cpr")]
    #[qapi(since = "10.0")]
    Cpr,
}
#[qapi(since = "2.4")]
pub enum MigrationParameter {
    #[qapi(name = "compress-level")]
    #[qapi(until = "9.1")]
    CompressLevel,
    #[qapi(name = "compress-threads")]
    #[qapi(until = "9.1")]
    CompressThreads,
    #[qapi(name = "decompress-threads")]
    #[qapi(until = "9.1")]
    DecompressThreads,
    #[qapi(name = "cpu-throttle-initial")]
    CpuThrottleInitial,
    #[qapi(name = "cpu-throttle-increment")]
    CpuThrottleIncrement,
    #[qapi(name = "tls-creds")]
    TlsCreds,
    #[qapi(name = "tls-hostname")]
    TlsHostname,
    #[qapi(name = "max-bandwidth")]
    MaxBandwidth,
    #[qapi(name = "downtime-limit")]
    DowntimeLimit,
    #[qapi(name = "x-checkpoint-delay")]
    XCheckpointDelay,
    #[qapi(name = "block-incremental")]
    #[qapi(since = "2.10")]
    #[qapi(until = "9.1")]
    BlockIncremental,
    #[qapi(name = "x-multifd-channels")]
    #[qapi(since = "2.11")]
    #[qapi(until = "4.0")]
    XMultifdChannels,
    #[qapi(name = "x-multifd-page-count")]
    #[qapi(since = "2.11")]
    #[qapi(until = "4.0")]
    XMultifdPageCount,
    #[qapi(name = "xbzrle-cache-size")]
    #[qapi(since = "2.11")]
    XbzrleCacheSize,
    #[qapi(name = "max-postcopy-bandwidth")]
    #[qapi(since = "3.0")]
    MaxPostcopyBandwidth,
    #[qapi(name = "compress-wait-thread")]
    #[qapi(since = "3.1")]
    #[qapi(until = "9.1")]
    CompressWaitThread,
    #[qapi(name = "max-cpu-throttle")]
    #[qapi(since = "3.1")]
    MaxCpuThrottle,
    #[qapi(name = "announce-initial")]
    #[qapi(since = "4.0")]
    AnnounceInitial,
    #[qapi(name = "announce-max")]
    #[qapi(since = "4.0")]
    AnnounceMax,
    #[qapi(name = "announce-rounds")]
    #[qapi(since = "4.0")]
    AnnounceRounds,
    #[qapi(name = "announce-step")]
    #[qapi(since = "4.0")]
    AnnounceStep,
    #[qapi(name = "tls-authz")]
    #[qapi(since = "4.0")]
    TlsAuthz,
    #[qapi(name = "multifd-channels")]
    #[qapi(since = "4.0")]
    MultifdChannels,
    #[qapi(name = "throttle-trigger-threshold")]
    #[qapi(since = "5.0")]
    ThrottleTriggerThreshold,
    #[qapi(name = "multifd-compression")]
    #[qapi(since = "5.0")]
    MultifdCompression,
    #[qapi(name = "multifd-zlib-level")]
    #[qapi(since = "5.0")]
    MultifdZlibLevel,
    #[qapi(name = "multifd-zstd-level")]
    #[qapi(since = "5.0")]
    MultifdZstdLevel,
    #[qapi(name = "cpu-throttle-tailslow")]
    #[qapi(since = "5.1")]
    CpuThrottleTailslow,
    #[qapi(name = "block-bitmap-mapping")]
    #[qapi(since = "5.2")]
    BlockBitmapMapping,
    #[qapi(name = "x-vcpu-dirty-limit-period")]
    #[qapi(feature = "unstable")]
    #[qapi(since = "8.1")]
    XVcpuDirtyLimitPeriod,
    #[qapi(name = "vcpu-dirty-limit")]
    #[qapi(since = "8.1")]
    VcpuDirtyLimit,
    #[qapi(name = "avail-switchover-bandwidth")]
    #[qapi(since = "8.2")]
    AvailSwitchoverBandwidth,
    #[qapi(name = "mode")]
    #[qapi(since = "8.2")]
    Mode,
    #[qapi(name = "zero-page-detection")]
    #[qapi(since = "9.0")]
    ZeroPageDetection,
    #[qapi(name = "direct-io")]
    #[qapi(since = "9.1")]
    DirectIo,
    #[qapi(name = "multifd-qatzip-level")]
    #[qapi(since = "9.2")]
    MultifdQatzipLevel,
    #[qapi(name = "cpr-exec-command")]
    #[qapi(since = "10.2")]
    CprExecCommand,
}
#[qapi(since = "2.3")]
pub enum MigrationStatus {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "setup")]
    Setup,
    #[qapi(name = "cancelling")]
    Cancelling,
    #[qapi(name = "cancelled")]
    Cancelled,
    #[qapi(name = "active")]
    Active,
    #[qapi(name = "completed")]
    Completed,
    #[qapi(name = "failed")]
    Failed,
    #[qapi(name = "postcopy-active")]
    #[qapi(since = "2.5")]
    PostcopyActive,
    #[qapi(name = "colo")]
    #[qapi(since = "2.8")]
    Colo,
    #[qapi(name = "pre-switchover")]
    #[qapi(since = "2.11")]
    PreSwitchover,
    #[qapi(name = "device")]
    #[qapi(since = "2.11")]
    Device,
    #[qapi(name = "postcopy-paused")]
    #[qapi(since = "3.0")]
    PostcopyPaused,
    #[qapi(name = "postcopy-recover")]
    #[qapi(since = "3.0")]
    PostcopyRecover,
    #[qapi(name = "wait-unplug")]
    #[qapi(since = "4.2")]
    WaitUnplug,
    #[qapi(name = "postcopy-recover-setup")]
    #[qapi(since = "9.1")]
    PostcopyRecoverSetup,
    #[qapi(name = "postcopy-device")]
    #[qapi(since = "10.2")]
    PostcopyDevice,
}
#[qapi(since = "3.0")]
pub enum MirrorCopyMode {
    #[qapi(name = "background")]
    Background,
    #[qapi(name = "write-blocking")]
    WriteBlocking,
}
#[qapi(since = "1.3")]
pub enum MirrorSyncMode {
    #[qapi(name = "top")]
    Top,
    #[qapi(name = "full")]
    Full,
    #[qapi(name = "none")]
    None,
    #[qapi(name = "incremental")]
    #[qapi(since = "2.4")]
    Incremental,
    #[qapi(name = "bitmap")]
    #[qapi(since = "4.2")]
    Bitmap,
}
#[qapi(since = "5.0")]
pub enum MonitorMode {
    #[qapi(name = "readline")]
    Readline,
    #[qapi(name = "control")]
    Control,
}
#[qapi(name = "MultiFDCompression")]
#[qapi(since = "5.0")]
pub enum MultiFdCompression {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "zlib")]
    Zlib,
    #[qapi(name = "zstd")]
    #[qapi(condition = "defined(CONFIG_ZSTD)")]
    Zstd,
    #[qapi(name = "qpl")]
    #[qapi(condition = "CONFIG_QPL")]
    #[qapi(since = "9.1")]
    Qpl,
    #[qapi(name = "uadk")]
    #[qapi(condition = "CONFIG_UADK")]
    #[qapi(since = "9.1")]
    Uadk,
    #[qapi(name = "qatzip")]
    #[qapi(condition = "CONFIG_QATZIP")]
    #[qapi(since = "9.2")]
    Qatzip,
}
#[qapi(name = "NFSTransport")]
#[qapi(since = "2.8")]
pub enum NfsTransport {
    #[qapi(name = "inet")]
    Inet,
}
#[qapi(since = "2.12")]
#[qapi(until = "5.2")]
pub enum NbdServerRemoveMode {
    #[qapi(name = "safe")]
    Safe,
    #[qapi(name = "hard")]
    Hard,
}
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
    #[qapi(name = "l2tpv3")]
    L2tpv3,
    #[qapi(name = "socket")]
    Socket,
    #[qapi(name = "vde")]
    Vde,
    #[qapi(name = "dump")]
    #[qapi(until = "2.12")]
    Dump,
    #[qapi(name = "bridge")]
    Bridge,
    #[qapi(name = "hubport")]
    Hubport,
    #[qapi(name = "netmap")]
    Netmap,
    #[qapi(name = "vhost-user")]
    VhostUser,
    #[qapi(name = "vhost-vdpa")]
    #[qapi(since = "5.1")]
    VhostVdpa,
    #[qapi(name = "vmnet-host")]
    #[qapi(condition = "CONFIG_VMNET")]
    #[qapi(since = "7.1")]
    VmnetHost,
    #[qapi(name = "vmnet-shared")]
    #[qapi(condition = "CONFIG_VMNET")]
    #[qapi(since = "7.1")]
    VmnetShared,
    #[qapi(name = "vmnet-bridged")]
    #[qapi(condition = "CONFIG_VMNET")]
    #[qapi(since = "7.1")]
    VmnetBridged,
    #[qapi(name = "stream")]
    #[qapi(since = "7.2")]
    Stream,
    #[qapi(name = "dgram")]
    #[qapi(since = "7.2")]
    Dgram,
    #[qapi(name = "af-xdp")]
    #[qapi(condition = "CONFIG_AF_XDP")]
    #[qapi(since = "8.2")]
    AfXdp,
    #[qapi(name = "passt")]
    #[qapi(condition = "CONFIG_PASST")]
    #[qapi(since = "10.1")]
    Passt,
}
#[qapi(since = "1.6")]
#[qapi(until = "2.7")]
pub enum NetClientOptionsBranch {
    #[qapi(name = "none")]
    None(NetdevNoneOptions),
    #[qapi(name = "nic")]
    Nic(NetLegacyNicOptions),
    #[qapi(name = "user")]
    User(NetdevUserOptions),
    #[qapi(name = "tap")]
    Tap(NetdevTapOptions),
    #[qapi(name = "socket")]
    Socket(NetdevSocketOptions),
    #[qapi(name = "vde")]
    Vde(NetdevVdeOptions),
    #[qapi(name = "dump")]
    Dump(NetdevDumpOptions),
    #[qapi(name = "bridge")]
    Bridge(NetdevBridgeOptions),
    #[qapi(name = "hubport")]
    Hubport(NetdevHubPortOptions),
    #[qapi(name = "l2tpv3")]
    #[qapi(since = "2.3")]
    L2tpv3(NetdevL2tPv3Options),
    #[qapi(name = "netmap")]
    #[qapi(since = "2.3")]
    Netmap(NetdevNetmapOptions),
    #[qapi(name = "vhost-user")]
    #[qapi(since = "2.3")]
    VhostUser(NetdevVhostUserOptions),
}
#[qapi(since = "1.2")]
#[qapi(until = "2.7")]
pub enum NetClientOptionsKind {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "nic")]
    Nic,
    #[qapi(name = "user")]
    User,
    #[qapi(name = "tap")]
    Tap,
    #[qapi(name = "socket")]
    Socket,
    #[qapi(name = "vde")]
    Vde,
    #[qapi(name = "dump")]
    Dump,
    #[qapi(name = "bridge")]
    Bridge,
    #[qapi(name = "hubport")]
    Hubport,
    #[qapi(name = "l2tpv3")]
    #[qapi(since = "2.3")]
    L2tpv3,
    #[qapi(name = "netmap")]
    #[qapi(since = "2.3")]
    Netmap,
    #[qapi(name = "vhost-user")]
    #[qapi(since = "2.3")]
    VhostUser,
}
#[qapi(since = "2.5")]
pub enum NetFilterDirection {
    #[qapi(name = "all")]
    All,
    #[qapi(name = "rx")]
    Rx,
    #[qapi(name = "tx")]
    Tx,
}
#[qapi(since = "2.7")]
#[qapi(until = "5.1")]
pub enum NetLegacyOptionsBranch {
    #[qapi(name = "none")]
    #[qapi(until = "3.0")]
    None(NetdevNoneOptions),
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
    #[qapi(name = "vde")]
    Vde(NetdevVdeOptions),
    #[qapi(name = "dump")]
    #[qapi(until = "2.12")]
    Dump(NetdevDumpOptions),
    #[qapi(name = "bridge")]
    Bridge(NetdevBridgeOptions),
    #[qapi(name = "netmap")]
    Netmap(NetdevNetmapOptions),
    #[qapi(name = "vhost-user")]
    VhostUser(NetdevVhostUserOptions),
}
#[qapi(since = "1.2")]
#[qapi(until = "2.9")]
pub enum NetLegacyOptionsKind {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "nic")]
    Nic,
    #[qapi(name = "user")]
    User,
    #[qapi(name = "tap")]
    Tap,
    #[qapi(name = "l2tpv3")]
    L2tpv3,
    #[qapi(name = "socket")]
    Socket,
    #[qapi(name = "vde")]
    Vde,
    #[qapi(name = "dump")]
    Dump,
    #[qapi(name = "bridge")]
    Bridge,
    #[qapi(name = "netmap")]
    Netmap,
    #[qapi(name = "vhost-user")]
    VhostUser,
}
#[qapi(since = "1.2")]
#[qapi(until = "5.1")]
pub enum NetLegacyOptionsType {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "nic")]
    Nic,
    #[qapi(name = "user")]
    User,
    #[qapi(name = "tap")]
    Tap,
    #[qapi(name = "l2tpv3")]
    L2tpv3,
    #[qapi(name = "socket")]
    Socket,
    #[qapi(name = "vde")]
    Vde,
    #[qapi(name = "dump")]
    #[qapi(until = "2.12")]
    Dump,
    #[qapi(name = "bridge")]
    Bridge,
    #[qapi(name = "netmap")]
    Netmap,
    #[qapi(name = "vhost-user")]
    VhostUser,
}
#[qapi(since = "2.7")]
pub enum NetdevBranch {
    #[qapi(name = "none")]
    #[qapi(until = "3.0")]
    None(NetdevNoneOptions),
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
    #[qapi(name = "vde")]
    Vde(NetdevVdeOptions),
    #[qapi(name = "dump")]
    #[qapi(until = "2.12")]
    Dump(NetdevDumpOptions),
    #[qapi(name = "bridge")]
    Bridge(NetdevBridgeOptions),
    #[qapi(name = "hubport")]
    Hubport(NetdevHubPortOptions),
    #[qapi(name = "netmap")]
    Netmap(NetdevNetmapOptions),
    #[qapi(name = "vhost-user")]
    VhostUser(NetdevVhostUserOptions),
    #[qapi(name = "vhost-vdpa")]
    #[qapi(since = "5.1")]
    VhostVdpa(NetdevVhostVdpaOptions),
    #[qapi(name = "vmnet-host")]
    #[qapi(condition = "CONFIG_VMNET")]
    #[qapi(since = "7.1")]
    VmnetHost(NetdevVmnetHostOptions),
    #[qapi(name = "vmnet-shared")]
    #[qapi(condition = "CONFIG_VMNET")]
    #[qapi(since = "7.1")]
    VmnetShared(NetdevVmnetSharedOptions),
    #[qapi(name = "vmnet-bridged")]
    #[qapi(condition = "CONFIG_VMNET")]
    #[qapi(since = "7.1")]
    VmnetBridged(NetdevVmnetBridgedOptions),
    #[qapi(name = "stream")]
    #[qapi(since = "7.2")]
    Stream(NetdevStreamOptions),
    #[qapi(name = "dgram")]
    #[qapi(since = "7.2")]
    Dgram(NetdevDgramOptions),
    #[qapi(name = "af-xdp")]
    #[qapi(condition = "CONFIG_AF_XDP")]
    #[qapi(since = "8.2")]
    AfXdp(NetdevAfxdpOptions),
    #[qapi(name = "passt")]
    #[qapi(condition = "CONFIG_PASST")]
    #[qapi(since = "10.1")]
    Passt(NetDevPasstOptions),
}
#[qapi(since = "5.0")]
pub enum NetfilterInsert {
    #[qapi(name = "before")]
    Before,
    #[qapi(name = "behind")]
    Behind,
}
#[qapi(since = "2.1")]
pub enum NetworkAddressFamily {
    #[qapi(name = "ipv4")]
    Ipv4,
    #[qapi(name = "ipv6")]
    Ipv6,
    #[qapi(name = "unix")]
    Unix,
    #[qapi(name = "unknown")]
    Unknown,
    #[qapi(name = "vsock")]
    #[qapi(since = "2.8")]
    Vsock,
}
#[qapi(since = "1.1")]
pub enum NewImageMode {
    #[qapi(name = "existing")]
    Existing,
    #[qapi(name = "absolute-paths")]
    AbsolutePaths,
}
#[qapi(since = "7.2")]
pub enum NotifyVmexitOption {
    #[qapi(name = "run")]
    Run,
    #[qapi(name = "internal-error")]
    InternalError,
    #[qapi(name = "disable")]
    Disable,
}
#[qapi(since = "2.1")]
pub enum NumaOptionsBranch {
    #[qapi(name = "node")]
    Node(NumaNodeOptions),
    #[qapi(name = "dist")]
    #[qapi(since = "2.10")]
    Dist(NumaDistOptions),
    #[qapi(name = "cpu")]
    #[qapi(since = "2.10")]
    Cpu(NumaCpuOptions),
    #[qapi(name = "hmat-lb")]
    #[qapi(since = "5.0")]
    HmatLb(NumaHmatLbOptions),
    #[qapi(name = "hmat-cache")]
    #[qapi(since = "5.0")]
    HmatCache(NumaHmatCacheOptions),
}
#[qapi(since = "2.1")]
#[qapi(until = "2.9")]
pub enum NumaOptionsKind {
    #[qapi(name = "node")]
    Node,
}
#[qapi(since = "2.1")]
pub enum NumaOptionsType {
    #[qapi(name = "node")]
    Node,
    #[qapi(name = "dist")]
    #[qapi(since = "2.10")]
    Dist,
    #[qapi(name = "cpu")]
    #[qapi(since = "2.10")]
    Cpu,
    #[qapi(name = "hmat-lb")]
    #[qapi(since = "5.0")]
    HmatLb,
    #[qapi(name = "hmat-cache")]
    #[qapi(since = "5.0")]
    HmatCache,
}
#[qapi(since = "6.0")]
pub enum ObjectOptionsBranch {
    #[qapi(name = "authz-list")]
    AuthzList(AuthZListProperties),
    #[qapi(name = "authz-listfile")]
    AuthzListfile(AuthZListFileProperties),
    #[qapi(name = "authz-pam")]
    AuthzPam(AuthZpamProperties),
    #[qapi(name = "authz-simple")]
    AuthzSimple(AuthZSimpleProperties),
    #[qapi(name = "can-host-socketcan")]
    CanHostSocketcan(CanHostSocketcanProperties),
    #[qapi(name = "colo-compare")]
    ColoCompare(ColoCompareProperties),
    #[qapi(name = "cryptodev-backend")]
    CryptodevBackend(CryptodevBackendProperties),
    #[qapi(name = "cryptodev-backend-builtin")]
    CryptodevBackendBuiltin(CryptodevBackendProperties),
    #[qapi(name = "cryptodev-vhost-user")]
    #[qapi(condition = "defined(CONFIG_VHOST_CRYPTO)")]
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
    InputLinux(InputLinuxProperties),
    #[qapi(name = "iothread")]
    Iothread(IothreadProperties),
    #[qapi(name = "memory-backend-file")]
    MemoryBackendFile(MemoryBackendFileProperties),
    #[qapi(name = "memory-backend-memfd")]
    #[qapi(condition = "defined(CONFIG_LINUX)")]
    MemoryBackendMemfd(MemoryBackendMemfdProperties),
    #[qapi(name = "memory-backend-ram")]
    MemoryBackendRam(MemoryBackendProperties),
    #[qapi(name = "pr-manager-helper")]
    PrManagerHelper(PrManagerHelperProperties),
    #[qapi(name = "rng-builtin")]
    RngBuiltin(RngProperties),
    #[qapi(name = "rng-egd")]
    RngEgd(RngEgdProperties),
    #[qapi(name = "rng-random")]
    RngRandom(RngRandomProperties),
    #[qapi(name = "secret")]
    Secret(SecretProperties),
    #[qapi(name = "secret_keyring")]
    SecretKeyring(SecretKeyringProperties),
    #[qapi(name = "sev-guest")]
    SevGuest(SevGuestProperties),
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
    #[qapi(name = "qtest")]
    #[qapi(since = "6.1")]
    Qtest(QtestProperties),
    #[qapi(name = "memory-backend-epc")]
    #[qapi(condition = "CONFIG_LINUX")]
    #[qapi(since = "6.2")]
    MemoryBackendEpc(MemoryBackendEpcProperties),
    #[qapi(name = "main-loop")]
    #[qapi(since = "7.1")]
    MainLoop(MainLoopProperties),
    #[qapi(name = "x-vfio-user-server")]
    #[qapi(since = "7.1")]
    XVfioUserServer(VfioUserServerProperties),
    #[qapi(name = "cryptodev-backend-lkcf")]
    #[qapi(since = "7.2")]
    CryptodevBackendLkcf(CryptodevBackendProperties),
    #[qapi(name = "thread-context")]
    #[qapi(since = "7.2")]
    ThreadContext(ThreadContextProperties),
    #[qapi(name = "acpi-generic-initiator")]
    #[qapi(since = "9.0")]
    AcpiGenericInitiator(AcpiGenericInitiatorProperties),
    #[qapi(name = "iommufd")]
    #[qapi(since = "9.0")]
    Iommufd(IommufdProperties),
    #[qapi(name = "memory-backend-shm")]
    #[qapi(condition = "CONFIG_POSIX")]
    #[qapi(since = "9.1")]
    MemoryBackendShm(MemoryBackendShmProperties),
    #[qapi(name = "sev-snp-guest")]
    #[qapi(since = "9.1")]
    SevSnpGuest(SevSnpGuestProperties),
    #[qapi(name = "acpi-generic-port")]
    #[qapi(since = "9.2")]
    AcpiGenericPort(AcpiGenericPortProperties),
    #[qapi(name = "igvm-cfg")]
    #[qapi(condition = "CONFIG_IGVM")]
    #[qapi(since = "10.1")]
    IgvmCfg(IgvmCfgProperties),
    #[qapi(name = "tdx-guest")]
    #[qapi(since = "10.1")]
    TdxGuest(TdxGuestProperties),
}
#[qapi(since = "6.0")]
pub enum ObjectType {
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
    CanHostSocketcan,
    #[qapi(name = "colo-compare")]
    ColoCompare,
    #[qapi(name = "cryptodev-backend")]
    CryptodevBackend,
    #[qapi(name = "cryptodev-backend-builtin")]
    CryptodevBackendBuiltin,
    #[qapi(name = "cryptodev-vhost-user")]
    #[qapi(condition = "defined(CONFIG_VHOST_CRYPTO)")]
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
    InputLinux,
    #[qapi(name = "iothread")]
    Iothread,
    #[qapi(name = "memory-backend-file")]
    MemoryBackendFile,
    #[qapi(name = "memory-backend-memfd")]
    #[qapi(condition = "defined(CONFIG_LINUX)")]
    MemoryBackendMemfd,
    #[qapi(name = "memory-backend-ram")]
    MemoryBackendRam,
    #[qapi(name = "pef-guest")]
    PefGuest,
    #[qapi(name = "pr-manager-helper")]
    PrManagerHelper,
    #[qapi(name = "rng-builtin")]
    RngBuiltin,
    #[qapi(name = "rng-egd")]
    RngEgd,
    #[qapi(name = "rng-random")]
    RngRandom,
    #[qapi(name = "secret")]
    Secret,
    #[qapi(name = "secret_keyring")]
    SecretKeyring,
    #[qapi(name = "sev-guest")]
    SevGuest,
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
    XRemoteObject,
    #[qapi(name = "qtest")]
    #[qapi(since = "6.1")]
    Qtest,
    #[qapi(name = "memory-backend-epc")]
    #[qapi(condition = "CONFIG_LINUX")]
    #[qapi(since = "6.2")]
    MemoryBackendEpc,
    #[qapi(name = "main-loop")]
    #[qapi(since = "7.1")]
    MainLoop,
    #[qapi(name = "x-vfio-user-server")]
    #[qapi(feature = "unstable")]
    #[qapi(since = "7.1")]
    XVfioUserServer,
    #[qapi(name = "cryptodev-backend-lkcf")]
    #[qapi(since = "7.2")]
    CryptodevBackendLkcf,
    #[qapi(name = "thread-context")]
    #[qapi(since = "7.2")]
    ThreadContext,
    #[qapi(name = "acpi-generic-initiator")]
    #[qapi(since = "9.0")]
    AcpiGenericInitiator,
    #[qapi(name = "iommufd")]
    #[qapi(since = "9.0")]
    Iommufd,
    #[qapi(name = "memory-backend-shm")]
    #[qapi(condition = "CONFIG_POSIX")]
    #[qapi(since = "9.1")]
    MemoryBackendShm,
    #[qapi(name = "sev-snp-guest")]
    #[qapi(since = "9.1")]
    SevSnpGuest,
    #[qapi(name = "acpi-generic-port")]
    #[qapi(since = "9.2")]
    AcpiGenericPort,
    #[qapi(name = "igvm-cfg")]
    #[qapi(condition = "CONFIG_IGVM")]
    #[qapi(since = "10.1")]
    IgvmCfg,
    #[qapi(name = "tdx-guest")]
    #[qapi(since = "10.1")]
    TdxGuest,
}
#[qapi(name = "OffAutoPCIBAR")]
#[qapi(since = "2.12")]
pub enum OffAutoPcibar {
    #[qapi(name = "off")]
    Off,
    #[qapi(name = "auto")]
    Auto,
    #[qapi(name = "bar0")]
    Bar0,
    #[qapi(name = "bar1")]
    Bar1,
    #[qapi(name = "bar2")]
    Bar2,
    #[qapi(name = "bar3")]
    Bar3,
    #[qapi(name = "bar4")]
    Bar4,
    #[qapi(name = "bar5")]
    Bar5,
}
#[qapi(since = "7.1")]
pub enum OnCbwError {
    #[qapi(name = "break-guest-write")]
    BreakGuestWrite,
    #[qapi(name = "break-snapshot")]
    BreakSnapshot,
}
#[qapi(since = "2.2")]
pub enum OnOffAuto {
    #[qapi(name = "auto")]
    Auto,
    #[qapi(name = "on")]
    On,
    #[qapi(name = "off")]
    Off,
}
#[qapi(since = "2.6")]
pub enum OnOffSplit {
    #[qapi(name = "on")]
    On,
    #[qapi(name = "off")]
    Off,
    #[qapi(name = "split")]
    Split,
}
#[qapi(name = "PCIELinkSpeed")]
#[qapi(since = "4.0")]
pub enum PcieLinkSpeed {
    #[qapi(name = "2_5")]
    _25,
    #[qapi(name = "5")]
    _5,
    #[qapi(name = "8")]
    _8,
    #[qapi(name = "16")]
    _16,
    #[qapi(name = "32")]
    #[qapi(since = "9.0")]
    _32,
    #[qapi(name = "64")]
    #[qapi(since = "9.0")]
    _64,
}
#[qapi(name = "PCIELinkWidth")]
#[qapi(since = "4.0")]
pub enum PcieLinkWidth {
    #[qapi(name = "1")]
    _1,
    #[qapi(name = "2")]
    _2,
    #[qapi(name = "4")]
    _4,
    #[qapi(name = "8")]
    _8,
    #[qapi(name = "12")]
    _12,
    #[qapi(name = "16")]
    _16,
    #[qapi(name = "32")]
    _32,
}
#[qapi(since = "6.0")]
pub enum PanicAction {
    #[qapi(name = "pause")]
    Pause,
    #[qapi(name = "shutdown")]
    Shutdown,
    #[qapi(name = "none")]
    None,
    #[qapi(name = "exit-failure")]
    #[qapi(since = "7.1")]
    ExitFailure,
}
#[qapi(since = "2.2")]
pub enum PreallocMode {
    #[qapi(name = "off")]
    Off,
    #[qapi(name = "metadata")]
    Metadata,
    #[qapi(name = "falloc")]
    Falloc,
    #[qapi(name = "full")]
    Full,
}
#[qapi(since = "4.0")]
pub enum QAuthZListFormat {
    #[qapi(name = "exact")]
    Exact,
    #[qapi(name = "glob")]
    Glob,
}
#[qapi(since = "4.0")]
pub enum QAuthZListPolicy {
    #[qapi(name = "deny")]
    Deny,
    #[qapi(name = "allow")]
    Allow,
}
#[qapi(since = "7.1")]
pub enum QCryptoAkCipherAlgo {
    #[qapi(name = "rsa")]
    Rsa,
}
#[qapi(since = "7.1")]
#[qapi(until = "9.2")]
pub enum QCryptoAkCipherAlgorithm {
    #[qapi(name = "rsa")]
    Rsa,
}
#[qapi(since = "7.1")]
pub enum QCryptoAkCipherKeyType {
    #[qapi(name = "public")]
    Public,
    #[qapi(name = "private")]
    Private,
}
#[qapi(since = "7.1")]
pub enum QCryptoAkCipherOptionsBranch {
    #[qapi(name = "rsa")]
    Rsa(QCryptoAkCipherOptionsRsa),
}
#[qapi(since = "5.1")]
pub enum QCryptoBlockAmendOptionsBranch {
    #[qapi(name = "luks")]
    Luks(QCryptoBlockAmendOptionsLuks),
}
#[qapi(since = "2.6")]
pub enum QCryptoBlockCreateOptionsBranch {
    #[qapi(name = "qcow")]
    Qcow(QCryptoBlockOptionsQCow),
    #[qapi(name = "luks")]
    Luks(QCryptoBlockCreateOptionsLuks),
}
#[qapi(since = "2.6")]
pub enum QCryptoBlockFormat {
    #[qapi(name = "qcow")]
    Qcow,
    #[qapi(name = "luks")]
    Luks,
}
#[qapi(since = "2.7")]
pub enum QCryptoBlockInfoBranch {
    #[qapi(name = "qcow")]
    #[qapi(until = "3.0")]
    Qcow(QCryptoBlockInfoQCow),
    #[qapi(name = "luks")]
    Luks(QCryptoBlockInfoLuks),
}
#[qapi(name = "QCryptoBlockLUKSKeyslotState")]
#[qapi(since = "5.1")]
pub enum QCryptoBlockLuksKeyslotState {
    #[qapi(name = "active")]
    Active,
    #[qapi(name = "inactive")]
    Inactive,
}
#[qapi(since = "2.6")]
pub enum QCryptoBlockOpenOptionsBranch {
    #[qapi(name = "qcow")]
    Qcow(QCryptoBlockOptionsQCow),
    #[qapi(name = "luks")]
    Luks(QCryptoBlockOptionsLuks),
}
#[qapi(since = "2.6")]
pub enum QCryptoCipherAlgo {
    #[qapi(name = "aes-128")]
    Aes128,
    #[qapi(name = "aes-192")]
    Aes192,
    #[qapi(name = "aes-256")]
    Aes256,
    #[qapi(name = "des")]
    Des,
    #[qapi(name = "3des")]
    _3des,
    #[qapi(name = "cast5-128")]
    Cast5128,
    #[qapi(name = "serpent-128")]
    Serpent128,
    #[qapi(name = "serpent-192")]
    Serpent192,
    #[qapi(name = "serpent-256")]
    Serpent256,
    #[qapi(name = "twofish-128")]
    Twofish128,
    #[qapi(name = "twofish-192")]
    Twofish192,
    #[qapi(name = "twofish-256")]
    Twofish256,
    #[qapi(name = "sm4")]
    Sm4,
}
#[qapi(since = "2.6")]
#[qapi(until = "9.2")]
pub enum QCryptoCipherAlgorithm {
    #[qapi(name = "aes-128")]
    Aes128,
    #[qapi(name = "aes-192")]
    Aes192,
    #[qapi(name = "aes-256")]
    Aes256,
    #[qapi(name = "des-rfb")]
    #[qapi(until = "6.1")]
    DesRfb,
    #[qapi(name = "cast5-128")]
    Cast5128,
    #[qapi(name = "serpent-128")]
    Serpent128,
    #[qapi(name = "serpent-192")]
    Serpent192,
    #[qapi(name = "serpent-256")]
    Serpent256,
    #[qapi(name = "twofish-128")]
    Twofish128,
    #[qapi(name = "twofish-192")]
    Twofish192,
    #[qapi(name = "twofish-256")]
    Twofish256,
    #[qapi(name = "3des")]
    #[qapi(since = "2.9")]
    _3des,
    #[qapi(name = "des")]
    #[qapi(since = "6.1")]
    Des,
    #[qapi(name = "sm4")]
    #[qapi(since = "9.0")]
    Sm4,
}
#[qapi(since = "2.6")]
pub enum QCryptoCipherMode {
    #[qapi(name = "ecb")]
    Ecb,
    #[qapi(name = "cbc")]
    Cbc,
    #[qapi(name = "xts")]
    Xts,
    #[qapi(name = "ctr")]
    #[qapi(since = "2.8")]
    Ctr,
}
#[qapi(since = "2.6")]
pub enum QCryptoHashAlgo {
    #[qapi(name = "md5")]
    Md5,
    #[qapi(name = "sha1")]
    Sha1,
    #[qapi(name = "sha224")]
    Sha224,
    #[qapi(name = "sha256")]
    Sha256,
    #[qapi(name = "sha384")]
    Sha384,
    #[qapi(name = "sha512")]
    Sha512,
    #[qapi(name = "ripemd160")]
    Ripemd160,
    #[qapi(name = "sm3")]
    Sm3,
}
#[qapi(since = "2.6")]
#[qapi(until = "9.2")]
pub enum QCryptoHashAlgorithm {
    #[qapi(name = "md5")]
    Md5,
    #[qapi(name = "sha1")]
    Sha1,
    #[qapi(name = "sha256")]
    Sha256,
    #[qapi(name = "sha224")]
    #[qapi(since = "2.7")]
    Sha224,
    #[qapi(name = "sha384")]
    #[qapi(since = "2.7")]
    Sha384,
    #[qapi(name = "sha512")]
    #[qapi(since = "2.7")]
    Sha512,
    #[qapi(name = "ripemd160")]
    #[qapi(since = "2.7")]
    Ripemd160,
}
#[qapi(name = "QCryptoIVGenAlgo")]
#[qapi(since = "2.6")]
pub enum QCryptoIvGenAlgo {
    #[qapi(name = "plain")]
    Plain,
    #[qapi(name = "plain64")]
    Plain64,
    #[qapi(name = "essiv")]
    Essiv,
}
#[qapi(name = "QCryptoIVGenAlgorithm")]
#[qapi(since = "2.6")]
#[qapi(until = "9.2")]
pub enum QCryptoIvGenAlgorithm {
    #[qapi(name = "plain")]
    Plain,
    #[qapi(name = "plain64")]
    Plain64,
    #[qapi(name = "essiv")]
    Essiv,
}
#[qapi(name = "QCryptoRSAPaddingAlgo")]
#[qapi(since = "7.1")]
pub enum QCryptoRsaPaddingAlgo {
    #[qapi(name = "raw")]
    Raw,
    #[qapi(name = "pkcs1")]
    Pkcs1,
}
#[qapi(name = "QCryptoRSAPaddingAlgorithm")]
#[qapi(since = "7.1")]
#[qapi(until = "9.2")]
pub enum QCryptoRsaPaddingAlgorithm {
    #[qapi(name = "raw")]
    Raw,
    #[qapi(name = "pkcs1")]
    Pkcs1,
}
#[qapi(since = "2.6")]
pub enum QCryptoSecretFormat {
    #[qapi(name = "raw")]
    Raw,
    #[qapi(name = "base64")]
    Base64,
}
#[qapi(name = "QCryptoTLSCredsEndpoint")]
#[qapi(since = "2.5")]
pub enum QCryptoTlsCredsEndpoint {
    #[qapi(name = "client")]
    Client,
    #[qapi(name = "server")]
    Server,
}
#[qapi(since = "8.0")]
#[qapi(until = "9.2")]
pub enum QCryptodevBackendAlgType {
    #[qapi(name = "sym")]
    Sym,
    #[qapi(name = "asym")]
    Asym,
}
#[qapi(since = "8.0")]
pub enum QCryptodevBackendAlgoType {
    #[qapi(name = "sym")]
    Sym,
    #[qapi(name = "asym")]
    Asym,
}
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
#[qapi(since = "8.0")]
pub enum QCryptodevBackendType {
    #[qapi(name = "builtin")]
    Builtin,
    #[qapi(name = "vhost-user")]
    VhostUser,
    #[qapi(name = "lkcf")]
    Lkcf,
}
#[qapi(since = "1.3.0")]
pub enum QKeyCode {
    #[qapi(name = "shift")]
    Shift,
    #[qapi(name = "shift_r")]
    ShiftR,
    #[qapi(name = "alt")]
    Alt,
    #[qapi(name = "alt_r")]
    AltR,
    #[qapi(name = "altgr")]
    #[qapi(until = "2.10")]
    Altgr,
    #[qapi(name = "altgr_r")]
    #[qapi(until = "2.10")]
    AltgrR,
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
    #[qapi(name = "unmapped")]
    #[qapi(since = "2.0")]
    Unmapped,
    #[qapi(name = "pause")]
    #[qapi(since = "2.0")]
    Pause,
    #[qapi(name = "ro")]
    #[qapi(since = "2.4")]
    Ro,
    #[qapi(name = "kp_comma")]
    #[qapi(since = "2.4")]
    KpComma,
    #[qapi(name = "kp_equals")]
    #[qapi(since = "2.8")]
    KpEquals,
    #[qapi(name = "power")]
    #[qapi(since = "2.8")]
    Power,
    #[qapi(name = "hiragana")]
    #[qapi(since = "2.9")]
    Hiragana,
    #[qapi(name = "henkan")]
    #[qapi(since = "2.9")]
    Henkan,
    #[qapi(name = "yen")]
    #[qapi(since = "2.9")]
    Yen,
    #[qapi(name = "sleep")]
    #[qapi(since = "2.10")]
    Sleep,
    #[qapi(name = "wake")]
    #[qapi(since = "2.10")]
    Wake,
    #[qapi(name = "audionext")]
    #[qapi(since = "2.10")]
    Audionext,
    #[qapi(name = "audioprev")]
    #[qapi(since = "2.10")]
    Audioprev,
    #[qapi(name = "audiostop")]
    #[qapi(since = "2.10")]
    Audiostop,
    #[qapi(name = "audioplay")]
    #[qapi(since = "2.10")]
    Audioplay,
    #[qapi(name = "audiomute")]
    #[qapi(since = "2.10")]
    Audiomute,
    #[qapi(name = "volumeup")]
    #[qapi(since = "2.10")]
    Volumeup,
    #[qapi(name = "volumedown")]
    #[qapi(since = "2.10")]
    Volumedown,
    #[qapi(name = "mediaselect")]
    #[qapi(since = "2.10")]
    Mediaselect,
    #[qapi(name = "mail")]
    #[qapi(since = "2.10")]
    Mail,
    #[qapi(name = "calculator")]
    #[qapi(since = "2.10")]
    Calculator,
    #[qapi(name = "computer")]
    #[qapi(since = "2.10")]
    Computer,
    #[qapi(name = "ac_home")]
    #[qapi(since = "2.10")]
    AcHome,
    #[qapi(name = "ac_back")]
    #[qapi(since = "2.10")]
    AcBack,
    #[qapi(name = "ac_forward")]
    #[qapi(since = "2.10")]
    AcForward,
    #[qapi(name = "ac_refresh")]
    #[qapi(since = "2.10")]
    AcRefresh,
    #[qapi(name = "ac_bookmarks")]
    #[qapi(since = "2.10")]
    AcBookmarks,
    #[qapi(name = "muhenkan")]
    #[qapi(since = "2.12")]
    Muhenkan,
    #[qapi(name = "katakanahiragana")]
    #[qapi(since = "2.12")]
    Katakanahiragana,
    #[qapi(name = "lang1")]
    #[qapi(since = "6.1")]
    Lang1,
    #[qapi(name = "lang2")]
    #[qapi(since = "6.1")]
    Lang2,
    #[qapi(name = "f13")]
    #[qapi(since = "8.0")]
    F13,
    #[qapi(name = "f14")]
    #[qapi(since = "8.0")]
    F14,
    #[qapi(name = "f15")]
    #[qapi(since = "8.0")]
    F15,
    #[qapi(name = "f16")]
    #[qapi(since = "8.0")]
    F16,
    #[qapi(name = "f17")]
    #[qapi(since = "8.0")]
    F17,
    #[qapi(name = "f18")]
    #[qapi(since = "8.0")]
    F18,
    #[qapi(name = "f19")]
    #[qapi(since = "8.0")]
    F19,
    #[qapi(name = "f20")]
    #[qapi(since = "8.0")]
    F20,
    #[qapi(name = "f21")]
    #[qapi(since = "8.0")]
    F21,
    #[qapi(name = "f22")]
    #[qapi(since = "8.0")]
    F22,
    #[qapi(name = "f23")]
    #[qapi(since = "8.0")]
    F23,
    #[qapi(name = "f24")]
    #[qapi(since = "8.0")]
    F24,
}
#[qapi(name = "QMPCapability")]
#[qapi(since = "2.12")]
pub enum QmpCapability {
    #[qapi(name = "oob")]
    Oob,
}
#[qapi(since = "1.2")]
pub enum QapiErrorClass {
    GenericError,
    CommandNotFound,
    #[qapi(until = "2.10")]
    DeviceEncrypted,
    DeviceNotActive,
    DeviceNotFound,
    #[qapi(name = "KVMMissingCap")]
    KvmMissingCap,
}
#[qapi(since = "9.1")]
pub enum QapiVfioMigrationState {
    #[qapi(name = "stop")]
    Stop,
    #[qapi(name = "running")]
    Running,
    #[qapi(name = "stop-copy")]
    StopCopy,
    #[qapi(name = "resuming")]
    Resuming,
    #[qapi(name = "running-p2p")]
    RunningP2p,
    #[qapi(name = "pre-copy")]
    PreCopy,
    #[qapi(name = "pre-copy-p2p")]
    PreCopyP2p,
}
#[qapi(since = "4.0")]
pub enum Qcow2BitmapInfoFlags {
    #[qapi(name = "in-use")]
    InUse,
    #[qapi(name = "auto")]
    Auto,
}
#[qapi(since = "5.1")]
pub enum Qcow2CompressionType {
    #[qapi(name = "zlib")]
    Zlib,
    #[qapi(name = "zstd")]
    #[qapi(condition = "defined(CONFIG_ZSTD)")]
    Zstd,
}
#[qapi(since = "2.2")]
pub enum Qcow2OverlapCheckMode {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "constant")]
    Constant,
    #[qapi(name = "cached")]
    Cached,
    #[qapi(name = "all")]
    All,
}
#[qapi(since = "2.2")]
pub enum Qcow2OverlapChecks {
    #[qapi(name = "flags")]
    Flags(Qcow2OverlapCheckFlags),
    #[qapi(name = "mode")]
    Mode(Qcow2OverlapCheckMode),
}
#[qapi(since = "2.6")]
pub enum QuorumOpType {
    #[qapi(name = "read")]
    Read,
    #[qapi(name = "write")]
    Write,
    #[qapi(name = "flush")]
    Flush,
}
#[qapi(since = "2.2")]
pub enum QuorumReadPattern {
    #[qapi(name = "quorum")]
    Quorum,
    #[qapi(name = "fifo")]
    Fifo,
}
#[qapi(since = "3.0")]
pub enum RbdAuthMode {
    #[qapi(name = "cephx")]
    Cephx,
    #[qapi(name = "none")]
    None,
}
#[qapi(since = "6.1")]
pub enum RbdEncryptionCreateOptionsBranch {
    #[qapi(name = "luks")]
    Luks(RbdEncryptionCreateOptionsLuks),
    #[qapi(name = "luks2")]
    Luks2(RbdEncryptionCreateOptionsLuks2),
}
#[qapi(since = "6.1")]
pub enum RbdEncryptionOptionsBranch {
    #[qapi(name = "luks")]
    Luks(RbdEncryptionOptionsLuks),
    #[qapi(name = "luks2")]
    Luks2(RbdEncryptionOptionsLuks2),
    #[qapi(name = "luks-any")]
    #[qapi(since = "8.0")]
    LuksAny(RbdEncryptionOptionsLuksAny),
}
#[qapi(since = "6.1")]
pub enum RbdImageEncryptionFormat {
    #[qapi(name = "luks")]
    Luks,
    #[qapi(name = "luks2")]
    Luks2,
    #[qapi(name = "luks-any")]
    #[qapi(since = "8.0")]
    LuksAny,
}
#[qapi(since = "6.0")]
pub enum RebootAction {
    #[qapi(name = "reset")]
    Reset,
    #[qapi(name = "shutdown")]
    Shutdown,
}
#[qapi(since = "2.5")]
pub enum ReplayMode {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "record")]
    Record,
    #[qapi(name = "play")]
    Play,
}
#[qapi(since = "2.8")]
pub enum ReplicationMode {
    #[qapi(name = "primary")]
    Primary,
    #[qapi(name = "secondary")]
    Secondary,
}
#[qapi(since = "2.4")]
pub enum RockerPortAutoneg {
    #[qapi(name = "off")]
    Off,
    #[qapi(name = "on")]
    On,
}
#[qapi(since = "2.4")]
pub enum RockerPortDuplex {
    #[qapi(name = "half")]
    Half,
    #[qapi(name = "full")]
    Full,
}
#[qapi(since = "1.1")]
pub enum RunState {
    #[qapi(name = "debug")]
    Debug,
    #[qapi(name = "inmigrate")]
    Inmigrate,
    #[qapi(name = "internal-error")]
    InternalError,
    #[qapi(name = "io-error")]
    IoError,
    #[qapi(name = "paused")]
    Paused,
    #[qapi(name = "postmigrate")]
    Postmigrate,
    #[qapi(name = "prelaunch")]
    Prelaunch,
    #[qapi(name = "finish-migrate")]
    FinishMigrate,
    #[qapi(name = "restore-vm")]
    RestoreVm,
    #[qapi(name = "running")]
    Running,
    #[qapi(name = "save-vm")]
    SaveVm,
    #[qapi(name = "shutdown")]
    Shutdown,
    #[qapi(name = "suspended")]
    Suspended,
    #[qapi(name = "watchdog")]
    Watchdog,
    #[qapi(name = "guest-panicked")]
    #[qapi(since = "1.5")]
    GuestPanicked,
    #[qapi(name = "colo")]
    #[qapi(since = "2.8")]
    Colo,
    #[qapi(name = "preconfig")]
    #[qapi(since = "3.0")]
    #[qapi(until = "6.0")]
    Preconfig,
}
#[qapi(since = "1.6")]
pub enum RxState {
    #[qapi(name = "normal")]
    Normal,
    #[qapi(name = "none")]
    None,
    #[qapi(name = "all")]
    All,
}
#[qapi(since = "8.2")]
pub enum S390CpuEntitlement {
    #[qapi(name = "auto")]
    Auto,
    #[qapi(name = "low")]
    Low,
    #[qapi(name = "medium")]
    Medium,
    #[qapi(name = "high")]
    High,
}
#[qapi(condition = "TARGET_S390X")]
#[qapi(since = "8.2")]
pub enum S390CpuPolarization {
    #[qapi(name = "horizontal")]
    Horizontal,
    #[qapi(name = "vertical")]
    Vertical,
}
#[qapi(since = "2.12")]
pub enum S390CpuState {
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
#[qapi(since = "2.12")]
pub enum S390CrashReason {
    #[qapi(name = "unknown")]
    Unknown,
    #[qapi(name = "disabled-wait")]
    DisabledWait,
    #[qapi(name = "extint-loop")]
    ExtintLoop,
    #[qapi(name = "pgmint-loop")]
    PgmintLoop,
    #[qapi(name = "opint-loop")]
    OpintLoop,
}
#[qapi(since = "2.5")]
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
#[qapi(since = "2.5")]
pub enum SchemaMetaType {
    #[qapi(name = "builtin")]
    Builtin,
    #[qapi(name = "enum")]
    Enum,
    #[qapi(name = "array")]
    Array,
    #[qapi(name = "object")]
    Object,
    #[qapi(name = "alternate")]
    Alternate,
    #[qapi(name = "command")]
    Command,
    #[qapi(name = "event")]
    Event,
}
#[qapi(since = "7.0")]
pub enum SetPasswordAction {
    #[qapi(name = "keep")]
    Keep,
    #[qapi(name = "fail")]
    Fail,
    #[qapi(name = "disconnect")]
    Disconnect,
}
#[qapi(since = "7.0")]
pub enum SetPasswordOptionsBranch {
    #[qapi(name = "vnc")]
    Vnc(SetPasswordOptionsVnc),
}
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "6.2")]
pub enum SevGuestType {
    #[qapi(name = "sev")]
    Sev,
    #[qapi(name = "sev-snp")]
    SevSnp,
}
#[qapi(since = "9.1")]
pub enum SevInfoBranch {
    #[qapi(name = "sev")]
    Sev(SevGuestInfo),
    #[qapi(name = "sev-snp")]
    SevSnp(SevSnpGuestInfo),
}
#[qapi(since = "2.12")]
pub enum SevState {
    #[qapi(name = "uninit")]
    Uninit,
    #[qapi(name = "launch-update")]
    LaunchUpdate,
    #[qapi(name = "launch-secret")]
    LaunchSecret,
    #[qapi(name = "running")]
    Running,
    #[qapi(name = "send-update")]
    SendUpdate,
    #[qapi(name = "receive-update")]
    ReceiveUpdate,
}
#[qapi(since = "2.12")]
#[qapi(until = "6.1")]
pub enum SheepdogRedundancyBranch {
    #[qapi(name = "full")]
    Full(SheepdogRedundancyFull),
    #[qapi(name = "erasure-coded")]
    ErasureCoded(SheepdogRedundancyErasureCoded),
}
#[qapi(since = "2.12")]
#[qapi(until = "6.1")]
pub enum SheepdogRedundancyType {
    #[qapi(name = "full")]
    Full,
    #[qapi(name = "erasure-coded")]
    ErasureCoded,
}
#[qapi(since = "6.0")]
pub enum ShutdownAction {
    #[qapi(name = "poweroff")]
    Poweroff,
    #[qapi(name = "pause")]
    Pause,
}
#[qapi(since = "4.0")]
pub enum ShutdownCause {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "host-error")]
    HostError,
    #[qapi(name = "host-qmp-quit")]
    HostQmpQuit,
    #[qapi(name = "host-qmp-system-reset")]
    HostQmpSystemReset,
    #[qapi(name = "host-signal")]
    HostSignal,
    #[qapi(name = "host-ui")]
    HostUi,
    #[qapi(name = "guest-shutdown")]
    GuestShutdown,
    #[qapi(name = "guest-reset")]
    GuestReset,
    #[qapi(name = "guest-panic")]
    GuestPanic,
    #[qapi(name = "subsystem-reset")]
    SubsystemReset,
    #[qapi(name = "snapshot-load")]
    #[qapi(since = "7.2")]
    SnapshotLoad,
}
#[qapi(since = "7.0")]
pub enum SmbiosEntryPointType {
    #[qapi(name = "32")]
    _32,
    #[qapi(name = "64")]
    _64,
    #[qapi(name = "auto")]
    #[qapi(since = "9.0")]
    Auto,
}
#[qapi(since = "1.6")]
pub enum SocketAddressBranch {
    #[qapi(name = "inet")]
    Inet(InetSocketAddress),
    #[qapi(name = "unix")]
    Unix(UnixSocketAddress),
    #[qapi(name = "fd")]
    Fd(String),
    #[qapi(name = "vsock")]
    #[qapi(since = "2.8")]
    Vsock(VsockSocketAddress),
}
#[qapi(since = "2.9")]
#[qapi(until = "2.10")]
pub enum SocketAddressFlatBranch {
    #[qapi(name = "inet")]
    Inet(InetSocketAddress),
    #[qapi(name = "unix")]
    Unix(UnixSocketAddress),
    #[qapi(name = "vsock")]
    Vsock(VsockSocketAddress),
    #[qapi(name = "fd")]
    Fd(String),
}
#[qapi(since = "2.9")]
#[qapi(until = "2.10")]
pub enum SocketAddressFlatType {
    #[qapi(name = "inet")]
    Inet,
    #[qapi(name = "unix")]
    Unix,
    #[qapi(name = "vsock")]
    Vsock,
    #[qapi(name = "fd")]
    Fd,
}
#[qapi(since = "1.3")]
#[qapi(until = "2.10")]
pub enum SocketAddressKind {
    #[qapi(name = "inet")]
    Inet,
    #[qapi(name = "unix")]
    Unix,
    #[qapi(name = "fd")]
    Fd,
    #[qapi(name = "vsock")]
    #[qapi(since = "2.8")]
    Vsock,
}
#[qapi(since = "2.10")]
pub enum SocketAddressLegacyBranch {
    #[qapi(name = "inet")]
    Inet(InetSocketAddress),
    #[qapi(name = "unix")]
    Unix(UnixSocketAddress),
    #[qapi(name = "vsock")]
    Vsock(VsockSocketAddress),
    #[qapi(name = "fd")]
    Fd(String),
}
#[qapi(since = "1.3")]
#[qapi(until = "6.2")]
pub enum SocketAddressLegacyKind {
    #[qapi(name = "inet")]
    Inet,
    #[qapi(name = "unix")]
    Unix,
    #[qapi(name = "vsock")]
    Vsock,
    #[qapi(name = "fd")]
    Fd,
}
#[qapi(since = "2.9")]
pub enum SocketAddressType {
    #[qapi(name = "inet")]
    Inet,
    #[qapi(name = "unix")]
    Unix,
    #[qapi(name = "vsock")]
    Vsock,
    #[qapi(name = "fd")]
    Fd,
}
#[qapi(since = "1.1")]
pub enum SpiceQueryMouseMode {
    #[qapi(name = "client")]
    Client,
    #[qapi(name = "server")]
    Server,
    #[qapi(name = "unknown")]
    Unknown,
}
#[qapi(since = "2.12")]
pub enum SshHostKeyCheckBranch {
    #[qapi(name = "none")]
    #[qapi(until = "3.0")]
    None(SshHostKeyDummy),
    #[qapi(name = "hash")]
    Hash(SshHostKeyHash),
    #[qapi(name = "known_hosts")]
    #[qapi(until = "3.0")]
    KnownHosts(SshHostKeyDummy),
}
#[qapi(since = "2.12")]
pub enum SshHostKeyCheckHashType {
    #[qapi(name = "md5")]
    Md5,
    #[qapi(name = "sha1")]
    Sha1,
    #[qapi(name = "sha256")]
    #[qapi(since = "6.1")]
    Sha256,
}
#[qapi(since = "2.12")]
pub enum SshHostKeyCheckMode {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "hash")]
    Hash,
    #[qapi(name = "known_hosts")]
    KnownHosts,
}
#[qapi(since = "7.1")]
pub enum StatsFilterBranch {
    #[qapi(name = "vcpu")]
    Vcpu(StatsVcpuFilter),
}
#[qapi(since = "7.1")]
pub enum StatsProvider {
    #[qapi(name = "kvm")]
    Kvm,
    #[qapi(name = "cryptodev")]
    #[qapi(since = "8.0")]
    Cryptodev,
}
#[qapi(since = "7.1")]
pub enum StatsTarget {
    #[qapi(name = "vm")]
    Vm,
    #[qapi(name = "vcpu")]
    Vcpu,
    #[qapi(name = "cryptodev")]
    #[qapi(since = "8.0")]
    Cryptodev,
}
#[qapi(since = "7.1")]
pub enum StatsType {
    #[qapi(name = "cumulative")]
    Cumulative,
    #[qapi(name = "instant")]
    Instant,
    #[qapi(name = "peak")]
    Peak,
    #[qapi(name = "linear-histogram")]
    LinearHistogram,
    #[qapi(name = "log2-histogram")]
    Log2Histogram,
}
#[qapi(since = "7.1")]
pub enum StatsUnit {
    #[qapi(name = "bytes")]
    Bytes,
    #[qapi(name = "seconds")]
    Seconds,
    #[qapi(name = "cycles")]
    Cycles,
    #[qapi(name = "boolean")]
    Boolean,
}
#[qapi(since = "7.1")]
pub enum StatsValue {
    #[qapi(name = "scalar")]
    Scalar(u64),
    #[qapi(name = "boolean")]
    Boolean(bool),
    #[qapi(name = "list")]
    List(u64),
}
#[qapi(since = "2.10")]
pub enum StrOrNull {
    #[qapi(name = "s")]
    S(String),
    #[qapi(name = "n")]
    N(()),
}
#[qapi(since = "3.0")]
pub enum SysEmuTarget {
    #[qapi(name = "aarch64")]
    Aarch64,
    #[qapi(name = "alpha")]
    Alpha,
    #[qapi(name = "arm")]
    Arm,
    #[qapi(name = "cris")]
    #[qapi(until = "9.2")]
    Cris,
    #[qapi(name = "hppa")]
    Hppa,
    #[qapi(name = "i386")]
    I386,
    #[qapi(name = "lm32")]
    #[qapi(until = "6.1")]
    Lm32,
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
    #[qapi(name = "moxie")]
    #[qapi(until = "6.1")]
    Moxie,
    #[qapi(name = "nios2")]
    #[qapi(until = "9.1")]
    Nios2,
    #[qapi(name = "or1k")]
    Or1k,
    #[qapi(name = "ppc")]
    Ppc,
    #[qapi(name = "ppc64")]
    Ppc64,
    #[qapi(name = "ppcemb")]
    #[qapi(until = "3.1")]
    Ppcemb,
    #[qapi(name = "riscv32")]
    Riscv32,
    #[qapi(name = "riscv64")]
    Riscv64,
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
    #[qapi(name = "unicore32")]
    #[qapi(until = "6.1")]
    Unicore32,
    #[qapi(name = "x86_64")]
    X8664,
    #[qapi(name = "xtensa")]
    Xtensa,
    #[qapi(name = "xtensaeb")]
    Xtensaeb,
    #[qapi(name = "rx")]
    #[qapi(since = "5.0")]
    Rx,
    #[qapi(name = "avr")]
    #[qapi(since = "5.1")]
    Avr,
    #[qapi(name = "loongarch64")]
    #[qapi(since = "7.1")]
    Loongarch64,
}
#[qapi(since = "1.2.0")]
#[qapi(until = "1.6")]
pub enum TargetType {
    #[qapi(name = "alpha")]
    Alpha,
    #[qapi(name = "arm")]
    Arm,
    #[qapi(name = "cris")]
    Cris,
    #[qapi(name = "i386")]
    I386,
    #[qapi(name = "lm32")]
    Lm32,
    #[qapi(name = "m68k")]
    M68k,
    #[qapi(name = "microblazeel")]
    Microblazeel,
    #[qapi(name = "microblaze")]
    Microblaze,
    #[qapi(name = "mips64el")]
    Mips64el,
    #[qapi(name = "mips64")]
    Mips64,
    #[qapi(name = "mipsel")]
    Mipsel,
    #[qapi(name = "mips")]
    Mips,
    #[qapi(name = "or32")]
    Or32,
    #[qapi(name = "ppc64")]
    Ppc64,
    #[qapi(name = "ppcemb")]
    Ppcemb,
    #[qapi(name = "ppc")]
    Ppc,
    #[qapi(name = "s390x")]
    S390x,
    #[qapi(name = "sh4eb")]
    Sh4eb,
    #[qapi(name = "sh4")]
    Sh4,
    #[qapi(name = "sparc64")]
    Sparc64,
    #[qapi(name = "sparc")]
    Sparc,
    #[qapi(name = "unicore32")]
    Unicore32,
    #[qapi(name = "x86_64")]
    X8664,
    #[qapi(name = "xtensaeb")]
    Xtensaeb,
    #[qapi(name = "xtensa")]
    Xtensa,
    #[qapi(name = "moxie")]
    #[qapi(since = "1.5")]
    Moxie,
}
#[qapi(since = "8.2")]
pub enum TimeUnit {
    #[qapi(name = "second")]
    Second,
    #[qapi(name = "millisecond")]
    Millisecond,
}
#[qapi(since = "1.5")]
pub enum TpmModel {
    #[qapi(name = "tpm-tis")]
    TpmTis,
    #[qapi(name = "tpm-crb")]
    #[qapi(since = "2.12")]
    TpmCrb,
    #[qapi(name = "tpm-spapr")]
    #[qapi(since = "5.0")]
    TpmSpapr,
}
#[qapi(since = "1.5")]
pub enum TpmType {
    #[qapi(name = "passthrough")]
    Passthrough,
    #[qapi(name = "emulator")]
    #[qapi(since = "2.11")]
    Emulator,
}
#[qapi(since = "1.5")]
pub enum TpmTypeOptionsBranch {
    #[qapi(name = "passthrough")]
    Passthrough(TpmPassthroughOptions),
    #[qapi(name = "emulator")]
    #[qapi(since = "2.11")]
    Emulator(TpmEmulatorOptions),
}
#[qapi(since = "1.5")]
#[qapi(until = "6.2")]
pub enum TpmTypeOptionsKind {
    #[qapi(name = "passthrough")]
    Passthrough,
    #[qapi(name = "emulator")]
    #[qapi(since = "2.11")]
    Emulator,
}
#[qapi(since = "2.2")]
pub enum TraceEventState {
    #[qapi(name = "unavailable")]
    Unavailable,
    #[qapi(name = "disabled")]
    Disabled,
    #[qapi(name = "enabled")]
    Enabled,
}
#[qapi(since = "2.0")]
pub enum TransactionActionBranch {
    #[qapi(name = "blockdev-snapshot-sync")]
    BlockdevSnapshotSync(BlockdevSnapshot),
    #[qapi(name = "drive-backup")]
    DriveBackup(DriveBackup),
    #[qapi(name = "abort")]
    Abort(Abort),
    #[qapi(name = "blockdev-snapshot-internal-sync")]
    BlockdevSnapshotInternalSync(BlockdevSnapshotInternal),
    #[qapi(name = "blockdev-backup")]
    #[qapi(since = "2.4")]
    BlockdevBackup(BlockdevBackup),
    #[qapi(name = "blockdev-snapshot")]
    #[qapi(since = "2.8")]
    BlockdevSnapshot(BlockdevSnapshot),
    #[qapi(name = "block-dirty-bitmap-add")]
    #[qapi(since = "2.8")]
    BlockDirtyBitmapAdd(BlockDirtyBitmapAdd),
    #[qapi(name = "block-dirty-bitmap-clear")]
    #[qapi(since = "2.8")]
    BlockDirtyBitmapClear(BlockDirtyBitmap),
    #[qapi(name = "x-block-dirty-bitmap-enable")]
    #[qapi(since = "3.0")]
    #[qapi(until = "4.0")]
    XBlockDirtyBitmapEnable(BlockDirtyBitmap),
    #[qapi(name = "x-block-dirty-bitmap-disable")]
    #[qapi(since = "3.0")]
    #[qapi(until = "4.0")]
    XBlockDirtyBitmapDisable(BlockDirtyBitmap),
    #[qapi(name = "x-block-dirty-bitmap-merge")]
    #[qapi(since = "3.1")]
    #[qapi(until = "4.0")]
    XBlockDirtyBitmapMerge(BlockDirtyBitmapMerge),
    #[qapi(name = "block-dirty-bitmap-enable")]
    #[qapi(since = "4.0")]
    BlockDirtyBitmapEnable(BlockDirtyBitmap),
    #[qapi(name = "block-dirty-bitmap-disable")]
    #[qapi(since = "4.0")]
    BlockDirtyBitmapDisable(BlockDirtyBitmap),
    #[qapi(name = "block-dirty-bitmap-merge")]
    #[qapi(since = "4.0")]
    BlockDirtyBitmapMerge(BlockDirtyBitmapMerge),
    #[qapi(name = "block-dirty-bitmap-remove")]
    #[qapi(since = "4.2")]
    BlockDirtyBitmapRemove(BlockDirtyBitmap),
}
#[qapi(since = "2.0")]
pub enum TransactionActionKind {
    #[qapi(name = "blockdev-snapshot-sync")]
    BlockdevSnapshotSync,
    #[qapi(name = "drive-backup")]
    DriveBackup,
    #[qapi(name = "abort")]
    Abort,
    #[qapi(name = "blockdev-snapshot-internal-sync")]
    BlockdevSnapshotInternalSync,
    #[qapi(name = "blockdev-backup")]
    #[qapi(since = "2.4")]
    BlockdevBackup,
    #[qapi(name = "blockdev-snapshot")]
    #[qapi(since = "2.8")]
    BlockdevSnapshot,
    #[qapi(name = "block-dirty-bitmap-add")]
    #[qapi(since = "2.8")]
    BlockDirtyBitmapAdd,
    #[qapi(name = "block-dirty-bitmap-clear")]
    #[qapi(since = "2.8")]
    BlockDirtyBitmapClear,
    #[qapi(name = "x-block-dirty-bitmap-enable")]
    #[qapi(since = "3.0")]
    #[qapi(until = "4.0")]
    XBlockDirtyBitmapEnable,
    #[qapi(name = "x-block-dirty-bitmap-disable")]
    #[qapi(since = "3.0")]
    #[qapi(until = "4.0")]
    XBlockDirtyBitmapDisable,
    #[qapi(name = "x-block-dirty-bitmap-merge")]
    #[qapi(since = "3.1")]
    #[qapi(until = "4.0")]
    XBlockDirtyBitmapMerge,
    #[qapi(name = "block-dirty-bitmap-enable")]
    #[qapi(since = "4.0")]
    BlockDirtyBitmapEnable,
    #[qapi(name = "block-dirty-bitmap-disable")]
    #[qapi(since = "4.0")]
    BlockDirtyBitmapDisable,
    #[qapi(name = "block-dirty-bitmap-merge")]
    #[qapi(since = "4.0")]
    BlockDirtyBitmapMerge,
    #[qapi(name = "block-dirty-bitmap-remove")]
    #[qapi(since = "4.2")]
    BlockDirtyBitmapRemove,
}
#[qapi(name = "VMAppleVirtioBlkVariant")]
#[qapi(since = "9.2")]
pub enum VmAppleVirtioBlkVariant {
    #[qapi(name = "unspecified")]
    Unspecified,
    #[qapi(name = "root")]
    Root,
    #[qapi(name = "aux")]
    Aux,
}
#[qapi(since = "9.1")]
#[qapi(until = "9.2")]
pub enum VfioMigrationState {
    #[qapi(name = "stop")]
    Stop,
    #[qapi(name = "running")]
    Running,
    #[qapi(name = "stop-copy")]
    StopCopy,
    #[qapi(name = "resuming")]
    Resuming,
    #[qapi(name = "running-p2p")]
    RunningP2p,
    #[qapi(name = "pre-copy")]
    PreCopy,
    #[qapi(name = "pre-copy-p2p")]
    PreCopyP2p,
}
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
#[qapi(since = "2.1")]
pub enum WatchdogAction {
    #[qapi(name = "reset")]
    Reset,
    #[qapi(name = "shutdown")]
    Shutdown,
    #[qapi(name = "poweroff")]
    Poweroff,
    #[qapi(name = "pause")]
    Pause,
    #[qapi(name = "debug")]
    Debug,
    #[qapi(name = "none")]
    None,
    #[qapi(name = "inject-nmi")]
    InjectNmi,
}
#[qapi(since = "2.1")]
#[qapi(until = "2.11")]
pub enum WatchdogExpirationAction {
    #[qapi(name = "reset")]
    Reset,
    #[qapi(name = "shutdown")]
    Shutdown,
    #[qapi(name = "poweroff")]
    Poweroff,
    #[qapi(name = "pause")]
    Pause,
    #[qapi(name = "debug")]
    Debug,
    #[qapi(name = "none")]
    None,
    #[qapi(name = "inject-nmi")]
    #[qapi(since = "2.4")]
    InjectNmi,
}
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
#[qapi(since = "4.0")]
pub enum XDbgBlockGraphNodeType {
    #[qapi(name = "block-backend")]
    BlockBackend,
    #[qapi(name = "block-job")]
    BlockJob,
    #[qapi(name = "block-driver")]
    BlockDriver,
}
#[qapi(since = "6.0")]
pub enum YankInstanceBranch {
    #[qapi(name = "block-node")]
    BlockNode(YankInstanceBlockNode),
    #[qapi(name = "chardev")]
    Chardev(YankInstanceChardev),
}
#[qapi(since = "6.0")]
pub enum YankInstanceType {
    #[qapi(name = "block-node")]
    BlockNode,
    #[qapi(name = "chardev")]
    Chardev,
    #[qapi(name = "migration")]
    Migration,
}
#[qapi(since = "9.0")]
pub enum ZeroPageDetection {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "legacy")]
    Legacy,
    #[qapi(name = "multifd")]
    Multifd,
}
#[qapi(name = "ACPIOSTInfo")]
#[qapi(since = "2.1")]
pub struct AcpiostInfo {
    pub device: Option<String>,
    pub slot: String,
    #[qapi(name = "slot-type")]
    pub slot_type: AcpiSlotType,
    pub source: i64,
    pub status: i64,
}
#[qapi(event)]
#[qapi(name = "ACPI_DEVICE_OST")]
#[qapi(since = "2.1")]
pub struct AcpiDeviceOstEvent {
    pub info: AcpiostInfo,
}
#[qapi(since = "2.0")]
pub struct Abort {}
#[qapi(since = "1.6")]
pub struct AbortWrapper {
    pub data: Abort,
}
#[qapi(since = "10.2.0")]
pub struct AcceleratorInfo {
    pub enabled: Accelerator,
    pub present: Vec<Accelerator>,
}
#[qapi(since = "9.0")]
pub struct AcpiGenericInitiatorProperties {
    #[qapi(name = "pci-dev")]
    pub pci_dev: String,
    pub node: u32,
}
#[qapi(since = "9.2")]
pub struct AcpiGenericPortProperties {
    #[qapi(name = "pci-bus")]
    pub pci_bus: String,
    pub node: u32,
}
#[qapi(since = "1.5")]
pub struct AcpiTableOptions {
    pub sig: Option<String>,
    pub rev: Option<u8>,
    pub oem_id: Option<String>,
    pub oem_table_id: Option<String>,
    pub oem_rev: Option<u32>,
    pub asl_compiler_id: Option<String>,
    pub asl_compiler_rev: Option<u32>,
    pub file: Option<String>,
    pub data: Option<String>,
}
#[qapi(since = "1.2")]
pub struct AddfdInfo {
    #[qapi(name = "fdset-id")]
    pub fdset_id: i64,
    pub fd: i64,
}
#[qapi(since = "4.0")]
pub struct AnnounceParameters {
    pub initial: i64,
    pub max: i64,
    pub rounds: i64,
    pub step: i64,
    #[qapi(since = "4.1")]
    pub interfaces: Option<Vec<String>>,
    #[qapi(since = "4.1")]
    pub id: Option<String>,
}
#[qapi(since = "4.0")]
pub struct Audiodev {
    pub id: String,
    #[qapi(discriminator)]
    pub driver: AudiodevDriver,
    #[qapi(name = "timer-period")]
    pub timer_period: Option<u32>,
    #[qapi(union)]
    pub u: Option<AudiodevBranch>,
}
#[qapi(since = "4.0")]
pub struct AudiodevAlsaOptions {
    pub r#in: Option<AudiodevAlsaPerDirectionOptions>,
    pub out: Option<AudiodevAlsaPerDirectionOptions>,
    pub threshold: Option<u32>,
}
#[qapi(since = "4.0")]
pub struct AudiodevAlsaPerDirectionOptions {
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    pub frequency: Option<u32>,
    pub channels: Option<u32>,
    pub voices: Option<u32>,
    pub format: Option<AudioFormat>,
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    pub dev: Option<String>,
    #[qapi(name = "period-length")]
    pub period_length: Option<u32>,
    #[qapi(name = "try-poll")]
    pub try_poll: Option<bool>,
    #[qapi(name = "mixing-engine")]
    #[qapi(since = "4.2")]
    pub mixing_engine: Option<bool>,
}
#[qapi(since = "4.0")]
pub struct AudiodevCoreaudioOptions {
    pub r#in: Option<AudiodevCoreaudioPerDirectionOptions>,
    pub out: Option<AudiodevCoreaudioPerDirectionOptions>,
}
#[qapi(since = "4.0")]
pub struct AudiodevCoreaudioPerDirectionOptions {
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    pub frequency: Option<u32>,
    pub channels: Option<u32>,
    pub voices: Option<u32>,
    pub format: Option<AudioFormat>,
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    #[qapi(name = "buffer-count")]
    pub buffer_count: Option<u32>,
    #[qapi(name = "mixing-engine")]
    #[qapi(since = "4.2")]
    pub mixing_engine: Option<bool>,
}
#[qapi(since = "10.0")]
pub struct AudiodevDBusOptions {
    pub r#in: Option<AudiodevPerDirectionOptions>,
    pub out: Option<AudiodevPerDirectionOptions>,
    pub nsamples: Option<u32>,
}
#[qapi(since = "4.0")]
pub struct AudiodevDsoundOptions {
    pub r#in: Option<AudiodevPerDirectionOptions>,
    pub out: Option<AudiodevPerDirectionOptions>,
    pub latency: Option<u32>,
}
#[qapi(since = "4.0")]
pub struct AudiodevGenericOptions {
    pub r#in: Option<AudiodevPerDirectionOptions>,
    pub out: Option<AudiodevPerDirectionOptions>,
}
#[qapi(since = "5.1")]
pub struct AudiodevJackOptions {
    pub r#in: Option<AudiodevJackPerDirectionOptions>,
    pub out: Option<AudiodevJackPerDirectionOptions>,
}
#[qapi(since = "5.1")]
pub struct AudiodevJackPerDirectionOptions {
    #[qapi(name = "mixing-engine")]
    pub mixing_engine: Option<bool>,
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    pub frequency: Option<u32>,
    pub channels: Option<u32>,
    pub voices: Option<u32>,
    pub format: Option<AudioFormat>,
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    #[qapi(name = "server-name")]
    pub server_name: Option<String>,
    #[qapi(name = "client-name")]
    pub client_name: Option<String>,
    #[qapi(name = "connect-ports")]
    pub connect_ports: Option<String>,
    #[qapi(name = "start-server")]
    pub start_server: Option<bool>,
    #[qapi(name = "exact-name")]
    pub exact_name: Option<bool>,
}
#[qapi(since = "4.0")]
pub struct AudiodevOssOptions {
    pub r#in: Option<AudiodevOssPerDirectionOptions>,
    pub out: Option<AudiodevOssPerDirectionOptions>,
    #[qapi(name = "try-mmap")]
    pub try_mmap: Option<bool>,
    pub exclusive: Option<bool>,
    #[qapi(name = "dsp-policy")]
    pub dsp_policy: Option<u32>,
}
#[qapi(since = "4.0")]
pub struct AudiodevOssPerDirectionOptions {
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    pub frequency: Option<u32>,
    pub channels: Option<u32>,
    pub voices: Option<u32>,
    pub format: Option<AudioFormat>,
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    pub dev: Option<String>,
    #[qapi(name = "buffer-count")]
    pub buffer_count: Option<u32>,
    #[qapi(name = "try-poll")]
    pub try_poll: Option<bool>,
    #[qapi(name = "mixing-engine")]
    #[qapi(since = "4.2")]
    pub mixing_engine: Option<bool>,
}
#[qapi(since = "4.0")]
pub struct AudiodevPaOptions {
    pub r#in: Option<AudiodevPaPerDirectionOptions>,
    pub out: Option<AudiodevPaPerDirectionOptions>,
    pub server: Option<String>,
}
#[qapi(since = "4.0")]
pub struct AudiodevPaPerDirectionOptions {
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    pub frequency: Option<u32>,
    pub channels: Option<u32>,
    pub voices: Option<u32>,
    pub format: Option<AudioFormat>,
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    pub name: Option<String>,
    pub latency: Option<u32>,
    #[qapi(name = "mixing-engine")]
    #[qapi(since = "4.2")]
    pub mixing_engine: Option<bool>,
    #[qapi(name = "stream-name")]
    #[qapi(since = "4.2")]
    pub stream_name: Option<String>,
}
#[qapi(since = "4.0")]
pub struct AudiodevPerDirectionOptions {
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    pub frequency: Option<u32>,
    pub channels: Option<u32>,
    pub voices: Option<u32>,
    pub format: Option<AudioFormat>,
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    #[qapi(name = "mixing-engine")]
    #[qapi(since = "4.2")]
    pub mixing_engine: Option<bool>,
}
#[qapi(since = "8.1")]
pub struct AudiodevPipewireOptions {
    pub r#in: Option<AudiodevPipewirePerDirectionOptions>,
    pub out: Option<AudiodevPipewirePerDirectionOptions>,
}
#[qapi(since = "8.1")]
pub struct AudiodevPipewirePerDirectionOptions {
    #[qapi(name = "mixing-engine")]
    pub mixing_engine: Option<bool>,
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    pub frequency: Option<u32>,
    pub channels: Option<u32>,
    pub voices: Option<u32>,
    pub format: Option<AudioFormat>,
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    pub name: Option<String>,
    #[qapi(name = "stream-name")]
    pub stream_name: Option<String>,
    pub latency: Option<u32>,
}
#[qapi(since = "6.0")]
pub struct AudiodevSdlOptions {
    pub r#in: Option<AudiodevSdlPerDirectionOptions>,
    pub out: Option<AudiodevSdlPerDirectionOptions>,
}
#[qapi(since = "6.0")]
pub struct AudiodevSdlPerDirectionOptions {
    #[qapi(name = "mixing-engine")]
    pub mixing_engine: Option<bool>,
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    pub frequency: Option<u32>,
    pub channels: Option<u32>,
    pub voices: Option<u32>,
    pub format: Option<AudioFormat>,
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    #[qapi(name = "buffer-count")]
    pub buffer_count: Option<u32>,
}
#[qapi(since = "7.2")]
pub struct AudiodevSndioOptions {
    pub r#in: Option<AudiodevPerDirectionOptions>,
    pub out: Option<AudiodevPerDirectionOptions>,
    pub dev: Option<String>,
    pub latency: Option<u32>,
}
#[qapi(since = "4.0")]
pub struct AudiodevWavOptions {
    pub r#in: Option<AudiodevPerDirectionOptions>,
    pub out: Option<AudiodevPerDirectionOptions>,
    pub path: Option<String>,
}
#[qapi(since = "4.0")]
pub struct AuthZListFileProperties {
    pub filename: String,
    pub refresh: Option<bool>,
}
#[qapi(since = "4.0")]
pub struct AuthZListProperties {
    pub policy: Option<QAuthZListPolicy>,
    pub rules: Option<Vec<QAuthZListRule>>,
}
#[qapi(name = "AuthZPAMProperties")]
#[qapi(since = "4.0")]
pub struct AuthZpamProperties {
    pub service: String,
}
#[qapi(since = "4.0")]
pub struct AuthZSimpleProperties {
    pub identity: String,
}
#[qapi(event)]
#[qapi(name = "BALLOON_CHANGE")]
#[qapi(since = "1.2")]
pub struct BalloonChangeEvent {
    pub actual: i64,
}
#[qapi(event)]
#[qapi(name = "BLOCK_EXPORT_DELETED")]
#[qapi(since = "5.2")]
pub struct BlockExportDeletedEvent {
    pub id: String,
}
#[qapi(event)]
#[qapi(name = "BLOCK_IMAGE_CORRUPTED")]
#[qapi(since = "1.7")]
pub struct BlockImageCorruptedEvent {
    pub device: String,
    pub msg: String,
    pub offset: Option<i64>,
    pub size: Option<i64>,
    #[qapi(since = "2.2")]
    pub fatal: Option<bool>,
    #[qapi(name = "node-name")]
    #[qapi(since = "2.4")]
    pub node_name: Option<String>,
}
#[qapi(event)]
#[qapi(name = "BLOCK_IO_ERROR")]
#[qapi(since = "0.13.0")]
pub struct BlockIoErrorEvent {
    pub device: String,
    pub operation: IoOperationType,
    pub action: BlockErrorAction,
    #[qapi(since = "2.2")]
    pub nospace: Option<bool>,
    #[qapi(since = "2.2")]
    pub reason: Option<String>,
    #[qapi(name = "node-name")]
    #[qapi(since = "2.8")]
    pub node_name: Option<String>,
    #[qapi(name = "qom-path")]
    #[qapi(since = "9.2")]
    pub qom_path: Option<String>,
}
#[qapi(event)]
#[qapi(name = "BLOCK_JOB_CANCELLED")]
#[qapi(since = "1.1")]
pub struct BlockJobCancelledEvent {
    #[qapi(until = "2.4")]
    pub r#struct: Option<BlockJobType>,
    pub device: String,
    pub len: i64,
    pub offset: i64,
    pub speed: i64,
    #[qapi(since = "2.4")]
    pub r#type: Option<BlockJobType>,
}
#[qapi(event)]
#[qapi(name = "BLOCK_JOB_COMPLETED")]
#[qapi(since = "1.1")]
pub struct BlockJobCompletedEvent {
    #[qapi(until = "2.4")]
    pub r#struct: Option<BlockJobType>,
    pub device: String,
    pub len: i64,
    pub offset: i64,
    pub speed: i64,
    pub error: Option<String>,
    #[qapi(since = "2.4")]
    pub r#type: Option<BlockJobType>,
}
#[qapi(event)]
#[qapi(name = "BLOCK_JOB_ERROR")]
#[qapi(since = "1.3")]
pub struct BlockJobErrorEvent {
    pub device: String,
    pub operation: IoOperationType,
    pub action: BlockErrorAction,
}
#[qapi(event)]
#[qapi(name = "BLOCK_JOB_PENDING")]
#[qapi(since = "2.12")]
pub struct BlockJobPendingEvent {
    pub r#type: BlockJobType,
    pub id: String,
}
#[qapi(event)]
#[qapi(name = "BLOCK_JOB_READY")]
#[qapi(since = "1.3")]
pub struct BlockJobReadyEvent {
    #[qapi(until = "2.4")]
    pub r#struct: Option<BlockJobType>,
    pub device: String,
    pub len: i64,
    pub offset: i64,
    pub speed: i64,
    #[qapi(since = "2.4")]
    pub r#type: Option<BlockJobType>,
}
#[qapi(event)]
#[qapi(name = "BLOCK_WRITE_THRESHOLD")]
#[qapi(since = "2.3")]
pub struct BlockWriteThresholdEvent {
    #[qapi(name = "node-name")]
    pub node_name: String,
    #[qapi(name = "amount-exceeded")]
    pub amount_exceeded: u64,
    #[qapi(name = "write-threshold")]
    pub write_threshold: u64,
}
#[qapi(since = "4.2")]
pub struct BackupCommon {
    #[qapi(name = "job-id")]
    pub job_id: Option<String>,
    pub device: String,
    pub sync: MirrorSyncMode,
    pub speed: Option<i64>,
    pub bitmap: Option<String>,
    #[qapi(name = "bitmap-mode")]
    pub bitmap_mode: Option<BitmapSyncMode>,
    pub compress: Option<bool>,
    #[qapi(name = "on-source-error")]
    pub on_source_error: Option<BlockdevOnError>,
    #[qapi(name = "on-target-error")]
    pub on_target_error: Option<BlockdevOnError>,
    #[qapi(name = "auto-finalize")]
    pub auto_finalize: Option<bool>,
    #[qapi(name = "auto-dismiss")]
    pub auto_dismiss: Option<bool>,
    #[qapi(name = "filter-node-name")]
    pub filter_node_name: Option<String>,
    #[qapi(name = "x-perf")]
    #[qapi(since = "6.0")]
    pub x_perf: Option<BackupPerf>,
    #[qapi(name = "discard-source")]
    #[qapi(since = "9.1")]
    pub discard_source: Option<bool>,
    #[qapi(name = "on-cbw-error")]
    #[qapi(since = "10.1")]
    pub on_cbw_error: Option<OnCbwError>,
}
#[qapi(since = "6.0")]
pub struct BackupPerf {
    #[qapi(name = "use-copy-range")]
    pub use_copy_range: Option<bool>,
    #[qapi(name = "max-workers")]
    pub max_workers: Option<i64>,
    #[qapi(name = "max-chunk")]
    pub max_chunk: Option<i64>,
    #[qapi(name = "min-cluster-size")]
    #[qapi(since = "9.2")]
    pub min_cluster_size: Option<u64>,
}
#[qapi(since = "0.14.0")]
pub struct BalloonInfo {
    pub actual: i64,
    #[qapi(until = "1.4")]
    pub mem_swapped_in: Option<i64>,
    #[qapi(until = "1.4")]
    pub mem_swapped_out: Option<i64>,
    #[qapi(until = "1.4")]
    pub major_page_faults: Option<i64>,
    #[qapi(until = "1.4")]
    pub minor_page_faults: Option<i64>,
    #[qapi(until = "1.4")]
    pub free_mem: Option<i64>,
    #[qapi(until = "1.4")]
    pub total_mem: Option<i64>,
}
#[qapi(since = "5.2")]
pub struct BitmapMigrationBitmapAlias {
    pub name: String,
    pub alias: String,
    #[qapi(since = "6.0")]
    pub transform: Option<BitmapMigrationBitmapAliasTransform>,
}
#[qapi(since = "6.0")]
pub struct BitmapMigrationBitmapAliasTransform {
    pub persistent: Option<bool>,
}
#[qapi(since = "5.2")]
pub struct BitmapMigrationNodeAlias {
    #[qapi(name = "node-name")]
    pub node_name: String,
    pub alias: String,
    pub bitmaps: Vec<BitmapMigrationBitmapAlias>,
}
#[qapi(since = "2.0")]
pub struct BlkdebugInjectErrorOptions {
    pub event: BlkdebugEvent,
    pub state: Option<i64>,
    pub errno: Option<i64>,
    pub sector: Option<i64>,
    pub once: Option<bool>,
    pub immediately: Option<bool>,
    #[qapi(since = "4.1")]
    pub iotype: Option<BlkdebugIoType>,
}
#[qapi(since = "2.0")]
pub struct BlkdebugSetStateOptions {
    pub event: BlkdebugEvent,
    pub state: Option<i64>,
    pub new_state: i64,
}
#[qapi(since = "8.0")]
pub struct BlockChildInfo {
    pub name: String,
    pub info: Box<BlockGraphInfo>,
}
#[qapi(since = "0.14.0")]
pub struct BlockDeviceInfo {
    pub file: String,
    pub ro: bool,
    pub drv: String,
    pub backing_file: Option<String>,
    pub encrypted: bool,
    pub bps: i64,
    pub bps_rd: i64,
    pub bps_wr: i64,
    pub iops: i64,
    pub iops_rd: i64,
    pub iops_wr: i64,
    #[qapi(since = "1.2")]
    pub backing_file_depth: Option<i64>,
    #[qapi(since = "1.2")]
    #[qapi(until = "6.0")]
    pub encryption_key_missing: Option<bool>,
    #[qapi(since = "1.6")]
    pub image: Option<ImageInfo>,
    #[qapi(since = "1.7")]
    pub bps_max: Option<i64>,
    #[qapi(since = "1.7")]
    pub bps_rd_max: Option<i64>,
    #[qapi(since = "1.7")]
    pub bps_wr_max: Option<i64>,
    #[qapi(since = "1.7")]
    pub iops_max: Option<i64>,
    #[qapi(since = "1.7")]
    pub iops_rd_max: Option<i64>,
    #[qapi(since = "1.7")]
    pub iops_wr_max: Option<i64>,
    #[qapi(since = "1.7")]
    pub iops_size: Option<i64>,
    #[qapi(name = "node-name")]
    #[qapi(since = "2.1")]
    pub node_name: Option<String>,
    #[qapi(since = "2.1")]
    pub detect_zeroes: Option<BlockdevDetectZeroesOptions>,
    #[qapi(since = "2.3")]
    pub cache: Option<BlockdevCacheInfo>,
    #[qapi(since = "2.3")]
    pub write_threshold: Option<i64>,
    #[qapi(since = "2.4")]
    pub group: Option<String>,
    #[qapi(since = "2.6")]
    pub bps_max_length: Option<i64>,
    #[qapi(since = "2.6")]
    pub bps_rd_max_length: Option<i64>,
    #[qapi(since = "2.6")]
    pub bps_wr_max_length: Option<i64>,
    #[qapi(since = "2.6")]
    pub iops_max_length: Option<i64>,
    #[qapi(since = "2.6")]
    pub iops_rd_max_length: Option<i64>,
    #[qapi(since = "2.6")]
    pub iops_wr_max_length: Option<i64>,
    #[qapi(name = "dirty-bitmaps")]
    #[qapi(since = "4.2")]
    pub dirty_bitmaps: Option<Vec<BlockDirtyInfo>>,
    #[qapi(since = "10.0")]
    pub active: Option<bool>,
    #[qapi(since = "10.1")]
    pub children: Option<Vec<BlockdevChild>>,
}
#[qapi(since = "1.7")]
#[qapi(until = "5.2")]
pub struct BlockDeviceMapEntry {
    pub start: i64,
    pub length: i64,
    pub depth: i64,
    pub zero: bool,
    pub data: bool,
    pub offset: Option<i64>,
}
#[qapi(since = "0.14.0")]
pub struct BlockDeviceStats {
    pub rd_bytes: i64,
    pub wr_bytes: i64,
    pub rd_operations: i64,
    pub wr_operations: i64,
    pub flush_operations: i64,
    pub flush_total_time_ns: i64,
    pub wr_total_time_ns: i64,
    pub rd_total_time_ns: i64,
    pub wr_highest_offset: i64,
    #[qapi(since = "2.3")]
    pub rd_merged: Option<i64>,
    #[qapi(since = "2.3")]
    pub wr_merged: Option<i64>,
    #[qapi(since = "2.5")]
    pub idle_time_ns: Option<i64>,
    #[qapi(since = "2.5")]
    pub failed_rd_operations: Option<i64>,
    #[qapi(since = "2.5")]
    pub failed_wr_operations: Option<i64>,
    #[qapi(since = "2.5")]
    pub failed_flush_operations: Option<i64>,
    #[qapi(since = "2.5")]
    pub invalid_rd_operations: Option<i64>,
    #[qapi(since = "2.5")]
    pub invalid_wr_operations: Option<i64>,
    #[qapi(since = "2.5")]
    pub invalid_flush_operations: Option<i64>,
    #[qapi(since = "2.5")]
    pub account_invalid: Option<bool>,
    #[qapi(since = "2.5")]
    pub account_failed: Option<bool>,
    #[qapi(since = "2.5")]
    pub timed_stats: Option<Vec<BlockDeviceTimedStats>>,
    #[qapi(since = "2.12")]
    #[qapi(until = "4.0")]
    pub x_rd_latency_histogram: Option<BlockLatencyHistogramInfo>,
    #[qapi(since = "2.12")]
    #[qapi(until = "4.0")]
    pub x_wr_latency_histogram: Option<BlockLatencyHistogramInfo>,
    #[qapi(since = "2.12")]
    #[qapi(until = "4.0")]
    pub x_flush_latency_histogram: Option<BlockLatencyHistogramInfo>,
    #[qapi(since = "4.0")]
    pub rd_latency_histogram: Option<BlockLatencyHistogramInfo>,
    #[qapi(since = "4.0")]
    pub wr_latency_histogram: Option<BlockLatencyHistogramInfo>,
    #[qapi(since = "4.0")]
    pub flush_latency_histogram: Option<BlockLatencyHistogramInfo>,
    #[qapi(since = "4.2")]
    pub unmap_bytes: Option<i64>,
    #[qapi(since = "4.2")]
    pub unmap_operations: Option<i64>,
    #[qapi(since = "4.2")]
    pub unmap_total_time_ns: Option<i64>,
    #[qapi(since = "4.2")]
    pub unmap_merged: Option<i64>,
    #[qapi(since = "4.2")]
    pub failed_unmap_operations: Option<i64>,
    #[qapi(since = "4.2")]
    pub invalid_unmap_operations: Option<i64>,
    #[qapi(since = "8.1")]
    pub zone_append_bytes: Option<i64>,
    #[qapi(since = "8.1")]
    pub zone_append_operations: Option<i64>,
    #[qapi(since = "8.1")]
    pub zone_append_total_time_ns: Option<i64>,
    #[qapi(since = "8.1")]
    pub zone_append_merged: Option<i64>,
    #[qapi(since = "8.1")]
    pub failed_zone_append_operations: Option<i64>,
    #[qapi(since = "8.1")]
    pub invalid_zone_append_operations: Option<i64>,
    #[qapi(since = "8.1")]
    pub zone_append_latency_histogram: Option<BlockLatencyHistogramInfo>,
}
#[qapi(since = "2.5")]
pub struct BlockDeviceTimedStats {
    pub interval_length: i64,
    pub min_rd_latency_ns: i64,
    pub max_rd_latency_ns: i64,
    pub avg_rd_latency_ns: i64,
    pub min_wr_latency_ns: i64,
    pub max_wr_latency_ns: i64,
    pub avg_wr_latency_ns: i64,
    pub min_flush_latency_ns: i64,
    pub max_flush_latency_ns: i64,
    pub avg_flush_latency_ns: i64,
    pub avg_rd_queue_depth: f64,
    pub avg_wr_queue_depth: f64,
    #[qapi(since = "8.1")]
    pub min_zone_append_latency_ns: Option<i64>,
    #[qapi(since = "8.1")]
    pub max_zone_append_latency_ns: Option<i64>,
    #[qapi(since = "8.1")]
    pub avg_zone_append_latency_ns: Option<i64>,
    #[qapi(since = "8.1")]
    pub avg_zone_append_queue_depth: Option<f64>,
}
#[qapi(since = "2.4")]
pub struct BlockDirtyBitmap {
    pub node: String,
    pub name: String,
}
#[qapi(since = "2.4")]
pub struct BlockDirtyBitmapAdd {
    pub node: String,
    pub name: String,
    pub granularity: Option<u32>,
    #[qapi(since = "2.10")]
    pub persistent: Option<bool>,
    #[qapi(since = "2.10")]
    #[qapi(until = "4.2")]
    pub autoload: Option<bool>,
    #[qapi(name = "x-disabled")]
    #[qapi(since = "3.0")]
    #[qapi(until = "4.0")]
    pub x_disabled: Option<bool>,
    #[qapi(since = "4.0")]
    pub disabled: Option<bool>,
}
#[qapi(since = "2.5")]
pub struct BlockDirtyBitmapAddWrapper {
    pub data: BlockDirtyBitmapAdd,
}
#[qapi(since = "3.0")]
pub struct BlockDirtyBitmapMerge {
    pub node: String,
    #[qapi(until = "4.0")]
    pub dst_name: Option<String>,
    #[qapi(until = "4.0")]
    pub src_name: Option<String>,
    #[qapi(since = "4.0")]
    pub target: Option<String>,
    #[qapi(since = "4.0")]
    pub bitmaps: Option<Vec<String>>,
}
#[qapi(since = "4.0")]
pub struct BlockDirtyBitmapMergeWrapper {
    pub data: BlockDirtyBitmapMerge,
}
#[qapi(since = "2.10")]
pub struct BlockDirtyBitmapSha256 {
    pub sha256: String,
}
#[qapi(since = "2.5")]
pub struct BlockDirtyBitmapWrapper {
    pub data: BlockDirtyBitmap,
}
#[qapi(since = "1.3")]
pub struct BlockDirtyInfo {
    pub count: i64,
    #[qapi(since = "1.4")]
    pub granularity: Option<i64>,
    #[qapi(since = "2.4")]
    pub name: Option<String>,
    #[qapi(since = "2.4")]
    #[qapi(until = "6.0")]
    pub status: Option<DirtyBitmapStatus>,
    #[qapi(since = "4.0")]
    pub recording: Option<bool>,
    #[qapi(since = "4.0")]
    pub busy: Option<bool>,
    #[qapi(since = "4.0")]
    pub persistent: Option<bool>,
    #[qapi(since = "4.0")]
    pub inconsistent: Option<bool>,
}
#[qapi(since = "4.2")]
#[qapi(until = "5.2")]
pub struct BlockExport {
    #[qapi(discriminator)]
    pub r#type: BlockExportType,
    #[qapi(union)]
    pub u: Option<BlockExportBranch>,
}
#[qapi(since = "5.2")]
pub struct BlockExportInfo {
    pub id: String,
    pub r#type: BlockExportType,
    #[qapi(name = "node-name")]
    pub node_name: String,
    #[qapi(name = "shutting-down")]
    pub shutting_down: bool,
}
#[qapi(since = "5.0")]
#[qapi(until = "5.2")]
pub struct BlockExportNbd {
    pub device: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub writable: Option<bool>,
    pub bitmap: Option<String>,
}
#[qapi(since = "4.2")]
pub struct BlockExportOptions {
    #[qapi(discriminator)]
    pub r#type: BlockExportType,
    pub id: String,
    #[qapi(name = "fixed-iothread")]
    pub fixed_iothread: Option<bool>,
    pub iothread: Option<String>,
    #[qapi(name = "node-name")]
    pub node_name: String,
    pub writable: Option<bool>,
    pub writethrough: Option<bool>,
    #[qapi(union)]
    pub u: Option<BlockExportOptionsBranch>,
    #[qapi(name = "allow-inactive")]
    #[qapi(since = "10.0")]
    pub allow_inactive: Option<bool>,
}
#[qapi(condition = "defined(CONFIG_FUSE)")]
#[qapi(since = "6.0")]
pub struct BlockExportOptionsFuse {
    pub mountpoint: String,
    pub growable: Option<bool>,
    #[qapi(name = "allow-other")]
    #[qapi(since = "6.1")]
    pub allow_other: Option<FuseExportAllowOther>,
}
#[qapi(since = "5.2")]
pub struct BlockExportOptionsNbd {
    pub name: Option<String>,
    pub description: Option<String>,
    pub bitmaps: Option<Vec<String>>,
    #[qapi(name = "allocation-depth")]
    pub allocation_depth: Option<bool>,
}
#[qapi(since = "5.0")]
pub struct BlockExportOptionsNbdBase {
    pub name: Option<String>,
    pub description: Option<String>,
}
#[qapi(since = "7.1")]
pub struct BlockExportOptionsVduseBlk {
    pub name: String,
    #[qapi(name = "num-queues")]
    pub num_queues: Option<u16>,
    #[qapi(name = "queue-size")]
    pub queue_size: Option<u16>,
    #[qapi(name = "logical-block-size")]
    pub logical_block_size: Option<u64>,
    pub serial: Option<String>,
}
#[qapi(since = "5.2")]
pub struct BlockExportOptionsVhostUserBlk {
    pub addr: SocketAddress,
    #[qapi(name = "logical-block-size")]
    pub logical_block_size: Option<u64>,
    #[qapi(name = "num-queues")]
    pub num_queues: Option<u16>,
}
#[qapi(since = "8.0")]
pub struct BlockGraphInfo {
    pub filename: String,
    pub format: String,
    #[qapi(name = "dirty-flag")]
    pub dirty_flag: Option<bool>,
    #[qapi(name = "actual-size")]
    pub actual_size: Option<i64>,
    #[qapi(name = "virtual-size")]
    pub virtual_size: i64,
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<i64>,
    pub encrypted: Option<bool>,
    pub compressed: Option<bool>,
    #[qapi(name = "backing-filename")]
    pub backing_filename: Option<String>,
    #[qapi(name = "full-backing-filename")]
    pub full_backing_filename: Option<String>,
    #[qapi(name = "backing-filename-format")]
    pub backing_filename_format: Option<String>,
    pub snapshots: Option<Vec<SnapshotInfo>>,
    #[qapi(name = "format-specific")]
    pub format_specific: Option<ImageInfoSpecific>,
    pub children: Box<Vec<BlockChildInfo>>,
    #[qapi(since = "10.2")]
    pub limits: Option<BlockLimitsInfo>,
}
#[qapi(name = "BlockIOThrottle")]
#[qapi(since = "1.1")]
pub struct BlockIoThrottle {
    pub device: String,
    pub bps: i64,
    pub bps_rd: i64,
    pub bps_wr: i64,
    pub iops: i64,
    pub iops_rd: i64,
    pub iops_wr: i64,
    pub bps_max: Option<i64>,
    pub bps_rd_max: Option<i64>,
    pub bps_wr_max: Option<i64>,
    pub iops_max: Option<i64>,
    pub iops_rd_max: Option<i64>,
    pub iops_wr_max: Option<i64>,
    pub bps_max_length: Option<i64>,
    pub bps_rd_max_length: Option<i64>,
    pub bps_wr_max_length: Option<i64>,
    pub iops_max_length: Option<i64>,
    pub iops_rd_max_length: Option<i64>,
    pub iops_wr_max_length: Option<i64>,
    pub iops_size: Option<i64>,
    pub group: Option<String>,
    #[qapi(since = "2.8")]
    pub id: Option<String>,
}
#[qapi(since = "0.14.0")]
pub struct BlockInfo {
    pub device: String,
    pub r#type: String,
    pub removable: bool,
    pub locked: bool,
    pub inserted: Option<BlockDeviceInfo>,
    pub tray_open: Option<bool>,
    #[qapi(name = "io-status")]
    pub io_status: Option<BlockDeviceIoStatus>,
    #[qapi(since = "1.3")]
    #[qapi(until = "2.1")]
    pub dirty: Option<BlockDirtyInfo>,
    #[qapi(name = "dirty-bitmaps")]
    #[qapi(since = "2.1")]
    #[qapi(until = "6.0")]
    pub dirty_bitmaps: Option<Vec<BlockDirtyInfo>>,
    #[qapi(since = "2.10")]
    pub qdev: Option<String>,
}
#[qapi(since = "8.2")]
pub struct BlockJobChangeOptions {
    pub id: String,
    #[qapi(discriminator)]
    pub r#type: JobType,
    #[qapi(union)]
    pub u: Option<BlockJobChangeOptionsBranch>,
}
#[qapi(since = "8.2")]
pub struct BlockJobChangeOptionsMirror {
    #[qapi(name = "copy-mode")]
    pub copy_mode: MirrorCopyMode,
}
#[qapi(since = "1.1")]
pub struct BlockJobInfo {
    pub r#type: String,
    pub device: String,
    pub len: i64,
    pub offset: i64,
    pub speed: i64,
    #[qapi(since = "1.3")]
    pub busy: Option<bool>,
    #[qapi(since = "1.3")]
    pub paused: Option<bool>,
    #[qapi(name = "io-status")]
    #[qapi(since = "1.3")]
    pub io_status: Option<BlockDeviceIoStatus>,
    #[qapi(since = "2.2")]
    pub ready: Option<bool>,
    #[qapi(since = "2.12")]
    pub status: Option<BlockJobStatus>,
    #[qapi(name = "auto-finalize")]
    #[qapi(since = "2.12")]
    pub auto_finalize: Option<bool>,
    #[qapi(name = "auto-dismiss")]
    #[qapi(since = "2.12")]
    pub auto_dismiss: Option<bool>,
    #[qapi(since = "3.0")]
    pub error: Option<String>,
    #[qapi(union)]
    #[qapi(since = "8.2")]
    pub u: Option<BlockJobInfoBranch>,
}
#[qapi(since = "8.2")]
pub struct BlockJobInfoMirror {
    #[qapi(name = "actively-synced")]
    pub actively_synced: bool,
}
#[qapi(since = "2.12")]
pub struct BlockLatencyHistogramInfo {
    pub boundaries: Vec<u64>,
    pub bins: Vec<u64>,
}
#[qapi(since = "10.2")]
pub struct BlockLimitsInfo {
    #[qapi(name = "request-alignment")]
    pub request_alignment: u32,
    #[qapi(name = "max-discard")]
    pub max_discard: Option<u64>,
    #[qapi(name = "discard-alignment")]
    pub discard_alignment: Option<u32>,
    #[qapi(name = "max-write-zeroes")]
    pub max_write_zeroes: Option<u64>,
    #[qapi(name = "write-zeroes-alignment")]
    pub write_zeroes_alignment: Option<u32>,
    #[qapi(name = "opt-transfer")]
    pub opt_transfer: Option<u32>,
    #[qapi(name = "max-transfer")]
    pub max_transfer: Option<u32>,
    #[qapi(name = "max-hw-transfer")]
    pub max_hw_transfer: Option<u32>,
    #[qapi(name = "max-iov")]
    pub max_iov: i64,
    #[qapi(name = "max-hw-iov")]
    pub max_hw_iov: Option<i64>,
    #[qapi(name = "min-mem-alignment")]
    pub min_mem_alignment: u64,
    #[qapi(name = "opt-mem-alignment")]
    pub opt_mem_alignment: u64,
}
#[qapi(since = "2.10")]
pub struct BlockMeasureInfo {
    pub required: i64,
    #[qapi(name = "fully-allocated")]
    pub fully_allocated: i64,
    #[qapi(since = "5.1")]
    pub bitmaps: Option<i64>,
}
#[qapi(since = "8.0")]
pub struct BlockNodeInfo {
    pub filename: String,
    pub format: String,
    #[qapi(name = "dirty-flag")]
    pub dirty_flag: Option<bool>,
    #[qapi(name = "actual-size")]
    pub actual_size: Option<i64>,
    #[qapi(name = "virtual-size")]
    pub virtual_size: i64,
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<i64>,
    pub encrypted: Option<bool>,
    pub compressed: Option<bool>,
    #[qapi(name = "backing-filename")]
    pub backing_filename: Option<String>,
    #[qapi(name = "full-backing-filename")]
    pub full_backing_filename: Option<String>,
    #[qapi(name = "backing-filename-format")]
    pub backing_filename_format: Option<String>,
    pub snapshots: Option<Vec<SnapshotInfo>>,
    #[qapi(name = "format-specific")]
    pub format_specific: Option<ImageInfoSpecific>,
    #[qapi(since = "10.2")]
    pub limits: Option<BlockLimitsInfo>,
}
#[qapi(since = "0.14.0")]
pub struct BlockStats {
    pub device: Option<String>,
    pub stats: BlockDeviceStats,
    pub parent: Option<Box<BlockStats>>,
    #[qapi(since = "2.1")]
    pub backing: Option<Box<BlockStats>>,
    #[qapi(name = "node-name")]
    #[qapi(since = "2.3")]
    pub node_name: Option<String>,
    #[qapi(since = "3.0")]
    pub qdev: Option<String>,
    #[qapi(name = "driver-specific")]
    #[qapi(since = "4.2")]
    pub driver_specific: Option<BlockStatsSpecific>,
}
#[qapi(since = "4.2")]
pub struct BlockStatsSpecific {
    #[qapi(discriminator)]
    pub driver: BlockdevDriver,
    #[qapi(union)]
    pub u: Option<BlockStatsSpecificBranch>,
}
#[qapi(since = "4.2")]
pub struct BlockStatsSpecificFile {
    #[qapi(name = "discard-nb-ok")]
    pub discard_nb_ok: u64,
    #[qapi(name = "discard-nb-failed")]
    pub discard_nb_failed: u64,
    #[qapi(name = "discard-bytes-ok")]
    pub discard_bytes_ok: u64,
}
#[qapi(since = "5.2")]
pub struct BlockStatsSpecificNvme {
    #[qapi(name = "completion-errors")]
    pub completion_errors: u64,
    #[qapi(name = "aligned-accesses")]
    pub aligned_accesses: u64,
    #[qapi(name = "unaligned-accesses")]
    pub unaligned_accesses: u64,
}
#[qapi(since = "1.1")]
#[qapi(until = "1.6")]
pub struct BlockdevAction {
    #[qapi(discriminator)]
    pub r#type: BlockdevActionKind,
    #[qapi(union)]
    pub u: Option<BlockdevActionBranch>,
}
#[qapi(since = "5.1")]
pub struct BlockdevAmendOptions {
    #[qapi(discriminator)]
    pub driver: BlockdevDriver,
    #[qapi(union)]
    pub u: Option<BlockdevAmendOptionsBranch>,
}
#[qapi(name = "BlockdevAmendOptionsLUKS")]
#[qapi(since = "5.1")]
pub struct BlockdevAmendOptionsLuks {
    pub state: QCryptoBlockLuksKeyslotState,
    #[qapi(name = "new-secret")]
    pub new_secret: Option<String>,
    #[qapi(name = "old-secret")]
    pub old_secret: Option<String>,
    pub keyslot: Option<i64>,
    #[qapi(name = "iter-time")]
    pub iter_time: Option<i64>,
    pub secret: Option<String>,
}
#[qapi(since = "5.1")]
pub struct BlockdevAmendOptionsQcow2 {
    pub encrypt: Option<QCryptoBlockAmendOptions>,
}
#[qapi(since = "2.3")]
pub struct BlockdevBackup {
    pub device: String,
    pub target: String,
    pub sync: MirrorSyncMode,
    pub speed: Option<i64>,
    #[qapi(name = "on-source-error")]
    pub on_source_error: Option<BlockdevOnError>,
    #[qapi(name = "on-target-error")]
    pub on_target_error: Option<BlockdevOnError>,
    #[qapi(name = "job-id")]
    #[qapi(since = "2.7")]
    pub job_id: Option<String>,
    #[qapi(since = "2.8")]
    pub compress: Option<bool>,
    #[qapi(name = "auto-finalize")]
    #[qapi(since = "2.12")]
    pub auto_finalize: Option<bool>,
    #[qapi(name = "auto-dismiss")]
    #[qapi(since = "2.12")]
    pub auto_dismiss: Option<bool>,
    #[qapi(since = "3.1")]
    pub bitmap: Option<String>,
    #[qapi(name = "bitmap-mode")]
    #[qapi(since = "4.2")]
    pub bitmap_mode: Option<BitmapSyncMode>,
    #[qapi(name = "filter-node-name")]
    #[qapi(since = "4.2")]
    pub filter_node_name: Option<String>,
    #[qapi(name = "x-perf")]
    #[qapi(since = "6.0")]
    pub x_perf: Option<BackupPerf>,
    #[qapi(name = "discard-source")]
    #[qapi(since = "9.1")]
    pub discard_source: Option<bool>,
    #[qapi(name = "on-cbw-error")]
    #[qapi(since = "10.1")]
    pub on_cbw_error: Option<OnCbwError>,
}
#[qapi(since = "2.3")]
pub struct BlockdevBackupWrapper {
    pub data: BlockdevBackup,
}
#[qapi(since = "2.3")]
pub struct BlockdevCacheInfo {
    pub writeback: bool,
    pub direct: bool,
    #[qapi(name = "no-flush")]
    pub no_flush: bool,
}
#[qapi(since = "1.7")]
pub struct BlockdevCacheOptions {
    #[qapi(until = "2.6")]
    pub writeback: Option<bool>,
    pub direct: Option<bool>,
    #[qapi(name = "no-flush")]
    pub no_flush: Option<bool>,
}
#[qapi(since = "10.1")]
pub struct BlockdevChild {
    pub child: String,
    #[qapi(name = "node-name")]
    pub node_name: String,
}
#[qapi(since = "2.12")]
#[qapi(until = "3.0")]
pub struct BlockdevCreateNotSupported {}
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptions {
    #[qapi(discriminator)]
    pub driver: BlockdevDriver,
    #[qapi(union)]
    pub u: Option<BlockdevCreateOptionsBranch>,
}
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsFile {
    pub filename: String,
    pub size: u64,
    pub preallocation: Option<PreallocMode>,
    pub nocow: Option<bool>,
    #[qapi(name = "extent-size-hint")]
    #[qapi(since = "5.1")]
    pub extent_size_hint: Option<u64>,
}
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsGluster {
    pub location: BlockdevOptionsGluster,
    pub size: u64,
    pub preallocation: Option<PreallocMode>,
}
#[qapi(name = "BlockdevCreateOptionsLUKS")]
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsLuks {
    #[qapi(name = "key-secret")]
    pub key_secret: Option<String>,
    #[qapi(name = "cipher-alg")]
    pub cipher_alg: Option<QCryptoCipherAlgorithm>,
    #[qapi(name = "cipher-mode")]
    pub cipher_mode: Option<QCryptoCipherMode>,
    #[qapi(name = "ivgen-alg")]
    pub ivgen_alg: Option<QCryptoIvGenAlgorithm>,
    #[qapi(name = "ivgen-hash-alg")]
    pub ivgen_hash_alg: Option<QCryptoHashAlgorithm>,
    #[qapi(name = "hash-alg")]
    pub hash_alg: Option<QCryptoHashAlgorithm>,
    #[qapi(name = "iter-time")]
    pub iter_time: Option<i64>,
    pub file: BlockdevRef,
    pub size: u64,
    #[qapi(since = "4.2")]
    pub preallocation: Option<PreallocMode>,
    #[qapi(name = "detached-header")]
    #[qapi(since = "9.0")]
    #[qapi(until = "9.1")]
    pub detached_header: Option<bool>,
    #[qapi(since = "9.0")]
    pub header: Option<BlockdevRef>,
}
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsNfs {
    pub location: BlockdevOptionsNfs,
    pub size: u64,
}
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsParallels {
    pub file: BlockdevRef,
    pub size: u64,
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<u64>,
}
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsQcow {
    pub file: BlockdevRef,
    pub size: u64,
    #[qapi(name = "backing-file")]
    pub backing_file: Option<String>,
    pub encrypt: Option<QCryptoBlockCreateOptions>,
}
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsQcow2 {
    pub file: BlockdevRef,
    pub size: u64,
    pub version: Option<BlockdevQcow2Version>,
    #[qapi(name = "backing-file")]
    pub backing_file: Option<String>,
    #[qapi(name = "backing-fmt")]
    pub backing_fmt: Option<BlockdevDriver>,
    pub encrypt: Option<QCryptoBlockCreateOptions>,
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<u64>,
    pub preallocation: Option<PreallocMode>,
    #[qapi(name = "lazy-refcounts")]
    pub lazy_refcounts: Option<bool>,
    #[qapi(name = "refcount-bits")]
    pub refcount_bits: Option<i64>,
    #[qapi(name = "data-file")]
    #[qapi(since = "4.0")]
    pub data_file: Option<BlockdevRef>,
    #[qapi(name = "data-file-raw")]
    #[qapi(since = "4.0")]
    pub data_file_raw: Option<bool>,
    #[qapi(name = "compression-type")]
    #[qapi(since = "5.1")]
    pub compression_type: Option<Qcow2CompressionType>,
    #[qapi(name = "extended-l2")]
    #[qapi(since = "5.2")]
    pub extended_l2: Option<bool>,
}
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsQed {
    pub file: BlockdevRef,
    pub size: u64,
    #[qapi(name = "backing-file")]
    pub backing_file: Option<String>,
    #[qapi(name = "backing-fmt")]
    pub backing_fmt: Option<BlockdevDriver>,
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<u64>,
    #[qapi(name = "table-size")]
    pub table_size: Option<i64>,
}
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsRbd {
    pub location: BlockdevOptionsRbd,
    pub size: u64,
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<u64>,
    #[qapi(since = "6.1")]
    pub encrypt: Option<RbdEncryptionCreateOptions>,
}
#[qapi(since = "2.12")]
#[qapi(until = "6.1")]
pub struct BlockdevCreateOptionsSheepdog {
    pub location: BlockdevOptionsSheepdog,
    pub size: u64,
    #[qapi(name = "backing-file")]
    pub backing_file: Option<String>,
    pub preallocation: Option<PreallocMode>,
    pub redundancy: Option<SheepdogRedundancy>,
    #[qapi(name = "object-size")]
    pub object_size: Option<u64>,
}
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsSsh {
    pub location: BlockdevOptionsSsh,
    pub size: u64,
}
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsVdi {
    pub file: BlockdevRef,
    pub size: u64,
    pub preallocation: Option<PreallocMode>,
}
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsVhdx {
    pub file: BlockdevRef,
    pub size: u64,
    #[qapi(name = "log-size")]
    pub log_size: Option<u64>,
    #[qapi(name = "block-size")]
    pub block_size: Option<u64>,
    pub subformat: Option<BlockdevVhdxSubformat>,
    #[qapi(name = "block-state-zero")]
    pub block_state_zero: Option<bool>,
}
#[qapi(since = "4.0")]
pub struct BlockdevCreateOptionsVmdk {
    pub file: BlockdevRef,
    pub size: u64,
    pub extents: Option<Vec<BlockdevRef>>,
    pub subformat: Option<BlockdevVmdkSubformat>,
    #[qapi(name = "backing-file")]
    pub backing_file: Option<String>,
    #[qapi(name = "adapter-type")]
    pub adapter_type: Option<BlockdevVmdkAdapterType>,
    pub hwversion: Option<String>,
    #[qapi(name = "zeroed-grain")]
    pub zeroed_grain: Option<bool>,
    #[qapi(since = "6.2")]
    pub toolsversion: Option<String>,
}
#[qapi(since = "2.12")]
pub struct BlockdevCreateOptionsVpc {
    pub file: BlockdevRef,
    pub size: u64,
    pub subformat: Option<BlockdevVpcSubformat>,
    #[qapi(name = "force-size")]
    pub force_size: Option<bool>,
}
#[qapi(since = "1.7")]
pub struct BlockdevOptions {
    #[qapi(discriminator)]
    pub driver: BlockdevDriver,
    #[qapi(until = "2.8")]
    pub id: Option<String>,
    #[qapi(name = "node-name")]
    pub node_name: Option<String>,
    pub discard: Option<BlockdevDiscardOptions>,
    pub cache: Option<BlockdevCacheOptions>,
    #[qapi(until = "2.8")]
    pub aio: Option<BlockdevAioOptions>,
    #[qapi(until = "2.7")]
    pub rerror: Option<BlockdevOnError>,
    #[qapi(until = "2.7")]
    pub werror: Option<BlockdevOnError>,
    #[qapi(name = "read-only")]
    pub read_only: Option<bool>,
    #[qapi(union)]
    pub u: Option<Box<BlockdevOptionsBranch>>,
    #[qapi(name = "detect-zeroes")]
    #[qapi(since = "2.1")]
    pub detect_zeroes: Option<BlockdevDetectZeroesOptions>,
    #[qapi(name = "stats-account-invalid")]
    #[qapi(since = "2.5")]
    #[qapi(until = "2.7")]
    pub stats_account_invalid: Option<bool>,
    #[qapi(name = "stats-account-failed")]
    #[qapi(since = "2.5")]
    #[qapi(until = "2.7")]
    pub stats_account_failed: Option<bool>,
    #[qapi(name = "stats-intervals")]
    #[qapi(since = "2.5")]
    #[qapi(until = "2.7")]
    pub stats_intervals: Option<Vec<i64>>,
    #[qapi(name = "force-share")]
    #[qapi(since = "2.10")]
    pub force_share: Option<bool>,
    #[qapi(name = "auto-read-only")]
    #[qapi(since = "3.1")]
    pub auto_read_only: Option<bool>,
    #[qapi(since = "10.0")]
    pub active: Option<bool>,
}
#[qapi(since = "2.2")]
#[qapi(until = "2.9")]
pub struct BlockdevOptionsArchipelago {
    pub volume: String,
    pub mport: Option<i64>,
    pub vport: Option<i64>,
    pub segment: Option<String>,
}
#[qapi(since = "1.7")]
#[qapi(until = "2.6")]
pub struct BlockdevOptionsBase {
    pub driver: BlockdevDriver,
    pub id: Option<String>,
    #[qapi(name = "node-name")]
    pub node_name: Option<String>,
    pub discard: Option<BlockdevDiscardOptions>,
    pub cache: Option<BlockdevCacheOptions>,
    pub aio: Option<BlockdevAioOptions>,
    pub rerror: Option<BlockdevOnError>,
    pub werror: Option<BlockdevOnError>,
    #[qapi(name = "read-only")]
    pub read_only: Option<bool>,
    #[qapi(name = "detect-zeroes")]
    #[qapi(since = "2.1")]
    pub detect_zeroes: Option<BlockdevDetectZeroesOptions>,
    #[qapi(name = "stats-account-invalid")]
    #[qapi(since = "2.5")]
    pub stats_account_invalid: Option<bool>,
    #[qapi(name = "stats-account-failed")]
    #[qapi(since = "2.5")]
    pub stats_account_failed: Option<bool>,
    #[qapi(name = "stats-intervals")]
    #[qapi(since = "2.5")]
    pub stats_intervals: Option<Vec<i64>>,
}
#[qapi(since = "2.0")]
pub struct BlockdevOptionsBlkdebug {
    pub image: Box<BlockdevRef>,
    pub config: Option<String>,
    pub align: Option<i64>,
    #[qapi(name = "inject-error")]
    pub inject_error: Option<Vec<BlkdebugInjectErrorOptions>>,
    #[qapi(name = "set-state")]
    pub set_state: Option<Vec<BlkdebugSetStateOptions>>,
    #[qapi(name = "max-transfer")]
    #[qapi(since = "2.10")]
    pub max_transfer: Option<i32>,
    #[qapi(name = "opt-write-zero")]
    #[qapi(since = "2.10")]
    pub opt_write_zero: Option<i32>,
    #[qapi(name = "max-write-zero")]
    #[qapi(since = "2.10")]
    pub max_write_zero: Option<i32>,
    #[qapi(name = "opt-discard")]
    #[qapi(since = "2.10")]
    pub opt_discard: Option<i32>,
    #[qapi(name = "max-discard")]
    #[qapi(since = "2.10")]
    pub max_discard: Option<i32>,
    #[qapi(name = "take-child-perms")]
    #[qapi(since = "5.0")]
    pub take_child_perms: Option<Vec<BlockPermission>>,
    #[qapi(name = "unshare-child-perms")]
    #[qapi(since = "5.0")]
    pub unshare_child_perms: Option<Vec<BlockPermission>>,
}
#[qapi(since = "3.0")]
pub struct BlockdevOptionsBlklogwrites {
    pub file: Box<BlockdevRef>,
    pub log: Box<BlockdevRef>,
    #[qapi(name = "log-sector-size")]
    pub log_sector_size: Option<u32>,
    #[qapi(name = "log-append")]
    pub log_append: Option<bool>,
    #[qapi(name = "log-super-update-interval")]
    pub log_super_update_interval: Option<u64>,
}
#[qapi(since = "4.2")]
pub struct BlockdevOptionsBlkreplay {
    pub image: Box<BlockdevRef>,
}
#[qapi(since = "2.0")]
pub struct BlockdevOptionsBlkverify {
    pub test: Box<BlockdevRef>,
    pub raw: Box<BlockdevRef>,
}
#[qapi(since = "6.2")]
pub struct BlockdevOptionsCbw {
    pub file: Box<BlockdevRef>,
    pub target: Box<BlockdevRef>,
    #[qapi(since = "7.0")]
    pub bitmap: Option<BlockDirtyBitmap>,
    #[qapi(name = "on-cbw-error")]
    #[qapi(since = "7.1")]
    pub on_cbw_error: Option<OnCbwError>,
    #[qapi(name = "cbw-timeout")]
    #[qapi(since = "7.1")]
    pub cbw_timeout: Option<u32>,
    #[qapi(name = "min-cluster-size")]
    #[qapi(since = "9.2")]
    pub min_cluster_size: Option<u64>,
}
#[qapi(since = "6.0")]
pub struct BlockdevOptionsCor {
    pub file: BlockdevRef,
    pub bottom: Option<String>,
}
#[qapi(since = "1.7")]
#[qapi(until = "2.9")]
pub struct BlockdevOptionsCurl {
    pub filename: String,
}
#[qapi(since = "2.9")]
pub struct BlockdevOptionsCurlBase {
    pub url: String,
    pub readahead: Option<i64>,
    pub timeout: Option<i64>,
    pub username: Option<String>,
    #[qapi(name = "password-secret")]
    pub password_secret: Option<String>,
    #[qapi(name = "proxy-username")]
    pub proxy_username: Option<String>,
    #[qapi(name = "proxy-password-secret")]
    pub proxy_password_secret: Option<String>,
}
#[qapi(since = "2.9")]
pub struct BlockdevOptionsCurlFtp {
    pub url: String,
    pub readahead: Option<i64>,
    pub timeout: Option<i64>,
    pub username: Option<String>,
    #[qapi(name = "password-secret")]
    pub password_secret: Option<String>,
    #[qapi(name = "proxy-username")]
    pub proxy_username: Option<String>,
    #[qapi(name = "proxy-password-secret")]
    pub proxy_password_secret: Option<String>,
}
#[qapi(since = "2.9")]
pub struct BlockdevOptionsCurlFtps {
    pub url: String,
    pub readahead: Option<i64>,
    pub timeout: Option<i64>,
    pub username: Option<String>,
    #[qapi(name = "password-secret")]
    pub password_secret: Option<String>,
    #[qapi(name = "proxy-username")]
    pub proxy_username: Option<String>,
    #[qapi(name = "proxy-password-secret")]
    pub proxy_password_secret: Option<String>,
    pub sslverify: Option<bool>,
}
#[qapi(since = "2.9")]
pub struct BlockdevOptionsCurlHttp {
    pub url: String,
    pub readahead: Option<i64>,
    pub timeout: Option<i64>,
    pub username: Option<String>,
    #[qapi(name = "password-secret")]
    pub password_secret: Option<String>,
    #[qapi(name = "proxy-username")]
    pub proxy_username: Option<String>,
    #[qapi(name = "proxy-password-secret")]
    pub proxy_password_secret: Option<String>,
    pub cookie: Option<String>,
    #[qapi(name = "cookie-secret")]
    #[qapi(since = "2.10")]
    pub cookie_secret: Option<String>,
}
#[qapi(since = "2.9")]
pub struct BlockdevOptionsCurlHttps {
    pub url: String,
    pub readahead: Option<i64>,
    pub timeout: Option<i64>,
    pub username: Option<String>,
    #[qapi(name = "password-secret")]
    pub password_secret: Option<String>,
    #[qapi(name = "proxy-username")]
    pub proxy_username: Option<String>,
    #[qapi(name = "proxy-password-secret")]
    pub proxy_password_secret: Option<String>,
    pub cookie: Option<String>,
    pub sslverify: Option<bool>,
    #[qapi(name = "cookie-secret")]
    #[qapi(since = "2.10")]
    pub cookie_secret: Option<String>,
}
#[qapi(since = "1.7")]
pub struct BlockdevOptionsFile {
    pub filename: String,
    #[qapi(since = "2.8")]
    pub aio: Option<BlockdevAioOptions>,
    #[qapi(since = "2.10")]
    pub locking: Option<OnOffAuto>,
    #[qapi(name = "pr-manager")]
    #[qapi(since = "2.11")]
    pub pr_manager: Option<String>,
    #[qapi(name = "x-check-cache-dropped")]
    #[qapi(since = "3.0")]
    pub x_check_cache_dropped: Option<bool>,
    #[qapi(name = "drop-cache")]
    #[qapi(condition = "defined(CONFIG_LINUX)")]
    #[qapi(since = "4.0")]
    pub drop_cache: Option<bool>,
    #[qapi(name = "aio-max-batch")]
    #[qapi(since = "6.2")]
    pub aio_max_batch: Option<i64>,
}
#[qapi(name = "BlockdevOptionsGenericCOWFormat")]
#[qapi(since = "1.7")]
pub struct BlockdevOptionsGenericCowFormat {
    pub file: Box<BlockdevRef>,
    pub backing: Option<Box<BlockdevRef>>,
}
#[qapi(since = "1.7")]
pub struct BlockdevOptionsGenericFormat {
    pub file: Box<BlockdevRef>,
}
#[qapi(since = "2.7")]
pub struct BlockdevOptionsGluster {
    pub volume: String,
    pub path: String,
    pub server: Vec<GlusterServer>,
    #[qapi(name = "debug-level")]
    #[qapi(until = "2.8")]
    pub debug_level: Option<i64>,
    #[qapi(since = "2.8")]
    pub debug: Option<i64>,
    #[qapi(since = "2.8")]
    pub logfile: Option<String>,
}
#[qapi(condition = "CONFIG_BLKIO")]
#[qapi(since = "7.2")]
pub struct BlockdevOptionsIoUring {
    pub filename: String,
}
#[qapi(since = "2.9")]
pub struct BlockdevOptionsIscsi {
    pub transport: IscsiTransport,
    pub portal: String,
    pub target: String,
    pub lun: Option<i64>,
    pub user: Option<String>,
    #[qapi(name = "password-secret")]
    pub password_secret: Option<String>,
    #[qapi(name = "initiator-name")]
    pub initiator_name: Option<String>,
    #[qapi(name = "header-digest")]
    pub header_digest: Option<IscsiHeaderDigest>,
    pub timeout: Option<i64>,
}
#[qapi(name = "BlockdevOptionsLUKS")]
#[qapi(since = "2.6")]
pub struct BlockdevOptionsLuks {
    pub file: Box<BlockdevRef>,
    #[qapi(name = "key-secret")]
    pub key_secret: Option<String>,
    #[qapi(since = "9.0")]
    pub header: Option<Box<BlockdevRef>>,
}
#[qapi(name = "BlockdevOptionsNVMe")]
#[qapi(since = "2.12")]
pub struct BlockdevOptionsNvMe {
    pub device: String,
    pub namespace: i64,
}
#[qapi(since = "2.8")]
pub struct BlockdevOptionsNbd {
    pub server: SocketAddress,
    pub export: Option<String>,
    #[qapi(name = "tls-creds")]
    pub tls_creds: Option<String>,
    #[qapi(name = "x-dirty-bitmap")]
    #[qapi(since = "3.0")]
    pub x_dirty_bitmap: Option<String>,
    #[qapi(name = "reconnect-delay")]
    #[qapi(since = "4.2")]
    pub reconnect_delay: Option<u32>,
    #[qapi(name = "tls-hostname")]
    #[qapi(since = "7.0")]
    pub tls_hostname: Option<String>,
    #[qapi(name = "open-timeout")]
    #[qapi(since = "7.0")]
    pub open_timeout: Option<u32>,
}
#[qapi(since = "2.8")]
pub struct BlockdevOptionsNfs {
    pub server: NfsServer,
    pub path: String,
    pub user: Option<i64>,
    pub group: Option<i64>,
    #[qapi(name = "tcp-syn-count")]
    pub tcp_syn_count: Option<i64>,
    #[qapi(name = "readahead-size")]
    pub readahead_size: Option<i64>,
    #[qapi(name = "page-cache-size")]
    pub page_cache_size: Option<i64>,
    pub debug: Option<i64>,
}
#[qapi(since = "2.2")]
pub struct BlockdevOptionsNull {
    pub size: Option<i64>,
    #[qapi(name = "latency-ns")]
    #[qapi(since = "2.4")]
    pub latency_ns: Option<u64>,
    #[qapi(name = "read-zeroes")]
    #[qapi(since = "4.1")]
    pub read_zeroes: Option<bool>,
}
#[qapi(condition = "CONFIG_BLKIO")]
#[qapi(since = "7.2")]
pub struct BlockdevOptionsNvmeIoUring {
    pub path: String,
}
#[qapi(since = "6.0")]
pub struct BlockdevOptionsPreallocate {
    pub file: Box<BlockdevRef>,
    #[qapi(name = "prealloc-align")]
    pub prealloc_align: Option<i64>,
    #[qapi(name = "prealloc-size")]
    pub prealloc_size: Option<i64>,
}
#[qapi(since = "2.10")]
pub struct BlockdevOptionsQcow {
    pub file: BlockdevRef,
    pub backing: Option<BlockdevRefOrNull>,
    pub encrypt: Option<BlockdevQcowEncryption>,
}
#[qapi(since = "1.7")]
pub struct BlockdevOptionsQcow2 {
    pub file: Box<BlockdevRef>,
    pub backing: Option<Box<BlockdevRef>>,
    #[qapi(name = "lazy-refcounts")]
    pub lazy_refcounts: Option<bool>,
    #[qapi(name = "pass-discard-request")]
    pub pass_discard_request: Option<bool>,
    #[qapi(name = "pass-discard-snapshot")]
    pub pass_discard_snapshot: Option<bool>,
    #[qapi(name = "pass-discard-other")]
    pub pass_discard_other: Option<bool>,
    #[qapi(name = "overlap-check")]
    #[qapi(since = "2.2")]
    pub overlap_check: Option<Qcow2OverlapChecks>,
    #[qapi(name = "cache-size")]
    #[qapi(since = "2.2")]
    pub cache_size: Option<i64>,
    #[qapi(name = "l2-cache-size")]
    #[qapi(since = "2.2")]
    pub l2_cache_size: Option<i64>,
    #[qapi(name = "refcount-cache-size")]
    #[qapi(since = "2.2")]
    pub refcount_cache_size: Option<i64>,
    #[qapi(name = "cache-clean-interval")]
    #[qapi(since = "2.5")]
    pub cache_clean_interval: Option<i64>,
    #[qapi(since = "2.10")]
    pub encrypt: Option<BlockdevQcow2Encryption>,
    #[qapi(name = "l2-cache-entry-size")]
    #[qapi(since = "2.12")]
    pub l2_cache_entry_size: Option<i64>,
    #[qapi(name = "data-file")]
    #[qapi(since = "4.0")]
    pub data_file: Option<Box<BlockdevRef>>,
    #[qapi(name = "discard-no-unref")]
    #[qapi(since = "8.1")]
    pub discard_no_unref: Option<bool>,
}
#[qapi(since = "2.0")]
pub struct BlockdevOptionsQuorum {
    pub blkverify: Option<bool>,
    pub children: Box<Vec<BlockdevRef>>,
    #[qapi(name = "vote-threshold")]
    pub vote_threshold: i64,
    #[qapi(name = "rewrite-corrupted")]
    #[qapi(since = "2.1")]
    pub rewrite_corrupted: Option<bool>,
    #[qapi(name = "read-pattern")]
    #[qapi(since = "2.2")]
    pub read_pattern: Option<QuorumReadPattern>,
}
#[qapi(since = "2.8")]
pub struct BlockdevOptionsRaw {
    pub file: BlockdevRef,
    pub offset: Option<i64>,
    pub size: Option<i64>,
}
#[qapi(since = "2.9")]
pub struct BlockdevOptionsRbd {
    pub pool: String,
    pub image: String,
    pub conf: Option<String>,
    pub snapshot: Option<String>,
    pub user: Option<String>,
    pub server: Option<Vec<InetSocketAddressBase>>,
    #[qapi(name = "auth-client-required")]
    #[qapi(since = "3.0")]
    pub auth_client_required: Option<Vec<RbdAuthMode>>,
    #[qapi(name = "key-secret")]
    #[qapi(since = "3.0")]
    pub key_secret: Option<String>,
    #[qapi(since = "5.0")]
    pub namespace: Option<String>,
    #[qapi(since = "6.1")]
    pub encrypt: Option<RbdEncryptionOptions>,
}
#[qapi(since = "2.8")]
pub struct BlockdevOptionsReplication {
    pub file: Box<BlockdevRef>,
    pub mode: ReplicationMode,
    #[qapi(name = "top-id")]
    pub top_id: Option<String>,
}
#[qapi(since = "2.9")]
#[qapi(until = "6.1")]
pub struct BlockdevOptionsSheepdog {
    pub server: SocketAddressFlat,
    pub vdi: String,
    #[qapi(name = "snap-id")]
    pub snap_id: Option<u32>,
    pub tag: Option<String>,
}
#[qapi(since = "2.8")]
pub struct BlockdevOptionsSsh {
    pub server: InetSocketAddress,
    pub path: String,
    pub user: Option<String>,
    #[qapi(name = "host-key-check")]
    #[qapi(since = "2.12")]
    pub host_key_check: Option<SshHostKeyCheck>,
}
#[qapi(since = "2.11")]
pub struct BlockdevOptionsThrottle {
    #[qapi(name = "throttle-group")]
    pub throttle_group: String,
    pub file: Box<BlockdevRef>,
}
#[qapi(name = "BlockdevOptionsVVFAT")]
#[qapi(since = "1.7")]
pub struct BlockdevOptionsVvfat {
    pub dir: String,
    #[qapi(name = "fat-type")]
    pub fat_type: Option<i64>,
    pub floppy: Option<bool>,
    pub rw: Option<bool>,
    #[qapi(since = "2.4")]
    pub label: Option<String>,
}
#[qapi(condition = "CONFIG_BLKIO")]
#[qapi(since = "7.2")]
pub struct BlockdevOptionsVirtioBlkVfioPci {
    pub path: String,
}
#[qapi(condition = "CONFIG_BLKIO")]
#[qapi(since = "7.2")]
pub struct BlockdevOptionsVirtioBlkVhostUser {
    pub path: String,
}
#[qapi(condition = "CONFIG_BLKIO")]
#[qapi(since = "7.2")]
pub struct BlockdevOptionsVirtioBlkVhostVdpa {
    pub path: String,
}
#[qapi(name = "BlockdevOptionsVxHS")]
#[qapi(since = "2.10")]
#[qapi(until = "5.1")]
pub struct BlockdevOptionsVxHs {
    #[qapi(name = "vdisk-id")]
    pub vdisk_id: String,
    pub server: InetSocketAddressBase,
    #[qapi(name = "tls-creds")]
    pub tls_creds: Option<String>,
}
#[qapi(since = "2.10")]
pub struct BlockdevQcow2Encryption {
    #[qapi(discriminator)]
    pub format: BlockdevQcow2EncryptionFormat,
    #[qapi(union)]
    pub u: Option<BlockdevQcow2EncryptionBranch>,
}
#[qapi(since = "2.10")]
pub struct BlockdevQcowEncryption {
    #[qapi(discriminator)]
    pub format: BlockdevQcowEncryptionFormat,
    #[qapi(union)]
    pub u: Option<BlockdevQcowEncryptionBranch>,
}
#[qapi(since = "1.1")]
pub struct BlockdevSnapshot {
    #[qapi(until = "2.5")]
    pub device: Option<String>,
    #[qapi(name = "snapshot-file")]
    #[qapi(until = "2.5")]
    pub snapshot_file: Option<String>,
    #[qapi(until = "2.5")]
    pub format: Option<String>,
    #[qapi(until = "2.5")]
    pub mode: Option<NewImageMode>,
    #[qapi(name = "node-name")]
    #[qapi(since = "2.1")]
    #[qapi(until = "2.5")]
    pub node_name: Option<String>,
    #[qapi(name = "snapshot-node-name")]
    #[qapi(since = "2.1")]
    #[qapi(until = "2.5")]
    pub snapshot_node_name: Option<String>,
    #[qapi(since = "2.5")]
    pub node: Option<String>,
    #[qapi(since = "2.5")]
    pub overlay: Option<String>,
}
#[qapi(since = "1.7")]
pub struct BlockdevSnapshotInternal {
    pub device: String,
    pub name: String,
}
#[qapi(since = "1.7")]
pub struct BlockdevSnapshotInternalWrapper {
    pub data: BlockdevSnapshotInternal,
}
#[qapi(since = "2.5")]
pub struct BlockdevSnapshotSync {
    pub device: Option<String>,
    #[qapi(name = "node-name")]
    pub node_name: Option<String>,
    #[qapi(name = "snapshot-file")]
    pub snapshot_file: String,
    #[qapi(name = "snapshot-node-name")]
    pub snapshot_node_name: Option<String>,
    pub format: Option<String>,
    pub mode: Option<NewImageMode>,
}
#[qapi(since = "1.1")]
pub struct BlockdevSnapshotSyncWrapper {
    pub data: BlockdevSnapshotSync,
}
#[qapi(since = "2.5")]
pub struct BlockdevSnapshotWrapper {
    pub data: BlockdevSnapshot,
}
#[qapi(since = "7.1")]
pub struct BootConfiguration {
    pub order: Option<String>,
    pub once: Option<String>,
    pub menu: Option<bool>,
    pub splash: Option<String>,
    #[qapi(name = "splash-time")]
    pub splash_time: Option<i64>,
    #[qapi(name = "reboot-timeout")]
    pub reboot_timeout: Option<i64>,
    pub strict: Option<bool>,
}
#[qapi(name = "COLOStatus")]
#[qapi(since = "3.1")]
pub struct ColoStatus {
    pub mode: ColoMode,
    pub reason: ColoExitReason,
    #[qapi(name = "last-mode")]
    #[qapi(since = "4.0")]
    pub last_mode: Option<ColoMode>,
}
#[qapi(event)]
#[qapi(name = "COLO_EXIT")]
#[qapi(since = "3.1")]
pub struct ColoExitEvent {
    pub mode: ColoMode,
    pub reason: ColoExitReason,
}
#[qapi(event)]
#[qapi(name = "COMMAND_DROPPED")]
#[qapi(since = "2.12")]
#[qapi(until = "4.0")]
pub struct CommandDroppedEvent {
    pub id: serde_json::Value,
    pub reason: CommandDropReason,
}
#[qapi(event)]
#[qapi(name = "CPU_POLARIZATION_CHANGE")]
#[qapi(condition = "(TARGET_S390X && CONFIG_KVM)")]
#[qapi(feature = "unstable")]
#[qapi(since = "8.2")]
pub struct CpuPolarizationChangeEvent {
    pub polarization: CpuS390Polarization,
}
#[qapi(name = "CXLFMWProperties")]
#[qapi(since = "7.1")]
pub struct CxlfmwProperties {
    #[qapi(name = "cxl-fmw")]
    pub cxl_fmw: Vec<CxlFixedMemoryWindowOptions>,
}
#[qapi(name = "CXLFixedMemoryWindowOptions")]
#[qapi(since = "7.1")]
pub struct CxlFixedMemoryWindowOptions {
    pub size: u64,
    #[qapi(name = "interleave-granularity")]
    pub interleave_granularity: Option<u64>,
    pub targets: Vec<String>,
}
#[qapi(name = "CXLUncorErrorRecord")]
#[qapi(since = "8.0")]
pub struct CxlUncorErrorRecord {
    pub r#type: CxlUncorErrorType,
    pub header: Vec<u32>,
}
#[qapi(since = "2.12")]
pub struct CanHostSocketcanProperties {
    pub r#if: String,
    pub canbus: String,
}
#[qapi(since = "1.4")]
pub struct ChardevBackend {
    #[qapi(discriminator)]
    pub r#type: ChardevBackendKind,
    #[qapi(union)]
    pub u: Option<ChardevBackendBranch>,
}
#[qapi(since = "2.0")]
pub struct ChardevBackendInfo {
    pub name: String,
}
#[qapi(since = "2.6")]
pub struct ChardevCommon {
    pub logfile: Option<String>,
    pub logappend: Option<bool>,
}
#[qapi(since = "2.6")]
pub struct ChardevCommonWrapper {
    pub data: ChardevCommon,
}
#[qapi(condition = "CONFIG_DBUS_DISPLAY")]
#[qapi(since = "7.0")]
pub struct ChardevDBus {
    pub logfile: Option<String>,
    pub logappend: Option<bool>,
    pub name: String,
}
#[qapi(condition = "CONFIG_DBUS_DISPLAY")]
#[qapi(since = "7.0")]
pub struct ChardevDBusWrapper {
    pub data: ChardevDBus,
}
#[qapi(since = "1.4")]
#[qapi(until = "2.5")]
pub struct ChardevDummy {}
#[qapi(since = "1.4")]
pub struct ChardevFile {
    pub r#in: Option<String>,
    pub out: String,
    #[qapi(since = "2.8")]
    pub logfile: Option<String>,
    #[qapi(since = "2.8")]
    pub logappend: Option<bool>,
    #[qapi(since = "2.8")]
    pub append: Option<bool>,
}
#[qapi(since = "1.4")]
pub struct ChardevFileWrapper {
    pub data: ChardevFile,
}
#[qapi(since = "1.4")]
pub struct ChardevHostdev {
    pub device: String,
    #[qapi(since = "2.8")]
    pub logfile: Option<String>,
    #[qapi(since = "2.8")]
    pub logappend: Option<bool>,
}
#[qapi(since = "1.4")]
pub struct ChardevHostdevWrapper {
    pub data: ChardevHostdev,
}
#[qapi(since = "10.0")]
pub struct ChardevHub {
    pub logfile: Option<String>,
    pub logappend: Option<bool>,
    pub chardevs: Vec<String>,
}
#[qapi(since = "10.0")]
pub struct ChardevHubWrapper {
    pub data: ChardevHub,
}
#[qapi(since = "0.14.0")]
pub struct ChardevInfo {
    pub label: String,
    pub filename: String,
    #[qapi(name = "frontend-open")]
    #[qapi(since = "2.8")]
    pub frontend_open: Option<bool>,
}
#[qapi(since = "1.5")]
pub struct ChardevMux {
    pub chardev: String,
    #[qapi(since = "2.8")]
    pub logfile: Option<String>,
    #[qapi(since = "2.8")]
    pub logappend: Option<bool>,
}
#[qapi(since = "1.5")]
pub struct ChardevMuxWrapper {
    pub data: ChardevMux,
}
#[qapi(since = "9.2")]
pub struct ChardevPty {
    pub logfile: Option<String>,
    pub logappend: Option<bool>,
    pub path: Option<String>,
}
#[qapi(since = "9.2")]
pub struct ChardevPtyWrapper {
    pub data: ChardevPty,
}
#[qapi(name = "ChardevQemuVDAgent")]
#[qapi(condition = "defined(CONFIG_SPICE_PROTOCOL)")]
#[qapi(since = "6.1")]
pub struct ChardevQemuVdAgent {
    pub logfile: Option<String>,
    pub logappend: Option<bool>,
    pub mouse: Option<bool>,
    pub clipboard: Option<bool>,
}
#[qapi(name = "ChardevQemuVDAgentWrapper")]
#[qapi(condition = "CONFIG_SPICE_PROTOCOL")]
#[qapi(since = "6.1")]
pub struct ChardevQemuVdAgentWrapper {
    pub data: ChardevQemuVdAgent,
}
#[qapi(since = "1.4")]
pub struct ChardevReturn {
    pub pty: Option<String>,
}
#[qapi(since = "1.5")]
pub struct ChardevRingbuf {
    pub size: Option<i64>,
    #[qapi(since = "2.8")]
    pub logfile: Option<String>,
    #[qapi(since = "2.8")]
    pub logappend: Option<bool>,
}
#[qapi(since = "1.5")]
pub struct ChardevRingbufWrapper {
    pub data: ChardevRingbuf,
}
#[qapi(since = "1.4")]
pub struct ChardevSocket {
    pub addr: SocketAddress,
    pub server: Option<bool>,
    pub wait: Option<bool>,
    pub nodelay: Option<bool>,
    pub telnet: Option<bool>,
    #[qapi(since = "2.2")]
    #[qapi(until = "10.2")]
    pub reconnect: Option<i64>,
    #[qapi(since = "2.8")]
    pub logfile: Option<String>,
    #[qapi(since = "2.8")]
    pub logappend: Option<bool>,
    #[qapi(name = "tls-creds")]
    #[qapi(since = "2.8")]
    pub tls_creds: Option<String>,
    #[qapi(since = "2.10")]
    pub tn3270: Option<bool>,
    #[qapi(since = "3.1")]
    pub websocket: Option<bool>,
    #[qapi(name = "tls-authz")]
    #[qapi(since = "4.0")]
    pub tls_authz: Option<String>,
    #[qapi(name = "reconnect-ms")]
    #[qapi(since = "9.2")]
    pub reconnect_ms: Option<i64>,
}
#[qapi(since = "1.4")]
pub struct ChardevSocketWrapper {
    pub data: ChardevSocket,
}
#[qapi(since = "1.5")]
pub struct ChardevSpiceChannel {
    pub r#type: String,
    #[qapi(since = "2.8")]
    pub logfile: Option<String>,
    #[qapi(since = "2.8")]
    pub logappend: Option<bool>,
}
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(since = "1.5")]
pub struct ChardevSpiceChannelWrapper {
    pub data: ChardevSpiceChannel,
}
#[qapi(since = "1.5")]
pub struct ChardevSpicePort {
    pub fqdn: String,
    #[qapi(since = "2.8")]
    pub logfile: Option<String>,
    #[qapi(since = "2.8")]
    pub logappend: Option<bool>,
}
#[qapi(condition = "CONFIG_SPICE")]
#[qapi(since = "1.5")]
pub struct ChardevSpicePortWrapper {
    pub data: ChardevSpicePort,
}
#[qapi(since = "1.5")]
pub struct ChardevStdio {
    pub signal: Option<bool>,
    #[qapi(since = "2.8")]
    pub logfile: Option<String>,
    #[qapi(since = "2.8")]
    pub logappend: Option<bool>,
}
#[qapi(since = "1.5")]
pub struct ChardevStdioWrapper {
    pub data: ChardevStdio,
}
#[qapi(since = "1.5")]
pub struct ChardevUdp {
    pub remote: SocketAddress,
    pub local: Option<SocketAddress>,
    #[qapi(since = "2.8")]
    pub logfile: Option<String>,
    #[qapi(since = "2.8")]
    pub logappend: Option<bool>,
}
#[qapi(since = "1.5")]
pub struct ChardevUdpWrapper {
    pub data: ChardevUdp,
}
#[qapi(name = "ChardevVC")]
#[qapi(since = "1.5")]
pub struct ChardevVc {
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub cols: Option<i64>,
    pub rows: Option<i64>,
    #[qapi(since = "2.8")]
    pub logfile: Option<String>,
    #[qapi(since = "2.8")]
    pub logappend: Option<bool>,
}
#[qapi(name = "ChardevVCWrapper")]
#[qapi(since = "1.5")]
pub struct ChardevVcWrapper {
    pub data: ChardevVc,
}
#[qapi(since = "2.8")]
pub struct ColoCompareProperties {
    pub primary_in: String,
    pub secondary_in: String,
    pub outdev: String,
    pub iothread: String,
    pub notify_dev: Option<String>,
    pub compare_timeout: Option<u64>,
    pub expired_scan_cycle: Option<u32>,
    pub max_queue_size: Option<u32>,
    pub vnet_hdr_support: Option<bool>,
}
#[qapi(since = "0.14.0")]
pub struct CommandInfo {
    pub name: String,
}
#[qapi(since = "1.5")]
pub struct CommandLineOptionInfo {
    pub option: String,
    pub parameters: Vec<CommandLineParameterInfo>,
}
#[qapi(since = "1.5")]
pub struct CommandLineParameterInfo {
    pub name: String,
    pub r#type: CommandLineParameterType,
    pub help: Option<String>,
    #[qapi(since = "2.1")]
    pub default: Option<String>,
}
#[qapi(since = "6.0")]
pub struct CompatPolicy {
    #[qapi(name = "deprecated-input")]
    pub deprecated_input: Option<CompatPolicyInput>,
    #[qapi(name = "deprecated-output")]
    pub deprecated_output: Option<CompatPolicyOutput>,
    #[qapi(name = "unstable-input")]
    #[qapi(since = "6.2")]
    pub unstable_input: Option<CompatPolicyInput>,
    #[qapi(name = "unstable-output")]
    #[qapi(since = "6.2")]
    pub unstable_output: Option<CompatPolicyOutput>,
}
#[qapi(since = "9.1")]
pub struct CompatProperty {
    #[qapi(name = "qom-type")]
    pub qom_type: String,
    pub property: String,
    pub value: String,
}
#[qapi(since = "3.1")]
pub struct CompressionStats {
    pub pages: i64,
    pub busy: i64,
    #[qapi(name = "busy-rate")]
    pub busy_rate: f64,
    #[qapi(name = "compressed-size")]
    pub compressed_size: i64,
    #[qapi(name = "compression-rate")]
    pub compression_rate: f64,
}
#[qapi(since = "1.2.0")]
pub struct CpuDefinitionInfo {
    pub name: String,
    #[qapi(name = "migration-safe")]
    #[qapi(since = "2.8")]
    pub migration_safe: Option<bool>,
    #[qapi(since = "2.8")]
    pub r#static: Option<bool>,
    #[qapi(name = "unavailable-features")]
    #[qapi(since = "2.8")]
    pub unavailable_features: Option<Vec<String>>,
    #[qapi(since = "2.9")]
    pub typename: Option<String>,
    #[qapi(name = "alias-of")]
    #[qapi(since = "4.1")]
    pub alias_of: Option<String>,
    #[qapi(since = "5.2")]
    pub deprecated: Option<bool>,
}
#[qapi(since = "0.14.0")]
#[qapi(until = "6.0")]
pub struct CpuInfo {
    #[qapi(name = "CPU")]
    pub cpu: i64,
    pub current: bool,
    pub halted: bool,
    #[qapi(name = "PC")]
    #[qapi(until = "2.6")]
    pub pc: Option<i64>,
    #[qapi(until = "2.6")]
    pub nip: Option<i64>,
    #[qapi(until = "2.6")]
    pub npc: Option<i64>,
    #[qapi(name = "PC")]
    #[qapi(until = "1.2")]
    pub pc_alt: Option<i64>,
    pub thread_id: i64,
    #[qapi(since = "2.5")]
    pub qom_path: Option<String>,
    #[qapi(discriminator)]
    #[qapi(since = "2.6")]
    pub arch: Option<CpuInfoArch>,
    #[qapi(union)]
    #[qapi(since = "2.6")]
    pub u: Option<CpuInfoBranch>,
    #[qapi(since = "2.10")]
    pub props: Option<CpuInstanceProperties>,
}
#[qapi(since = "2.12")]
pub struct CpuInfoFast {
    #[qapi(name = "cpu-index")]
    pub cpu_index: i64,
    #[qapi(name = "qom-path")]
    pub qom_path: String,
    #[qapi(name = "thread-id")]
    pub thread_id: i64,
    pub props: Option<CpuInstanceProperties>,
    #[qapi(discriminator)]
    #[qapi(until = "6.0")]
    pub arch: Option<CpuInfoArch>,
    #[qapi(union)]
    pub u: Option<CpuInfoFastBranch>,
    #[qapi(discriminator)]
    #[qapi(since = "3.0")]
    pub target: Option<SysEmuTarget>,
    #[qapi(name = "qom-type")]
    #[qapi(since = "10.1")]
    pub qom_type: Option<String>,
}
#[qapi(name = "CpuInfoMIPS")]
#[qapi(since = "2.6")]
#[qapi(until = "6.0")]
pub struct CpuInfoMips {
    #[qapi(name = "PC")]
    pub pc: i64,
}
#[qapi(since = "2.6")]
#[qapi(until = "3.0")]
pub struct CpuInfoOther {}
#[qapi(name = "CpuInfoPPC")]
#[qapi(since = "2.6")]
#[qapi(until = "6.0")]
pub struct CpuInfoPpc {
    pub nip: i64,
}
#[qapi(name = "CpuInfoRISCV")]
#[qapi(since = "2.12")]
#[qapi(until = "6.0")]
pub struct CpuInfoRiscv {
    pub pc: i64,
}
#[qapi(since = "2.12")]
pub struct CpuInfoS390 {
    #[qapi(name = "cpu-state")]
    pub cpu_state: CpuS390State,
    #[qapi(since = "8.2")]
    pub dedicated: Option<bool>,
    #[qapi(since = "8.2")]
    pub entitlement: Option<CpuS390Entitlement>,
}
#[qapi(name = "CpuInfoSPARC")]
#[qapi(since = "2.6")]
#[qapi(until = "6.0")]
pub struct CpuInfoSparc {
    pub pc: i64,
    pub npc: i64,
}
#[qapi(since = "2.6")]
#[qapi(until = "6.0")]
pub struct CpuInfoTricore {
    #[qapi(name = "PC")]
    pub pc: i64,
}
#[qapi(since = "2.6")]
#[qapi(until = "6.0")]
pub struct CpuInfoX86 {
    pub pc: i64,
}
#[qapi(since = "2.7")]
pub struct CpuInstanceProperties {
    #[qapi(name = "node-id")]
    pub node_id: Option<i64>,
    #[qapi(name = "socket-id")]
    pub socket_id: Option<i64>,
    #[qapi(name = "core-id")]
    pub core_id: Option<i64>,
    #[qapi(name = "thread-id")]
    pub thread_id: Option<i64>,
    #[qapi(name = "die-id")]
    #[qapi(since = "4.1")]
    pub die_id: Option<i64>,
    #[qapi(name = "cluster-id")]
    #[qapi(since = "7.1")]
    pub cluster_id: Option<i64>,
    #[qapi(name = "drawer-id")]
    #[qapi(since = "8.2")]
    pub drawer_id: Option<i64>,
    #[qapi(name = "book-id")]
    #[qapi(since = "8.2")]
    pub book_id: Option<i64>,
    #[qapi(name = "module-id")]
    #[qapi(since = "9.1")]
    pub module_id: Option<i64>,
}
#[qapi(since = "2.8.0")]
pub struct CpuModelBaselineInfo {
    pub model: CpuModelInfo,
}
#[qapi(since = "2.8.0")]
pub struct CpuModelCompareInfo {
    pub result: CpuModelCompareResult,
    #[qapi(name = "responsible-properties")]
    pub responsible_properties: Vec<String>,
}
#[qapi(since = "2.8.0")]
pub struct CpuModelExpansionInfo {
    pub model: CpuModelInfo,
    #[qapi(name = "deprecated-props")]
    #[qapi(condition = "TARGET_S390X")]
    #[qapi(since = "9.1")]
    pub deprecated_props: Option<Vec<String>>,
}
#[qapi(since = "2.8.0")]
pub struct CpuModelInfo {
    pub name: String,
    pub props: Option<serde_json::Value>,
}
#[qapi(condition = "(TARGET_S390X && CONFIG_KVM)")]
#[qapi(since = "8.2")]
pub struct CpuPolarizationInfo {
    pub polarization: CpuS390Polarization,
}
#[qapi(since = "2.8")]
pub struct CryptodevBackendProperties {
    pub queues: Option<u32>,
    #[qapi(name = "throttle-bps")]
    #[qapi(since = "8.0")]
    pub throttle_bps: Option<u64>,
    #[qapi(name = "throttle-ops")]
    #[qapi(since = "8.0")]
    pub throttle_ops: Option<u64>,
}
#[qapi(since = "2.12")]
pub struct CryptodevVhostUserProperties {
    pub queues: Option<u32>,
    pub chardev: String,
    #[qapi(name = "throttle-bps")]
    #[qapi(since = "8.0")]
    pub throttle_bps: Option<u64>,
    #[qapi(name = "throttle-ops")]
    #[qapi(since = "8.0")]
    pub throttle_ops: Option<u64>,
}
#[qapi(since = "4.0")]
pub struct CurrentMachineParams {
    #[qapi(name = "wakeup-suspend-support")]
    pub wakeup_suspend_support: bool,
}
#[qapi(since = "9.1")]
pub struct CxlDynamicCapacityExtent {
    pub offset: u64,
    pub len: u64,
}
#[qapi(name = "DBusVMStateProperties")]
#[qapi(since = "5.0")]
pub struct DBusVmStateProperties {
    pub addr: String,
    #[qapi(name = "id-list")]
    pub id_list: Option<String>,
}
#[qapi(event)]
#[qapi(name = "DEVICE_DELETED")]
#[qapi(since = "1.5")]
pub struct DeviceDeletedEvent {
    pub device: Option<String>,
    pub path: String,
}
#[qapi(event)]
#[qapi(name = "DEVICE_TRAY_MOVED")]
#[qapi(since = "1.1")]
pub struct DeviceTrayMovedEvent {
    pub device: String,
    #[qapi(name = "tray-open")]
    pub tray_open: bool,
    #[qapi(since = "2.8")]
    pub id: Option<String>,
}
#[qapi(event)]
#[qapi(name = "DEVICE_UNPLUG_GUEST_ERROR")]
#[qapi(since = "6.2")]
pub struct DeviceUnplugGuestErrorEvent {
    pub device: Option<String>,
    pub path: String,
}
#[qapi(event)]
#[qapi(name = "DUMP_COMPLETED")]
#[qapi(since = "2.6")]
pub struct DumpCompletedEvent {
    pub result: DumpQueryResult,
    pub error: Option<String>,
}
#[qapi(since = "1.2")]
#[qapi(until = "2.12")]
pub struct DevicePropertyInfo {
    pub name: String,
    pub r#type: String,
    #[qapi(since = "2.2")]
    pub description: Option<String>,
}
#[qapi(since = "7.1")]
pub struct DirtyLimitInfo {
    #[qapi(name = "cpu-index")]
    pub cpu_index: i64,
    #[qapi(name = "limit-rate")]
    pub limit_rate: u64,
    #[qapi(name = "current-rate")]
    pub current_rate: u64,
}
#[qapi(since = "5.2")]
pub struct DirtyRateInfo {
    #[qapi(name = "dirty-rate")]
    pub dirty_rate: Option<i64>,
    pub status: DirtyRateStatus,
    #[qapi(name = "start-time")]
    pub start_time: i64,
    #[qapi(name = "calc-time")]
    pub calc_time: i64,
    #[qapi(name = "sample-pages")]
    #[qapi(since = "6.1")]
    pub sample_pages: Option<u64>,
    #[qapi(since = "6.2")]
    pub mode: Option<DirtyRateMeasureMode>,
    #[qapi(name = "vcpu-dirty-rate")]
    #[qapi(since = "6.2")]
    pub vcpu_dirty_rate: Option<Vec<DirtyRateVcpu>>,
    #[qapi(name = "calc-time-unit")]
    #[qapi(since = "8.2")]
    pub calc_time_unit: Option<TimeUnit>,
}
#[qapi(since = "6.2")]
pub struct DirtyRateVcpu {
    pub id: i64,
    #[qapi(name = "dirty-rate")]
    pub dirty_rate: i64,
}
#[qapi(since = "7.0")]
pub struct DisplayCocoa {
    #[qapi(name = "left-command-key")]
    pub left_command_key: Option<bool>,
    #[qapi(name = "full-grab")]
    pub full_grab: Option<bool>,
    #[qapi(name = "swap-opt-cmd")]
    pub swap_opt_cmd: Option<bool>,
    #[qapi(name = "zoom-to-fit")]
    #[qapi(since = "8.2")]
    pub zoom_to_fit: Option<bool>,
    #[qapi(name = "zoom-interpolation")]
    #[qapi(since = "9.0")]
    pub zoom_interpolation: Option<bool>,
}
#[qapi(since = "4.0")]
pub struct DisplayCurses {
    pub charset: Option<String>,
}
#[qapi(since = "7.0")]
pub struct DisplayDBus {
    pub rendernode: Option<String>,
    pub addr: Option<String>,
    pub p2p: Option<bool>,
    pub audiodev: Option<String>,
}
#[qapi(name = "DisplayEGLHeadless")]
#[qapi(since = "3.1")]
pub struct DisplayEglHeadless {
    pub rendernode: Option<String>,
}
#[qapi(name = "DisplayGTK")]
#[qapi(since = "2.12")]
pub struct DisplayGtk {
    #[qapi(name = "grab-on-hover")]
    pub grab_on_hover: Option<bool>,
    #[qapi(name = "zoom-to-fit")]
    #[qapi(since = "3.1")]
    pub zoom_to_fit: Option<bool>,
    #[qapi(name = "show-tabs")]
    #[qapi(since = "7.1")]
    pub show_tabs: Option<bool>,
    #[qapi(name = "show-menubar")]
    #[qapi(since = "7.2")]
    pub show_menubar: Option<bool>,
    #[qapi(name = "keep-aspect-ratio")]
    #[qapi(since = "10.1")]
    pub keep_aspect_ratio: Option<bool>,
    #[qapi(since = "10.1")]
    pub scale: Option<f64>,
}
#[qapi(since = "2.12")]
#[qapi(until = "3.0")]
pub struct DisplayNoOpts {}
#[qapi(since = "2.12")]
pub struct DisplayOptions {
    #[qapi(discriminator)]
    pub r#type: DisplayType,
    #[qapi(name = "full-screen")]
    pub full_screen: Option<bool>,
    #[qapi(name = "window-close")]
    pub window_close: Option<bool>,
    pub gl: Option<bool>,
    #[qapi(union)]
    pub u: Option<DisplayOptionsBranch>,
    #[qapi(name = "show-cursor")]
    #[qapi(since = "5.0")]
    pub show_cursor: Option<bool>,
}
#[qapi(since = "6.0")]
pub struct DisplayReloadOptions {
    #[qapi(discriminator)]
    pub r#type: DisplayReloadType,
    #[qapi(union)]
    pub u: Option<DisplayReloadOptionsBranch>,
}
#[qapi(name = "DisplayReloadOptionsVNC")]
#[qapi(since = "6.0")]
pub struct DisplayReloadOptionsVnc {
    #[qapi(name = "tls-certs")]
    pub tls_certs: Option<bool>,
}
#[qapi(name = "DisplaySDL")]
#[qapi(since = "7.1")]
pub struct DisplaySdl {
    #[qapi(name = "grab-mod")]
    pub grab_mod: Option<HotKeyMod>,
}
#[qapi(since = "7.1")]
pub struct DisplayUpdateOptions {
    #[qapi(discriminator)]
    pub r#type: DisplayUpdateType,
    #[qapi(union)]
    pub u: Option<DisplayUpdateOptionsBranch>,
}
#[qapi(name = "DisplayUpdateOptionsVNC")]
#[qapi(since = "7.1")]
pub struct DisplayUpdateOptionsVnc {
    pub addresses: Option<Vec<SocketAddress>>,
}
#[qapi(since = "1.6")]
pub struct DriveBackup {
    pub device: String,
    pub target: String,
    pub format: Option<String>,
    pub sync: MirrorSyncMode,
    pub mode: Option<NewImageMode>,
    pub speed: Option<i64>,
    #[qapi(name = "on-source-error")]
    pub on_source_error: Option<BlockdevOnError>,
    #[qapi(name = "on-target-error")]
    pub on_target_error: Option<BlockdevOnError>,
    #[qapi(since = "2.4")]
    pub bitmap: Option<String>,
    #[qapi(name = "job-id")]
    #[qapi(since = "2.7")]
    pub job_id: Option<String>,
    #[qapi(since = "2.8")]
    pub compress: Option<bool>,
    #[qapi(name = "auto-finalize")]
    #[qapi(since = "2.12")]
    pub auto_finalize: Option<bool>,
    #[qapi(name = "auto-dismiss")]
    #[qapi(since = "2.12")]
    pub auto_dismiss: Option<bool>,
    #[qapi(name = "bitmap-mode")]
    #[qapi(since = "4.2")]
    pub bitmap_mode: Option<BitmapSyncMode>,
    #[qapi(name = "filter-node-name")]
    #[qapi(since = "4.2")]
    pub filter_node_name: Option<String>,
    #[qapi(name = "x-perf")]
    #[qapi(since = "6.0")]
    pub x_perf: Option<BackupPerf>,
    #[qapi(name = "discard-source")]
    #[qapi(since = "9.1")]
    pub discard_source: Option<bool>,
    #[qapi(name = "on-cbw-error")]
    #[qapi(since = "10.1")]
    pub on_cbw_error: Option<OnCbwError>,
}
#[qapi(since = "1.6")]
pub struct DriveBackupWrapper {
    pub data: DriveBackup,
}
#[qapi(since = "1.3")]
pub struct DriveMirror {
    #[qapi(name = "job-id")]
    pub job_id: Option<String>,
    pub device: String,
    pub target: String,
    pub format: Option<String>,
    #[qapi(name = "node-name")]
    pub node_name: Option<String>,
    pub replaces: Option<String>,
    pub sync: MirrorSyncMode,
    pub mode: Option<NewImageMode>,
    pub speed: Option<i64>,
    pub granularity: Option<u32>,
    #[qapi(name = "buf-size")]
    pub buf_size: Option<i64>,
    #[qapi(name = "on-source-error")]
    pub on_source_error: Option<BlockdevOnError>,
    #[qapi(name = "on-target-error")]
    pub on_target_error: Option<BlockdevOnError>,
    pub unmap: Option<bool>,
    #[qapi(name = "copy-mode")]
    #[qapi(since = "3.0")]
    pub copy_mode: Option<MirrorCopyMode>,
    #[qapi(name = "auto-finalize")]
    #[qapi(since = "3.1")]
    pub auto_finalize: Option<bool>,
    #[qapi(name = "auto-dismiss")]
    #[qapi(since = "3.1")]
    pub auto_dismiss: Option<bool>,
}
#[qapi(since = "8.0")]
pub struct DummyBlockCoreForceArrays {
    #[qapi(name = "unused-block-graph-info")]
    pub unused_block_graph_info: Vec<BlockGraphInfo>,
}
#[qapi(since = "2.5")]
pub struct DummyForceArrays {
    pub unused: Vec<X86cpuFeatureWordInfo>,
}
#[qapi(since = "9.0")]
pub struct DummyVirtioForceArrays {
    #[qapi(name = "unused-iothread-vq-mapping")]
    pub unused_iothread_vq_mapping: Vec<IoThreadVirtQueueMapping>,
    #[qapi(name = "unused-virtio-gpu-output")]
    #[qapi(since = "10.1")]
    pub unused_virtio_gpu_output: Option<Vec<VirtIogpuOutput>>,
}
#[qapi(since = "2.0")]
pub struct DumpGuestMemoryCapability {
    pub formats: Vec<DumpGuestMemoryFormat>,
}
#[qapi(since = "2.6")]
pub struct DumpQueryResult {
    pub status: DumpStatus,
    pub completed: i64,
    pub total: i64,
}
#[qapi(condition = "CONFIG_EBPF")]
#[qapi(since = "9.0")]
pub struct EbpfObject {
    pub object: String,
}
#[qapi(since = "1.2.0")]
#[qapi(until = "6.0")]
pub struct EventInfo {
    pub name: String,
}
#[qapi(since = "7.1")]
pub struct EventLoopBaseProperties {
    #[qapi(name = "aio-max-batch")]
    pub aio_max_batch: Option<i64>,
    #[qapi(name = "thread-pool-min")]
    pub thread_pool_min: Option<i64>,
    #[qapi(name = "thread-pool-max")]
    pub thread_pool_max: Option<i64>,
}
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "8.0")]
pub struct EvtchnInfo {
    pub port: u16,
    pub vcpu: u32,
    pub r#type: EvtchnPortType,
    #[qapi(name = "remote-domain")]
    pub remote_domain: String,
    pub target: u16,
    pub pending: bool,
    pub masked: bool,
}
#[qapi(since = "7.0")]
pub struct ExpirePasswordOptions {
    #[qapi(discriminator)]
    pub protocol: DisplayProtocol,
    pub time: String,
    #[qapi(union)]
    pub u: Option<ExpirePasswordOptionsBranch>,
}
#[qapi(since = "7.0")]
pub struct ExpirePasswordOptionsVnc {
    pub display: Option<String>,
}
#[qapi(event)]
#[qapi(name = "FAILOVER_NEGOTIATED")]
#[qapi(since = "4.2")]
pub struct FailoverNegotiatedEvent {
    #[qapi(name = "device-id")]
    pub device_id: String,
}
#[qapi(since = "1.2")]
pub struct FdSocketAddress {
    pub str: String,
}
#[qapi(since = "1.3")]
pub struct FdSocketAddressWrapper {
    pub data: FdSocketAddress,
}
#[qapi(since = "1.2.0")]
pub struct FdsetFdInfo {
    pub fd: i64,
    pub opaque: Option<String>,
}
#[qapi(since = "1.2.0")]
pub struct FdsetInfo {
    #[qapi(name = "fdset-id")]
    pub fdset_id: i64,
    pub fds: Vec<FdsetFdInfo>,
}
#[qapi(since = "8.2")]
pub struct FileMigrationArgs {
    pub filename: String,
    pub offset: u64,
}
#[qapi(since = "2.5")]
pub struct FilterBufferProperties {
    pub netdev: String,
    pub queue: Option<NetFilterDirection>,
    pub status: Option<String>,
    pub position: Option<String>,
    pub insert: Option<NetfilterInsert>,
    pub interval: u32,
}
#[qapi(since = "2.5")]
pub struct FilterDumpProperties {
    pub netdev: String,
    pub queue: Option<NetFilterDirection>,
    pub status: Option<String>,
    pub position: Option<String>,
    pub insert: Option<NetfilterInsert>,
    pub file: String,
    pub maxlen: Option<u32>,
}
#[qapi(since = "2.6")]
pub struct FilterMirrorProperties {
    pub netdev: String,
    pub queue: Option<NetFilterDirection>,
    pub status: Option<String>,
    pub position: Option<String>,
    pub insert: Option<NetfilterInsert>,
    pub outdev: String,
    pub vnet_hdr_support: Option<bool>,
}
#[qapi(since = "2.6")]
pub struct FilterRedirectorProperties {
    pub netdev: String,
    pub queue: Option<NetFilterDirection>,
    pub status: Option<String>,
    pub position: Option<String>,
    pub insert: Option<NetfilterInsert>,
    pub indev: Option<String>,
    pub outdev: Option<String>,
    pub vnet_hdr_support: Option<bool>,
}
#[qapi(since = "2.8")]
pub struct FilterRewriterProperties {
    pub netdev: String,
    pub queue: Option<NetFilterDirection>,
    pub status: Option<String>,
    pub position: Option<String>,
    pub insert: Option<NetfilterInsert>,
    pub vnet_hdr_support: Option<bool>,
}
#[qapi(since = "10.2")]
pub struct FirmwareLog {
    pub version: Option<String>,
    pub log: String,
}
#[qapi(name = "GICCapability")]
#[qapi(since = "2.6")]
pub struct GicCapability {
    pub version: i64,
    pub emulated: bool,
    pub kernel: bool,
}
#[qapi(event)]
#[qapi(name = "GUEST_CRASHLOADED")]
#[qapi(since = "5.0")]
pub struct GuestCrashloadedEvent {
    pub action: GuestPanicAction,
    pub info: Option<GuestPanicInformation>,
}
#[qapi(event)]
#[qapi(name = "GUEST_PANICKED")]
#[qapi(since = "1.5")]
pub struct GuestPanickedEvent {
    pub action: GuestPanicAction,
    #[qapi(since = "2.9")]
    pub info: Option<GuestPanicInformation>,
}
#[qapi(event)]
#[qapi(name = "GUEST_PVSHUTDOWN")]
#[qapi(since = "9.1")]
pub struct GuestPvshutdownEvent {}
#[qapi(since = "2.7")]
#[qapi(until = "2.9")]
pub struct GlusterServer {
    #[qapi(discriminator)]
    pub r#type: GlusterTransport,
    #[qapi(union)]
    pub u: Option<GlusterServerBranch>,
}
#[qapi(since = "2.9")]
pub struct GuestPanicInformation {
    #[qapi(discriminator)]
    pub r#type: GuestPanicInformationType,
    #[qapi(union)]
    pub u: Option<GuestPanicInformationBranch>,
}
#[qapi(since = "2.9")]
pub struct GuestPanicInformationHyperV {
    pub arg1: u64,
    pub arg2: u64,
    pub arg3: u64,
    pub arg4: u64,
    pub arg5: u64,
}
#[qapi(since = "2.12")]
pub struct GuestPanicInformationS390 {
    pub core: u32,
    #[qapi(name = "psw-mask")]
    pub psw_mask: u64,
    #[qapi(name = "psw-addr")]
    pub psw_addr: u64,
    pub reason: S390CrashReason,
}
#[qapi(since = "10.1")]
pub struct GuestPanicInformationTdx {
    #[qapi(name = "error-code")]
    pub error_code: u32,
    pub message: String,
    pub gpa: Option<u64>,
}
#[qapi(since = "2.9")]
pub struct GuidInfo {
    pub guid: String,
}
#[qapi(event)]
#[qapi(name = "HV_BALLOON_STATUS_REPORT")]
#[qapi(since = "8.2")]
pub struct HvBalloonStatusReportEvent {
    pub committed: u64,
    pub available: u64,
}
#[qapi(name = "HotpluggableCPU")]
#[qapi(since = "2.7")]
pub struct HotpluggableCpu {
    pub r#type: String,
    #[qapi(name = "vcpus-count")]
    pub vcpus_count: i64,
    pub props: CpuInstanceProperties,
    #[qapi(name = "qom-path")]
    pub qom_path: Option<String>,
}
#[qapi(since = "6.2")]
pub struct HumanReadableText {
    #[qapi(name = "human-readable-text")]
    pub human_readable_text: String,
}
#[qapi(since = "8.2")]
pub struct HvBalloonDeviceInfo {
    pub id: Option<String>,
    pub memaddr: Option<u64>,
    #[qapi(name = "max-size")]
    pub max_size: u64,
    pub memdev: Option<String>,
}
#[qapi(since = "8.2")]
pub struct HvBalloonDeviceInfoWrapper {
    pub data: HvBalloonDeviceInfo,
}
#[qapi(since = "8.2")]
pub struct HvBalloonInfo {
    pub committed: u64,
    pub available: u64,
}
#[qapi(name = "IOMMUFDProperties")]
#[qapi(since = "9.0")]
pub struct IommufdProperties {
    pub fd: Option<String>,
}
#[qapi(name = "IOThreadInfo")]
#[qapi(since = "2.0")]
pub struct IoThreadInfo {
    pub id: String,
    #[qapi(name = "thread-id")]
    pub thread_id: i64,
    #[qapi(name = "poll-max-ns")]
    #[qapi(since = "2.9")]
    pub poll_max_ns: Option<i64>,
    #[qapi(name = "poll-grow")]
    #[qapi(since = "2.9")]
    pub poll_grow: Option<i64>,
    #[qapi(name = "poll-shrink")]
    #[qapi(since = "2.9")]
    pub poll_shrink: Option<i64>,
    #[qapi(name = "aio-max-batch")]
    #[qapi(since = "6.1")]
    pub aio_max_batch: Option<i64>,
}
#[qapi(name = "IOThreadVirtQueueMapping")]
#[qapi(since = "9.0")]
pub struct IoThreadVirtQueueMapping {
    pub iothread: String,
    pub vqs: Option<Vec<u16>>,
}
#[qapi(condition = "CONFIG_IGVM")]
#[qapi(since = "10.1")]
pub struct IgvmCfgProperties {
    pub file: String,
}
#[qapi(since = "1.4")]
pub struct ImageCheck {
    pub filename: String,
    pub format: String,
    #[qapi(name = "check-errors")]
    pub check_errors: i64,
    #[qapi(name = "image-end-offset")]
    pub image_end_offset: Option<i64>,
    pub corruptions: Option<i64>,
    pub leaks: Option<i64>,
    #[qapi(name = "corruptions-fixed")]
    pub corruptions_fixed: Option<i64>,
    #[qapi(name = "leaks-fixed")]
    pub leaks_fixed: Option<i64>,
    #[qapi(name = "total-clusters")]
    pub total_clusters: Option<i64>,
    #[qapi(name = "allocated-clusters")]
    pub allocated_clusters: Option<i64>,
    #[qapi(name = "fragmented-clusters")]
    pub fragmented_clusters: Option<i64>,
    #[qapi(name = "compressed-clusters")]
    pub compressed_clusters: Option<i64>,
}
#[qapi(since = "1.3")]
pub struct ImageInfo {
    pub filename: String,
    pub format: String,
    #[qapi(name = "dirty-flag")]
    pub dirty_flag: Option<bool>,
    #[qapi(name = "actual-size")]
    pub actual_size: Option<i64>,
    #[qapi(name = "virtual-size")]
    pub virtual_size: i64,
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<i64>,
    pub encrypted: Option<bool>,
    #[qapi(name = "backing-filename")]
    pub backing_filename: Option<String>,
    #[qapi(name = "full-backing-filename")]
    pub full_backing_filename: Option<String>,
    #[qapi(name = "backing-filename-format")]
    pub backing_filename_format: Option<String>,
    pub snapshots: Option<Vec<SnapshotInfo>>,
    #[qapi(name = "backing-image")]
    #[qapi(since = "1.6")]
    pub backing_image: Option<Box<ImageInfo>>,
    #[qapi(since = "1.7")]
    pub compressed: Option<bool>,
    #[qapi(name = "format-specific")]
    #[qapi(since = "1.7")]
    pub format_specific: Option<Box<ImageInfoSpecific>>,
    #[qapi(since = "10.2")]
    pub limits: Option<BlockLimitsInfo>,
}
#[qapi(since = "1.7")]
pub struct ImageInfoSpecific {
    #[qapi(discriminator)]
    pub r#type: ImageInfoSpecificKind,
    #[qapi(union)]
    pub u: Option<Box<ImageInfoSpecificBranch>>,
}
#[qapi(since = "8.0")]
pub struct ImageInfoSpecificFile {
    #[qapi(name = "extent-size-hint")]
    pub extent_size_hint: Option<u64>,
}
#[qapi(since = "8.0")]
pub struct ImageInfoSpecificFileWrapper {
    pub data: ImageInfoSpecificFile,
}
#[qapi(name = "ImageInfoSpecificLUKSWrapper")]
#[qapi(since = "2.7")]
pub struct ImageInfoSpecificLuksWrapper {
    pub data: QCryptoBlockInfoLuks,
}
#[qapi(since = "1.7")]
pub struct ImageInfoSpecificQCow2 {
    pub compat: String,
    #[qapi(name = "lazy-refcounts")]
    pub lazy_refcounts: Option<bool>,
    #[qapi(since = "2.2")]
    pub corrupt: Option<bool>,
    #[qapi(name = "refcount-bits")]
    #[qapi(since = "2.3")]
    pub refcount_bits: Option<i64>,
    #[qapi(since = "2.10")]
    pub encrypt: Option<ImageInfoSpecificQCow2Encryption>,
    #[qapi(name = "data-file")]
    #[qapi(since = "4.0")]
    pub data_file: Option<String>,
    #[qapi(name = "data-file-raw")]
    #[qapi(since = "4.0")]
    pub data_file_raw: Option<bool>,
    #[qapi(since = "4.0")]
    pub bitmaps: Option<Vec<Qcow2BitmapInfo>>,
    #[qapi(name = "compression-type")]
    #[qapi(since = "5.1")]
    pub compression_type: Option<Qcow2CompressionType>,
    #[qapi(name = "extended-l2")]
    #[qapi(since = "5.2")]
    pub extended_l2: Option<bool>,
}
#[qapi(since = "2.10")]
pub struct ImageInfoSpecificQCow2Encryption {
    #[qapi(discriminator)]
    pub format: BlockdevQcow2EncryptionFormat,
    #[qapi(union)]
    pub u: Option<ImageInfoSpecificQCow2EncryptionBranch>,
}
#[qapi(since = "2.10")]
pub struct ImageInfoSpecificQCow2EncryptionBase {
    pub format: BlockdevQcow2EncryptionFormat,
}
#[qapi(since = "1.7")]
pub struct ImageInfoSpecificQCow2Wrapper {
    pub data: ImageInfoSpecificQCow2,
}
#[qapi(since = "6.1")]
pub struct ImageInfoSpecificRbd {
    #[qapi(name = "encryption-format")]
    pub encryption_format: Option<RbdImageEncryptionFormat>,
}
#[qapi(since = "6.1")]
pub struct ImageInfoSpecificRbdWrapper {
    pub data: ImageInfoSpecificRbd,
}
#[qapi(since = "1.7")]
pub struct ImageInfoSpecificVmdk {
    #[qapi(name = "create-type")]
    pub create_type: String,
    pub cid: i64,
    #[qapi(name = "parent-cid")]
    pub parent_cid: i64,
    pub extents: Box<Vec<ImageInfo>>,
}
#[qapi(since = "6.1")]
pub struct ImageInfoSpecificVmdkWrapper {
    pub data: ImageInfoSpecificVmdk,
}
#[qapi(since = "1.3")]
pub struct InetSocketAddress {
    pub host: String,
    pub port: String,
    pub to: Option<u16>,
    pub ipv4: Option<bool>,
    pub ipv6: Option<bool>,
    #[qapi(since = "2.9")]
    pub numeric: Option<bool>,
    #[qapi(name = "keep-alive")]
    #[qapi(since = "4.2")]
    pub keep_alive: Option<bool>,
    #[qapi(condition = "defined(IPPROTO_MPTCP)")]
    #[qapi(since = "6.1")]
    pub mptcp: Option<bool>,
    #[qapi(name = "keep-alive-count")]
    #[qapi(condition = "HAVE_TCP_KEEPCNT")]
    #[qapi(since = "10.1")]
    pub keep_alive_count: Option<u32>,
    #[qapi(name = "keep-alive-idle")]
    #[qapi(condition = "HAVE_TCP_KEEPIDLE")]
    #[qapi(since = "10.1")]
    pub keep_alive_idle: Option<u32>,
    #[qapi(name = "keep-alive-interval")]
    #[qapi(condition = "HAVE_TCP_KEEPINTVL")]
    #[qapi(since = "10.1")]
    pub keep_alive_interval: Option<u32>,
}
#[qapi(since = "2.9")]
pub struct InetSocketAddressBase {
    pub host: String,
    pub port: String,
}
#[qapi(since = "1.3")]
pub struct InetSocketAddressWrapper {
    pub data: InetSocketAddress,
}
#[qapi(since = "4.2")]
pub struct InputBarrierProperties {
    pub name: String,
    pub server: Option<String>,
    pub port: Option<String>,
    #[qapi(name = "x-origin")]
    pub x_origin: Option<String>,
    #[qapi(name = "y-origin")]
    pub y_origin: Option<String>,
    pub width: Option<String>,
    pub height: Option<String>,
}
#[qapi(since = "2.0")]
pub struct InputBtnEvent {
    pub button: InputButton,
    pub down: bool,
}
#[qapi(since = "2.0")]
pub struct InputBtnEventWrapper {
    pub data: InputBtnEvent,
}
#[qapi(since = "2.0")]
pub struct InputEvent {
    #[qapi(discriminator)]
    pub r#type: InputEventKind,
    #[qapi(union)]
    pub u: Option<InputEventBranch>,
}
#[qapi(since = "2.0")]
pub struct InputKeyEvent {
    pub key: KeyValue,
    pub down: bool,
}
#[qapi(since = "2.0")]
pub struct InputKeyEventWrapper {
    pub data: InputKeyEvent,
}
#[qapi(since = "2.6")]
pub struct InputLinuxProperties {
    pub evdev: String,
    pub grab_all: Option<bool>,
    pub repeat: Option<bool>,
    #[qapi(name = "grab-toggle")]
    pub grab_toggle: Option<GrabToggleKeys>,
}
#[qapi(since = "2.0")]
pub struct InputMoveEvent {
    pub axis: InputAxis,
    pub value: i64,
}
#[qapi(since = "2.0")]
pub struct InputMoveEventWrapper {
    pub data: InputMoveEvent,
}
#[qapi(since = "8.1")]
pub struct InputMultiTouchEvent {
    pub r#type: InputMultiTouchType,
    pub slot: i64,
    #[qapi(name = "tracking-id")]
    pub tracking_id: i64,
    pub axis: InputAxis,
    pub value: i64,
}
#[qapi(since = "8.1")]
pub struct InputMultiTouchEventWrapper {
    pub data: InputMultiTouchEvent,
}
#[qapi(since = "1.3")]
pub struct IntWrapper {
    pub data: i64,
}
#[qapi(since = "2.0")]
pub struct IothreadProperties {
    #[qapi(name = "poll-max-ns")]
    pub poll_max_ns: Option<i64>,
    #[qapi(name = "poll-grow")]
    pub poll_grow: Option<i64>,
    #[qapi(name = "poll-shrink")]
    pub poll_shrink: Option<i64>,
    #[qapi(name = "aio-max-batch")]
    #[qapi(since = "6.1")]
    pub aio_max_batch: Option<i64>,
    #[qapi(name = "thread-pool-min")]
    #[qapi(since = "7.1")]
    pub thread_pool_min: Option<i64>,
    #[qapi(name = "thread-pool-max")]
    #[qapi(since = "7.1")]
    pub thread_pool_max: Option<i64>,
}
#[qapi(event)]
#[qapi(name = "JOB_STATUS_CHANGE")]
#[qapi(since = "3.0")]
pub struct JobStatusChangeEvent {
    pub id: String,
    pub status: JobStatus,
}
#[qapi(since = "3.0")]
pub struct JobInfo {
    pub id: String,
    pub r#type: JobType,
    pub status: JobStatus,
    #[qapi(name = "current-progress")]
    pub current_progress: i64,
    #[qapi(name = "total-progress")]
    pub total_progress: i64,
    pub error: Option<String>,
}
#[qapi(since = "1.3")]
pub struct KeyValue {
    #[qapi(discriminator)]
    pub r#type: KeyValueKind,
    #[qapi(union)]
    pub u: Option<KeyValueBranch>,
}
#[qapi(since = "0.14.0")]
pub struct KvmInfo {
    pub enabled: bool,
    pub present: bool,
}
#[qapi(event)]
#[qapi(name = "MEMORY_DEVICE_SIZE_CHANGE")]
#[qapi(since = "5.1")]
pub struct MemoryDeviceSizeChangeEvent {
    pub id: Option<String>,
    pub size: u64,
    #[qapi(name = "qom-path")]
    #[qapi(since = "6.2")]
    pub qom_path: Option<String>,
}
#[qapi(event)]
#[qapi(name = "MEMORY_FAILURE")]
#[qapi(since = "5.2")]
pub struct MemoryFailureEvent {
    pub recipient: MemoryFailureRecipient,
    pub action: MemoryFailureAction,
    pub flags: MemoryFailureFlags,
}
#[qapi(event)]
#[qapi(name = "MEM_UNPLUG_ERROR")]
#[qapi(since = "2.4")]
#[qapi(until = "9.1")]
pub struct MemUnplugErrorEvent {
    pub device: String,
    pub msg: String,
}
#[qapi(event)]
#[qapi(name = "MIGRATION")]
#[qapi(since = "2.4")]
pub struct MigrationEvent {
    pub status: MigrationStatus,
}
#[qapi(event)]
#[qapi(name = "MIGRATION_PASS")]
#[qapi(since = "2.6")]
pub struct MigrationPassEvent {
    pub pass: i64,
}
#[qapi(since = "1.2.0")]
pub struct MachineInfo {
    pub name: String,
    pub alias: Option<String>,
    #[qapi(name = "is-default")]
    pub is_default: Option<bool>,
    #[qapi(name = "cpu-max")]
    pub cpu_max: i64,
    #[qapi(name = "hotpluggable-cpus")]
    #[qapi(since = "2.7")]
    pub hotpluggable_cpus: Option<bool>,
    #[qapi(name = "numa-mem-supported")]
    #[qapi(since = "4.1")]
    pub numa_mem_supported: Option<bool>,
    #[qapi(since = "4.1")]
    pub deprecated: Option<bool>,
    #[qapi(name = "default-cpu-type")]
    #[qapi(since = "4.2")]
    pub default_cpu_type: Option<String>,
    #[qapi(name = "default-ram-id")]
    #[qapi(since = "5.2")]
    pub default_ram_id: Option<String>,
    #[qapi(since = "8.0")]
    pub acpi: Option<bool>,
    #[qapi(name = "compat-props")]
    #[qapi(feature = "unstable")]
    #[qapi(since = "9.1")]
    pub compat_props: Option<Vec<CompatProperty>>,
}
#[qapi(since = "7.1")]
pub struct MainLoopProperties {
    #[qapi(name = "aio-max-batch")]
    pub aio_max_batch: Option<i64>,
    #[qapi(name = "thread-pool-min")]
    pub thread_pool_min: Option<i64>,
    #[qapi(name = "thread-pool-max")]
    pub thread_pool_max: Option<i64>,
}
#[qapi(since = "2.6")]
pub struct MapEntry {
    pub start: i64,
    pub length: i64,
    pub data: bool,
    pub zero: bool,
    pub depth: i64,
    pub offset: Option<i64>,
    pub filename: Option<String>,
    #[qapi(since = "6.1")]
    pub present: Option<bool>,
    #[qapi(since = "8.2")]
    pub compressed: Option<bool>,
}
#[qapi(since = "2.1")]
pub struct Memdev {
    pub size: u64,
    pub merge: bool,
    pub dump: bool,
    pub prealloc: bool,
    #[qapi(name = "host-nodes")]
    pub host_nodes: Vec<u16>,
    pub policy: HostMemPolicy,
    #[qapi(since = "2.9")]
    pub id: Option<String>,
    #[qapi(since = "6.1")]
    pub share: Option<bool>,
    #[qapi(since = "6.1")]
    pub reserve: Option<bool>,
}
#[qapi(since = "6.2")]
pub struct MemoryBackendEpcProperties {
    pub dump: Option<bool>,
    #[qapi(name = "host-nodes")]
    pub host_nodes: Option<Vec<u16>>,
    pub merge: Option<bool>,
    pub policy: Option<HostMemPolicy>,
    pub prealloc: Option<bool>,
    #[qapi(name = "prealloc-threads")]
    pub prealloc_threads: Option<u32>,
    pub share: Option<bool>,
    pub reserve: Option<bool>,
    pub size: u64,
    #[qapi(name = "x-use-canonical-path-for-ramblock-id")]
    pub x_use_canonical_path_for_ramblock_id: Option<bool>,
    #[qapi(name = "prealloc-context")]
    #[qapi(since = "7.2")]
    pub prealloc_context: Option<String>,
}
#[qapi(since = "2.1")]
pub struct MemoryBackendFileProperties {
    pub dump: Option<bool>,
    #[qapi(name = "host-nodes")]
    pub host_nodes: Option<Vec<u16>>,
    pub merge: Option<bool>,
    pub policy: Option<HostMemPolicy>,
    pub prealloc: Option<bool>,
    #[qapi(name = "prealloc-threads")]
    pub prealloc_threads: Option<u32>,
    pub share: Option<bool>,
    pub size: u64,
    #[qapi(name = "x-use-canonical-path-for-ramblock-id")]
    pub x_use_canonical_path_for_ramblock_id: Option<bool>,
    pub align: Option<u64>,
    #[qapi(name = "discard-data")]
    pub discard_data: Option<bool>,
    #[qapi(name = "mem-path")]
    pub mem_path: String,
    #[qapi(condition = "defined(CONFIG_LIBPMEM)")]
    pub pmem: Option<bool>,
    pub readonly: Option<bool>,
    #[qapi(since = "6.1")]
    pub reserve: Option<bool>,
    #[qapi(name = "prealloc-context")]
    #[qapi(since = "7.2")]
    pub prealloc_context: Option<String>,
    #[qapi(since = "8.1")]
    pub offset: Option<u64>,
    #[qapi(since = "8.2")]
    pub rom: Option<OnOffAuto>,
}
#[qapi(since = "2.12")]
pub struct MemoryBackendMemfdProperties {
    pub dump: Option<bool>,
    #[qapi(name = "host-nodes")]
    pub host_nodes: Option<Vec<u16>>,
    pub merge: Option<bool>,
    pub policy: Option<HostMemPolicy>,
    pub prealloc: Option<bool>,
    #[qapi(name = "prealloc-threads")]
    pub prealloc_threads: Option<u32>,
    pub share: Option<bool>,
    pub size: u64,
    #[qapi(name = "x-use-canonical-path-for-ramblock-id")]
    pub x_use_canonical_path_for_ramblock_id: Option<bool>,
    pub hugetlb: Option<bool>,
    pub hugetlbsize: Option<u64>,
    pub seal: Option<bool>,
    #[qapi(since = "6.1")]
    pub reserve: Option<bool>,
    #[qapi(name = "prealloc-context")]
    #[qapi(since = "7.2")]
    pub prealloc_context: Option<String>,
}
#[qapi(since = "2.1")]
pub struct MemoryBackendProperties {
    pub dump: Option<bool>,
    #[qapi(name = "host-nodes")]
    pub host_nodes: Option<Vec<u16>>,
    pub merge: Option<bool>,
    pub policy: Option<HostMemPolicy>,
    pub prealloc: Option<bool>,
    #[qapi(name = "prealloc-threads")]
    pub prealloc_threads: Option<u32>,
    pub share: Option<bool>,
    pub size: u64,
    #[qapi(name = "x-use-canonical-path-for-ramblock-id")]
    pub x_use_canonical_path_for_ramblock_id: Option<bool>,
    #[qapi(since = "6.1")]
    pub reserve: Option<bool>,
    #[qapi(name = "prealloc-context")]
    #[qapi(since = "7.2")]
    pub prealloc_context: Option<String>,
}
#[qapi(condition = "CONFIG_POSIX")]
#[qapi(since = "9.1")]
pub struct MemoryBackendShmProperties {
    pub dump: Option<bool>,
    #[qapi(name = "host-nodes")]
    pub host_nodes: Option<Vec<u16>>,
    pub merge: Option<bool>,
    pub policy: Option<HostMemPolicy>,
    pub prealloc: Option<bool>,
    #[qapi(name = "prealloc-threads")]
    pub prealloc_threads: Option<u32>,
    #[qapi(name = "prealloc-context")]
    pub prealloc_context: Option<String>,
    pub share: Option<bool>,
    pub reserve: Option<bool>,
    pub size: u64,
    #[qapi(name = "x-use-canonical-path-for-ramblock-id")]
    pub x_use_canonical_path_for_ramblock_id: Option<bool>,
}
#[qapi(since = "2.1")]
pub struct MemoryDeviceInfo {
    #[qapi(discriminator)]
    pub r#type: MemoryDeviceInfoKind,
    #[qapi(union)]
    pub u: Option<MemoryDeviceInfoBranch>,
}
#[qapi(since = "5.2")]
pub struct MemoryFailureFlags {
    #[qapi(name = "action-required")]
    pub action_required: bool,
    pub recursive: bool,
}
#[qapi(since = "2.11.0")]
pub struct MemoryInfo {
    #[qapi(name = "base-memory")]
    pub base_memory: u64,
    #[qapi(name = "plugged-memory")]
    pub plugged_memory: Option<u64>,
}
#[qapi(since = "7.1")]
pub struct MemorySizeConfiguration {
    pub size: Option<u64>,
    #[qapi(name = "max-size")]
    pub max_size: Option<u64>,
    pub slots: Option<u64>,
}
#[qapi(since = "2.4")]
pub struct MigrateSetParameters {
    #[qapi(name = "compress-level")]
    #[qapi(until = "9.1")]
    pub compress_level: Option<i64>,
    #[qapi(name = "compress-threads")]
    #[qapi(until = "9.1")]
    pub compress_threads: Option<i64>,
    #[qapi(name = "decompress-threads")]
    #[qapi(until = "9.1")]
    pub decompress_threads: Option<i64>,
    #[qapi(name = "cpu-throttle-initial")]
    pub cpu_throttle_initial: Option<i64>,
    #[qapi(name = "cpu-throttle-increment")]
    pub cpu_throttle_increment: Option<i64>,
    #[qapi(name = "tls-creds")]
    pub tls_creds: Option<StrOrNull>,
    #[qapi(name = "tls-hostname")]
    pub tls_hostname: Option<StrOrNull>,
    #[qapi(name = "max-bandwidth")]
    pub max_bandwidth: Option<i64>,
    #[qapi(name = "downtime-limit")]
    pub downtime_limit: Option<i64>,
    #[qapi(name = "x-checkpoint-delay")]
    pub x_checkpoint_delay: Option<i64>,
    #[qapi(name = "block-incremental")]
    #[qapi(until = "9.1")]
    pub block_incremental: Option<bool>,
    #[qapi(name = "x-multifd-channels")]
    #[qapi(since = "2.11")]
    #[qapi(until = "4.0")]
    pub x_multifd_channels: Option<i64>,
    #[qapi(name = "x-multifd-page-count")]
    #[qapi(since = "2.11")]
    #[qapi(until = "4.0")]
    pub x_multifd_page_count: Option<i64>,
    #[qapi(name = "xbzrle-cache-size")]
    #[qapi(since = "2.11")]
    pub xbzrle_cache_size: Option<u64>,
    #[qapi(name = "max-postcopy-bandwidth")]
    #[qapi(since = "3.0")]
    pub max_postcopy_bandwidth: Option<u64>,
    #[qapi(name = "compress-wait-thread")]
    #[qapi(since = "3.1")]
    #[qapi(until = "9.1")]
    pub compress_wait_thread: Option<bool>,
    #[qapi(name = "max-cpu-throttle")]
    #[qapi(since = "3.1")]
    pub max_cpu_throttle: Option<i64>,
    #[qapi(name = "announce-initial")]
    #[qapi(since = "4.0")]
    pub announce_initial: Option<u64>,
    #[qapi(name = "announce-max")]
    #[qapi(since = "4.0")]
    pub announce_max: Option<u64>,
    #[qapi(name = "announce-rounds")]
    #[qapi(since = "4.0")]
    pub announce_rounds: Option<u64>,
    #[qapi(name = "announce-step")]
    #[qapi(since = "4.0")]
    pub announce_step: Option<u64>,
    #[qapi(name = "tls-authz")]
    #[qapi(since = "4.0")]
    pub tls_authz: Option<StrOrNull>,
    #[qapi(name = "multifd-channels")]
    #[qapi(since = "4.0")]
    pub multifd_channels: Option<i64>,
    #[qapi(name = "throttle-trigger-threshold")]
    #[qapi(since = "5.0")]
    pub throttle_trigger_threshold: Option<i64>,
    #[qapi(name = "multifd-compression")]
    #[qapi(since = "5.0")]
    pub multifd_compression: Option<MultiFdCompression>,
    #[qapi(name = "multifd-zlib-level")]
    #[qapi(since = "5.0")]
    pub multifd_zlib_level: Option<i64>,
    #[qapi(name = "multifd-zstd-level")]
    #[qapi(since = "5.0")]
    pub multifd_zstd_level: Option<i64>,
    #[qapi(name = "cpu-throttle-tailslow")]
    #[qapi(since = "5.1")]
    pub cpu_throttle_tailslow: Option<bool>,
    #[qapi(name = "block-bitmap-mapping")]
    #[qapi(since = "5.2")]
    pub block_bitmap_mapping: Option<Vec<BitmapMigrationNodeAlias>>,
    #[qapi(name = "x-vcpu-dirty-limit-period")]
    #[qapi(feature = "unstable")]
    #[qapi(since = "8.1")]
    pub x_vcpu_dirty_limit_period: Option<u64>,
    #[qapi(name = "vcpu-dirty-limit")]
    #[qapi(since = "8.1")]
    pub vcpu_dirty_limit: Option<u64>,
    #[qapi(name = "avail-switchover-bandwidth")]
    #[qapi(since = "8.2")]
    pub avail_switchover_bandwidth: Option<u64>,
    #[qapi(since = "8.2")]
    pub mode: Option<MigMode>,
    #[qapi(name = "zero-page-detection")]
    #[qapi(since = "9.0")]
    pub zero_page_detection: Option<ZeroPageDetection>,
    #[qapi(name = "direct-io")]
    #[qapi(since = "9.1")]
    pub direct_io: Option<bool>,
    #[qapi(name = "multifd-qatzip-level")]
    #[qapi(since = "9.2")]
    pub multifd_qatzip_level: Option<u8>,
    #[qapi(name = "cpr-exec-command")]
    #[qapi(since = "10.2")]
    pub cpr_exec_command: Option<Vec<String>>,
}
#[qapi(since = "8.2")]
pub struct MigrationAddress {
    #[qapi(discriminator)]
    pub transport: MigrationAddressType,
    #[qapi(union)]
    pub u: Option<MigrationAddressBranch>,
}
#[qapi(since = "1.2")]
pub struct MigrationCapabilityStatus {
    pub capability: MigrationCapability,
    pub state: bool,
}
#[qapi(since = "8.1")]
pub struct MigrationChannel {
    #[qapi(name = "channel-type")]
    pub channel_type: MigrationChannelType,
    pub addr: MigrationAddress,
}
#[qapi(since = "8.2")]
pub struct MigrationExecCommand {
    pub args: Vec<String>,
}
#[qapi(since = "0.14.0")]
pub struct MigrationInfo {
    pub status: Option<String>,
    pub ram: Option<MigrationStats>,
    #[qapi(until = "9.1")]
    pub disk: Option<MigrationStats>,
    #[qapi(name = "xbzrle-cache")]
    #[qapi(since = "1.2")]
    pub xbzrle_cache: Option<XbzrleCacheStats>,
    #[qapi(name = "total-time")]
    #[qapi(since = "1.2")]
    pub total_time: Option<i64>,
    #[qapi(name = "expected-downtime")]
    #[qapi(since = "1.3")]
    pub expected_downtime: Option<i64>,
    #[qapi(since = "1.3")]
    pub downtime: Option<i64>,
    #[qapi(name = "setup-time")]
    #[qapi(since = "1.6")]
    pub setup_time: Option<i64>,
    #[qapi(name = "x-cpu-throttle-percentage")]
    #[qapi(since = "2.5")]
    #[qapi(until = "2.7")]
    pub x_cpu_throttle_percentage: Option<i64>,
    #[qapi(name = "cpu-throttle-percentage")]
    #[qapi(since = "2.7")]
    pub cpu_throttle_percentage: Option<i64>,
    #[qapi(name = "error-desc")]
    #[qapi(since = "2.7")]
    pub error_desc: Option<String>,
    #[qapi(name = "postcopy-blocktime")]
    #[qapi(since = "3.0")]
    pub postcopy_blocktime: Option<u32>,
    #[qapi(name = "postcopy-vcpu-blocktime")]
    #[qapi(since = "3.0")]
    pub postcopy_vcpu_blocktime: Option<Vec<u32>>,
    #[qapi(since = "3.1")]
    #[qapi(until = "9.1")]
    pub compression: Option<CompressionStats>,
    #[qapi(name = "socket-address")]
    #[qapi(since = "4.0")]
    pub socket_address: Option<Vec<SocketAddress>>,
    #[qapi(since = "5.2")]
    pub vfio: Option<VfioStats>,
    #[qapi(feature = "deprecated")]
    #[qapi(since = "6.0")]
    #[qapi(until = "6.1")]
    pub blocked: Option<bool>,
    #[qapi(name = "blocked-reasons")]
    #[qapi(since = "6.0")]
    pub blocked_reasons: Option<Vec<String>>,
    #[qapi(name = "dirty-limit-throttle-time-per-round")]
    #[qapi(since = "8.1")]
    pub dirty_limit_throttle_time_per_round: Option<u64>,
    #[qapi(name = "dirty-limit-ring-full-time")]
    #[qapi(since = "8.1")]
    pub dirty_limit_ring_full_time: Option<u64>,
    #[qapi(name = "postcopy-latency")]
    #[qapi(feature = "unstable")]
    #[qapi(since = "10.1")]
    pub postcopy_latency: Option<u64>,
    #[qapi(name = "postcopy-latency-dist")]
    #[qapi(feature = "unstable")]
    #[qapi(since = "10.1")]
    pub postcopy_latency_dist: Option<Vec<u64>>,
    #[qapi(name = "postcopy-vcpu-latency")]
    #[qapi(feature = "unstable")]
    #[qapi(since = "10.1")]
    pub postcopy_vcpu_latency: Option<Vec<u64>>,
    #[qapi(name = "postcopy-non-vcpu-latency")]
    #[qapi(feature = "unstable")]
    #[qapi(since = "10.1")]
    pub postcopy_non_vcpu_latency: Option<u64>,
}
#[qapi(since = "2.4")]
pub struct MigrationParameters {
    #[qapi(name = "compress-level")]
    #[qapi(until = "9.1")]
    pub compress_level: Option<i64>,
    #[qapi(name = "compress-threads")]
    #[qapi(until = "9.1")]
    pub compress_threads: Option<i64>,
    #[qapi(name = "decompress-threads")]
    #[qapi(until = "9.1")]
    pub decompress_threads: Option<i64>,
    #[qapi(name = "cpu-throttle-initial")]
    pub cpu_throttle_initial: Option<i64>,
    #[qapi(name = "cpu-throttle-increment")]
    pub cpu_throttle_increment: Option<i64>,
    #[qapi(name = "tls-creds")]
    pub tls_creds: Option<String>,
    #[qapi(name = "tls-hostname")]
    pub tls_hostname: Option<String>,
    #[qapi(name = "max-bandwidth")]
    pub max_bandwidth: Option<i64>,
    #[qapi(name = "downtime-limit")]
    pub downtime_limit: Option<i64>,
    #[qapi(name = "x-checkpoint-delay")]
    pub x_checkpoint_delay: Option<i64>,
    #[qapi(name = "block-incremental")]
    #[qapi(since = "2.10")]
    #[qapi(until = "9.1")]
    pub block_incremental: Option<bool>,
    #[qapi(name = "x-multifd-channels")]
    #[qapi(since = "2.11")]
    #[qapi(until = "4.0")]
    pub x_multifd_channels: Option<i64>,
    #[qapi(name = "x-multifd-page-count")]
    #[qapi(since = "2.11")]
    #[qapi(until = "4.0")]
    pub x_multifd_page_count: Option<i64>,
    #[qapi(name = "xbzrle-cache-size")]
    #[qapi(since = "2.11")]
    pub xbzrle_cache_size: Option<u64>,
    #[qapi(name = "max-postcopy-bandwidth")]
    #[qapi(since = "3.0")]
    pub max_postcopy_bandwidth: Option<u64>,
    #[qapi(name = "compress-wait-thread")]
    #[qapi(since = "3.1")]
    #[qapi(until = "9.1")]
    pub compress_wait_thread: Option<bool>,
    #[qapi(name = "max-cpu-throttle")]
    #[qapi(since = "3.1")]
    pub max_cpu_throttle: Option<u8>,
    #[qapi(name = "announce-initial")]
    #[qapi(since = "4.0")]
    pub announce_initial: Option<u64>,
    #[qapi(name = "announce-max")]
    #[qapi(since = "4.0")]
    pub announce_max: Option<u64>,
    #[qapi(name = "announce-rounds")]
    #[qapi(since = "4.0")]
    pub announce_rounds: Option<u64>,
    #[qapi(name = "announce-step")]
    #[qapi(since = "4.0")]
    pub announce_step: Option<u64>,
    #[qapi(name = "tls-authz")]
    #[qapi(since = "4.0")]
    pub tls_authz: Option<String>,
    #[qapi(name = "multifd-channels")]
    #[qapi(since = "4.0")]
    pub multifd_channels: Option<u8>,
    #[qapi(name = "throttle-trigger-threshold")]
    #[qapi(since = "5.0")]
    pub throttle_trigger_threshold: Option<u8>,
    #[qapi(name = "multifd-compression")]
    #[qapi(since = "5.0")]
    pub multifd_compression: Option<MultiFdCompression>,
    #[qapi(name = "multifd-zlib-level")]
    #[qapi(since = "5.0")]
    pub multifd_zlib_level: Option<u8>,
    #[qapi(name = "multifd-zstd-level")]
    #[qapi(since = "5.0")]
    pub multifd_zstd_level: Option<u8>,
    #[qapi(name = "cpu-throttle-tailslow")]
    #[qapi(since = "5.1")]
    pub cpu_throttle_tailslow: Option<bool>,
    #[qapi(name = "block-bitmap-mapping")]
    #[qapi(since = "5.2")]
    pub block_bitmap_mapping: Option<Vec<BitmapMigrationNodeAlias>>,
    #[qapi(name = "x-vcpu-dirty-limit-period")]
    #[qapi(feature = "unstable")]
    #[qapi(since = "8.1")]
    pub x_vcpu_dirty_limit_period: Option<u64>,
    #[qapi(name = "vcpu-dirty-limit")]
    #[qapi(since = "8.1")]
    pub vcpu_dirty_limit: Option<u64>,
    #[qapi(name = "avail-switchover-bandwidth")]
    #[qapi(since = "8.2")]
    pub avail_switchover_bandwidth: Option<u64>,
    #[qapi(since = "8.2")]
    pub mode: Option<MigMode>,
    #[qapi(name = "zero-page-detection")]
    #[qapi(since = "9.0")]
    pub zero_page_detection: Option<ZeroPageDetection>,
    #[qapi(name = "direct-io")]
    #[qapi(since = "9.1")]
    pub direct_io: Option<bool>,
    #[qapi(name = "multifd-qatzip-level")]
    #[qapi(since = "9.2")]
    pub multifd_qatzip_level: Option<u8>,
    #[qapi(name = "cpr-exec-command")]
    #[qapi(since = "10.2")]
    pub cpr_exec_command: Option<Vec<String>>,
}
#[qapi(since = "0.14.0.")]
pub struct MigrationStats {
    pub transferred: i64,
    pub remaining: i64,
    pub total: i64,
    #[qapi(since = "1.2")]
    pub duplicate: Option<i64>,
    #[qapi(since = "1.2")]
    pub normal: Option<i64>,
    #[qapi(name = "normal-bytes")]
    #[qapi(since = "1.2")]
    pub normal_bytes: Option<i64>,
    #[qapi(name = "dirty-pages-rate")]
    #[qapi(since = "1.3")]
    pub dirty_pages_rate: Option<i64>,
    #[qapi(since = "1.5")]
    #[qapi(until = "9.1")]
    pub skipped: Option<i64>,
    #[qapi(since = "1.6")]
    pub mbps: Option<f64>,
    #[qapi(name = "dirty-sync-count")]
    #[qapi(since = "2.8")]
    pub dirty_sync_count: Option<i64>,
    #[qapi(name = "postcopy-requests")]
    #[qapi(since = "2.8")]
    pub postcopy_requests: Option<i64>,
    #[qapi(name = "page-size")]
    #[qapi(since = "2.10")]
    pub page_size: Option<i64>,
    #[qapi(name = "multifd-bytes")]
    #[qapi(since = "3.0")]
    pub multifd_bytes: Option<u64>,
    #[qapi(name = "pages-per-second")]
    #[qapi(since = "4.0")]
    pub pages_per_second: Option<u64>,
    #[qapi(name = "precopy-bytes")]
    #[qapi(since = "7.0")]
    pub precopy_bytes: Option<u64>,
    #[qapi(name = "downtime-bytes")]
    #[qapi(since = "7.0")]
    pub downtime_bytes: Option<u64>,
    #[qapi(name = "postcopy-bytes")]
    #[qapi(since = "7.0")]
    pub postcopy_bytes: Option<u64>,
    #[qapi(name = "dirty-sync-missed-zero-copy")]
    #[qapi(since = "7.1")]
    pub dirty_sync_missed_zero_copy: Option<u64>,
}
#[qapi(since = "7.2")]
pub struct MigrationThreadInfo {
    pub name: String,
    #[qapi(name = "thread-id")]
    pub thread_id: i64,
}
#[qapi(since = "5.0")]
pub struct MonitorOptions {
    pub id: Option<String>,
    pub mode: Option<MonitorMode>,
    pub pretty: Option<bool>,
    pub chardev: String,
}
#[qapi(since = "0.14.0")]
pub struct MouseInfo {
    pub name: String,
    pub index: i64,
    pub current: bool,
    pub absolute: bool,
}
#[qapi(event)]
#[qapi(name = "NETDEV_STREAM_CONNECTED")]
#[qapi(since = "7.2")]
pub struct NetdevStreamConnectedEvent {
    #[qapi(name = "netdev-id")]
    pub netdev_id: String,
    pub addr: SocketAddress,
}
#[qapi(event)]
#[qapi(name = "NETDEV_STREAM_DISCONNECTED")]
#[qapi(since = "7.2")]
pub struct NetdevStreamDisconnectedEvent {
    #[qapi(name = "netdev-id")]
    pub netdev_id: String,
}
#[qapi(event)]
#[qapi(name = "NETDEV_VHOST_USER_CONNECTED")]
#[qapi(since = "10.0")]
pub struct NetdevVhostUserConnectedEvent {
    #[qapi(name = "netdev-id")]
    pub netdev_id: String,
    #[qapi(name = "chardev-id")]
    pub chardev_id: String,
}
#[qapi(event)]
#[qapi(name = "NETDEV_VHOST_USER_DISCONNECTED")]
#[qapi(since = "10.0")]
pub struct NetdevVhostUserDisconnectedEvent {
    #[qapi(name = "netdev-id")]
    pub netdev_id: String,
}
#[qapi(name = "NFSServer")]
#[qapi(since = "2.8")]
pub struct NfsServer {
    pub r#type: NfsTransport,
    pub host: String,
}
#[qapi(event)]
#[qapi(name = "NIC_RX_FILTER_CHANGED")]
#[qapi(since = "1.6")]
pub struct NicRxFilterChangedEvent {
    pub name: Option<String>,
    pub path: String,
}
#[qapi(since = "0.14.0")]
pub struct NameInfo {
    pub name: Option<String>,
}
#[qapi(since = "5.0")]
pub struct NbdServerAddOptions {
    pub name: Option<String>,
    pub description: Option<String>,
    pub device: String,
    pub writable: Option<bool>,
    pub bitmap: Option<String>,
}
#[qapi(since = "4.2")]
pub struct NbdServerOptions {
    pub addr: SocketAddress,
    #[qapi(name = "tls-creds")]
    pub tls_creds: Option<String>,
    #[qapi(name = "tls-authz")]
    pub tls_authz: Option<String>,
    #[qapi(name = "max-connections")]
    #[qapi(since = "5.2")]
    pub max_connections: Option<u32>,
    #[qapi(name = "handshake-max-seconds")]
    #[qapi(since = "10.0")]
    pub handshake_max_seconds: Option<u32>,
}
#[qapi(since = "10.0")]
pub struct NbdServerOptionsBase {
    #[qapi(name = "handshake-max-seconds")]
    pub handshake_max_seconds: Option<u32>,
    #[qapi(name = "tls-creds")]
    pub tls_creds: Option<String>,
    #[qapi(name = "tls-authz")]
    pub tls_authz: Option<String>,
    #[qapi(name = "max-connections")]
    pub max_connections: Option<u32>,
}
#[qapi(since = "10.0")]
pub struct NbdServerOptionsLegacy {
    #[qapi(name = "handshake-max-seconds")]
    pub handshake_max_seconds: Option<u32>,
    #[qapi(name = "tls-creds")]
    pub tls_creds: Option<String>,
    #[qapi(name = "tls-authz")]
    pub tls_authz: Option<String>,
    #[qapi(name = "max-connections")]
    pub max_connections: Option<u32>,
    pub addr: SocketAddressLegacy,
}
#[qapi(since = "1.6")]
#[qapi(until = "2.7")]
pub struct NetClientOptions {
    #[qapi(discriminator)]
    pub r#type: NetClientOptionsKind,
    #[qapi(union)]
    pub u: Option<NetClientOptionsBranch>,
}
#[qapi(condition = "CONFIG_PASST")]
#[qapi(since = "10.1")]
pub struct NetDevPasstOptions {
    pub path: Option<String>,
    pub quiet: Option<bool>,
    #[qapi(name = "vhost-user")]
    pub vhost_user: Option<bool>,
    pub mtu: Option<i64>,
    pub address: Option<String>,
    pub netmask: Option<String>,
    pub mac: Option<String>,
    pub gateway: Option<String>,
    pub interface: Option<String>,
    pub outbound: Option<String>,
    #[qapi(name = "outbound-if4")]
    pub outbound_if4: Option<String>,
    #[qapi(name = "outbound-if6")]
    pub outbound_if6: Option<String>,
    pub dns: Option<String>,
    pub search: Option<Vec<String>>,
    pub fqdn: Option<String>,
    #[qapi(name = "dhcp-dns")]
    pub dhcp_dns: Option<bool>,
    #[qapi(name = "dhcp-search")]
    pub dhcp_search: Option<bool>,
    #[qapi(name = "map-host-loopback")]
    pub map_host_loopback: Option<String>,
    #[qapi(name = "map-guest-addr")]
    pub map_guest_addr: Option<String>,
    #[qapi(name = "dns-forward")]
    pub dns_forward: Option<String>,
    #[qapi(name = "dns-host")]
    pub dns_host: Option<String>,
    pub tcp: Option<bool>,
    pub udp: Option<bool>,
    pub icmp: Option<bool>,
    pub dhcp: Option<bool>,
    pub ndp: Option<bool>,
    pub dhcpv6: Option<bool>,
    pub ra: Option<bool>,
    pub freebind: Option<bool>,
    pub ipv4: Option<bool>,
    pub ipv6: Option<bool>,
    #[qapi(name = "tcp-ports")]
    pub tcp_ports: Option<Vec<String>>,
    #[qapi(name = "udp-ports")]
    pub udp_ports: Option<Vec<String>>,
    pub param: Option<Vec<String>>,
}
#[qapi(since = "1.2")]
#[qapi(until = "5.1")]
pub struct NetLegacy {
    #[qapi(until = "3.0")]
    pub vlan: Option<i32>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub opts: NetClientOptions,
}
#[qapi(since = "1.2")]
pub struct NetLegacyNicOptions {
    pub netdev: Option<String>,
    pub macaddr: Option<String>,
    pub model: Option<String>,
    pub addr: Option<String>,
    pub vectors: Option<u32>,
}
#[qapi(since = "2.7")]
#[qapi(until = "5.1")]
pub struct NetLegacyOptions {
    #[qapi(discriminator)]
    pub r#type: NetLegacyOptionsKind,
    #[qapi(union)]
    pub u: Option<NetLegacyOptionsBranch>,
}
#[qapi(since = "1.2")]
pub struct Netdev {
    pub id: String,
    #[qapi(until = "2.7")]
    pub opts: Option<NetClientOptions>,
    #[qapi(discriminator)]
    #[qapi(since = "2.7")]
    pub r#type: Option<NetClientDriver>,
    #[qapi(union)]
    #[qapi(since = "2.7")]
    pub u: Option<NetdevBranch>,
}
#[qapi(name = "NetdevAFXDPOptions")]
#[qapi(condition = "CONFIG_AF_XDP")]
#[qapi(since = "8.2")]
pub struct NetdevAfxdpOptions {
    pub ifname: String,
    pub mode: Option<AfxdpMode>,
    #[qapi(name = "force-copy")]
    pub force_copy: Option<bool>,
    pub queues: Option<i64>,
    #[qapi(name = "start-queue")]
    pub start_queue: Option<i64>,
    pub inhibit: Option<bool>,
    #[qapi(name = "sock-fds")]
    pub sock_fds: Option<String>,
    #[qapi(name = "map-path")]
    #[qapi(since = "10.1")]
    pub map_path: Option<String>,
    #[qapi(name = "map-start-index")]
    #[qapi(since = "10.1")]
    pub map_start_index: Option<i32>,
}
#[qapi(since = "1.2")]
pub struct NetdevBridgeOptions {
    pub br: Option<String>,
    pub helper: Option<String>,
}
#[qapi(since = "7.2")]
pub struct NetdevDgramOptions {
    pub local: Option<SocketAddress>,
    pub remote: Option<SocketAddress>,
}
#[qapi(since = "1.2")]
#[qapi(until = "2.12")]
pub struct NetdevDumpOptions {
    pub len: Option<u64>,
    pub file: Option<String>,
}
#[qapi(since = "1.2")]
pub struct NetdevHubPortOptions {
    pub hubid: i32,
    #[qapi(since = "2.12")]
    pub netdev: Option<String>,
}
#[qapi(name = "NetdevL2TPv3Options")]
#[qapi(since = "2.1")]
pub struct NetdevL2tPv3Options {
    pub src: String,
    pub dst: String,
    pub srcport: Option<String>,
    pub dstport: Option<String>,
    pub ipv6: Option<bool>,
    pub udp: Option<bool>,
    pub cookie64: Option<bool>,
    pub counter: Option<bool>,
    pub pincounter: Option<bool>,
    pub txcookie: Option<u64>,
    pub rxcookie: Option<u64>,
    pub txsession: u32,
    pub rxsession: Option<u32>,
    pub offset: Option<u32>,
}
#[qapi(since = "2.0")]
pub struct NetdevNetmapOptions {
    pub ifname: String,
    pub devname: Option<String>,
}
#[qapi(since = "1.2")]
#[qapi(until = "3.0")]
pub struct NetdevNoneOptions {}
#[qapi(since = "1.2")]
pub struct NetdevSocketOptions {
    pub fd: Option<String>,
    pub listen: Option<String>,
    pub connect: Option<String>,
    pub mcast: Option<String>,
    pub localaddr: Option<String>,
    pub udp: Option<String>,
}
#[qapi(since = "7.2")]
pub struct NetdevStreamOptions {
    pub addr: SocketAddress,
    pub server: Option<bool>,
    #[qapi(since = "8.0")]
    #[qapi(until = "10.2")]
    pub reconnect: Option<u32>,
    #[qapi(name = "reconnect-ms")]
    #[qapi(since = "9.2")]
    pub reconnect_ms: Option<i64>,
}
#[qapi(since = "1.2")]
pub struct NetdevTapOptions {
    pub ifname: Option<String>,
    pub fd: Option<String>,
    pub fds: Option<String>,
    pub script: Option<String>,
    pub downscript: Option<String>,
    pub helper: Option<String>,
    pub sndbuf: Option<u64>,
    pub vnet_hdr: Option<bool>,
    pub vhost: Option<bool>,
    pub vhostfd: Option<String>,
    pub vhostfds: Option<String>,
    pub vhostforce: Option<bool>,
    pub queues: Option<u32>,
    #[qapi(name = "poll-us")]
    #[qapi(since = "2.7")]
    pub poll_us: Option<u32>,
    #[qapi(since = "2.8")]
    pub br: Option<String>,
}
#[qapi(since = "1.2")]
pub struct NetdevUserOptions {
    pub hostname: Option<String>,
    pub restrict: Option<bool>,
    pub ip: Option<String>,
    pub net: Option<String>,
    pub host: Option<String>,
    pub tftp: Option<String>,
    pub bootfile: Option<String>,
    pub dhcpstart: Option<String>,
    pub dns: Option<String>,
    pub dnssearch: Option<Vec<String>>,
    pub smb: Option<String>,
    pub smbserver: Option<String>,
    pub hostfwd: Option<Vec<String>>,
    pub guestfwd: Option<Vec<String>>,
    #[qapi(since = "2.6")]
    pub ipv4: Option<bool>,
    #[qapi(since = "2.6")]
    pub ipv6: Option<bool>,
    #[qapi(name = "ipv6-prefix")]
    #[qapi(since = "2.6")]
    pub ipv6_prefix: Option<String>,
    #[qapi(name = "ipv6-prefixlen")]
    #[qapi(since = "2.6")]
    pub ipv6_prefixlen: Option<i64>,
    #[qapi(name = "ipv6-host")]
    #[qapi(since = "2.6")]
    pub ipv6_host: Option<String>,
    #[qapi(name = "ipv6-dns")]
    #[qapi(since = "2.6")]
    pub ipv6_dns: Option<String>,
    #[qapi(since = "3.0")]
    pub domainname: Option<String>,
    #[qapi(name = "tftp-server-name")]
    #[qapi(since = "3.1")]
    pub tftp_server_name: Option<String>,
}
#[qapi(since = "1.2")]
pub struct NetdevVdeOptions {
    pub sock: Option<String>,
    pub port: Option<u16>,
    pub group: Option<String>,
    pub mode: Option<u16>,
}
#[qapi(since = "2.1")]
pub struct NetdevVhostUserOptions {
    pub chardev: String,
    pub vhostforce: Option<bool>,
    #[qapi(since = "2.5")]
    pub queues: Option<i64>,
}
#[qapi(name = "NetdevVhostVDPAOptions")]
#[qapi(since = "5.1")]
pub struct NetdevVhostVdpaOptions {
    pub vhostdev: Option<String>,
    pub queues: Option<i64>,
    #[qapi(name = "x-svq")]
    #[qapi(feature = "unstable")]
    #[qapi(since = "7.1")]
    pub x_svq: Option<bool>,
    #[qapi(since = "7.2")]
    pub vhostfd: Option<String>,
}
#[qapi(condition = "CONFIG_VMNET")]
#[qapi(since = "7.1")]
pub struct NetdevVmnetBridgedOptions {
    pub ifname: String,
    pub isolated: Option<bool>,
}
#[qapi(condition = "CONFIG_VMNET")]
#[qapi(since = "7.1")]
pub struct NetdevVmnetHostOptions {
    #[qapi(name = "start-address")]
    pub start_address: Option<String>,
    #[qapi(name = "end-address")]
    pub end_address: Option<String>,
    #[qapi(name = "subnet-mask")]
    pub subnet_mask: Option<String>,
    pub isolated: Option<bool>,
    #[qapi(name = "net-uuid")]
    pub net_uuid: Option<String>,
}
#[qapi(condition = "CONFIG_VMNET")]
#[qapi(since = "7.1")]
pub struct NetdevVmnetSharedOptions {
    #[qapi(name = "start-address")]
    pub start_address: Option<String>,
    #[qapi(name = "end-address")]
    pub end_address: Option<String>,
    #[qapi(name = "subnet-mask")]
    pub subnet_mask: Option<String>,
    pub isolated: Option<bool>,
    #[qapi(name = "nat66-prefix")]
    pub nat66_prefix: Option<String>,
}
#[qapi(since = "2.5")]
pub struct NetfilterProperties {
    pub netdev: String,
    pub queue: Option<NetFilterDirection>,
    pub status: Option<String>,
    pub position: Option<String>,
    pub insert: Option<NetfilterInsert>,
}
#[qapi(since = "2.10")]
pub struct NumaCpuOptions {
    #[qapi(name = "node-id")]
    pub node_id: Option<i64>,
    #[qapi(name = "socket-id")]
    pub socket_id: Option<i64>,
    #[qapi(name = "core-id")]
    pub core_id: Option<i64>,
    #[qapi(name = "thread-id")]
    pub thread_id: Option<i64>,
    #[qapi(name = "die-id")]
    #[qapi(since = "4.1")]
    pub die_id: Option<i64>,
    #[qapi(name = "cluster-id")]
    #[qapi(since = "7.1")]
    pub cluster_id: Option<i64>,
    #[qapi(name = "drawer-id")]
    #[qapi(since = "8.2")]
    pub drawer_id: Option<i64>,
    #[qapi(name = "book-id")]
    #[qapi(since = "8.2")]
    pub book_id: Option<i64>,
    #[qapi(name = "module-id")]
    #[qapi(since = "9.1")]
    pub module_id: Option<i64>,
}
#[qapi(since = "2.10")]
pub struct NumaDistOptions {
    pub src: u16,
    pub dst: u16,
    pub val: u8,
}
#[qapi(since = "5.0")]
pub struct NumaHmatCacheOptions {
    #[qapi(name = "node-id")]
    pub node_id: u32,
    pub size: u64,
    pub level: u8,
    pub associativity: HmatCacheAssociativity,
    pub policy: HmatCacheWritePolicy,
    pub line: u16,
}
#[qapi(name = "NumaHmatLBOptions")]
#[qapi(since = "5.0")]
pub struct NumaHmatLbOptions {
    pub initiator: u16,
    pub target: u16,
    pub hierarchy: HmatLbMemoryHierarchy,
    #[qapi(name = "data-type")]
    pub data_type: HmatLbDataType,
    pub latency: Option<u64>,
    pub bandwidth: Option<u64>,
}
#[qapi(since = "2.1")]
pub struct NumaNodeOptions {
    pub nodeid: Option<u16>,
    pub cpus: Option<Vec<u16>>,
    pub mem: Option<u64>,
    pub memdev: Option<String>,
    #[qapi(since = "5.0")]
    pub initiator: Option<u16>,
}
#[qapi(since = "2.1")]
pub struct NumaOptions {
    #[qapi(discriminator)]
    pub r#type: NumaOptionsKind,
    #[qapi(union)]
    pub u: Option<NumaOptionsBranch>,
}
#[qapi(since = "6.0")]
pub struct ObjectOptions {
    #[qapi(name = "qom-type")]
    #[qapi(discriminator)]
    pub qom_type: ObjectType,
    pub id: String,
    #[qapi(union)]
    pub u: Option<ObjectOptionsBranch>,
}
#[qapi(since = "10.1")]
pub struct ObjectPropertiesValues {
    pub properties: Vec<ObjectPropertyValue>,
}
#[qapi(since = "1.1")]
pub struct ObjectPropertyInfo {
    pub name: String,
    pub r#type: String,
    #[qapi(since = "2.12")]
    pub description: Option<String>,
    #[qapi(name = "default-value")]
    #[qapi(since = "5.0")]
    pub default_value: Option<serde_json::Value>,
}
#[qapi(since = "10.1")]
pub struct ObjectPropertyValue {
    pub name: String,
    pub r#type: String,
    pub value: Option<serde_json::Value>,
}
#[qapi(since = "1.1")]
pub struct ObjectTypeInfo {
    pub name: String,
    #[qapi(since = "2.10")]
    pub r#abstract: Option<bool>,
    #[qapi(since = "2.10")]
    pub parent: Option<String>,
}
#[qapi(name = "PCDIMMDeviceInfo")]
#[qapi(since = "2.1")]
pub struct PcdimmDeviceInfo {
    pub id: Option<String>,
    pub addr: i64,
    pub size: i64,
    pub slot: i64,
    pub node: i64,
    pub memdev: String,
    pub hotplugged: bool,
    pub hotpluggable: bool,
}
#[qapi(name = "PCDIMMDeviceInfoWrapper")]
#[qapi(since = "2.1")]
pub struct PcdimmDeviceInfoWrapper {
    pub data: PcdimmDeviceInfo,
}
#[qapi(event)]
#[qapi(name = "POWERDOWN")]
#[qapi(since = "0.12.0")]
pub struct PowerdownEvent {}
#[qapi(name = "PRManagerInfo")]
#[qapi(since = "3.0")]
pub struct PrManagerInfo {
    pub id: String,
    pub connected: bool,
}
#[qapi(event)]
#[qapi(name = "PR_MANAGER_STATUS_CHANGED")]
#[qapi(since = "3.0")]
pub struct PrManagerStatusChangedEvent {
    pub id: String,
    pub connected: bool,
}
#[qapi(since = "1.1")]
pub struct PciBridgeInfo {
    pub bus: PciBusInfo,
    pub devices: Option<Box<Vec<PciDeviceInfo>>>,
}
#[qapi(since = "0.14.0")]
pub struct PciBusInfo {
    pub number: i64,
    pub secondary: i64,
    pub subordinate: i64,
    pub io_range: PciMemoryRange,
    pub memory_range: PciMemoryRange,
    pub prefetchable_range: PciMemoryRange,
}
#[qapi(since = "1.1")]
pub struct PciDeviceClass {
    pub desc: Option<String>,
    pub class: i64,
}
#[qapi(since = "0.14.0")]
pub struct PciDeviceId {
    pub device: i64,
    pub vendor: i64,
    #[qapi(since = "3.1")]
    pub subsystem: Option<i64>,
    #[qapi(name = "subsystem-vendor")]
    #[qapi(since = "3.1")]
    pub subsystem_vendor: Option<i64>,
}
#[qapi(since = "1.1")]
pub struct PciDeviceInfo {
    pub bus: i64,
    pub slot: i64,
    pub function: i64,
    pub class_info: PciDeviceClass,
    pub id: PciDeviceId,
    pub irq: Option<i64>,
    pub qdev_id: String,
    pub pci_bridge: Option<Box<PciBridgeInfo>>,
    pub regions: Vec<PciMemoryRegion>,
    #[qapi(since = "5.1")]
    pub irq_pin: Option<i64>,
}
#[qapi(since = "0.14.0")]
pub struct PciInfo {
    pub bus: i64,
    pub devices: Vec<PciDeviceInfo>,
}
#[qapi(since = "0.14.0")]
pub struct PciMemoryRange {
    pub base: i64,
    pub limit: i64,
}
#[qapi(since = "0.14.0")]
pub struct PciMemoryRegion {
    pub bar: i64,
    pub r#type: String,
    pub address: i64,
    pub size: i64,
    pub prefetch: Option<bool>,
    pub mem_type_64: Option<bool>,
}
#[qapi(since = "2.11")]
pub struct PrManagerHelperProperties {
    pub path: String,
}
#[qapi(since = "4.0")]
pub struct QAuthZListRule {
    pub r#match: String,
    pub policy: QAuthZListPolicy,
    pub format: Option<QAuthZListFormat>,
}
#[qapi(since = "4.0")]
#[qapi(until = "6.0")]
pub struct QAuthZListRuleListHack {
    pub unused: Vec<QAuthZListRule>,
}
#[qapi(since = "7.1")]
pub struct QCryptoAkCipherOptions {
    #[qapi(discriminator)]
    pub alg: QCryptoAkCipherAlgorithm,
    #[qapi(union)]
    pub u: Option<QCryptoAkCipherOptionsBranch>,
}
#[qapi(name = "QCryptoAkCipherOptionsRSA")]
#[qapi(since = "7.1")]
pub struct QCryptoAkCipherOptionsRsa {
    #[qapi(name = "hash-alg")]
    pub hash_alg: QCryptoHashAlgorithm,
    #[qapi(name = "padding-alg")]
    pub padding_alg: QCryptoRsaPaddingAlgorithm,
}
#[qapi(since = "5.1")]
pub struct QCryptoBlockAmendOptions {
    #[qapi(discriminator)]
    pub format: QCryptoBlockFormat,
    #[qapi(union)]
    pub u: Option<QCryptoBlockAmendOptionsBranch>,
}
#[qapi(name = "QCryptoBlockAmendOptionsLUKS")]
#[qapi(since = "5.1")]
pub struct QCryptoBlockAmendOptionsLuks {
    pub state: QCryptoBlockLuksKeyslotState,
    #[qapi(name = "new-secret")]
    pub new_secret: Option<String>,
    #[qapi(name = "old-secret")]
    pub old_secret: Option<String>,
    pub keyslot: Option<i64>,
    #[qapi(name = "iter-time")]
    pub iter_time: Option<i64>,
    pub secret: Option<String>,
}
#[qapi(since = "2.6")]
pub struct QCryptoBlockCreateOptions {
    #[qapi(discriminator)]
    pub format: QCryptoBlockFormat,
    #[qapi(union)]
    pub u: Option<QCryptoBlockCreateOptionsBranch>,
}
#[qapi(name = "QCryptoBlockCreateOptionsLUKS")]
#[qapi(since = "2.6")]
pub struct QCryptoBlockCreateOptionsLuks {
    #[qapi(name = "key-secret")]
    pub key_secret: Option<String>,
    #[qapi(name = "cipher-alg")]
    pub cipher_alg: Option<QCryptoCipherAlgorithm>,
    #[qapi(name = "cipher-mode")]
    pub cipher_mode: Option<QCryptoCipherMode>,
    #[qapi(name = "ivgen-alg")]
    pub ivgen_alg: Option<QCryptoIvGenAlgorithm>,
    #[qapi(name = "ivgen-hash-alg")]
    pub ivgen_hash_alg: Option<QCryptoHashAlgorithm>,
    #[qapi(name = "hash-alg")]
    pub hash_alg: Option<QCryptoHashAlgorithm>,
    #[qapi(name = "iter-time")]
    #[qapi(since = "2.8")]
    pub iter_time: Option<i64>,
    #[qapi(name = "detached-header")]
    #[qapi(since = "9.0")]
    #[qapi(until = "9.1")]
    pub detached_header: Option<bool>,
}
#[qapi(since = "2.7")]
pub struct QCryptoBlockInfo {
    #[qapi(discriminator)]
    pub format: QCryptoBlockFormat,
    #[qapi(union)]
    pub u: Option<QCryptoBlockInfoBranch>,
}
#[qapi(since = "2.7")]
pub struct QCryptoBlockInfoBase {
    pub format: QCryptoBlockFormat,
}
#[qapi(name = "QCryptoBlockInfoLUKS")]
#[qapi(since = "2.7")]
pub struct QCryptoBlockInfoLuks {
    #[qapi(name = "cipher-alg")]
    pub cipher_alg: QCryptoCipherAlgorithm,
    #[qapi(name = "cipher-mode")]
    pub cipher_mode: QCryptoCipherMode,
    #[qapi(name = "ivgen-alg")]
    pub ivgen_alg: QCryptoIvGenAlgorithm,
    #[qapi(name = "ivgen-hash-alg")]
    pub ivgen_hash_alg: Option<QCryptoHashAlgorithm>,
    #[qapi(name = "hash-alg")]
    pub hash_alg: QCryptoHashAlgorithm,
    #[qapi(name = "payload-offset")]
    pub payload_offset: i64,
    #[qapi(name = "master-key-iters")]
    pub master_key_iters: i64,
    pub uuid: String,
    pub slots: Vec<QCryptoBlockInfoLuksSlot>,
    #[qapi(name = "detached-header")]
    #[qapi(since = "9.0")]
    pub detached_header: Option<bool>,
}
#[qapi(name = "QCryptoBlockInfoLUKSSlot")]
#[qapi(since = "2.7")]
pub struct QCryptoBlockInfoLuksSlot {
    pub active: bool,
    pub iters: Option<i64>,
    pub stripes: Option<i64>,
    #[qapi(name = "key-offset")]
    pub key_offset: i64,
}
#[qapi(since = "2.7")]
#[qapi(until = "3.0")]
pub struct QCryptoBlockInfoQCow {}
#[qapi(since = "2.6")]
pub struct QCryptoBlockOpenOptions {
    #[qapi(discriminator)]
    pub format: QCryptoBlockFormat,
    #[qapi(union)]
    pub u: Option<QCryptoBlockOpenOptionsBranch>,
}
#[qapi(since = "2.6")]
pub struct QCryptoBlockOptionsBase {
    pub format: QCryptoBlockFormat,
}
#[qapi(name = "QCryptoBlockOptionsLUKS")]
#[qapi(since = "2.6")]
pub struct QCryptoBlockOptionsLuks {
    #[qapi(name = "key-secret")]
    pub key_secret: Option<String>,
}
#[qapi(since = "2.6")]
pub struct QCryptoBlockOptionsQCow {
    #[qapi(name = "key-secret")]
    pub key_secret: Option<String>,
}
#[qapi(since = "8.0")]
pub struct QCryptodevBackendClient {
    pub queue: u32,
    pub r#type: QCryptodevBackendType,
}
#[qapi(since = "8.0")]
pub struct QCryptodevInfo {
    pub id: String,
    pub service: Vec<QCryptodevBackendServiceType>,
    pub client: Vec<QCryptodevBackendClient>,
}
#[qapi(since = "1.3")]
pub struct QKeyCodeWrapper {
    pub data: QKeyCode,
}
#[qapi(event)]
#[qapi(name = "QUORUM_FAILURE")]
#[qapi(since = "2.0")]
pub struct QuorumFailureEvent {
    pub reference: String,
    #[qapi(name = "sector-num")]
    pub sector_num: i64,
    #[qapi(name = "sectors-count")]
    pub sectors_count: i64,
}
#[qapi(event)]
#[qapi(name = "QUORUM_REPORT_BAD")]
#[qapi(since = "2.0")]
pub struct QuorumReportBadEvent {
    pub error: Option<String>,
    #[qapi(name = "node-name")]
    pub node_name: String,
    #[qapi(name = "sector-num")]
    pub sector_num: i64,
    #[qapi(name = "sectors-count")]
    pub sectors_count: i64,
    #[qapi(since = "2.6")]
    pub r#type: Option<QuorumOpType>,
}
#[qapi(since = "4.0")]
pub struct Qcow2BitmapInfo {
    pub name: String,
    pub granularity: u32,
    pub flags: Vec<Qcow2BitmapInfoFlags>,
}
#[qapi(since = "2.2")]
pub struct Qcow2OverlapCheckFlags {
    pub template: Option<Qcow2OverlapCheckMode>,
    #[qapi(name = "main-header")]
    pub main_header: Option<bool>,
    #[qapi(name = "active-l1")]
    pub active_l1: Option<bool>,
    #[qapi(name = "active-l2")]
    pub active_l2: Option<bool>,
    #[qapi(name = "refcount-table")]
    pub refcount_table: Option<bool>,
    #[qapi(name = "refcount-block")]
    pub refcount_block: Option<bool>,
    #[qapi(name = "snapshot-table")]
    pub snapshot_table: Option<bool>,
    #[qapi(name = "inactive-l1")]
    pub inactive_l1: Option<bool>,
    #[qapi(name = "inactive-l2")]
    pub inactive_l2: Option<bool>,
    #[qapi(name = "bitmap-directory")]
    #[qapi(since = "3.0")]
    pub bitmap_directory: Option<bool>,
}
#[qapi(since = "1.2")]
pub struct QemuTargetInfo {
    pub arch: SysEmuTarget,
}
#[qapi(since = "6.0")]
pub struct QtestProperties {
    pub chardev: String,
    pub log: Option<String>,
}
#[qapi(event)]
#[qapi(name = "RDMA_GID_STATUS_CHANGED")]
#[qapi(since = "4.0")]
#[qapi(until = "9.1")]
pub struct RdmaGidStatusChangedEvent {
    pub netdev: String,
    #[qapi(name = "gid-status")]
    pub gid_status: bool,
    #[qapi(name = "subnet-prefix")]
    pub subnet_prefix: u64,
    #[qapi(name = "interface-id")]
    pub interface_id: u64,
}
#[qapi(event)]
#[qapi(name = "RESET")]
#[qapi(since = "0.12.0")]
pub struct ResetEvent {
    #[qapi(since = "2.10")]
    pub guest: Option<bool>,
    #[qapi(since = "4.0")]
    pub reason: Option<ShutdownCause>,
}
#[qapi(event)]
#[qapi(name = "RESUME")]
#[qapi(since = "0.12.0")]
pub struct ResumeEvent {}
#[qapi(event)]
#[qapi(name = "RTC_CHANGE")]
#[qapi(since = "0.13.0")]
pub struct RtcChangeEvent {
    pub offset: i64,
    #[qapi(name = "qom-path")]
    #[qapi(since = "7.0")]
    pub qom_path: Option<String>,
}
#[qapi(since = "6.1")]
pub struct RbdEncryptionCreateOptions {
    #[qapi(discriminator)]
    pub format: RbdImageEncryptionFormat,
    #[qapi(union)]
    pub u: Option<RbdEncryptionCreateOptionsBranch>,
}
#[qapi(name = "RbdEncryptionCreateOptionsLUKS")]
#[qapi(since = "6.1")]
pub struct RbdEncryptionCreateOptionsLuks {
    #[qapi(name = "key-secret")]
    pub key_secret: String,
    #[qapi(name = "cipher-alg")]
    pub cipher_alg: Option<QCryptoCipherAlgorithm>,
}
#[qapi(name = "RbdEncryptionCreateOptionsLUKS2")]
#[qapi(since = "6.1")]
pub struct RbdEncryptionCreateOptionsLuks2 {
    #[qapi(name = "key-secret")]
    pub key_secret: String,
    #[qapi(name = "cipher-alg")]
    pub cipher_alg: Option<QCryptoCipherAlgorithm>,
}
#[qapi(name = "RbdEncryptionCreateOptionsLUKSBase")]
#[qapi(since = "6.1")]
pub struct RbdEncryptionCreateOptionsLuksBase {
    #[qapi(name = "key-secret")]
    pub key_secret: String,
    #[qapi(name = "cipher-alg")]
    pub cipher_alg: Option<QCryptoCipherAlgorithm>,
}
#[qapi(since = "6.1")]
pub struct RbdEncryptionOptions {
    #[qapi(discriminator)]
    pub format: RbdImageEncryptionFormat,
    #[qapi(union)]
    pub u: Option<RbdEncryptionOptionsBranch>,
    #[qapi(since = "8.0")]
    pub parent: Option<Box<RbdEncryptionOptions>>,
}
#[qapi(name = "RbdEncryptionOptionsLUKS")]
#[qapi(since = "6.1")]
pub struct RbdEncryptionOptionsLuks {
    #[qapi(name = "key-secret")]
    pub key_secret: String,
}
#[qapi(name = "RbdEncryptionOptionsLUKS2")]
#[qapi(since = "6.1")]
pub struct RbdEncryptionOptionsLuks2 {
    #[qapi(name = "key-secret")]
    pub key_secret: String,
}
#[qapi(name = "RbdEncryptionOptionsLUKSAny")]
#[qapi(since = "8.0")]
pub struct RbdEncryptionOptionsLuksAny {
    #[qapi(name = "key-secret")]
    pub key_secret: String,
}
#[qapi(name = "RbdEncryptionOptionsLUKSBase")]
#[qapi(since = "6.1")]
pub struct RbdEncryptionOptionsLuksBase {
    #[qapi(name = "key-secret")]
    pub key_secret: String,
}
#[qapi(since = "6.0")]
pub struct RemoteObjectProperties {
    pub fd: String,
    pub devid: String,
}
#[qapi(since = "5.2")]
pub struct ReplayInfo {
    pub mode: ReplayMode,
    pub filename: Option<String>,
    pub icount: i64,
}
#[qapi(since = "2.9")]
pub struct ReplicationStatus {
    pub error: bool,
    pub desc: Option<String>,
}
#[qapi(since = "1.3")]
pub struct RngEgdProperties {
    #[qapi(feature = "deprecated")]
    pub opened: Option<bool>,
    pub chardev: String,
}
#[qapi(since = "1.3")]
pub struct RngProperties {
    #[qapi(feature = "deprecated")]
    pub opened: Option<bool>,
}
#[qapi(since = "1.3")]
pub struct RngRandomProperties {
    #[qapi(feature = "deprecated")]
    pub opened: Option<bool>,
    pub filename: Option<String>,
}
#[qapi(since = "2.4")]
pub struct RockerOfDpaFlow {
    pub cookie: u64,
    pub hits: u64,
    pub key: RockerOfDpaFlowKey,
    pub mask: RockerOfDpaFlowMask,
    pub action: RockerOfDpaFlowAction,
}
#[qapi(since = "2.4")]
pub struct RockerOfDpaFlowAction {
    #[qapi(name = "goto-tbl")]
    pub goto_tbl: Option<u32>,
    #[qapi(name = "group-id")]
    pub group_id: Option<u32>,
    #[qapi(name = "tunnel-lport")]
    pub tunnel_lport: Option<u32>,
    #[qapi(name = "vlan-id")]
    pub vlan_id: Option<u16>,
    #[qapi(name = "new-vlan-id")]
    pub new_vlan_id: Option<u16>,
    #[qapi(name = "out-pport")]
    pub out_pport: Option<u32>,
}
#[qapi(since = "2.4")]
pub struct RockerOfDpaFlowKey {
    pub priority: u32,
    #[qapi(name = "tbl-id")]
    pub tbl_id: u32,
    #[qapi(name = "in-pport")]
    pub in_pport: Option<u32>,
    #[qapi(name = "tunnel-id")]
    pub tunnel_id: Option<u32>,
    #[qapi(name = "vlan-id")]
    pub vlan_id: Option<u16>,
    #[qapi(name = "eth-type")]
    pub eth_type: Option<u16>,
    #[qapi(name = "eth-src")]
    pub eth_src: Option<String>,
    #[qapi(name = "eth-dst")]
    pub eth_dst: Option<String>,
    #[qapi(name = "ip-proto")]
    pub ip_proto: Option<u8>,
    #[qapi(name = "ip-tos")]
    pub ip_tos: Option<u8>,
    #[qapi(name = "ip-dst")]
    pub ip_dst: Option<String>,
}
#[qapi(since = "2.4")]
pub struct RockerOfDpaFlowMask {
    #[qapi(name = "in-pport")]
    pub in_pport: Option<u32>,
    #[qapi(name = "tunnel-id")]
    pub tunnel_id: Option<u32>,
    #[qapi(name = "vlan-id")]
    pub vlan_id: Option<u16>,
    #[qapi(name = "eth-src")]
    pub eth_src: Option<String>,
    #[qapi(name = "eth-dst")]
    pub eth_dst: Option<String>,
    #[qapi(name = "ip-proto")]
    pub ip_proto: Option<u8>,
    #[qapi(name = "ip-tos")]
    pub ip_tos: Option<u8>,
}
#[qapi(since = "2.4")]
pub struct RockerOfDpaGroup {
    pub id: u32,
    pub r#type: u8,
    #[qapi(name = "vlan-id")]
    pub vlan_id: Option<u16>,
    pub pport: Option<u32>,
    pub index: Option<u32>,
    #[qapi(name = "out-pport")]
    pub out_pport: Option<u32>,
    #[qapi(name = "group-id")]
    pub group_id: Option<u32>,
    #[qapi(name = "set-vlan-id")]
    pub set_vlan_id: Option<u16>,
    #[qapi(name = "pop-vlan")]
    pub pop_vlan: Option<u8>,
    #[qapi(name = "group-ids")]
    pub group_ids: Option<Vec<u32>>,
    #[qapi(name = "set-eth-src")]
    pub set_eth_src: Option<String>,
    #[qapi(name = "set-eth-dst")]
    pub set_eth_dst: Option<String>,
    #[qapi(name = "ttl-check")]
    pub ttl_check: Option<u8>,
}
#[qapi(since = "2.4")]
pub struct RockerPort {
    pub name: String,
    pub enabled: bool,
    #[qapi(name = "link-up")]
    pub link_up: bool,
    pub speed: u32,
    pub duplex: RockerPortDuplex,
    pub autoneg: RockerPortAutoneg,
}
#[qapi(since = "2.4")]
pub struct RockerSwitch {
    pub name: String,
    pub id: u64,
    pub ports: u32,
}
#[qapi(since = "1.6")]
pub struct RxFilterInfo {
    pub name: String,
    pub promiscuous: bool,
    pub multicast: RxState,
    pub unicast: RxState,
    pub vlan: RxState,
    #[qapi(name = "broadcast-allowed")]
    pub broadcast_allowed: bool,
    #[qapi(name = "multicast-overflow")]
    pub multicast_overflow: bool,
    #[qapi(name = "unicast-overflow")]
    pub unicast_overflow: bool,
    #[qapi(name = "main-mac")]
    pub main_mac: String,
    #[qapi(name = "vlan-table")]
    pub vlan_table: Vec<i64>,
    #[qapi(name = "unicast-table")]
    pub unicast_table: Vec<String>,
    #[qapi(name = "multicast-table")]
    pub multicast_table: Vec<String>,
}
#[qapi(event)]
#[qapi(name = "SCLP_CPI_INFO_AVAILABLE")]
#[qapi(feature = "unstable")]
#[qapi(since = "10.2")]
pub struct SclpCpiInfoAvailableEvent {}
#[qapi(name = "SGXEPCSection")]
#[qapi(since = "7.0")]
#[qapi(until = "10.1")]
pub struct SgxepcSection {
    pub node: i64,
    pub size: u64,
}
#[qapi(name = "SGXInfo")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "6.2")]
#[qapi(until = "10.1")]
pub struct SgxInfo {
    pub sgx: bool,
    pub sgx1: bool,
    pub sgx2: bool,
    pub flc: bool,
    #[qapi(name = "section-size")]
    #[qapi(until = "8.0")]
    pub section_size: Option<u64>,
    #[qapi(since = "7.0")]
    pub sections: Option<Vec<SgxepcSection>>,
}
#[qapi(event)]
#[qapi(name = "SHUTDOWN")]
#[qapi(since = "0.12.0")]
pub struct ShutdownEvent {
    #[qapi(since = "2.10")]
    pub guest: Option<bool>,
    #[qapi(since = "4.0")]
    pub reason: Option<ShutdownCause>,
}
#[qapi(name = "SMPConfiguration")]
#[qapi(since = "6.1")]
pub struct SmpConfiguration {
    pub cpus: Option<i64>,
    pub sockets: Option<i64>,
    pub dies: Option<i64>,
    pub cores: Option<i64>,
    pub threads: Option<i64>,
    pub maxcpus: Option<i64>,
    #[qapi(since = "7.0")]
    pub clusters: Option<i64>,
    #[qapi(since = "8.2")]
    pub drawers: Option<i64>,
    #[qapi(since = "8.2")]
    pub books: Option<i64>,
    #[qapi(since = "9.1")]
    pub modules: Option<i64>,
}
#[qapi(event)]
#[qapi(name = "SPICE_CONNECTED")]
#[qapi(since = "0.14.0")]
pub struct SpiceConnectedEvent {
    pub server: SpiceBasicInfo,
    pub client: SpiceBasicInfo,
}
#[qapi(event)]
#[qapi(name = "SPICE_DISCONNECTED")]
#[qapi(since = "0.14.0")]
pub struct SpiceDisconnectedEvent {
    pub server: SpiceBasicInfo,
    pub client: SpiceBasicInfo,
}
#[qapi(event)]
#[qapi(name = "SPICE_INITIALIZED")]
#[qapi(since = "0.14.0")]
pub struct SpiceInitializedEvent {
    pub server: SpiceServerInfo,
    pub client: SpiceChannel,
}
#[qapi(event)]
#[qapi(name = "SPICE_MIGRATE_COMPLETED")]
#[qapi(since = "1.3")]
pub struct SpiceMigrateCompletedEvent {}
#[qapi(event)]
#[qapi(name = "STOP")]
#[qapi(since = "0.12.0")]
pub struct StopEvent {}
#[qapi(event)]
#[qapi(name = "SUSPEND")]
#[qapi(since = "1.1")]
pub struct SuspendEvent {}
#[qapi(event)]
#[qapi(name = "SUSPEND_DISK")]
#[qapi(since = "1.2")]
pub struct SuspendDiskEvent {}
#[qapi(since = "2.5")]
pub struct SchemaInfo {
    pub name: String,
    #[qapi(name = "meta-type")]
    #[qapi(discriminator)]
    pub meta_type: SchemaMetaType,
    #[qapi(union)]
    pub u: Option<SchemaInfoBranch>,
    #[qapi(since = "5.0")]
    pub features: Option<Vec<String>>,
}
#[qapi(since = "2.5")]
pub struct SchemaInfoAlternate {
    pub members: Vec<SchemaInfoAlternateMember>,
}
#[qapi(since = "2.5")]
pub struct SchemaInfoAlternateMember {
    pub r#type: String,
}
#[qapi(since = "2.5")]
pub struct SchemaInfoArray {
    #[qapi(name = "element-type")]
    pub element_type: String,
}
#[qapi(since = "2.5")]
#[qapi(until = "2.6")]
pub struct SchemaInfoBase {
    pub name: String,
    #[qapi(name = "meta-type")]
    pub meta_type: SchemaMetaType,
}
#[qapi(since = "2.5")]
pub struct SchemaInfoBuiltin {
    #[qapi(name = "json-type")]
    pub json_type: JsonType,
}
#[qapi(since = "2.5")]
pub struct SchemaInfoCommand {
    #[qapi(name = "arg-type")]
    pub arg_type: String,
    #[qapi(name = "ret-type")]
    pub ret_type: String,
    #[qapi(name = "allow-oob")]
    #[qapi(since = "2.12")]
    pub allow_oob: Option<bool>,
    #[qapi(since = "4.2")]
    #[qapi(until = "5.0")]
    pub features: Option<Vec<String>>,
}
#[qapi(since = "2.5")]
pub struct SchemaInfoEnum {
    pub values: Vec<String>,
    #[qapi(since = "6.2")]
    pub members: Option<Vec<SchemaInfoEnumMember>>,
}
#[qapi(since = "6.2")]
pub struct SchemaInfoEnumMember {
    pub name: String,
    pub features: Option<Vec<String>>,
}
#[qapi(since = "2.5")]
pub struct SchemaInfoEvent {
    #[qapi(name = "arg-type")]
    pub arg_type: String,
}
#[qapi(since = "2.5")]
pub struct SchemaInfoObject {
    pub members: Vec<SchemaInfoObjectMember>,
    pub tag: Option<String>,
    pub variants: Option<Vec<SchemaInfoObjectVariant>>,
    #[qapi(since = "4.1")]
    #[qapi(until = "5.0")]
    pub features: Option<Vec<String>>,
}
#[qapi(since = "2.5")]
pub struct SchemaInfoObjectMember {
    pub name: String,
    pub r#type: String,
    pub default: Option<serde_json::Value>,
    #[qapi(since = "5.0")]
    pub features: Option<Vec<String>>,
}
#[qapi(since = "2.5")]
pub struct SchemaInfoObjectVariant {
    pub case: String,
    pub r#type: String,
}
#[qapi(since = "2.6")]
pub struct SecretCommonProperties {
    #[qapi(feature = "deprecated")]
    #[qapi(until = "9.2")]
    pub loaded: Option<bool>,
    pub format: Option<QCryptoSecretFormat>,
    pub keyid: Option<String>,
    pub iv: Option<String>,
}
#[qapi(since = "5.1")]
pub struct SecretKeyringProperties {
    #[qapi(feature = "deprecated")]
    #[qapi(until = "9.2")]
    pub loaded: Option<bool>,
    pub format: Option<QCryptoSecretFormat>,
    pub keyid: Option<String>,
    pub iv: Option<String>,
    pub serial: i32,
}
#[qapi(since = "2.6")]
pub struct SecretProperties {
    #[qapi(feature = "deprecated")]
    #[qapi(until = "9.2")]
    pub loaded: Option<bool>,
    pub format: Option<QCryptoSecretFormat>,
    pub keyid: Option<String>,
    pub iv: Option<String>,
    pub data: Option<String>,
    pub file: Option<String>,
}
#[qapi(since = "7.0")]
pub struct SetPasswordOptions {
    #[qapi(discriminator)]
    pub protocol: DisplayProtocol,
    pub password: String,
    pub connected: Option<SetPasswordAction>,
    #[qapi(union)]
    pub u: Option<SetPasswordOptionsBranch>,
}
#[qapi(since = "7.0")]
pub struct SetPasswordOptionsVnc {
    pub display: Option<String>,
}
#[qapi(condition = "defined(TARGET_I386)")]
#[qapi(since = "6.1")]
pub struct SevAttestationReport {
    pub data: String,
}
#[qapi(since = "2.12")]
pub struct SevCapability {
    pub pdh: String,
    #[qapi(name = "cert-chain")]
    pub cert_chain: String,
    pub cbitpos: i64,
    #[qapi(name = "reduced-phys-bits")]
    pub reduced_phys_bits: i64,
    #[qapi(name = "cpu0-id")]
    #[qapi(since = "7.1")]
    pub cpu0_id: Option<String>,
}
#[qapi(since = "9.1")]
pub struct SevCommonProperties {
    #[qapi(name = "sev-device")]
    pub sev_device: Option<String>,
    pub cbitpos: Option<u32>,
    #[qapi(name = "reduced-phys-bits")]
    pub reduced_phys_bits: u32,
    #[qapi(name = "kernel-hashes")]
    pub kernel_hashes: Option<bool>,
}
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "2.12")]
pub struct SevGuestInfo {
    pub policy: u32,
    pub handle: u32,
}
#[qapi(since = "2.12")]
pub struct SevGuestProperties {
    #[qapi(name = "sev-device")]
    pub sev_device: Option<String>,
    #[qapi(name = "dh-cert-file")]
    pub dh_cert_file: Option<String>,
    #[qapi(name = "session-file")]
    pub session_file: Option<String>,
    pub policy: Option<u32>,
    pub handle: Option<u32>,
    pub cbitpos: Option<u32>,
    #[qapi(name = "reduced-phys-bits")]
    pub reduced_phys_bits: u32,
    #[qapi(name = "kernel-hashes")]
    #[qapi(since = "6.2")]
    pub kernel_hashes: Option<bool>,
    #[qapi(name = "legacy-vm-type")]
    #[qapi(since = "9.1")]
    pub legacy_vm_type: Option<OnOffAuto>,
}
#[qapi(since = "2.12")]
pub struct SevInfo {
    pub enabled: bool,
    #[qapi(name = "api-major")]
    pub api_major: u8,
    #[qapi(name = "api-minor")]
    pub api_minor: u8,
    #[qapi(name = "build-id")]
    pub build_id: u8,
    #[qapi(until = "9.1")]
    pub policy: Option<u32>,
    pub state: SevState,
    #[qapi(until = "9.1")]
    pub handle: Option<u32>,
    #[qapi(name = "sev-type")]
    #[qapi(discriminator)]
    #[qapi(since = "9.1")]
    pub sev_type: Option<SevGuestType>,
    #[qapi(union)]
    #[qapi(since = "9.1")]
    pub u: Option<SevInfoBranch>,
}
#[qapi(since = "2.12")]
pub struct SevLaunchMeasureInfo {
    pub data: String,
}
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "9.1")]
pub struct SevSnpGuestInfo {
    #[qapi(name = "snp-policy")]
    pub snp_policy: u64,
}
#[qapi(since = "9.1")]
pub struct SevSnpGuestProperties {
    #[qapi(name = "sev-device")]
    pub sev_device: Option<String>,
    pub cbitpos: Option<u32>,
    #[qapi(name = "reduced-phys-bits")]
    pub reduced_phys_bits: u32,
    #[qapi(name = "kernel-hashes")]
    pub kernel_hashes: Option<bool>,
    pub policy: Option<u64>,
    #[qapi(name = "guest-visible-workarounds")]
    pub guest_visible_workarounds: Option<String>,
    #[qapi(name = "id-block")]
    pub id_block: Option<String>,
    #[qapi(name = "id-auth")]
    pub id_auth: Option<String>,
    #[qapi(name = "author-key-enabled")]
    pub author_key_enabled: Option<bool>,
    #[qapi(name = "host-data")]
    pub host_data: Option<String>,
    #[qapi(name = "vcek-disabled")]
    pub vcek_disabled: Option<bool>,
}
#[qapi(name = "SgxEPC")]
#[qapi(since = "6.2")]
pub struct SgxEpc {
    pub memdev: String,
    #[qapi(since = "7.0")]
    pub node: Option<i64>,
}
#[qapi(name = "SgxEPCDeviceInfo")]
#[qapi(since = "6.2")]
pub struct SgxEpcDeviceInfo {
    pub id: Option<String>,
    pub memaddr: u64,
    pub size: u64,
    pub memdev: String,
    #[qapi(since = "7.0")]
    pub node: Option<i64>,
}
#[qapi(name = "SgxEPCDeviceInfoWrapper")]
#[qapi(since = "6.2")]
pub struct SgxEpcDeviceInfoWrapper {
    pub data: SgxEpcDeviceInfo,
}
#[qapi(name = "SgxEPCProperties")]
#[qapi(since = "6.2")]
pub struct SgxEpcProperties {
    #[qapi(name = "sgx-epc")]
    pub sgx_epc: Vec<SgxEpc>,
}
#[qapi(since = "7.0")]
pub struct SgxEpcSection {
    pub node: i64,
    pub size: u64,
}
#[qapi(since = "2.12")]
#[qapi(until = "6.1")]
pub struct SheepdogRedundancy {
    #[qapi(discriminator)]
    pub r#type: SheepdogRedundancyType,
    #[qapi(union)]
    pub u: Option<SheepdogRedundancyBranch>,
}
#[qapi(since = "2.12")]
#[qapi(until = "6.1")]
pub struct SheepdogRedundancyErasureCoded {
    #[qapi(name = "data-strips")]
    pub data_strips: i64,
    #[qapi(name = "parity-strips")]
    pub parity_strips: i64,
}
#[qapi(since = "2.12")]
#[qapi(until = "6.1")]
pub struct SheepdogRedundancyFull {
    pub copies: i64,
}
#[qapi(since = "9.2")]
pub struct SmpCacheProperties {
    pub cache: CacheLevelAndType,
    pub topology: CpuTopologyLevel,
}
#[qapi(since = "9.2")]
pub struct SmpCachePropertiesWrapper {
    pub caches: Vec<SmpCacheProperties>,
}
#[qapi(since = "1.3")]
pub struct SnapshotInfo {
    pub id: String,
    pub name: String,
    #[qapi(name = "vm-state-size")]
    pub vm_state_size: i64,
    #[qapi(name = "date-sec")]
    pub date_sec: i64,
    #[qapi(name = "date-nsec")]
    pub date_nsec: i64,
    #[qapi(name = "vm-clock-sec")]
    pub vm_clock_sec: i64,
    #[qapi(name = "vm-clock-nsec")]
    pub vm_clock_nsec: i64,
    #[qapi(since = "5.2")]
    pub icount: Option<i64>,
}
#[qapi(since = "1.6")]
pub struct SocketAddress {
    #[qapi(discriminator)]
    pub r#type: SocketAddressKind,
    #[qapi(union)]
    pub u: Option<SocketAddressBranch>,
}
#[qapi(since = "2.9")]
#[qapi(until = "2.10")]
pub struct SocketAddressFlat {
    #[qapi(discriminator)]
    pub r#type: SocketAddressFlatType,
    #[qapi(union)]
    pub u: Option<SocketAddressFlatBranch>,
}
#[qapi(since = "2.10")]
pub struct SocketAddressLegacy {
    #[qapi(discriminator)]
    pub r#type: SocketAddressLegacyKind,
    #[qapi(union)]
    pub u: Option<SocketAddressLegacyBranch>,
}
#[qapi(since = "2.1")]
pub struct SpiceBasicInfo {
    pub host: String,
    pub port: String,
    pub family: NetworkAddressFamily,
}
#[qapi(since = "0.14.0")]
pub struct SpiceChannel {
    pub host: String,
    pub family: String,
    pub port: String,
    #[qapi(name = "connection-id")]
    pub connection_id: i64,
    #[qapi(name = "channel-type")]
    pub channel_type: i64,
    #[qapi(name = "channel-id")]
    pub channel_id: i64,
    pub tls: bool,
}
#[qapi(since = "0.14.0")]
pub struct SpiceInfo {
    pub enabled: bool,
    pub host: Option<String>,
    pub port: Option<i64>,
    #[qapi(name = "tls-port")]
    pub tls_port: Option<i64>,
    pub auth: Option<String>,
    #[qapi(name = "compiled-version")]
    pub compiled_version: Option<String>,
    #[qapi(name = "mouse-mode")]
    pub mouse_mode: SpiceQueryMouseMode,
    pub channels: Option<Vec<SpiceChannel>>,
    #[qapi(since = "1.3")]
    pub migrated: Option<bool>,
}
#[qapi(since = "2.1")]
pub struct SpiceServerInfo {
    pub host: String,
    pub port: String,
    pub family: NetworkAddressFamily,
    pub auth: Option<String>,
}
#[qapi(since = "2.12")]
pub struct SshHostKeyCheck {
    #[qapi(discriminator)]
    pub mode: SshHostKeyCheckMode,
    #[qapi(union)]
    pub u: Option<SshHostKeyCheckBranch>,
}
#[qapi(since = "2.12")]
#[qapi(until = "3.0")]
pub struct SshHostKeyDummy {}
#[qapi(since = "2.12")]
pub struct SshHostKeyHash {
    pub r#type: SshHostKeyCheckHashType,
    pub hash: String,
}
#[qapi(since = "7.1")]
pub struct Stats {
    pub name: String,
    pub value: StatsValue,
}
#[qapi(since = "7.1")]
pub struct StatsFilter {
    #[qapi(discriminator)]
    pub target: StatsTarget,
    pub providers: Option<Vec<StatsRequest>>,
    #[qapi(union)]
    pub u: Option<StatsFilterBranch>,
}
#[qapi(since = "7.1")]
pub struct StatsRequest {
    pub provider: StatsProvider,
    pub names: Option<Vec<String>>,
}
#[qapi(since = "7.1")]
pub struct StatsResult {
    pub provider: StatsProvider,
    #[qapi(name = "qom-path")]
    pub qom_path: Option<String>,
    pub stats: Vec<Stats>,
}
#[qapi(since = "7.1")]
pub struct StatsSchema {
    pub provider: StatsProvider,
    pub target: StatsTarget,
    pub stats: Vec<StatsSchemaValue>,
}
#[qapi(since = "7.1")]
pub struct StatsSchemaValue {
    pub name: String,
    pub r#type: StatsType,
    pub unit: Option<StatsUnit>,
    pub base: Option<i8>,
    pub exponent: i16,
    #[qapi(name = "bucket-size")]
    pub bucket_size: Option<u32>,
}
#[qapi(name = "StatsVCPUFilter")]
#[qapi(since = "7.1")]
pub struct StatsVcpuFilter {
    pub vcpus: Option<Vec<String>>,
}
#[qapi(since = "0.14.0")]
pub struct StatusInfo {
    pub running: bool,
    #[qapi(until = "9.0")]
    pub singlestep: Option<bool>,
    pub status: RunState,
}
#[qapi(since = "1.3")]
#[qapi(until = "9.0")]
pub struct StringWrapper {
    pub data: String,
}
#[qapi(name = "TPMEmulatorOptions")]
#[qapi(since = "2.11")]
pub struct TpmEmulatorOptions {
    pub chardev: String,
}
#[qapi(name = "TPMEmulatorOptionsWrapper")]
#[qapi(condition = "CONFIG_TPM")]
#[qapi(since = "2.11")]
pub struct TpmEmulatorOptionsWrapper {
    pub data: TpmEmulatorOptions,
}
#[qapi(name = "TPMInfo")]
#[qapi(since = "1.5")]
pub struct TpmInfo {
    pub id: String,
    pub model: TpmModel,
    pub options: TpmTypeOptions,
}
#[qapi(name = "TPMPassthroughOptions")]
#[qapi(since = "1.5")]
pub struct TpmPassthroughOptions {
    pub path: Option<String>,
    #[qapi(name = "cancel-path")]
    pub cancel_path: Option<String>,
}
#[qapi(name = "TPMPassthroughOptionsWrapper")]
#[qapi(condition = "CONFIG_TPM")]
#[qapi(since = "1.5")]
pub struct TpmPassthroughOptionsWrapper {
    pub data: TpmPassthroughOptions,
}
#[qapi(since = "1.2.0")]
#[qapi(until = "10.1")]
pub struct TargetInfo {
    pub arch: TargetType,
}
#[qapi(since = "10.1")]
pub struct TdxGuestProperties {
    pub attributes: Option<u64>,
    #[qapi(name = "sept-ve-disable")]
    pub sept_ve_disable: Option<bool>,
    pub mrconfigid: Option<String>,
    pub mrowner: Option<String>,
    pub mrownerconfig: Option<String>,
    #[qapi(name = "quote-generation-socket")]
    pub quote_generation_socket: Option<SocketAddress>,
}
#[qapi(since = "7.2")]
pub struct ThreadContextProperties {
    #[qapi(name = "cpu-affinity")]
    pub cpu_affinity: Option<Vec<u16>>,
    #[qapi(name = "node-affinity")]
    pub node_affinity: Option<Vec<u16>>,
}
#[qapi(since = "2.11")]
pub struct ThrottleGroupProperties {
    pub limits: Option<ThrottleLimits>,
    #[qapi(name = "x-iops-total")]
    pub x_iops_total: Option<i64>,
    #[qapi(name = "x-iops-total-max")]
    pub x_iops_total_max: Option<i64>,
    #[qapi(name = "x-iops-total-max-length")]
    pub x_iops_total_max_length: Option<i64>,
    #[qapi(name = "x-iops-read")]
    pub x_iops_read: Option<i64>,
    #[qapi(name = "x-iops-read-max")]
    pub x_iops_read_max: Option<i64>,
    #[qapi(name = "x-iops-read-max-length")]
    pub x_iops_read_max_length: Option<i64>,
    #[qapi(name = "x-iops-write")]
    pub x_iops_write: Option<i64>,
    #[qapi(name = "x-iops-write-max")]
    pub x_iops_write_max: Option<i64>,
    #[qapi(name = "x-iops-write-max-length")]
    pub x_iops_write_max_length: Option<i64>,
    #[qapi(name = "x-bps-total")]
    pub x_bps_total: Option<i64>,
    #[qapi(name = "x-bps-total-max")]
    pub x_bps_total_max: Option<i64>,
    #[qapi(name = "x-bps-total-max-length")]
    pub x_bps_total_max_length: Option<i64>,
    #[qapi(name = "x-bps-read")]
    pub x_bps_read: Option<i64>,
    #[qapi(name = "x-bps-read-max")]
    pub x_bps_read_max: Option<i64>,
    #[qapi(name = "x-bps-read-max-length")]
    pub x_bps_read_max_length: Option<i64>,
    #[qapi(name = "x-bps-write")]
    pub x_bps_write: Option<i64>,
    #[qapi(name = "x-bps-write-max")]
    pub x_bps_write_max: Option<i64>,
    #[qapi(name = "x-bps-write-max-length")]
    pub x_bps_write_max_length: Option<i64>,
    #[qapi(name = "x-iops-size")]
    pub x_iops_size: Option<i64>,
}
#[qapi(since = "2.11")]
pub struct ThrottleLimits {
    #[qapi(name = "iops-total")]
    pub iops_total: Option<i64>,
    #[qapi(name = "iops-total-max")]
    pub iops_total_max: Option<i64>,
    #[qapi(name = "iops-total-max-length")]
    pub iops_total_max_length: Option<i64>,
    #[qapi(name = "iops-read")]
    pub iops_read: Option<i64>,
    #[qapi(name = "iops-read-max")]
    pub iops_read_max: Option<i64>,
    #[qapi(name = "iops-read-max-length")]
    pub iops_read_max_length: Option<i64>,
    #[qapi(name = "iops-write")]
    pub iops_write: Option<i64>,
    #[qapi(name = "iops-write-max")]
    pub iops_write_max: Option<i64>,
    #[qapi(name = "iops-write-max-length")]
    pub iops_write_max_length: Option<i64>,
    #[qapi(name = "bps-total")]
    pub bps_total: Option<i64>,
    #[qapi(name = "bps-total-max")]
    pub bps_total_max: Option<i64>,
    #[qapi(name = "bps-total-max-length")]
    pub bps_total_max_length: Option<i64>,
    #[qapi(name = "bps-read")]
    pub bps_read: Option<i64>,
    #[qapi(name = "bps-read-max")]
    pub bps_read_max: Option<i64>,
    #[qapi(name = "bps-read-max-length")]
    pub bps_read_max_length: Option<i64>,
    #[qapi(name = "bps-write")]
    pub bps_write: Option<i64>,
    #[qapi(name = "bps-write-max")]
    pub bps_write_max: Option<i64>,
    #[qapi(name = "bps-write-max-length")]
    pub bps_write_max_length: Option<i64>,
    #[qapi(name = "iops-size")]
    pub iops_size: Option<i64>,
}
#[qapi(since = "2.5")]
pub struct TlsCredsAnonProperties {
    #[qapi(name = "verify-peer")]
    pub verify_peer: Option<bool>,
    pub dir: Option<String>,
    pub endpoint: Option<QCryptoTlsCredsEndpoint>,
    pub priority: Option<String>,
    #[qapi(feature = "deprecated")]
    #[qapi(until = "9.2")]
    pub loaded: Option<bool>,
}
#[qapi(since = "2.5")]
pub struct TlsCredsProperties {
    #[qapi(name = "verify-peer")]
    pub verify_peer: Option<bool>,
    pub dir: Option<String>,
    pub endpoint: Option<QCryptoTlsCredsEndpoint>,
    pub priority: Option<String>,
}
#[qapi(since = "3.0")]
pub struct TlsCredsPskProperties {
    #[qapi(name = "verify-peer")]
    pub verify_peer: Option<bool>,
    pub dir: Option<String>,
    pub endpoint: Option<QCryptoTlsCredsEndpoint>,
    pub priority: Option<String>,
    #[qapi(feature = "deprecated")]
    #[qapi(until = "9.2")]
    pub loaded: Option<bool>,
    pub username: Option<String>,
}
#[qapi(since = "2.5")]
pub struct TlsCredsX509Properties {
    #[qapi(name = "verify-peer")]
    pub verify_peer: Option<bool>,
    pub dir: Option<String>,
    pub endpoint: Option<QCryptoTlsCredsEndpoint>,
    pub priority: Option<String>,
    #[qapi(feature = "deprecated")]
    #[qapi(until = "9.2")]
    pub loaded: Option<bool>,
    #[qapi(name = "sanity-check")]
    pub sanity_check: Option<bool>,
    pub passwordid: Option<String>,
}
#[qapi(since = "1.5")]
pub struct TpmTypeOptions {
    #[qapi(discriminator)]
    pub r#type: TpmTypeOptionsKind,
    #[qapi(union)]
    pub u: Option<TpmTypeOptionsBranch>,
}
#[qapi(since = "2.2")]
pub struct TraceEventInfo {
    pub name: String,
    pub state: TraceEventState,
    #[qapi(since = "2.7")]
    #[qapi(until = "9.1")]
    pub vcpu: Option<bool>,
}
#[qapi(since = "2.0")]
pub struct TransactionAction {
    #[qapi(discriminator)]
    pub r#type: TransactionActionKind,
    #[qapi(union)]
    pub u: Option<TransactionActionBranch>,
}
#[qapi(since = "2.5")]
pub struct TransactionProperties {
    #[qapi(name = "completion-mode")]
    pub completion_mode: Option<ActionCompletionMode>,
}
#[qapi(event)]
#[qapi(name = "UNPLUG_PRIMARY")]
#[qapi(since = "4.2")]
pub struct UnplugPrimaryEvent {
    #[qapi(name = "device-id")]
    pub device_id: String,
}
#[qapi(since = "10.0")]
pub struct UefiVarStore {
    pub version: i64,
    pub variables: Vec<UefiVariable>,
}
#[qapi(since = "10.0")]
pub struct UefiVariable {
    pub guid: String,
    pub name: String,
    pub attr: i64,
    pub data: String,
    pub time: Option<String>,
    pub digest: Option<String>,
}
#[qapi(since = "1.3")]
pub struct UnixSocketAddress {
    pub path: String,
    #[qapi(since = "5.1")]
    pub tight: Option<bool>,
    #[qapi(since = "5.1")]
    pub r#abstract: Option<bool>,
}
#[qapi(since = "1.3")]
pub struct UnixSocketAddressWrapper {
    pub data: UnixSocketAddress,
}
#[qapi(since = "0.14.0")]
pub struct UuidInfo {
    #[qapi(name = "UUID")]
    pub uuid: String,
}
#[qapi(event)]
#[qapi(name = "VFIO_MIGRATION")]
#[qapi(since = "9.1")]
pub struct VfioMigrationEvent {
    #[qapi(name = "device-id")]
    pub device_id: String,
    #[qapi(name = "qom-path")]
    pub qom_path: String,
    #[qapi(name = "device-state")]
    pub device_state: VfioMigrationState,
}
#[qapi(event)]
#[qapi(name = "VFU_CLIENT_HANGUP")]
#[qapi(since = "7.1")]
pub struct VfuClientHangupEvent {
    #[qapi(name = "vfu-id")]
    pub vfu_id: String,
    #[qapi(name = "vfu-qom-path")]
    pub vfu_qom_path: String,
    #[qapi(name = "dev-id")]
    pub dev_id: String,
    #[qapi(name = "dev-qom-path")]
    pub dev_qom_path: String,
}
#[qapi(event)]
#[qapi(name = "VNC_CONNECTED")]
#[qapi(since = "0.13.0")]
pub struct VncConnectedEvent {
    pub server: VncServerInfo,
    pub client: VncBasicInfo,
}
#[qapi(event)]
#[qapi(name = "VNC_DISCONNECTED")]
#[qapi(since = "0.13.0")]
pub struct VncDisconnectedEvent {
    pub server: VncServerInfo,
    pub client: VncClientInfo,
}
#[qapi(event)]
#[qapi(name = "VNC_INITIALIZED")]
#[qapi(since = "0.13.0")]
pub struct VncInitializedEvent {
    pub server: VncServerInfo,
    pub client: VncClientInfo,
}
#[qapi(event)]
#[qapi(name = "VSERPORT_CHANGE")]
#[qapi(since = "2.1")]
pub struct VserportChangeEvent {
    pub id: String,
    pub open: bool,
}
#[qapi(since = "1.1")]
pub struct VersionInfo {
    pub qemu: VersionInfoQemu,
    pub package: String,
}
#[qapi(since = "0.14.0")]
#[qapi(until = "2.4")]
pub struct VersionInfoQemu {
    pub major: i64,
    pub minor: i64,
    pub micro: i64,
}
#[qapi(since = "2.4")]
pub struct VersionTriple {
    pub major: i64,
    pub minor: i64,
    pub micro: i64,
}
#[qapi(since = "5.2")]
pub struct VfioStats {
    pub transferred: i64,
}
#[qapi(since = "7.1")]
pub struct VfioUserServerProperties {
    pub socket: SocketAddress,
    pub device: String,
}
#[qapi(since = "7.2")]
pub struct VhostDeviceProtocols {
    pub protocols: Vec<String>,
    #[qapi(name = "unknown-protocols")]
    pub unknown_protocols: Option<u64>,
}
#[qapi(since = "7.2")]
pub struct VhostStatus {
    #[qapi(name = "n-mem-sections")]
    pub n_mem_sections: i64,
    #[qapi(name = "n-tmp-sections")]
    pub n_tmp_sections: i64,
    pub nvqs: u32,
    #[qapi(name = "vq-index")]
    pub vq_index: i64,
    pub features: VirtioDeviceFeatures,
    #[qapi(name = "acked-features")]
    pub acked_features: VirtioDeviceFeatures,
    #[qapi(name = "backend-features")]
    pub backend_features: VirtioDeviceFeatures,
    #[qapi(name = "protocol-features")]
    pub protocol_features: VhostDeviceProtocols,
    #[qapi(name = "max-queues")]
    pub max_queues: u64,
    #[qapi(name = "backend-cap")]
    pub backend_cap: u64,
    #[qapi(name = "log-enabled")]
    pub log_enabled: bool,
    #[qapi(name = "log-size")]
    pub log_size: u64,
}
#[qapi(name = "VirtIOGPUOutput")]
#[qapi(since = "10.1")]
pub struct VirtIogpuOutput {
    pub name: String,
}
#[qapi(since = "7.2")]
pub struct VirtQueueStatus {
    pub name: String,
    #[qapi(name = "queue-index")]
    pub queue_index: u16,
    pub inuse: u32,
    #[qapi(name = "vring-num")]
    pub vring_num: u32,
    #[qapi(name = "vring-num-default")]
    pub vring_num_default: u32,
    #[qapi(name = "vring-align")]
    pub vring_align: u32,
    #[qapi(name = "vring-desc")]
    pub vring_desc: u64,
    #[qapi(name = "vring-avail")]
    pub vring_avail: u64,
    #[qapi(name = "vring-used")]
    pub vring_used: u64,
    #[qapi(name = "last-avail-idx")]
    pub last_avail_idx: Option<u16>,
    #[qapi(name = "shadow-avail-idx")]
    pub shadow_avail_idx: Option<u16>,
    #[qapi(name = "used-idx")]
    pub used_idx: u16,
    #[qapi(name = "signalled-used")]
    pub signalled_used: u16,
    #[qapi(name = "signalled-used-valid")]
    pub signalled_used_valid: bool,
}
#[qapi(since = "7.2")]
pub struct VirtVhostQueueStatus {
    pub name: String,
    pub kick: i64,
    pub call: i64,
    pub desc: u64,
    pub avail: u64,
    pub used: u64,
    pub num: i64,
    #[qapi(name = "desc-phys")]
    pub desc_phys: u64,
    #[qapi(name = "desc-size")]
    pub desc_size: u32,
    #[qapi(name = "avail-phys")]
    pub avail_phys: u64,
    #[qapi(name = "avail-size")]
    pub avail_size: u32,
    #[qapi(name = "used-phys")]
    pub used_phys: u64,
    #[qapi(name = "used-size")]
    pub used_size: u32,
}
#[qapi(since = "7.2")]
pub struct VirtioDeviceFeatures {
    pub transports: Vec<String>,
    #[qapi(name = "dev-features")]
    pub dev_features: Option<Vec<String>>,
    #[qapi(name = "unknown-dev-features")]
    pub unknown_dev_features: Option<u64>,
    #[qapi(name = "unknown-dev-features2")]
    #[qapi(since = "10.2")]
    pub unknown_dev_features2: Option<u64>,
}
#[qapi(since = "7.2")]
pub struct VirtioDeviceStatus {
    pub statuses: Vec<String>,
    #[qapi(name = "unknown-statuses")]
    pub unknown_statuses: Option<u8>,
}
#[qapi(since = "7.2")]
pub struct VirtioInfo {
    pub path: String,
    pub name: String,
}
#[qapi(name = "VirtioMEMDeviceInfo")]
#[qapi(since = "5.1")]
pub struct VirtioMemDeviceInfo {
    pub id: Option<String>,
    pub memaddr: u64,
    #[qapi(name = "requested-size")]
    pub requested_size: u64,
    pub size: u64,
    #[qapi(name = "max-size")]
    pub max_size: u64,
    #[qapi(name = "block-size")]
    pub block_size: u64,
    pub node: i64,
    pub memdev: String,
}
#[qapi(name = "VirtioMEMDeviceInfoWrapper")]
#[qapi(since = "2.1")]
pub struct VirtioMemDeviceInfoWrapper {
    pub data: VirtioMemDeviceInfo,
}
#[qapi(name = "VirtioPMEMDeviceInfo")]
#[qapi(since = "4.1")]
pub struct VirtioPmemDeviceInfo {
    pub id: Option<String>,
    pub memaddr: u64,
    pub size: u64,
    pub memdev: String,
}
#[qapi(name = "VirtioPMEMDeviceInfoWrapper")]
#[qapi(since = "2.1")]
pub struct VirtioPmemDeviceInfoWrapper {
    pub data: VirtioPmemDeviceInfo,
}
#[qapi(since = "7.2")]
pub struct VirtioQueueElement {
    pub name: String,
    pub index: u32,
    pub descs: Vec<VirtioRingDesc>,
    pub avail: VirtioRingAvail,
    pub used: VirtioRingUsed,
}
#[qapi(since = "7.2")]
pub struct VirtioRingAvail {
    pub flags: u16,
    pub idx: u16,
    pub ring: u16,
}
#[qapi(since = "7.2")]
pub struct VirtioRingDesc {
    pub addr: u64,
    pub len: u32,
    pub flags: Vec<String>,
}
#[qapi(since = "7.2")]
pub struct VirtioRingUsed {
    pub flags: u16,
    pub idx: u16,
}
#[qapi(since = "7.2")]
pub struct VirtioStatus {
    pub name: String,
    #[qapi(name = "device-id")]
    pub device_id: u16,
    #[qapi(name = "vhost-started")]
    pub vhost_started: bool,
    #[qapi(name = "device-endian")]
    pub device_endian: String,
    #[qapi(name = "guest-features")]
    pub guest_features: VirtioDeviceFeatures,
    #[qapi(name = "host-features")]
    pub host_features: VirtioDeviceFeatures,
    #[qapi(name = "backend-features")]
    pub backend_features: VirtioDeviceFeatures,
    #[qapi(name = "num-vqs")]
    pub num_vqs: i64,
    pub status: VirtioDeviceStatus,
    pub isr: u8,
    #[qapi(name = "queue-sel")]
    pub queue_sel: u16,
    #[qapi(name = "vm-running")]
    pub vm_running: bool,
    pub broken: bool,
    pub disabled: bool,
    #[qapi(name = "use-started")]
    pub use_started: bool,
    pub started: bool,
    #[qapi(name = "start-on-kick")]
    pub start_on_kick: bool,
    #[qapi(name = "disable-legacy-check")]
    pub disable_legacy_check: bool,
    #[qapi(name = "bus-name")]
    pub bus_name: String,
    #[qapi(name = "use-guest-notifier-mask")]
    pub use_guest_notifier_mask: bool,
    #[qapi(name = "vhost-dev")]
    pub vhost_dev: Option<VhostStatus>,
}
#[qapi(since = "8.0")]
pub struct VmdkExtentInfo {
    pub filename: String,
    pub format: String,
    #[qapi(name = "virtual-size")]
    pub virtual_size: i64,
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<i64>,
    pub compressed: Option<bool>,
}
#[qapi(since = "2.1")]
pub struct VncBasicInfo {
    pub host: String,
    pub service: String,
    pub family: NetworkAddressFamily,
    pub websocket: bool,
}
#[qapi(since = "0.14.0")]
pub struct VncClientInfo {
    pub host: String,
    pub family: String,
    pub service: String,
    pub x509_dname: Option<String>,
    pub sasl_username: Option<String>,
    #[qapi(since = "2.3")]
    pub websocket: Option<bool>,
}
#[qapi(since = "0.14.0")]
pub struct VncInfo {
    pub enabled: bool,
    pub host: Option<String>,
    pub family: Option<String>,
    pub service: Option<String>,
    pub auth: Option<String>,
    pub clients: Option<Vec<VncClientInfo>>,
}
#[qapi(since = "2.3")]
pub struct VncInfo2 {
    pub id: String,
    pub server: Vec<VncBasicInfo>,
    pub clients: Vec<VncClientInfo>,
    pub auth: VncPrimaryAuth,
    pub vencrypt: Option<VncVencryptSubAuth>,
    pub display: Option<String>,
}
#[qapi(since = "2.1")]
pub struct VncServerInfo {
    pub host: String,
    pub service: String,
    pub family: NetworkAddressFamily,
    pub websocket: bool,
    pub auth: Option<String>,
}
#[qapi(since = "2.9")]
pub struct VncServerInfo2 {
    pub host: String,
    pub service: String,
    pub family: NetworkAddressFamily,
    pub websocket: bool,
    pub auth: VncPrimaryAuth,
    pub vencrypt: Option<VncVencryptSubAuth>,
}
#[qapi(since = "2.8")]
pub struct VsockSocketAddress {
    pub cid: String,
    pub port: String,
}
#[qapi(since = "2.8")]
pub struct VsockSocketAddressWrapper {
    pub data: VsockSocketAddress,
}
#[qapi(event)]
#[qapi(name = "WAKEUP")]
#[qapi(since = "1.1")]
pub struct WakeupEvent {}
#[qapi(event)]
#[qapi(name = "WATCHDOG")]
#[qapi(since = "0.13.0")]
pub struct WatchdogEvent {
    pub action: WatchdogExpirationAction,
}
#[qapi(name = "X86CPUFeatureWordInfo")]
#[qapi(since = "1.5")]
pub struct X86cpuFeatureWordInfo {
    #[qapi(name = "cpuid-input-eax")]
    pub cpuid_input_eax: i64,
    #[qapi(name = "cpuid-input-ecx")]
    pub cpuid_input_ecx: Option<i64>,
    #[qapi(name = "cpuid-register")]
    pub cpuid_register: X86cpuRegister32,
    pub features: i64,
}
#[qapi(name = "XBZRLECacheStats")]
#[qapi(since = "1.2")]
pub struct XbzrleCacheStats {
    #[qapi(name = "cache-size")]
    pub cache_size: i64,
    pub bytes: i64,
    pub pages: i64,
    #[qapi(name = "cache-miss")]
    pub cache_miss: i64,
    pub overflow: i64,
    #[qapi(name = "cache-miss-rate")]
    #[qapi(since = "2.8")]
    pub cache_miss_rate: Option<f64>,
    #[qapi(name = "encoding-rate")]
    #[qapi(since = "5.1")]
    pub encoding_rate: Option<f64>,
}
#[qapi(since = "4.0")]
pub struct XDbgBlockGraph {
    pub nodes: Vec<XDbgBlockGraphNode>,
    pub edges: Vec<XDbgBlockGraphEdge>,
}
#[qapi(since = "4.0")]
pub struct XDbgBlockGraphEdge {
    pub parent: u64,
    pub child: u64,
    pub name: String,
    pub perm: Vec<BlockPermission>,
    #[qapi(name = "shared-perm")]
    pub shared_perm: Vec<BlockPermission>,
}
#[qapi(since = "4.0")]
pub struct XDbgBlockGraphNode {
    pub id: u64,
    pub r#type: XDbgBlockGraphNodeType,
    pub name: String,
}
#[qapi(since = "6.0")]
pub struct YankInstance {
    #[qapi(discriminator)]
    pub r#type: YankInstanceType,
    #[qapi(union)]
    pub u: Option<YankInstanceBranch>,
}
#[qapi(since = "6.0")]
pub struct YankInstanceBlockNode {
    #[qapi(name = "node-name")]
    pub node_name: String,
}
#[qapi(since = "6.0")]
pub struct YankInstanceChardev {
    pub id: String,
}
#[qapi(command)]
#[qapi(name = "add-fd")]
#[qapi(since = "1.2.0")]
#[qapi(returns = "AddfdInfo")]
pub struct AddFd {
    #[qapi(name = "fdset-id")]
    pub fdset_id: Option<i64>,
    pub opaque: Option<String>,
}
#[qapi(command)]
#[qapi(name = "add_client")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct AddClient {
    pub protocol: String,
    pub fdname: String,
    pub skipauth: Option<bool>,
    pub tls: Option<bool>,
}
#[qapi(command)]
#[qapi(name = "announce-self")]
#[qapi(since = "4.0")]
#[qapi(returns = "()")]
pub struct AnnounceSelf {
    #[qapi(flatten)]
    pub data: AnnounceParameters,
}
#[qapi(command)]
#[qapi(name = "balloon")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct Balloon {
    pub value: i64,
}
#[qapi(command)]
#[qapi(name = "block-commit")]
#[qapi(since = "1.3")]
#[qapi(returns = "()")]
pub struct BlockCommit {
    pub device: String,
    pub base: Option<String>,
    pub top: String,
    pub speed: Option<i64>,
    #[qapi(name = "backing-file")]
    #[qapi(since = "2.1")]
    pub backing_file: Option<String>,
    #[qapi(name = "job-id")]
    #[qapi(since = "2.7")]
    pub job_id: Option<String>,
    #[qapi(name = "filter-node-name")]
    #[qapi(since = "2.9")]
    pub filter_node_name: Option<String>,
    #[qapi(name = "base-node")]
    #[qapi(since = "3.1")]
    pub base_node: Option<String>,
    #[qapi(name = "top-node")]
    #[qapi(since = "3.1")]
    pub top_node: Option<String>,
    #[qapi(name = "auto-finalize")]
    #[qapi(since = "3.1")]
    pub auto_finalize: Option<bool>,
    #[qapi(name = "auto-dismiss")]
    #[qapi(since = "3.1")]
    pub auto_dismiss: Option<bool>,
    #[qapi(name = "on-error")]
    #[qapi(since = "5.0")]
    pub on_error: Option<BlockdevOnError>,
    #[qapi(name = "backing-mask-protocol")]
    #[qapi(since = "9.0")]
    pub backing_mask_protocol: Option<bool>,
}
#[qapi(command)]
#[qapi(name = "block-dirty-bitmap-clear")]
#[qapi(since = "2.4")]
#[qapi(returns = "()")]
pub struct BlockDirtyBitmapClear {
    #[qapi(flatten)]
    pub data: BlockDirtyBitmap,
}
#[qapi(command)]
#[qapi(name = "block-dirty-bitmap-disable")]
#[qapi(since = "4.0")]
#[qapi(returns = "()")]
pub struct BlockDirtyBitmapDisable {
    #[qapi(flatten)]
    pub data: BlockDirtyBitmap,
}
#[qapi(command)]
#[qapi(name = "block-dirty-bitmap-enable")]
#[qapi(since = "4.0")]
#[qapi(returns = "()")]
pub struct BlockDirtyBitmapEnable {
    #[qapi(flatten)]
    pub data: BlockDirtyBitmap,
}
#[qapi(command)]
#[qapi(name = "block-dirty-bitmap-remove")]
#[qapi(since = "2.4")]
#[qapi(returns = "()")]
pub struct BlockDirtyBitmapRemove {
    #[qapi(flatten)]
    pub data: BlockDirtyBitmap,
}
#[qapi(command)]
#[qapi(name = "block-export-add")]
#[qapi(since = "5.2")]
#[qapi(returns = "()")]
pub struct BlockExportAdd {
    #[qapi(flatten)]
    pub data: BlockExportOptions,
}
#[qapi(command)]
#[qapi(name = "block-export-del")]
#[qapi(since = "5.2")]
#[qapi(returns = "()")]
pub struct BlockExportDel {
    pub id: String,
    pub mode: Option<BlockExportRemoveMode>,
}
#[qapi(command)]
#[qapi(name = "block-job-cancel")]
#[qapi(since = "1.1")]
#[qapi(returns = "()")]
pub struct BlockJobCancel {
    pub device: String,
    #[qapi(since = "1.3")]
    pub force: Option<bool>,
}
#[qapi(command)]
#[qapi(name = "block-job-change")]
#[qapi(since = "8.2")]
#[qapi(returns = "()")]
pub struct BlockJobChange {
    #[qapi(flatten)]
    pub data: BlockJobChangeOptions,
}
#[qapi(command)]
#[qapi(name = "block-job-complete")]
#[qapi(since = "1.3")]
#[qapi(returns = "()")]
pub struct BlockJobComplete {
    pub device: String,
}
#[qapi(command)]
#[qapi(name = "block-job-dismiss")]
#[qapi(since = "2.12")]
#[qapi(returns = "()")]
pub struct BlockJobDismiss {
    pub id: String,
}
#[qapi(command)]
#[qapi(name = "block-job-finalize")]
#[qapi(since = "2.12")]
#[qapi(returns = "()")]
pub struct BlockJobFinalize {
    pub id: String,
}
#[qapi(command)]
#[qapi(name = "block-job-pause")]
#[qapi(since = "1.3")]
#[qapi(returns = "()")]
pub struct BlockJobPause {
    pub device: String,
}
#[qapi(command)]
#[qapi(name = "block-job-resume")]
#[qapi(since = "1.3")]
#[qapi(returns = "()")]
pub struct BlockJobResume {
    pub device: String,
}
#[qapi(command)]
#[qapi(name = "block-job-set-speed")]
#[qapi(since = "1.1")]
#[qapi(returns = "()")]
pub struct BlockJobSetSpeed {
    pub device: String,
    pub speed: i64,
}
#[qapi(command)]
#[qapi(name = "block-latency-histogram-set")]
#[qapi(since = "4.0")]
#[qapi(returns = "()")]
pub struct BlockLatencyHistogramSet {
    pub id: String,
    pub boundaries: Option<Vec<u64>>,
    #[qapi(name = "boundaries-read")]
    pub boundaries_read: Option<Vec<u64>>,
    #[qapi(name = "boundaries-write")]
    pub boundaries_write: Option<Vec<u64>>,
    #[qapi(name = "boundaries-flush")]
    pub boundaries_flush: Option<Vec<u64>>,
    #[qapi(name = "boundaries-zap")]
    #[qapi(since = "8.1")]
    pub boundaries_zap: Option<Vec<u64>>,
}
#[qapi(command)]
#[qapi(name = "block-set-write-threshold")]
#[qapi(since = "2.3")]
#[qapi(returns = "()")]
pub struct BlockSetWriteThreshold {
    #[qapi(name = "node-name")]
    pub node_name: String,
    #[qapi(name = "write-threshold")]
    pub write_threshold: u64,
}
#[qapi(command)]
#[qapi(name = "block-stream")]
#[qapi(since = "1.1")]
#[qapi(returns = "()")]
pub struct BlockStream {
    pub device: String,
    pub base: Option<String>,
    pub speed: Option<i64>,
    #[qapi(name = "on-error")]
    #[qapi(since = "1.3")]
    pub on_error: Option<BlockdevOnError>,
    #[qapi(name = "backing-file")]
    #[qapi(since = "2.1")]
    pub backing_file: Option<String>,
    #[qapi(name = "job-id")]
    #[qapi(since = "2.7")]
    pub job_id: Option<String>,
    #[qapi(name = "base-node")]
    #[qapi(since = "2.8")]
    pub base_node: Option<String>,
    #[qapi(name = "auto-finalize")]
    #[qapi(since = "3.1")]
    pub auto_finalize: Option<bool>,
    #[qapi(name = "auto-dismiss")]
    #[qapi(since = "3.1")]
    pub auto_dismiss: Option<bool>,
    #[qapi(since = "6.0")]
    pub bottom: Option<String>,
    #[qapi(name = "filter-node-name")]
    #[qapi(since = "6.0")]
    pub filter_node_name: Option<String>,
    #[qapi(name = "backing-mask-protocol")]
    #[qapi(since = "9.0")]
    pub backing_mask_protocol: Option<bool>,
}
#[qapi(command)]
#[qapi(name = "block_passwd")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
#[qapi(until = "6.0")]
pub struct BlockPasswd {
    pub device: String,
    pub password: String,
    #[qapi(name = "node-name")]
    #[qapi(since = "2.1")]
    pub node_name: Option<String>,
}
#[qapi(command)]
#[qapi(name = "block_resize")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct BlockResize {
    pub device: String,
    pub size: i64,
    #[qapi(name = "node-name")]
    #[qapi(since = "2.1")]
    pub node_name: Option<String>,
}
#[qapi(command)]
#[qapi(name = "block_set_io_throttle")]
#[qapi(since = "1.1")]
#[qapi(returns = "()")]
pub struct BlockSetIoThrottle {
    #[qapi(until = "2.7")]
    pub device: Option<String>,
    #[qapi(until = "2.7")]
    pub bps: Option<i64>,
    #[qapi(until = "2.7")]
    pub bps_rd: Option<i64>,
    #[qapi(until = "2.7")]
    pub bps_wr: Option<i64>,
    #[qapi(until = "2.7")]
    pub iops: Option<i64>,
    #[qapi(until = "2.7")]
    pub iops_rd: Option<i64>,
    #[qapi(until = "2.7")]
    pub iops_wr: Option<i64>,
    #[qapi(since = "2.0")]
    #[qapi(until = "2.7")]
    pub bps_max: Option<i64>,
    #[qapi(since = "2.0")]
    #[qapi(until = "2.7")]
    pub bps_rd_max: Option<i64>,
    #[qapi(since = "2.0")]
    #[qapi(until = "2.7")]
    pub bps_wr_max: Option<i64>,
    #[qapi(since = "2.0")]
    #[qapi(until = "2.7")]
    pub iops_max: Option<i64>,
    #[qapi(since = "2.0")]
    #[qapi(until = "2.7")]
    pub iops_rd_max: Option<i64>,
    #[qapi(since = "2.0")]
    #[qapi(until = "2.7")]
    pub iops_wr_max: Option<i64>,
    #[qapi(since = "2.0")]
    #[qapi(until = "2.7")]
    pub iops_size: Option<i64>,
    #[qapi(since = "2.4")]
    #[qapi(until = "2.7")]
    pub group: Option<String>,
    #[qapi(since = "2.6")]
    #[qapi(until = "2.7")]
    pub bps_max_length: Option<i64>,
    #[qapi(since = "2.6")]
    #[qapi(until = "2.7")]
    pub bps_rd_max_length: Option<i64>,
    #[qapi(since = "2.6")]
    #[qapi(until = "2.7")]
    pub bps_wr_max_length: Option<i64>,
    #[qapi(since = "2.6")]
    #[qapi(until = "2.7")]
    pub iops_max_length: Option<i64>,
    #[qapi(since = "2.6")]
    #[qapi(until = "2.7")]
    pub iops_rd_max_length: Option<i64>,
    #[qapi(since = "2.6")]
    #[qapi(until = "2.7")]
    pub iops_wr_max_length: Option<i64>,
    #[qapi(flatten)]
    #[qapi(since = "2.7")]
    pub data: Option<BlockIoThrottle>,
}
#[qapi(command)]
#[qapi(name = "blockdev-add")]
#[qapi(since = "1.7")]
#[qapi(returns = "()")]
pub struct BlockdevAdd {
    #[qapi(until = "2.8")]
    pub options: Option<BlockdevOptions>,
    #[qapi(flatten)]
    #[qapi(since = "2.8")]
    pub data: Option<BlockdevOptions>,
}
#[qapi(command)]
#[qapi(name = "blockdev-change-medium")]
#[qapi(since = "2.5")]
#[qapi(returns = "()")]
pub struct BlockdevChangeMedium {
    pub device: String,
    pub filename: String,
    pub format: Option<String>,
    #[qapi(name = "read-only-mode")]
    pub read_only_mode: Option<BlockdevChangeReadOnlyMode>,
    #[qapi(since = "2.8")]
    pub id: Option<String>,
    #[qapi(since = "7.1")]
    pub force: Option<bool>,
}
#[qapi(command)]
#[qapi(name = "blockdev-close-tray")]
#[qapi(since = "2.5")]
#[qapi(returns = "()")]
pub struct BlockdevCloseTray {
    pub device: String,
    #[qapi(since = "2.8")]
    pub id: Option<String>,
}
#[qapi(command)]
#[qapi(name = "blockdev-create")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
pub struct BlockdevCreate {
    #[qapi(name = "job-id")]
    pub job_id: String,
    pub options: BlockdevCreateOptions,
}
#[qapi(command)]
#[qapi(name = "blockdev-del")]
#[qapi(since = "2.9")]
#[qapi(returns = "()")]
pub struct BlockdevDel {
    #[qapi(name = "node-name")]
    pub node_name: String,
}
#[qapi(command)]
#[qapi(name = "blockdev-insert-medium")]
#[qapi(since = "2.12")]
#[qapi(returns = "()")]
pub struct BlockdevInsertMedium {
    pub id: String,
    #[qapi(name = "node-name")]
    pub node_name: String,
}
#[qapi(command)]
#[qapi(name = "blockdev-mirror")]
#[qapi(since = "2.6")]
#[qapi(returns = "()")]
pub struct BlockdevMirror {
    pub device: String,
    pub target: String,
    pub replaces: Option<String>,
    pub sync: MirrorSyncMode,
    pub speed: Option<i64>,
    pub granularity: Option<u32>,
    #[qapi(name = "buf-size")]
    pub buf_size: Option<i64>,
    #[qapi(name = "on-source-error")]
    pub on_source_error: Option<BlockdevOnError>,
    #[qapi(name = "on-target-error")]
    pub on_target_error: Option<BlockdevOnError>,
    #[qapi(name = "job-id")]
    #[qapi(since = "2.7")]
    pub job_id: Option<String>,
    #[qapi(name = "filter-node-name")]
    #[qapi(since = "2.9")]
    pub filter_node_name: Option<String>,
    #[qapi(name = "copy-mode")]
    #[qapi(since = "3.0")]
    pub copy_mode: Option<MirrorCopyMode>,
    #[qapi(name = "auto-finalize")]
    #[qapi(since = "3.1")]
    pub auto_finalize: Option<bool>,
    #[qapi(name = "auto-dismiss")]
    #[qapi(since = "3.1")]
    pub auto_dismiss: Option<bool>,
    #[qapi(name = "target-is-zero")]
    #[qapi(since = "10.1")]
    pub target_is_zero: Option<bool>,
}
#[qapi(command)]
#[qapi(name = "blockdev-open-tray")]
#[qapi(since = "2.5")]
#[qapi(returns = "()")]
pub struct BlockdevOpenTray {
    pub device: String,
    pub force: Option<bool>,
    #[qapi(since = "2.8")]
    pub id: Option<String>,
}
#[qapi(command)]
#[qapi(name = "blockdev-remove-medium")]
#[qapi(since = "2.12")]
#[qapi(returns = "()")]
pub struct BlockdevRemoveMedium {
    pub id: String,
}
#[qapi(command)]
#[qapi(name = "blockdev-reopen")]
#[qapi(since = "6.1")]
#[qapi(returns = "()")]
pub struct BlockdevReopen {
    pub options: Vec<BlockdevOptions>,
}
#[qapi(command)]
#[qapi(name = "blockdev-set-active")]
#[qapi(since = "10.0")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct BlockdevSetActive {
    #[qapi(name = "node-name")]
    pub node_name: Option<String>,
    pub active: bool,
}
#[qapi(command)]
#[qapi(name = "blockdev-snapshot-delete-internal-sync")]
#[qapi(since = "1.7")]
#[qapi(returns = "SnapshotInfo")]
pub struct BlockdevSnapshotDeleteInternalSync {
    pub device: String,
    pub id: Option<String>,
    pub name: Option<String>,
}
#[qapi(command)]
#[qapi(name = "blockdev-snapshot-internal-sync")]
#[qapi(since = "1.7")]
#[qapi(returns = "()")]
pub struct BlockdevSnapshotInternalSync {
    #[qapi(flatten)]
    pub data: BlockdevSnapshotInternal,
}
#[qapi(command)]
#[qapi(name = "calc-dirty-rate")]
#[qapi(since = "5.2")]
#[qapi(returns = "()")]
pub struct CalcDirtyRate {
    #[qapi(name = "calc-time")]
    pub calc_time: i64,
    #[qapi(name = "sample-pages")]
    #[qapi(since = "6.1")]
    pub sample_pages: Option<i64>,
    #[qapi(since = "6.2")]
    pub mode: Option<DirtyRateMeasureMode>,
    #[qapi(name = "calc-time-unit")]
    #[qapi(since = "8.2")]
    pub calc_time_unit: Option<TimeUnit>,
}
#[qapi(command)]
#[qapi(name = "cancel-vcpu-dirty-limit")]
#[qapi(since = "7.1")]
#[qapi(returns = "()")]
pub struct CancelVcpuDirtyLimit {
    #[qapi(name = "cpu-index")]
    pub cpu_index: Option<i64>,
}
#[qapi(command)]
#[qapi(name = "change")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
#[qapi(until = "6.0")]
pub struct Change {
    pub device: String,
    pub target: String,
    pub arg: Option<String>,
}
#[qapi(command)]
#[qapi(name = "change-backing-file")]
#[qapi(since = "2.1")]
#[qapi(returns = "()")]
pub struct ChangeBackingFile {
    pub device: String,
    #[qapi(name = "image-node-name")]
    pub image_node_name: String,
    #[qapi(name = "backing-file")]
    pub backing_file: String,
}
#[qapi(command)]
#[qapi(name = "change-vnc-password")]
#[qapi(since = "1.1")]
#[qapi(returns = "()")]
pub struct ChangeVncPassword {
    pub password: String,
}
#[qapi(command)]
#[qapi(name = "chardev-add")]
#[qapi(since = "1.4")]
#[qapi(returns = "ChardevReturn")]
pub struct ChardevAdd {
    pub id: String,
    pub backend: ChardevBackend,
}
#[qapi(command)]
#[qapi(name = "chardev-change")]
#[qapi(since = "2.10")]
#[qapi(returns = "ChardevReturn")]
pub struct ChardevChange {
    pub id: String,
    pub backend: ChardevBackend,
}
#[qapi(command)]
#[qapi(name = "chardev-remove")]
#[qapi(since = "1.4")]
#[qapi(returns = "()")]
pub struct ChardevRemove {
    pub id: String,
}
#[qapi(command)]
#[qapi(name = "chardev-send-break")]
#[qapi(since = "2.10")]
#[qapi(returns = "()")]
pub struct ChardevSendBreak {
    pub id: String,
}
#[qapi(command)]
#[qapi(name = "client_migrate_info")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct ClientMigrateInfo {
    pub protocol: String,
    pub hostname: String,
    pub port: Option<i64>,
    #[qapi(name = "tls-port")]
    pub tls_port: Option<i64>,
    #[qapi(name = "cert-subject")]
    pub cert_subject: Option<String>,
}
#[qapi(command)]
#[qapi(name = "closefd")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct Closefd {
    pub fdname: String,
}
#[qapi(command)]
#[qapi(name = "cont")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct Cont {}
#[qapi(command)]
#[qapi(name = "cpu")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
#[qapi(until = "2.12")]
pub struct Cpu {
    pub index: i64,
}
#[qapi(command)]
#[qapi(name = "cpu-add")]
#[qapi(since = "1.5")]
#[qapi(returns = "()")]
#[qapi(until = "5.2")]
pub struct CpuAdd {
    pub id: i64,
}
#[qapi(command)]
#[qapi(name = "cxl-add-dynamic-capacity")]
#[qapi(feature = "unstable")]
#[qapi(returns = "()")]
#[qapi(since = "9.1")]
pub struct CxlAddDynamicCapacity {
    pub path: String,
    #[qapi(name = "host-id")]
    pub host_id: u16,
    #[qapi(name = "selection-policy")]
    pub selection_policy: CxlExtentSelectionPolicy,
    pub region: u8,
    pub tag: Option<String>,
    pub extents: Vec<CxlDynamicCapacityExtent>,
}
#[qapi(command)]
#[qapi(name = "cxl-inject-correctable-error")]
#[qapi(since = "8.0")]
#[qapi(returns = "()")]
pub struct CxlInjectCorrectableError {
    pub path: String,
    pub r#type: CxlCorErrorType,
}
#[qapi(command)]
#[qapi(name = "cxl-inject-dram-event")]
#[qapi(since = "8.1")]
#[qapi(returns = "()")]
pub struct CxlInjectDramEvent {
    pub path: String,
    pub log: CxlEventLog,
    pub flags: u8,
    pub dpa: u64,
    pub descriptor: u8,
    pub r#type: u8,
    #[qapi(name = "transaction-type")]
    pub transaction_type: u8,
    pub channel: Option<u8>,
    pub rank: Option<u8>,
    #[qapi(name = "nibble-mask")]
    pub nibble_mask: Option<u32>,
    #[qapi(name = "bank-group")]
    pub bank_group: Option<u8>,
    pub bank: Option<u8>,
    pub row: Option<u32>,
    pub column: Option<u16>,
    #[qapi(name = "correction-mask")]
    pub correction_mask: Option<Vec<u64>>,
}
#[qapi(command)]
#[qapi(name = "cxl-inject-general-media-event")]
#[qapi(since = "8.1")]
#[qapi(returns = "()")]
pub struct CxlInjectGeneralMediaEvent {
    pub path: String,
    pub log: CxlEventLog,
    pub flags: u8,
    pub dpa: u64,
    pub descriptor: u8,
    pub r#type: u8,
    #[qapi(name = "transaction-type")]
    pub transaction_type: u8,
    pub channel: Option<u8>,
    pub rank: Option<u8>,
    pub device: Option<u32>,
    #[qapi(name = "component-id")]
    pub component_id: Option<String>,
}
#[qapi(command)]
#[qapi(name = "cxl-inject-memory-module-event")]
#[qapi(since = "8.1")]
#[qapi(returns = "()")]
pub struct CxlInjectMemoryModuleEvent {
    pub path: String,
    pub log: CxlEventLog,
    pub flags: u8,
    pub r#type: u8,
    #[qapi(name = "health-status")]
    pub health_status: u8,
    #[qapi(name = "media-status")]
    pub media_status: u8,
    #[qapi(name = "additional-status")]
    pub additional_status: u8,
    #[qapi(name = "life-used")]
    pub life_used: u8,
    pub temperature: i16,
    #[qapi(name = "dirty-shutdown-count")]
    pub dirty_shutdown_count: u32,
    #[qapi(name = "corrected-volatile-error-count")]
    pub corrected_volatile_error_count: u32,
    #[qapi(name = "corrected-persistent-error-count")]
    pub corrected_persistent_error_count: u32,
}
#[qapi(command)]
#[qapi(name = "cxl-inject-poison")]
#[qapi(since = "8.1")]
#[qapi(returns = "()")]
pub struct CxlInjectPoison {
    pub path: String,
    pub start: u64,
    pub length: u64,
}
#[qapi(command)]
#[qapi(name = "cxl-inject-uncorrectable-errors")]
#[qapi(since = "8.0")]
#[qapi(returns = "()")]
pub struct CxlInjectUncorrectableErrors {
    pub path: String,
    pub errors: Vec<CxlUncorErrorRecord>,
}
#[qapi(command)]
#[qapi(name = "cxl-release-dynamic-capacity")]
#[qapi(feature = "unstable")]
#[qapi(returns = "()")]
#[qapi(since = "9.1")]
pub struct CxlReleaseDynamicCapacity {
    pub path: String,
    #[qapi(name = "host-id")]
    pub host_id: u16,
    #[qapi(name = "removal-policy")]
    pub removal_policy: CxlExtentRemovalPolicy,
    #[qapi(name = "forced-removal")]
    pub forced_removal: Option<bool>,
    #[qapi(name = "sanitize-on-release")]
    pub sanitize_on_release: Option<bool>,
    pub region: u8,
    pub tag: Option<String>,
    pub extents: Vec<CxlDynamicCapacityExtent>,
}
#[qapi(command)]
#[qapi(name = "device-list-properties")]
#[qapi(since = "1.2")]
#[qapi(returns = "Vec<DevicePropertyInfo>")]
pub struct DeviceListProperties {
    pub typename: String,
}
#[qapi(command)]
#[qapi(name = "device-sync-config")]
#[qapi(feature = "unstable")]
#[qapi(since = "9.2")]
#[qapi(returns = "()")]
pub struct DeviceSyncConfig {
    pub id: String,
}
#[qapi(command)]
#[qapi(name = "device_add")]
#[qapi(since = "0.13")]
#[qapi(returns = "()")]
pub struct DeviceAdd {
    pub driver: String,
    pub id: String,
    #[qapi(since = "2.9")]
    pub bus: Option<String>,
}
#[qapi(command)]
#[qapi(name = "device_del")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct DeviceDel {
    pub id: String,
}
#[qapi(command)]
#[qapi(name = "display-reload")]
#[qapi(since = "6.0")]
#[qapi(returns = "()")]
pub struct DisplayReload {
    #[qapi(flatten)]
    pub data: DisplayReloadOptions,
}
#[qapi(command)]
#[qapi(name = "display-update")]
#[qapi(since = "7.1")]
#[qapi(returns = "()")]
pub struct DisplayUpdate {
    #[qapi(flatten)]
    pub data: DisplayUpdateOptions,
}
#[qapi(command)]
#[qapi(name = "dump-guest-memory")]
#[qapi(since = "1.2")]
#[qapi(returns = "()")]
pub struct DumpGuestMemory {
    pub paging: bool,
    pub protocol: String,
    pub begin: Option<i64>,
    pub length: Option<i64>,
    #[qapi(since = "2.3")]
    pub format: Option<DumpGuestMemoryFormat>,
    #[qapi(since = "2.6")]
    pub detach: Option<bool>,
}
#[qapi(command)]
#[qapi(name = "dump-skeys")]
#[qapi(since = "2.5")]
#[qapi(returns = "()")]
pub struct DumpSkeys {
    pub filename: String,
}
#[qapi(command)]
#[qapi(name = "dumpdtb")]
#[qapi(condition = "CONFIG_FDT")]
#[qapi(since = "7.2")]
#[qapi(returns = "()")]
pub struct Dumpdtb {
    pub filename: String,
}
#[qapi(command)]
#[qapi(name = "eject")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct Eject {
    pub device: String,
    pub force: Option<bool>,
    #[qapi(since = "2.8")]
    pub id: Option<String>,
}
#[qapi(command)]
#[qapi(name = "expire_password")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct ExpirePassword {
    #[qapi(until = "7.0")]
    pub protocol: Option<String>,
    #[qapi(until = "7.0")]
    pub time: Option<String>,
    #[qapi(flatten)]
    #[qapi(since = "7.0")]
    pub data: Option<ExpirePasswordOptions>,
}
#[qapi(command)]
#[qapi(name = "get-win32-socket")]
#[qapi(condition = "CONFIG_WIN32")]
#[qapi(since = "8.0")]
#[qapi(returns = "()")]
pub struct GetWin32Socket {
    pub info: String,
    pub fdname: String,
}
#[qapi(command)]
#[qapi(name = "getfd")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct Getfd {
    pub fdname: String,
}
#[qapi(command)]
#[qapi(name = "human-monitor-command")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "String")]
pub struct HumanMonitorCommand {
    #[qapi(name = "command-line")]
    pub command_line: String,
    #[qapi(name = "cpu-index")]
    pub cpu_index: Option<i64>,
}
#[qapi(command)]
#[qapi(name = "inject-ghes-v2-error")]
#[qapi(feature = "unstable")]
#[qapi(since = "10.2")]
#[qapi(returns = "()")]
pub struct InjectGhesV2Error {
    pub cper: String,
}
#[qapi(command)]
#[qapi(name = "inject-nmi")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct InjectNmi {}
#[qapi(command)]
#[qapi(name = "input-send-event")]
#[qapi(since = "2.6")]
#[qapi(returns = "()")]
pub struct InputSendEvent {
    pub device: Option<String>,
    pub head: Option<i64>,
    pub events: Vec<InputEvent>,
}
#[qapi(command)]
#[qapi(name = "job-cancel")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
pub struct JobCancel {
    pub id: String,
}
#[qapi(command)]
#[qapi(name = "job-complete")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
pub struct JobComplete {
    pub id: String,
}
#[qapi(command)]
#[qapi(name = "job-dismiss")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
pub struct JobDismiss {
    pub id: String,
}
#[qapi(command)]
#[qapi(name = "job-finalize")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
pub struct JobFinalize {
    pub id: String,
}
#[qapi(command)]
#[qapi(name = "job-pause")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
pub struct JobPause {
    pub id: String,
}
#[qapi(command)]
#[qapi(name = "job-resume")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
pub struct JobResume {
    pub id: String,
}
#[qapi(command)]
#[qapi(name = "memsave")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct Memsave {
    pub val: i64,
    pub size: i64,
    pub filename: String,
    #[qapi(name = "cpu-index")]
    pub cpu_index: Option<i64>,
}
#[qapi(command)]
#[qapi(name = "migrate")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct Migrate {
    pub uri: String,
    #[qapi(until = "9.1")]
    pub blk: Option<bool>,
    #[qapi(until = "9.1")]
    pub inc: Option<bool>,
    pub detach: Option<bool>,
    #[qapi(since = "3.0")]
    pub resume: Option<bool>,
    #[qapi(since = "8.2")]
    pub channels: Option<Vec<MigrationChannel>>,
}
#[qapi(command)]
#[qapi(name = "migrate-continue")]
#[qapi(since = "2.11")]
#[qapi(returns = "()")]
pub struct MigrateContinue {
    pub state: MigrationStatus,
}
#[qapi(command)]
#[qapi(name = "migrate-incoming")]
#[qapi(since = "2.3")]
#[qapi(returns = "()")]
pub struct MigrateIncoming {
    pub uri: String,
    #[qapi(since = "8.2")]
    pub channels: Option<Vec<MigrationChannel>>,
    #[qapi(name = "exit-on-error")]
    #[qapi(since = "9.1")]
    pub exit_on_error: Option<bool>,
}
#[qapi(command)]
#[qapi(name = "migrate-pause")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
#[qapi(allow_oob)]
pub struct MigratePause {}
#[qapi(command)]
#[qapi(name = "migrate-recover")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
#[qapi(allow_oob)]
pub struct MigrateRecover {
    pub uri: String,
}
#[qapi(command)]
#[qapi(name = "migrate-set-cache-size")]
#[qapi(since = "1.2")]
#[qapi(returns = "()")]
#[qapi(until = "6.0")]
pub struct MigrateSetCacheSize {
    pub value: i64,
}
#[qapi(command)]
#[qapi(name = "migrate-set-capabilities")]
#[qapi(since = "1.2")]
#[qapi(returns = "()")]
pub struct MigrateSetCapabilities {
    pub capabilities: Vec<MigrationCapabilityStatus>,
}
#[qapi(command)]
#[qapi(name = "migrate-start-postcopy")]
#[qapi(returns = "()")]
#[qapi(since = "2.5")]
pub struct MigrateStartPostcopy {}
#[qapi(command)]
#[qapi(name = "migrate_cancel")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct MigrateCancel {}
#[qapi(command)]
#[qapi(name = "migrate_set_downtime")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
#[qapi(until = "6.0")]
pub struct MigrateSetDowntime {
    pub value: f64,
}
#[qapi(command)]
#[qapi(name = "migrate_set_speed")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
#[qapi(until = "6.0")]
pub struct MigrateSetSpeed {
    pub value: i64,
}
#[qapi(command)]
#[qapi(name = "nbd-server-add")]
#[qapi(since = "1.3.0")]
#[qapi(returns = "()")]
pub struct NbdServerAdd {
    #[qapi(until = "5.0")]
    pub device: Option<String>,
    #[qapi(until = "5.0")]
    pub writable: Option<bool>,
    #[qapi(since = "2.12")]
    #[qapi(until = "5.0")]
    pub name: Option<String>,
    #[qapi(since = "4.0")]
    #[qapi(until = "5.0")]
    pub bitmap: Option<String>,
    #[qapi(flatten)]
    #[qapi(since = "5.0")]
    pub data: Option<BlockExportNbd>,
}
#[qapi(command)]
#[qapi(name = "nbd-server-remove")]
#[qapi(since = "2.12")]
#[qapi(returns = "()")]
pub struct NbdServerRemove {
    pub name: String,
    pub mode: Option<NbdServerRemoveMode>,
}
#[qapi(command)]
#[qapi(name = "nbd-server-start")]
#[qapi(since = "1.3.0")]
#[qapi(returns = "()")]
pub struct NbdServerStart {
    #[qapi(until = "10.0")]
    pub addr: Option<SocketAddress>,
    #[qapi(name = "tls-creds")]
    #[qapi(since = "2.6")]
    #[qapi(until = "10.0")]
    pub tls_creds: Option<String>,
    #[qapi(name = "tls-authz")]
    #[qapi(since = "4.0")]
    #[qapi(until = "10.0")]
    pub tls_authz: Option<String>,
    #[qapi(name = "max-connections")]
    #[qapi(since = "5.2")]
    #[qapi(until = "10.0")]
    pub max_connections: Option<u32>,
    #[qapi(flatten)]
    #[qapi(since = "10.0")]
    pub data: Option<NbdServerOptionsLegacy>,
}
#[qapi(command)]
#[qapi(name = "nbd-server-stop")]
#[qapi(since = "1.3.0")]
#[qapi(returns = "()")]
pub struct NbdServerStop {}
#[qapi(command)]
#[qapi(name = "netdev_add")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct NetdevAdd {
    #[qapi(until = "5.0")]
    pub r#type: Option<String>,
    #[qapi(until = "5.0")]
    pub id: Option<String>,
    #[qapi(until = "2.5")]
    pub props: Option<serde_json::Value>,
    #[qapi(flatten)]
    #[qapi(since = "5.0")]
    pub data: Option<Netdev>,
}
#[qapi(command)]
#[qapi(name = "netdev_del")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct NetdevDel {
    pub id: String,
}
#[qapi(command)]
#[qapi(name = "object-add")]
#[qapi(since = "2.0")]
#[qapi(returns = "()")]
pub struct ObjectAdd {
    #[qapi(name = "qom-type")]
    #[qapi(until = "6.0")]
    pub qom_type: Option<String>,
    #[qapi(until = "6.0")]
    pub id: Option<String>,
    #[qapi(until = "6.0")]
    pub props: Option<serde_json::Value>,
    #[qapi(flatten)]
    #[qapi(since = "6.0")]
    pub data: Option<ObjectOptions>,
}
#[qapi(command)]
#[qapi(name = "object-del")]
#[qapi(since = "2.0")]
#[qapi(returns = "()")]
pub struct ObjectDel {
    pub id: String,
}
#[qapi(command)]
#[qapi(name = "pmemsave")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct Pmemsave {
    pub val: i64,
    pub size: i64,
    pub filename: String,
}
#[qapi(command)]
#[qapi(name = "qmp_capabilities")]
#[qapi(since = "0.13")]
#[qapi(returns = "()")]
pub struct QmpCapabilities {
    #[qapi(since = "2.12")]
    pub enable: Option<Vec<QmpCapability>>,
}
#[qapi(command)]
#[qapi(name = "qom-get")]
#[qapi(since = "1.1")]
#[qapi(returns = "serde_json::Value")]
pub struct QomGet {
    pub path: String,
    pub property: String,
}
#[qapi(command)]
#[qapi(name = "qom-list")]
#[qapi(since = "1.1")]
#[qapi(returns = "Vec<ObjectPropertyInfo>")]
pub struct QomList {
    pub path: String,
}
#[qapi(command)]
#[qapi(name = "qom-list-get")]
#[qapi(since = "10.1")]
#[qapi(returns = "Vec<ObjectPropertiesValues>")]
#[qapi(allow_preconfig)]
pub struct QomListGet {
    pub paths: Vec<String>,
}
#[qapi(command)]
#[qapi(name = "qom-list-properties")]
#[qapi(since = "2.12")]
#[qapi(returns = "Vec<ObjectPropertyInfo>")]
pub struct QomListProperties {
    pub typename: String,
}
#[qapi(command)]
#[qapi(name = "qom-list-types")]
#[qapi(since = "1.1")]
#[qapi(returns = "Vec<ObjectTypeInfo>")]
pub struct QomListTypes {
    pub implements: Option<String>,
    pub r#abstract: Option<bool>,
}
#[qapi(command)]
#[qapi(name = "qom-set")]
#[qapi(since = "1.1")]
#[qapi(returns = "()")]
pub struct QomSet {
    pub path: String,
    pub property: String,
    pub value: serde_json::Value,
}
#[qapi(command)]
#[qapi(name = "query-accelerators")]
#[qapi(since = "10.2.0")]
#[qapi(returns = "AcceleratorInfo")]
pub struct QueryAccelerators {}
#[qapi(command)]
#[qapi(name = "query-acpi-ospm-status")]
#[qapi(since = "2.1")]
#[qapi(returns = "Vec<AcpiostInfo>")]
pub struct QueryAcpiOspmStatus {}
#[qapi(command)]
#[qapi(name = "query-audiodevs")]
#[qapi(since = "8.0")]
#[qapi(returns = "Vec<Audiodev>")]
pub struct QueryAudiodevs {}
#[qapi(command)]
#[qapi(name = "query-balloon")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "BalloonInfo")]
pub struct QueryBalloon {}
#[qapi(command)]
#[qapi(name = "query-block")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "Vec<BlockInfo>")]
pub struct QueryBlock {}
#[qapi(command)]
#[qapi(name = "query-block-exports")]
#[qapi(since = "5.2")]
#[qapi(returns = "Vec<BlockExportInfo>")]
pub struct QueryBlockExports {}
#[qapi(command)]
#[qapi(name = "query-block-jobs")]
#[qapi(since = "1.1")]
#[qapi(returns = "Vec<BlockJobInfo>")]
pub struct QueryBlockJobs {}
#[qapi(command)]
#[qapi(name = "query-blockstats")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "Vec<BlockStats>")]
pub struct QueryBlockstats {
    #[qapi(name = "query-nodes")]
    #[qapi(since = "2.3")]
    pub query_nodes: Option<bool>,
}
#[qapi(command)]
#[qapi(name = "query-chardev")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "Vec<ChardevInfo>")]
pub struct QueryChardev {}
#[qapi(command)]
#[qapi(name = "query-chardev-backends")]
#[qapi(since = "2.0")]
#[qapi(returns = "Vec<ChardevBackendInfo>")]
pub struct QueryChardevBackends {}
#[qapi(command)]
#[qapi(name = "query-colo-status")]
#[qapi(since = "3.1")]
#[qapi(returns = "ColoStatus")]
pub struct QueryColoStatus {}
#[qapi(command)]
#[qapi(name = "query-command-line-options")]
#[qapi(since = "1.5")]
#[qapi(returns = "Vec<CommandLineOptionInfo>")]
pub struct QueryCommandLineOptions {
    pub option: Option<String>,
}
#[qapi(command)]
#[qapi(name = "query-commands")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "Vec<CommandInfo>")]
pub struct QueryCommands {}
#[qapi(command)]
#[qapi(name = "query-cpu-definitions")]
#[qapi(since = "1.2.0")]
#[qapi(returns = "Vec<CpuDefinitionInfo>")]
pub struct QueryCpuDefinitions {}
#[qapi(command)]
#[qapi(name = "query-cpu-model-baseline")]
#[qapi(since = "2.8.0")]
#[qapi(returns = "CpuModelBaselineInfo")]
pub struct QueryCpuModelBaseline {
    pub modela: CpuModelInfo,
    pub modelb: CpuModelInfo,
}
#[qapi(command)]
#[qapi(name = "query-cpu-model-comparison")]
#[qapi(since = "2.8.0")]
#[qapi(returns = "CpuModelCompareInfo")]
pub struct QueryCpuModelComparison {
    pub modela: CpuModelInfo,
    pub modelb: CpuModelInfo,
}
#[qapi(command)]
#[qapi(name = "query-cpu-model-expansion")]
#[qapi(since = "2.8.0")]
#[qapi(returns = "CpuModelExpansionInfo")]
pub struct QueryCpuModelExpansion {
    pub r#type: CpuModelExpansionType,
    pub model: CpuModelInfo,
}
#[qapi(command)]
#[qapi(name = "query-cpus")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "Vec<CpuInfo>")]
#[qapi(until = "6.0")]
pub struct QueryCpus {}
#[qapi(command)]
#[qapi(name = "query-cpus-fast")]
#[qapi(since = "2.12")]
#[qapi(returns = "Vec<CpuInfoFast>")]
pub struct QueryCpusFast {}
#[qapi(command)]
#[qapi(name = "query-cryptodev")]
#[qapi(since = "8.0")]
#[qapi(returns = "Vec<QCryptodevInfo>")]
pub struct QueryCryptodev {}
#[qapi(command)]
#[qapi(name = "query-current-machine")]
#[qapi(since = "4.0")]
#[qapi(returns = "CurrentMachineParams")]
pub struct QueryCurrentMachine {}
#[qapi(command)]
#[qapi(name = "query-dirty-rate")]
#[qapi(since = "5.2")]
#[qapi(returns = "DirtyRateInfo")]
pub struct QueryDirtyRate {
    #[qapi(name = "calc-time-unit")]
    #[qapi(since = "8.2")]
    pub calc_time_unit: Option<TimeUnit>,
}
#[qapi(command)]
#[qapi(name = "query-display-options")]
#[qapi(since = "3.1")]
#[qapi(returns = "DisplayOptions")]
pub struct QueryDisplayOptions {}
#[qapi(command)]
#[qapi(name = "query-dump")]
#[qapi(since = "2.6")]
#[qapi(returns = "DumpQueryResult")]
pub struct QueryDump {}
#[qapi(command)]
#[qapi(name = "query-dump-guest-memory-capability")]
#[qapi(since = "2.0")]
#[qapi(returns = "DumpGuestMemoryCapability")]
pub struct QueryDumpGuestMemoryCapability {}
#[qapi(command)]
#[qapi(name = "query-events")]
#[qapi(since = "1.2.0")]
#[qapi(returns = "Vec<EventInfo>")]
#[qapi(until = "6.0")]
pub struct QueryEvents {}
#[qapi(command)]
#[qapi(name = "query-fdsets")]
#[qapi(since = "1.2.0")]
#[qapi(returns = "Vec<FdsetInfo>")]
pub struct QueryFdsets {}
#[qapi(command)]
#[qapi(name = "query-firmware-log")]
#[qapi(since = "10.2")]
#[qapi(returns = "FirmwareLog")]
pub struct QueryFirmwareLog {
    #[qapi(name = "max-size")]
    pub max_size: Option<u64>,
}
#[qapi(command)]
#[qapi(name = "query-gic-capabilities")]
#[qapi(since = "2.6")]
#[qapi(returns = "Vec<GicCapability>")]
pub struct QueryGicCapabilities {}
#[qapi(command)]
#[qapi(name = "query-hotpluggable-cpus")]
#[qapi(since = "2.7")]
#[qapi(returns = "Vec<HotpluggableCpu>")]
pub struct QueryHotpluggableCpus {}
#[qapi(command)]
#[qapi(name = "query-hv-balloon-status-report")]
#[qapi(since = "8.2")]
#[qapi(returns = "HvBalloonInfo")]
pub struct QueryHvBalloonStatusReport {}
#[qapi(command)]
#[qapi(name = "query-iothreads")]
#[qapi(since = "2.0")]
#[qapi(returns = "Vec<IoThreadInfo>")]
pub struct QueryIothreads {}
#[qapi(command)]
#[qapi(name = "query-jobs")]
#[qapi(since = "3.0")]
#[qapi(returns = "Vec<JobInfo>")]
pub struct QueryJobs {}
#[qapi(command)]
#[qapi(name = "query-kvm")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "KvmInfo")]
pub struct QueryKvm {}
#[qapi(command)]
#[qapi(name = "query-machines")]
#[qapi(since = "1.2.0")]
#[qapi(returns = "Vec<MachineInfo>")]
pub struct QueryMachines {
    #[qapi(name = "compat-props")]
    #[qapi(feature = "unstable")]
    #[qapi(since = "9.1")]
    pub compat_props: Option<bool>,
}
#[qapi(command)]
#[qapi(name = "query-memdev")]
#[qapi(since = "2.1")]
#[qapi(returns = "Vec<Memdev>")]
pub struct QueryMemdev {}
#[qapi(command)]
#[qapi(name = "query-memory-devices")]
#[qapi(since = "2.1")]
#[qapi(returns = "Vec<MemoryDeviceInfo>")]
pub struct QueryMemoryDevices {}
#[qapi(command)]
#[qapi(name = "query-memory-size-summary")]
#[qapi(since = "2.11.0")]
#[qapi(returns = "MemoryInfo")]
pub struct QueryMemorySizeSummary {}
#[qapi(command)]
#[qapi(name = "query-mice")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "Vec<MouseInfo>")]
pub struct QueryMice {}
#[qapi(command)]
#[qapi(name = "query-migrate")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "MigrationInfo")]
pub struct QueryMigrate {}
#[qapi(command)]
#[qapi(name = "query-migrate-cache-size")]
#[qapi(since = "1.2")]
#[qapi(returns = "i64")]
#[qapi(until = "6.0")]
pub struct QueryMigrateCacheSize {}
#[qapi(command)]
#[qapi(name = "query-migrate-capabilities")]
#[qapi(since = "1.2")]
#[qapi(returns = "Vec<MigrationCapabilityStatus>")]
pub struct QueryMigrateCapabilities {}
#[qapi(command)]
#[qapi(name = "query-migrate-parameters")]
#[qapi(since = "2.4")]
#[qapi(returns = "MigrationParameters")]
pub struct QueryMigrateParameters {}
#[qapi(command)]
#[qapi(name = "query-migrationthreads")]
#[qapi(since = "7.2")]
#[qapi(returns = "Vec<MigrationThreadInfo>")]
pub struct QueryMigrationthreads {}
#[qapi(command)]
#[qapi(name = "query-name")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "NameInfo")]
pub struct QueryName {}
#[qapi(command)]
#[qapi(name = "query-named-block-nodes")]
#[qapi(since = "2.0")]
#[qapi(returns = "Vec<BlockDeviceInfo>")]
pub struct QueryNamedBlockNodes {
    #[qapi(since = "5.0")]
    pub flat: Option<bool>,
}
#[qapi(command)]
#[qapi(name = "query-pci")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "Vec<PciInfo>")]
pub struct QueryPci {}
#[qapi(command)]
#[qapi(name = "query-pr-managers")]
#[qapi(since = "3.0")]
#[qapi(returns = "Vec<PrManagerInfo>")]
#[qapi(allow_preconfig)]
pub struct QueryPrManagers {}
#[qapi(command)]
#[qapi(name = "query-qmp-schema")]
#[qapi(since = "2.5")]
#[qapi(returns = "Vec<SchemaInfo>")]
pub struct QueryQmpSchema {}
#[qapi(command)]
#[qapi(name = "query-replay")]
#[qapi(since = "5.2")]
#[qapi(returns = "ReplayInfo")]
pub struct QueryReplay {}
#[qapi(command)]
#[qapi(name = "query-rocker")]
#[qapi(since = "2.4")]
#[qapi(returns = "RockerSwitch")]
pub struct QueryRocker {
    pub name: String,
}
#[qapi(command)]
#[qapi(name = "query-rocker-of-dpa-flows")]
#[qapi(since = "2.4")]
#[qapi(returns = "Vec<RockerOfDpaFlow>")]
pub struct QueryRockerOfDpaFlows {
    pub name: String,
    #[qapi(name = "tbl-id")]
    pub tbl_id: Option<u32>,
}
#[qapi(command)]
#[qapi(name = "query-rocker-of-dpa-groups")]
#[qapi(since = "2.4")]
#[qapi(returns = "Vec<RockerOfDpaGroup>")]
pub struct QueryRockerOfDpaGroups {
    pub name: String,
    pub r#type: Option<u8>,
}
#[qapi(command)]
#[qapi(name = "query-rocker-ports")]
#[qapi(since = "2.4")]
#[qapi(returns = "Vec<RockerPort>")]
pub struct QueryRockerPorts {
    pub name: String,
}
#[qapi(command)]
#[qapi(name = "query-rx-filter")]
#[qapi(since = "1.6")]
#[qapi(returns = "Vec<RxFilterInfo>")]
pub struct QueryRxFilter {
    pub name: Option<String>,
}
#[qapi(command)]
#[qapi(name = "query-s390x-cpu-polarization")]
#[qapi(condition = "(TARGET_S390X && CONFIG_KVM)")]
#[qapi(feature = "unstable")]
#[qapi(since = "8.2")]
#[qapi(returns = "CpuPolarizationInfo")]
pub struct QueryS390xCpuPolarization {}
#[qapi(command)]
#[qapi(name = "query-sev")]
#[qapi(since = "2.12")]
#[qapi(returns = "SevInfo")]
pub struct QuerySev {}
#[qapi(command)]
#[qapi(name = "query-sev-attestation-report")]
#[qapi(condition = "defined(TARGET_I386)")]
#[qapi(since = "6.1")]
#[qapi(returns = "SevAttestationReport")]
pub struct QuerySevAttestationReport {
    pub mnonce: String,
}
#[qapi(command)]
#[qapi(name = "query-sev-capabilities")]
#[qapi(since = "2.12")]
#[qapi(returns = "SevCapability")]
pub struct QuerySevCapabilities {}
#[qapi(command)]
#[qapi(name = "query-sev-launch-measure")]
#[qapi(since = "2.12")]
#[qapi(returns = "SevLaunchMeasureInfo")]
pub struct QuerySevLaunchMeasure {}
#[qapi(command)]
#[qapi(name = "query-sgx")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "6.2")]
#[qapi(returns = "SgxInfo")]
pub struct QuerySgx {}
#[qapi(command)]
#[qapi(name = "query-sgx-capabilities")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "6.2")]
#[qapi(returns = "SgxInfo")]
pub struct QuerySgxCapabilities {}
#[qapi(command)]
#[qapi(name = "query-spice")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "SpiceInfo")]
pub struct QuerySpice {}
#[qapi(command)]
#[qapi(name = "query-stats")]
#[qapi(since = "7.1")]
#[qapi(returns = "Vec<StatsResult>")]
pub struct QueryStats {
    #[qapi(flatten)]
    pub data: StatsFilter,
}
#[qapi(command)]
#[qapi(name = "query-stats-schemas")]
#[qapi(since = "7.1")]
#[qapi(returns = "Vec<StatsSchema>")]
pub struct QueryStatsSchemas {
    pub provider: Option<StatsProvider>,
}
#[qapi(command)]
#[qapi(name = "query-status")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "StatusInfo")]
pub struct QueryStatus {}
#[qapi(command)]
#[qapi(name = "query-target")]
#[qapi(since = "1.2.0")]
#[qapi(returns = "TargetInfo")]
pub struct QueryTarget {}
#[qapi(command)]
#[qapi(name = "query-tpm")]
#[qapi(since = "1.5")]
#[qapi(returns = "Vec<TpmInfo>")]
pub struct QueryTpm {}
#[qapi(command)]
#[qapi(name = "query-tpm-models")]
#[qapi(since = "1.5")]
#[qapi(returns = "Vec<TpmModel>")]
pub struct QueryTpmModels {}
#[qapi(command)]
#[qapi(name = "query-tpm-types")]
#[qapi(since = "1.5")]
#[qapi(returns = "Vec<TpmType>")]
pub struct QueryTpmTypes {}
#[qapi(command)]
#[qapi(name = "query-uuid")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "UuidInfo")]
pub struct QueryUuid {}
#[qapi(command)]
#[qapi(name = "query-vcpu-dirty-limit")]
#[qapi(since = "7.1")]
#[qapi(returns = "Vec<DirtyLimitInfo>")]
pub struct QueryVcpuDirtyLimit {}
#[qapi(command)]
#[qapi(name = "query-version")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "VersionInfo")]
pub struct QueryVersion {}
#[qapi(command)]
#[qapi(name = "query-vm-generation-id")]
#[qapi(since = "2.9")]
#[qapi(returns = "GuidInfo")]
pub struct QueryVmGenerationId {}
#[qapi(command)]
#[qapi(name = "query-vnc")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "VncInfo")]
pub struct QueryVnc {}
#[qapi(command)]
#[qapi(name = "query-vnc-servers")]
#[qapi(since = "2.3")]
#[qapi(returns = "Vec<VncInfo2>")]
pub struct QueryVncServers {}
#[qapi(command)]
#[qapi(name = "query-xen-replication-status")]
#[qapi(since = "2.9")]
#[qapi(returns = "ReplicationStatus")]
pub struct QueryXenReplicationStatus {}
#[qapi(command)]
#[qapi(name = "query-yank")]
#[qapi(since = "6.0")]
#[qapi(returns = "Vec<YankInstance>")]
#[qapi(allow_oob)]
pub struct QueryYank {}
#[qapi(command)]
#[qapi(name = "quit")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct Quit {}
#[qapi(command)]
#[qapi(name = "remove-fd")]
#[qapi(since = "1.2.0")]
#[qapi(returns = "()")]
pub struct RemoveFd {
    #[qapi(name = "fdset-id")]
    pub fdset_id: i64,
    pub fd: Option<i64>,
}
#[qapi(command)]
#[qapi(name = "replay-break")]
#[qapi(since = "5.2")]
#[qapi(returns = "()")]
pub struct ReplayBreak {
    pub icount: i64,
}
#[qapi(command)]
#[qapi(name = "replay-delete-break")]
#[qapi(since = "5.2")]
#[qapi(returns = "()")]
pub struct ReplayDeleteBreak {}
#[qapi(command)]
#[qapi(name = "replay-seek")]
#[qapi(since = "5.2")]
#[qapi(returns = "()")]
pub struct ReplaySeek {
    pub icount: i64,
}
#[qapi(command)]
#[qapi(name = "request-ebpf")]
#[qapi(condition = "CONFIG_EBPF")]
#[qapi(since = "9.0")]
#[qapi(returns = "EbpfObject")]
pub struct RequestEbpf {
    pub id: EbpfProgramId,
}
#[qapi(command)]
#[qapi(name = "ringbuf-read")]
#[qapi(since = "1.4")]
#[qapi(returns = "String")]
pub struct RingbufRead {
    pub device: String,
    pub size: i64,
    pub format: Option<DataFormat>,
}
#[qapi(command)]
#[qapi(name = "ringbuf-write")]
#[qapi(since = "1.4")]
#[qapi(returns = "()")]
pub struct RingbufWrite {
    pub device: String,
    pub data: String,
    pub format: Option<DataFormat>,
}
#[qapi(command)]
#[qapi(name = "rtc-reset-reinjection")]
#[qapi(since = "2.1")]
#[qapi(returns = "()")]
pub struct RtcResetReinjection {}
#[qapi(command)]
#[qapi(name = "screendump")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct Screendump {
    pub filename: String,
    #[qapi(since = "2.12")]
    pub device: Option<String>,
    #[qapi(since = "2.12")]
    pub head: Option<i64>,
    #[qapi(since = "7.1")]
    pub format: Option<ImageFormat>,
}
#[qapi(command)]
#[qapi(name = "send-key")]
#[qapi(since = "1.3.0")]
#[qapi(returns = "()")]
pub struct SendKey {
    pub keys: Vec<KeyValue>,
    #[qapi(name = "hold-time")]
    pub hold_time: Option<i64>,
}
#[qapi(command)]
#[qapi(name = "set-action")]
#[qapi(since = "6.0")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct SetAction {
    pub reboot: Option<RebootAction>,
    pub shutdown: Option<ShutdownAction>,
    pub panic: Option<PanicAction>,
    pub watchdog: Option<WatchdogAction>,
}
#[qapi(command)]
#[qapi(name = "set-cpu-topology")]
#[qapi(condition = "(TARGET_S390X && CONFIG_KVM)")]
#[qapi(feature = "unstable")]
#[qapi(since = "8.2")]
#[qapi(returns = "()")]
pub struct SetCpuTopology {
    #[qapi(name = "core-id")]
    pub core_id: u16,
    #[qapi(name = "socket-id")]
    pub socket_id: Option<u16>,
    #[qapi(name = "book-id")]
    pub book_id: Option<u16>,
    #[qapi(name = "drawer-id")]
    pub drawer_id: Option<u16>,
    pub entitlement: Option<CpuS390Entitlement>,
    pub dedicated: Option<bool>,
}
#[qapi(command)]
#[qapi(name = "set-numa-node")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct SetNumaNode {
    #[qapi(flatten)]
    pub data: NumaOptions,
}
#[qapi(command)]
#[qapi(name = "set-vcpu-dirty-limit")]
#[qapi(since = "7.1")]
#[qapi(returns = "()")]
pub struct SetVcpuDirtyLimit {
    #[qapi(name = "cpu-index")]
    pub cpu_index: Option<i64>,
    #[qapi(name = "dirty-rate")]
    pub dirty_rate: u64,
}
#[qapi(command)]
#[qapi(name = "set_link")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct SetLink {
    pub name: String,
    pub up: bool,
}
#[qapi(command)]
#[qapi(name = "set_password")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct SetPassword {
    #[qapi(until = "7.0")]
    pub protocol: Option<String>,
    #[qapi(until = "7.0")]
    pub password: Option<String>,
    #[qapi(until = "7.0")]
    pub connected: Option<String>,
    #[qapi(flatten)]
    #[qapi(since = "7.0")]
    pub data: Option<SetPasswordOptions>,
}
#[qapi(command)]
#[qapi(name = "sev-inject-launch-secret")]
#[qapi(condition = "defined(TARGET_I386)")]
#[qapi(since = "6.0")]
#[qapi(returns = "()")]
pub struct SevInjectLaunchSecret {
    #[qapi(name = "packet-header")]
    pub packet_header: String,
    pub secret: String,
    pub gpa: Option<u64>,
}
#[qapi(command)]
#[qapi(name = "snapshot-delete")]
#[qapi(since = "6.0")]
#[qapi(returns = "()")]
pub struct SnapshotDelete {
    #[qapi(name = "job-id")]
    pub job_id: String,
    pub tag: String,
    pub devices: Vec<String>,
}
#[qapi(command)]
#[qapi(name = "snapshot-load")]
#[qapi(since = "6.0")]
#[qapi(returns = "()")]
pub struct SnapshotLoad {
    #[qapi(name = "job-id")]
    pub job_id: String,
    pub tag: String,
    pub vmstate: String,
    pub devices: Vec<String>,
}
#[qapi(command)]
#[qapi(name = "snapshot-save")]
#[qapi(since = "6.0")]
#[qapi(returns = "()")]
pub struct SnapshotSave {
    #[qapi(name = "job-id")]
    pub job_id: String,
    pub tag: String,
    pub vmstate: String,
    pub devices: Vec<String>,
}
#[qapi(command)]
#[qapi(name = "stop")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct Stop {}
#[qapi(command)]
#[qapi(name = "system_powerdown")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct SystemPowerdown {}
#[qapi(command)]
#[qapi(name = "system_reset")]
#[qapi(since = "0.14.0")]
#[qapi(returns = "()")]
pub struct SystemReset {}
#[qapi(command)]
#[qapi(name = "system_wakeup")]
#[qapi(since = "1.1")]
#[qapi(returns = "()")]
pub struct SystemWakeup {}
#[qapi(command)]
#[qapi(name = "trace-event-get-state")]
#[qapi(since = "2.2")]
#[qapi(returns = "Vec<TraceEventInfo>")]
pub struct TraceEventGetState {
    pub name: String,
    #[qapi(since = "2.7")]
    #[qapi(until = "9.1")]
    pub vcpu: Option<i64>,
}
#[qapi(command)]
#[qapi(name = "trace-event-set-state")]
#[qapi(since = "2.2")]
#[qapi(returns = "()")]
pub struct TraceEventSetState {
    pub name: String,
    pub enable: bool,
    #[qapi(name = "ignore-unavailable")]
    pub ignore_unavailable: Option<bool>,
    #[qapi(since = "2.7")]
    #[qapi(until = "9.1")]
    pub vcpu: Option<i64>,
}
#[qapi(command)]
#[qapi(name = "transaction")]
#[qapi(since = "1.1")]
#[qapi(returns = "()")]
pub struct Transaction {
    pub actions: Vec<BlockdevAction>,
    #[qapi(since = "2.8")]
    pub properties: Option<TransactionProperties>,
}
#[qapi(command)]
#[qapi(name = "watchdog-set-action")]
#[qapi(since = "2.11")]
#[qapi(returns = "()")]
pub struct WatchdogSetAction {
    pub action: WatchdogAction,
}
#[qapi(command)]
#[qapi(name = "x-accel-stats")]
#[qapi(feature = "unstable")]
#[qapi(since = "10.1")]
#[qapi(returns = "HumanReadableText")]
pub struct XAccelStats {}
#[qapi(command)]
#[qapi(name = "x-block-dirty-bitmap-disable")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
#[qapi(until = "4.0")]
pub struct XBlockDirtyBitmapDisable {
    #[qapi(flatten)]
    pub data: BlockDirtyBitmap,
}
#[qapi(command)]
#[qapi(name = "x-block-dirty-bitmap-enable")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
#[qapi(until = "4.0")]
pub struct XBlockDirtyBitmapEnable {
    #[qapi(flatten)]
    pub data: BlockDirtyBitmap,
}
#[qapi(command)]
#[qapi(name = "x-block-dirty-bitmap-merge")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
#[qapi(until = "4.0")]
pub struct XBlockDirtyBitmapMerge {
    #[qapi(flatten)]
    pub data: BlockDirtyBitmapMerge,
}
#[qapi(command)]
#[qapi(name = "x-block-latency-histogram-set")]
#[qapi(since = "2.12")]
#[qapi(returns = "()")]
#[qapi(until = "4.0")]
pub struct XBlockLatencyHistogramSet {
    pub device: String,
    pub boundaries: Option<Vec<u64>>,
    #[qapi(name = "boundaries-read")]
    pub boundaries_read: Option<Vec<u64>>,
    #[qapi(name = "boundaries-write")]
    pub boundaries_write: Option<Vec<u64>>,
    #[qapi(name = "boundaries-flush")]
    pub boundaries_flush: Option<Vec<u64>>,
}
#[qapi(command)]
#[qapi(name = "x-blockdev-amend")]
#[qapi(since = "5.1")]
#[qapi(returns = "()")]
pub struct XBlockdevAmend {
    #[qapi(name = "job-id")]
    pub job_id: String,
    #[qapi(name = "node-name")]
    pub node_name: String,
    pub options: BlockdevAmendOptions,
    pub force: Option<bool>,
}
#[qapi(command)]
#[qapi(name = "x-blockdev-change")]
#[qapi(since = "2.7")]
#[qapi(returns = "()")]
pub struct XBlockdevChange {
    pub parent: String,
    pub child: Option<String>,
    pub node: Option<String>,
}
#[qapi(command)]
#[qapi(name = "x-blockdev-create")]
#[qapi(since = "2.12")]
#[qapi(returns = "()")]
#[qapi(until = "3.0")]
pub struct XBlockdevCreate {
    #[qapi(flatten)]
    pub data: BlockdevCreateOptions,
}
#[qapi(command)]
#[qapi(name = "x-blockdev-del")]
#[qapi(since = "2.5")]
#[qapi(returns = "()")]
#[qapi(until = "2.9")]
pub struct XBlockdevDel {
    #[qapi(until = "2.8")]
    pub id: Option<String>,
    #[qapi(name = "node-name")]
    pub node_name: Option<String>,
}
#[qapi(command)]
#[qapi(name = "x-blockdev-insert-medium")]
#[qapi(since = "2.5")]
#[qapi(returns = "()")]
#[qapi(until = "2.12")]
pub struct XBlockdevInsertMedium {
    pub device: String,
    #[qapi(name = "node-name")]
    pub node_name: String,
    #[qapi(since = "2.8")]
    pub id: Option<String>,
}
#[qapi(command)]
#[qapi(name = "x-blockdev-remove-medium")]
#[qapi(since = "2.5")]
#[qapi(returns = "()")]
#[qapi(until = "2.12")]
pub struct XBlockdevRemoveMedium {
    pub device: String,
    #[qapi(since = "2.8")]
    pub id: Option<String>,
}
#[qapi(command)]
#[qapi(name = "x-blockdev-reopen")]
#[qapi(since = "4.0")]
#[qapi(returns = "()")]
#[qapi(until = "6.1")]
pub struct XBlockdevReopen {
    #[qapi(flatten)]
    pub data: BlockdevOptions,
}
#[qapi(command)]
#[qapi(name = "x-blockdev-set-iothread")]
#[qapi(since = "2.12")]
#[qapi(returns = "()")]
pub struct XBlockdevSetIothread {
    #[qapi(name = "node-name")]
    pub node_name: String,
    pub iothread: StrOrNull,
    pub force: Option<bool>,
}
#[qapi(command)]
#[qapi(name = "x-colo-lost-heartbeat")]
#[qapi(since = "2.8")]
#[qapi(returns = "()")]
pub struct XColoLostHeartbeat {}
#[qapi(command)]
#[qapi(name = "x-debug-block-dirty-bitmap-sha256")]
#[qapi(since = "2.10")]
#[qapi(returns = "BlockDirtyBitmapSha256")]
pub struct XDebugBlockDirtyBitmapSha256 {
    #[qapi(flatten)]
    pub data: BlockDirtyBitmap,
}
#[qapi(command)]
#[qapi(name = "x-debug-query-block-graph")]
#[qapi(since = "4.0")]
#[qapi(returns = "XDbgBlockGraph")]
pub struct XDebugQueryBlockGraph {}
#[qapi(command)]
#[qapi(name = "x-exit-preconfig")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
#[qapi(allow_preconfig)]
pub struct XExitPreconfig {}
#[qapi(command)]
#[qapi(name = "x-input-send-event")]
#[qapi(since = "2.2")]
#[qapi(returns = "()")]
#[qapi(until = "2.5")]
pub struct XInputSendEvent {
    pub console: Option<i64>,
    pub events: Vec<InputEvent>,
}
#[qapi(command)]
#[qapi(name = "x-nbd-server-add-bitmap")]
#[qapi(since = "3.0")]
#[qapi(returns = "()")]
#[qapi(until = "4.0")]
pub struct XNbdServerAddBitmap {
    pub name: String,
    pub bitmap: String,
    #[qapi(name = "bitmap-export-name")]
    pub bitmap_export_name: Option<String>,
}
#[qapi(command)]
#[qapi(name = "x-oob-test")]
#[qapi(since = "2.12")]
#[qapi(returns = "()")]
#[qapi(allow_oob)]
#[qapi(until = "3.0")]
pub struct XOobTest {
    pub lock: bool,
}
#[qapi(command)]
#[qapi(name = "x-query-interrupt-controllers")]
#[qapi(feature = "unstable")]
#[qapi(since = "9.1")]
#[qapi(returns = "HumanReadableText")]
pub struct XQueryInterruptControllers {}
#[qapi(command)]
#[qapi(name = "x-query-irq")]
#[qapi(feature = "unstable")]
#[qapi(since = "6.2")]
#[qapi(returns = "HumanReadableText")]
pub struct XQueryIrq {}
#[qapi(command)]
#[qapi(name = "x-query-jit")]
#[qapi(condition = "CONFIG_TCG")]
#[qapi(feature = "unstable")]
#[qapi(since = "6.2")]
#[qapi(returns = "HumanReadableText")]
pub struct XQueryJit {}
#[qapi(command)]
#[qapi(name = "x-query-numa")]
#[qapi(feature = "unstable")]
#[qapi(since = "6.2")]
#[qapi(returns = "HumanReadableText")]
pub struct XQueryNuma {}
#[qapi(command)]
#[qapi(name = "x-query-opcount")]
#[qapi(condition = "CONFIG_TCG")]
#[qapi(feature = "unstable")]
#[qapi(since = "6.2")]
#[qapi(returns = "HumanReadableText")]
#[qapi(until = "10.1")]
pub struct XQueryOpcount {}
#[qapi(command)]
#[qapi(name = "x-query-profile")]
#[qapi(feature = "unstable")]
#[qapi(since = "6.2")]
#[qapi(returns = "HumanReadableText")]
#[qapi(until = "8.1")]
pub struct XQueryProfile {}
#[qapi(command)]
#[qapi(name = "x-query-ramblock")]
#[qapi(feature = "unstable")]
#[qapi(since = "6.2")]
#[qapi(returns = "HumanReadableText")]
pub struct XQueryRamblock {}
#[qapi(command)]
#[qapi(name = "x-query-rdma")]
#[qapi(feature = "unstable")]
#[qapi(since = "6.2")]
#[qapi(returns = "HumanReadableText")]
#[qapi(until = "9.1")]
pub struct XQueryRdma {}
#[qapi(command)]
#[qapi(name = "x-query-roms")]
#[qapi(feature = "unstable")]
#[qapi(since = "6.2")]
#[qapi(returns = "HumanReadableText")]
pub struct XQueryRoms {}
#[qapi(command)]
#[qapi(name = "x-query-usb")]
#[qapi(feature = "unstable")]
#[qapi(since = "6.2")]
#[qapi(returns = "HumanReadableText")]
pub struct XQueryUsb {}
#[qapi(command)]
#[qapi(name = "x-query-virtio")]
#[qapi(feature = "unstable")]
#[qapi(since = "7.2")]
#[qapi(returns = "Vec<VirtioInfo>")]
pub struct XQueryVirtio {}
#[qapi(command)]
#[qapi(name = "x-query-virtio-queue-element")]
#[qapi(feature = "unstable")]
#[qapi(since = "7.2")]
#[qapi(returns = "VirtioQueueElement")]
pub struct XQueryVirtioQueueElement {
    pub path: String,
    pub queue: u16,
    pub index: Option<u16>,
}
#[qapi(command)]
#[qapi(name = "x-query-virtio-queue-status")]
#[qapi(feature = "unstable")]
#[qapi(since = "7.2")]
#[qapi(returns = "VirtQueueStatus")]
pub struct XQueryVirtioQueueStatus {
    pub path: String,
    pub queue: u16,
}
#[qapi(command)]
#[qapi(name = "x-query-virtio-status")]
#[qapi(feature = "unstable")]
#[qapi(since = "7.2")]
#[qapi(returns = "VirtioStatus")]
pub struct XQueryVirtioStatus {
    pub path: String,
}
#[qapi(command)]
#[qapi(name = "x-query-virtio-vhost-queue-status")]
#[qapi(feature = "unstable")]
#[qapi(since = "7.2")]
#[qapi(returns = "VirtVhostQueueStatus")]
pub struct XQueryVirtioVhostQueueStatus {
    pub path: String,
    pub queue: u16,
}
#[qapi(command)]
#[qapi(name = "xen-colo-do-checkpoint")]
#[qapi(since = "2.9")]
#[qapi(returns = "()")]
pub struct XenColoDoCheckpoint {}
#[qapi(command)]
#[qapi(name = "xen-event-inject")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "8.0")]
#[qapi(returns = "()")]
pub struct XenEventInject {
    pub port: u32,
}
#[qapi(command)]
#[qapi(name = "xen-event-list")]
#[qapi(condition = "TARGET_I386")]
#[qapi(since = "8.0")]
#[qapi(returns = "Vec<EvtchnInfo>")]
pub struct XenEventList {}
#[qapi(command)]
#[qapi(name = "xen-load-devices-state")]
#[qapi(since = "2.7")]
#[qapi(returns = "()")]
pub struct XenLoadDevicesState {
    pub filename: String,
}
#[qapi(command)]
#[qapi(name = "xen-save-devices-state")]
#[qapi(since = "1.6")]
#[qapi(returns = "()")]
pub struct XenSaveDevicesState {
    pub filename: String,
    #[qapi(since = "2.11")]
    pub live: Option<bool>,
}
#[qapi(command)]
#[qapi(name = "xen-set-global-dirty-log")]
#[qapi(since = "1.3")]
#[qapi(returns = "()")]
pub struct XenSetGlobalDirtyLog {
    pub enable: bool,
}
#[qapi(command)]
#[qapi(name = "xen-set-replication")]
#[qapi(since = "2.9")]
#[qapi(returns = "()")]
pub struct XenSetReplication {
    pub enable: bool,
    pub primary: bool,
    pub failover: Option<bool>,
}
#[qapi(command)]
#[qapi(name = "yank")]
#[qapi(since = "6.0")]
#[qapi(returns = "()")]
#[qapi(allow_oob)]
pub struct Yank {
    pub instances: Vec<YankInstance>,
}

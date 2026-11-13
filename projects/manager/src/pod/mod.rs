//! QEMU supervision inside an existing container sandbox. No host namespaces,
//! qrab-init helper, confidential-guest hardware or Kubernetes API is required.

mod agent;
mod config;
mod control;
mod disks;
mod events;
mod hotplug;
mod iscsi;
pub mod nic;
mod persistent;
mod process;
mod remote;
pub mod socket;
mod state;
mod supervisor;
mod tpm;
mod view;

pub use agent::{AgentStatus, FreezeState, UpdateState, UpdateStatus};
pub use config::{
    parse_guest_mac, Accelerator, Config, Disk, DiskFormat, Hotplug, Nic, Tpm, Uefi,
    HOTPLUG_BLOCK_MIB, NIC_LIMIT,
};
pub use control::{control_channel, control_channel_for, Action, Control, ControlReceiver};
pub use events::{
    EventPage, QemuTime, VmEvent, EVENT_CAPACITY, EVENT_DATA_LIMIT, EVENT_PAGE_LIMIT,
};
pub use hotplug::{GuestSize, Resize, ResizePhase};
pub use iscsi::{IscsiBus, IscsiDisk};
pub use remote::{Backend, Remote, POOL_SIZE as REMOTE_POOL_SIZE};
pub use state::{probe, status, Phase, Status};
pub use supervisor::{run, run_controlled, Outcome};
pub use view::{
    BootMode, ConfigView, Deployment, DiskView, HotplugView, IscsiTargetView, NicView, PathView,
};

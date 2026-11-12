//! QEMU supervision inside an existing container sandbox. No host namespaces,
//! qrab-init helper, confidential-guest hardware or Kubernetes API is required.

mod agent;
mod config;
mod control;
mod events;
mod hotplug;
mod iscsi;
mod persistent;
mod process;
mod remote;
pub mod socket;
mod state;
mod supervisor;
mod tpm;
mod view;

pub use agent::{AgentStatus, FreezeState, UpdateState, UpdateStatus};
pub use config::{Accelerator, Config, Disk, DiskFormat, Hotplug, Tpm, Uefi, HOTPLUG_BLOCK_MIB};
pub use control::{control_channel, control_channel_for, Action, Control, ControlReceiver};
pub use events::{
    EventPage, QemuTime, VmEvent, EVENT_CAPACITY, EVENT_DATA_LIMIT, EVENT_PAGE_LIMIT,
};
pub use hotplug::{GuestSize, Resize, ResizePhase};
pub use iscsi::{IscsiBus, IscsiDisk};
pub use remote::{Backend, Remote};
pub use state::{probe, status, Phase, Status};
pub use supervisor::{run, run_controlled, Outcome};
pub use view::{
    BootMode, ConfigView, Deployment, DiskView, HotplugView, IscsiTargetView, PathView,
};

//! QEMU supervision inside an existing container sandbox. No host namespaces,
//! qrab-init helper, confidential-guest hardware or Kubernetes API is required.

mod config;
mod control;
mod iscsi;
mod persistent;
mod process;
mod state;
mod supervisor;
mod tpm;

pub use config::{Accelerator, Config, Disk, DiskFormat, Tpm, Uefi};
pub use control::{control_channel, Action, Control, ControlReceiver};
pub use iscsi::{IscsiBus, IscsiDisk};
pub use state::{probe, status, Phase, Status};
pub use supervisor::{run, run_controlled, Outcome};

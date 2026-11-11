//! QEMU supervision inside an existing container sandbox. No host namespaces,
//! qrab-init helper, confidential-guest hardware or Kubernetes API is required.

mod config;
mod state;
mod supervisor;

pub use config::{Accelerator, Config, Disk, DiskFormat};
pub use state::{probe, status, Phase, Status};
pub use supervisor::{run, Outcome};

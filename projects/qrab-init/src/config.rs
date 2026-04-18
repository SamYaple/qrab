use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration passed from manager to qrab-init via pipe fd.
#[derive(Debug, Serialize, Deserialize)]
pub struct InitConfig {
    /// Absolute path to QEMU binary.
    pub qemu_binary: PathBuf,
    /// Complete QEMU argument list.
    pub qemu_args: Vec<String>,
    /// Raw fd number for pre-opened /dev/kvm. None if KVM disabled (TCG mode).
    pub kvm_fd: Option<i32>,
    /// Raw fd number for pre-opened /dev/sev. None if SEV-SNP disabled.
    /// QEMU accesses via sev-device=/proc/self/fd/N.
    #[serde(default)]
    pub sev_fd: Option<i32>,
    /// Extra device nodes to bind-mount into /dev.
    #[serde(default)]
    pub extra_dev_nodes: Vec<PathBuf>,
}

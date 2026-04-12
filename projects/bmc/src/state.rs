use std::collections::VecDeque;
use std::sync::{Arc, OnceLock};

use anyhow::{bail, Result};
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::task::JoinHandle;

use manager::arch::{Arch, MachineConfig};
use manager::vm::{PreconfigVm, Vm, VmBuilder};
use qapi_protocol::QmpEvent;

use crate::types::{BmcConfig, VmPhase};

const EVENT_HISTORY_CAP: usize = 1000;

static BMC: OnceLock<Arc<BmcState>> = OnceLock::new();

/// Get the global BmcState, initializing with defaults if needed.
pub fn bmc() -> Arc<BmcState> {
    BMC.get_or_init(|| BmcState::new()).clone()
}

/// Internal VM state. Not serializable — holds owned QEMU handles.
pub enum VmState {
    Off,
    Preconfig(PreconfigVm),
    Running(Vm),
    ShuttingDown,
}

/// Central BMC state shared across server functions.
pub struct BmcState {
    pub vm: Mutex<VmState>,
    pub event_tx: broadcast::Sender<QmpEvent>,
    pub event_history: Arc<Mutex<VecDeque<QmpEvent>>>,
    pub config: Mutex<BmcConfig>,
    event_forwarder: Mutex<Option<JoinHandle<()>>>,
}

impl BmcState {
    pub fn new() -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(256);
        Arc::new(Self {
            vm: Mutex::new(VmState::Off),
            event_tx,
            event_history: Arc::new(Mutex::new(VecDeque::with_capacity(EVENT_HISTORY_CAP))),
            config: Mutex::new(BmcConfig::default()),
            event_forwarder: Mutex::new(None),
        })
    }

    /// Spawn a VM with the current config. Transitions Off -> Preconfig.
    pub async fn power_on(self: &Arc<Self>) -> Result<()> {
        let mut vm_guard = self.vm.lock().await;
        if !matches!(*vm_guard, VmState::Off) {
            bail!("VM is not off");
        }

        let config = self.config.lock().await.clone();
        let arch = parse_arch(&config.arch)?;
        let machine_config = parse_machine(&config)?;

        let mut preconfig = VmBuilder::new(arch, machine_config).spawn().await?;

        // Extract the real event receiver and replace with a dummy.
        let (dummy_tx, dummy_rx) = mpsc::channel(1);
        drop(dummy_tx);
        let real_rx = std::mem::replace(&mut preconfig.events, dummy_rx);

        // Spawn event forwarder task.
        let tx = self.event_tx.clone();
        let history = self.event_history.clone();
        let handle = tokio::spawn(async move {
            event_forwarder(real_rx, tx, history).await;
        });

        let mut fwd = self.event_forwarder.lock().await;
        *fwd = Some(handle);

        *vm_guard = VmState::Preconfig(preconfig);
        Ok(())
    }

    /// Exit preconfig and start guest execution. Transitions Preconfig -> Running.
    pub async fn start_vm(&self) -> Result<()> {
        let mut vm_guard = self.vm.lock().await;
        let state = std::mem::replace(&mut *vm_guard, VmState::ShuttingDown);
        match state {
            VmState::Preconfig(pvm) => {
                let vm = pvm.start().await?;
                *vm_guard = VmState::Running(vm);
                Ok(())
            }
            other => {
                *vm_guard = other;
                bail!("VM is not in preconfig state");
            }
        }
    }

    /// Pause guest execution.
    pub async fn pause(&self) -> Result<()> {
        let vm_guard = self.vm.lock().await;
        match &*vm_guard {
            VmState::Running(vm) => {
                vm.stop().await?;
                Ok(())
            }
            _ => bail!("VM is not running"),
        }
    }

    /// Resume guest execution.
    pub async fn resume(&self) -> Result<()> {
        let vm_guard = self.vm.lock().await;
        match &*vm_guard {
            VmState::Running(vm) => {
                vm.cont().await?;
                Ok(())
            }
            _ => bail!("VM is not running"),
        }
    }

    /// Reset the guest.
    pub async fn reset(&self) -> Result<()> {
        let vm_guard = self.vm.lock().await;
        match &*vm_guard {
            VmState::Running(vm) => {
                vm.reset().await?;
                Ok(())
            }
            _ => bail!("VM is not running"),
        }
    }

    /// Request guest powerdown via ACPI.
    pub async fn power_off(&self) -> Result<()> {
        let vm_guard = self.vm.lock().await;
        match &*vm_guard {
            VmState::Running(vm) => {
                vm.powerdown().await?;
                Ok(())
            }
            _ => bail!("VM is not running"),
        }
    }

    /// Force quit the QEMU process.
    pub async fn force_off(&self) -> Result<()> {
        let mut vm_guard = self.vm.lock().await;
        let state = std::mem::replace(&mut *vm_guard, VmState::ShuttingDown);
        match state {
            VmState::Preconfig(pvm) => {
                pvm.shutdown().await;
            }
            VmState::Running(vm) => {
                vm.shutdown().await;
            }
            VmState::Off => {
                *vm_guard = VmState::Off;
                return Ok(());
            }
            VmState::ShuttingDown => {
                *vm_guard = VmState::ShuttingDown;
                bail!("VM is already shutting down");
            }
        }
        self.stop_forwarder().await;
        *vm_guard = VmState::Off;
        Ok(())
    }

    /// Get the current VM phase.
    pub async fn phase(&self) -> VmPhase {
        let vm_guard = self.vm.lock().await;
        match &*vm_guard {
            VmState::Off => VmPhase::Off,
            VmState::Preconfig(_) => VmPhase::Preconfig,
            VmState::Running(vm) => {
                match vm.execute(&qapi::commands::QueryStatus {}).await {
                    Ok(status) if !status.running => VmPhase::Paused,
                    _ => VmPhase::Running,
                }
            }
            VmState::ShuttingDown => VmPhase::ShuttingDown,
        }
    }

    async fn stop_forwarder(&self) {
        let mut fwd = self.event_forwarder.lock().await;
        if let Some(handle) = fwd.take() {
            handle.abort();
        }
    }
}

async fn event_forwarder(
    mut rx: mpsc::Receiver<QmpEvent>,
    tx: broadcast::Sender<QmpEvent>,
    history: Arc<Mutex<VecDeque<QmpEvent>>>,
) {
    while let Some(event) = rx.recv().await {
        let _ = tx.send(event.clone());
        let mut h = history.lock().await;
        h.push_back(event);
        if h.len() > EVENT_HISTORY_CAP {
            h.pop_front();
        }
    }
}

fn parse_arch(s: &str) -> Result<Arch> {
    match s {
        "x86_64" => Ok(Arch::X86_64),
        "i386" => Ok(Arch::I386),
        "aarch64" => Ok(Arch::Aarch64),
        "arm" => Ok(Arch::Arm),
        _ => bail!("Unknown architecture: {s}"),
    }
}

fn parse_machine(config: &BmcConfig) -> Result<MachineConfig> {
    match config.machine.as_str() {
        "none" => Ok(MachineConfig::None),
        "bios" => Ok(MachineConfig::Bios {
            memory: config.memory.clone(),
        }),
        "uefi" => Ok(MachineConfig::Uefi {
            memory: config.memory.clone(),
        }),
        _ => bail!("Unknown machine type: {}", config.machine),
    }
}

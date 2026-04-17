use dioxus::prelude::*;

use crate::types::{BmcConfig, VmPhase};
#[cfg(feature = "server")]
use crate::types::{BmcSevSnpConfig, BmcTpmConfig};

#[server]
pub async fn power_on() -> Result<VmPhase, ServerFnError> {
    let state = crate::state::bmc();
    state
        .power_on()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(state.phase().await)
}

#[server]
pub async fn pause() -> Result<VmPhase, ServerFnError> {
    let state = crate::state::bmc();
    state
        .pause()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(state.phase().await)
}

#[server]
pub async fn resume() -> Result<VmPhase, ServerFnError> {
    let state = crate::state::bmc();
    state
        .resume()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(state.phase().await)
}

#[server]
pub async fn reset() -> Result<VmPhase, ServerFnError> {
    let state = crate::state::bmc();
    state
        .reset()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(state.phase().await)
}

#[server]
pub async fn power_off() -> Result<VmPhase, ServerFnError> {
    let state = crate::state::bmc();
    state
        .power_off()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(state.phase().await)
}

#[server]
pub async fn force_off() -> Result<VmPhase, ServerFnError> {
    let state = crate::state::bmc();
    state
        .force_off()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(state.phase().await)
}

#[server]
pub async fn set_kvm(enabled: bool) -> Result<bool, ServerFnError> {
    let state = crate::state::bmc();
    state.config.lock().await.kvm = enabled;
    Ok(enabled)
}

#[server]
pub async fn get_kvm() -> Result<bool, ServerFnError> {
    let state = crate::state::bmc();
    let kvm = state.config.lock().await.kvm;
    Ok(kvm)
}

/// Get the full VM configuration.
#[server]
pub async fn get_config() -> Result<BmcConfig, ServerFnError> {
    let state = crate::state::bmc();
    let state = state.config.lock().await.clone();
    Ok(state)
}

/// Set SEV-SNP enabled/disabled. When enabled, uses default config.
#[server]
pub async fn set_sev_snp(enabled: bool) -> Result<(), ServerFnError> {
    let state = crate::state::bmc();
    let mut config = state.config.lock().await;
    config.sev_snp = if enabled {
        Some(BmcSevSnpConfig::default())
    } else {
        None
    };
    Ok(())
}

/// Set TPM enabled/disabled. Uses swtpm when enabled.
#[server]
pub async fn set_tpm(enabled: bool) -> Result<(), ServerFnError> {
    let state = crate::state::bmc();
    let mut config = state.config.lock().await;
    config.tpm = if enabled {
        Some(BmcTpmConfig { persistent: true })
    } else {
        None
    };
    Ok(())
}

/// Set guest vCPU count. 0 or None = QEMU default (1).
#[server]
pub async fn set_cpus(cpus: u32) -> Result<(), ServerFnError> {
    let state = crate::state::bmc();
    let mut config = state.config.lock().await;
    config.cpus = if cpus == 0 { None } else { Some(cpus) };
    Ok(())
}

/// Set guest memory (e.g. "256M", "1G", "4096M").
#[server]
pub async fn set_memory(memory: String) -> Result<(), ServerFnError> {
    let state = crate::state::bmc();
    state.config.lock().await.memory = memory;
    Ok(())
}

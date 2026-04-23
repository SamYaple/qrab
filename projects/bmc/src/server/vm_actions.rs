use dioxus::prelude::*;

use crate::types::{BmcConfig, VmPhase};

#[server]
pub async fn power_on() -> Result<VmPhase, ServerFnError> {
    let state = crate::state::bmc();
    state
        .power_on()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    state.push_dashboard().await;
    state.push_queue().await;
    Ok(state.phase().await)
}

#[server]
pub async fn pause() -> Result<VmPhase, ServerFnError> {
    let state = crate::state::bmc();
    state
        .pause()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    // The QMP `STOP` event already triggers a dashboard push from the
    // forwarder; this explicit push covers the case where the event
    // hasn't propagated yet and lets the client UI flip immediately.
    state.push_dashboard().await;
    Ok(state.phase().await)
}

#[server]
pub async fn resume() -> Result<VmPhase, ServerFnError> {
    let state = crate::state::bmc();
    state
        .resume()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    state.push_dashboard().await;
    Ok(state.phase().await)
}

#[server]
pub async fn reset() -> Result<VmPhase, ServerFnError> {
    let state = crate::state::bmc();
    state
        .reset()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    state.push_dashboard().await;
    Ok(state.phase().await)
}

#[server]
pub async fn power_off() -> Result<VmPhase, ServerFnError> {
    let state = crate::state::bmc();
    state
        .power_off()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    state.push_dashboard().await;
    state.push_queue().await;
    Ok(state.phase().await)
}

#[server]
pub async fn force_off() -> Result<VmPhase, ServerFnError> {
    let state = crate::state::bmc();
    state
        .force_off()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    state.push_dashboard().await;
    state.push_queue().await;
    Ok(state.phase().await)
}

/// Get the full VM configuration.
#[server]
pub async fn get_config() -> Result<BmcConfig, ServerFnError> {
    let state = crate::state::bmc();
    let state = state.config.lock().await.clone();
    Ok(state)
}

/// Set guest vCPU count. 0 or None = QEMU default (1).
#[server]
pub async fn set_cpus(cpus: u32) -> Result<(), ServerFnError> {
    let state = crate::state::bmc();
    {
        let mut config = state.config.lock().await;
        config.cpus = if cpus == 0 { None } else { Some(cpus) };
    }
    state.push_config().await;
    Ok(())
}

/// Set guest memory (e.g. "256M", "1G", "4096M").
#[server]
pub async fn set_memory(memory: String) -> Result<(), ServerFnError> {
    let state = crate::state::bmc();
    state.config.lock().await.memory = memory;
    state.push_config().await;
    Ok(())
}


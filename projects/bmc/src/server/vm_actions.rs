use dioxus::prelude::*;

use crate::types::VmPhase;

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
pub async fn start_vm() -> Result<VmPhase, ServerFnError> {
    let state = crate::state::bmc();
    state
        .start_vm()
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

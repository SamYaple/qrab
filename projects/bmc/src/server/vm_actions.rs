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

/// Send an attestation challenge nonce to the guest via the virtio-serial
/// attestation channel. The guest's `attest.sh` script reads from
/// `/dev/virtio-ports/org.qrab.attestation` and responds with TPM quotes
/// and SNP attestation report data.
///
/// `nonce` must be a 64-character hex string.
#[server]
pub async fn send_challenge_to_guest(nonce: String) -> Result<String, ServerFnError> {
    // Validate nonce: must be 64 hex chars (32 bytes).
    if nonce.len() != 64 || !nonce.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ServerFnError::new(
            "nonce must be exactly 64 hex characters (32 bytes)",
        ));
    }

    let state = crate::state::bmc();
    let attest_sock = state.attest_sock().await.ok_or_else(|| {
        ServerFnError::new("no attestation channel (VM not running or SEV-SNP not enabled)")
    })?;

    // Connect to the virtio-serial socket and write the nonce.
    // Then read the response (attestation evidence) with a timeout.
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;

    let mut stream = UnixStream::connect(&attest_sock)
        .await
        .map_err(|e| ServerFnError::new(format!("connect to attestation socket: {e}")))?;

    // Write nonce + newline (attest.sh reads a line).
    stream
        .write_all(format!("{nonce}\n").as_bytes())
        .await
        .map_err(|e| ServerFnError::new(format!("write nonce: {e}")))?;

    // Read response with 30s timeout.
    let mut buf = vec![0u8; 16384];
    let n = tokio::time::timeout(std::time::Duration::from_secs(30), stream.read(&mut buf))
        .await
        .map_err(|_| ServerFnError::new("attestation response timed out (30s)"))?
        .map_err(|e| ServerFnError::new(format!("read attestation response: {e}")))?;

    let response = String::from_utf8_lossy(&buf[..n]).to_string();
    Ok(response)
}

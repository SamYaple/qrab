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
    let attest_sock = state
        .attest_sock()
        .await
        .ok_or_else(|| ServerFnError::new("no attestation channel (VM not running or SEV-SNP not enabled)"))?;

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
    let n = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        stream.read(&mut buf),
    )
    .await
    .map_err(|_| ServerFnError::new("attestation response timed out (30s)"))?
    .map_err(|e| ServerFnError::new(format!("read attestation response: {e}")))?;

    let response = String::from_utf8_lossy(&buf[..n]).to_string();
    Ok(response)
}

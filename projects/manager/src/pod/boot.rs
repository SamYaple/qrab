//! Actual guest UEFI BootOrder/BootNext, on the existing bounded agent worker.
use qapi_protocol::{qga::Qga, Error, Result};
use qrab_guest_agent::boot::{valid_reference, QrabBootNext, QrabBootState, MAX_BOOT_OPTIONS};
pub use qrab_guest_agent::boot::{BootOption, BootState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BootSnapshot {
    pub observed_unix_ms: u64,
    pub state: BootState,
}
impl BootSnapshot {
    pub(super) async fn query(qga: &mut Qga) -> Result<Self> {
        Ok(Self::new(qga.execute(&QrabBootState {}).await?))
    }
    pub(super) async fn set_next(qga: &mut Qga, reference: Option<String>) -> Result<Self> {
        if reference.as_deref().is_some_and(|r| !valid_reference(r)) {
            return Err(Error::Protocol("invalid UEFI boot option reference".into()));
        }
        let snapshot = Self::new(
            qga.execute(&QrabBootNext {
                reference: reference.clone(),
            })
            .await?,
        );
        if !snapshot.state.efi_runtime_available || snapshot.state.boot_next != reference {
            return Err(Error::Protocol(
                "guest did not confirm the requested UEFI BootNext".into(),
            ));
        }
        Ok(snapshot)
    }
    fn new(mut state: BootState) -> Self {
        if !state.efi_runtime_available {
            state = BootState::default();
        }
        state.boot_order = state
            .boot_order
            .into_iter()
            .take(MAX_BOOT_OPTIONS)
            .filter(|r| valid_reference(r))
            .fold(Vec::new(), |mut out, r| {
                if !out.contains(&r) {
                    out.push(r);
                }
                out
            });
        state.options = state
            .options
            .into_iter()
            .take(MAX_BOOT_OPTIONS)
            .filter(|o| {
                valid_reference(&o.reference)
                    && state.boot_order.contains(&o.reference)
                    && o.description.len() <= 2048
                    && !o.description.chars().any(char::is_control)
            })
            .fold(Vec::new(), |mut out, o| {
                if !out.iter().any(|p: &BootOption| p.reference == o.reference) {
                    out.push(o);
                }
                out
            });
        state.boot_next = state.boot_next.filter(|r| valid_reference(r));
        Self {
            observed_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            state,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn untrusted_boot_results_are_bounded_without_inventing_references() {
        let snapshot = BootSnapshot::new(BootState {
            efi_runtime_available: true,
            boot_order: vec!["Boot0001".into(), "Boot0001".into(), "/private/host".into()],
            boot_next: Some("../Boot0001".into()),
            options: vec![
                BootOption {
                    reference: "Boot0001".into(),
                    description: "Linux".into(),
                    active: true,
                },
                BootOption {
                    reference: "Boot0002".into(),
                    description: "not in order".into(),
                    active: true,
                },
            ],
        });
        assert_eq!(snapshot.state.boot_order, ["Boot0001"]);
        assert_eq!(snapshot.state.options.len(), 1);
        assert!(snapshot.state.boot_next.is_none());
        assert_eq!(
            BootSnapshot::new(BootState::default()).state,
            BootState::default()
        );
    }
}

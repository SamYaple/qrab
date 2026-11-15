//! VM service IDs are bound to metadata already checked by visible_vm/pod_ip.
use qrab_operator::api::VirtualMachine;
use qrab_spiffe::{Identity, SpiffeId};

pub fn peer(identity: &Identity, vm: &VirtualMachine, component: &str) -> anyhow::Result<SpiffeId> {
    let own = identity.id()?;
    Ok(qrab_spiffe::vm_service_id(
        own.trust_domain().as_ref(),
        vm.metadata
            .namespace
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("VM namespace unavailable"))?,
        vm.metadata
            .uid
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("VM UID unavailable"))?,
        component,
    )?)
}

//! Conntrack handoff between qrab-netds, node to node. A migration's target
//! node pulls the NIC's entries straight from the source node's agent, so no
//! VM manager is in the path: a manager sits inside its VM's trust boundary
//! (QEMU runs beside it), and entries it relayed could name flows the NIC's
//! security groups would not admit as new.
//!
//! The source agent listens on [`PEER_PORT`] in its node's host network
//! namespace, which only host processes can bind, and serves one address's
//! entries to a requester only when that requester's address is the node of
//! another anchor of the same VM's same NIC holding that address (the
//! migration's standby or new active anchor); see [`may_fetch`].
//!
//! The production transport is TLS 1.3 with exact node SPIFFE IDs in
//! `peer_tls`; there is no plaintext peer transport.
//!
//! ```
//! use qrab_netd::peer::{may_fetch, sources, Holder};
//! let anchor = |host: [u8; 4]| Holder { owner: "vm-uid".into(), nic: "eth0".into(), host: host.into() };
//! let source = anchor([10, 203, 0, 11]);
//! let target = anchor([10, 203, 0, 12]);
//! let cluster = [source.clone(), target.clone()];
//! // The target node asks the source node, and only it may.
//! assert_eq!(sources(&target, &cluster), [source.host]);
//! assert!(may_fetch(&[source.clone()], &cluster, target.host).is_ok());
//! assert!(may_fetch(&[source.clone()], &cluster, [10, 203, 0, 13].into()).is_err());
//! ```
use std::{net::Ipv4Addr, time::Duration};

/// The agents' node-to-node port.
pub const PEER_PORT: u16 = 7911;
/// Deadline for each authenticated peer handshake/read/write.
pub const DEADLINE: Duration = Duration::from_secs(5);

/// A tap anchor holding an address: its VM (owner UID), NIC and node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Holder {
    pub owner: String,
    pub nic: String,
    pub host: Ipv4Addr,
}

/// Whether the node at `peer` may fetch an address's entries from this node.
/// `local` are this node's anchors holding the address, `cluster` every
/// node's: `peer` must be another node holding an anchor of the same VM's
/// same NIC.
pub fn may_fetch(local: &[Holder], cluster: &[Holder], peer: Ipv4Addr) -> Result<(), String> {
    let mine = local
        .first()
        .ok_or("no anchor on this node holds the address")?;
    if peer == mine.host {
        return Err("the requester is this node".into());
    }
    if cluster
        .iter()
        .any(|h| h.host == peer && h.owner == mine.owner && h.nic == mine.nic)
    {
        Ok(())
    } else {
        Err(format!(
            "node {peer} holds no anchor of NIC {} of VM {}",
            mine.nic, mine.owner
        ))
    }
}

/// The other nodes holding an anchor of `mine`'s VM and NIC, to pull from.
pub fn sources(mine: &Holder, cluster: &[Holder]) -> Vec<Ipv4Addr> {
    let mut hosts: Vec<Ipv4Addr> = cluster
        .iter()
        .filter(|h| h.host != mine.host && h.owner == mine.owner && h.nic == mine.nic)
        .map(|h| h.host)
        .collect();
    hosts.sort();
    hosts.dedup();
    hosts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_another_node_of_the_same_nic_may_fetch() {
        let holder = |owner: &str, nic: &str, host: [u8; 4]| Holder {
            owner: owner.into(),
            nic: nic.into(),
            host: host.into(),
        };
        let mine = holder("vm", "eth0", [10, 203, 0, 11]);
        let target = holder("vm", "eth0", [10, 203, 0, 12]);
        let other_nic = holder("vm", "eth1", [10, 203, 0, 13]);
        let other_vm = holder("other", "eth0", [10, 203, 0, 13]);
        let cluster = [mine.clone(), target.clone(), other_nic, other_vm];
        assert!(may_fetch(std::slice::from_ref(&mine), &cluster, target.host).is_ok());
        for peer in [[10, 203, 0, 11], [10, 203, 0, 13], [10, 244, 3, 9]] {
            assert!(
                may_fetch(std::slice::from_ref(&mine), &cluster, peer.into()).is_err(),
                "{peer:?}"
            );
        }
        assert!(
            may_fetch(&[], &cluster, target.host).is_err(),
            "nothing held here"
        );
        assert_eq!(sources(&target, &cluster), [mine.host]);
        assert!(sources(&holder("vm", "eth0", [10, 203, 0, 13]), &[]).is_empty());
    }
}

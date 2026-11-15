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
use crate::ct::Export;
use serde::{Deserialize, Serialize};
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, TcpStream},
    time::Duration,
};

/// The agents' node-to-node port.
pub const PEER_PORT: u16 = 7911;
/// Deadline for each read and write of an exchange.
pub const DEADLINE: Duration = Duration::from_secs(5);
const REQUEST_LIMIT: u64 = 256;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PeerRequest {
    address: Ipv4Addr,
}

#[derive(Serialize, Deserialize)]
struct PeerReply {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    conntrack: Option<Export>,
}

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

/// Serve one requester: read its address, answer with `answer`'s entries or
/// its refusal.
pub fn serve_one(
    stream: &TcpStream,
    answer: impl FnOnce(Ipv4Addr) -> Result<Export, String>,
) -> io::Result<()> {
    stream.set_read_timeout(Some(DEADLINE))?;
    stream.set_write_timeout(Some(DEADLINE))?;
    let mut line = String::new();
    BufReader::new(stream.take(REQUEST_LIMIT)).read_line(&mut line)?;
    let reply = match serde_json::from_str::<PeerRequest>(line.trim_end()) {
        Ok(request) => match answer(request.address) {
            Ok(export) => PeerReply {
                error: None,
                conntrack: Some(export),
            },
            Err(error) => PeerReply {
                error: Some(error),
                conntrack: None,
            },
        },
        Err(error) => PeerReply {
            error: Some(format!("bad request: {error}")),
            conntrack: None,
        },
    };
    let mut body = serde_json::to_vec(&reply)?;
    body.push(b'\n');
    let mut stream = stream;
    stream.write_all(&body)
}

/// Fetch `address`'s entries over a connection to another node's agent.
pub fn fetch(stream: &TcpStream, address: Ipv4Addr) -> io::Result<Export> {
    stream.set_read_timeout(Some(DEADLINE))?;
    stream.set_write_timeout(Some(DEADLINE))?;
    let mut request = serde_json::to_vec(&PeerRequest { address })?;
    request.push(b'\n');
    let mut writer = stream;
    writer.write_all(&request)?;
    let mut body = Vec::new();
    stream
        .take(crate::wire::PAYLOAD_LIMIT as u64 + 1)
        .read_to_end(&mut body)?;
    if body.len() > crate::wire::PAYLOAD_LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "reply too large",
        ));
    }
    let end = body.iter().position(|b| *b == b'\n').unwrap_or(body.len());
    let reply: PeerReply = serde_json::from_slice(&body[..end])
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("bad reply: {e}")))?;
    match (reply.conntrack, reply.error) {
        (Some(export), None) => Ok(export),
        (_, error) => Err(io::Error::other(format!(
            "the source node refused: {}",
            error.unwrap_or_default()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn exchange(
        answer: impl FnOnce(Ipv4Addr) -> Result<Export, String> + Send + 'static,
    ) -> io::Result<Export> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            serve_one(&stream, answer).unwrap();
        });
        let stream = TcpStream::connect(addr).unwrap();
        let fetched = fetch(&stream, Ipv4Addr::new(10, 244, 128, 7));
        server.join().unwrap();
        fetched
    }

    #[test]
    fn entries_travel_node_to_node_and_refusals_come_back() {
        let export = Export {
            now: 42,
            entries: (0..300)
                .map(|i| ('t', format!("{i:028x}"), "00".repeat(56)))
                .collect(),
            truncated: false,
        };
        let served = export.clone();
        let fetched = exchange(move |address| {
            assert_eq!(address, Ipv4Addr::new(10, 244, 128, 7));
            Ok(served)
        })
        .unwrap();
        assert_eq!(fetched, export);
        let refused = exchange(|_| Err("node 10.203.0.13 holds no anchor".into())).unwrap_err();
        assert!(refused.to_string().contains("holds no anchor"), "{refused}");
    }

    #[test]
    fn an_oversized_or_malformed_request_is_refused() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            serve_one(&stream, |_| panic!("no answer for a bad request")).unwrap();
        });
        let mut stream = TcpStream::connect(addr).unwrap();
        stream.write_all(&[b'x'; 1024]).unwrap();
        let mut reply = String::new();
        BufReader::new(&stream).read_line(&mut reply).unwrap();
        assert!(reply.contains("bad request"), "{reply}");
        server.join().unwrap();
    }

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

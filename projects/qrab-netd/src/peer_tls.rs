//! Authenticated node-to-node conntrack transport. TLS identity binds a host
//! address to the node name learned from Kubernetes anchors. The caller must
//! additionally authorize the requested VM/NIC through `peer::may_fetch`.
use crate::{authz, ct::Export, peer::DEADLINE, wire::PAYLOAD_LIMIT};
use k8s_openapi::api::core::v1::Pod;
use qrab_spiffe::{Identity, SpiffeId};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{collections::BTreeSet, future::Future, io, net::Ipv4Addr};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    net::TcpStream,
};

const REQUEST_LIMIT: usize = 256;

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

/// One URI component, using Kubernetes node DNS-subdomain names only.
fn node_name(node: &str) -> bool {
    !node.is_empty()
        && node.len() <= 253
        && node.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.as_bytes()[0].is_ascii_alphanumeric()
                && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

/// Exact workload ID. The SPIRE registration must bind the daemon's real
/// Pod node, ServiceAccount, container and UID; no caller-supplied ID is used.
pub fn node_id(domain: &str, node: &str) -> io::Result<SpiffeId> {
    if !node_name(node) {
        return Err(invalid("invalid netd node name"));
    }
    qrab_spiffe::id(&format!(
        "spiffe://{domain}/kubernetes/qrab-node/qrab-netd/{node}"
    ))
    .map_err(invalid)
}

/// Resolve a host IP from current, nonterminating anchors. An absent or
/// ambiguous mapping refuses TLS rather than trusting an address alone.
pub fn node_for_host<'a>(
    pods: impl IntoIterator<Item = &'a Pod>,
    host: Ipv4Addr,
) -> io::Result<String> {
    let nodes: BTreeSet<_> = pods
        .into_iter()
        .filter(|p| authz::is_tap_anchor(p))
        .filter(|p| {
            p.status
                .as_ref()
                .and_then(|s| s.host_ip.as_deref())
                .and_then(|ip| ip.parse::<Ipv4Addr>().ok())
                == Some(host)
        })
        .filter_map(|p| p.spec.as_ref()?.node_name.as_deref())
        .collect();
    match nodes.into_iter().collect::<Vec<_>>().as_slice() {
        [node] if node_name(node) => Ok((*node).into()),
        _ => Err(invalid("no unambiguous live anchor node for peer host IP")),
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    address: Ipv4Addr,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    conntrack: Option<Export>,
}

/// Rotating identity with an enforced local node binding. No mode disables
/// TLS or admits a peer by trust-domain prefix alone.
#[derive(Clone)]
pub struct Transport {
    identity: Identity,
    domain: String,
    own: SpiffeId,
}

impl Transport {
    pub fn new(identity: Identity, node: &str) -> io::Result<Self> {
        let own = identity.id().map_err(invalid)?;
        let domain = own.trust_domain().to_string();
        if own != node_id(&domain, node)? {
            return Err(invalid("SPIFFE identity does not match this netd node"));
        }
        Ok(Self {
            identity,
            domain,
            own,
        })
    }

    fn check_identity(&self) -> io::Result<()> {
        if self.identity.id().map_err(invalid)? != self.own {
            return Err(invalid("netd SPIFFE identity changed node binding"));
        }
        Ok(())
    }

    pub async fn fetch(&self, tcp: TcpStream, node: &str, address: Ipv4Addr) -> io::Result<Export> {
        bounded(async {
            self.check_identity()?;
            let connector = self
                .identity
                .connector(&[node_id(&self.domain, node)?])
                .map_err(invalid)?;
            let mut stream = connector.connect(tcp).await.map_err(invalid)?;
            write(&mut stream, &Request { address }, REQUEST_LIMIT).await?;
            let reply: Reply = read(&mut stream, PAYLOAD_LIMIT).await?;
            match (reply.conntrack, reply.error) {
                (Some(export), None) => {
                    // Reject invalid records before a caller can import them.
                    export.decode().map_err(invalid)?;
                    Ok(export)
                }
                (None, Some(error)) => Err(io::Error::other(format!(
                    "the source node refused: {error}"
                ))),
                _ => Err(invalid("ambiguous conntrack reply")),
            }
        })
        .await
    }

    /// Authenticate and read before invoking the authorization/export callback.
    /// The callback is awaited to completion, even if the client abandons its
    /// request, so the daemon's connection permit also bounds blocking exports.
    pub async fn serve<F, Fut>(&self, tcp: TcpStream, node: &str, answer: F) -> io::Result<()>
    where
        F: FnOnce(Ipv4Addr) -> Fut,
        Fut: Future<Output = Result<Export, String>>,
    {
        let (mut stream, request) = bounded(async {
            self.check_identity()?;
            let acceptor = self
                .identity
                .acceptor(&[node_id(&self.domain, node)?])
                .map_err(invalid)?;
            let mut stream = acceptor.accept(tcp).await.map_err(invalid)?;
            let request: Request = read(&mut stream, REQUEST_LIMIT).await?;
            Ok((stream, request))
        })
        .await?;
        let reply = match answer(request.address).await {
            Ok(export) => Reply {
                error: None,
                conntrack: Some(export),
            },
            Err(error) => Reply {
                error: Some(error),
                conntrack: None,
            },
        };
        bounded(write(&mut stream, &reply, PAYLOAD_LIMIT)).await
    }
}

async fn bounded<T>(future: impl Future<Output = io::Result<T>>) -> io::Result<T> {
    tokio::time::timeout(DEADLINE, future)
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "conntrack peer deadline elapsed"))?
}

async fn read<T: DeserializeOwned, IO: AsyncRead + Unpin>(
    stream: &mut IO,
    limit: usize,
) -> io::Result<T> {
    let mut line = Vec::new();
    BufReader::new(stream)
        .take(limit as u64 + 1)
        .read_until(b'\n', &mut line)
        .await?;
    if line.len() > limit || line.last() != Some(&b'\n') {
        return Err(invalid("oversized or incomplete conntrack frame"));
    }
    serde_json::from_slice(&line).map_err(invalid)
}

async fn write<T: Serialize, IO: AsyncWrite + Unpin>(
    stream: &mut IO,
    frame: &T,
    limit: usize,
) -> io::Result<()> {
    let mut line = serde_json::to_vec(frame).map_err(invalid)?;
    line.push(b'\n');
    if line.len() > limit {
        return Err(invalid("conntrack frame exceeds limit"));
    }
    stream.write_all(&line).await?;
    stream.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn framing_rejects_oversize_malformed_unterminated_and_extra_fields() {
        for bytes in [
            vec![b'x'; REQUEST_LIMIT + 1],
            b"{bad}\n".to_vec(),
            b"{\"address\":\"10.0.0.1\"}".to_vec(),
            b"{\"address\":\"10.0.0.1\",\"spiffe_id\":\"trusted\"}\n".to_vec(),
        ] {
            let (mut tx, mut rx) = tokio::io::duplex(1024);
            tx.write_all(&bytes).await.unwrap();
            tx.shutdown().await.unwrap();
            assert!(read::<Request, _>(&mut rx, REQUEST_LIMIT).await.is_err());
        }
        let (mut tx, mut rx) = tokio::io::duplex(1024);
        assert!(write(&mut tx, &"x".repeat(REQUEST_LIMIT), REQUEST_LIMIT)
            .await
            .is_err());
        tx.shutdown().await.unwrap();
        let mut bytes = Vec::new();
        rx.read_to_end(&mut bytes).await.unwrap();
        assert!(bytes.is_empty());
    }

    #[tokio::test]
    async fn random_frames_are_bounded_and_never_decode_as_requests() {
        let mut seed = 0x9e3779b97f4a7c15u64;
        for round in 0..200 {
            let mut bytes = Vec::new();
            for _ in 0..round * 3 {
                seed ^= seed >> 12;
                seed ^= seed << 25;
                seed ^= seed >> 27;
                bytes.push(seed as u8);
            }
            bytes.push(b'\n');
            let (mut tx, mut rx) = tokio::io::duplex(1024);
            tx.write_all(&bytes).await.unwrap();
            tx.shutdown().await.unwrap();
            assert!(read::<Request, _>(&mut rx, REQUEST_LIMIT).await.is_err());
        }
    }
}

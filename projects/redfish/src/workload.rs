//! Optional SPIFFE mTLS transport. Workload identity is an additional admission
//! check: the ordinary router still authenticates and authorizes every request.
//! No identity is taken from HTTP headers and no role is granted by this module.
use anyhow::{ensure, Context, Result};
pub use qrab_spiffe::http::{Connection, Listener, MAX_CONNECTIONS, MAX_HANDSHAKES};
use qrab_spiffe::{Identity, SpiffeId};
use std::net::SocketAddr;

pub struct Config {
    listen: SocketAddr,
    peers: Vec<SpiffeId>,
    endpoint: String,
}

impl Config {
    /// Disabled unless explicitly configured. Partial configuration is an error.
    pub fn from_env(api: SocketAddr, metrics: SocketAddr) -> Result<Option<Self>> {
        let value = |name| match std::env::var(name) {
            Ok(value) => Ok(Some(value)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(error) => Err(anyhow::Error::new(error).context(name)),
        };
        let listen = value("REDFISH_WORKLOAD_LISTEN")?;
        let peers = value("REDFISH_WORKLOAD_PEERS")?;
        let endpoint = value("SPIFFE_ENDPOINT_SOCKET")?.or(value(qrab_spiffe::ENDPOINT_ENV)?);
        Self::parse(
            listen.as_deref(),
            peers.as_deref(),
            endpoint.as_deref(),
            api,
            metrics,
        )
    }

    fn parse(
        listen: Option<&str>,
        peers: Option<&str>,
        endpoint: Option<&str>,
        api: SocketAddr,
        metrics: SocketAddr,
    ) -> Result<Option<Self>> {
        let Some(listen) = listen else {
            ensure!(
                peers.is_none(),
                "REDFISH_WORKLOAD_PEERS requires REDFISH_WORKLOAD_LISTEN"
            );
            return Ok(None);
        };
        let listen: SocketAddr = listen.parse().context("REDFISH_WORKLOAD_LISTEN")?;
        ensure!(
            listen.port() != api.port() && listen.port() != metrics.port(),
            "workload listener must use a separate port"
        );
        let peers = peers.context("REDFISH_WORKLOAD_PEERS is required")?;
        let peers: Vec<_> = peers
            .split(',')
            .map(|peer| {
                ensure!(!peer.trim().is_empty(), "empty workload peer");
                qrab_spiffe::id(peer.trim()).map_err(anyhow::Error::new)
            })
            .collect::<Result<_>>()?;
        ensure!(peers.len() <= 64, "at most 64 workload peers are supported");
        for (index, peer) in peers.iter().enumerate() {
            ensure!(!peers[..index].contains(peer), "duplicate workload peer");
        }
        let endpoint = endpoint.unwrap_or(qrab_spiffe::DEFAULT_ENDPOINT);
        ensure!(
            endpoint.starts_with("unix:") && endpoint.len() > 5,
            "SPIFFE_ENDPOINT_SOCKET must name a Unix socket"
        );
        Ok(Some(Self {
            listen,
            peers,
            endpoint: endpoint.into(),
        }))
    }

    /// Connect once for API, metrics and history; all use rotating SVIDs.
    pub async fn identity(&self) -> Result<Identity> {
        let identity = Identity::connect(&self.endpoint).await?;
        if let Ok(expected) = std::env::var("QRAB_EXPECTED_SPIFFE_ID") {
            ensure!(
                identity.id()? == qrab_spiffe::id(&expected)?,
                "unexpected Redfish SPIFFE identity"
            );
        }
        Ok(identity)
    }
    pub async fn bind_identity(self, identity: &Identity) -> Result<Listener> {
        Ok(Listener::bind(self.listen, identity, &self.peers).await?)
    }
    /// Fails closed if no rotating identity or exact-peer TLS config is available.
    pub async fn bind(self) -> Result<Listener> {
        let identity = self.identity().await?;
        self.bind_identity(&identity).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opt_in_configuration_is_fail_closed() {
        let api = "0.0.0.0:8081".parse().unwrap();
        let metrics = "0.0.0.0:9181".parse().unwrap();
        let peer = "spiffe://example.org/validator";
        let parse = |listen, peers, endpoint| Config::parse(listen, peers, endpoint, api, metrics);
        assert!(parse(None, None, None).unwrap().is_none());
        assert!(parse(None, Some(peer), None).is_err());
        for peers in [
            None,
            Some(""),
            Some("spiffe://example.org/validator,"),
            Some("not-an-id"),
            Some("spiffe://example.org/a,spiffe://example.org/a"),
        ] {
            assert!(parse(Some("127.0.0.1:8443"), peers, None).is_err());
        }
        for listen in ["", "localhost:8443", "127.0.0.1:8081", "127.0.0.1:9181"] {
            assert!(parse(Some(listen), Some(peer), None).is_err());
        }
        assert!(parse(
            Some("127.0.0.1:8443"),
            Some(peer),
            Some("tcp://localhost:1")
        )
        .is_err());
        assert!(parse(Some("127.0.0.1:8443"), Some(peer), None)
            .unwrap()
            .is_some());
    }
}

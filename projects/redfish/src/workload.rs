//! Optional SPIFFE mTLS transport. Workload identity is an additional admission
//! check: the ordinary router still authenticates and authorizes every request.
//! No identity is taken from HTTP headers and no role is granted by this module.
use anyhow::{bail, ensure, Context, Result};
use axum::serve::Listener as AxumListener;
use futures::{future::BoxFuture, stream::FuturesUnordered, StreamExt};
use qrab_spiffe::{Acceptor, Identity, SpiffeId};
use std::{
    io,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context as TaskContext, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::{TcpListener, TcpStream},
    sync::{OwnedSemaphorePermit, Semaphore},
};

/// Includes pending handshakes and established HTTP connections. A permit is
/// retained by the IO until Axum drops it, including upgraded/streaming bodies.
pub const MAX_CONNECTIONS: usize = 32;
/// Slow clients do not serialize other handshakes or allocate unbounded tasks.
pub const MAX_HANDSHAKES: usize = 8;

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

    /// Fails closed if no rotating identity or exact-peer TLS config is available.
    pub async fn bind(self) -> Result<Listener> {
        let identity = Identity::connect(&self.endpoint).await?;
        Listener::bind(self.listen, &identity, &self.peers).await
    }
}

type Accepted = Result<(Connection, SocketAddr), qrab_spiffe::Error>;

pub struct Listener {
    tcp: TcpListener,
    acceptor: Acceptor,
    handshakes: FuturesUnordered<BoxFuture<'static, Accepted>>,
    slots: Arc<Semaphore>,
    reserved: Option<OwnedSemaphorePermit>,
}

impl Listener {
    pub async fn bind(
        address: SocketAddr,
        identity: &Identity,
        peers: &[SpiffeId],
    ) -> Result<Self> {
        if peers.is_empty() {
            bail!("at least one exact workload peer is required");
        }
        let local_id = identity.id()?;
        ensure!(
            peers
                .iter()
                .all(|peer| peer.trust_domain() == local_id.trust_domain()),
            "workload peers must belong to the local SPIFFE trust domain"
        );
        let acceptor = identity.acceptor(peers)?;
        let tcp = TcpListener::bind(address).await?;
        Ok(Self {
            tcp,
            acceptor,
            handshakes: FuturesUnordered::new(),
            slots: Arc::new(Semaphore::new(MAX_CONNECTIONS)),
            reserved: None,
        })
    }
}

impl AxumListener for Listener {
    type Io = Connection;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            tokio::select! {
                completed = self.handshakes.next(), if !self.handshakes.is_empty() => {
                    match completed.expect("nonempty handshake set") {
                        Ok(connection) => return connection,
                        // Do not log certificates, peer identifiers or credentials.
                        Err(_) => tracing::debug!("workload TLS handshake rejected"),
                    }
                }
                permit = self.slots.clone().acquire_owned(),
                    if self.reserved.is_none() && self.handshakes.len() < MAX_HANDSHAKES => {
                    self.reserved = Some(permit.expect("private semaphore is never closed"));
                }
                accepted = self.tcp.accept(),
                    if self.reserved.is_some() && self.handshakes.len() < MAX_HANDSHAKES => {
                    match accepted {
                        Ok((socket, address)) => {
                            let permit = self.reserved.take().expect("reserved connection slot");
                            let acceptor = self.acceptor.clone();
                            self.handshakes.push(Box::pin(async move {
                                let stream = acceptor.accept(socket).await?;
                                Ok((Connection { stream, _permit: permit }, address))
                            }));
                        }
                        Err(error) => {
                            tracing::warn!(%error, "workload listener accept failed");
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    }
                }
            }
        }
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.tcp.local_addr()
    }
}

/// An authenticated TLS 1.3 stream. The shared qrab-spiffe acceptor enforces
/// mandatory certificates, exact peer IDs, local trust domain and handshake timeout.
pub struct Connection {
    stream: qrab_spiffe::server::TlsStream<TcpStream>,
    _permit: OwnedSemaphorePermit,
}

impl AsyncRead for Connection {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for Connection {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
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

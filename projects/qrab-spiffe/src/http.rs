//! Bounded SPIFFE TLS HTTP listener and exact-peer HTTPS client configuration.
use crate::{Acceptor, Identity, SpiffeId};
use axum::serve::Listener as AxumListener;
use futures::{future::BoxFuture, stream::FuturesUnordered, StreamExt};
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

/// Includes pending handshakes and response/upgraded stream lifetimes.
pub const MAX_CONNECTIONS: usize = 32;
/// Maximum concurrently polled handshakes; each has the shared TLS deadline.
pub const MAX_HANDSHAKES: usize = 8;

type Accepted = crate::Result<(Connection, SocketAddr)>;

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
    ) -> crate::Result<Self> {
        if peers.is_empty() {
            return Err(crate::Error::Config(
                "at least one exact workload peer is required".into(),
            ));
        }
        let local_id = identity.id()?;
        if !peers
            .iter()
            .all(|peer| peer.trust_domain() == local_id.trust_domain())
        {
            return Err(crate::Error::Config(
                "workload peers must belong to the local SPIFFE trust domain".into(),
            ));
        }
        Self::bind_acceptor(address, identity.acceptor(peers)?).await
    }

    /// Bounded listener with a dynamic exact-ID admission policy.
    pub async fn bind_authorized(
        address: SocketAddr,
        identity: &Identity,
        authorize: impl crate::Authorizer,
    ) -> crate::Result<Self> {
        Self::bind_acceptor(address, identity.acceptor_with_authorizer(authorize)?).await
    }

    async fn bind_acceptor(address: SocketAddr, acceptor: Acceptor) -> crate::Result<Self> {
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
                                let peer = Peer::from_stream(&stream)?;
                                Ok((Connection { stream, peer, _permit: permit }, address))
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
    stream: crate::server::TlsStream<TcpStream>,
    peer: Peer,
    _permit: OwnedSemaphorePermit,
}

/// Verified certificate identity for request-time authorization, never an HTTP
/// header. `valid()` also expires established keep-alive connections.
#[derive(Clone, Debug)]
pub struct Peer {
    pub id: SpiffeId,
    not_before: i64,
    not_after: i64,
}
impl Peer {
    fn from_stream(stream: &crate::server::TlsStream<TcpStream>) -> crate::Result<Self> {
        let der = stream
            .get_ref()
            .1
            .peer_certificates()
            .and_then(|c| c.first())
            .ok_or_else(|| crate::Error::Config("client certificate required".into()))?;
        let bad = || crate::Error::Config("invalid verified peer certificate".into());
        let id = spiffe::cert::spiffe_id_from_der(der.as_ref()).map_err(|_| bad())?;
        let (_, cert) = x509_parser::parse_x509_certificate(der.as_ref()).map_err(|_| bad())?;
        Ok(Self {
            id,
            not_before: cert.validity().not_before.timestamp(),
            not_after: cert.validity().not_after.timestamp(),
        })
    }
    pub fn valid(&self) -> bool {
        let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else {
            return false;
        };
        let Ok(now) = i64::try_from(now.as_secs()) else {
            return false;
        };
        self.not_before <= now && now < self.not_after
    }
}
impl axum::extract::connect_info::Connected<axum::serve::IncomingStream<'_, Listener>> for Peer {
    fn connect_info(stream: axum::serve::IncomingStream<'_, Listener>) -> Self {
        stream.io().peer.clone()
    }
}

/// Request-time certificate expiry guard for long-lived HTTP connections.
/// Apply only with into_make_service_with_connect_info::<Peer>().
pub async fn require_valid_peer(
    axum::extract::ConnectInfo(peer): axum::extract::ConnectInfo<Peer>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if !peer.valid() {
        return axum::response::IntoResponse::into_response(axum::http::StatusCode::UNAUTHORIZED);
    }
    next.run(request).await
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

/// Install before other rustls users in HTTP-client binaries. The SPIFFE SDK
/// builds configs from the process provider; filtering suites here makes a
/// TLS-1.2-only peer fail negotiation even with a preconfigured reqwest config.
/// A preinstalled incompatible provider is an error, never a silent downgrade.
pub fn install_tls13_provider() -> crate::Result<()> {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let mut provider = rustls::crypto::ring::default_provider();
        provider
            .cipher_suites
            .retain(|suite| suite.version().version == rustls::ProtocolVersion::TLSv1_3);
        let _ = provider.install_default();
    }
    let provider = rustls::crypto::CryptoProvider::get_default()
        .ok_or_else(|| crate::Error::Config("TLS provider unavailable".into()))?;
    if provider
        .cipher_suites
        .iter()
        .any(|suite| suite.version().version != rustls::ProtocolVersion::TLSv1_3)
    {
        return Err(crate::Error::Config("SPIFFE HTTPS clients require TLS-1.3-only provider initialization before other TLS clients".into()));
    }
    Ok(())
}

/// HTTPS-only client for one exact workload ID, without redirects or proxies.
/// Request timeout belongs to the caller so SSE can have its own lifetime.
pub fn client(identity: &Identity, peer: SpiffeId) -> crate::Result<reqwest::Client> {
    install_tls13_provider()?;
    let tls = identity.client_config(&[peer], &[b"http/1.1"])?;
    reqwest::Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .http1_only()
        .pool_max_idle_per_host(4)
        .pool_idle_timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(3))
        .use_preconfigured_tls(tls)
        .build()
        .map_err(|e| crate::Error::Config(e.to_string()))
}

/// Plain HTTP is confined to the local kernel/network namespace. Remote
/// application connections must use a workload TLS listener.
pub fn require_loopback(address: SocketAddr) -> crate::Result<()> {
    if !address.ip().is_loopback() {
        return Err(crate::Error::Config(
            "plain HTTP must bind literal loopback; use workload mTLS for network access".into(),
        ));
    }
    Ok(())
}

/// Restrict the ordinary listener to health in workload mode. The authenticated
/// application router remains available only through its TLS listener.
pub async fn health_only(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if request.method() != axum::http::Method::GET
        || request.uri().query().is_some()
        || !matches!(request.uri().path(), "/healthz" | "/readyz")
    {
        return axum::response::IntoResponse::into_response(axum::http::StatusCode::NOT_FOUND);
    }
    next.run(request).await
}

/// Fixed-path local exec probe. A single deadline covers connect, headers and
/// response; no identity, Kubernetes client or OIDC discovery is initialized.
pub async fn probe_health(address: SocketAddr, path: &str) -> crate::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    require_loopback(address)?;
    if !matches!(path, "/healthz" | "/readyz") {
        return Err(crate::Error::Config(
            "invalid local health probe path".into(),
        ));
    }
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut stream = TcpStream::connect(address).await?;
        stream
            .write_all(
                format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await?;
        let mut status = [0; 16];
        stream.read_exact(&mut status).await?;
        if !status.starts_with(b"HTTP/1.1 200 ") {
            return Err(crate::Error::Config("local service is not ready".into()));
        }
        Ok(())
    })
    .await
    .map_err(|_| crate::Error::Timeout)?
}

/// Optional extra listener configuration. Setting peers without a listener,
/// invalid IDs, duplicates or a colliding port refuses startup.
pub struct Config {
    pub address: SocketAddr,
    pub peers: Vec<SpiffeId>,
}
impl Config {
    pub fn from_env(prefix: &str, excluded_ports: &[u16]) -> crate::Result<Option<Self>> {
        let value = |name: String| match std::env::var(&name) {
            Ok(value) => Ok(Some(value)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(_) => Err(crate::Error::Config(format!("invalid {name}"))),
        };
        let listen = value(format!("{prefix}_WORKLOAD_LISTEN"))?;
        let peers = value(format!("{prefix}_WORKLOAD_PEERS"))?;
        Self::parse(listen.as_deref(), peers.as_deref(), excluded_ports)
    }
    pub fn parse(
        listen: Option<&str>,
        peers: Option<&str>,
        excluded_ports: &[u16],
    ) -> crate::Result<Option<Self>> {
        let bad = || {
            crate::Error::Config(
                "workload listener requires a distinct address and 1..64 unique exact peer IDs"
                    .into(),
            )
        };
        let Some(listen) = listen else {
            return if peers.is_none() {
                Ok(None)
            } else {
                Err(bad())
            };
        };
        let address: SocketAddr = listen.parse().map_err(|_| bad())?;
        if excluded_ports.contains(&address.port()) {
            return Err(bad());
        }
        let peers = peers
            .ok_or_else(bad)?
            .split(',')
            .map(|p| crate::id(p.trim()))
            .collect::<crate::Result<Vec<_>>>()?;
        if peers.is_empty()
            || peers.len() > 64
            || peers
                .iter()
                .enumerate()
                .any(|(i, p)| peers[..i].contains(p))
        {
            return Err(bad());
        }
        Ok(Some(Self { address, peers }))
    }
    pub async fn bind(self, identity: &Identity) -> crate::Result<Listener> {
        Listener::bind(self.address, identity, &self.peers).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optional_listener_configuration_never_ignores_partial_or_ambiguous_values() {
        let peer = "spiffe://yapplesauce.com/allowed";
        assert!(Config::parse(None, None, &[]).unwrap().is_none());
        assert!(Config::parse(None, Some(peer), &[]).is_err());
        for peers in [
            None,
            Some(""),
            Some("bad"),
            Some("spiffe://yapplesauce.com/a,spiffe://yapplesauce.com/a"),
            Some("spiffe://yapplesauce.com/a,"),
        ] {
            assert!(Config::parse(Some("127.0.0.1:8443"), peers, &[8080]).is_err());
        }
        assert!(Config::parse(Some("127.0.0.1:8080"), Some(peer), &[8080]).is_err());
        assert!(Config::parse(Some("127.0.0.1:8443"), Some(peer), &[8080])
            .unwrap()
            .is_some());
    }
    #[test]
    fn vm_service_ids_cannot_encode_wildcards_or_path_components() {
        assert_eq!(
            crate::vm_service_id("yapplesauce.com", "qrab", "uid-a", "bmc")
                .unwrap()
                .to_string(),
            "spiffe://yapplesauce.com/kubernetes/qrab/vm/uid-a/bmc"
        );
        for (namespace, uid, service) in [
            ("qrab", "../other", "bmc"),
            ("qrab", "uid%2fa", "redfish"),
            ("qrab/other", "uid-a", "bmc"),
            ("qrab", "uid-a", "manager"),
            ("qrab", "*", "bmc"),
            ("qrab", "", "bmc"),
        ] {
            assert!(crate::vm_service_id("yapplesauce.com", namespace, uid, service).is_err());
        }
    }
}

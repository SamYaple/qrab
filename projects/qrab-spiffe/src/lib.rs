//! SPIFFE X509-SVID mutual TLS for qrab components.
//!
//! One [`Identity`] wraps the pod's Workload API source. Every client and
//! server built from it authorizes an exact list of peer SPIFFE IDs within the
//! local trust domain, and every established stream is checked to be TLS 1.3.
//! Certificates rotate underneath without rebuilding configs.
use rustls::{pki_types::ServerName, ClientConfig, ProtocolVersion, ServerConfig};
use std::{sync::Arc, time::Duration};
use tokio::io::{AsyncRead, AsyncWrite};

pub use spiffe::{SpiffeId, X509Source};
pub use tokio_rustls::{client, server};

#[cfg(feature = "fixture")]
pub mod fixture;

/// The default Workload API socket that the csi.spiffe.io volume provides.
pub const DEFAULT_ENDPOINT: &str = "unix:/run/spire/agent.sock";
/// The environment variable naming the Workload API socket.
pub const ENDPOINT_ENV: &str = "QRAB_SPIFFE_ENDPOINT_SOCKET";
/// Bound on a single TLS handshake.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("SPIFFE Workload API identity unavailable")]
    Unavailable,
    #[error("the SPIFFE Workload API must be a Unix socket")]
    Endpoint,
    #[error("invalid SPIFFE ID {0:?}")]
    Id(String),
    #[error("SPIFFE TLS configuration: {0}")]
    Config(String),
    #[error("TLS handshake timed out")]
    Timeout,
    #[error("peer negotiated {0:?}; TLS 1.3 is required")]
    Version(Option<ProtocolVersion>),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Parse a SPIFFE ID, mapping the error to [`Error::Id`].
pub fn id(text: &str) -> Result<SpiffeId> {
    text.parse().map_err(|_| Error::Id(text.into()))
}

/// The pod's SPIFFE identity, backed by a rotating Workload API source.
#[derive(Clone)]
pub struct Identity {
    source: X509Source,
}

impl Identity {
    /// Connect to the Workload API at `endpoint` (`unix:` only) and wait up to
    /// 30 s for the first SVID.
    pub async fn connect(endpoint: &str) -> Result<Self> {
        if !endpoint.starts_with("unix:") {
            return Err(Error::Endpoint);
        }
        let source = X509Source::builder()
            .endpoint(endpoint)
            .initial_sync_timeout(Duration::from_secs(30))
            .shutdown_timeout(Some(Duration::from_secs(5)))
            .build()
            .await
            .map_err(|_| Error::Unavailable)?;
        Ok(Self { source })
    }

    /// Connect to the endpoint named by [`ENDPOINT_ENV`], else [`DEFAULT_ENDPOINT`].
    pub async fn from_env() -> Result<Self> {
        let endpoint = std::env::var(ENDPOINT_ENV).unwrap_or_else(|_| DEFAULT_ENDPOINT.into());
        Self::connect(&endpoint).await
    }

    /// The underlying source, for libraries that take one directly.
    pub fn source(&self) -> &X509Source {
        &self.source
    }

    /// This workload's current SPIFFE ID.
    pub fn id(&self) -> Result<SpiffeId> {
        let svid = self.source.svid().map_err(|_| Error::Unavailable)?;
        Ok(svid.spiffe_id().clone())
    }

    fn local(&self) -> Result<spiffe_rustls::TrustDomainPolicy> {
        Ok(spiffe_rustls::LocalOnly(self.id()?.trust_domain().clone()))
    }

    /// A client config that accepts only servers holding one of `peers`.
    pub fn client_config(&self, peers: &[SpiffeId], alpn: &[&[u8]]) -> Result<ClientConfig> {
        spiffe_rustls::mtls_client(self.source.clone())
            .authorize(spiffe_rustls::exact(peers.iter().cloned()).map_err(config)?)
            .trust_domain_policy(self.local()?)
            .with_alpn_protocols(alpn.iter().map(|p| p.to_vec()))
            .build()
            .map_err(config)
    }

    /// A server config that accepts only clients holding one of `peers`.
    /// Session resumption stays disabled, so every connection re-verifies.
    pub fn server_config(&self, peers: &[SpiffeId], alpn: &[&[u8]]) -> Result<ServerConfig> {
        spiffe_rustls::mtls_server(self.source.clone())
            .authorize(spiffe_rustls::exact(peers.iter().cloned()).map_err(config)?)
            .trust_domain_policy(self.local()?)
            .with_alpn_protocols(alpn.iter().map(|p| p.to_vec()))
            .build()
            .map_err(config)
    }

    /// A connector for raw streams to servers holding one of `peers`.
    pub fn connector(&self, peers: &[SpiffeId]) -> Result<Connector> {
        Ok(Connector(tokio_rustls::TlsConnector::from(Arc::new(
            self.client_config(peers, &[])?,
        ))))
    }

    /// An acceptor for raw streams from clients holding one of `peers`.
    pub fn acceptor(&self, peers: &[SpiffeId]) -> Result<Acceptor> {
        Ok(Acceptor(tokio_rustls::TlsAcceptor::from(Arc::new(
            self.server_config(peers, &[])?,
        ))))
    }
}

fn config(error: impl std::fmt::Display) -> Error {
    Error::Config(error.to_string())
}

fn tls13(version: Option<ProtocolVersion>) -> Result<()> {
    match version {
        Some(ProtocolVersion::TLSv1_3) => Ok(()),
        other => Err(Error::Version(other)),
    }
}

/// Dials mTLS streams; the peer's identity, not its address, is verified.
#[derive(Clone)]
pub struct Connector(tokio_rustls::TlsConnector);

impl Connector {
    pub async fn connect<IO>(&self, io: IO) -> Result<client::TlsStream<IO>>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
    {
        // The SPIFFE verifier ignores the server name; rustls still needs one.
        let name = ServerName::try_from("spiffe.invalid").expect("static server name");
        let stream = tokio::time::timeout(HANDSHAKE_TIMEOUT, self.0.connect(name, io))
            .await
            .map_err(|_| Error::Timeout)??;
        tls13(stream.get_ref().1.protocol_version())?;
        Ok(stream)
    }
}

/// Accepts mTLS streams from authorized peers only.
#[derive(Clone)]
pub struct Acceptor(tokio_rustls::TlsAcceptor);

impl Acceptor {
    pub async fn accept<IO>(&self, io: IO) -> Result<server::TlsStream<IO>>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
    {
        let stream = tokio::time::timeout(HANDSHAKE_TIMEOUT, self.0.accept(io))
            .await
            .map_err(|_| Error::Timeout)??;
        tls13(stream.get_ref().1.protocol_version())?;
        Ok(stream)
    }
}

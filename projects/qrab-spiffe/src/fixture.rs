//! A local Workload API and certificate authority for tests.
//!
//! `Authority::svid` issues SPIFFE-conformant X509-SVIDs; `Workload` serves
//! them over the real Workload API gRPC stream on a Unix socket, so code under
//! test runs the same `X509Source` path as in production.
use prost::Message;
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa,
    Issuer, KeyPair, KeyUsagePurpose, SanType,
};
use std::{
    convert::Infallible,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use time::{Duration, OffsetDateTime};
use tokio::{net::UnixListener, sync::watch};
use tokio_stream::{
    wrappers::{UnixListenerStream, WatchStream},
    Stream, StreamExt,
};
use tonic::{codegen::http, Request, Response, Status};

#[derive(Clone, PartialEq, Message)]
pub struct X509Request {}

#[derive(Clone, PartialEq, Message)]
pub struct Svid {
    #[prost(string, tag = "1")]
    pub spiffe_id: String,
    #[prost(bytes = "vec", tag = "2")]
    pub x509_svid: Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub x509_svid_key: Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    pub bundle: Vec<u8>,
    #[prost(string, tag = "5")]
    pub hint: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct X509Response {
    #[prost(message, repeated, tag = "1")]
    pub svids: Vec<Svid>,
    #[prost(bytes = "vec", repeated, tag = "2")]
    pub crl: Vec<Vec<u8>>,
    #[prost(map = "string, bytes", tag = "3")]
    pub federated_bundles: std::collections::HashMap<String, Vec<u8>>,
}

/// A throwaway CA for one trust domain.
pub struct Authority {
    params: CertificateParams,
    key: KeyPair,
    der: Vec<u8>,
}

impl Default for Authority {
    fn default() -> Self {
        Self::new()
    }
}

impl Authority {
    pub fn new() -> Self {
        let key = KeyPair::generate().expect("CA key");
        let mut params = CertificateParams::new(Vec::<String>::new()).expect("CA params");
        params
            .distinguished_name
            .push(DnType::CommonName, "qrab test root");
        params.not_before = OffsetDateTime::now_utc() - Duration::minutes(1);
        params.not_after = OffsetDateTime::now_utc() + Duration::hours(1);
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign];
        let der = params.self_signed(&key).expect("CA").der().to_vec();
        Self { params, key, der }
    }

    /// An SVID for `id`, valid for ten minutes.
    pub fn svid(&self, id: &str) -> Svid {
        let key = KeyPair::generate().expect("SVID key");
        let mut params = CertificateParams::new(Vec::<String>::new()).expect("SVID params");
        params.distinguished_name = DistinguishedName::new();
        params.subject_alt_names = vec![SanType::URI(id.try_into().expect("URI SAN"))];
        params.not_before = OffsetDateTime::now_utc() - Duration::minutes(1);
        params.not_after = OffsetDateTime::now_utc() + Duration::minutes(10);
        // spiffe rejects a leaf without an explicit CA=false constraint.
        params.is_ca = IsCa::ExplicitNoCa;
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![
            ExtendedKeyUsagePurpose::ClientAuth,
            ExtendedKeyUsagePurpose::ServerAuth,
        ];
        let issuer = Issuer::from_params(&self.params, &self.key);
        let mut der = params
            .signed_by(&key, &issuer)
            .expect("SVID")
            .der()
            .to_vec();
        der.extend_from_slice(&self.der);
        spiffe::X509Svid::parse_from_der(&der, &key.serialize_der())
            .expect("fixture SVID must satisfy SPIFFE constraints");
        Svid {
            spiffe_id: id.into(),
            x509_svid: der,
            x509_svid_key: key.serialize_der(),
            bundle: self.der.clone(),
            hint: String::new(),
        }
    }
}

#[derive(Clone)]
struct Api(Arc<watch::Sender<X509Response>>);

impl tonic::server::NamedService for Api {
    const NAME: &'static str = "SpiffeWorkloadAPI";
}

struct Fetch(Arc<watch::Sender<X509Response>>);

type FetchStream = Pin<Box<dyn Stream<Item = Result<X509Response, Status>> + Send>>;

impl tonic::server::ServerStreamingService<X509Request> for Fetch {
    type Response = X509Response;
    type ResponseStream = FetchStream;
    type Future =
        Pin<Box<dyn std::future::Future<Output = Result<Response<FetchStream>, Status>> + Send>>;

    fn call(&mut self, request: Request<X509Request>) -> Self::Future {
        let sender = self.0.clone();
        Box::pin(async move {
            if request
                .metadata()
                .get("workload.spiffe.io")
                .and_then(|v| v.to_str().ok())
                != Some("true")
            {
                return Err(Status::permission_denied(
                    "Workload API security header required",
                ));
            }
            let stream = WatchStream::new(sender.subscribe()).map(Ok);
            Ok(Response::new(Box::pin(stream) as FetchStream))
        })
    }
}

impl<B> tower::Service<http::Request<B>> for Api
where
    B: tonic::codegen::Body + Send + 'static,
    B::Error: Into<tonic::codegen::StdError> + Send + 'static,
{
    type Response = http::Response<tonic::body::Body>;
    type Error = Infallible;
    type Future = tonic::codegen::BoxFuture<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: http::Request<B>) -> Self::Future {
        if request.uri().path() != "/SpiffeWorkloadAPI/FetchX509SVID" {
            return Box::pin(async {
                Ok(http::Response::builder()
                    .status(200)
                    .header("grpc-status", "12")
                    .header("content-type", "application/grpc")
                    .body(tonic::body::Body::empty())
                    .expect("unimplemented response"))
            });
        }
        let method = Fetch(self.0.clone());
        Box::pin(async move {
            let mut grpc = tonic::server::Grpc::new(tonic_prost::ProstCodec::default());
            Ok(grpc.server_streaming(method, request).await)
        })
    }
}

/// A Workload API serving fixed SVIDs on a temporary Unix socket.
pub struct Workload {
    _directory: tempfile::TempDir,
    /// The `unix:` endpoint for [`crate::Identity::connect`].
    pub endpoint: String,
    sender: Arc<watch::Sender<X509Response>>,
    server: tokio::task::JoinHandle<()>,
}

impl Workload {
    /// Serve `svids`; call from inside a Tokio runtime.
    pub fn serve(svids: Vec<Svid>) -> Self {
        let directory = tempfile::tempdir().expect("workload dir");
        let path = directory.path().join("agent.sock");
        let listener = UnixListener::bind(&path).expect("workload socket");
        let (sender, _) = watch::channel(response(svids));
        let sender = Arc::new(sender);
        let api = Api(sender.clone());
        let server = tokio::spawn(async move {
            let _ = tonic::transport::Server::builder()
                .add_service(api)
                .serve_with_incoming(UnixListenerStream::new(listener))
                .await;
        });
        Self {
            endpoint: format!("unix://{}", path.display()),
            _directory: directory,
            sender,
            server,
        }
    }

    /// Replace the served SVIDs (rotation).
    pub fn replace(&self, svids: Vec<Svid>) {
        self.sender.send_replace(response(svids));
    }
}

fn response(svids: Vec<Svid>) -> X509Response {
    X509Response {
        svids,
        crl: Vec::new(),
        federated_bundles: Default::default(),
    }
}

impl Drop for Workload {
    fn drop(&mut self) {
        self.server.abort();
    }
}

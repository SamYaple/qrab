//! qrab-tpm-issuer signs EK and DevID certificates for a VM's vTPM, only for
//! the VM named by the caller's `vm/<uid>` SVID.
//!
//! # Deployment contract
//!
//! The CA directory (`--ca-dir`, a mounted Secret) holds four files, each key
//! a PKCS#8 PEM (`BEGIN PRIVATE KEY`, RSA or ECDSA P-256/P-384) and each
//! certificate exactly one PEM CA certificate (`cA=TRUE`, keyCertSign):
//!
//! | File | Role | SPIRE server setting |
//! | --- | --- | --- |
//! | `ek-ca.pem`, `ek-ca.key` | qrab endorsement CA | `endorsement_ca_path` |
//! | `devid-ca.pem`, `devid-ca.key` | qrab DevID CA | `devid_ca_path` |
//!
//! The state directory (`--state-dir`, a persistent volume) holds one
//! `<uid>.json` record per VM with the certified `TPMT_PUBLIC`s and the
//! issued certificates.
//!
//! # Wire API
//!
//! HTTPS over SPIFFE mTLS (TLS 1.3). The client must hold exactly
//! `spiffe://yapplesauce.com/kubernetes/qrab/vm/<uuid>`; the uid always comes
//! from that certificate. Public areas are base64 `TPM2B_PUBLIC` (as written
//! by `tpm2_createek -u` / `tpm2_create -u`) or bare `TPMT_PUBLIC`.
//!
//! - `POST /v1/ek` `{"ek_public": b64}` returns an EK certificate.
//! - `POST /v1/devid` `{"ldevid_public": b64, "ek_public": b64}` returns a
//!   DevID certificate with subject CN `qrab-vm-<uid>`, only once an EK
//!   certificate exists for the uid and for that same EK.
//!
//! Both answer [`IssueResponse`]. A repeat with the same key returns the
//! stored certificate; a different key is refused (409) unless
//! `--allow-rekey`. Errors are `{"error": text}`.
pub mod cert;
pub mod der;
pub mod store;
pub mod tpm;

use axum::{
    extract::{ConnectInfo, DefaultBodyLimit, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use qrab_spiffe::{http::Peer, SpiffeId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// The only admitted peers: `<prefix><uuid>`.
pub const VM_ID_PREFIX: &str = "spiffe://yapplesauce.com/kubernetes/qrab/vm/";
pub const EK_CA: &str = "ek-ca";
pub const DEVID_CA: &str = "devid-ca";
pub const MAX_BODY: usize = 16 * 1024;

/// A Kubernetes VM UID in canonical lowercase UUID form. Uppercase is refused
/// so one VM can never own two state records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VmUid(String);

impl VmUid {
    /// ```
    /// use qrab_tpm_issuer::VmUid;
    /// assert!(VmUid::parse("0b6f2a3e-5c1d-4e8f-9a7b-1c2d3e4f5a6b").is_some());
    /// assert!(VmUid::parse("0B6F2A3E-5C1D-4E8F-9A7B-1C2D3E4F5A6B").is_none());
    /// assert!(VmUid::parse("0b6f2a3e5c1d4e8f9a7b1c2d3e4f5a6b").is_none());
    /// assert!(VmUid::parse("../6f2a3e-5c1d-4e8f-9a7b-1c2d3e4f5a6b").is_none());
    /// ```
    pub fn parse(text: &str) -> Option<Self> {
        let ok = text.len() == 36
            && text.bytes().enumerate().all(|(i, b)| match i {
                8 | 13 | 18 | 23 => b == b'-',
                _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
            });
        ok.then(|| Self(text.into()))
    }

    /// The uid of an exact `vm/<uuid>` SPIFFE ID, else `None`.
    ///
    /// ```
    /// use qrab_tpm_issuer::VmUid;
    /// let id = |s: &str| qrab_spiffe::id(s).unwrap();
    /// let vm = "spiffe://yapplesauce.com/kubernetes/qrab/vm/0b6f2a3e-5c1d-4e8f-9a7b-1c2d3e4f5a6b";
    /// assert!(VmUid::from_spiffe_id(&id(vm)).is_some());
    /// assert!(VmUid::from_spiffe_id(&id(&format!("{vm}/bmc"))).is_none());
    /// assert!(VmUid::from_spiffe_id(&id("spiffe://yapplesauce.com/kubernetes/qrab/nic/0b6f2a3e-5c1d-4e8f-9a7b-1c2d3e4f5a6b")).is_none());
    /// assert!(VmUid::from_spiffe_id(&id("spiffe://example.org/kubernetes/qrab/vm/0b6f2a3e-5c1d-4e8f-9a7b-1c2d3e4f5a6b")).is_none());
    /// ```
    pub fn from_spiffe_id(id: &SpiffeId) -> Option<Self> {
        Self::parse(id.to_string().strip_prefix(VM_ID_PREFIX)?)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Ek,
    Devid,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Ek => "ek",
            Kind::Devid => "devid",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IssueError {
    #[error("malformed {0}: {1}")]
    Malformed(&'static str, String),
    #[error("{0}")]
    Profile(&'static str),
    #[error("no EK certificate has been issued for this VM")]
    NoEk,
    #[error("ek_public is not the EK certified for this VM")]
    EkMismatch,
    #[error("a different {0} key is already certified for this VM; re-keying is disabled")]
    Rekey(&'static str),
    #[error("state store: {0}")]
    Store(#[from] std::io::Error),
    #[error("signing failed")]
    Sign(#[from] rcgen::Error),
}

impl IssueError {
    fn status(&self) -> StatusCode {
        match self {
            Self::Malformed(..) => StatusCode::BAD_REQUEST,
            Self::Profile(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::NoEk | Self::EkMismatch | Self::Rekey(_) => StatusCode::CONFLICT,
            Self::Store(_) | Self::Sign(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// A certificate for one VM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issued {
    pub der: Vec<u8>,
    /// True when an identical earlier request's certificate was returned.
    pub existing: bool,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The two CAs and the issuance state.
pub struct Issuer {
    ek_ca: cert::Ca,
    devid_ca: cert::Ca,
    store: store::Store,
    allow_rekey: bool,
    lock: Mutex<()>,
}

fn public(field: &'static str, bytes: &[u8]) -> Result<tpm::Public, IssueError> {
    tpm::Public::parse(bytes).map_err(|e| IssueError::Malformed(field, e.to_string()))
}

impl Issuer {
    pub fn new(
        ek_ca: cert::Ca,
        devid_ca: cert::Ca,
        store: store::Store,
        allow_rekey: bool,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !ek_ca.same_key(&devid_ca),
            "the endorsement and DevID CAs must be distinct"
        );
        Ok(Self {
            ek_ca,
            devid_ca,
            store,
            allow_rekey,
            lock: Mutex::new(()),
        })
    }

    pub fn load(ca_dir: &Path, state_dir: &Path, allow_rekey: bool) -> anyhow::Result<Self> {
        Self::new(
            cert::Ca::load(ca_dir, EK_CA)?,
            cert::Ca::load(ca_dir, DEVID_CA)?,
            store::Store::open(state_dir)?,
            allow_rekey,
        )
    }

    pub fn ca(&self, kind: Kind) -> &cert::Ca {
        match kind {
            Kind::Ek => &self.ek_ca,
            Kind::Devid => &self.devid_ca,
        }
    }

    fn audit(uid: &VmUid, kind: Kind, result: &Result<Issued, IssueError>) {
        match result {
            Ok(issued) => tracing::info!(
                target: "audit",
                uid = uid.as_str(),
                kind = kind.as_str(),
                sha256 = sha256_hex(&issued.der),
                existing = issued.existing,
                "certificate issued"
            ),
            Err(error) => tracing::warn!(
                target: "audit",
                uid = uid.as_str(),
                kind = kind.as_str(),
                %error,
                "certificate refused"
            ),
        }
    }

    pub fn issue_ek(&self, uid: &VmUid, ek_public: &[u8]) -> Result<Issued, IssueError> {
        let result = self.ek(uid, ek_public);
        Self::audit(uid, Kind::Ek, &result);
        result
    }

    fn ek(&self, uid: &VmUid, ek_public: &[u8]) -> Result<Issued, IssueError> {
        let ek = public("ek_public", ek_public)?;
        ek.check_ek().map_err(IssueError::Profile)?;
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut record = self.store.load(uid)?;
        if let Some(entry) = &record.ek {
            if entry.public == ek.tpmt {
                return Ok(Issued {
                    der: entry.certificate.clone(),
                    existing: true,
                });
            }
            if !self.allow_rekey {
                return Err(IssueError::Rekey("EK"));
            }
        }
        let der = cert::ek(&self.ek_ca, uid, &ek)?;
        record.ek = Some(store::Entry {
            public: ek.tpmt,
            certificate: der.clone(),
        });
        // The old DevID was bound to the old EK.
        record.devid = None;
        self.store.save(uid, &record)?;
        Ok(Issued {
            der,
            existing: false,
        })
    }

    pub fn issue_devid(
        &self,
        uid: &VmUid,
        ldevid_public: &[u8],
        ek_public: &[u8],
    ) -> Result<Issued, IssueError> {
        let result = self.devid(uid, ldevid_public, ek_public);
        Self::audit(uid, Kind::Devid, &result);
        result
    }

    fn devid(
        &self,
        uid: &VmUid,
        ldevid_public: &[u8],
        ek_public: &[u8],
    ) -> Result<Issued, IssueError> {
        let devid = public("ldevid_public", ldevid_public)?;
        devid.check_devid().map_err(IssueError::Profile)?;
        let ek = public("ek_public", ek_public)?;
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut record = self.store.load(uid)?;
        let certified = record.ek.as_ref().ok_or(IssueError::NoEk)?;
        if certified.public != ek.tpmt {
            return Err(IssueError::EkMismatch);
        }
        if let Some(entry) = &record.devid {
            if entry.public == devid.tpmt {
                return Ok(Issued {
                    der: entry.certificate.clone(),
                    existing: true,
                });
            }
            if !self.allow_rekey {
                return Err(IssueError::Rekey("DevID"));
            }
        }
        let der = cert::devid(&self.devid_ca, uid, &devid)?;
        record.devid = Some(store::Entry {
            public: devid.tpmt,
            certificate: der.clone(),
        });
        self.store.save(uid, &record)?;
        Ok(Issued {
            der,
            existing: false,
        })
    }
}

mod b64 {
    use super::*;
    pub fn decode<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        STANDARD
            .decode(String::deserialize(d)?)
            .map_err(serde::de::Error::custom)
    }
}

/// `POST /v1/ek` body. Unknown fields, such as a `uid`, are refused.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EkRequest {
    #[serde(deserialize_with = "b64::decode")]
    pub ek_public: Vec<u8>,
}

/// `POST /v1/devid` body. Unknown fields, such as a `uid`, are refused.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DevidRequest {
    #[serde(deserialize_with = "b64::decode")]
    pub ldevid_public: Vec<u8>,
    #[serde(deserialize_with = "b64::decode")]
    pub ek_public: Vec<u8>,
}

/// Both endpoints' success body.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueResponse {
    pub uid: String,
    /// `ek` or `devid`.
    pub kind: String,
    /// Base64 DER: the EK certificate for NV index 0x01c00002.
    pub certificate: String,
    /// The same certificate as PEM: SPIRE's `devid_cert_path` format.
    pub certificate_pem: String,
    /// The issuing CA certificate (PEM).
    pub ca_certificate_pem: String,
    /// Lowercase hex SHA-256 of the DER certificate, as audit-logged.
    pub sha256: String,
    /// True when an earlier identical request's certificate was returned.
    pub existing: bool,
}

fn respond(
    issuer: &Issuer,
    uid: &VmUid,
    kind: Kind,
    result: Result<Issued, IssueError>,
) -> Response {
    match result {
        Ok(issued) => Json(IssueResponse {
            uid: uid.as_str().into(),
            kind: kind.as_str().into(),
            certificate: STANDARD.encode(&issued.der),
            certificate_pem: cert::pem("CERTIFICATE", &issued.der),
            ca_certificate_pem: issuer.ca(kind).pem().into(),
            sha256: sha256_hex(&issued.der),
            existing: issued.existing,
        })
        .into_response(),
        Err(error) => (
            error.status(),
            Json(serde_json::json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

async fn ek(
    State(issuer): State<Arc<Issuer>>,
    ConnectInfo(peer): ConnectInfo<Peer>,
    Json(request): Json<EkRequest>,
) -> Response {
    // The handshake authorizer already admitted only vm/<uuid>.
    let Some(uid) = VmUid::from_spiffe_id(&peer.id) else {
        return StatusCode::FORBIDDEN.into_response();
    };
    let result = issuer.issue_ek(&uid, &request.ek_public);
    respond(&issuer, &uid, Kind::Ek, result)
}

async fn devid(
    State(issuer): State<Arc<Issuer>>,
    ConnectInfo(peer): ConnectInfo<Peer>,
    Json(request): Json<DevidRequest>,
) -> Response {
    // The handshake authorizer already admitted only vm/<uuid>.
    let Some(uid) = VmUid::from_spiffe_id(&peer.id) else {
        return StatusCode::FORBIDDEN.into_response();
    };
    let result = issuer.issue_devid(&uid, &request.ldevid_public, &request.ek_public);
    respond(&issuer, &uid, Kind::Devid, result)
}

/// The HTTP routes. Serve with `into_make_service_with_connect_info::<Peer>()`
/// on a [`bind`] listener.
pub fn router(issuer: Arc<Issuer>) -> Router {
    Router::new()
        .route("/v1/ek", post(ek))
        .route("/v1/devid", post(devid))
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .layer(axum::middleware::from_fn(
            qrab_spiffe::http::require_valid_peer,
        ))
        .with_state(issuer)
}

/// A SPIFFE mTLS listener whose handshake admits only exact `vm/<uuid>` IDs.
pub async fn bind(
    address: SocketAddr,
    identity: &qrab_spiffe::Identity,
) -> qrab_spiffe::Result<qrab_spiffe::http::Listener> {
    qrab_spiffe::http::Listener::bind_authorized(address, identity, |id: &SpiffeId| {
        VmUid::from_spiffe_id(id).is_some()
    })
    .await
}

/// Command-line configuration; each flag falls back to its environment variable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub listen: SocketAddr,
    pub ca_dir: PathBuf,
    pub state_dir: PathBuf,
    pub allow_rekey: bool,
}

pub const USAGE: &str =
    "usage: qrab-tpm-issuer [--listen ADDR] [--ca-dir DIR] [--state-dir DIR] [--allow-rekey]

  --listen ADDR     SPIFFE mTLS listen address (QRAB_TPM_ISSUER_LISTEN, default 0.0.0.0:8443)
  --ca-dir DIR      ek-ca.{pem,key} and devid-ca.{pem,key} (QRAB_TPM_ISSUER_CA_DIR,
                    default /etc/qrab-tpm-issuer/ca)
  --state-dir DIR   per-VM issuance records (QRAB_TPM_ISSUER_STATE_DIR,
                    default /var/lib/qrab-tpm-issuer)
  --allow-rekey     certify a different key for a VM that already has one
                    (QRAB_TPM_ISSUER_ALLOW_REKEY=true)

The Workload API socket is QRAB_SPIFFE_ENDPOINT_SOCKET (default unix:/run/spire/agent.sock).";

impl Config {
    /// ```
    /// use qrab_tpm_issuer::Config;
    /// let env = |name: &str| (name == "QRAB_TPM_ISSUER_STATE_DIR").then(|| "/state".to_string());
    /// let config = Config::parse(["--allow-rekey", "--listen", "127.0.0.1:9000"].map(String::from), env).unwrap();
    /// assert_eq!(config.state_dir, std::path::PathBuf::from("/state"));
    /// assert_eq!(config.listen.port(), 9000);
    /// assert!(config.allow_rekey);
    /// assert!(Config::parse(["--bogus".to_string()], |_| None).is_err());
    /// ```
    pub fn parse(
        args: impl IntoIterator<Item = String>,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, String> {
        let mut listen = env("QRAB_TPM_ISSUER_LISTEN").unwrap_or_else(|| "0.0.0.0:8443".into());
        let mut ca_dir =
            env("QRAB_TPM_ISSUER_CA_DIR").unwrap_or_else(|| "/etc/qrab-tpm-issuer/ca".into());
        let mut state_dir =
            env("QRAB_TPM_ISSUER_STATE_DIR").unwrap_or_else(|| "/var/lib/qrab-tpm-issuer".into());
        let mut allow_rekey = match env("QRAB_TPM_ISSUER_ALLOW_REKEY").as_deref() {
            None | Some("" | "0" | "false") => false,
            Some("1" | "true") => true,
            Some(_) => return Err("QRAB_TPM_ISSUER_ALLOW_REKEY must be true or false".into()),
        };
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
            match arg.as_str() {
                "--listen" => listen = value("--listen")?,
                "--ca-dir" => ca_dir = value("--ca-dir")?,
                "--state-dir" => state_dir = value("--state-dir")?,
                "--allow-rekey" => allow_rekey = true,
                other => return Err(format!("unknown argument {other:?}\n{USAGE}")),
            }
        }
        Ok(Self {
            listen: listen
                .parse()
                .map_err(|_| format!("invalid listen address {listen:?}"))?,
            ca_dir: ca_dir.into(),
            state_dir: state_dir.into(),
            allow_rekey,
        })
    }
}

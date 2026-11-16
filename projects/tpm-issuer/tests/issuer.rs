//! End-to-end: real SPIFFE mTLS listener, real Workload API fixture, swtpm
//! public areas, and `openssl verify` against the two CA certificates.
//!
//! `tests/data/*.pub` are TPM2B_PUBLIC blobs from swtpm 0.10.2 and
//! tpm2-tools 5.7.1:
//!
//! ```text
//! swtpm socket --tpm2 --tpmstate dir=state --server type=tcp,port=23321 \
//!   --ctrl type=tcp,port=23322 --flags not-need-init,startup-clear --daemon
//! export TPM2TOOLS_TCTI=swtpm:host=127.0.0.1,port=23321
//! tpm2_createek -c ek-rsa.ctx -G rsa -u ek-rsa.pub
//! tpm2_createek -c ek-ecc.ctx -G ecc -u ek-ecc.pub
//! tpm2_flushcontext -t
//! tpm2_createprimary -C o -G rsa2048:null:aes128cfb \
//!   -a "fixedtpm|fixedparent|sensitivedataorigin|userwithauth|restricted|decrypt|noda" -c srk.ctx
//! tpm2_create -C srk.ctx -G rsa2048:rsassa-sha256:null \
//!   -a "fixedtpm|fixedparent|sensitivedataorigin|userwithauth|sign" -u devid-rsa.pub -r devid-rsa.priv
//! tpm2_flushcontext -t   # then recreate srk.ctx
//! tpm2_create -C srk.ctx -G ecc256:ecdsa-sha256:null \
//!   -a "fixedtpm|fixedparent|sensitivedataorigin|userwithauth|sign" -u devid-ecc.pub -r devid-ecc.priv
//! ```
use base64::{engine::general_purpose::STANDARD, Engine};
use qrab_spiffe::{
    fixture::{Authority, Workload},
    Identity,
};
use qrab_tpm_issuer::{IssueResponse, Issuer};
use rcgen::{
    BasicConstraints, CertificateParams, DnType, IsCa, KeyIdMethod, KeyPair, KeyUsagePurpose,
};
use std::{path::Path, process::Command, sync::Arc};
use x509_parser::prelude::*;

const EK_RSA: &[u8] = include_bytes!("data/ek-rsa.pub");
const EK_ECC: &[u8] = include_bytes!("data/ek-ecc.pub");
const DEVID_RSA: &[u8] = include_bytes!("data/devid-rsa.pub");
const DEVID_ECC: &[u8] = include_bytes!("data/devid-ecc.pub");

const UID: &str = "0b6f2a3e-5c1d-4e8f-9a7b-1c2d3e4f5a6b";
const OTHER_UID: &str = "7d1e9c40-2b3a-4f5e-8d6c-9a0b1c2d3e4f";
const SERVER_ID: &str = "spiffe://yapplesauce.com/kubernetes/qrab-tpm-issuer/tpm-issuer";

fn vm(uid: &str) -> String {
    format!("spiffe://yapplesauce.com/kubernetes/qrab/vm/{uid}")
}

/// A CA shaped like the production Secret: self-signed ECDSA P-256,
/// critical CA:TRUE pathlen:0, critical keyCertSign+cRLSign, SKI, 10 years.
fn write_ca(dir: &Path, name: &str, cn: &str) {
    let key = KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
    let mut params = CertificateParams::default();
    params.distinguished_name = rcgen::DistinguishedName::new();
    params.distinguished_name.push(DnType::CommonName, cn);
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    params.key_identifier_method = KeyIdMethod::Sha256;
    params.not_before = ::time::OffsetDateTime::now_utc() - ::time::Duration::days(1);
    params.not_after = params.not_before + ::time::Duration::days(3650);
    let cert = params.self_signed(&key).unwrap();
    std::fs::write(dir.join(format!("{name}.pem")), cert.pem()).unwrap();
    std::fs::write(dir.join(format!("{name}.key")), key.serialize_pem()).unwrap();
}

fn ca_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write_ca(dir.path(), "ek-ca", "qrab-endorsement-ca-1");
    write_ca(dir.path(), "devid-ca", "qrab-devid-ca-1");
    dir
}

struct Harness {
    ca: tempfile::TempDir,
    state: tempfile::TempDir,
    authority: Authority,
    address: std::net::SocketAddr,
    _server_api: Workload,
    _task: tokio::task::JoinHandle<()>,
}

impl Harness {
    async fn start(allow_rekey: bool) -> Self {
        qrab_spiffe::http::install_tls13_provider().unwrap();
        let ca = ca_dir();
        let state = tempfile::tempdir().unwrap();
        let issuer =
            Arc::new(Issuer::load(ca.path(), &state.path().join("records"), allow_rekey).unwrap());
        let authority = Authority::new();
        let server_api = Workload::serve(vec![authority.svid(SERVER_ID)]);
        let identity = Identity::connect(&server_api.endpoint).await.unwrap();
        let listener = qrab_tpm_issuer::bind("127.0.0.1:0".parse().unwrap(), &identity)
            .await
            .unwrap();
        let address = axum::serve::Listener::local_addr(&listener).unwrap();
        let task = tokio::spawn(async move {
            axum::serve(
                listener,
                qrab_tpm_issuer::router(issuer)
                    .into_make_service_with_connect_info::<qrab_spiffe::http::Peer>(),
            )
            .await
            .unwrap();
        });
        Self {
            ca,
            state,
            authority,
            address,
            _server_api: server_api,
            _task: task,
        }
    }

    async fn client(&self, id: &str) -> (Workload, reqwest::Client) {
        let api = Workload::serve(vec![self.authority.svid(id)]);
        let identity = Identity::connect(&api.endpoint).await.unwrap();
        let client =
            qrab_spiffe::http::client(&identity, qrab_spiffe::id(SERVER_ID).unwrap()).unwrap();
        (api, client)
    }

    async fn post(
        &self,
        client: &reqwest::Client,
        path: &str,
        body: serde_json::Value,
    ) -> reqwest::Result<(u16, serde_json::Value)> {
        let response = client
            .post(format!("https://{}{path}", self.address))
            .header("content-type", "application/json")
            .body(body.to_string())
            .send()
            .await?;
        let status = response.status().as_u16();
        let text = response.text().await?;
        Ok((
            status,
            serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text)),
        ))
    }

    fn record_exists(&self, uid: &str) -> bool {
        self.state
            .path()
            .join("records")
            .join(format!("{uid}.json"))
            .exists()
    }
}

fn b64(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

fn ek_body(ek: &[u8]) -> serde_json::Value {
    serde_json::json!({ "ek_public": b64(ek) })
}

fn devid_body(devid: &[u8], ek: &[u8]) -> serde_json::Value {
    serde_json::json!({ "ldevid_public": b64(devid), "ek_public": b64(ek) })
}

fn issued(value: serde_json::Value) -> IssueResponse {
    serde_json::from_value(value).unwrap()
}

fn common_name(der: &[u8]) -> String {
    let (_, cert) = parse_x509_certificate(der).unwrap();
    let cn = cert
        .subject()
        .iter_common_name()
        .next()
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    cn
}

fn der(response: &IssueResponse) -> Vec<u8> {
    STANDARD.decode(&response.certificate).unwrap()
}

#[tokio::test]
async fn vm_peer_gets_ek_then_devid_for_its_own_uid_only() {
    let h = Harness::start(false).await;
    let (_api, client) = h.client(&vm(UID)).await;

    // DevID before EK is refused, and records nothing.
    let (status, body) = h
        .post(&client, "/v1/devid", devid_body(DEVID_RSA, EK_RSA))
        .await
        .unwrap();
    assert_eq!(status, 409, "{body}");
    assert!(body["error"].as_str().unwrap().contains("no EK"));
    assert!(!h.record_exists(UID));

    // A body naming another uid is refused outright.
    let mut spoof = ek_body(EK_RSA);
    spoof["uid"] = OTHER_UID.into();
    let (status, _) = h.post(&client, "/v1/ek", spoof).await.unwrap();
    assert!((400..500).contains(&status), "{status}");
    assert!(!h.record_exists(OTHER_UID) && !h.record_exists(UID));

    let (status, body) = h.post(&client, "/v1/ek", ek_body(EK_RSA)).await.unwrap();
    assert_eq!(status, 200, "{body}");
    let ek = issued(body);
    assert_eq!(
        (ek.uid.as_str(), ek.kind.as_str(), ek.existing),
        (UID, "ek", false)
    );
    assert_eq!(common_name(&der(&ek)), format!("qrab-vm-{UID}"));
    assert_eq!(ek.sha256, qrab_tpm_issuer::sha256_hex(&der(&ek)));

    // Idempotent: the same EK returns the stored certificate.
    let (_, body) = h.post(&client, "/v1/ek", ek_body(EK_RSA)).await.unwrap();
    let again = issued(body);
    assert!(again.existing);
    assert_eq!(again.certificate, ek.certificate);

    // A DevID against a different EK is refused.
    let (status, _) = h
        .post(&client, "/v1/devid", devid_body(DEVID_RSA, EK_ECC))
        .await
        .unwrap();
    assert_eq!(status, 409);
    // An EK is not a DevID signing key.
    let (status, _) = h
        .post(&client, "/v1/devid", devid_body(EK_RSA, EK_RSA))
        .await
        .unwrap();
    assert_eq!(status, 422);

    let (status, body) = h
        .post(&client, "/v1/devid", devid_body(DEVID_RSA, EK_RSA))
        .await
        .unwrap();
    assert_eq!(status, 200, "{body}");
    let devid = issued(body);
    assert_eq!(common_name(&der(&devid)), format!("qrab-vm-{UID}"));
    let devid_der = der(&devid);
    let (_, cert) = parse_x509_certificate(&devid_der).unwrap();
    assert_eq!(
        cert.issuer()
            .iter_common_name()
            .next()
            .unwrap()
            .as_str()
            .unwrap(),
        "qrab-devid-ca-1"
    );

    // Same DevID again: idempotent. A different DevID: refused without --allow-rekey.
    let (_, body) = h
        .post(&client, "/v1/devid", devid_body(DEVID_RSA, EK_RSA))
        .await
        .unwrap();
    assert!(issued(body).existing);
    let (status, _) = h
        .post(&client, "/v1/devid", devid_body(DEVID_ECC, EK_RSA))
        .await
        .unwrap();
    assert_eq!(status, 409);
    // So is a different EK.
    let mut other_ek = EK_RSA.to_vec();
    let last = other_ek.len() - 1;
    other_ek[last] ^= 1;
    let (status, _) = h.post(&client, "/v1/ek", ek_body(&other_ek)).await.unwrap();
    assert_eq!(status, 409);

    // Another VM's SVID gets its own uid, not this one's.
    let (_api2, other) = h.client(&vm(OTHER_UID)).await;
    let (status, body) = h.post(&other, "/v1/ek", ek_body(EK_RSA)).await.unwrap();
    assert_eq!(status, 200);
    let ek2 = issued(body);
    assert_eq!(ek2.uid, OTHER_UID);
    assert!(!ek2.existing);
    assert_eq!(common_name(&der(&ek2)), format!("qrab-vm-{OTHER_UID}"));

    // The ECC EK parses but is refused: SPIRE's tpm_devid is RSA-EK-only.
    let (status, body) = h.post(&other, "/v1/ek", ek_body(EK_ECC)).await.unwrap();
    assert_eq!(status, 422, "{body}");
    assert!(body["error"].as_str().unwrap().contains("RSA-2048"));
    let (status, body) = h
        .post(&client, "/v1/ek", ek_body(b"not a tpm public"))
        .await
        .unwrap();
    assert_eq!(status, 400, "{body}");
}

#[tokio::test]
async fn non_vm_spiffe_ids_are_refused_at_tls() {
    let h = Harness::start(false).await;
    for id in [
        format!("spiffe://yapplesauce.com/kubernetes/qrab/nic/{UID}"),
        "spiffe://yapplesauce.com/kubernetes/qrab/vm/not-a-uuid".to_string(),
        format!(
            "spiffe://yapplesauce.com/kubernetes/qrab/vm/{}",
            UID.to_uppercase()
        ),
        format!("spiffe://yapplesauce.com/kubernetes/qrab/vm/{UID}/bmc"),
        format!("spiffe://yapplesauce.com/kubernetes/other/vm/{UID}"),
    ] {
        let (_api, client) = h.client(&id).await;
        let result = h.post(&client, "/v1/ek", ek_body(EK_RSA)).await;
        assert!(result.is_err(), "{id} must be refused, got {result:?}");
    }
    // A raw TLS handshake from a non-vm ID never yields an HTTP response.
    let api = Workload::serve(vec![h.authority.svid(&format!(
        "spiffe://yapplesauce.com/kubernetes/qrab/nic/{UID}"
    ))]);
    let identity = Identity::connect(&api.endpoint).await.unwrap();
    let connector = identity
        .connector(&[qrab_spiffe::id(SERVER_ID).unwrap()])
        .unwrap();
    let tcp = tokio::net::TcpStream::connect(h.address).await.unwrap();
    if let Ok(mut tls) = connector.connect(tcp).await {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let _ = tls
            .write_all(b"POST /v1/ek HTTP/1.1\r\nHost: x\r\nContent-Length: 0\r\n\r\n")
            .await;
        let mut out = Vec::new();
        assert!(tls.read_to_end(&mut out).await.is_err() || out.is_empty());
    }
    assert!(!h.record_exists(UID));
    // Plain HTTP is not served either.
    let plain = reqwest::Client::new()
        .post(format!("http://{}/v1/ek", h.address))
        .body(ek_body(EK_RSA).to_string())
        .send()
        .await;
    assert!(plain.is_err() || !plain.unwrap().status().is_success());
}

#[tokio::test]
async fn allow_rekey_reissues_and_drops_the_old_devid() {
    let h = Harness::start(true).await;
    let (_api, client) = h.client(&vm(UID)).await;
    let (status, _) = h.post(&client, "/v1/ek", ek_body(EK_RSA)).await.unwrap();
    assert_eq!(status, 200);
    let (status, body) = h
        .post(&client, "/v1/devid", devid_body(DEVID_RSA, EK_RSA))
        .await
        .unwrap();
    assert_eq!(status, 200);
    let first = issued(body);
    let (status, body) = h
        .post(&client, "/v1/devid", devid_body(DEVID_ECC, EK_RSA))
        .await
        .unwrap();
    assert_eq!(status, 200);
    let second = issued(body);
    assert!(!second.existing);
    assert_ne!(first.certificate, second.certificate);

    let mut new_ek = EK_RSA.to_vec();
    let last = new_ek.len() - 1;
    new_ek[last] ^= 1;
    let (status, _) = h.post(&client, "/v1/ek", ek_body(&new_ek)).await.unwrap();
    assert_eq!(status, 200);
    // The DevID was bound to the old EK and is gone; the old EK no longer matches.
    let (status, _) = h
        .post(&client, "/v1/devid", devid_body(DEVID_ECC, EK_RSA))
        .await
        .unwrap();
    assert_eq!(status, 409);
    let (status, body) = h
        .post(&client, "/v1/devid", devid_body(DEVID_ECC, &new_ek))
        .await
        .unwrap();
    assert_eq!(status, 200);
    assert!(!issued(body).existing);
}

fn rust_verify(leaf: &[u8], ca_pem: &str) {
    let (_, ca_der) = parse_x509_pem(ca_pem.as_bytes()).unwrap();
    let (_, ca) = parse_x509_certificate(&ca_der.contents).unwrap();
    let (_, cert) = parse_x509_certificate(leaf).unwrap();
    cert.verify_signature(Some(ca.public_key())).unwrap();
    assert_eq!(cert.issuer().as_raw(), ca.subject().as_raw());
    assert_eq!(
        cert.signature_algorithm.algorithm.to_id_string(),
        "1.2.840.10045.4.3.2",
        "ecdsa-with-SHA256"
    );
    let ski = ca
        .extensions()
        .iter()
        .find_map(|e| match e.parsed_extension() {
            ParsedExtension::SubjectKeyIdentifier(id) => Some(id.0.to_vec()),
            _ => None,
        })
        .unwrap();
    let aki = cert
        .extensions()
        .iter()
        .find_map(|e| match e.parsed_extension() {
            ParsedExtension::AuthorityKeyIdentifier(aki) => {
                aki.key_identifier.as_ref().map(|k| k.0.to_vec())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(aki, ski, "AKI must match the CA SKI");
    // Not a CA, and valid now.
    assert!(!cert.is_ca());
    assert!(cert.validity().is_valid());
}

fn openssl_verify(dir: &Path, name: &str, leaf_pem: &str, ca: &str) -> bool {
    let leaf = dir.join(format!("{name}.pem"));
    std::fs::write(&leaf, leaf_pem).unwrap();
    let output = Command::new("openssl")
        .arg("verify")
        .arg("-x509_strict")
        .arg("-CAfile")
        .arg(dir.join(ca))
        .arg(&leaf)
        .output()
        .unwrap();
    eprintln!(
        "openssl verify {name}: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output.status.success()
}

#[tokio::test]
async fn issued_certificates_verify_against_the_two_cas() {
    let h = Harness::start(false).await;
    let (_api, client) = h.client(&vm(UID)).await;
    let (_, body) = h.post(&client, "/v1/ek", ek_body(EK_RSA)).await.unwrap();
    let ek = issued(body);
    let (_, body) = h
        .post(&client, "/v1/devid", devid_body(DEVID_ECC, EK_RSA))
        .await
        .unwrap();
    let devid = issued(body);

    let ek_ca = std::fs::read_to_string(h.ca.path().join("ek-ca.pem")).unwrap();
    let devid_ca = std::fs::read_to_string(h.ca.path().join("devid-ca.pem")).unwrap();
    assert_eq!(ek.ca_certificate_pem, ek_ca);
    assert_eq!(devid.ca_certificate_pem, devid_ca);
    rust_verify(&der(&ek), &ek_ca);
    rust_verify(&der(&devid), &devid_ca);
    // Each leaf chains only to its own CA.
    let (_, wrong) = parse_x509_pem(devid_ca.as_bytes()).unwrap();
    let (_, wrong) = parse_x509_certificate(&wrong.contents).unwrap();
    let ek_der = der(&ek);
    let (_, ek_cert) = parse_x509_certificate(&ek_der).unwrap();
    assert!(ek_cert.verify_signature(Some(wrong.public_key())).is_err());

    if Command::new("openssl").arg("version").output().is_err() {
        eprintln!("SKIPPED openssl verify: the openssl CLI is not installed");
        return;
    }
    let work = tempfile::tempdir().unwrap();
    for name in ["ek-ca.pem", "devid-ca.pem"] {
        std::fs::copy(h.ca.path().join(name), work.path().join(name)).unwrap();
    }
    assert!(openssl_verify(
        work.path(),
        "ek",
        &ek.certificate_pem,
        "ek-ca.pem"
    ));
    assert!(openssl_verify(
        work.path(),
        "devid",
        &devid.certificate_pem,
        "devid-ca.pem"
    ));
    // Cross-checks prove the oracle can fail.
    assert!(!openssl_verify(
        work.path(),
        "ek-x",
        &ek.certificate_pem,
        "devid-ca.pem"
    ));
    assert!(!openssl_verify(
        work.path(),
        "devid-x",
        &devid.certificate_pem,
        "ek-ca.pem"
    ));
}

/// CAs made the way an operator would, with the openssl CLI.
#[test]
fn openssl_generated_p256_cas_load_and_sign() {
    if Command::new("openssl").arg("version").output().is_err() {
        eprintln!("SKIPPED: the openssl CLI is not installed");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    for (name, cn) in [
        ("ek-ca", "qrab-endorsement-ca-1"),
        ("devid-ca", "qrab-devid-ca-1"),
    ] {
        let status = Command::new("openssl")
            .args([
                "req",
                "-x509",
                "-newkey",
                "ec",
                "-pkeyopt",
                "ec_paramgen_curve:P-256",
            ])
            .args(["-nodes", "-days", "3650", "-subj", &format!("/CN={cn}")])
            .args(["-addext", "basicConstraints=critical,CA:TRUE,pathlen:0"])
            .args(["-addext", "keyUsage=critical,keyCertSign,cRLSign"])
            .args(["-addext", "subjectKeyIdentifier=hash"])
            .arg("-keyout")
            .arg(dir.path().join(format!("{name}.key")))
            .arg("-out")
            .arg(dir.path().join(format!("{name}.pem")))
            .output()
            .unwrap();
        assert!(status.status.success(), "{status:?}");
    }
    let state = tempfile::tempdir().unwrap();
    let issuer = Issuer::load(dir.path(), state.path(), false).unwrap();
    let uid = qrab_tpm_issuer::VmUid::parse(UID).unwrap();
    let ek = issuer.issue_ek(&uid, EK_RSA).unwrap();
    let devid = issuer.issue_devid(&uid, DEVID_RSA, EK_RSA).unwrap();
    let ek_ca = std::fs::read_to_string(dir.path().join("ek-ca.pem")).unwrap();
    let devid_ca = std::fs::read_to_string(dir.path().join("devid-ca.pem")).unwrap();
    rust_verify(&ek.der, &ek_ca);
    rust_verify(&devid.der, &devid_ca);
    assert!(openssl_verify(
        dir.path(),
        "ek",
        &qrab_tpm_issuer::cert::pem("CERTIFICATE", &ek.der),
        "ek-ca.pem"
    ));
    assert!(openssl_verify(
        dir.path(),
        "devid",
        &qrab_tpm_issuer::cert::pem("CERTIFICATE", &devid.der),
        "devid-ca.pem"
    ));
}

#[test]
fn identical_cas_and_missing_files_are_refused() {
    let dir = ca_dir();
    let state = tempfile::tempdir().unwrap();
    for ext in ["pem", "key"] {
        std::fs::copy(
            dir.path().join(format!("ek-ca.{ext}")),
            dir.path().join(format!("devid-ca.{ext}")),
        )
        .unwrap();
    }
    assert!(Issuer::load(dir.path(), state.path(), false).is_err());
    std::fs::remove_file(dir.path().join("devid-ca.key")).unwrap();
    let error = Issuer::load(dir.path(), state.path(), false)
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("devid-ca.key"), "{error}");
}

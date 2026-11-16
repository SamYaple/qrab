//! The two CAs and the certificate profiles SPIRE v1.15.3 `tpm_devid` accepts.
//!
//! SPIRE's server verifies the EK certificate with Go `x509.Verify` against
//! `endorsement_ca_path` after dropping only an unhandled critical
//! subjectAltName, and requires its RSA key to equal the EK it regenerates.
//! It verifies the DevID certificate against `devid_ca_path` with
//! `ExtKeyUsageAny` and turns the leaf subject and issuer CNs into the
//! `tpm_devid:subject:cn:` and `tpm_devid:issuer:cn:` selectors. Go builds the
//! chain by exact issuer/subject DER bytes, so [`Ca::load`] proves at startup
//! that issued leaves name the CA byte-for-byte and verify under its key.
use crate::{der, tpm, VmUid};
use ::time::{Duration, OffsetDateTime};
use base64::Engine;
use rcgen::{
    CertificateParams, CustomExtension, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose, PublicKeyData, SerialNumber, SignatureAlgorithm,
};
use std::path::Path;
use x509_parser::prelude::*;

/// TCG `tcg-kp-EKCertificate` extended key usage.
pub const OID_TCG_KP_EK_CERTIFICATE: &[u64] = &[2, 23, 133, 8, 1];
/// TCG `tcg-at-tpmManufacturer`, `tcg-at-tpmModel`, `tcg-at-tpmVersion`.
pub const OID_TCG_AT_TPM_MANUFACTURER: &[u64] = &[2, 23, 133, 2, 1];
pub const OID_TCG_AT_TPM_MODEL: &[u64] = &[2, 23, 133, 2, 2];
pub const OID_TCG_AT_TPM_VERSION: &[u64] = &[2, 23, 133, 2, 3];
/// TCG `tcg-at-tpmSpecification`.
pub const OID_TCG_AT_TPM_SPECIFICATION: &[u64] = &[2, 23, 133, 2, 16];
const OID_SUBJECT_ALT_NAME: &[u64] = &[2, 5, 29, 17];
const OID_SUBJECT_DIRECTORY_ATTRIBUTES: &[u64] = &[2, 5, 29, 9];

/// The vTPM is swtpm/libtpms, whose TCG vendor ID is IBM's.
pub const TPM_MANUFACTURER: &str = "id:00001014";
pub const TPM_MODEL: &str = "swtpm";
pub const TPM_VERSION: &str = "id:20191023";

#[derive(Debug, thiserror::Error)]
pub enum CaError {
    #[error("{0}: {1}")]
    Io(String, std::io::Error),
    #[error("{0}: {1}")]
    Invalid(String, &'static str),
}

/// A loaded CA keypair.
pub struct Ca {
    issuer: Issuer<'static, KeyPair>,
    der: Vec<u8>,
    pem: String,
    not_after: OffsetDateTime,
    spki: Vec<u8>,
}

/// Wrap DER as one PEM block.
pub fn pem(label: &str, der: &[u8]) -> String {
    let body = base64::engine::general_purpose::STANDARD.encode(der);
    let mut out = format!("-----BEGIN {label}-----\n");
    for line in body.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(line).expect("base64 is ASCII"));
        out.push('\n');
    }
    out.push_str(&format!("-----END {label}-----\n"));
    out
}

impl Ca {
    /// Load `<dir>/<name>.pem` (one CA certificate) and `<dir>/<name>.key`
    /// (its PKCS#8 private key).
    pub fn load(dir: &Path, name: &str) -> Result<Self, CaError> {
        let read = |file: String| {
            let path = dir.join(&file);
            std::fs::read_to_string(&path).map_err(|e| CaError::Io(path.display().to_string(), e))
        };
        let cert = read(format!("{name}.pem"))?;
        let key = read(format!("{name}.key"))?;
        Self::from_pem(&cert, &key).map_err(|e| match e {
            CaError::Invalid(_, why) => CaError::Invalid(name.into(), why),
            other => other,
        })
    }

    pub fn from_pem(cert_pem: &str, key_pem: &str) -> Result<Self, CaError> {
        let invalid = |why| CaError::Invalid("CA".into(), why);
        let mut blocks = Pem::iter_from_buffer(cert_pem.as_bytes());
        let block = blocks
            .next()
            .ok_or(invalid("no PEM certificate"))?
            .map_err(|_| invalid("unparseable PEM certificate"))?;
        if blocks.next().is_some() {
            return Err(invalid(
                "the certificate file must hold exactly one CA certificate",
            ));
        }
        if block.label != "CERTIFICATE" {
            return Err(invalid("the certificate file is not a PEM CERTIFICATE"));
        }
        let der = block.contents;
        let (rest, cert) =
            parse_x509_certificate(&der).map_err(|_| invalid("unparseable CA certificate"))?;
        if !rest.is_empty() {
            return Err(invalid("trailing data after the CA certificate"));
        }
        if !cert.is_ca() {
            return Err(invalid(
                "certificate is not a CA (basicConstraints cA=TRUE)",
            ));
        }
        if let Ok(Some(usage)) = cert.key_usage() {
            if !usage.value.key_cert_sign() {
                return Err(invalid("CA key usage lacks keyCertSign"));
            }
        }
        let now = OffsetDateTime::now_utc();
        let not_after = cert.validity().not_after.to_datetime();
        if cert.validity().not_before.to_datetime() > now || not_after <= now {
            return Err(invalid("CA certificate is not currently valid"));
        }
        let subject = cert.subject().as_raw().to_vec();
        let spki = cert.public_key().raw.to_vec();
        let key = KeyPair::from_pem(key_pem).map_err(|_| invalid("unparseable PKCS#8 CA key"))?;
        let issuer = Issuer::from_ca_cert_pem(cert_pem, key)
            .map_err(|_| invalid("CA subject cannot be re-encoded"))?;
        let ca = Self {
            issuer,
            pem: pem("CERTIFICATE", &der),
            der: der.clone(),
            not_after,
            spki,
        };
        // Prove the key matches the certificate and the issuer name encodes
        // byte-for-byte, so Go and OpenSSL can build the chain.
        let probe_key = KeyPair::generate().map_err(|_| invalid("probe key"))?;
        let mut params = CertificateParams::default();
        params.distinguished_name = rcgen::DistinguishedName::new();
        params.distinguished_name.push(DnType::CommonName, "probe");
        let probe = ca
            .sign(params, &probe_key)
            .map_err(|_| invalid("CA key cannot sign"))?;
        let (_, probe) = parse_x509_certificate(&probe).map_err(|_| invalid("probe"))?;
        if probe.issuer().as_raw() != subject.as_slice() {
            return Err(invalid("CA subject does not round-trip byte-for-byte"));
        }
        probe
            .verify_signature(Some(cert.public_key()))
            .map_err(|_| invalid("CA private key does not match the CA certificate"))?;
        Ok(ca)
    }

    pub fn der(&self) -> &[u8] {
        &self.der
    }

    pub fn pem(&self) -> &str {
        &self.pem
    }

    pub fn same_key(&self, other: &Ca) -> bool {
        self.spki == other.spki
    }

    fn sign(
        &self,
        mut params: CertificateParams,
        key: &impl PublicKeyData,
    ) -> Result<Vec<u8>, rcgen::Error> {
        let mut serial = [0u8; 16];
        ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut serial)
            .map_err(|_| rcgen::Error::RingUnspecified)?;
        // Positive and full-length, so DER never strips or pads it.
        serial[0] = (serial[0] & 0x7f) | 0x40;
        params.serial_number = Some(SerialNumber::from_slice(&serial));
        params.not_before = OffsetDateTime::now_utc() - Duration::minutes(5);
        // A leaf never outlives the CA that signed it.
        params.not_after = self.not_after;
        params.use_authority_key_identifier_extension = true;
        Ok(params.signed_by(key, &self.issuer)?.der().to_vec())
    }
}

/// A TPM public key as an X.509 SubjectPublicKeyInfo.
struct TpmKey {
    alg: &'static SignatureAlgorithm,
    key: Vec<u8>,
}

impl PublicKeyData for TpmKey {
    fn der_bytes(&self) -> &[u8] {
        &self.key
    }
    fn algorithm(&self) -> &'static SignatureAlgorithm {
        self.alg
    }
}

fn tpm_key(public: &tpm::Public) -> TpmKey {
    let alg = match public.key {
        tpm::Key::Rsa { .. } => &rcgen::PKCS_RSA_SHA256,
        tpm::Key::EccP256 { .. } => &rcgen::PKCS_ECDSA_P256_SHA256,
    };
    TpmKey {
        alg,
        key: public.subject_public_key(),
    }
}

/// `qrab-vm-<uid>`: the DevID subject CN that becomes SPIRE's
/// `tpm_devid:subject:cn:qrab-vm-<uid>` selector.
pub fn common_name(uid: &VmUid) -> String {
    format!("qrab-vm-{}", uid.as_str())
}

fn leaf(uid: &VmUid, usage: KeyUsagePurpose) -> CertificateParams {
    let mut params = CertificateParams::default();
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::CommonName, common_name(uid));
    params.is_ca = IsCa::ExplicitNoCa;
    params.key_usages = vec![usage];
    params
}

fn tpm_directory_name() -> Vec<u8> {
    let rdn =
        |oid: &[u64], value: &str| der::set(&[der::sequence(&[der::oid(oid), der::utf8(value)])]);
    let name = der::sequence(&[
        rdn(OID_TCG_AT_TPM_MANUFACTURER, TPM_MANUFACTURER),
        rdn(OID_TCG_AT_TPM_MODEL, TPM_MODEL),
        rdn(OID_TCG_AT_TPM_VERSION, TPM_VERSION),
    ]);
    // GeneralNames ::= SEQUENCE { directoryName [4] EXPLICIT Name }
    der::sequence(&[der::tlv(0xa4, &name)])
}

fn tpm_specification() -> Vec<u8> {
    // SubjectDirectoryAttributes ::= SEQUENCE OF Attribute, with
    // TPMSpecification ::= SEQUENCE { family UTF8String, level INTEGER, revision INTEGER }
    let spec = der::sequence(&[
        der::utf8("2.0"),
        der::unsigned_integer(&[0]),
        der::unsigned_integer(&164u16.to_be_bytes()),
    ]);
    der::sequence(&[der::sequence(&[
        der::oid(OID_TCG_AT_TPM_SPECIFICATION),
        der::set(&[spec]),
    ])])
}

/// An EK certificate per the TCG EK Credential Profile: keyEncipherment,
/// `tcg-kp-EKCertificate`, a critical TPM-attribute directoryName SAN (the
/// one critical extension SPIRE tolerates beyond those Go handles) and the
/// TPM specification attribute.
pub fn ek(ca: &Ca, uid: &VmUid, public: &tpm::Public) -> Result<Vec<u8>, rcgen::Error> {
    let mut params = leaf(uid, KeyUsagePurpose::KeyEncipherment);
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::Other(
        OID_TCG_KP_EK_CERTIFICATE.to_vec(),
    )];
    let mut san = CustomExtension::from_oid_content(OID_SUBJECT_ALT_NAME, tpm_directory_name());
    san.set_criticality(true);
    params.custom_extensions = vec![
        san,
        CustomExtension::from_oid_content(OID_SUBJECT_DIRECTORY_ATTRIBUTES, tpm_specification()),
    ];
    ca.sign(params, &tpm_key(public))
}

/// A DevID certificate (IEEE 802.1AR LDevID): digitalSignature, subject CN
/// `qrab-vm-<uid>`. SPIRE accepts any EKU, so none is set.
pub fn devid(ca: &Ca, uid: &VmUid, public: &tpm::Public) -> Result<Vec<u8>, rcgen::Error> {
    ca.sign(
        leaf(uid, KeyUsagePurpose::DigitalSignature),
        &tpm_key(public),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ca(cn: &str) -> (String, String) {
        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::default();
        params.distinguished_name = rcgen::DistinguishedName::new();
        params.distinguished_name.push(DnType::CommonName, cn);
        params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        params.not_before = OffsetDateTime::now_utc() - Duration::days(1);
        params.not_after = OffsetDateTime::now_utc() + Duration::days(365);
        (params.self_signed(&key).unwrap().pem(), key.serialize_pem())
    }

    #[test]
    fn ca_load_refuses_mismatched_keys_and_non_cas() {
        let (cert, key) = ca("a");
        let (_, other) = ca("b");
        Ca::from_pem(&cert, &key).unwrap();
        assert!(Ca::from_pem(&cert, &other)
            .err()
            .unwrap()
            .to_string()
            .contains("does not match"));
        let leaf = KeyPair::generate().unwrap();
        let leaf_cert = CertificateParams::new(vec!["x".into()])
            .unwrap()
            .self_signed(&leaf)
            .unwrap()
            .pem();
        assert!(Ca::from_pem(&leaf_cert, &leaf.serialize_pem()).is_err());
        assert!(Ca::from_pem(&format!("{cert}{cert}"), &key).is_err());
    }

    #[test]
    fn profiles_carry_spire_and_tcg_fields() {
        let (cert, key) = ca("qrab test EK CA");
        let ca = Ca::from_pem(&cert, &key).unwrap();
        let uid = VmUid::parse("0b6f2a3e-5c1d-4e8f-9a7b-1c2d3e4f5a6b").unwrap();
        let ek_public = tpm::Public::parse(include_bytes!("../tests/data/ek-rsa.pub")).unwrap();
        let (_, ca_cert) = parse_x509_certificate(ca.der()).unwrap();
        let der = ek(&ca, &uid, &ek_public).unwrap();
        let (_, x) = parse_x509_certificate(&der).unwrap();
        assert_eq!(
            x.subject()
                .iter_common_name()
                .next()
                .unwrap()
                .as_str()
                .unwrap(),
            "qrab-vm-0b6f2a3e-5c1d-4e8f-9a7b-1c2d3e4f5a6b"
        );
        assert!(x.key_usage().unwrap().unwrap().value.key_encipherment());
        let critical: Vec<String> = x
            .extensions()
            .iter()
            .filter(|e| e.critical)
            .map(|e| e.oid.to_id_string())
            .collect();
        // basicConstraints, keyUsage, subjectAltName: nothing else is critical.
        assert_eq!(critical.len(), 3, "{critical:?}");
        assert!(critical.contains(&"2.5.29.17".to_string()));
        let eku = x.extended_key_usage().unwrap().unwrap().value;
        assert_eq!(eku.other[0].to_id_string(), "2.23.133.8.1");
        let san = x.subject_alternative_name().unwrap().unwrap().value;
        let GeneralName::DirectoryName(name) = &san.general_names[0] else {
            panic!("directoryName SAN expected");
        };
        assert!(name.to_string().contains("id:00001014"));
        x.verify_signature(Some(ca_cert.public_key())).unwrap();
        assert_eq!(x.issuer().as_raw(), ca_cert.subject().as_raw());
        // The certificate key is the EK modulus.
        let tpm::Key::Rsa { modulus, .. } = &ek_public.key else {
            unreachable!()
        };
        let x509_parser::public_key::PublicKey::RSA(rsa) = x.public_key().parsed().unwrap() else {
            panic!("RSA key expected");
        };
        assert_eq!(rsa.modulus.strip_prefix(&[0]).unwrap(), modulus.as_slice());
        assert!(x.validity().not_after.to_datetime() <= ca.not_after);

        let devid_public =
            tpm::Public::parse(include_bytes!("../tests/data/devid-ecc.pub")).unwrap();
        let der = devid(&ca, &uid, &devid_public).unwrap();
        let (_, x) = parse_x509_certificate(&der).unwrap();
        assert!(x.key_usage().unwrap().unwrap().value.digital_signature());
        assert!(x.extended_key_usage().unwrap().is_none());
        x.verify_signature(Some(ca_cert.public_key())).unwrap();
        let tpm::Key::EccP256 { x: px, .. } = devid_public.key else {
            unreachable!()
        };
        assert_eq!(&x.public_key().subject_public_key.data[1..33], &px);
    }
}

//! TPM 2.0 `TPMT_PUBLIC` parsing (TPM 2.0 Library Part 2, §12.2) for the RSA
//! and ECC objects this issuer certifies, plus the key profiles SPIRE's
//! `tpm_devid` attestor can use.

/// `TPM_ALG_ID` values from TPM 2.0 Part 2, §6.3.
pub mod alg {
    pub const RSA: u16 = 0x0001;
    pub const SHA1: u16 = 0x0004;
    pub const AES: u16 = 0x0006;
    pub const MGF1: u16 = 0x0007;
    pub const SHA256: u16 = 0x000B;
    pub const SHA384: u16 = 0x000C;
    pub const SHA512: u16 = 0x000D;
    pub const NULL: u16 = 0x0010;
    pub const SM3_256: u16 = 0x0012;
    pub const SM4: u16 = 0x0013;
    pub const RSASSA: u16 = 0x0014;
    pub const RSAES: u16 = 0x0015;
    pub const RSAPSS: u16 = 0x0016;
    pub const OAEP: u16 = 0x0017;
    pub const ECDSA: u16 = 0x0018;
    pub const ECDH: u16 = 0x0019;
    pub const ECDAA: u16 = 0x001A;
    pub const SM2: u16 = 0x001B;
    pub const ECSCHNORR: u16 = 0x001C;
    pub const ECMQV: u16 = 0x001D;
    pub const KDF1_SP800_56A: u16 = 0x0020;
    pub const KDF2: u16 = 0x0021;
    pub const KDF1_SP800_108: u16 = 0x0022;
    pub const ECC: u16 = 0x0023;
    pub const CAMELLIA: u16 = 0x0026;
    pub const CFB: u16 = 0x0043;
}

/// `TPMA_OBJECT` bits from TPM 2.0 Part 2, §8.3.
pub mod attr {
    pub const FIXED_TPM: u32 = 1 << 1;
    pub const FIXED_PARENT: u32 = 1 << 4;
    pub const SENSITIVE_DATA_ORIGIN: u32 = 1 << 5;
    pub const USER_WITH_AUTH: u32 = 1 << 6;
    pub const ADMIN_WITH_POLICY: u32 = 1 << 7;
    pub const RESTRICTED: u32 = 1 << 16;
    pub const DECRYPT: u32 = 1 << 17;
    pub const SIGN: u32 = 1 << 18;
}

/// `TPM_ECC_NIST_P256`.
pub const ECC_NIST_P256: u16 = 0x0003;

/// The TCG default EK template's attributes (EK Credential Profile 2.0,
/// template L-1; go-tpm-tools `DefaultEKTemplateRSA`).
pub const EK_ATTRIBUTES: u32 = attr::FIXED_TPM
    | attr::FIXED_PARENT
    | attr::SENSITIVE_DATA_ORIGIN
    | attr::ADMIN_WITH_POLICY
    | attr::RESTRICTED
    | attr::DECRYPT;

/// `PolicySecret(TPM_RH_ENDORSEMENT)`, the default EK template's authPolicy.
pub const EK_AUTH_POLICY: [u8; 32] = [
    0x83, 0x71, 0x97, 0x67, 0x44, 0x84, 0xb3, 0xf8, 0x1a, 0x90, 0xcc, 0x8d, 0x46, 0xa5, 0xd7, 0x24,
    0xfd, 0x52, 0xd7, 0x6e, 0x06, 0x52, 0x0b, 0x64, 0xf2, 0xa1, 0xda, 0x1b, 0x33, 0x14, 0x69, 0xaa,
];

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("public area truncated")]
    Truncated,
    #[error("{0} trailing bytes after the public area")]
    Trailing(usize),
    #[error("unsupported object type 0x{0:04x}; RSA and ECC are supported")]
    Type(u16),
    #[error("unsupported {0} algorithm 0x{1:04x}")]
    Algorithm(&'static str, u16),
    #[error("RSA key size {0} is not 2048")]
    RsaBits(u16),
    #[error("RSA modulus is not a 2048-bit value")]
    RsaModulus,
    #[error("ECC curve 0x{0:04x} is not NIST P-256")]
    Curve(u16),
    #[error("ECC point coordinates are not 32-byte P-256 values")]
    EccPoint,
}

/// A symmetric definition (`TPMT_SYM_DEF_OBJECT`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Symmetric {
    pub algorithm: u16,
    pub key_bits: u16,
    pub mode: u16,
}

/// An asymmetric scheme (`TPMT_RSA_SCHEME` / `TPMT_ECC_SCHEME` / `TPMT_KDF_SCHEME`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scheme {
    pub scheme: u16,
    /// `TPM_ALG_NULL` when the scheme carries no hash.
    pub hash: u16,
}

impl Scheme {
    const NULL: Self = Self {
        scheme: alg::NULL,
        hash: alg::NULL,
    };
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Rsa {
        key_bits: u16,
        /// The effective public exponent (0 on the wire means 65537).
        exponent: u32,
        /// Big-endian modulus, exactly 256 bytes.
        modulus: Vec<u8>,
    },
    EccP256 {
        kdf: Scheme,
        x: [u8; 32],
        y: [u8; 32],
    },
}

/// A parsed `TPMT_PUBLIC` for an RSA-2048 or ECC NIST P-256 object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Public {
    pub name_alg: u16,
    pub attributes: u32,
    pub auth_policy: Vec<u8>,
    pub symmetric: Option<Symmetric>,
    pub scheme: Scheme,
    pub key: Key,
    /// The canonical `TPMT_PUBLIC` bytes (no `TPM2B` size prefix).
    pub tpmt: Vec<u8>,
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], ParseError> {
        if self.0.len() < n {
            return Err(ParseError::Truncated);
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(head)
    }
    fn u16(&mut self) -> Result<u16, ParseError> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32, ParseError> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn tpm2b(&mut self) -> Result<&'a [u8], ParseError> {
        let n = self.u16()?;
        self.take(n.into())
    }
}

fn hash_alg(what: &'static str, value: u16) -> Result<u16, ParseError> {
    match value {
        alg::SHA1 | alg::SHA256 | alg::SHA384 | alg::SHA512 | alg::SM3_256 => Ok(value),
        other => Err(ParseError::Algorithm(what, other)),
    }
}

fn symmetric(r: &mut Reader) -> Result<Option<Symmetric>, ParseError> {
    match r.u16()? {
        alg::NULL => Ok(None),
        algorithm @ (alg::AES | alg::SM4 | alg::CAMELLIA) => Ok(Some(Symmetric {
            algorithm,
            key_bits: r.u16()?,
            mode: r.u16()?,
        })),
        other => Err(ParseError::Algorithm("symmetric", other)),
    }
}

fn rsa_scheme(r: &mut Reader) -> Result<Scheme, ParseError> {
    match r.u16()? {
        alg::NULL => Ok(Scheme::NULL),
        // TPMS_ENC_SCHEME_RSAES is empty.
        alg::RSAES => Ok(Scheme {
            scheme: alg::RSAES,
            hash: alg::NULL,
        }),
        scheme @ (alg::RSASSA | alg::RSAPSS | alg::OAEP) => Ok(Scheme {
            scheme,
            hash: hash_alg("RSA scheme hash", r.u16()?)?,
        }),
        other => Err(ParseError::Algorithm("RSA scheme", other)),
    }
}

fn ecc_scheme(r: &mut Reader) -> Result<Scheme, ParseError> {
    match r.u16()? {
        alg::NULL => Ok(Scheme::NULL),
        scheme @ (alg::ECDSA | alg::ECDH | alg::SM2 | alg::ECSCHNORR | alg::ECMQV) => Ok(Scheme {
            scheme,
            hash: hash_alg("ECC scheme hash", r.u16()?)?,
        }),
        alg::ECDAA => {
            let hash = hash_alg("ECC scheme hash", r.u16()?)?;
            let _count = r.u16()?;
            Ok(Scheme {
                scheme: alg::ECDAA,
                hash,
            })
        }
        other => Err(ParseError::Algorithm("ECC scheme", other)),
    }
}

fn kdf_scheme(r: &mut Reader) -> Result<Scheme, ParseError> {
    match r.u16()? {
        alg::NULL => Ok(Scheme::NULL),
        scheme @ (alg::MGF1 | alg::KDF1_SP800_56A | alg::KDF2 | alg::KDF1_SP800_108) => {
            Ok(Scheme {
                scheme,
                hash: hash_alg("KDF hash", r.u16()?)?,
            })
        }
        other => Err(ParseError::Algorithm("KDF", other)),
    }
}

fn coordinate(bytes: &[u8]) -> Result<[u8; 32], ParseError> {
    if bytes.is_empty() || bytes.len() > 32 {
        return Err(ParseError::EccPoint);
    }
    let mut out = [0u8; 32];
    out[32 - bytes.len()..].copy_from_slice(bytes);
    Ok(out)
}

/// Strip a `TPM2B_PUBLIC` size prefix when present. `tpm2_createek -u` and
/// `tpm2_create -u` write `TPM2B_PUBLIC`; SPIRE's `devid_pub_path` and
/// `TPM2_ReadPublic` encodings are bare `TPMT_PUBLIC`. The forms cannot be
/// confused: a `TPMT_PUBLIC` starts with its type (0x0001 or 0x0023), which
/// is never its own remaining length.
pub fn unwrap_tpm2b(bytes: &[u8]) -> &[u8] {
    if bytes.len() >= 2 && usize::from(u16::from_be_bytes([bytes[0], bytes[1]])) == bytes.len() - 2
    {
        &bytes[2..]
    } else {
        bytes
    }
}

impl Public {
    /// Parse a `TPM2B_PUBLIC` or bare `TPMT_PUBLIC`.
    ///
    /// ```
    /// let ek = include_bytes!("../tests/data/ek-rsa.pub");
    /// let public = qrab_tpm_issuer::tpm::Public::parse(ek).unwrap();
    /// assert!(public.check_ek().is_ok());
    /// assert_eq!(public.tpmt.len(), ek.len() - 2);
    /// ```
    pub fn parse(bytes: &[u8]) -> Result<Self, ParseError> {
        let tpmt = unwrap_tpm2b(bytes);
        let mut r = Reader(tpmt);
        let kind = r.u16()?;
        let name_alg = hash_alg("name", r.u16()?)?;
        let attributes = r.u32()?;
        let auth_policy = r.tpm2b()?.to_vec();
        let (symmetric, scheme, key) = match kind {
            alg::RSA => {
                let symmetric = symmetric(&mut r)?;
                let scheme = rsa_scheme(&mut r)?;
                let key_bits = r.u16()?;
                let exponent = match r.u32()? {
                    0 => 65537,
                    e => e,
                };
                let modulus = r.tpm2b()?;
                if key_bits != 2048 {
                    return Err(ParseError::RsaBits(key_bits));
                }
                if modulus.len() != 256 || modulus[0] & 0x80 == 0 {
                    return Err(ParseError::RsaModulus);
                }
                let key = Key::Rsa {
                    key_bits,
                    exponent,
                    modulus: modulus.to_vec(),
                };
                (symmetric, scheme, key)
            }
            alg::ECC => {
                let symmetric = symmetric(&mut r)?;
                let scheme = ecc_scheme(&mut r)?;
                let curve = r.u16()?;
                if curve != ECC_NIST_P256 {
                    return Err(ParseError::Curve(curve));
                }
                let kdf = kdf_scheme(&mut r)?;
                let x = coordinate(r.tpm2b()?)?;
                let y = coordinate(r.tpm2b()?)?;
                (symmetric, scheme, Key::EccP256 { kdf, x, y })
            }
            other => return Err(ParseError::Type(other)),
        };
        if !r.0.is_empty() {
            return Err(ParseError::Trailing(r.0.len()));
        }
        Ok(Self {
            name_alg,
            attributes,
            auth_policy,
            symmetric,
            scheme,
            key,
            tpmt: tpmt.to_vec(),
        })
    }

    fn has(&self, bits: u32) -> bool {
        self.attributes & bits == bits
    }

    /// An EK SPIRE can use: SPIRE's agent regenerates the EK from go-tpm-tools
    /// `DefaultEKTemplateRSA` (TCG template L-1), the server compares its
    /// modulus with the EK certificate, and credential activation is RSA-only.
    pub fn check_ek(&self) -> Result<(), &'static str> {
        let Key::Rsa {
            exponent, modulus, ..
        } = &self.key
        else {
            return Err("SPIRE tpm_devid supports only the RSA-2048 EK (TCG template L-1)");
        };
        let template = self.name_alg == alg::SHA256
            && self.attributes == EK_ATTRIBUTES
            && self.auth_policy == EK_AUTH_POLICY
            && self.symmetric
                == Some(Symmetric {
                    algorithm: alg::AES,
                    key_bits: 128,
                    mode: alg::CFB,
                })
            && self.scheme == Scheme::NULL
            && *exponent == 65537
            && modulus.len() == 256;
        if template {
            Ok(())
        } else {
            Err("EK public area does not match the TCG default RSA EK template L-1")
        }
    }

    /// A DevID signing key SPIRE can use: a non-decrypting, TPM-bound signing
    /// key whose explicit scheme is RSASSA-SHA256 or ECDSA-SHA256. SPIRE's
    /// agent signs with the key's scheme hash, and its server verifies with
    /// SHA256WithRSA (PKCS#1 v1.5) or ECDSAWithSHA256.
    pub fn check_devid(&self) -> Result<(), &'static str> {
        if !self.has(attr::SIGN) || self.has(attr::DECRYPT) {
            return Err("DevID key must be a signing-only key");
        }
        if !self.has(attr::FIXED_TPM | attr::FIXED_PARENT | attr::SENSITIVE_DATA_ORIGIN) {
            return Err("DevID key must be fixedTPM, fixedParent and sensitiveDataOrigin");
        }
        if self.name_alg != alg::SHA256 {
            return Err("DevID key name algorithm must be SHA-256");
        }
        let expected = match self.key {
            Key::Rsa { .. } => alg::RSASSA,
            Key::EccP256 { .. } => alg::ECDSA,
        };
        if self.scheme
            != (Scheme {
                scheme: expected,
                hash: alg::SHA256,
            })
        {
            return Err("DevID key scheme must be RSASSA-SHA256 (RSA) or ECDSA-SHA256 (P-256)");
        }
        Ok(())
    }

    /// The subjectPublicKey BIT STRING contents: a DER `RSAPublicKey` or an
    /// uncompressed SEC1 point.
    pub fn subject_public_key(&self) -> Vec<u8> {
        match &self.key {
            Key::Rsa {
                exponent, modulus, ..
            } => crate::der::sequence(&[
                crate::der::unsigned_integer(modulus),
                crate::der::unsigned_integer(&exponent.to_be_bytes()),
            ]),
            Key::EccP256 { x, y, .. } => {
                let mut point = Vec::with_capacity(65);
                point.push(0x04);
                point.extend_from_slice(x);
                point.extend_from_slice(y);
                point
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EK_RSA: &[u8] = include_bytes!("../tests/data/ek-rsa.pub");
    const EK_ECC: &[u8] = include_bytes!("../tests/data/ek-ecc.pub");
    const DEVID_RSA: &[u8] = include_bytes!("../tests/data/devid-rsa.pub");
    const DEVID_ECC: &[u8] = include_bytes!("../tests/data/devid-ecc.pub");

    #[test]
    fn swtpm_rsa_ek_is_the_tcg_default_template() {
        let ek = Public::parse(EK_RSA).unwrap();
        assert_eq!(ek.name_alg, alg::SHA256);
        assert_eq!(ek.attributes, 0x0003_00b2);
        assert_eq!(ek.auth_policy, EK_AUTH_POLICY);
        let Key::Rsa {
            key_bits,
            exponent,
            ref modulus,
        } = ek.key
        else {
            panic!("RSA EK expected");
        };
        assert_eq!((key_bits, exponent, modulus.len()), (2048, 65537, 256));
        ek.check_ek().unwrap();
        assert!(ek.check_devid().is_err());
        // The bare TPMT_PUBLIC (what TPM2_ReadPublic returns to SPIRE) parses identically.
        assert_eq!(Public::parse(&EK_RSA[2..]).unwrap(), ek);
    }

    #[test]
    fn swtpm_ecc_ek_parses_but_spire_cannot_use_it() {
        let ek = Public::parse(EK_ECC).unwrap();
        assert!(matches!(ek.key, Key::EccP256 { .. }));
        assert_eq!(ek.attributes, EK_ATTRIBUTES);
        assert!(ek.check_ek().unwrap_err().contains("RSA-2048"));
    }

    #[test]
    fn swtpm_devid_keys_are_spire_signing_keys() {
        for (blob, ecc) in [(DEVID_RSA, false), (DEVID_ECC, true)] {
            let key = Public::parse(blob).unwrap();
            assert_eq!(matches!(key.key, Key::EccP256 { .. }), ecc);
            key.check_devid().unwrap();
            assert!(key.check_ek().is_err());
        }
        let ecc = Public::parse(DEVID_ECC).unwrap();
        assert_eq!(
            ecc.scheme,
            Scheme {
                scheme: alg::ECDSA,
                hash: alg::SHA256
            }
        );
        assert_eq!(ecc.subject_public_key().len(), 65);
    }

    /// TPM 2.0 Part 2 §12.2.4 hand-built TPMT_PUBLIC, independent of swtpm.
    fn ecc_public(scheme: &[u8], curve: u16, x: &[u8], y: &[u8]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&alg::ECC.to_be_bytes());
        b.extend_from_slice(&alg::SHA256.to_be_bytes());
        b.extend_from_slice(&0x0004_0072u32.to_be_bytes());
        b.extend_from_slice(&0u16.to_be_bytes()); // empty authPolicy
        b.extend_from_slice(&alg::NULL.to_be_bytes()); // symmetric
        b.extend_from_slice(scheme);
        b.extend_from_slice(&curve.to_be_bytes());
        b.extend_from_slice(&alg::NULL.to_be_bytes()); // kdf
        for c in [x, y] {
            b.extend_from_slice(&(c.len() as u16).to_be_bytes());
            b.extend_from_slice(c);
        }
        b
    }

    #[test]
    fn hand_built_structures_follow_part_2() {
        let ecdsa = [0x00, 0x18, 0x00, 0x0B];
        let tpmt = ecc_public(&ecdsa, ECC_NIST_P256, &[7; 32], &[9; 31]);
        let mut tpm2b = (tpmt.len() as u16).to_be_bytes().to_vec();
        tpm2b.extend_from_slice(&tpmt);
        let parsed = Public::parse(&tpm2b).unwrap();
        assert_eq!(parsed.tpmt, tpmt);
        let Key::EccP256 { x, y, .. } = parsed.key else {
            panic!("ECC expected")
        };
        assert_eq!((x, y[0], y[1]), ([7; 32], 0, 9));
        parsed.check_devid().unwrap();

        // ECDAA carries a count after its hash.
        let ecdaa = [0x00, 0x1A, 0x00, 0x0B, 0x00, 0x01];
        assert_eq!(
            Public::parse(&ecc_public(&ecdaa, ECC_NIST_P256, &[1; 32], &[1; 32]))
                .unwrap()
                .scheme
                .scheme,
            alg::ECDAA
        );
        assert_eq!(
            Public::parse(&ecc_public(&ecdsa, 0x0004, &[1; 48], &[1; 48])),
            Err(ParseError::Curve(0x0004))
        );
        assert_eq!(
            Public::parse(&ecc_public(&ecdsa, ECC_NIST_P256, &[1; 33], &[1; 32])),
            Err(ParseError::EccPoint)
        );
        let mut trailing = tpmt.clone();
        trailing.push(0);
        assert_eq!(Public::parse(&trailing), Err(ParseError::Trailing(1)));
        assert_eq!(
            Public::parse(&tpmt[..tpmt.len() - 1]),
            Err(ParseError::Truncated)
        );
        let mut keyedhash = tpmt.clone();
        keyedhash[..2].copy_from_slice(&0x0008u16.to_be_bytes());
        assert_eq!(Public::parse(&keyedhash), Err(ParseError::Type(0x0008)));
    }

    #[test]
    fn hand_built_rsa_and_profile_refusals() {
        let mut b = Vec::new();
        b.extend_from_slice(&alg::RSA.to_be_bytes());
        b.extend_from_slice(&alg::SHA256.to_be_bytes());
        b.extend_from_slice(&EK_ATTRIBUTES.to_be_bytes());
        b.extend_from_slice(&32u16.to_be_bytes());
        b.extend_from_slice(&EK_AUTH_POLICY);
        b.extend_from_slice(&[0x00, 0x06, 0x00, 0x80, 0x00, 0x43]); // AES-128-CFB
        b.extend_from_slice(&alg::NULL.to_be_bytes());
        b.extend_from_slice(&2048u16.to_be_bytes());
        b.extend_from_slice(&0u32.to_be_bytes());
        b.extend_from_slice(&256u16.to_be_bytes());
        let mut modulus = vec![0x5a; 256];
        modulus[0] = 0xc1;
        b.extend_from_slice(&modulus);
        let ek = Public::parse(&b).unwrap();
        ek.check_ek().unwrap();
        // RSAPublicKey DER: SEQUENCE { INTEGER (0x00 || n), INTEGER 65537 }.
        let spk = ek.subject_public_key();
        assert_eq!(&spk[..5], &[0x30, 0x82, 0x01, 0x0a, 0x02]);
        assert_eq!(&spk[spk.len() - 5..], &[0x02, 0x03, 0x01, 0x00, 0x01]);

        // A user-auth EK is not the default template SPIRE regenerates.
        let mut user = b.clone();
        user[4..8].copy_from_slice(&(EK_ATTRIBUTES | attr::USER_WITH_AUTH).to_be_bytes());
        assert!(Public::parse(&user).unwrap().check_ek().is_err());
        // RSA-3072 is refused at parse.
        let mut big = b.clone();
        let bits = 4 + 4 + 2 + 32 + 6 + 2;
        big[bits..bits + 2].copy_from_slice(&3072u16.to_be_bytes());
        assert_eq!(Public::parse(&big), Err(ParseError::RsaBits(3072)));
        // An RSAPSS signing key is not a SPIRE DevID (PKCS#1 v1.5 verification).
        let mut pss = b.clone();
        pss[4..8].copy_from_slice(&0x0004_0072u32.to_be_bytes());
        let scheme = 4 + 4 + 2 + 32 + 6;
        pss.splice(scheme..scheme + 2, [0x00, 0x16, 0x00, 0x0B]);
        let pss = Public::parse(&pss).unwrap();
        assert!(pss.check_devid().unwrap_err().contains("RSASSA"));
    }
}

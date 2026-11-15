//! A VPN profile's key. Proton certifies an Ed25519 public key; the
//! WireGuard key is the same secret converted to X25519 (libsodium's
//! `crypto_sign_ed25519_sk_to_curve25519`), as ProtonVPN's own clients do.
//!
//! ```
//! use qrab_proton::keys::ProfileKey;
//! // RFC 8032 test 1's seed; the expected values come from an independent
//! // implementation (Python `cryptography`).
//! let key = ProfileKey::from_seed([
//!     0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
//!     0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
//! ]);
//! assert_eq!(
//!     key.public_pem(),
//!     "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEA11qYAYKxCrfVS/7TyWQHOg7hcvPapiMlrwIaaPcHURo=\n-----END PUBLIC KEY-----\n"
//! );
//! assert_eq!(key.wireguard_public(), "2F4H7CKwrYgVN8L0TWYtGhQ8+DDFespDBdhcepD2ti4=");
//! // The secret never shows in Debug.
//! assert!(!format!("{key:?}").contains("MHyDhk8o"));
//! ```
use base64::{engine::general_purpose::STANDARD, Engine};
use curve25519_dalek::{EdwardsPoint, MontgomeryPoint};
use sha2::{Digest, Sha512};
use zeroize::{Zeroize, Zeroizing};

/// DER prefix of an Ed25519 SubjectPublicKeyInfo (RFC 8410).
const SPKI_PREFIX: [u8; 12] = [
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
];

/// An Ed25519 seed and everything derived from it.
#[derive(Clone)]
pub struct ProfileKey {
    seed: Zeroizing<[u8; 32]>,
}

impl std::fmt::Debug for ProfileKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProfileKey")
            .field("wireguard_public", &self.wireguard_public())
            .finish_non_exhaustive()
    }
}

impl ProfileKey {
    /// A fresh key from the OS's random source.
    pub fn generate() -> Result<Self, getrandom::Error> {
        let mut seed = Zeroizing::new([0u8; 32]);
        getrandom::fill(seed.as_mut())?;
        Ok(Self { seed })
    }

    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self {
            seed: Zeroizing::new(seed),
        }
    }

    /// The seed as stored (base64), for the profile's own Secret.
    pub fn to_stored(&self) -> Zeroizing<String> {
        Zeroizing::new(STANDARD.encode(self.seed.as_ref()))
    }

    /// Read back what [`ProfileKey::to_stored`] wrote.
    pub fn from_stored(text: &str) -> Result<Self, String> {
        let mut bytes = Zeroizing::new(
            STANDARD
                .decode(text.trim())
                .map_err(|_| "the stored profile key is not base64".to_owned())?,
        );
        let seed: [u8; 32] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| "the stored profile key is not 32 bytes".to_owned())?;
        bytes.zeroize();
        Ok(Self::from_seed(seed))
    }

    /// The clamped X25519 scalar: SHA-512(seed)[..32], as Ed25519 and
    /// libsodium's conversion both use it.
    fn scalar(&self) -> Zeroizing<[u8; 32]> {
        let digest = Zeroizing::new(Sha512::digest(self.seed.as_ref()));
        let mut scalar = Zeroizing::new([0u8; 32]);
        scalar.copy_from_slice(&digest[..32]);
        scalar[0] &= 248;
        scalar[31] &= 127;
        scalar[31] |= 64;
        scalar
    }

    /// The Ed25519 public key as PEM, Proton's `ClientPublicKey`.
    pub fn public_pem(&self) -> String {
        let public = EdwardsPoint::mul_base_clamped(*self.scalar()).compress();
        let mut der = SPKI_PREFIX.to_vec();
        der.extend_from_slice(public.as_bytes());
        format!(
            "-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----\n",
            STANDARD.encode(der)
        )
    }

    /// The WireGuard private key (base64), for `wg.conf` only.
    pub fn wireguard_private(&self) -> Zeroizing<String> {
        Zeroizing::new(STANDARD.encode(self.scalar().as_ref()))
    }

    /// The WireGuard public key (base64).
    pub fn wireguard_public(&self) -> String {
        STANDARD.encode(MontgomeryPoint::mul_base_clamped(*self.scalar()).as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wireguard_key_is_the_converted_ed25519_key() {
        let key = ProfileKey::generate().unwrap();
        // The Montgomery form of the Ed25519 public key is the WireGuard
        // public key: what Proton certifies is what the server will see.
        let edwards = EdwardsPoint::mul_base_clamped(*key.scalar());
        assert_eq!(
            STANDARD.encode(edwards.to_montgomery().as_bytes()),
            key.wireguard_public()
        );
        let again = ProfileKey::from_stored(&key.to_stored()).unwrap();
        assert_eq!(again.wireguard_public(), key.wireguard_public());
        assert_ne!(
            ProfileKey::generate().unwrap().wireguard_public(),
            key.wireguard_public()
        );
        assert!(ProfileKey::from_stored("AAAA").is_err());
        assert!(ProfileKey::from_stored("not base64!").is_err());
    }
}

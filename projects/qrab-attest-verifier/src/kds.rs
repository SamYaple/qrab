//! AMD Key Distribution Service (KDS) VCEK / VLEK / cert-chain
//! fetcher.
//!
//! AMD publishes a VCEK per `(chip_id, TCB)` tuple at
//! `https://kdsintf.amd.com/vcek/v1/{product}/{chip_id_hex}?blSPL=...&teeSPL=...&snpSPL=...&ucodeSPL=...`
//! The `product` segment is one of `Milan`, `Genoa`, `Bergamo`,
//! `Turin` — the "chip family" in marketing terms. The chip id is
//! the 64-byte `report.chip_id` in hex.
//!
//! The returned body is a DER-encoded `X.509` certificate (single
//! VCEK). The ARK + ASK are served separately under
//! `/vcek/v1/{product}/cert_chain` and are stable for the product
//! family — one fetch at process startup + local cache suffices.
//!
//! All endpoints are HTTPS, served by AMD's WAF on Cloudfront-ish
//! infrastructure. `ureq`'s `tls` feature pulls in `rustls` which
//! uses Mozilla's CA bundle via `webpki-roots`; AMD's cert is
//! signed by DigiCert and validates cleanly.
//!
//! ## Caching
//!
//! This module does not cache. Callers should key a local disk /
//! in-memory cache by `(product, chip_id, TCB)`. AMD rate-limits
//! the VCEK endpoint (~5 req/min/IP as of 2026-04); a busy
//! attest-server *must* cache or it will start seeing 429s within
//! a handful of guest boots.
//!
//! ## attest-server integration status (B12-15)
//!
//! Currently `attest-server/src/verify.rs::handle_post_report_bytes`
//! returns `PostReportError::AskMissing` / `VcekMissing` directly
//! when the guest's `cert_blob` lacks an ASK/VCEK entry — it does
//! NOT fall back to `fetch_vcek_from_kds` from this module. The
//! fallback is deliberately deferred until a persistent cache is in
//! place (the AMD KDS rate-limit would otherwise turn every cold
//! boot of a chain-less guest into a 429 storm). Operators who need
//! the fallback today point `QRAB_ATTEST_VERIFIER_ASK_PEM` /
//! `..._VCEK_PEM` at a local PEM file. Wiring the in-process KDS
//! call lives under `reference/followups.md` — do not remove this
//! module assuming it is dead code.

use std::time::Duration;

use thiserror::Error;

/// AMD chip families served by KDS. Each maps to a URL segment.
/// `Display` yields the exact case-sensitive token the KDS URL
/// expects — AMD's endpoint 404s on lowercase `milan`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KdsProduct {
    /// 3rd-gen EPYC (Milan, 7003 series). SEV-SNP introduced here.
    Milan,
    /// 4th-gen EPYC (Genoa, 9004 series). Current dev host family.
    Genoa,
    /// 4th-gen EPYC-C (Bergamo, 9704 series).
    Bergamo,
    /// 5th-gen EPYC (Turin, 9005 series).
    Turin,
}

impl core::fmt::Display for KdsProduct {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Milan => "Milan",
            Self::Genoa => "Genoa",
            Self::Bergamo => "Bergamo",
            Self::Turin => "Turin",
        })
    }
}

/// TCB-version components. All four go on the VCEK URL query
/// string; each is a u8 (0..255). `reported_tcb` in the
/// attestation report packs these into a single u64 — callers are
/// expected to unpack once and pass the four bytes here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcbParts {
    /// Bootloader SPL.
    pub bl: u8,
    /// TEE (PSP OS) SPL.
    pub tee: u8,
    /// SNP firmware SPL.
    pub snp: u8,
    /// Microcode SPL.
    pub ucode: u8,
}

impl TcbParts {
    /// Unpack the 64-bit `reported_tcb` field as laid out in the
    /// AMD SEV-SNP ABI spec §5.5.2 Table 10 — bytes 0,1,6,7 of the
    /// little-endian encoding hold bootloader, tee, snp, ucode.
    pub fn from_reported_tcb_le(value: u64) -> Self {
        let b = value.to_le_bytes();
        Self { bl: b[0], tee: b[1], snp: b[6], ucode: b[7] }
    }
}

/// Base URL for the KDS endpoint. Overridable via
/// [`fetch_vcek_from_kds_with`] for tests that point at a mock.
pub const KDS_BASE_URL: &str = "https://kdsintf.amd.com";

/// Errors from [`fetch_vcek_from_kds`] and
/// [`fetch_cert_chain_from_kds`].
#[derive(Debug, Error)]
pub enum KdsError {
    /// `ureq` / TLS / DNS level failure.
    #[error("HTTP transport: {0}")]
    Transport(String),
    /// KDS returned a non-2xx response. The embedded `status` is
    /// the HTTP code (404 = no VCEK for that chip_id+TCB; 429 =
    /// rate-limited; 5xx = KDS outage).
    #[error("HTTP {status} {body}")]
    HttpStatus { status: u16, body: String },
    /// Response body was not a well-formed DER certificate.
    #[error("KDS returned {0} bytes; not a DER cert")]
    MalformedDer(usize),
}

/// Format the chip-id bytes as lower-case hex for the KDS URL.
/// AMD accepts uppercase too, but lowercase is the convention in
/// KDS docs + Linux sev-guest driver.
pub fn format_chip_id(chip_id: &[u8; 64]) -> String {
    let mut s = String::with_capacity(128);
    for b in chip_id {
        use core::fmt::Write as _;
        write!(&mut s, "{b:02x}").expect("string push cannot fail");
    }
    s
}

/// Build the KDS URL for a given VCEK lookup.
pub fn build_vcek_url(
    base: &str,
    product: KdsProduct,
    chip_id: &[u8; 64],
    tcb: TcbParts,
) -> String {
    format!(
        "{base}/vcek/v1/{product}/{chip_id}?blSPL={bl:02}&teeSPL={tee:02}&snpSPL={snp:02}&ucodeSPL={ucode:02}",
        chip_id = format_chip_id(chip_id),
        bl = tcb.bl,
        tee = tcb.tee,
        snp = tcb.snp,
        ucode = tcb.ucode,
    )
}

/// Build the KDS URL for the ARK + ASK cert chain bundle.
/// AMD returns a PEM concatenation of `ASK || ARK`.
pub fn build_cert_chain_url(base: &str, product: KdsProduct) -> String {
    format!("{base}/vcek/v1/{product}/cert_chain")
}

/// Fetch the VCEK DER for `(product, chip_id, tcb)` from the
/// default AMD KDS endpoint with a 15 s timeout. Blocking.
pub fn fetch_vcek_from_kds(
    product: KdsProduct,
    chip_id: &[u8; 64],
    tcb: TcbParts,
) -> Result<Vec<u8>, KdsError> {
    fetch_vcek_from_kds_with(
        KDS_BASE_URL,
        product,
        chip_id,
        tcb,
        crate::KDS_DEFAULT_TIMEOUT,
    )
}

/// Same as [`fetch_vcek_from_kds`] but lets tests override the
/// base URL + timeout. The returned bytes are the raw DER body —
/// we do a cheap sanity check that the prefix looks like an
/// `SEQUENCE` tag (`0x30`) so a WAF HTML-error page doesn't
/// silently replace the DER.
pub fn fetch_vcek_from_kds_with(
    base: &str,
    product: KdsProduct,
    chip_id: &[u8; 64],
    tcb: TcbParts,
    timeout: Duration,
) -> Result<Vec<u8>, KdsError> {
    let url = build_vcek_url(base, product, chip_id, tcb);
    http_get(&url, timeout).and_then(sanity_check_der)
}

fn http_get(url: &str, timeout: Duration) -> Result<Vec<u8>, KdsError> {
    let agent = ureq::AgentBuilder::new()
        .timeout(timeout)
        .build();
    let resp = match agent.get(url).call() {
        Ok(r) => r,
        Err(ureq::Error::Status(code, r)) => {
            let body = r.into_string().unwrap_or_else(|_| "<non-utf8>".into());
            return Err(KdsError::HttpStatus { status: code, body });
        }
        Err(e) => return Err(KdsError::Transport(e.to_string())),
    };
    let mut buf = Vec::new();
    if let Err(e) = resp.into_reader().take(1 << 20 /* 1 MiB cap */).read_to_end(&mut buf) {
        return Err(KdsError::Transport(e.to_string()));
    }
    Ok(buf)
}

fn sanity_check_der(bytes: Vec<u8>) -> Result<Vec<u8>, KdsError> {
    // DER-encoded X.509 always starts with `SEQUENCE` (tag 0x30)
    // followed by a length encoding. HTML error pages from AMD's
    // WAF start with `<`, which is 0x3C — close in ASCII but not
    // 0x30. A single-byte check catches the common substitution.
    match bytes.first() {
        Some(0x30) => Ok(bytes),
        _ => Err(KdsError::MalformedDer(bytes.len())),
    }
}

use std::io::Read as _;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_url_segment() {
        assert_eq!(format!("{}", KdsProduct::Milan), "Milan");
        assert_eq!(format!("{}", KdsProduct::Genoa), "Genoa");
        assert_eq!(format!("{}", KdsProduct::Bergamo), "Bergamo");
        assert_eq!(format!("{}", KdsProduct::Turin), "Turin");
    }

    #[test]
    fn vcek_url_shape() {
        let chip_id = [0xabu8; 64];
        let tcb = TcbParts { bl: 3, tee: 0, snp: 20, ucode: 219 };
        let url = build_vcek_url("https://kdsintf.amd.com", KdsProduct::Genoa, &chip_id, tcb);
        // chip_id 128 hex chars + fixed query string
        assert!(url.starts_with("https://kdsintf.amd.com/vcek/v1/Genoa/"));
        assert!(url.contains("blSPL=03"));
        assert!(url.contains("teeSPL=00"));
        assert!(url.contains("snpSPL=20"));
        assert!(url.contains("ucodeSPL=219"));
    }

    #[test]
    fn cert_chain_url_shape() {
        assert_eq!(
            build_cert_chain_url("https://kdsintf.amd.com", KdsProduct::Milan),
            "https://kdsintf.amd.com/vcek/v1/Milan/cert_chain"
        );
    }

    #[test]
    fn format_chip_id_lowercase() {
        let mut cid = [0u8; 64];
        cid[0] = 0xAB;
        cid[63] = 0xCD;
        let hex = format_chip_id(&cid);
        assert_eq!(hex.len(), 128);
        assert!(hex.starts_with("ab"));
        assert!(hex.ends_with("cd"));
    }

    #[test]
    fn tcb_unpacks_from_reported_le() {
        // bl=0x03, tee=0x00, (reserved), snp=0x14, ucode=0xDB
        // packed as LE bytes 0,1,_,_,_,_,6,7
        let packed: u64 = u64::from_le_bytes([0x03, 0x00, 0, 0, 0, 0, 0x14, 0xDB]);
        let parts = TcbParts::from_reported_tcb_le(packed);
        assert_eq!(parts.bl, 3);
        assert_eq!(parts.tee, 0);
        assert_eq!(parts.snp, 0x14);
        assert_eq!(parts.ucode, 0xDB);
    }

    #[test]
    fn sanity_check_rejects_html_error() {
        // An HTML error page bytes-in: 0x3C == '<'.
        let err = sanity_check_der(b"<html>error</html>".to_vec()).unwrap_err();
        match err {
            KdsError::MalformedDer(n) => assert_eq!(n, 18),
            other => panic!("expected MalformedDer, got {other:?}"),
        }
    }

    #[test]
    fn sanity_check_accepts_der_sequence() {
        // Minimal-enough: tag 0x30 + zero length.
        let ok = sanity_check_der(vec![0x30, 0x00]).unwrap();
        assert_eq!(ok, vec![0x30, 0x00]);
    }

    /// Live KDS hit. Only runs when `RUN_LIVE_KDS=1` — AMD rate-
    /// limits aggressively, so the default test run must not
    /// depend on network. The chip id + TCB here don't matter —
    /// we only assert that the endpoint returns a 404 / valid DER
    /// shape, not the cert contents.
    #[test]
    fn live_kds_responds() {
        if std::env::var_os("RUN_LIVE_KDS").is_none() {
            eprintln!("skipped — set RUN_LIVE_KDS=1 to hit AMD KDS live");
            return;
        }
        let chip_id = [0u8; 64];
        let tcb = TcbParts { bl: 3, tee: 0, snp: 20, ucode: 219 };
        let res = fetch_vcek_from_kds(KdsProduct::Genoa, &chip_id, tcb);
        // All-zero chip id isn't real, so expect a 404 from KDS.
        // Any success / other error is still a signal the endpoint
        // is reachable — that's what this test is for.
        match res {
            Err(KdsError::HttpStatus { status: 404, .. }) => { /* expected */ }
            Ok(_) => panic!("all-zero chip id should 404, got cert?!"),
            Err(e) => panic!("live KDS reachable but errored unexpectedly: {e:?}"),
        }
    }
}

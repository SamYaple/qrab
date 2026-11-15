#![cfg(feature = "http")]
#[test]
fn a_preinstalled_tls12_provider_is_rejected_instead_of_downgraded() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap();
    // In this isolated binary, ordinary rustls TLS1.2 suites are compiled in by
    // the HTTP client feature. A preinstalled nonconforming provider is refused.
    assert!(qrab_spiffe::http::install_tls13_provider().is_err());
}

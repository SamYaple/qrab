# SNP cert pins — ARK bundle

**Contents:** `ARK-Genoa.pem` is the AMD Root Key for Genoa (EPYC 9xx4,
Zen 4) that `attest-server` pins when verifying a guest-supplied
attestation report chain (F2). Reserve slots for `ARK-Milan.pem`,
`ARK-Bergamo.pem`, `ARK-Turin.pem` will appear as the project
tests on those generations.

## DEV PLACEHOLDER WARNING

`ARK-Genoa.pem` in this directory is **not** AMD's real ARK. It is a
self-signed RSA-4096 SHA-384 cert generated at build time so
`include_str!` has something parseable to compile against + so
hermetic tests that override via `QRAB_ATTEST_VERIFIER_ARK_PEM` do
not need to craft a PEM at runtime.

A real AMD ARK for Genoa is published at:

```
https://kdsintf.amd.com/vcek/v1/Genoa/cert_chain
```

The returned PEM is a concatenation of `ASK || ARK` — split on the
cert boundary and commit only the ARK half to replace this
placeholder. Verify the fingerprint against AMD's published SHA-256
digest before trusting any production path.

## Loading order in `attest-server`

1. `QRAB_ATTEST_VERIFIER_ARK_PEM=/path/to/ARK.pem` — env override.
2. Else: built-in `include_str!("../../contrib/snp-cert-pins/ARK-Genoa.pem")`.

`attest-server` logs `QRAB_ATTEST_ARK_SOURCE=env|bundle` on startup
so an operator can see which ARK is active.

## Refreshing

```
curl -sSfo /tmp/ASK_ARK.pem https://kdsintf.amd.com/vcek/v1/Genoa/cert_chain
# Sanity: SHA-256 the returned blob, compare to AMD's published
# value before proceeding.
# Split: the second cert block is the ARK.
awk '/BEGIN CERTIFICATE/{n++} n==2' /tmp/ASK_ARK.pem \
    > contrib/snp-cert-pins/ARK-Genoa.pem
```

Bump `contrib/snp-cert-pins/VERSION` (if added) when refreshing, so
downstream BMC policy can refuse a stale pin.

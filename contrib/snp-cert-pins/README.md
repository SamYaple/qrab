# SNP cert pins — ARK bundle

**Contents:** `ARK-Genoa.pem` is the AMD Root Key for Genoa (EPYC 9xx4,
Zen 4) that `attest-server` pins when verifying a guest-supplied
attestation report chain (F2). Reserve slots for `ARK-Milan.pem`,
`ARK-Bergamo.pem`, `ARK-Turin.pem` will appear as the project
tests on those generations.

## Current pin: AMD ARK-Genoa (real, 2026-04-20)

`ARK-Genoa.pem` contains the genuine AMD Root Key for Genoa/Zen 4,
fetched from `https://kdsintf.amd.com/vcek/v1/Genoa/cert_chain`
(the second block; the first is the ASK).

Fingerprint (SHA-256):
`4C:65:98:D1:9C:18:71:9C:5D:FD:4A:7D:33:5F:67:4E:5B:FE:1D:8F:80:0C:EA:2C:F2:70:C1:0D:10:3D:B2:F1`

Subject = `CN=ARK-Genoa, O=Advanced Micro Devices, OU=Engineering`.
Self-signed RSA-4096 PSS SHA-384.

If this fingerprint drifts without a deliberate refresh commit,
treat as a supply-chain incident — the ARK is the root of the
`attest-server` trust chain when no `QRAB_ATTEST_VERIFIER_ARK_PEM`
env override is set.

Refresh URL (for when AMD rotates a future ARK generation):

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

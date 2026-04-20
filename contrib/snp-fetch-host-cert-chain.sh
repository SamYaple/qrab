#!/usr/bin/env bash
# Populate $ROOT/target/snp-cert-chain/ with the host's SNP cert
# chain (ARK + ASK from AMD KDS, VCEK bound to this chip) so QEMU
# can forward them to the guest via `-object sev-snp-guest,certs-path=`.
#
# snphost 0.7 exposes KDS fetches under `snphost fetch`:
#   snphost fetch ca  pem <dir>         -> ARK + ASK (product-level)
#   snphost fetch vek pem <dir>         -> VCEK (chip-level)
#
# Optional refresh: re-run at any time. Files written:
#   $OUT/ark.pem   AMD root key (Genoa by default)
#   $OUT/ask.pem   AMD signing key
#   $OUT/vcek.pem  chip-unique endorsement key
#
# Exit 0: directory populated.
# Exit 1: snphost missing / fetch failed.
#
# Consumed by: contrib/smoke-snp-attest.sh with QRAB_SNP_USE_CERT_CHAIN=1.
set -euo pipefail

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
OUT="${QRAB_SNP_CERT_CHAIN_DIR:-$ROOT/target/snp-cert-chain}"
mkdir -p "$OUT"

command -v snphost >/dev/null 2>&1 \
    || { echo "error: snphost not on PATH (virtee/snphost crate)" >&2; exit 1; }

echo "fetch: CA (ARK + ASK) → $OUT"
snphost fetch ca pem "$OUT"

echo "fetch: VCEK → $OUT"
snphost fetch vek pem "$OUT"

ls -la "$OUT"
echo "ok: cert chain ready at $OUT"

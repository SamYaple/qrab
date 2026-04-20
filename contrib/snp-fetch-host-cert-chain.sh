#!/usr/bin/env bash
# Populate $ROOT/target/snp-cert-chain/ from the host PSP so QEMU
# can pass it into the guest via `-object sev-snp-guest,certs-path=`.
#
# Needs snphost on PATH + read access to /dev/sev. On an EPYC
# Genoa system the canonical output is an ARK / ASK / VCEK trio
# in PEM (or DER) form that AMD's kernel driver surfaces.
#
# Exit 0: directory populated (ls output shown).
# Exit 1: snphost absent / export failed / the operator needs to
#         populate manually.
#
# Consumed by:  contrib/smoke-snp-attest.sh with QRAB_SNP_USE_CERT_CHAIN=1.
set -euo pipefail

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
OUT="${QRAB_SNP_CERT_CHAIN_DIR:-$ROOT/target/snp-cert-chain}"
mkdir -p "$OUT"

command -v snphost >/dev/null 2>&1 \
    || { echo "error: snphost not on PATH (virtee/snphost crate)" >&2; exit 1; }

# snphost 0.7's export command shape has drifted across versions.
# Try the modern form first, fall back to the legacy subcommand,
# and as a last resort surface a clear "do it yourself" message.
echo "fetch: snphost export → $OUT"
if snphost export --encoding pem "$OUT" 2>/dev/null; then
    :
elif snphost certs export "$OUT" 2>/dev/null; then
    :
elif snphost export-cert-chain --directory "$OUT" 2>/dev/null; then
    :
else
    echo "error: snphost export paths all failed. Populate $OUT manually:" >&2
    echo "  - ARK (root) as ARK.pem" >&2
    echo "  - ASK (intermediate) as ASK.pem" >&2
    echo "  - VCEK (chip leaf) as VCEK.pem" >&2
    echo "See contrib/snp-cert-pins/README.md for AMD KDS endpoints." >&2
    exit 1
fi

ls -la "$OUT"
echo "ok: cert chain ready at $OUT"

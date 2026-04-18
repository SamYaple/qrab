#!/usr/bin/env bash
# Generate an ECDSA-P256 signing key for attest-server and export the
# SEC1-uncompressed public half where qrab-boot's build.rs picks it up.
#
#   ./contrib/gen-attest-keys.sh              # skip if keys exist
#   ./contrib/gen-attest-keys.sh --force      # regenerate
#
# Writes:
#   contrib/attest-keys/server.pem        PKCS#8 PEM private key
#   contrib/attest-keys/server.sec1       65-byte 0x04||X||Y pubkey
#
# Intended for dev flows only — production should use an HSM-backed
# key and a sealed export for the firmware rebuild.
set -euo pipefail

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
KEYDIR="$ROOT/contrib/attest-keys"
PEM="$KEYDIR/server.pem"
SEC1="$KEYDIR/server.sec1"

mkdir -p "$KEYDIR"

force=false
if [[ "${1:-}" == "--force" ]]; then
    force=true
fi

if [[ -s "$PEM" && -s "$SEC1" && "$force" == "false" ]]; then
    echo "gen-attest-keys: reuse existing $PEM + $SEC1 (pass --force to regenerate)" >&2
    exit 0
fi

command -v openssl >/dev/null || { echo "openssl not on PATH" >&2; exit 1; }
command -v cargo   >/dev/null || { echo "cargo not on PATH"   >&2; exit 1; }

# 1. Generate PKCS#8 PEM ECDSA-P256 key.
openssl genpkey \
    -algorithm EC \
    -pkeyopt ec_paramgen_curve:P-256 \
    -pkeyopt ec_param_enc:named_curve \
    -out "$PEM" 2>/dev/null
chmod 0600 "$PEM"
echo "gen-attest-keys: wrote $PEM" >&2

# 2. Have attest-server emit the SEC1 pubkey for the build to embed.
#    attest-server reads QRAB_ATTEST_KEY_PATH and drops the 65-byte
#    0x04||X||Y blob at the path we pass.
QRAB_ATTEST_KEY_PATH="$PEM" \
    cargo run --quiet -p attest-server -- \
        --write-pubkey-sec1 "$SEC1"
echo "gen-attest-keys: wrote $SEC1 ($(stat -c%s "$SEC1") bytes)" >&2

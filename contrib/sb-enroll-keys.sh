#!/usr/bin/env bash
# Generate the qrab-boot Secure Boot key material:
#   PK  — Platform Key (owns the SB hierarchy)
#   KEK — Key Exchange Key (signs db/dbx updates)
#   db  — signs UEFI applications the firmware will launch (our UKIs, and
#         optionally qrab-boot.efi itself)
#
# Algorithm: RSA-2048 / SHA-256. This is the broadest-compatibility choice
# for UEFI Secure Boot (PE Authenticode). ECDSA works too but RSA-2048 is
# what every firmware and signing tool we care about has been tested
# against.
#
# Output (all under contrib/sb-keys/, git-ignored):
#   PK.key / PK.crt       PEM private key + self-signed X.509
#   KEK.key / KEK.crt
#   db.key / db.crt
#   info.txt              fingerprints + creation timestamp
#
# Re-running is idempotent *unless* QRAB_SB_FORCE=1 is set; this prevents
# accidentally rotating the keys that a built firmware has baked in.
#
# Later phases (2b) will turn these certs into ESLs and .auth blobs for
# firmware enrollment. Phase 2a stops at the PEM key/cert pairs because
# that is all `sbsign` needs.
set -euo pipefail

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
KEYDIR="$ROOT/contrib/sb-keys"
VALIDITY_DAYS="${QRAB_SB_VALIDITY_DAYS:-3650}"

die() { echo "sb-enroll-keys.sh: error: $*" >&2; exit 1; }
log() { echo "sb-enroll-keys.sh: $*"; }

mkdir -p "$KEYDIR"
chmod 700 "$KEYDIR"

if [[ -f "$KEYDIR/db.key" && "${QRAB_SB_FORCE:-0}" != 1 ]]; then
    log "keys already exist at $KEYDIR; refusing to overwrite"
    log "  rerun with QRAB_SB_FORCE=1 to rotate (breaks existing firmware builds!)"
    exit 0
fi

gen() {
    local name="$1" subject="$2"
    openssl req \
        -newkey rsa:2048 \
        -nodes \
        -sha256 \
        -x509 \
        -keyout "$KEYDIR/${name}.key" \
        -out    "$KEYDIR/${name}.crt" \
        -days   "$VALIDITY_DAYS" \
        -subj   "$subject" 2>/dev/null
    chmod 600 "$KEYDIR/${name}.key"
    chmod 644 "$KEYDIR/${name}.crt"
    log "generated $name → $KEYDIR/${name}.{key,crt}"
}

gen PK  '/CN=qrab-boot Platform Key/O=qrab-boot dev/OU=SB-PK'
gen KEK '/CN=qrab-boot Key Exchange Key/O=qrab-boot dev/OU=SB-KEK'
gen db  '/CN=qrab-boot db signer/O=qrab-boot dev/OU=SB-db'

# Record fingerprints so a reviewer can tell which keys a firmware build
# was pinned to.
{
    echo "# qrab-boot SB keys"
    echo "# generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "# validity : $VALIDITY_DAYS days"
    echo
    for name in PK KEK db; do
        printf '%-4s  %s  %s\n' "$name" \
            "$(openssl x509 -in "$KEYDIR/${name}.crt" -noout -fingerprint -sha256 | cut -d= -f2)" \
            "$(openssl x509 -in "$KEYDIR/${name}.crt" -noout -subject    | sed 's/^subject= *//')"
    done
} > "$KEYDIR/info.txt"

log "info:"
sed 's/^/  /' "$KEYDIR/info.txt"

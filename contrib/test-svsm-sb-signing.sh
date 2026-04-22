#!/usr/bin/env bash
# Verify sign-efi.sh + db keys produce an sbverify-passing signature.
# Uses a throwaway PE+ stub (the EFI shell from edk2) as signing input if
# available, otherwise synthesises a minimal PE that sbsign will accept.
# Fast: no edk2 or qrab-boot build.
set -euo pipefail

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
KEYDIR="$ROOT/contrib/sb-keys"

die() { echo "test-svsm-sb-signing: $*" >&2; exit 1; }
ok()  { printf "  \033[32mOK\033[0m   %s\n" "$*"; }
bad() { printf "  \033[31mFAIL\033[0m %s\n" "$*"; FAIL=1; }

FAIL=0

# --- 1. prerequisites ----------------------------------------------------
for t in sbsign sbverify openssl; do
    command -v "$t" >/dev/null 2>&1 || die "required tool '$t' not on PATH"
done
[[ -f "$KEYDIR/db.key" && -f "$KEYDIR/db.crt" ]] \
    || die "contrib/sb-keys/ missing db.key or db.crt; run sb-enroll-keys.sh first"
ok "prereqs satisfied (sbsign, sbverify, openssl, db.key, db.crt)"

# --- 2. build a tiny PE+ stub to sign ------------------------------------
# Use edk2's prebuilt UEFI shell if the AmdSev staging has one, else fall
# back to a minimal PE synthesised by `objcopy` from a small C blob.
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# Simplest PE that sbsign will accept: the stock UEFI Shell .efi included
# with gnu-efi or edk2-ovmf binary packages. Try a few common paths.
STUB=""
for p in /usr/share/edk2/ovmf/Shell.efi \
         /usr/share/ovmf/Shell.efi \
         /usr/share/edk2-ovmf/Shell.efi \
         /usr/share/edk2/AmdSev/OVMF.fd; do
    if [[ -f "$p" ]]; then STUB="$p"; break; fi
done
if [[ -z "$STUB" ]]; then
    # Synthesise via gcc + pe-riscof style link. Cheapest: copy any small PE+
    # we have. /usr/bin/qemu-system-x86_64 is ELF, not PE — not usable.
    # Generate a minimal stub via mkpe-like approach: use lld's pe output.
    cat > "$TMP/stub.c" <<'EOF'
typedef unsigned long long UINTN;
typedef unsigned short CHAR16;
UINTN __attribute__((ms_abi))
efi_main(void *ImageHandle, void *SystemTable) { (void)ImageHandle; (void)SystemTable; return 0; }
EOF
    # Try clang with PE target (lld-link bundled). Fall back to skip if not
    # available.
    if command -v clang >/dev/null 2>&1 && command -v lld-link >/dev/null 2>&1 || \
       clang --version >/dev/null 2>&1; then
        if clang -target x86_64-unknown-windows -ffreestanding -fshort-wchar \
            -c "$TMP/stub.c" -o "$TMP/stub.o" 2>/dev/null && \
           clang -target x86_64-unknown-windows -nostdlib -fuse-ld=lld \
            -Wl,-subsystem:efi_application -Wl,-entry:efi_main \
            "$TMP/stub.o" -o "$TMP/stub.efi" 2>/dev/null; then
            STUB="$TMP/stub.efi"
            ok "synthesised PE+ stub via clang+lld"
        fi
    fi
fi
if [[ -z "$STUB" ]]; then
    echo "  SKIP no PE+ stub available (install edk2-ovmf or clang+lld to enable)"
    echo
    echo "test-svsm-sb-signing: SKIPPED (environment lacks PE+ source)"
    exit 0
fi
cp "$STUB" "$TMP/in.efi"
ok "signing input: $STUB"

# --- 3. sign + verify round-trip ----------------------------------------
bash "$ROOT/contrib/sign-efi.sh" "$TMP/in.efi" "$TMP/out.efi" >/dev/null \
    || bad "sign-efi.sh returned non-zero"
[[ -f "$TMP/out.efi" ]] || bad "sign-efi.sh did not produce output"

if sbverify --cert "$KEYDIR/db.crt" "$TMP/out.efi" >/dev/null 2>&1; then
    ok "sbverify accepts signed image against db.crt"
else
    bad "sbverify rejected signed image"
fi

# --- 4. negative: unrelated cert rejects ---------------------------------
# Generate a throwaway cert that is NOT the db.crt. Signed image MUST be
# rejected against it.
openssl req -x509 -newkey rsa:2048 -nodes -keyout "$TMP/other.key" \
    -out "$TMP/other.crt" -days 1 -subj '/CN=test-other' 2>/dev/null
if sbverify --cert "$TMP/other.crt" "$TMP/out.efi" >/dev/null 2>&1; then
    bad "sbverify accepted signed image against UNRELATED cert — fail-closed broken"
else
    ok "sbverify rejects signed image against unrelated cert (fail-closed)"
fi

# --- summary -------------------------------------------------------------
if [[ $FAIL -eq 0 ]]; then
    echo
    echo "test-svsm-sb-signing: all checks passed"
    exit 0
else
    echo
    echo "test-svsm-sb-signing: FAILED"
    exit 1
fi

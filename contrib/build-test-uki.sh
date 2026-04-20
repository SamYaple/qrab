#!/usr/bin/env bash
# Build the real-UKI fixture used by `contrib/matrix-smoke.sh`
# scenario 1r (follow-up F10).
#
# The fixture is a standalone `x86_64-unknown-uefi` PE produced by the
# `projects/qrab-real-uki-demo` crate, which is excluded from the
# top-level workspace for the same reason `qrab-boot` is (see the
# `[workspace.exclude]` comment in the root `Cargo.toml`). On
# execution the PE writes a distinctive `QRAB_REAL_UKI_BOOT_OK` token
# to the QEMU debugcon port and halts — enough to prove that
# `qrab-boot`'s `BootServices::load_image` + `start_image` chain
# really transfers control (rather than silently skipping on the
# scenario-1 stub).
#
# Idempotent: if the staged artefact already exists and is newer than
# the crate's `src/main.rs` / `Cargo.toml` it is kept as-is. Pass
# `--force` or set `QRAB_BUILD_UKI_FORCE=1` to always rebuild.
#
# Exit codes:
#   0  fixture built or already current
#   1  build failure / size budget exceeded
set -euo pipefail

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
CRATE="$ROOT/projects/qrab-real-uki-demo"
BIN="$CRATE/target/x86_64-unknown-uefi/release/qrab-real-uki-demo.efi"
STAGE_DIR="$ROOT/projects/qrab-boot-tests/fixtures"
STAGE="$STAGE_DIR/uki-real.efi"

# `MAX_FRAMED_PAYLOAD` in `projects/qrab-boot-proto/src/lib.rs` — the
# vsock wire cap the PE must fit under. Changing that constant
# requires updating this value too.
MAX_BYTES=131072

log() { echo "build-test-uki: $*" >&2; }
die() { log "error: $*"; exit 1; }

force=0
for arg in "$@"; do
    case "$arg" in
        --force) force=1 ;;
        -h|--help)
            cat <<EOF
usage: $0 [--force]

Builds projects/qrab-real-uki-demo and stages the resulting PE at
projects/qrab-boot-tests/fixtures/uki-real.efi. Idempotent — rebuilds
only when the source is newer than the staged fixture (or --force).
EOF
            exit 0
            ;;
        *) die "unknown arg: $arg" ;;
    esac
done
if [[ "${QRAB_BUILD_UKI_FORCE:-0}" == "1" ]]; then
    force=1
fi

command -v cargo >/dev/null || die "cargo not on PATH"

mkdir -p "$STAGE_DIR"

# Idempotency gate: if the staged fixture exists and is newer than
# every source input, skip the cargo build entirely. Keeps matrix-
# smoke.sh's "run me on every invocation" invocation cheap.
if [[ $force -eq 0 && -s "$STAGE" ]]; then
    newest_src=$(find "$CRATE/src" "$CRATE/Cargo.toml" \
                      "$CRATE/rust-toolchain.toml" "$CRATE/.cargo" \
                      -type f -printf '%T@\n' 2>/dev/null | sort -nr | head -1)
    stage_mtime=$(stat -c '%Y' "$STAGE")
    if [[ -n "$newest_src" ]] \
        && awk -v s="$newest_src" -v t="$stage_mtime" \
               'BEGIN{exit !(s+0 <= t+0)}'; then
        size=$(stat -c '%s' "$STAGE")
        log "staged fixture current ($size B, budget $MAX_BYTES B)"
        exit 0
    fi
fi

log "building qrab-real-uki-demo (release)"
(cd "$CRATE" && cargo build --release >&2) || die "cargo build failed"

[[ -f "$BIN" ]] || die "expected output $BIN not produced"

size=$(stat -c '%s' "$BIN")
if (( size > MAX_BYTES )); then
    die "EFI PE is $size bytes, exceeds MAX_FRAMED_PAYLOAD $MAX_BYTES"
fi

install -m 0644 "$BIN" "$STAGE"
log "staged $STAGE ($size B, budget $MAX_BYTES B)"

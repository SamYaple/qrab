#!/usr/bin/env bash
# Build an AmdSev OVMF firmware image with qrab-boot.efi embedded in place
# of the upstream GRUB UEFI application.
#
# This script never mutates `vendor/edk2/`. It rsyncs the pinned submodule
# into `target/edk2-build/edk2/`, recursively initialises edk2's own
# submodules there (openssl, brotli, etc.), substitutes qrab-boot.efi for
# the upstream grub.efi binary, neutralises the GRUB PREBUILD hook, and
# then runs the standard edk2 `build` tool.
#
# Outputs:
#   target/edk2-build/edk2/Build/AmdSev/RELEASE_GCC5/FV/OVMF.fd
#   target/ovmf/OVMF.fd                                    (copy)
#
# Prerequisites (Debian/Ubuntu names; similar on other distros):
#   build-essential iasl nasm uuid-dev python3 python-is-python3 git rsync
#
# Typical build time on a modern x86_64 host: ~5-10 min for first build,
# ~1 min incremental.
set -euo pipefail

die() { echo "build-ovmf.sh: error: $*" >&2; exit 1; }
log() { echo "build-ovmf.sh: $*"; }

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
VENDOR_EDK2="$ROOT/vendor/edk2"
BUILD_ROOT="$ROOT/target/edk2-build"
BUILD_EDK2="$BUILD_ROOT/edk2"
OUT_DIR="$ROOT/target/ovmf"
BOOT_CRATE="$ROOT/projects/qrab-boot"
BOOT_EFI="$BOOT_CRATE/target/x86_64-unknown-uefi/release/qrab-boot.efi"

EDK2_STABLE_TAG="${QRAB_EDK2_TAG:-edk2-stable202602}"
EDK2_TOOLCHAIN="${QRAB_EDK2_TOOLCHAIN:-GCC5}"
EDK2_BUILDTYPE="${QRAB_EDK2_BUILDTYPE:-RELEASE}"

# ---- sanity checks -------------------------------------------------------
[[ -d "$VENDOR_EDK2/.git" || -f "$VENDOR_EDK2/.git" ]] \
    || die "vendor/edk2 submodule missing. Run: git submodule update --init vendor/edk2"

(cd "$VENDOR_EDK2" && git describe --tags --exact-match >/dev/null 2>&1) \
    || log "warn: vendor/edk2 is not at an exact tag"

for tool in rsync make gcc python3 iasl nasm; do
    command -v "$tool" >/dev/null 2>&1 || die "required tool '$tool' not on PATH"
done

# ---- build qrab-boot.efi -------------------------------------------------
log "building qrab-boot.efi"
(cd "$BOOT_CRATE" && cargo build --release)
[[ -f "$BOOT_EFI" ]] || die "qrab-boot.efi not produced at $BOOT_EFI"
log "qrab-boot.efi: $(stat -c'%s bytes' "$BOOT_EFI")"

# ---- stage build tree ----------------------------------------------------
#
# We do NOT rsync the vendored submodule: its `.git` is a gitlink that
# points back to the superproject's `.git/modules` tree, and that link
# breaks after a copy (especially inside a worktree checkout). Instead we
# clone the vendored working tree as an independent repository sharing
# the object database. The result is a standalone `git` repo at
# `$BUILD_EDK2` with working submodule support.
mkdir -p "$BUILD_ROOT" "$OUT_DIR"
vendor_head="$(cd "$VENDOR_EDK2" && git rev-parse HEAD)"
staged_head=""
if [[ -d "$BUILD_EDK2/.git" ]]; then
    staged_head="$(cd "$BUILD_EDK2" && git rev-parse HEAD 2>/dev/null || true)"
fi

if [[ "${QRAB_FORCE_RESTAGE:-0}" = 1 || "$staged_head" != "$vendor_head" ]]; then
    log "staging edk2 build tree at $BUILD_EDK2"
    rm -rf "$BUILD_EDK2"
    # --no-local forces a clean clone (not a hardlink farm), --shared keeps
    # the on-disk footprint small by referencing the vendor object DB.
    git clone --shared --no-hardlinks "$VENDOR_EDK2" "$BUILD_EDK2" >/dev/null
    (
        cd "$BUILD_EDK2"
        git checkout --detach "$vendor_head" >/dev/null 2>&1
    )

    # Recursive submodule init against the build copy only. Transitive
    # submodules (openssl, brotli, ...) are fetched into the build copy
    # so the vendored submodule stays shallow on disk.
    log "fetching edk2 transitive submodules into build copy"
    (cd "$BUILD_EDK2" && git submodule update --init --depth 1 --recursive)
else
    log "reusing staged edk2 tree at $BUILD_EDK2 (override with QRAB_FORCE_RESTAGE=1)"
fi

# ---- embed qrab-boot.efi -------------------------------------------------
log "embedding qrab-boot.efi into AmdSev OVMF firmware volume"
python3 "$ROOT/contrib/embed-efi.py" --edk2 "$BUILD_EDK2" --efi "$BOOT_EFI"

# ---- build ---------------------------------------------------------------
cd "$BUILD_EDK2"

export WORKSPACE="$BUILD_EDK2"
export PACKAGES_PATH="$BUILD_EDK2"
# Unset any inherited edk2 env that could leak from a user shell.
unset EDK_TOOLS_PATH EDK_TOOLS_BIN
# edksetup.sh dereferences $PYTHON_COMMAND unconditionally; under our
# `set -u` that crashes if the caller hasn't already exported one.
export PYTHON_COMMAND="${PYTHON_COMMAND:-python3}"

# shellcheck disable=SC1091
# `set -u` conflicts with edksetup.sh's use of unset variables; relax
# while we source it and restore afterwards.
set +u
source edksetup.sh BaseTools >/dev/null
set -u

log "building BaseTools"
# Upstream BaseTools Makefile has a parallel-build race in VfrCompile:
# the C++ sources depend on antlr/dlg-generated headers, but no explicit
# rule forces generation before compilation. On GCC 15 with -j > 1 this
# surfaces as "static DfaState st-1[1]" / "ANTLRTokenType does not name
# a type" errors from stale-or-partial generated headers. Serial build
# (-j1) avoids the race.
make -C BaseTools >"$BUILD_ROOT/basetools.log" 2>&1 \
    || { tail -40 "$BUILD_ROOT/basetools.log" >&2; die "BaseTools build failed; log: $BUILD_ROOT/basetools.log"; }

log "building AmdSev OVMF ($EDK2_BUILDTYPE / $EDK2_TOOLCHAIN)"
build \
    -p OvmfPkg/AmdSev/AmdSevX64.dsc \
    -a X64 \
    -t "$EDK2_TOOLCHAIN" \
    -b "$EDK2_BUILDTYPE" \
    -n "$(nproc)" \
    2>&1 | tee "$BUILD_ROOT/ovmf.log" | grep -E '(- Done -|ERROR|error:|WARNING)' || true

FV="$BUILD_EDK2/Build/AmdSev/${EDK2_BUILDTYPE}_${EDK2_TOOLCHAIN}/FV/OVMF.fd"
[[ -f "$FV" ]] || die "OVMF.fd not produced at $FV (see $BUILD_ROOT/ovmf.log)"
cp "$FV" "$OUT_DIR/OVMF.fd"
log "OVMF.fd staged at $OUT_DIR/OVMF.fd ($(stat -c'%s bytes' "$OUT_DIR/OVMF.fd"))"
log "edk2 tag: $EDK2_STABLE_TAG"
log "done."

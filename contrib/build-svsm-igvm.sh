#!/usr/bin/env bash
# Build the coconut-svsm "IGVM" lane:
#   1. microsoft/igvm C library         (vendor/igvm)
#   2. coconut-svsm/qemu --enable-igvm  (vendor/qemu-svsm)
#   3. coconut-svsm/edk2 OVMF.fd        (vendor/edk2-svsm)
#   4. coconut-svsm/svsm svsm.bin +     (vendor/svsm)
#      igvmbuilder -> coconut-qemu.igvm
#
# Outputs under target/svsm-build/:
#   igvminst/                      (headers + libigvm.so/.pc)
#   qemu-svsm/bin/qemu-system-x86_64
#   edk2-svsm/Build/.../FV/OVMF.fd (copied to out/OVMF-SVSM.fd)
#   svsm/bin/svsm.bin              (copied to out/svsm.bin)
#   svsm/bin/coconut-qemu.igvm     (copied to out/coconut-qemu.igvm)
#
# This script never mutates vendor/*. It clones each submodule into
# $BUILD_ROOT via `git clone --shared --no-hardlinks` and only touches
# the build copy.
#
# Guarded by QRAB_BUILD_SVSM=1 (default off) so the SVSM lane stays
# optional. Set QRAB_SVSM_BUILD_ROOT to override cache location.
#
# Prereqs (Gentoo names; adapt for other distros):
#   dev-lang/rust (nightly), dev-util/cargo-c, sys-devel/clang,
#   dev-util/cmake, dev-util/pkgconf, dev-lang/perl,
#   dev-libs/openssl, ninja, python3, iasl, nasm, rsync, flock
set -euo pipefail

die() { echo "build-svsm-igvm: error: $*" >&2; exit 1; }
log() { echo "build-svsm-igvm: $*"; }

if [[ "${QRAB_BUILD_SVSM:-0}" != "1" ]]; then
    die "guard-bit not set. Export QRAB_BUILD_SVSM=1 to build the SVSM lane."
fi

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
GIT_COMMON_DIR="$(git -C "$ROOT" rev-parse --git-common-dir)"
case "$GIT_COMMON_DIR" in
    /*) : ;;
    *)  GIT_COMMON_DIR="$ROOT/$GIT_COMMON_DIR" ;;
esac
MAIN_CHECKOUT="$(dirname "$GIT_COMMON_DIR")"

BUILD_ROOT="${QRAB_SVSM_BUILD_ROOT:-$MAIN_CHECKOUT/target/svsm-build}"
OUT_DIR="$ROOT/target/svsm-out"
mkdir -p "$BUILD_ROOT" "$OUT_DIR"

# Single lock for the whole SVSM lane. Separate from build-ovmf.sh's
# edk2-build lock so the two lanes can run in parallel.
exec 9>"$BUILD_ROOT/.build.lock"
if ! flock -n 9; then
    die "another build-svsm-igvm.sh is using $BUILD_ROOT"
fi

VENDOR_IGVM="$MAIN_CHECKOUT/vendor/igvm"
VENDOR_QEMU="$MAIN_CHECKOUT/vendor/qemu-svsm"
VENDOR_EDK2="$MAIN_CHECKOUT/vendor/edk2-svsm"
VENDOR_SVSM="$MAIN_CHECKOUT/vendor/svsm"

for d in "$VENDOR_IGVM" "$VENDOR_QEMU" "$VENDOR_EDK2" "$VENDOR_SVSM"; do
    [[ -d "$d/.git" || -f "$d/.git" ]] \
        || die "submodule missing at $d. Run: git submodule update --init --recursive"
done

BUILD_IGVM="$BUILD_ROOT/igvm"
BUILD_QEMU="$BUILD_ROOT/qemu-svsm"
BUILD_EDK2="$BUILD_ROOT/edk2-svsm"
BUILD_SVSM="$BUILD_ROOT/svsm"

IGVM_INST="$BUILD_ROOT/igvminst"
QEMU_INST="$BUILD_ROOT/qemu-svsm-inst"

JOBS="${QRAB_JOBS:-$(nproc)}"

# Stage a vendor submodule into the build root as a standalone working
# clone that shares the object database. Detaches HEAD at the
# submodule's recorded commit so re-runs reproduce exactly.
stage_tree() {
    local name="$1" src="$2" dst="$3"
    local vendor_head staged_head
    vendor_head="$(git -C "$src" rev-parse HEAD)"
    staged_head=""
    if [[ -d "$dst/.git" ]]; then
        staged_head="$(git -C "$dst" rev-parse HEAD 2>/dev/null || true)"
    fi
    if [[ "${QRAB_FORCE_RESTAGE:-0}" = 1 || "$staged_head" != "$vendor_head" ]]; then
        log "staging $name at $dst ($vendor_head)"
        rm -rf "$dst"
        git clone --shared --no-hardlinks "$src" "$dst" >/dev/null
        git -C "$dst" checkout --detach "$vendor_head" >/dev/null 2>&1
    else
        log "reusing staged $name at $dst ($staged_head)"
    fi
}

# ---- 1. microsoft/igvm --------------------------------------------------
log "=== igvm C library ==="
stage_tree igvm "$VENDOR_IGVM" "$BUILD_IGVM"

if ! command -v cargo-cinstall >/dev/null 2>&1; then
    log "installing cargo-c (--locked)"
    cargo install --locked cargo-c >/dev/null
fi

if [[ ! -f "$IGVM_INST/lib/pkgconfig/igvm.pc" && ! -f "$IGVM_INST/lib64/pkgconfig/igvm.pc" ]]; then
    log "building libigvm -> $IGVM_INST"
    (
        cd "$BUILD_IGVM"
        # Upstream Makefile honours PREFIX for install path.
        PREFIX="$IGVM_INST" make -f igvm_c/Makefile install
    )
else
    log "libigvm already installed at $IGVM_INST"
fi

# pkg-config path used by qemu configure below.
IGVM_PKGCFG="$(find "$IGVM_INST" -name igvm.pc -printf '%h\n' | head -1)"
[[ -n "$IGVM_PKGCFG" ]] || die "igvm.pc not found under $IGVM_INST after build"

# ---- 2. coconut-svsm/qemu ------------------------------------------------
log "=== coconut-svsm/qemu (svsm-igvm) ==="
stage_tree qemu-svsm "$VENDOR_QEMU" "$BUILD_QEMU"

if [[ ! -x "$QEMU_INST/bin/qemu-system-x86_64" \
   || "${QRAB_FORCE_REBUILD_QEMU:-0}" = 1 ]]; then
    log "configuring + building qemu (may take ~10 min)"
    (
        cd "$BUILD_QEMU"
        # Transitive qemu submodules (dtc, meson, ...).
        git submodule update --init --depth 1 --recursive 2>/dev/null || true
        PKG_CONFIG_PATH="$IGVM_PKGCFG${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}" \
        ./configure \
            --prefix="$QEMU_INST" \
            --target-list=x86_64-softmmu \
            --enable-igvm \
            --enable-kvm \
            --disable-werror \
            >"$BUILD_ROOT/qemu-configure.log" 2>&1 \
            || { tail -40 "$BUILD_ROOT/qemu-configure.log" >&2;
                 die "qemu configure failed; log: $BUILD_ROOT/qemu-configure.log"; }
        make -j"$JOBS" >"$BUILD_ROOT/qemu-build.log" 2>&1 \
            || { tail -40 "$BUILD_ROOT/qemu-build.log" >&2;
                 die "qemu make failed; log: $BUILD_ROOT/qemu-build.log"; }
        make install >"$BUILD_ROOT/qemu-install.log" 2>&1
    )
else
    log "qemu already built at $QEMU_INST"
fi

QEMU_BIN="$QEMU_INST/bin/qemu-system-x86_64"
[[ -x "$QEMU_BIN" ]] || die "qemu binary missing at $QEMU_BIN"
log "qemu: $("$QEMU_BIN" --version | head -1)"
if ! "$QEMU_BIN" -object help 2>&1 | grep -q '^  igvm-cfg$'; then
    die "qemu built without igvm-cfg object (check --enable-igvm)"
fi
log "qemu advertises igvm-cfg"

# ---- 3. coconut-svsm/edk2 OVMF ------------------------------------------
log "=== coconut-svsm/edk2 (svsm branch) OVMF ==="
stage_tree edk2-svsm "$VENDOR_EDK2" "$BUILD_EDK2"

EDK2_TOOLCHAIN="${QRAB_EDK2_TOOLCHAIN:-GCC5}"
EDK2_BUILDTYPE="${QRAB_EDK2_BUILDTYPE:-RELEASE}"
OVMF_FD="$BUILD_EDK2/Build/AmdSev/${EDK2_BUILDTYPE}_${EDK2_TOOLCHAIN}/FV/OVMF.fd"

if [[ ! -f "$OVMF_FD" || "${QRAB_FORCE_REBUILD_OVMF:-0}" = 1 ]]; then
    (
        cd "$BUILD_EDK2"
        git submodule update --init --depth 1 --recursive 2>/dev/null || true
        export WORKSPACE="$BUILD_EDK2"
        export PACKAGES_PATH="$BUILD_EDK2"
        export PYTHON_COMMAND="${PYTHON_COMMAND:-python3}"
        unset EDK_TOOLS_PATH EDK_TOOLS_BIN
        set +u
        # shellcheck disable=SC1091
        source edksetup.sh BaseTools >/dev/null
        set -u
        log "building BaseTools"
        make -C BaseTools >"$BUILD_ROOT/edk2-basetools.log" 2>&1 \
            || { tail -40 "$BUILD_ROOT/edk2-basetools.log" >&2;
                 die "BaseTools build failed"; }
        log "building OVMF (TPM2 + DEBUG_ON_SERIAL_PORT + PcdUninstallMemAttrProtocol=TRUE)"
        build \
            -p OvmfPkg/AmdSev/AmdSevX64.dsc \
            -a X64 \
            -t "$EDK2_TOOLCHAIN" \
            -b "$EDK2_BUILDTYPE" \
            -n "$JOBS" \
            -D TPM2_ENABLE \
            -D DEBUG_ON_SERIAL_PORT \
            --pcd PcdUninstallMemAttrProtocol=TRUE \
            2>&1 | tee "$BUILD_ROOT/edk2-ovmf.log" \
                 | grep -E '(- Done -|ERROR|error:|WARNING)' || true
    )
else
    log "OVMF.fd already built at $OVMF_FD"
fi
[[ -f "$OVMF_FD" ]] || die "OVMF.fd not produced (see $BUILD_ROOT/edk2-ovmf.log)"
cp "$OVMF_FD" "$OUT_DIR/OVMF-SVSM.fd"
log "OVMF-SVSM.fd: $(stat -c'%s bytes' "$OUT_DIR/OVMF-SVSM.fd")"

# ---- 4. coconut-svsm/svsm + igvmbuilder ---------------------------------
log "=== coconut-svsm/svsm (v2026.03-devel) ==="
stage_tree svsm "$VENDOR_SVSM" "$BUILD_SVSM"

SVSM_BIN="$BUILD_SVSM/bin/svsm.bin"
SVSM_IGVM="$BUILD_SVSM/bin/coconut-qemu.igvm"

if [[ ! -f "$SVSM_IGVM" || "${QRAB_FORCE_REBUILD_SVSM:-0}" = 1 ]]; then
    (
        cd "$BUILD_SVSM"
        git submodule update --init --depth 1 --recursive 2>/dev/null || true
        log "cargo xbuild (recipe in configs/qemu-target.json) ... may take ~5 min"
        FW_FILE="$OVMF_FD" cargo xbuild --release configs/qemu-target.json \
            >"$BUILD_ROOT/svsm-cargo.log" 2>&1 \
            || { tail -60 "$BUILD_ROOT/svsm-cargo.log" >&2;
                 die "svsm xbuild failed; log: $BUILD_ROOT/svsm-cargo.log"; }
    )
else
    log "svsm + IGVM already built at $BUILD_SVSM/bin"
fi
[[ -f "$SVSM_BIN"  ]] || die "svsm.bin not produced"
[[ -f "$SVSM_IGVM" ]] || die "coconut-qemu.igvm not produced"
cp "$SVSM_BIN"  "$OUT_DIR/svsm.bin"
cp "$SVSM_IGVM" "$OUT_DIR/coconut-qemu.igvm"

# ---- 5. verify IGVM magic ----------------------------------------------
# IGVM files start with `IGVM` magic per microsoft/igvm spec.
magic="$(head -c4 "$OUT_DIR/coconut-qemu.igvm" | od -c | head -1 | awk '{print $2$3$4$5}')"
log "coconut-qemu.igvm first 4 bytes: $magic"
log "coconut-qemu.igvm: $(stat -c'%s bytes' "$OUT_DIR/coconut-qemu.igvm")"
log "svsm.bin:         $(stat -c'%s bytes' "$OUT_DIR/svsm.bin")"

log "done. QEMU binary: $QEMU_BIN"
log "Feed IGVM to QEMU: -object igvm-cfg,id=igvm0,file=$OUT_DIR/coconut-qemu.igvm"

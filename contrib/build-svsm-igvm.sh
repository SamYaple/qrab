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

# NASM 3.0+ rejects `push strict dword` in 64-bit mode, which the
# coconut-svsm/edk2 `svsm` branch still uses in
# UefiCpuPkg/Library/CpuExceptionHandlerLib/X64/ExceptionHandlerAsm.nasm.
# Fail loud + early so the operator does not sit through 10 min of
# submodule fetches before hitting the build error. Override with
# QRAB_ALLOW_NASM3=1 when coconut-svsm/edk2 picks up the upstream fix.
if command -v nasm >/dev/null 2>&1; then
    nasm_major="$(nasm --version 2>/dev/null | awk '{print $3}' | cut -d. -f1)"
    if [[ "${nasm_major:-0}" -ge 3 && "${QRAB_ALLOW_NASM3:-0}" != "1" ]]; then
        die "nasm ${nasm_major}.x detected; coconut-svsm/edk2 svsm branch needs nasm 2.x. \
Downgrade (Gentoo: emerge -C1 '=dev-lang/nasm-2*') or set QRAB_ALLOW_NASM3=1 to override."
    fi
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

# Submodules land in the worktree that added them. Prefer the caller's
# worktree, fall back to the main checkout after merge.
pick_vendor() {
    local name="$1"
    if [[ -d "$ROOT/vendor/$name/.git" || -f "$ROOT/vendor/$name/.git" ]]; then
        echo "$ROOT/vendor/$name"
    else
        echo "$MAIN_CHECKOUT/vendor/$name"
    fi
}
VENDOR_IGVM="$(pick_vendor igvm)"
VENDOR_QEMU="$(pick_vendor qemu-svsm)"
VENDOR_EDK2="$(pick_vendor edk2-svsm)"
VENDOR_SVSM="$(pick_vendor svsm)"

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

if ! command -v cbindgen >/dev/null 2>&1; then
    log "installing cbindgen (--locked)"
    cargo install --locked cbindgen >/dev/null \
        || die "cargo install cbindgen failed"
fi

if [[ ! -f "$IGVM_INST/lib/pkgconfig/igvm.pc" && ! -f "$IGVM_INST/lib64/pkgconfig/igvm.pc" ]]; then
    log "building libigvm (make build) then installing -> $IGVM_INST"
    (
        cd "$BUILD_IGVM"
        # `make` default is `all: build test`; `test` needs libcunit which
        # is not a hard dep for qrab — drop to just `build`.
        make -f igvm_c/Makefile build \
            >"$BUILD_ROOT/igvm-build.log" 2>&1 \
            || { tail -40 "$BUILD_ROOT/igvm-build.log" >&2;
                 die "igvm build failed; log: $BUILD_ROOT/igvm-build.log"; }
        PREFIX="$IGVM_INST" make -f igvm_c/Makefile install \
            >"$BUILD_ROOT/igvm-install.log" 2>&1 \
            || { tail -40 "$BUILD_ROOT/igvm-install.log" >&2;
                 die "igvm install failed; log: $BUILD_ROOT/igvm-install.log"; }
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

# Phase 1.5 — Secure Boot wiring. Apply DSC/FDF edits on the staged tree
# only. Vendor tree stays untouched. Idempotent.
SB_BUILD_ARGS=()
SB_SUFFIX=""
SB_KEYS_DIR="$ROOT/contrib/sb-keys"
if [[ "${QRAB_SVSM_SB:-0}" = 1 ]]; then
    patcher_args=("$BUILD_EDK2")
    if [[ "${QRAB_SVSM_SB_BAKE:-auto}" = 1 \
       || ( "${QRAB_SVSM_SB_BAKE:-auto}" = auto \
            && -f "$SB_KEYS_DIR/KEK.crt" && -f "$SB_KEYS_DIR/db.crt" ) ]]; then
        log "Secure Boot: applying wiring + baking qrab certs from $SB_KEYS_DIR"
        patcher_args=(--bake-keys "$SB_KEYS_DIR" "${patcher_args[@]}")
    else
        log "Secure Boot: applying wiring (keys not baked — firmware will ship with upstream MS KEK+CAs)"
    fi
    python3 "$ROOT/contrib/patches/edk2-svsm/apply-sb-wiring.py" "${patcher_args[@]}" \
        || die "apply-sb-wiring.py failed"
    SB_BUILD_ARGS=(-D SECURE_BOOT_ENABLE=TRUE)
    SB_SUFFIX="-SB"
    log "Secure Boot: ENABLED (OVMF will enforce db signatures once keys enrolled via EnrollDefaultKeys.efi)"
else
    log "Secure Boot: disabled (set QRAB_SVSM_SB=1 to enable)"
fi

# qrab-boot.efi must be signed with db.key before embed when SB is on. Under
# SB=off, embed a plain unsigned image (firmware won't verify). Build the
# .efi here so this script is self-contained and matches build-ovmf.sh's
# behaviour.
BOOT_CRATE="$ROOT/projects/qrab-boot"
BOOT_EFI="$BOOT_CRATE/target/x86_64-unknown-uefi/release/qrab-boot.efi"
if [[ -d "$BOOT_CRATE" ]]; then
    if [[ -z "${QRAB_ATTEST_PUBKEY_SEC1:-}" \
       && ! -s "$ROOT/contrib/attest-keys/server.sec1" ]]; then
        log "attest keys missing; regenerating via contrib/gen-attest-keys.sh"
        bash "$ROOT/contrib/gen-attest-keys.sh" >/dev/null 2>&1 \
            || die "gen-attest-keys.sh failed"
    fi
    log "building qrab-boot.efi"
    (cd "$BOOT_CRATE" && cargo build --release)
    [[ -f "$BOOT_EFI" ]] || die "qrab-boot.efi not produced at $BOOT_EFI"
    if [[ -n "$SB_SUFFIX" \
       && -f "$SB_KEYS_DIR/db.key" && -f "$SB_KEYS_DIR/db.crt" ]]; then
        log "signing qrab-boot.efi with contrib/sb-keys/db.key"
        bash "$ROOT/contrib/sign-efi.sh" "$BOOT_EFI" "$BOOT_EFI.signed" \
            >/dev/null || die "sign-efi.sh failed"
        BOOT_EFI="$BOOT_EFI.signed"
    fi
    log "embedding qrab-boot.efi into AmdSev FV"
    python3 "$ROOT/contrib/embed-efi.py" --edk2 "$BUILD_EDK2" --efi "$BOOT_EFI"
else
    log "projects/qrab-boot absent in this worktree; skipping embed"
fi

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
        log "building OVMF (TPM2 + DEBUG_ON_SERIAL_PORT + PcdUninstallMemAttrProtocol=TRUE${SB_SUFFIX:+ + SECURE_BOOT_ENABLE})"
        build \
            -p OvmfPkg/AmdSev/AmdSevX64.dsc \
            -a X64 \
            -t "$EDK2_TOOLCHAIN" \
            -b "$EDK2_BUILDTYPE" \
            "${SB_BUILD_ARGS[@]}" \
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
OVMF_OUT="$OUT_DIR/OVMF-SVSM${SB_SUFFIX}.fd"
cp "$OVMF_FD" "$OVMF_OUT"
log "$(basename "$OVMF_OUT"): $(stat -c'%s bytes' "$OVMF_OUT")"

# Phase 1.5 sanity: SB-enabled build must have compiled the SB driver
# .efi binaries. The .fd FV is a packed / compressed container so
# `strings` finds nothing meaningful inside it — check the edk2 build
# output directory for the expected driver .efis instead.
if [[ -n "$SB_SUFFIX" ]]; then
    efi_dir="$BUILD_EDK2/Build/AmdSev/${EDK2_BUILDTYPE}_${EDK2_TOOLCHAIN}/X64"
    missing=0
    for e in SecureBootConfigDxe.efi EnrollDefaultKeys.efi; do
        if [[ ! -f "$efi_dir/$e" ]]; then
            log "WARN edk2 build did not produce $e — SB wiring may have silently dropped"
            missing=$((missing + 1))
        fi
    done
    [[ $missing -eq 0 ]] \
        && log "SB drivers ($efi_dir/{SecureBootConfigDxe,EnrollDefaultKeys}.efi) produced" \
        || die "SB build is missing $missing of 2 expected SB driver .efi outputs"
fi

# ---- 4. coconut-svsm/svsm + igvmbuilder ---------------------------------
log "=== coconut-svsm/svsm (v2026.03-devel) ==="
stage_tree svsm "$VENDOR_SVSM" "$BUILD_SVSM"

SVSM_ELF="$BUILD_SVSM/bin/svsm"
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
[[ -f "$SVSM_ELF"  ]] || die "svsm ELF not produced at $SVSM_ELF"
[[ -f "$SVSM_IGVM" ]] || die "coconut-qemu.igvm not produced at $SVSM_IGVM"
cp "$SVSM_ELF"  "$OUT_DIR/svsm"
cp "$SVSM_IGVM" "$OUT_DIR/coconut-qemu.igvm"

# ---- 5. verify IGVM magic ----------------------------------------------
# IGVM files start with `IGVM` magic per microsoft/igvm spec.
magic="$(head -c4 "$OUT_DIR/coconut-qemu.igvm" | od -c | head -1 | awk '{print $2$3$4$5}')"
log "coconut-qemu.igvm first 4 bytes: $magic"
log "coconut-qemu.igvm: $(stat -c'%s bytes' "$OUT_DIR/coconut-qemu.igvm")"
log "svsm ELF:         $(stat -c'%s bytes' "$OUT_DIR/svsm")"

log "done. QEMU binary: $QEMU_BIN"
log "Feed IGVM to QEMU: -object igvm-cfg,id=igvm0,file=$OUT_DIR/coconut-qemu.igvm"

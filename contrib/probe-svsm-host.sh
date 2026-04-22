#!/usr/bin/env bash
# Probe host for SVSM readiness. Checks the six things Phase 1 of the
# SVSM-vTPM-integration plan cares about:
#
#   1. Running kernel advertises coconut-svsm (or equivalent) build tag.
#   2. KVM SEV/SEV-ES/SEV-SNP enabled in the AMD KVM driver.
#   3. /dev/sev + /dev/kvm present and readable.
#   4. Running kernel source tree carries the SVSM-aware UAPI ioctl
#      KVM_SEV_SNP_LAUNCH_UPDATE_VMPLS (or, if source tree absent, the
#      host-side SVSM commits detectable via dmesg / modinfo).
#   5. snphost reports PSP + RMP happy (optional — soft-fail if absent).
#   6. target/svsm-build output directory is writable (so the full
#      build-svsm-igvm.sh script can cache artifacts).
#
# Exit 0 if ready, 1 otherwise. Emits `host_kernel_ready: true|false`
# on the last line so higher-level scripts can grep.
set -euo pipefail

die() { echo "probe-svsm-host: error: $*" >&2; exit 1; }
ok()  { printf "  \033[32mOK\033[0m   %s\n" "$*"; }
bad() { printf "  \033[31mFAIL\033[0m %s\n" "$*"; FAIL=1; }
note(){ printf "  \033[33mNOTE\033[0m %s\n" "$*"; }

FAIL=0
ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
KERNEL_SRC="${QRAB_KERNEL_SRC:-$HOME/repos/kernel}"

echo "probe-svsm-host: scanning $(uname -n)"

# --- 1. kernel tag --------------------------------------------------------
KREL="$(uname -r)"
echo "kernel: $KREL"
if [[ "$KREL" == *coconut-svsm* ]]; then
    ok "kernel tag carries 'coconut-svsm'"
else
    bad "kernel tag does not contain 'coconut-svsm' (got: $KREL)"
    note "install via staging/sys-kernel/coconut-svsm-kernel/install.sh and reboot"
fi

# --- 2. KVM AMD module params --------------------------------------------
for p in sev sev_es sev_snp; do
    f=/sys/module/kvm_amd/parameters/$p
    if [[ -r $f && "$(<"$f")" == "Y" ]]; then
        ok "kvm_amd.$p = Y"
    else
        bad "kvm_amd.$p not enabled (check /proc/cmdline + BIOS)"
    fi
done

# --- 3. device nodes -----------------------------------------------------
for d in /dev/sev /dev/kvm; do
    if [[ -r "$d" || -w "$d" ]]; then
        ok "$d accessible"
    else
        bad "$d missing or unreadable"
    fi
done

# --- 4. SVSM-aware UAPI in kernel source ---------------------------------
if [[ -r "$KERNEL_SRC/arch/x86/include/uapi/asm/kvm.h" ]]; then
    if grep -q 'KVM_SEV_SNP_LAUNCH_UPDATE_VMPLS' \
        "$KERNEL_SRC/arch/x86/include/uapi/asm/kvm.h"; then
        ok "UAPI carries KVM_SEV_SNP_LAUNCH_UPDATE_VMPLS"
    else
        bad "kernel source at $KERNEL_SRC lacks SVSM-aware VMPLS ioctl"
    fi
else
    note "kernel source not at $KERNEL_SRC; skipping UAPI grep"
    note "set QRAB_KERNEL_SRC to the tree that built /boot/vmlinuz-$KREL"
fi

# --- 5. snphost ok (optional) --------------------------------------------
if command -v snphost >/dev/null 2>&1; then
    if snphost ok 2>&1 \
        | grep -qE 'RMP table initialized.*PASS|PASS.*RMP table initialized'; then
        ok "snphost ok reports RMP table initialized"
    else
        note "snphost ok did not report RMP PASS — inspect manually"
    fi
else
    note "snphost not installed; install from github.com/virtee/snphost for deeper probe"
fi

# --- 6. build-cache path writable ----------------------------------------
CACHE="${QRAB_SVSM_BUILD_ROOT:-$ROOT/target/svsm-build}"
mkdir -p "$CACHE" 2>/dev/null || bad "cannot create $CACHE"
if [[ -w "$CACHE" ]]; then
    ok "svsm build cache $CACHE writable"
else
    bad "$CACHE not writable"
fi

# --- summary --------------------------------------------------------------
if [[ $FAIL -eq 0 ]]; then
    echo
    echo "host_kernel_ready: true"
    exit 0
else
    echo
    echo "host_kernel_ready: false"
    exit 1
fi

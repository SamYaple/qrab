#!/usr/bin/env bash
# dracut module installer for qrab-boot secret injection.
#
# Runs at `dracut` build time. Declares the module's identity and
# copies `qrab-read-secret.sh` into the initrd so it runs pre-pivot
# and drops the attestation secret into `/run/qrab/secret` before
# userspace starts.
#
# Install path (typical):
#   /usr/lib/dracut/modules.d/70qrab-boot/module-setup.sh
#   /usr/lib/dracut/modules.d/70qrab-boot/qrab-read-secret.sh
#
# Enable via `/etc/dracut.conf.d/qrab-boot.conf`:
#   add_dracutmodules+=" qrab-boot "
#
# Regenerate initrds with `dracut --force` after each qrab-boot
# install. The module is a no-op on hosts without an efivarfs mount.

# shellcheck disable=SC2034  # dracut calls the function variables
check() {
    # Always build the module when included — the hook script
    # self-disables when `/sys/firmware/efi/efivars` is absent.
    return 0
}

depends() {
    # systemd provides the pre-pivot hook runner; efivarfs provides
    # the variable read path. Both are already pulled in on any SNP
    # guest image but declaring them explicitly lets dracut build on
    # hosts that strip either module.
    echo systemd
    return 0
}

install() {
    inst_simple "$moddir/qrab-read-secret.sh" \
        /usr/lib/qrab-boot/qrab-read-secret.sh
    inst_simple "$moddir/qrab-read-public.sh" \
        /usr/lib/qrab-boot/qrab-read-public.sh

    # Run the hooks shortly before the initrd hands off to real-root.
    # `pre-pivot` is the idiomatic dracut place for this — the
    # efivarfs is mounted and userspace is about to take over.
    #
    # Priority 70 runs FIRST: qrab-read-secret.sh fails-closed on
    # missing/zero-length secret (exit 1) so a broken boot halts
    # before the public-var hook burns cycles. Priority 71 runs
    # second: qrab-read-public.sh is best-effort, copies QrabReport +
    # QrabSession as 0644 into /run/qrab/ for qrab-guest-init to
    # read.
    inst_hook pre-pivot 70 "$moddir/qrab-read-secret.sh"
    inst_hook pre-pivot 71 "$moddir/qrab-read-public.sh"

    # Needed at runtime: rm for efivarfs wipe, mktemp for atomic
    # write, cmp for tripwire sanity, dd for the 4-byte attrs-strip
    # on every efivar copy. Everything else (read, printf, wc) is
    # builtin to busybox / dash.
    inst_multiple rm mktemp cmp dd wc
}

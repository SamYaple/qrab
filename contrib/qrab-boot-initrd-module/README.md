# qrab-boot dracut initrd module

Consumer side of the EFI runtime variable written by qrab-boot's
`src/secret.rs`. The hook reads `QrabSecret` from efivarfs, copies
the payload into `/run/qrab/secret` with mode 0600, and then wipes
the firmware variable so it cannot be picked up from userspace
after pivot.

## Layout

```
contrib/qrab-boot-initrd-module/
  module-setup.sh        # dracut build-time descriptor (check/depends/install)
  qrab-read-secret.sh    # pre-pivot hook — reads + wipes the variable
  README.md              # this file
```

## Install

```sh
sudo install -d /usr/lib/dracut/modules.d/70qrab-boot
sudo install -m 0755 module-setup.sh      /usr/lib/dracut/modules.d/70qrab-boot/
sudo install -m 0755 qrab-read-secret.sh  /usr/lib/dracut/modules.d/70qrab-boot/
sudo tee /etc/dracut.conf.d/qrab-boot.conf >/dev/null <<EOF
add_dracutmodules+=" qrab-boot "
EOF
sudo dracut --force
```

## Runtime expectations

On every successful boot of a qrab-boot-signed kernel, the journal
will carry four serial-style tokens:

- `qrab-boot: QRAB_INITRD_OK len=<n>` — secret landed at
  `/run/qrab/secret`, `<n>` bytes.
- `qrab-boot: QRAB_INITRD_WIPED` — firmware variable deleted; a
  post-boot `ls /sys/firmware/efi/efivars | grep QrabSecret` should
  return nothing.

If either token is missing, assume the bootloader never published a
secret this boot — fail-closed in your application rather than use
a stale `/run/qrab/secret` from a previous cycle.

## Constants

The GUID and variable name MUST stay in lockstep with
`projects/qrab-boot/src/secret.rs`:

```
QRAB_NAME = QrabSecret
QRAB_GUID = 6e6f1cba-b5ba-4e54-a3b3-2a7a8f3d9b00
```

Rotating either end requires rebuilding both `qrab-boot.efi` and
this initrd module together and regenerating the guest image with
`dracut --force`.

## Host-side sanity check

`contrib/qrab-boot-initrd-module/tests/simulate.sh` builds a fake
efivarfs layout under a tmpdir, writes a 4-byte attribute prefix +
body, runs the hook against it via `EFIVARFS=$tmp` override, and
asserts `/run/qrab/secret` + wipe both happened. Runs without
privileges; keeps the module honest across edits.

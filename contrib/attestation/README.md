# Guest Attestation Scripts

Scripts for SEV-SNP attestation inside qrab-managed VMs.

## Architecture

When SEV-SNP is enabled, qrab adds a virtio-serial port to the VM:

- **Host side:** Unix socket at `<tmpdir>/attest.sock`
- **Guest side:** `/dev/virtio-ports/org.qrab.attestation`

The BMC server function `send_challenge_to_guest` connects to the host
socket, writes a hex nonce, and reads back attestation evidence.

## attest.sh

Interactive attestation helper. Run inside the guest.

```bash
# Using virtio-serial (auto-detected)
./attest.sh

# Force serial device
./attest.sh --serial /dev/ttyS0

# Include QR code output (requires qrencode)
./attest.sh --qr
```

### Flow

1. Prints SEV-SNP status from sysfs
2. Waits for 64-char hex nonce on the attestation channel
3. Creates TPM2 Attestation Key (EK + AK) if needed
4. Quotes PCRs 0-7 with the nonce via `tpm2_quote`
5. Reads SNP attestation report via `snpguest` if available
6. Outputs base64-encoded evidence (QUOTE_MSG, QUOTE_SIG, QUOTE_PCRS, SNP_REPORT)

### Guest Requirements

- `tpm2-tools` for TPM quotes
- `snpguest` for SNP attestation reports (https://github.com/virtee/snpguest)
- `qrencode` for optional QR output

Install on Debian/Ubuntu:
```bash
apt install tpm2-tools
# snpguest: build from source or use virtee packages
cargo install snpguest
```

## initramfs-hook.sh

Automatic boot-time attestation. Runs early in initramfs, before rootfs mount.

### Install (initramfs-tools)

```bash
cp initramfs-hook.sh /etc/initramfs-tools/scripts/local-bottom/qrab-attest
chmod +x /etc/initramfs-tools/scripts/local-bottom/qrab-attest
update-initramfs -u
```

### Install (dracut)

Create a dracut module or add as a hook:
```bash
mkdir -p /usr/lib/dracut/modules.d/99qrab-attest
cp initramfs-hook.sh /usr/lib/dracut/modules.d/99qrab-attest/module-setup.sh
# Wrap appropriately for dracut module interface
dracut -f
```

### Behavior

- Only runs when `/sys/kernel/security/coco/sev-guest` exists
- Generates random nonce, performs self-attestation
- Outputs to virtio-serial port (or serial fallback)
- Non-blocking: exits silently on non-SEV guests

## Virtio-Serial Channel

The attestation channel uses QEMU virtio-serial:

```
-device virtio-serial-pci,id=virtio-serial-attest
-chardev socket,id=attest-chardev,path=<tmpdir>/attest.sock,server=on,wait=off
-device virtserialport,chardev=attest-chardev,name=org.qrab.attestation
```

Host connects to `attest.sock` (Unix socket). Guest reads/writes
`/dev/virtio-ports/org.qrab.attestation`. This keeps attestation
traffic separate from the console serial port.

# qrab-boot

Rust UEFI bootloader targeting SEV-SNP guests. Replaces GRUB inside the
AmdSev OVMF firmware volume. See the full plan in
`~/workspace/llm-rag/reference/qrab-boot-plan.md`.

## Status

Phase 3b — postcard attestation round-trip over virtio-vsock. The
built firmware at `target/ovmf/OVMF.fd` still boots under SEV-SNP via
the `gGrubFileGuid` FV file (phase 1); on top of that the bootloader
now:

- reads PE Authenticode / RSA-PKCS#1 signature logic from
  `qrab-boot-verify` (phase 2b),
- discovers the virtio-vsock PCI device (phase 3a),
- initialises the virtio transport + `VsockConnectionManager`
  against the MCH-reported PCIe ECAM window (phase 3b.1–3b.5),
- and exchanges a framed `AttestRequest` / `AttestResponse`
  (`qrab-boot-proto`, phase 3b.7–3b.9) with the host-side
  `attest-server` binary.

Still missing from the plan: real randomness on the client nonce,
SNP-aware DMA mapping, UKI fetch, measurement, secret injection.

## Workspace note

This crate targets `x86_64-unknown-uefi`, which is incompatible with the
top-level qrab workspace targeting `x86_64-unknown-linux-gnu`. It is
therefore declared in `exclude` at the workspace root and forms its own
mini-workspace rooted at `projects/qrab-boot/`. All builds must run from
inside this directory (or with `--manifest-path`).

## Build

```sh
# from projects/qrab-boot/
cargo build --release
# -> target/x86_64-unknown-uefi/release/qrab-boot.efi
```

The `rust-toolchain.toml` pins a nightly snapshot and requests the
`x86_64-unknown-uefi` target. `rustup` will install both automatically on
first `cargo` invocation.

## Smoke tests

Run from the qrab repository root (worktree or main checkout).

**Phase 0 (vanilla OVMF, no SEV):**

```sh
contrib/run-qrab-boot-qemu.sh
```

Boots the built EFI under QEMU q35 with distro-provided `OVMF_CODE.fd`
(override via `QRAB_BOOT_OVMF_CODE`). Writes `target/qrab-boot-smoke/serial.log`.

**Phase 1 (AmdSev OVMF, SEV-SNP):**

```sh
contrib/build-ovmf.sh          # ~7 min first time, ~1 min incremental
contrib/smoke-snp-ovmf.sh       # requires /dev/sev + kvm_amd.sev_snp=Y
```

`build-ovmf.sh` clones the pinned edk2 submodule into
`target/edk2-build/edk2/` (never mutating `vendor/edk2`), substitutes
`qrab-boot.efi` for `OvmfPkg/AmdSev/Grub/grub.efi`, disables the upstream
`PREBUILD` hook, and runs the standard edk2 build tool. `smoke-snp-ovmf.sh`
launches the firmware under SEV-SNP and greps serial for the banner.

Expected serial tail in both cases:

```
BdsDxe: starting Boot0002 "Grub Bootloader" from Fv(...)/FvFile(B5AE312C-BC8A-43B1-9C62-EBB826DD5D07)
[ INFO]:  src/main.rs@032: QRAB BOOT v0.1.0 phase0 (build qrab-boot)
[ INFO]:  src/main.rs@033: QRAB_PHASE0_OK
```

The FvFile GUID `B5AE312C-...` is `gGrubFileGuid` — reused unchanged so
BDS does not need patching. The VM hangs forever; this is the intended
fail-closed behaviour for every qrab-boot phase.

## Layout

```
src/
  main.rs     # #[entry] efi_main, banner, halt
  panic.rs    # fail-closed panic handler
Cargo.toml
rust-toolchain.toml
.cargo/config.toml
```

Future phases add `net.rs`, `attest.rs`, `ghcb.rs`, `uki.rs`,
`measure.rs`, `secret.rs`, `config.rs`, `error.rs`.

# qrab-boot

Rust UEFI bootloader targeting SEV-SNP guests. Replaces GRUB inside the
AmdSev OVMF firmware volume. See the full plan in
`~/workspace/llm-rag/reference/qrab-boot-plan.md`.

## Status

Phase 0 — skeleton. Prints a banner and halts. No attestation, no
networking, no UKI load yet. Subsequent phases land behind their own
`agent/qrab-boot-phase<N>` worktrees.

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

## Smoke test (Phase 0)

From the qrab repository root (worktree or main checkout):

```sh
contrib/run-qrab-boot-qemu.sh
```

The script boots the built EFI under QEMU q35 with vanilla OVMF (non-SEV)
and streams the QEMU `debugcon` (port `0x402`) and serial to stdout. Look
for:

```
QRAB BOOT v0.1.0 phase0 (build qrab-boot)
QRAB_PHASE0_OK
```

The VM will hang forever after printing — this is the intended fail-closed
behaviour for every qrab-boot phase. Kill QEMU with `Ctrl-A X` (or
whatever monitor channel you have open).

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

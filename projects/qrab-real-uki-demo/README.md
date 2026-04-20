# qrab-real-uki-demo

Smallest legal PE that `qrab-boot`'s `BootServices::load_image` +
`start_image` path can hand off to — the real-UKI fixture for
`contrib/matrix-smoke.sh` scenario 1r (follow-up **F10**).

## Why not a full Linux + initrd UKI?

The vsock wire framing caps a single payload at
`qrab_boot_proto::MAX_FRAMED_PAYLOAD = 128 KiB` (see
`projects/qrab-boot-proto/src/lib.rs`). A real Linux kernel image is
~12–15 MiB and will not fit. Scenario 1r's purpose is to prove that
qrab-boot's terminal `LoadImage` / `StartImage` chain actually
transfers control to a fresh PE — not to boot a distro. A minimum
viable PE that writes a distinctive marker token to the QEMU debugcon
port and halts is enough to close that gap, and well under the frame
budget.

## Build

```sh
cd projects/qrab-real-uki-demo
cargo build --release
# -> target/x86_64-unknown-uefi/release/qrab-real-uki-demo.efi
```

`contrib/build-test-uki.sh` wraps this and stages the resulting PE at
`projects/qrab-boot-tests/fixtures/uki-real.efi`. `matrix-smoke.sh`
invokes the wrapper on demand so a fresh checkout produces the
fixture without manual intervention.

## Runtime contract

On execution the image writes `QRAB_REAL_UKI_BOOT_OK version=<pkg>\n`
one byte at a time to I/O port `0x402` (QEMU debugcon — the same port
`qrab-boot` routes `log-debugcon` output through, so the marker lands
in `serial.log` alongside the rest of the smoke's tokens) and then
halts. No UEFI services are used, no allocator is installed, and no
external crate is linked other than `uefi` for the `#[entry]`
proc-macro and the `Status` return type.

## Why excluded from the top-level workspace

`x86_64-unknown-uefi` is incompatible with the host-side crates under
`projects/` (same reason `qrab-boot` is excluded — see the comment on
`[workspace.exclude]` in the root `Cargo.toml`). This crate is built
out-of-band by `contrib/build-test-uki.sh` only.

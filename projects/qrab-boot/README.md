# qrab-boot (legacy)

Rust UEFI bootloader for the confidential-guest experiments. It is separate
from the current pod-native manager; see [legacy tooling](../../docs/legacy.md)
and the [active runtime](../../docs/manager.md).

The source contains the boot, measurement, attestation, transport and handoff
implementations. Historical phase plans and audit references are provenance;
this README does not claim that a current firmware or hardware stack has been
validated. The former phase-zero smoke script has been retired.

This crate targets `x86_64-unknown-uefi` and has its own workspace/toolchain.
Build it from this directory:

```sh
cargo build --release
# target/x86_64-unknown-uefi/release/qrab-boot.efi
```

`rust-toolchain.toml` specifies the UEFI toolchain. Firmware integration is in
`contrib/build-ovmf.sh` and `contrib/build-svsm-igvm.sh`; those specialized builds
need their own vendor sources and firmware toolchains. Runtime validation also
requires suitable SEV/SVSM hardware. Read the source and prerequisites before use. They are not required for a normal VM pod.

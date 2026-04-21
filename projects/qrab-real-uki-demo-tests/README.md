# qrab-real-uki-demo-tests

Host-side invariant tests for the sibling `qrab-real-uki-demo` UEFI PE
fixture. Closes [B13-6](../../../llm-rag/audits/bugs.md#b13-6) — prior
to this crate the byte-size budget and PE shape of that fixture were
only enforced by a shell assertion in `contrib/build-test-uki.sh`.

## Why a separate crate?

`qrab-real-uki-demo` targets `x86_64-unknown-uefi` and is excluded from
the top-level workspace (see `[workspace.exclude]` in the root
`Cargo.toml`). Host-side Rust tests cannot live inside that crate, and
adding it back to the workspace would break workspace-wide
`cargo build` by mixing incompatible target triples. This sibling
crate is host-target (`x86_64-unknown-linux-gnu`), workspace-included,
and dependency-free — `cargo test -p qrab-real-uki-demo-tests` covers
the invariants without touching cross-target linkage.

## What is checked

See `tests/pe_invariants.rs` for the authoritative list. In summary:

| Invariant | Symptom if it regresses |
|---|---|
| Size ≤ 128 KiB (`MAX_FRAMED_PAYLOAD`) | Fixture no longer fits in one vsock framed payload; smoke-test scenario 1r fails late |
| `MZ` + `PE\0\0` + COFF `Machine = 0x8664` | Link regression emits a non-PE or non-x86_64 blob; `BootServices::load_image` rejects at runtime |
| Contains `QRAB_REAL_UKI_BOOT_OK` bytes | `strip = "symbols"` or LTO removes the marker; smoke `grep` never matches |

A self-test module also covers the skip/helper machinery so a
path-resolution regression surfaces even when no fixture is built.

## Fixture discovery

Checked in order:

1. `$QRAB_REAL_UKI_FIXTURE` — explicit path override.
2. `projects/qrab-boot-tests/fixtures/uki-real.efi` — staged by
   `contrib/build-test-uki.sh`.
3. `projects/qrab-real-uki-demo/target/x86_64-unknown-uefi/release/qrab-real-uki-demo.efi`
   — raw `cargo build --release` output of the sibling crate.

If none is present, every fixture-dependent test **skips** (prints a
message and returns `Ok`). This keeps a clean-checkout
`cargo test --workspace` green even when the UEFI target toolchain is
not installed. Set `QRAB_REAL_UKI_TESTS_REQUIRE=1` to turn the skip
into a hard panic — CI jobs that build the fixture before running
tests should opt into this.

## Running

```sh
# Plain run. Skips if the fixture is not present.
cargo test -p qrab-real-uki-demo-tests

# Build the fixture first, then require it:
contrib/build-test-uki.sh
QRAB_REAL_UKI_TESTS_REQUIRE=1 cargo test -p qrab-real-uki-demo-tests
```

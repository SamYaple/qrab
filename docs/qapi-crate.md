# The qapi Crate

The primary consumer-facing crate. Contains generated types for all QEMU QMP
and QGA commands, events, and data structures, plus the runtime traits and
version management that make them work.

## Files

| File | Lines | Content |
|---|---|---|
| `src/lib.rs` | ~67 | Traits, version management, re-exports |
| `src/generated.rs` | ~22,294 | QMP types (machine-generated, do not edit) |
| `src/qga.rs` | ~1,255 | QGA types (machine-generated, do not edit) |
| `tests/serialization.rs` | ~220 | Serde round-trip tests |
| `build.rs` | ~4 | Declares rerun-if-env-changed for version env vars |

## Traits

### `QapiCommand`

Implemented by every command struct. Provides:

| Associated Item | Type | Purpose |
|---|---|---|
| `Returns` | type | Deserializable return type |
| `NAME` | `&str` | QMP wire name |
| `ALLOW_OOB` | `bool` | Out-of-band execution support |
| `ALLOW_PRECONFIG` | `bool` | Safe during preconfig state |
| `SUCCESS_RESPONSE` | `bool` | Whether QEMU sends a success response |
| `UNSTABLE` | `bool` | Marked unstable in schema |
| `AVAILABLE_SINCE` | `(u32, u32, u32)` | First QEMU version with this command |
| `REMOVED_IN` | `Option<(u32, u32, u32)>` | QEMU version that removed it, if any |
| `PRECONFIG_SINCE` | `Option<(u32, u32, u32)>` | When allow-preconfig was added |

### `QapiEvent`

Implemented by event structs. Provides `NAME`.

### `PreconfigSafe`

Marker trait for commands safe during QEMU's preconfig state. Generated for
commands with `#[qapi(allow_preconfig)]`. Used by `manager::PreconfigVm` for
compile-time enforcement.

## Version Gating

### Runtime

```rust
qapi::set_qemu_version(9, 1, 0);  // set once per process (OnceLock)
let v = qapi::qemu_version();     // returns &(u32, u32, u32)
```

Fields and variants annotated with `since` / `until` are skipped during
serialize/deserialize if the runtime version is outside their range. This
means you can use the same compiled binary against multiple QEMU versions.

### Compile-Time

Set `QEMU_MIN_VERSION` and/or `QEMU_MAX_VERSION` environment variables to
prune types, fields, and variants that fall entirely outside the window.
This reduces binary size when targeting a known version range.

### QGA

Same pattern: `set_qga_version()` / `qga_version()`. QGA types live in the
`qga` submodule, re-exported at the crate root as `qapi::qga::*`.

## Generated Code Structure

Types in `generated.rs` are alphabetically sorted. Every type is annotated with
`#[qapi(...)]` attributes that the `qapi-macros` proc macro processes at
compile time. Common patterns:

- `#[qapi(name = "wire-name")]` — serde rename for wire format
- `#[qapi(since = "X.Y.Z")]` / `#[qapi(until = "X.Y.Z")]` — version lifecycle
- `#[qapi(command, returns = "ReturnType")]` — command trait generation
- `#[qapi(event)]` — event trait generation
- `#[qapi(discriminator)]` / `#[qapi(union)]` — flat union encoding
- `#[qapi(condition = "CONFIG_LINUX")]` — conditional compilation


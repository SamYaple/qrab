# The #[qapi] Macro (qapi-macros)

A proc-macro crate that expands `#[qapi(...)]` attributes on structs and enums
into Serialize/Deserialize implementations and trait impls with version-aware
behavior.

## Location

Single-file crate: `projects/qapi-macros/src/lib.rs` (~1343 lines).

## Type-Level Attributes

| Attribute | Type | Effect |
|---|---|---|
| `name = "wire-name"` | string | Sets the QMP wire name for serde |
| `since = "X.Y.Z"` | version | Runtime version gating (lower bound) |
| `until = "X.Y.Z"` | version | Runtime version gating (upper bound) |
| `deprecated` | flag | Marks type as deprecated |
| `unstable` | flag | Marks as unstable, generates `#[deprecated]` warning |
| `condition = "CONFIG_*"` | string | Maps to `#[cfg]`; CONFIG_LINUX/WIN32/POSIX are real, others are no-op |
| `command` | flag | Generates `QapiCommand` trait impl |
| `event` | flag | Generates `QapiEvent` trait impl |
| `returns = "Type"` | string | `QapiCommand::Returns` associated type |
| `allow_oob` | flag | `QapiCommand::ALLOW_OOB = true` |
| `allow_preconfig` | flag | `QapiCommand::ALLOW_PRECONFIG = true` + `PreconfigSafe` impl |
| `no_success_response` | flag | `QapiCommand::SUCCESS_RESPONSE = false` |
| `allow_preconfig_since` | version | When allow-preconfig was added |
| `allow_preconfig_until` | version | When allow-preconfig was removed |

## Field/Variant-Level Attributes

| Attribute | Type | Effect |
|---|---|---|
| `name = "wire-name"` | string | Serde field rename |
| `since = "X.Y.Z"` | version | Skip field if runtime version is below |
| `until = "X.Y.Z"` | version | Skip field if runtime version is at or above |
| `deprecated` | flag | Field deprecated |
| `condition = "CONFIG_*"` | string | Conditional compilation |
| `feature = "deprecated"` | string | Generates `#[deprecated]` warning on use |
| `feature = "unstable"` | string | Generates `#[deprecated]` warning on use |
| `flatten` | flag | Inline fields into parent via `serialize_map` |
| `discriminator` | flag | Marks the discriminator field of a flat union |
| `union` | flag | Marks the union payload field |

## What Gets Generated

For a `#[qapi(command)]` struct, the macro generates:

- `impl Serialize` — version-gated field serialization
- `impl Deserialize` — version-gated field deserialization
- `impl QapiCommand` — all associated constants (NAME, Returns, version info)
- `impl PreconfigSafe` — if `allow_preconfig` is set

For a `#[qapi(event)]` struct: `Serialize`, `Deserialize`, `QapiEvent`.

For plain types: `Serialize`, `Deserialize` only.

## Version Gating in Serde

During serialization/deserialization, fields with `since`/`until` check the
global version set by `qapi::set_qemu_version()`. Fields outside the active
version range are:

- **Serialization:** skipped (not sent on the wire)
- **Deserialization:** treated as optional / default

## Compile-Time Pruning

The macro reads `QEMU_MIN_VERSION` and `QEMU_MAX_VERSION` environment variables
at compile time. Types and fields entirely outside this window are excluded
from compilation. Key functions: `get_version_bounds()`,
`excluded_by_bounds()`.

## Internal Types

| Struct | Purpose |
|---|---|
| `TypeAttrs` | Parsed type-level `#[qapi(...)]` attributes |
| `ItemAttrs` | Parsed field/variant-level attributes |
| `AttrRange` | Version-ranged attribute with optional added/removed bounds |
| `VersionBounds` | Compile-time bounds from env vars |

# qapi

Generated Rust types for QEMU's [QAPI](https://www.qemu.org/docs/master/devel/qapi-code-gen.html) schema.

`generated.rs` and `qga.rs` are produced by `qapi-codegen` and should not be edited by hand. `lib.rs` defines the runtime traits and version management used by the generated code and by `qapi-protocol`.

## Modules

- **`generated.rs`** — QMP types (structs, enums, commands, events)
- **`qga.rs`** — QGA (Guest Agent) types

## Runtime traits

- `QapiCommand` — implemented by command types; carries the wire name, return type, and capability flags
- `QapiEvent` — implemented by event types; carries the wire name

Version gating is controlled by `set_qemu_version()`, called once after connecting to a QEMU instance.

## Regenerating

```sh
cargo run -p qapi-codegen -- vendor/qemu > projects/qapi/src/generated.rs
cargo run -p qapi-codegen -- vendor/qemu --qga > projects/qapi/src/qga.rs
```

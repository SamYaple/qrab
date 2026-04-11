# qapi-macros

Proc-macro crate. Expands `#[qapi(...)]` attributes on generated types into
version-aware Serialize/Deserialize impls and trait impls.

See [../../docs/macro.md](../../docs/macro.md) for attribute reference and internals.

Single-file crate: `src/lib.rs` (~1350 lines).

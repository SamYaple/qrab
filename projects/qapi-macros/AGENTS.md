# qapi-macros

Proc-macro crate. Expands `#[qapi(...)]` attributes on generated types into
version-aware Serialize/Deserialize impls and trait impls.

See [../../docs/codegen.md](../../docs/codegen.md) for the generation pipeline
and `src/lib.rs` for the authoritative attribute parser and expansion.

Single-file implementation: `src/lib.rs`.

# qapi

Primary consumer-facing crate. Contains generated QMP/QGA types and runtime
traits (`QapiCommand`, `QapiEvent`, `PreconfigSafe`).

See [../../docs/qapi-crate.md](../../docs/qapi-crate.md) for type details and version gating.

**Key rule:** `generated.rs` and `qga.rs` are machine-generated outputs.
Edit `qapi-codegen` to change them, never edit directly.

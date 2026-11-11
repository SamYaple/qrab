# qapi

Primary consumer-facing crate. Contains generated QMP/QGA types and runtime
traits (`QapiCommand`, `QapiEvent`, `PreconfigSafe`).

See [../../docs/codegen.md](../../docs/codegen.md) for generation and
[../../docs/protocol.md](../../docs/protocol.md) for version contexts.

**Key rule:** `src/generated/` and `src/qga/` are machine-generated outputs.
Edit `qapi-codegen` to change them, never edit directly.

# manager

High-level typestate VM lifecycle management built on `qapi-protocol`.

See [../../docs/manager.md](../../docs/manager.md) for the typestate pattern, VmBuilder, and internals.

States: `VmBuilder` -> `PreconfigVm` (compile-time `PreconfigSafe` enforcement) -> `Vm` (full QMP access).

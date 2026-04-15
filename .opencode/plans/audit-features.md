# Feature Gaps Audit Report

Generated: 2026-04-13 | Round 6

## Changes This Round

| # | Gap | Change |
|---|-----|--------|
| N45 | Graceful shutdown | **CLOSED** — ACPI→quit→SIGKILL chain implemented |
| N13 | Graceful Qmp shutdown | ADDRESSED — `shutdown()` method exists |

## NEW Gaps (Round 6)

| # | Gap | Priority |
|---|-----|----------|
| N49 | OnceLock blocks power-cycle version tracking | should-have |
| N50 | No VmBuilder::memory() (buried in MachineConfig) | nice-to-have |
| N51 | Runtime version guard missing (AVAILABLE_SINCE not checked before send) | should-have |
| N52 | No event dispatch enum (80+ event structs, no match-based dispatch) | should-have |
| N53 | build.rs reads QEMU_MIN/MAX_VERSION but emits zero cfg flags — compile-time gating INERT | **must-have** |

## Priority Matrix

| Priority | Items |
|----------|-------|
| **Must-have** | Typed errors (#2, N9, N14), compile-time gating fix (N53), typed event dispatch (N52) |
| **Should-have** | OOB (N6), per-connection version (N4/N49), SMP/accel builders (N23/N24), runtime version guard (N51), QmpBuilder (N20), Send/Sync (N8), syn 2 (N16) |
| **Nice-to-have** | Feature flags (#7), rustdoc (#8), memory builder (N50), must_use (N17), health check (N26) |

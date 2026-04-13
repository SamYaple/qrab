# Feature Gaps Audit Report

Generated: 2026-04-13 | Round 4

## Prior Gaps Status

| # | Gap | Status |
|---|-----|--------|
| 1 | Compile-time version gating | ADDRESSED |
| 2 | No typed errors | STILL MISSING |
| 3 | Typed event deserialization | STILL PARTIAL |
| 4 | QGA client | ADDRESSED |
| 5 | QMP client gaps (OOB, reconnect, TCP) | MOSTLY MISSING (timeout partial) |
| 6 | Manager gaps (disk/net/KVM/stderr) | STILL PARTIAL |
| 7 | Feature flags | PARTIAL (bmc has features, core crates don't) |
| 8 | Documentation gaps | STILL MISSING |
| 9 | syn version mismatch | STILL PRESENT |
| N3 | No Drop on Qmp | **ADDRESSED** — Drop impl exists |
| N4 | Global version lock | STILL PRESENT |
| N5 | Missing builder methods | STILL PARTIAL |
| N6 | No capability negotiation | STILL MISSING |
| N7 | Event channel bounds | Bounded at 100, not configurable |
| N8 | No Send/Sync assertions | STILL MISSING |
| N9 | No QGA error type | STILL MISSING |
| N10 | QGA not async-safe | STILL MISSING |
| N11 | No typed event dispatch | STILL MISSING |
| N12 | Mock QGA server helper | STILL MISSING |

## NEW Gaps (Round 4)

| # | Gap | Priority |
|---|-----|----------|
| N13 | No graceful Qmp shutdown (Drop aborts, no async close) | should-have |
| N14 | No structured QMP error parsing (raw Value) | must-have |
| N15 | VmBuilder memory only as string, no validation | nice-to-have |
| N16 | syn 1 blocks MSRV progress | should-have |
| N17 | No `#[must_use]` on builder methods | nice-to-have |
| N18 | `detect_version` is blocking sync in async crate | should-have |

## Priority Matrix

| Priority | Items |
|----------|-------|
| **Must-have** | Typed errors (#2, N9, N14), typed event dispatch (N11), disk/net/smp builders (N5) |
| **Should-have** | OOB (N6), per-connection version (N4), Send/Sync (N8), graceful shutdown (N13), syn 2 (N16), async detect_version (N18), QGA concurrency (N10) |
| **Nice-to-have** | Feature flags (#7), rustdoc (#8), memory validation (N15), must_use (N17), mock QGA (N12) |

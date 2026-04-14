# Feature Gaps Audit Report

Generated: 2026-04-13 | Round 5

## Prior Gaps

| # | Gap | Status |
|---|-----|--------|
| 1 | Compile-time version gating | ADDRESSED |
| 2 | No typed errors | STILL MISSING |
| 3 | Typed event deserialization | PARTIAL (trait exists, test helper exists, not public) |
| 4 | QGA client | ADDRESSED |
| 5 | QMP client gaps (OOB, reconnect, TCP) | STILL MISSING |
| 6 | Manager gaps | IMPROVED (stderr now forwarded to tracing) |
| 7 | Feature flags | STILL PARTIAL |
| 8 | Documentation gaps | STILL MISSING |
| 9 | syn version mismatch | STILL PRESENT |
| N3 | No Drop on Qmp | ADDRESSED |
| N4 | Global version lock | STILL PRESENT |
| N5 | Missing builder methods | STILL PARTIAL |
| N6 | No capability negotiation | STILL MISSING |
| N8 | No Send/Sync assertions | STILL MISSING |
| N9 | No QGA error type | STILL MISSING |
| N10 | QGA not async-safe | STILL MISSING |
| N11 | No typed event dispatch | STILL MISSING (test-only) |
| N12 | Mock QGA server | PARTIALLY ADDRESSED |
| N13 | No graceful Qmp shutdown | **ADDRESSED** — `shutdown()` method exists |
| N14 | No structured QMP error parsing | STILL MISSING |
| N16 | syn 1 blocks MSRV | STILL PRESENT |
| N17 | No `#[must_use]` on builders | STILL MISSING |
| N18 | detect_version blocking sync | STILL PRESENT |

## NEW Gaps (Round 5)

| # | Gap | Priority |
|---|-----|----------|
| N19 | `wait_for_typed_event` not public API | should-have |
| N20 | No QmpBuilder/connection options (timeout, channel cap) | should-have |
| N21 | `set_qemu_version` silently drops second call (same as BUG 4) | must-have |
| N22 | No Qga Drop/shutdown | nice-to-have |
| N23 | MachineConfig missing KVM/TCG accel choice | should-have |
| N24 | No SMP topology builder | should-have |
| N25 | Qmp/Qga execute duplicate logic | nice-to-have |
| N26 | No connection health check | nice-to-have |

## Priority Matrix

| Priority | Items |
|----------|-------|
| **Must-have** | Typed errors (#2, N9, N14, N21), typed event dispatch (N11/N19), SMP/accel builders (N23, N24) |
| **Should-have** | OOB (N6), per-connection version (N4), QmpBuilder (N20), Send/Sync (N8), syn 2 (N16), async detect_version (N18), QGA concurrency (N10) |
| **Nice-to-have** | Feature flags (#7), rustdoc (#8), must_use (N17), Qga shutdown (N22), dedup (N25), health check (N26) |

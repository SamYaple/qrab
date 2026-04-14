# Feature Gaps Audit Report

Generated: 2026-04-13 | Round 10

## 9 gaps CLOSED this round
- N38 execute_raw — ADDRESSED (QMP + QGA)
- N39 QmpError struct — ADDRESSED
- N40 execute_with_timeout — ADDRESSED
- N17/N2 `#[must_use]` — ADDRESSED (24 annotations)
- N23 SMP builder — ADDRESSED
- N24 Accel builder — ADDRESSED
- N34 is_available — ADDRESSED
- N30 wait_shutdown/exit_watch — ADDRESSED
- N15 Memory validation — ADDRESSED

## NEW Gaps

| # | Gap | Priority |
|---|-----|----------|
| N42 | No `TryFrom<QmpEvent>` for typed events | should-have |
| N43 | execute_raw API inconsistency QMP vs QGA | nice-to-have |
| N44 | No `Qmp::subscribe<E: QapiEvent>()` filtered channel | nice-to-have |

## Priority Matrix

| Priority | Items |
|----------|-------|
| **Must-have** | Typed error enums per-command (#2), compile-time version gating (N4/N21) |
| **Should-have** | OOB (N6), TCP (N28), typed event dispatch (N11/N19/N42), per-command timeout defaults (N29), timeout disambiguation (N41) |
| **Nice-to-have** | ErrorClass dedup (N37), feature flags (#7), rustdoc (#8), health check (N26), execute_raw alignment (N43), subscribe helper (N44) |
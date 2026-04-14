# Feature Gaps Audit Report

Generated: 2026-04-13 | Round 8

## Changes This Round
- N35 (execute_raw) → CLOSED for QGA, still open for QMP
- N36 (Vm::events accessor) → CLOSED (pub field access works)

## NEW Gaps

| # | Gap | Priority |
|---|-----|----------|
| N38 | `Qmp::execute_raw` missing — QGA has it, QMP doesn't | should-have |
| N39 | QMP error not parsed into QapiErrorClass — blocks typed errors | must-have |
| N40 | No `execute_with_timeout` — 30s hardcoded, no override | should-have |
| N41 | Timeout vs disconnect indistinguishable (acknowledged in code) | should-have |

## Priority Matrix

| Priority | Items |
|----------|-------|
| **Must-have** | Typed errors (#2, N9, N39), per-command timeout (N29), version lock (N4/N21) |
| **Should-have** | execute_raw (N38), execute_with_timeout (N40), timeout disambiguation (N41), typed events public (N11/N19), OOB (N6), TCP (N28), SMP/accel (N23/N24), is_available (N34), wait_shutdown (N30) |
| **Nice-to-have** | ErrorClass dedup (N37), feature flags (#7), rustdoc (#8), must_use (N17), health check (N26) |

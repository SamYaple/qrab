# Feature Gaps Audit Report

Generated: 2026-04-13 | Round 7

## Changes This Round
- N31 (socket collision) → DEMOTED to nice-to-have (pid+id sufficient)
- N14 (structured QMP error) → PARTIAL (QapiErrorClass enum exists in generated, not wired to protocol)

## Still Missing (key must-have items)
- #2 — No typed errors (anyhow everywhere, 42 occurrences)
- N4 — Global version lock (OnceLock)
- N11/N19 — Typed event dispatch test-only
- N29 — Per-command timeout hardcoded 30s

## NEW Gaps (Round 7)

| # | Gap | Priority |
|---|-----|----------|
| N35 | No `Qmp::execute_raw` — can't send arbitrary JSON | should-have |
| N36 | No `Vm::events()` accessor — can't subscribe post-construction | should-have |
| N37 | Duplicate ErrorClass vs QapiErrorClass in generated types | nice-to-have |

## Priority Matrix

| Priority | Items |
|----------|-------|
| **Must-have** | Typed errors (#2, N9, N14), per-command timeout (N29), version lock (N4/N21) |
| **Should-have** | Typed events public (N11/N19), OOB (N6), QmpBuilder (N20), TCP (N28), execute_raw (N35), events accessor (N36), SMP/accel (N23/N24), is_available (N34), wait_shutdown (N30) |
| **Nice-to-have** | Feature flags (#7), rustdoc (#8), ErrorClass dedup (N37), socket collision (N31), must_use (N17), dedup (N25), health check (N26) |

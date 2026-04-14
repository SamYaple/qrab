# Feature Gaps Audit Report

Generated: 2026-04-13 | Round 6

## Status Changes This Round
- N13 (graceful Qmp shutdown) — CONFIRMED ADDRESSED
- N27 (QGA sync timeout) — FIXED (10s timeout wraps handshake)
- #6 (manager gaps) — FURTHER IMPROVED (stderr→tracing, socket perms, firmware validation)

## Still Missing (key items)
- #2 — No typed errors (anyhow everywhere)
- #5 — No OOB, no reconnect, no TCP
- N4 — Global version lock (OnceLock)
- N11/N19 — Typed event dispatch test-only, not public
- N14 — No structured QMP error parsing

## NEW Gaps (Round 6)

| # | Gap | Priority |
|---|-----|----------|
| N27 | No backpressure config on Qmp channels | should-have |
| N28 | No TCP/IP transport (Unix-only) | should-have |
| N29 | No per-command timeout (30s hardcoded in write_loop) | must-have |
| N30 | No `Vm::wait_shutdown` helper | should-have |
| N31 | Socket path collision risk (predictable, no randomness) | must-have |
| N32 | No `PreconfigVm::execute_raw` for paused-mode VMs | nice-to-have |
| N33 | QgaSession mock missing close() | nice-to-have |
| N34 | No `QapiCommand::is_available` runtime helper | should-have |

## Priority Matrix

| Priority | Items |
|----------|-------|
| **Must-have** | Typed errors (#2, N9, N14), per-command timeout (N29), socket security (N31), version lock (N21) |
| **Should-have** | Typed events public (N11/N19), OOB (N6), per-connection version (N4), QmpBuilder (N20), TCP (N28), backpressure (N27), SMP/accel (N23/N24), is_available (N34), wait_shutdown (N30) |
| **Nice-to-have** | Feature flags (#7), rustdoc (#8), must_use (N17), dedup (N25), health check (N26), raw preconfig (N32), mock close (N33) |

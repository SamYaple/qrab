# Feature Gaps Audit Report

Generated: 2026-04-13 | Round 11

## New Functionality Landed (not closing prior gaps)
- Namespace isolation (user/PID/net/mount)
- Graceful shutdown escalation (ACPI→quit→SIGKILL)
- CPU model builder
- qrab-init crate

## 0 prior gaps closed this round.

## NEW Gaps

| # | Gap | Priority |
|---|-----|----------|
| N45 | PreconfigVm::graceful_shutdown is stub (ignores timeout) | should-have |
| N46 | No VmBuilder::disk() / VmBuilder::net() | should-have |
| N47 | No QMP reconnect on socket drop | nice-to-have |
| N48 | Namespace isolation not optional (breaks macOS/non-root) | nice-to-have |

## Priority Matrix

| Priority | Items |
|----------|-------|
| **Must-have** | Typed error enums (#2), compile-time version gating (N4/N21) |
| **Should-have** | OOB (N6), TCP (N28), typed events (N11/N19/N42), per-command timeout defaults (N29), timeout disambiguation (N41), preconfig graceful_shutdown (N45), disk/net builders (N46) |
| **Nice-to-have** | ErrorClass dedup (N37), feature flags (#7), rustdoc (#8), health check (N26), reconnect (N47), optional namespaces (N48), subscribe helper (N44) |

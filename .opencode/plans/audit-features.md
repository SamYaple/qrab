# Feature Gaps Audit Report

Generated: 2026-04-13 | Re-audited: 2026-04-13 (x3)

## Prior Gaps Status

| # | Gap | Status |
|---|-----|--------|
| 1 | Compile-time version gating | **ADDRESSED** — macro reads env vars, excludes types outside range |
| 2 | No typed errors — anyhow everywhere | STILL MISSING |
| 3 | Missing typed event deserialization | PARTIAL — QapiEvent trait exists, but protocol still uses raw Value |
| 4 | QGA client gaps | **ADDRESSED** — full client with sync handshake, typed execute, version detection |
| 5 | QMP client gaps (timeout, OOB, reconnect, TCP) | STILL MISSING |
| 6 | Manager gaps (disk/net/KVM/stderr) | PARTIAL — memory config exists, rest missing |
| 7 | No feature flags | STILL MISSING |
| 8 | Documentation gaps | STILL MISSING |
| 9 | syn version mismatch | STILL PRESENT |

| # | New Gap (prior round) | Status |
|---|---|---|
| N1 | QGA client nonexistent | **REMOVED** — client confirmed working |
| N2 | Zero tests in protocol+manager | **REMOVED** — tests exist (21 protocol, 10 manager) |
| N3 | No Drop on Qmp | STILL MISSING |
| N4 | Global version lock | STILL PRESENT |
| N5 | Missing builder methods | PARTIAL — memory exists, no smp/disk/net |
| N6 | No capability negotiation | STILL MISSING |
| N7 | Event channel bounds | NOTED — bounded at 100, not configurable |
| N8 | No Send/Sync assertions | STILL MISSING |

## NEW Gaps (Round 3)

### N9. No Error Type for QGA
`qga.rs` uses `anyhow::bail!` — no typed QGA error enum.
- **Priority:** must-have | **Complexity:** small

### N10. QGA Client Not Async-Safe for Concurrent Use
`Qga::execute()` takes `&mut self`. Single-command-at-a-time. QMP uses channels.
- **Priority:** should-have | **Complexity:** medium

### N11. No Typed Event Dispatch
`QapiEvent` trait generated but no runtime dispatcher. User manually matches string + deserializes Value.
- **Priority:** must-have | **Complexity:** medium

### N12. Mock QGA Server Missing Version Response
No helper to respond to `guest-info` that `Qga::new()` sends. Test writers handle manually.
- **Priority:** nice-to-have | **Complexity:** small

## Priority Matrix

| Priority | Items |
|----------|-------|
| **Must-have** | Typed errors (#2), Qmp Drop (#N3), typed event dispatch (#N11), disk/net/smp builders (#N5), QGA error type (#N9) |
| **Should-have** | OOB negotiation (#N6), per-connection version (#N4), Send/Sync assertions (#N8), QGA concurrency (#N10) |
| **Nice-to-have** | Feature flags (#7), syn alignment (#9), rustdoc (#8), QGA mock helper (#N12) |

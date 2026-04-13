# Feature Gaps Audit Report

Generated: 2026-04-13 | Re-audited: 2026-04-13

## Prior Gaps Status

| # | Gap | Status |
|---|-----|--------|
| 1 | Compile-time version gating not wired | STILL MISSING |
| 2 | No typed errors — anyhow everywhere | STILL MISSING |
| 3 | Missing typed event deserialization | STILL MISSING (types generated but unused at runtime) |
| 4 | QGA client gaps | WORSE — client may be incomplete/absent |
| 5 | QMP client gaps (timeout, OOB, reconnect, TCP) | STILL MISSING |
| 6 | Manager gaps (disk/net/KVM/stderr) | STILL MISSING |
| 7 | No feature flags | STILL MISSING (only bmc has features) |
| 8 | Documentation gaps | STILL MISSING |
| 9 | syn version mismatch | STILL PRESENT (qapi-macros syn 1, codegen syn 2) |

**Score: 0/9 addressed. 1 regressed (QGA).**

## NEW Gaps Found

### N1. QGA Client Possibly Nonexistent
Prior report assumed working client with missing features. Re-audit found `pub mod qga` declared but implementation unclear. Blocks all guest-agent use cases.
- **Priority:** must-have | **Complexity:** large

### N2. Zero Tests in Protocol + Manager
No `#[test]` in either crate. Only codegen + qapi have tests.
- **Priority:** must-have | **Complexity:** medium

### N3. No Drop on Qmp Client
Spawned tokio tasks leak if dropped without `shutdown()`.
- **Priority:** must-have | **Complexity:** small

### N4. Global Version Lock — Single-QEMU-per-Process
`OnceLock` in `set_qemu_version()`. Can't manage multiple QEMU versions.
- **Priority:** should-have | **Complexity:** medium

### N5. Missing Builder Methods: memory, smp, cpu override
Users forced to use `.arg("-m")` / `.arg("-smp")`.
- **Priority:** must-have | **Complexity:** small

### N6. No Capability Negotiation Options
`qmp_capabilities` sent with no arguments. Can't enable OOB.
- **Priority:** should-have | **Complexity:** small

### N7. Event Channel Bounds Unknown
If bounded too small, events dropped. If unbounded, memory leak.
- **Priority:** should-have | **Complexity:** small

### N8. No `Send`/`Sync` Bounds Verification
No compile-time assertions. Could break silently for multi-threaded tokio.
- **Priority:** should-have | **Complexity:** small

## Priority Matrix (Updated)

| Priority | Items |
|----------|-------|
| **Must-have** | Typed errors, typed events, QGA client, version gating, disk/net/memory/smp builders, Qmp Drop, protocol+manager tests |
| **Should-have** | OOB/capability negotiation, per-connection version, event channel bounds, Send/Sync assertions, KVM/accel, more architectures, command timeouts |
| **Nice-to-have** | Feature flags, syn alignment, rustdoc, reconnection, TCP transport |
| **Future** | Runtime-agnostic, migration helpers, hotplug ergonomics |

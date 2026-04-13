# Feature Gaps Audit Report

Generated: 2026-04-13

## 1. Compile-Time Version Gating — Not Wired

Env vars read by macro but types not actually stripped. `build.rs` only does `rerun-if-env-changed`.

- **Priority:** must-have | **Complexity:** medium

## 2. No Typed Errors — anyhow Everywhere

All crates use `anyhow::Result`. Consumers can't match on error variants.

- **Priority:** must-have | **Complexity:** medium

## 3. Missing Typed Event Deserialization

`QmpEvent` stores raw `serde_json::Value`. 60 generated event types unused. No `try_into_typed()`.

- **Priority:** must-have | **Complexity:** medium

## 4. QGA Client Gaps

No timeouts, no reconnection, no `shutdown()` method.

- **Priority:** timeouts must-have | **Complexity:** small-medium

## 5. QMP Client Gaps

No reconnect, no OOB support, no command timeout, no concurrent commands, no TCP, global version lock.

- **Priority:** OOB + timeout must-have | **Complexity:** varies

## 6. Manager Gaps

No disk/net config, no snapshot/migration wrappers, no KVM/accel option, no stderr capture, missing architectures (riscv64, s390x, ppc64, mips).

- **Priority:** disk/net + KVM must-have | **Complexity:** medium

## 7. No Feature Flags

Core crates have zero feature flags. Should gate: `qga`, `mock-server`, tokio, tracing.

- **Priority:** nice-to-have | **Complexity:** small

## 8. Documentation Gaps

No rustdoc examples. No crate-level docs. No READMEs.

- **Priority:** nice-to-have | **Complexity:** small

## 9. syn Version Mismatch

`qapi-macros` uses `syn = "1"` (unmaintained). `qapi-codegen` uses `syn = "2"`.

- **Priority:** nice-to-have | **Complexity:** small

## Priority Matrix

| Priority | Items |
|----------|-------|
| **Must-have** | Typed errors, typed events, OOB, timeouts, version gating, disk/net builders, KVM, stderr capture, multi-VM version |
| **Nice-to-have** | Feature flags, syn alignment, QGA timeouts, reconnection, TCP, rustdoc, architectures |
| **Future** | Runtime-agnostic, BMC dashboard, migration helpers, hotplug ergonomics |

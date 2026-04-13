# Bug Audit Report

Generated: 2026-04-13 | Re-audited: 2026-04-13

## Prior Bugs Status

| # | Bug | Severity | Status |
|---|-----|----------|--------|
| 1 | QMP response/event interleave race | Critical | STILL PRESENT |
| 2 | No Drop on Qmp — task leak | High | STILL PRESENT |
| 3 | `as i64` nanos truncation | High | STILL PRESENT |
| 4 | OnceLock silently drops multi-VM version | High | STILL PRESENT |
| 5 | Zombie processes | Medium | STILL PRESENT |
| 6 | `.expect()` panic on ARM+BIOS | Medium | STILL PRESENT |
| 7 | No early-exit on QEMU crash | Medium | STILL PRESENT |
| 8 | QEMU orphaned on version mismatch | Medium | STILL PRESENT |
| 9 | Empty tags unwrap | Low | STILL PRESENT |
| 10 | `parent().unwrap()` | — | **FALSE POSITIVE** (`.parent()` returns `Some("")` for bare filenames) |
| 11 | panic! in IR processing | Low | STILL PRESENT |
| 12 | thread join masks panics | Low | STILL PRESENT |
| 13 | Alternate deser order-dependent | Medium | STILL PRESENT |
| 14 | deprecated_by_bounds broken | Low | STILL PRESENT |

## NEW Bugs Found (Re-audit)

### BUG 15 — shutdown may not interrupt read_loop

**Severity: MEDIUM** | `projects/qapi-protocol/src/lib.rs:169-180`

`shutdown()` sends on `shutdown_signal`, then awaits handles sequentially. If QEMU flooding events, `tokio::select!` may pick `next_line()` branch over shutdown. Shutdown delayed indefinitely.

**Fix:** After sending shutdown signal, abort `read_handle` with timeout.

---

### BUG 16 — write_loop blocks forever if QEMU hangs

**Severity: HIGH** | `projects/qapi-protocol/src/lib.rs:223`

After writing command, `write_loop` waits on `response_rx.recv()` outside any `select!`. If QEMU never responds, blocks forever. No timeout. Blocks all subsequent commands.

**Fix:** Wrap `response_rx.recv()` in `tokio::select!` with shutdown + timeout.

---

### BUG 17 — Greeting parse failure silently ignored

**Severity: MEDIUM** | `projects/qapi-protocol/src/lib.rs:54-65`

`if let Ok(parsed)` silently proceeds on corrupt greeting. Version stays `(0,0,0)`. Confusing downstream errors.

**Fix:** Bail on greeting parse failure with descriptive error.

---

### BUG 18 — Event before caps response misrouted

**Severity: MEDIUM** | `projects/qapi-protocol/src/lib.rs:73-81`

QEMU can emit events between greeting and capabilities response. If event arrives first, parsed as caps response. Negotiation appears to succeed but caps response never consumed.

**Fix:** Loop reading lines, skip events (those with `"event"` key), until response received.

---

### BUG 19 — Empty line vs disconnect in mock server

**Severity: LOW** | `projects/qapi-protocol/src/server.rs:71-76`

Client sends `"\n"` → `from_str("")` → confusing serde error instead of "client disconnected".

**Fix:** Check `line.trim().is_empty()` after read.

---

### BUG 20 — version_str clamps major to minimum 1

**Severity: LOW** | `projects/qapi-codegen/src/unified.rs:21-23`

Version `(0, x, y)` silently becomes `(1, x, y)`. Corrupts pre-1.0 version annotations.

**Fix:** Remove `.max(1)` or document why needed.

---

### BUG 21 — Vm/PreconfigVm QMP task leak on panic

**Severity: LOW** | `projects/manager/src/vm.rs:315-370,379-440`

If user code panics holding Vm, VmProcess::Drop kills child (good), but Qmp tasks leak (ties to BUG 2).

---

## Summary

| # | Bug | Severity | Crate | Status |
|---|-----|----------|-------|--------|
| 1 | QMP response/event interleave race | Critical | qapi-protocol | OPEN |
| 2 | No Drop on Qmp — task leak | High | qapi-protocol | OPEN |
| 3 | `as i64` nanos truncation | High | qapi-protocol | OPEN |
| 4 | OnceLock silently drops multi-VM version | High | qapi | OPEN |
| 5 | Zombie processes | Medium | manager | OPEN |
| 6 | `.expect()` panic on ARM+BIOS | Medium | manager | OPEN |
| 7 | No early-exit on QEMU crash | Medium | manager | OPEN |
| 8 | QEMU orphaned on version mismatch | Medium | manager | OPEN |
| 9 | Empty tags unwrap | Low | qapi-codegen | OPEN |
| 10 | parent().unwrap() | — | qapi-codegen | FALSE POSITIVE |
| 11 | panic! in IR processing | Low | qapi-codegen | OPEN |
| 12 | thread join masks panics | Low | qapi-codegen | OPEN |
| 13 | Alternate deser order-dependent | Medium | qapi-macros | OPEN |
| 14 | deprecated_by_bounds broken | Low | qapi-macros | OPEN |
| 15 | shutdown may not interrupt read_loop | Medium | qapi-protocol | NEW |
| 16 | write_loop blocks forever on hung QEMU | High | qapi-protocol | NEW |
| 17 | Greeting parse failure silent | Medium | qapi-protocol | NEW |
| 18 | Event before caps misrouted | Medium | qapi-protocol | NEW |
| 19 | Empty line in mock server | Low | qapi-protocol | NEW |
| 20 | version_str clamps major to 1 | Low | qapi-codegen | NEW |
| 21 | QMP task leak on panic | Low | manager | NEW |

**Totals:** 1 critical, 4 high, 7 medium, 8 low, 1 false positive. 7 new since first audit. Zero fixed.

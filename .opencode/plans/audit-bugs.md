# Bug Audit Report

Generated: 2026-04-13 | Re-audited: 2026-04-13 (x3)

## Prior Bugs Status

| # | Bug | Severity | Status |
|---|-----|----------|--------|
| 1 | QMP response/event interleave race | Critical | OPEN (partially mitigated — commands serialized, but unexpected JSON misrouted) |
| 2 | No Drop on Qmp — task leak | High | OPEN |
| 3 | `as i64` nanos truncation | High | OPEN |
| 4 | OnceLock silently drops multi-VM version | High | OPEN |
| 5 | Zombie processes | Medium | OPEN |
| 6 | `.expect()` panic on ARM+BIOS | Medium | **FIXED** — now `ok_or_else` with `?` |
| 7 | No early-exit on QEMU crash | Medium | OPEN |
| 8 | QEMU orphaned on version mismatch | Medium | **PARTIAL** — `kill_on_drop(true)` kills child, but Qmp tasks leak |
| 9 | Empty tags unwrap | Low | OPEN |
| 10 | `parent().unwrap()` | — | FALSE POSITIVE |
| 11 | panic! in IR processing | Low | OPEN |
| 12 | thread join masks panics | Low | OPEN |
| 13 | Alternate deser order-dependent | Medium | OPEN |
| 14 | deprecated_by_bounds broken | Low | OPEN |
| 15 | shutdown may not interrupt read_loop | Medium | OPEN (lower severity — eventually wins) |
| 16 | write_loop blocks forever on hung QEMU | High | OPEN |
| 17 | Greeting parse failure silent | Medium | **FIXED** — now `map_err` + `?` with descriptive error |
| 18 | Event before caps misrouted | Medium | **FIXED** — now loops skipping events |
| 19 | Empty line in mock server | Low | **PARTIAL** — QMP fixed (`trim().is_empty()`), QGA still broken |
| 20 | version_str clamps major to 1 | Low | **FIXED** — clean pass-through |
| 21 | QMP task leak on panic | Low | OPEN |

## NEW Bugs (Round 3)

### BUG 22 — Silent data loss on malformed JSON/events

**Severity: LOW** | `projects/qapi-protocol/src/lib.rs:200,248`

Two `if let Ok(parsed)` silently drop malformed JSON from QEMU. No logging.

**Fix:** Add `tracing::warn!` on parse failures.

### BUG 23 — Partial command write on newline failure

**Severity: MEDIUM** | `projects/qapi-protocol/src/lib.rs:225-228`

Command body write checked, but newline write error ignored with `let _ =`. Protocol corruption if newline fails.

**Fix:** Check newline write result; break on error.

### BUG 24 — Qga::sync() unwrap on SystemTime

**Severity: LOW** | `projects/qapi-protocol/src/qga.rs:43-46`

`.duration_since(UNIX_EPOCH).unwrap()` panics if clock before epoch.

**Fix:** `.unwrap_or_default()` or random i64.

### BUG 25 — Qmp::execute has no timeout

**Severity: MEDIUM** | `projects/qapi-protocol/src/lib.rs:140-145`

`reply_rx.await?` blocks forever. No way for caller to bound execution time.

**Fix:** Accept optional `Duration` timeout, wrap in `tokio::time::timeout`.

## Summary

| Severity | Open | Fixed |
|----------|------|-------|
| Critical | 1 | 0 |
| High | 4 | 0 |
| Medium | 6 | 3 |
| Low | 7 | 1 |
| **Total** | **18** | **4** |

# Bug Audit Report

Generated: 2026-04-13 | Round 4

## Status

| # | Bug | Severity | Status |
|---|-----|----------|--------|
| 1 | QMP response/event interleave | Medium | OPEN (reduced from Critical — commands serialized, theoretical only) |
| 2 | No Drop on Qmp | High | **FIXED** — Drop impl aborts all task handles |
| 3 | `as i64` nanos truncation | Low | OPEN (reduced — truncated value round-trips correctly) |
| 4 | OnceLock silently drops multi-VM version | High | OPEN |
| 5 | Zombie processes | Medium | **FIXED** — spawn_monitor + kill_on_drop |
| 6 | `.expect()` panic on ARM+BIOS | Medium | FIXED (r3) |
| 7 | No early-exit on QEMU crash | Medium | OPEN |
| 8 | QEMU orphaned on version mismatch | Medium | **FIXED** — kill_on_drop + Qmp Drop |
| 9 | Empty tags unwrap | Low | **FALSE POSITIVE** — ensure! guard exists |
| 10 | parent().unwrap() | — | FALSE POSITIVE |
| 11 | panic! in IR processing | Low | OPEN |
| 12 | thread join masks panics | Low | **FIXED** — unwrap_or_else → anyhow error |
| 13 | Alternate deser order-dependent | Medium | **FIXED** — alt_variant_priority sorts by specificity |
| 14 | deprecated_by_bounds broken | Low | **FIXED** — now compares dep_ver <= min |
| 15 | shutdown may not interrupt read_loop | Medium | **FIXED** — timeout + abort on each handle |
| 16 | write_loop blocks forever | High | **FIXED** — 30s timeout |
| 17 | Greeting parse failure silent | Medium | FIXED (r3) |
| 18 | Event before caps misrouted | Medium | FIXED (r3) |
| 19 | Empty line in mock server (QGA) | Low | PARTIAL — QMP fixed, QGA still uses `line.is_empty()` |
| 20 | version_str clamps major to 1 | Low | FIXED (r3) |
| 21 | QMP task leak on panic | Low | **FIXED** — via Qmp Drop |
| 22 | Silent data loss on malformed JSON | Low | OPEN |
| 23 | Partial command write on newline fail | Medium | OPEN |
| 24 | Qga::sync() unwrap on SystemTime | Low | **FIXED** — unwrap_or_default |
| 25 | Qmp::execute no timeout | Medium | **FIXED** — implicit via write_loop 30s |
| 26 | QGA execute_raw no timeout | Medium | NEW |
| 27 | QGA EOF cryptic error | Low | NEW |
| 28 | qapi-macros returns parse panics | Low | NEW |
| 29 | QEMU crash not propagated to events | Medium | NEW |
| 30 | VmProcess Drop sync I/O | Low | NEW |

## Totals

| | Count |
|---|---|
| Fixed | 15 |
| False Positive | 3 |
| Open | 12 (1 high, 5 medium, 6 low) |

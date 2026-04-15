# Bug Audit Report

Generated: 2026-04-13 | Round 6

## Status

| # | Bug | Severity | Status |
|---|-----|----------|--------|
| 1 | QMP response/event interleave | Low | OPEN (benign — commands serialized, id correlation added) |
| 2 | No Drop on Qmp | High | FIXED (r4) |
| 3 | `as i64` nanos truncation | Low | OPEN (by design) |
| 4 | OnceLock silently drops multi-VM version | High | OPEN |
| 5 | Zombie processes | Medium | FIXED (r4) |
| 6 | `.expect()` panic on ARM+BIOS | Medium | FIXED (r3) |
| 7 | No early-exit on QEMU crash | Medium | OPEN (monitor logs but doesn't notify events) |
| 8 | QEMU orphaned on version mismatch | Medium | FIXED (r4) |
| 9 | Empty tags unwrap | Low | FALSE POSITIVE (r4) |
| 10 | parent().unwrap() | — | FALSE POSITIVE |
| 11 | panic! in IR processing | Low | OPEN (mitigated — panics caught by thread scope) |
| 12 | thread join masks panics | Low | FIXED (r4) |
| 13 | Alternate deser order-dependent | Medium | FIXED (r4) |
| 14 | deprecated_by_bounds broken | Low | FIXED (r4) |
| 15 | shutdown may not interrupt read_loop | Medium | FIXED (r4) |
| 16 | write_loop blocks forever | High | FIXED (r4) |
| 17 | Greeting parse failure silent | Medium | FIXED (r3) |
| 18 | Event before caps misrouted | Medium | FIXED (r3) |
| 19 | Empty line in mock server (QGA) | Low | FIXED (r5) |
| 20 | version_str clamps major to 1 | Low | FIXED (r3) |
| 21 | QMP task leak on panic | Low | FIXED (r4) |
| 22 | Silent data loss on malformed JSON | Low | OPEN (logs via tracing::warn) |
| 23 | Partial command write on newline fail | Medium | OPEN |
| 24 | Qga::sync() unwrap on SystemTime | Low | FIXED (r4) |
| 25 | Qmp::execute no timeout | Medium | FIXED (r4) |
| 26 | QGA execute_raw no timeout | Medium | FIXED (r5) |
| 27 | QGA EOF cryptic error | Low | FIXED (r5) |
| 28 | qapi-macros returns parse panics | Low | FIXED (r5) |
| 29 | QEMU crash not propagated to events | Medium | OPEN |
| 30 | VmProcess Drop sync I/O | Low | FIXED (r5) |
| 31 | QGA sync() no timeout on 0xFF loop | Medium | OPEN |
| 32 | Misleading version error message | Low | OPEN |
| 33 | Temp file leak on version bail | Low | OPEN |
| 34 | Timeout vs channel-close indistinguishable | Low | OPEN (documented) |
| 35 | QGA sync() read_line no timeout | Low | OPEN |
| 46 | Mock server doesn't echo id field | Low | NEW (r6) |
| 47 | spawn_monitor error no exit publish | Medium | **FIXED** (r6) |
| 49 | event_worker backpressure | Low | OPEN (downgraded) |
| 50 | event_worker after consumer drop | Low | OPEN |
| 51 | Mock server unbounded read_line | Low | NEW (r6) |
| 52 | Mock negotiate event timing | Low | NEW (r6) |
| 53 | QGA no id correlation | Low | NEW (r6) |

## Totals

| | Count |
|---|---|
| Fixed | 21 |
| False Positive | 3 |
| Open | 15 (1 high, 4 medium, 10 low) |

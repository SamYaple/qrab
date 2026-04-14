# Bug Audit Report

Generated: 2026-04-13 | Round 8

## Totals: 29 fixed / 3 FP / 13 open (1 high, 6 medium, 6 low)

| # | Bug | Severity | Status |
|---|-----|----------|--------|
| 1 | QMP response/event interleave | Medium | OPEN (theoretical) |
| 4 | OnceLock silently drops multi-VM version | High | OPEN |
| 7 | No early-exit on QEMU crash | Medium | OPEN |
| 11 | panic! in IR processing | Low | OPEN (mitigated) |
| 22 | Silent data loss on malformed JSON | Low | OPEN (logs via tracing) |
| 23 | Partial command write on newline fail | Medium | OPEN |
| 29 | QEMU crash not propagated to events | Medium | OPEN |
| 32 | Misleading version error message | Low | OPEN |
| 34 | Timeout vs channel-close indistinguishable | Low | OPEN |
| 42 | Response desync after write_loop timeout | Medium | NEW |
| 43 | Event backpressure deadlocks QMP responses | Medium | NEW |
| 44 | Temp file leak on socket-wait bail | Low | NEW |
| 45 | Blocking Path::exists() in async | Low | NEW |

### Fixed this round: #40 (QGA write bounded), #41 (early-exit dead QEMU)

### All fixed bugs: 2,3(design),5,6,8,9(FP),10(FP),12,13,14,15,16,17,18,19,20,21,24,25,26,27,28,30,31,33,35,36,37,38,39,40,41

# Bug Audit Report

Generated: 2026-04-13 | Round 11

## Totals: 35 fixed / 4 FP / 10 open (1 high, 3 medium, 6 low) + 1 new

| # | Bug | Severity | Status |
|---|-----|----------|--------|
| 1 | QMP response/event interleave | Low | OPEN (benign — id correlation) |
| 4 | OnceLock silently drops multi-VM version | High | OPEN |
| 11 | panic! in IR processing | Low | OPEN (mitigated) |
| 22 | Silent data loss on malformed JSON | Low | OPEN (logs via tracing) |
| 29 | QEMU crash not propagated to events | Medium | OPEN |
| 32 | Misleading version error message | Low | OPEN |
| 34 | Timeout vs channel-close indistinguishable | Low | OPEN (documented) |
| 46 | Mock server doesn't echo id field | Low | OPEN |
| 47 | spawn_monitor error branch doesn't publish exit | Medium | OPEN |
| 49 | event_worker send vs try_send backpressure | Low | OPEN (downgraded — shutdown aborts after 500ms) |
| 50 | event_worker continues after consumer drops | Low | NEW |

### Changes: #48 → FALSE POSITIVE. #49 downgraded medium→low.
### Fixed this round: none (0 bug fixes detected)

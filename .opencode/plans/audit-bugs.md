# Bug Audit Report

Generated: 2026-04-13 | Round 10

## Totals: 35 fixed / 3 FP / 9 open (0 high, 4 medium, 5 low) + 3 new

| # | Bug | Severity | Status |
|---|-----|----------|--------|
| 1 | QMP response/event interleave | Low | OPEN (downgraded — id correlation makes benign) |
| 4 | OnceLock silently drops multi-VM version | High | OPEN |
| 11 | panic! in IR processing | Low | OPEN (mitigated) |
| 22 | Silent data loss on malformed JSON | Low | OPEN (logs via tracing) |
| 29 | QEMU crash not propagated to events | Medium | OPEN |
| 32 | Misleading version error message | Low | OPEN |
| 34 | Timeout vs channel-close indistinguishable | Low | OPEN (documented) |
| 46 | Mock server doesn't echo id field | Low | OPEN |
| 47 | spawn_monitor error branch doesn't publish exit | Medium | NEW |
| 48 | Mock negotiate doesn't echo id | Low | NEW |
| 49 | event_worker uses send not try_send (backpressure) | Medium | NEW |

### Fixed this round: #7 (exit_watch exposed), #23 (both writes checked)
### BUG 1 downgraded medium→low (id correlation)

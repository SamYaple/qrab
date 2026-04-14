# Bug Audit Report

Generated: 2026-04-13 | Round 9

## Totals: 33 fixed / 3 FP / 10 open (1 high, 4 medium, 5 low)

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
| 34 | Timeout vs channel-close indistinguishable | Low | OPEN (acknowledged in code) |
| 46 | Mock server doesn't echo id field (test gap) | Low | NEW |

### Fixed this round: #42 (response desync via id), #43 (event backpressure try_send), #44 (temp file cleanup), #45 (tokio::fs::try_exists)

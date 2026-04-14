# Industry Standard Features Audit

Generated: 2026-04-13 | Round 10

## Changes: #72 reclassified by-design. #74 confirmed HIGH. 8 new gaps.

## NEW Gaps

| # | Gap | Priority |
|---|-----|----------|
| 76 | BMC OnceLock singleton (test isolation) | LOW |
| 77 | Nested lock ordering in dashboard (deadlock risk) | MEDIUM |
| 78 | Mock QmpSession unbounded read_line | LOW |
| 79 | Mock QgaSession unbounded read_line | LOW |
| 80 | Qmp::shutdown races with Drop | MEDIUM |
| 81 | BMC ignores accel/smp builder methods | MEDIUM |
| 82 | event_worker backpressure (send not try_send) | LOW |
| 83 | bmc not in CLAUDE.md | LOW |

## Production Blockers (updated)
1. ~~#66+#67 No KVM/SMP~~ — FIXED
2. **#81** — BMC ignores accel/smp — builder exists but BMC never calls it
3. **#69+#74** — Dead QEMU invisible + phase() swallows errors
4. **#12** — No reconnect
5. **#60** — OnceLock blocks power-cycle

**Total: 83 gaps, ~74 open.**

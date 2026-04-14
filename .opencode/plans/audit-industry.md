# Industry Standard Features Audit

Generated: 2026-04-13 | Round 9

## Changes This Round
- **#66 No `-accel` builder → FIXED** — Kvm/Tcg/KvmOrTcg enum
- **#67 No `-smp` builder → FIXED** — `cpus=N` form

## NEW Gaps

| # | Gap | Priority |
|---|-----|----------|
| 71 | ensure_cirros_image curl no timeout | MEDIUM |
| 72 | force_off clears event_history | LOW |
| 73 | No structured error types in manager | MEDIUM |
| 74 | phase() swallows QMP errors as "Running" | HIGH |
| 75 | power_on holds vm mutex across spawn (~seconds) | MEDIUM |

## Production Blockers (updated)
1. ~~#66+#67 No KVM/SMP~~ — **FIXED**
2. **#69+#74** — Dead QEMU invisible + phase() swallows errors
3. **#12** — No reconnect
4. **#60** — OnceLock blocks power-cycle
5. **#68+#75** — Mutex held during I/O

**Total: 75 gaps, ~66 open. 2 fixed, 5 new.**

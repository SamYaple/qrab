# Industry Standard Features Audit

Generated: 2026-04-13 | Round 8

## Changes This Round
- **#34+#59 Connection timeout → FIXED** (5s connect + 10s handshake)
- **#18 Process death → PARTIAL** (exit_watch exists, BMC doesn't consume it)

## NEW Gaps

| # | Gap | Priority |
|---|-----|----------|
| 66 | No `-accel` builder method — can't enable KVM | HIGH |
| 67 | No `-smp` builder method — all VMs single-core | HIGH |
| 68 | phase() holds mutex during QMP round-trip | MEDIUM |
| 69 | exit_watch not consumed by BMC — dead VM invisible | MEDIUM |
| 70 | set_qemu_version test inconsistency (OnceLock) | LOW |

## Production Blockers
1. **#66+#67** — No KVM, no SMP. TCG single-core = unusable. Trivial fix (~15 lines).
2. **#69+#18** — Dead QEMU invisible. exit_watch exists but BMC ignores it.
3. **#12** — No reconnect. Any QMP drop = permanent failure.
4. **#60** — OnceLock blocks power-cycle correctness.

**Total: 70 gaps tracked, ~65 open.**

# Industry Standard Features Audit

Generated: 2026-04-13 | Round 11

## Changes
- #81 PARTIAL — BMC calls `.accel(Kvm).cpu("host")` but never `.smp()`
- #77 reclassified LOW (lock ordering serialized)
- #72 reclassified BY-DESIGN (intentional event clear)

## NEW Gaps

| # | Gap | Priority |
|---|-----|----------|
| 84 | Temp overlay leaked on force_off | MEDIUM |
| 85 | NsHelper single read() for PID (may split) | MEDIUM |
| 86 | Pre-exec RawFd capture race | LOW |
| 87 | ensure_cirros uses CARGO_MANIFEST_DIR at compile time | LOW |
| 88 | graceful_shutdown silent error drops | LOW |

## Production Blockers
1. **#69+#74** — Dead QEMU invisible + phase() swallows errors
2. **#81** — BMC ignores smp (single-CPU VMs)
3. **#84** — Overlay leak fills /tmp over power cycles
4. **#60** — OnceLock blocks power-cycle correctness
5. **#12** — No reconnect

**Total: 88 gaps, ~76 open.**

# Industry Standard Features Audit

Generated: 2026-04-13 | Round 6

Compared against: libvirt, proxmox-ve, cloud-hypervisor

## Changes This Round

| # | Gap | Change |
|---|-----|--------|
| #69/#74 | Dead QEMU invisible + phase() errors | PARTIALLY FIXED — exit_watch exists, but phase() swallows QMP errors |
| N45 | Graceful shutdown | CLOSED — ACPI→quit→SIGKILL chain |

## NEW Gaps (Round 6)

| # | Gap | Priority |
|---|-----|----------|
| 89 | No network device support — VMs have no network | HIGH |
| 90 | No migration/snapshot support | MEDIUM |
| 91 | BMC OnceLock singleton prevents multi-VM | MEDIUM |
| 92 | phase() holds vm Mutex during QMP round-trip (blocks all actions) | MEDIUM |
| 93 | No disk hotplug/unplug (only during paused pre-start) | MEDIUM |
| 94 | force_off never tries graceful first | MEDIUM |
| 95 | execute/execute_with_timeout code duplication | LOW |
| 96 | No TLS/auth on QMP socket | LOW |
| 97 | VNC proxy no auth | MEDIUM |

## Priority Summary

**Critical:** #12 retry/reconnect, #18 process death→state, #84 overlay leak

**HIGH:** #39 no KVM, #38 no SMP, #81 BMC ignores smp, #89 no networking, #60 OnceLock blocks power-cycle

**MEDIUM:** #90-#94, #97 + prior medium gaps

**Score vs industry:** qrab covers ~12 of 97 tracked capabilities. Libvirt covers all 97.

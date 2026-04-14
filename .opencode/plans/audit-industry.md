# Industry Standard Features Audit

Generated: 2026-04-13 | Round 7

Compared against: libvirt, proxmox-ve, cloud-hypervisor

## Changes This Round
- #34 Connection timeout → PARTIAL (handshake 10s, connect() itself unguarded)
- #16 Command timeout → PARTIAL (30s hardcoded, not configurable)
- #50 Event forwarder orphan → MITIGATED (500ms bounded wait, no abort)

## 65 Total Gaps (7 new this round)

### CRITICAL (2)
- #12 Retry/reconnect — STILL MISSING
- #18 Process death → state — STILL MISSING

### HIGH (18, +1 new)
All prior HIGH items unchanged. New:
- #59 No connect() timeout on UnixStream — HIGH

### MEDIUM (22, +3 new)
New:
- #61 Forwarder not aborted on timeout — MEDIUM
- #62 No disk removal from BMC — MEDIUM
- #64 No graceful BMC shutdown (SIGTERM) — MEDIUM

### LOW (10, +2 new)
New:
- #63 Dashboard polls phase() redundantly — LOW
- #65 SSE stream silent on broadcast lag — LOW

Also:
- #60 OnceLock blocks power-cycle correctness — HIGH (refinement of #32)

**Production blockers:** #39+#49 (no KVM) + #38 (no SMP). Then #18 (dead VM shows Running).

# Industry Standard Features Audit

Generated: 2026-04-13 | Round 5

Compared against: libvirt, proxmox-ve, cloud-hypervisor

## 48 Total Gaps

### CRITICAL (2)
- #12 Retry/reconnect — STILL MISSING
- #18 Process death → state transition — STILL MISSING

### HIGH (14)
- #3 Network — STILL MISSING
- #5 Migration — STILL MISSING
- #6 Snapshots — STILL MISSING
- #14 Graceful shutdown escalation — STILL MISSING
- #15 OOB commands — STILL NOT NEGOTIATED
- #21 Hot-plug/unplug — STILL MISSING
- #26 Feature detection — STILL MISSING
- #28 Event-driven state machine — STILL MISSING
- #29 QMP disconnect detection — STILL MISSING
- #31 Typed errors — STILL MISSING
- #32 OnceLock version — STILL PRESENT
- #34 Connection timeout — STILL MISSING
- #38 No SMP config — STILL MISSING
- #39 No KVM/accel — STILL MISSING
- #41 No QMP command ID field — NEW
- #42 No SIGTERM handler / orphan cleanup — NEW

### MEDIUM (18)
- #2 Storage (BMC-only) — IMPROVED
- #7 Device builders — PARTIAL
- #8 QGA integration — PARTIAL
- #9 SMP/NUMA/CPU — STILL MISSING
- #10 Observability — IMPROVED
- #16 Command timeout — PARTIAL (30s hardcoded)
- #17 Serial console — PARTIAL (file only)
- #19 Socket permissions — STILL MISSING
- #22 Resource limits — STILL MISSING
- #24 Cloud-init — STILL MISSING
- #25 VNC auth — STILL MISSING
- #27 Machine type versioning — STILL MISSING
- #30 Multi-VM — STILL MISSING
- #33 QMP pipelining — STILL MISSING
- #35 QGA thread safety — STILL MISSING
- #40 BMC config not persistent — STILL MISSING
- #43 SSE no heartbeat — NEW
- #44 No memory string validation — NEW
- #45 Cirros download no checksum — NEW

### LOW (5)
- #13 BMC docs — STILL MISSING
- #20 Multi-disk interfaces — STILL MISSING
- #23 UEFI/TPM — STILL MISSING
- #36 QEMU binary validation — STILL MISSING
- #46 No health endpoint — NEW
- #47 stderr not surfaced to UI — NEW
- #48 OVMF search hardcoded paths — NEW

**Production blockers:** #39 (no KVM) + #38 (no SMP) = every VM runs 1 TCG vCPU.

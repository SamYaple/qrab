# Industry Standard Features Audit

Generated: 2026-04-13 | Round 6

Compared against: libvirt, proxmox-ve, cloud-hypervisor

## Changes This Round
- **#19 Socket permissions → FIXED** (chmod 0o600)
- **#36 QEMU binary validation → PARTIAL** (detect_version + is_available)
- **#47 stderr not surfaced → FIXED** (tracing::warn)
- **#17 Serial console → PARTIAL+** (64KB tail read)
- **#10 Observability → MORE IMPROVED** (dashboard atomic snapshot)

## 58 Total Gaps (3 fixed, 1 improved, 10 new)

### CRITICAL (2)
- #12 Retry/reconnect — STILL MISSING
- #18 Process death → state — STILL MISSING

### HIGH (16)
#3 Network, #5 Migration, #6 Snapshots, #14 Shutdown escalation, #15 OOB, #21 Hot-plug, #26 Feature detection, #28 Event-driven state, #29 QMP disconnect, #31 Typed errors, #32 OnceLock version, #34 Connection timeout, #38 No SMP, #39 No KVM, #41 No command IDs, #42 No SIGTERM handler, #49 No KVM auto-detect (NEW)

### MEDIUM (19+)
#2 Storage (BMC-only), #7 Device builders, #8 QGA integration, #9 SMP/NUMA, #16 Command timeout (hardcoded), #22 Resource limits, #24 Cloud-init, #25 VNC auth, #27 Machine versioning, #30 Multi-VM, #33 QMP pipelining, #35 QGA safety, #40 Config persistence, #43 SSE heartbeat, #44 Memory validation, #45 Cirros checksum, #50 Event forwarder orphan (NEW), #52 phase() holds mutex (NEW), #53 Single-VM singleton (NEW), #56 No disk removal (NEW), #58 No CPU model selection (NEW)

### LOW (8)
#13 BMC docs, #20 Multi-disk, #23 UEFI/TPM, #46 Health endpoint, #48 OVMF paths, #54 No firmware env override (NEW), #55 kill_on_drop race (NEW), #57 Broadcast overflow (NEW)

**Production blockers:** #39+#49 (no KVM) + #38 (no SMP).

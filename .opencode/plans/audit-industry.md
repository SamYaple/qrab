# Industry Standard Features Audit

Generated: 2026-04-13 | Re-audited: 2026-04-13

Compared against: libvirt, proxmox-ve, cloud-hypervisor

## Prior Gaps Status

| # | Feature | Prior Status | Current Status |
|---|---------|-------------|----------------|
| 1 | VM Lifecycle | PARTIAL | **IMPROVED** — BMC adds VmState enum, VmPhase. Still missing graceful shutdown escalation |
| 2 | Storage Management | MISSING | **IMPROVED** — BMC has attach_disks() with blockdev-add chain. Not in manager crate |
| 3 | Network Management | MISSING | STILL MISSING — zero progress |
| 4 | Monitoring / Health | PARTIAL | **IMPROVED** — typed events generated (732 lines), BMC broadcasts events. wait_for_event exists but test-only |
| 5 | Migration | MISSING | STILL MISSING |
| 6 | Snapshots | MISSING | STILL MISSING |
| 7 | Device Management | MISSING | **PARTIAL** — device_add used in attach_disks. No typed device builders |
| 8 | Guest Agent | PARTIAL | STILL PARTIAL — QGA client exists, no manager integration |
| 9 | Configuration | PARTIAL | **MINOR PROGRESS** — memory config as string. Still no SMP/NUMA/CPU model |
| 10 | Observability | MINIMAL | STILL MINIMAL — few tracing calls |
| 11 | Connection Pooling | MISSING | N/A (correctly scoped as single-VM) |
| 12 | Retry / Reconnect | MISSING | STILL MISSING (CRITICAL) |

## NEW Gaps Identified

### 13. BMC Crate Not Documented (LOW)
`projects/bmc/` exists but not in CLAUDE.md workspace table.

### 14. Graceful Shutdown Escalation (HIGH)
No `shutdown_with_timeout(Duration)` that does ACPI powerdown → wait → force kill. User must orchestrate manually.

### 15. QMP Out-of-Band Commands (MEDIUM)
`QapiCommand::ALLOW_OOB` exists. Negotiation doesn't enable OOB. Needed for non-blocking queries during migration.

### 16. QMP Command Timeout (CRITICAL)
`execute()` blocks indefinitely on hung QEMU. No per-command timeout.

### 17. Chardev / Serial Console (MEDIUM)
VNC console exists (BMC). No interactive serial console access.

### 18. QEMU Process Monitoring (CRITICAL)
No monitoring of child process exit. If QEMU crashes, state stays Running. Need task watching `child.wait()`.

### 19. Socket Permissions (MEDIUM)
QMP socket in `/tmp` with default perms. Any local user can connect.

### 20. Multi-disk Interface Support (LOW)
Only `virtio-blk-pci`. Need scsi, ide, sata for guest compatibility.

## Priority Summary

| Priority | Features |
|----------|----------|
| **CRITICAL** | Retry/reconnect (#12), QMP command timeout (#16), QEMU process death detection (#18) |
| **HIGH** | Graceful shutdown escalation (#14), Migration (#5), Snapshots (#6), Network (#3), DiskBuilder in manager (not BMC-only), Guest agent integration (#8), Promote wait_for_event to public API |
| **MEDIUM** | OOB commands (#15), Socket security (#19), Serial console (#17), SMP/NUMA/CPU (#9), Observability (#10), Typed device builders (#7) |
| **LOW** | Multi-disk interfaces (#20), BMC docs (#13), Connection pooling (#11) |

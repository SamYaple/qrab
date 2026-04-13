# Industry Standard Features Audit

Generated: 2026-04-13 | Re-audited: 2026-04-13 (x3)

Compared against: libvirt, proxmox-ve, cloud-hypervisor

## Prior Gaps Status

| # | Feature | Status |
|---|---------|--------|
| 1 | VM Lifecycle | IMPROVED — typestate + VmState enum. Missing graceful shutdown escalation |
| 2 | Storage Management | IMPROVED — BMC has attach_disks(). Not in manager crate. No hot-unplug |
| 3 | Network Management | STILL MISSING |
| 4 | Monitoring / Health | IMPROVED — typed events generated, broadcast channel. wait_for_event test-only |
| 5 | Migration | STILL MISSING |
| 6 | Snapshots | STILL MISSING |
| 7 | Device Management | PARTIAL — device_add used, no typed builders |
| 8 | Guest Agent | STILL PARTIAL — client works, no manager integration |
| 9 | Configuration | STILL MINIMAL — memory exists, no SMP/NUMA/CPU |
| 10 | Observability | STILL MINIMAL |
| 11 | Connection Pooling | N/A |
| 12 | Retry / Reconnect | STILL MISSING (CRITICAL) |
| 13 | BMC Docs | STILL MISSING |
| 14 | Graceful Shutdown | STILL MISSING |
| 15 | OOB Commands | STILL NOT NEGOTIATED |
| 16 | Command Timeout | STILL MISSING (CRITICAL) |
| 17 | Serial Console | STILL MISSING |
| 18 | Process Monitoring | STILL MISSING (CRITICAL) |
| 19 | Socket Permissions | STILL MISSING |
| 20 | Multi-disk Interfaces | STILL MISSING |

## NEW Gaps (Round 3)

| # | Feature | Priority |
|---|---------|----------|
| 21 | Hot-plug / Hot-unplug | HIGH |
| 22 | Resource Limits / cgroups | MEDIUM |
| 23 | UEFI Secure Boot / TPM | LOW |
| 24 | Cloud-Init / Config Drive | MEDIUM |
| 25 | SPICE/VNC Authentication | MEDIUM |
| 26 | QEMU Feature Detection (query-qmp-schema) | HIGH |
| 27 | Firmware / Machine Type Versioning | MEDIUM |
| 28 | Event-Driven State Machine | HIGH |
| 29 | Graceful QMP Disconnect Detection | HIGH |
| 30 | Multi-VM Support in Manager | MEDIUM |

## Priority Summary

| Priority | Features |
|----------|----------|
| **CRITICAL** | Retry/reconnect (#12), Command timeout (#16), Process death detection (#18) |
| **HIGH** | Shutdown escalation (#14), Migration (#5), Snapshots (#6), Network (#3), Hot-plug (#21), Feature detection (#26), Event-driven state (#28), Disconnect detection (#29) |
| **MEDIUM** | Storage in manager (#2), Device builders (#7), QGA integration (#8), SMP/NUMA (#9), Observability (#10), OOB (#15), Serial console (#17), Socket perms (#19), Resource limits (#22), Cloud-init (#24), VNC auth (#25), Machine versioning (#27), Multi-VM (#30) |
| **LOW** | BMC docs (#13), Multi-disk (#20), UEFI/TPM (#23) |

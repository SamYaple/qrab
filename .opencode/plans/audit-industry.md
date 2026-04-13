# Industry Standard Features Audit

Generated: 2026-04-13 | Round 4

Compared against: libvirt, proxmox-ve, cloud-hypervisor

## 40 Total Gaps Tracked

### CRITICAL
| # | Feature | Status |
|---|---------|--------|
| 12 | Retry/reconnect | STILL MISSING |
| 18 | Process death → state transition | STILL MISSING (logs but doesn't update state) |

### HIGH
| # | Feature | Status |
|---|---------|--------|
| 3 | Network management | STILL MISSING |
| 5 | Migration | STILL MISSING |
| 6 | Snapshots | STILL MISSING |
| 14 | Graceful shutdown escalation | STILL MISSING |
| 15 | OOB commands | STILL NOT NEGOTIATED |
| 21 | Hot-plug/unplug | STILL MISSING |
| 26 | Feature detection (query-qmp-schema) | STILL MISSING |
| 28 | Event-driven state machine | STILL MISSING (polls query-status) |
| 29 | QMP disconnect detection | STILL MISSING |
| 31 | Typed errors (no error enums) | NEW |
| 32 | OnceLock version = single-VM global | NEW |
| 34 | Connection timeout on QMP connect | NEW |
| 38 | No SMP configuration | NEW |
| 39 | No KVM/acceleration opt-in | NEW — **every VM runs TCG** |

### MEDIUM
| # | Feature | Status |
|---|---------|--------|
| 2 | Storage in manager (BMC-only) | IMPROVED |
| 7 | Device builders | PARTIAL |
| 8 | QGA integration | PARTIAL |
| 9 | SMP/NUMA/CPU config | STILL MISSING |
| 10 | Observability | IMPROVED (SSE events, serial tail) |
| 16 | Command timeout (configurable) | PARTIAL (30s hardcoded) |
| 17 | Serial console | STILL MISSING |
| 19 | Socket permissions | STILL MISSING |
| 22 | Resource limits/cgroups | STILL MISSING |
| 24 | Cloud-init/config drive | STILL MISSING |
| 25 | VNC auth | STILL MISSING |
| 27 | Machine type versioning | STILL MISSING |
| 30 | Multi-VM manager | STILL MISSING |
| 33 | QMP command pipelining | NEW |
| 35 | QGA thread safety | NEW |
| 37 | Temp file collision risk | NEW |
| 40 | BMC config not persistent | NEW |

### LOW
| # | Feature | Status |
|---|---------|--------|
| 13 | BMC docs | STILL MISSING |
| 20 | Multi-disk interfaces | STILL MISSING |
| 23 | UEFI Secure Boot/TPM | STILL MISSING |
| 36 | QEMU binary validation | NEW |

**Biggest blockers:** #39 (no KVM) + #38 (no SMP) = unusable for production. Fix before everything else.

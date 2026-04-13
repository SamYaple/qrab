# Industry Standard Features Audit

Generated: 2026-04-13 | Compared against: libvirt, proxmox-ve, cloud-hypervisor

## 1. VM Lifecycle — PARTIAL (CRITICAL)

Missing: graceful shutdown with timeout+fallback, state machine tracking, wait-for-event helpers.

## 2. Storage Management — MISSING (CRITICAL)

Raw QMP commands exist. Manager has zero storage abstractions. Need `DiskBuilder`, `vm.attach_disk()`.

## 3. Network Management — MISSING (HIGH)

Need `NetBuilder` wrapping `netdev_add` + `device_add` for tap/user/bridge.

## 4. Monitoring / Health — PARTIAL (CRITICAL)

Have raw event channel. Missing: typed event deser, filtering, health checks, resource queries.

## 5. Migration — MISSING (HIGH)

Need `MigrationBuilder` with progress polling, postcopy, cancel, event-driven completion.

## 6. Snapshots — MISSING (HIGH)

Need `vm.snapshot(name)` wrapping `savevm`. External disk-only via `transaction`. QGA freeze/thaw coordination.

## 7. Device Management — MISSING (MEDIUM)

Need typed device configs: VfioDevice, UsbDevice, VirtioDevice. `vm.add_device()` / `vm.remove_device()`.

## 8. Guest Agent — PARTIAL (HIGH)

Have Qga client + 44 commands. Missing: manager integration, file transfer helpers, freeze/thaw coordination.

## 9. Configuration — PARTIAL (MEDIUM)

Missing: SMP config, NUMA topology, CPU model selection, memory backend (hugepages, shared).

## 10. Observability — MINIMAL (MEDIUM)

Single `tracing::debug!`. Need spans for QMP commands, events, lifecycle, migration. Optional metrics.

## 11. Connection Pooling — MISSING (LOW)

Single-VM per Vm correct. Optional VmRegistry for orchestration.

## 12. Retry / Reconnect — MISSING (CRITICAL)

Connection drops = dead. Need `QmpReconnect` with exponential backoff, re-negotiate, health ping.

## Priority Summary

| Priority | Features |
|----------|----------|
| **CRITICAL** | Lifecycle state, Event typing, Storage, Retry/reconnect |
| **HIGH** | Migration, Snapshots, Network, Guest agent integration |
| **MEDIUM** | Device mgmt, Config (SMP/NUMA), Observability |
| **LOW** | Connection pooling |

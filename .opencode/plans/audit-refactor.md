# Refactoring Audit Report

Generated: 2026-04-13 | Round 11

**Non-generated source:** ~19,418+ lines / 77+ files

## 7 NEW opportunities

| # | Opportunity | Lines | Risk |
|---|---|---:|---|
| 63 | build_unified/qga_registry merge-loop dedup | 100 | moderate |
| 64 | process_single_version extraction dedup | 30 | safe |
| 65 | Serial tail logic duplicated (bmc) | 30 | safe |
| 66 | PreconfigVm/Vm accessor dedup | 25 | moderate |
| 67 | BMC state action pattern (pause/resume/reset/power_off) | 40 | safe |
| 68 | execute/execute_with_timeout request construction | 15 | safe |
| 69 | Process IR metadata init repeated 8+ times | 20 | safe |

## Summary

| Category | Count | Lines |
|----------|-------|-------|
| Total resolved | ~12 | ~175 |
| **Total open** | **58** | **~1,610** |

## Cluster Wins

| Cluster | Lines |
|---------|-------|
| Registry unification | ~675 |
| BMC server dedup | ~275 |
| Protocol dedup | ~220 |
| Macro serializer | ~185 |
| Version annotation | ~125 |
| Merge logic | ~80 |
| Manager typestate | ~25 |

**Top safe wins:** #59 (60 lines), #62 (45 lines), #67 (40 lines), #65 (30 lines), #64 (30 lines).

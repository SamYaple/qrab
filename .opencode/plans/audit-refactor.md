# Refactoring Audit Report

Generated: 2026-04-13 | Round 6

**Non-generated source:** ~31,070 lines / 77 files
**Largest:** unified.rs (1651), qapi-macros/lib.rs (1434), process_qapi.rs (936)

## All 38 prior opportunities STILL OPEN. Zero resolved.

## 7 NEW opportunities found.

| # | Opportunity | Lines | Risk |
|---|---|---:|---|
| 39 | has_feature generalization | 10 | safe |
| 40 | process_single_version QMP/QGA dedup | 25 | safe |
| 41 | condition_cfg/condition_cfg_attr dedup | 10 | safe |
| 42 | Ser wrapper type-gate dedup in macros | 15 | safe |
| 43 | Qmp::shutdown handle-abort pattern 3x | 20 | safe |
| 44 | strip_version_attrs inline reuse | 5 | safe |
| 45 | VmProcess file cleanup dedup | 15 | safe |

## Summary

| Category | Count | Lines |
|----------|-------|-------|
| Prior | 38 | ~985 |
| New | 7 | ~100 |
| **Total** | **45** | **~1,085** |

## Cluster Wins

| Cluster | Items | Lines |
|---------|-------|-------|
| Registry unification | #1,12,23,25,26,27,31,38,40 | ~405 |
| Merge logic | #4,13,29,37,39,44 | ~120 |
| Protocol dedup | #2,3,24,30,43 | ~115 |
| Macro serializer | #10,16,33,41,42 | ~95 |
| Version annotation | #9,17,18,19 | ~75 |
| Manager lifecycle | #14,45 | ~30 |

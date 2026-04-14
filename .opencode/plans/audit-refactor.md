# Refactoring Audit Report

Generated: 2026-04-13 | Round 7

**Non-generated source:** ~18,931 lines / 77 files
**Largest:** unified.rs (1644), qapi-macros/lib.rs (1434), process_qapi.rs (936)

## 8 opportunities RESOLVED this round

| Resolved | Detail |
|----------|--------|
| has_feature generalization | Extracted helper |
| Qmp::shutdown handle-abort 3x | Extracted handle-wait helper |
| strip_version_attrs inline reuse | Folded into helper |
| VmProcess file cleanup dedup | Extracted cleanup_temp_files |
| RefStrategy single-variant enum | Removed entirely |
| UnifiedType::rust_name() | Extracted helper |
| field deprecation warn | Extracted helper |
| Version type unification | Unified qga/qemu version returns |

## 5 NEW opportunities

| # | Opportunity | Lines | Risk |
|---|---|---:|---|
| 46 | build_unified/qga_registry dedup | 80 | medium |
| 47 | generate_unified/qga_code dedup | 60 | medium |
| 48 | Display impl 3-branch dedup in macros | 15 | safe |
| 49 | clean_doc consolidation (15 calls) | 20 | safe |
| 50 | Clone reduction in unified.rs (54 clones) | 25 | medium |

## Summary

| Category | Count | Lines |
|----------|-------|-------|
| Resolved | 8 | ~100 |
| Still open | 37 | ~885 |
| New | 5 | ~200 |
| **Total open** | **42** | **~1,085** |

## Cluster Wins

| Cluster | Items | Lines |
|---------|-------|-------|
| Registry unification | #1,12,23,25,26,27,31,38,40,46,47 | ~545 |
| Macro serializer | #10,16,33,41,42,48 | ~110 |
| Protocol dedup | #2,3,24,30 | ~95 |
| Merge logic | #4,13,29,37 | ~80 |
| Version annotation | #9,17,18,19 | ~75 |
| Codegen hygiene | 49,50 | ~45 |

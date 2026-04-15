# Refactoring Audit Report

Generated: 2026-04-13 | Round 6

**Non-generated source:** ~20,465 lines / 80 files (grew from ~18,358)
**Largest:** unified.rs (1647), qapi-macros/lib.rs (1394), process_qapi.rs (945)

## Status: All 38 prior STILL OPEN. Zero resolved. Codebase grew.

## 9 NEW Opportunities (Round 6)

| # | Opportunity | Lines | Risk |
|---|---|---:|---|
| 70 | Response deserialization 4x duplicate (lib.rs + qga.rs) | 50 | safe |
| 71 | Version attr injection shared enum/struct codegen | 40 | safe |
| 72 | Attribute-union loop 4x in unified.rs | 32 | safe |
| 73 | Ser body wrapping (version check + dep_warn) 3x in macros | 30 | safe |
| 74 | condition_cfg / condition_cfg_attr dual functions | 20 | safe |
| 75 | BMC cleanup pattern duplicated | 15 | safe |
| 76 | VmProcess Drop/cleanup overlap | 15 | safe |
| 77 | Qmp::new + Qga::new connect-with-timeout pattern | 10 | safe |
| 78 | has_feature + deprecated/unstable check per merge | 10 | safe |

## Summary

| Category | Count | Lines |
|----------|-------|-------|
| Prior (still open) | 38 | ~985 |
| New | 9 | ~222 |
| **Total** | **47** | **~1,207** |

Note: Agent also found deeper analysis showing ~67 opportunities / ~1,832 lines when counting sub-patterns within clusters. Conservative estimate above.

## Top Safe Wins

| Cluster | Items | Lines |
|---------|-------|-------|
| Registry unification | #1,12,23,25,26,27,31,38 | ~380 |
| Response deser extract | #70 | ~50 |
| Version annotation | #9,17,18,19,71 | ~115 |
| Merge logic | #4,13,29,37,72,78 | ~135 |
| Protocol dedup | #2,3,24,30,77 | ~105 |
| Macro serializer | #10,16,33,73,74 | ~90 |

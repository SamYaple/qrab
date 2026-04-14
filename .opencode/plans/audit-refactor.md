# Refactoring Audit Report

Generated: 2026-04-13 | Round 5

**Non-generated source:** ~18,358 lines / 77 files
**Largest:** unified.rs (1647), qapi-macros/lib.rs (1394), process_qapi.rs (945)

## All 30 prior opportunities STILL OPEN. Zero resolved.

## 8 NEW opportunities found.

| # | Opportunity | Lines | Risk | Status |
|---|---|---:|---|---|
| 1-30 | (see prior report) | ~815 | safe-moderate | OPEN |
| 31 | build_registry unification (QMP/QGA) | 80 | safe | NEW |
| 32 | format_rust_file → GeneratedOutput method | 10 | safe | NEW |
| 33 | version_range_check type-gate dedup in macros | 15 | safe | NEW |
| 34 | parse_version 3-way duplication across crates | 10 | safe | NEW |
| 35 | Qga::execute trivial wrapper | 5 | safe | NEW |
| 36 | Enum/Struct generate() meta preamble dedup | 20 | safe | NEW |
| 37 | merge_enum/merge_struct lifecycle markers | 20 | safe | NEW |
| 38 | seen_rust_names builtin set as const | 10 | safe | NEW |

## Summary

| Category | Count | Lines |
|----------|-------|-------|
| Prior | 30 | ~815 |
| New | 8 | ~170 |
| **Total** | **38** | **~985** |

## Cluster Wins

| Cluster | Items | Lines |
|---------|-------|-------|
| Registry unification | #1,12,23,25,26,27,31,38 | ~380 |
| Merge logic | #4,13,29,37 | ~105 |
| Protocol dedup | #2,3,24,30 | ~95 |
| Version annotation | #9,17,18,19 | ~75 |
| Macro serializer | #10,16,33 | ~70 |

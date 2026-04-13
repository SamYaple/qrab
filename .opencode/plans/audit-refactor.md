# Refactoring Audit Report

Generated: 2026-04-13 | Re-audited: 2026-04-13

**Non-generated source:** ~18,343 lines / 75 files
**Largest:** unified.rs (1631), integration.rs (1599), qapi-macros/lib.rs (1343), process_qapi.rs (945), vm.rs (440)

## Prior Opportunities Status

All 10 prior opportunities STILL APPLICABLE. Zero resolved.

| # | Opportunity | Lines | Risk | Status |
|---|---|---:|---|---|
| 1 | QMP/QGA codegen unification | 200 | safe | STILL OPEN |
| 2 | Session dedup | 40 | safe | STILL OPEN |
| 3 | Response parsing | 25 | safe | STILL OPEN |
| 4 | has_feature generic | 15 | safe | STILL OPEN |
| 5 | Remove RefStrategy | 10 | safe | STILL OPEN |
| 6 | Remove dead variant | 20 | safe | STILL OPEN |
| 7 | condition_cfg merge | 10 | safe | STILL OPEN |
| 8 | Ord dedup | 30 | safe | STILL OPEN |
| 9 | Version attr helper | 20 | safe | STILL OPEN |
| 10 | Ser overlap | 40 | moderate | STILL OPEN |

## NEW Opportunities

### 11. Sort key extraction — ~20 lines (safe)

`unified.rs` L990-999 and L1519-1528: identical sort closure for `UnifiedType` rust name. Extract `fn unified_type_rust_name()`.

### 12. Classify+partition codegen pattern — ~60 lines (safe)

`generate_unified_code` L1002-1056 and `generate_qga_code` L1531-1574: identical 2-pass pattern building command/event name sets. Sub-opportunity of #1.

### 13. attrs_equivalent union loop — ~30 lines (safe)

Same attribute-unioning loop in `merge_enum` L567-576, variant merge L604-613, `merge_struct` L720-730, field merge L761-769. Four copies. Extract `fn union_attrs(target, source)`.

### 14. VmProcess cleanup sync/async duplication — ~15 lines (safe)

`vm.rs` L37-50 (`cleanup()`) vs L54-65 (`Drop`): same file removal logic.

### 15. Accessor duplication — ~10 lines (safe)

`PreconfigVm::serial_log` L327-329 identical to `Vm::serial_log` L389-391. Same for `vnc_sock`. Could expose via trait.

### 16. Deprecation warning gen pattern — ~15 lines (safe)

`qapi-macros/lib.rs`: field-level deprecation warning at L957-965 duplicated at L1036-1044. Extract helper.

### 17. Nested attr retain — ~5 lines (safe)

`unified.rs` L1222 and L1302: identical `.retain()` for nested deprecated/unstable attrs. Fold into `strip_version_attrs`.

## Summary

| # | Opportunity | Lines | Risk | Status |
|---|---|---:|---|---|
| 1 | QMP/QGA codegen unification | 200 | safe | OPEN |
| 2 | Session dedup | 40 | safe | OPEN |
| 3 | Response parsing | 25 | safe | OPEN |
| 4 | has_feature generic | 15 | safe | OPEN |
| 5 | Remove RefStrategy | 10 | safe | OPEN |
| 6 | Remove dead variant | 20 | safe | OPEN |
| 7 | condition_cfg merge | 10 | safe | OPEN |
| 8 | Ord dedup | 30 | safe | OPEN |
| 9 | Version attr helper | 20 | safe | OPEN |
| 10 | Ser overlap | 40 | moderate | OPEN |
| 11 | Sort key extraction | 20 | safe | NEW |
| 12 | Classify+partition | 60 | safe | NEW |
| 13 | attrs union loop | 30 | safe | NEW |
| 14 | VmProcess cleanup | 15 | safe | NEW |
| 15 | Accessor dedup | 10 | safe | NEW |
| 16 | Deprecation warn | 15 | safe | NEW |
| 17 | Nested attr retain | 5 | safe | NEW |
| | **Total** | **~565** | | |

# Refactoring Audit Report

Generated: 2026-04-13

**Non-generated source:** ~18,343 lines / 75 files
**Largest:** unified.rs (1631), integration.rs (1599), qapi-macros/lib.rs (1343), process_qapi.rs (945)

## 1. QMP/QGA codegen duplication — ~200 lines (safe)

`unified.rs`: `generate_unified_code` / `generate_qga_code` 90% identical. Same for `build_*_registry` and `process_single_version*`.

**Approach:** Extract shared functions. Pass `SchemaKind` enum.

## 2. QmpSession/QgaSession dedup — ~40 lines (safe)

`server.rs`: `recv_command`, `send_return`, `send_error` identical between both.

**Approach:** Extract `JsonRpcSession` base struct.

## 3. Response parsing dedup — ~25 lines (safe)

`lib.rs` + `qga.rs`: empty-object-to-null logic duplicated.

**Approach:** `fn parse_qmp_response<T: DeserializeOwned>(resp: Value) -> Result<T>`

## 4. has_deprecated/unstable → has_feature — ~15 lines (safe)

**Approach:** `fn has_feature(meta: &Metadata, name: &str) -> bool`

## 5. RefStrategy single-variant enum — ~10 lines (safe)

Dead abstraction. Remove enum, pass HashMap directly.

## 6. EnumVariantKind::Struct dead variant — ~20 lines (safe)

Never constructed. Remove variant + match arm.

## 7. condition_cfg duplication — ~10 lines (safe)

**Approach:** `condition_cfg_attr` calls `condition_cfg` internally.

## 8. Ord/PartialOrd boilerplate — ~30 lines (safe)

Four identical impls delegating to `name.cmp()`. Reorder fields + derive, or macro.

## 9. Version attr stripping repeated — ~20 lines (safe)

Same `.retain()` filter 4+ times.

**Approach:** `fn strip_version_attrs(attrs: &mut Vec<Attribute>)`

## 10. Ser codegen overlap — ~40 lines (moderate)

`gen_struct_ser` / `gen_struct_ser_flatten` share boilerplate.

**Approach:** Extract `wrap_ser_body()`.

## Summary

| # | Opportunity | Lines | Risk |
|---|---|---:|---|
| 1 | QMP/QGA codegen unification | 200 | safe |
| 2 | Session dedup | 40 | safe |
| 3 | Response parsing | 25 | safe |
| 4 | has_feature generic | 15 | safe |
| 5 | Remove RefStrategy | 10 | safe |
| 6 | Remove dead variant | 20 | safe |
| 7 | condition_cfg merge | 10 | safe |
| 8 | Ord dedup | 30 | safe |
| 9 | Version attr helper | 20 | safe |
| 10 | Ser overlap | 40 | moderate |
| | **Total** | **~410** | |

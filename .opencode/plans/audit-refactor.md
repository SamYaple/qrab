# Refactoring Audit Report

Generated: 2026-04-13 | Round 4

**Non-generated source:** ~18,358 lines / 77 files
**Largest:** unified.rs (1647), qapi-macros/lib.rs (1394), process_qapi.rs (945)

## All 21 prior opportunities STILL OPEN. Zero resolved.

## 9 NEW opportunities found.

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
| 11 | Sort key extraction | 20 | safe | OPEN |
| 12 | Classify+partition | 60 | safe | OPEN |
| 13 | attrs union loop | 30 | safe | OPEN |
| 14 | VmProcess cleanup | 15 | safe | OPEN |
| 15 | Accessor dedup | 10 | safe | OPEN |
| 16 | Deprecation warn gen | 15 | safe | OPEN |
| 17 | Nested attr retain | 5 | safe | OPEN |
| 18 | Version-annotation boilerplate | 40 | safe | OPEN |
| 19 | Clean-doc loop dedup | 10 | safe | OPEN |
| 20 | BMC accessor match dedup | 10 | safe | OPEN |
| 21 | condition_cfg micro-pattern | 10 | safe | OPEN |
| 22 | UnifiedType::rust_name() method | 30 | safe | NEW |
| 23 | process_single_version unification | 35 | safe | NEW |
| 24 | Shutdown handle timeout+abort dedup | 20 | safe | NEW |
| 25 | "mark unseen as removed" loop dedup | 15 | safe | NEW |
| 26 | Thread-scope panic-handling dedup | 10 | safe | NEW |
| 27 | Schema hash computation dedup | 10 | safe | NEW |
| 28 | Tracked attr name matching | 10 | safe | NEW |
| 29 | Variant/field rename detection | 40 | moderate | NEW |
| 30 | Drop cleanup pattern | 10 | safe | NEW |

## Summary

| Category | Count | Lines |
|----------|-------|-------|
| Prior | 21 | ~635 |
| New | 9 | ~180 |
| **Total** | **30** | **~815** |

**Cluster wins:** #1+#12+#23+#8 = ~315 lines (QMP/QGA unification). #29+#13+#25 = ~85 lines (merge logic). #9+#18+#19 = ~70 lines (version annotation).

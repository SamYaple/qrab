# Refactoring Audit Report

Generated: 2026-04-13 | Re-audited: 2026-04-13 (x3)

**Non-generated source:** ~18,358 lines / 77 files
**Largest:** unified.rs (1629), integration.rs (1599), qapi-macros/lib.rs (1343), process_qapi.rs (945), bmc/state.rs (499), vm.rs (443)

## All 17 prior opportunities STILL OPEN. Zero resolved.

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

## NEW Opportunities (Round 3)

| # | Opportunity | Lines | Risk |
|---|---|---:|---|
| 18 | Version-annotation boilerplate (enum/struct generators) | 40 | safe |
| 19 | Clean-doc loop dedup (6 copies in unified.rs) | 10 | safe |
| 20 | BMC accessor match dedup (vnc_sock/serial_log) | 10 | safe |
| 21 | condition_cfg micro-pattern (13 occurrences) | 10 | safe |

## Summary

| Category | Count | Lines |
|----------|-------|-------|
| Prior (still open) | 17 | ~565 |
| New | 4 | ~70 |
| **Total** | **21** | **~635** |

**Biggest wins:** #1 (200 lines), #12 (60 lines), #10+#18 (80 lines combined).

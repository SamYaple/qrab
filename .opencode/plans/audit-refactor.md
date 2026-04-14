# Refactoring Audit Report

Generated: 2026-04-13 | Round 8

**Non-generated source:** ~19,129 lines / 77 files

## 2 more resolved this round (10 total)

| Resolved | Detail |
|----------|--------|
| Display impl 3-branch dedup (#48) | `gen_display_impl` helper extracted |
| clean_doc consolidation (#49) | `clean_doc_in_place` wrapper, 15 call sites unified |

## 4 NEW opportunities (BMC crate)

| # | Opportunity | Lines | Risk |
|---|---|---:|---|
| 51 | vm_actions boilerplate macro (7 identical server fns) | 50 | safe |
| 52 | ServerFnError map_err trait (15 instances) | 15 | safe |
| 53 | VmState match dedup (9 match arms across 7 query fns) | 40 | medium |
| 54 | bmc() singleton + vm lock pattern | 20 | medium |

## Summary

| Category | Count | Lines |
|----------|-------|-------|
| Total resolved | 10 | ~135 |
| Still open | 40 | ~1,050 |
| New (BMC) | 4 | ~125 |
| **Total open** | **44** | **~1,175** |

## Cluster Wins

| Cluster | Lines |
|---------|-------|
| Registry unification | ~545 |
| BMC server dedup (NEW) | ~125 |
| Macro serializer | ~95 |
| Protocol dedup | ~95 |
| Merge logic | ~80 |
| Version annotation | ~75 |

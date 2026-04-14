# Refactoring Audit Report

Generated: 2026-04-13 | Round 9

**Non-generated source:** ~19,418 lines / 77 files

## Changes This Round
- Clone reduction in unified.rs PARTIALLY RESOLVED (~30 lines)
- Protocol dedup cluster GREW: 95→165 lines (3 execute variants copy-paste)

## 3 NEW opportunities

| # | Opportunity | Lines | Risk |
|---|---|---:|---|
| 55 | QMP response helper (error-check + return-extraction 3x) | 50 | safe |
| 56 | QMP request-send helper (args→JSON + channel send 3x) | 40 | safe |
| 57 | "command channel closed" error constant (6x) | 10 | safe |

## Summary

| Category | Count | Lines |
|----------|-------|-------|
| Total resolved | ~11 | ~165 |
| **Total open** | **46** | **~1,120** |

## Cluster Wins

| Cluster | Lines | Trend |
|---------|-------|-------|
| Registry unification | ~545 | — |
| **Protocol dedup** | **~165** | **↑** |
| BMC server dedup | ~125 | — |
| Macro serializer | ~95 | — |
| Merge logic | ~80 | — |
| Version annotation | ~75 | — |

**#55+#56 = ~90 lines safe refactor = highest-value next targets.**

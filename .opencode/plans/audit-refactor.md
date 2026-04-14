# Refactoring Audit Report

Generated: 2026-04-13 | Round 10

**Non-generated source:** ~19,418 lines / 77 files

## Changes: Protocol dedup grew to ~175 lines. 5 new opportunities.

## 5 NEW opportunities

| # | Opportunity | Lines | Risk |
|---|---|---:|---|
| 58 | write_json_line helper (5 sites in server.rs) | 20 | safe |
| 59 | QmpSession/QgaSession identical methods (recv/send/error) | 60 | safe |
| 60 | Version-attr emission helper (enum/struct generators) | 25 | safe |
| 61 | Response-parsing dedup across QMP+QGA (4 sites) | 70 | moderate |
| 62 | Serialize body version-check wrapper (3 fns) | 45 | safe |

## Summary

| Category | Count | Lines |
|----------|-------|-------|
| Total resolved | ~11 | ~165 |
| **Total open** | **51** | **~1,350** |

## Cluster Wins

| Cluster | Lines | Trend |
|---------|-------|-------|
| Registry unification | ~545 | — |
| BMC server dedup | ~205 | ↑↑ |
| Protocol dedup | ~175 | ↑ |
| Macro serializer | ~140 | ↑ |
| Version annotation | ~100 | ↑ |
| Merge logic | ~80 | — |

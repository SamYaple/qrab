# Best Practices Audit Report

Generated: 2026-04-13 | Round 7

## Changes This Round
- No new fixes detected. All prior issues confirmed still present.
- N31 (edition still 2021) and N32 (no workspace.lints) — confirmed, overlap with existing #6 and #7.

## HIGH (unchanged)
| # | Issue | Status |
|---|-------|--------|
| 1 | No thiserror — anyhow in public APIs | STILL PRESENT |
| 3 | syn v1 in qapi-macros | STILL PRESENT |
| 4 | No CI pipeline | STILL PRESENT |
| N2 | No `#[must_use]` anywhere | STILL PRESENT |
| N3 | Duplicated execute logic | STILL PRESENT |
| N23 | No error taxonomy | STILL PRESENT |

## MEDIUM (key items)
- #5 No workspace.dependencies — STILL PRESENT
- #8 Qga no Debug — STILL PRESENT
- #11 Public fields bypass typestate — STILL PRESENT
- #13/N28 No QMP connect timeout — PARTIAL (greeting has 10s timeout, connect itself unguarded)
- N29 pub use serde leaks impl deps — STILL PRESENT
- N30 No `#[non_exhaustive]` — STILL PRESENT

## Top 3 Actions (unchanged)
1. **Replace anyhow with thiserror** in library public APIs
2. **Add CI** — clippy + fmt + test + deny
3. **Add `[workspace.dependencies]` + `[workspace.lints]` + edition 2024**

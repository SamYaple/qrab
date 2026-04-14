# Best Practices Audit Report

Generated: 2026-04-13 | Round 9

## Changes This Round
- N33 edition mismatch WORSENED — 5/6 crates still 2021 with resolver=3
- NEW: N34 (bmc not documented), N35 (no deny(unsafe_code)), N36 (no workspace.package.edition)

## HIGH (unchanged)
| # | Issue |
|---|-------|
| 1 | No thiserror — anyhow in all 6 public APIs |
| 3 | syn v1 in qapi-macros |
| 4 | No CI pipeline |
| N2 | No `#[must_use]` |
| N3 | Duplicated execute logic |
| N23 | No error taxonomy |

## MEDIUM
- #5 No workspace.dependencies (6 crates)
- #8 Qga no Debug
- #11 Public fields bypass typestate
- N29 pub use serde leaks impl deps
- N30 No `#[non_exhaustive]`
- N33 Edition mismatch (worsened)
- N35 No deny(unsafe_code) (NEW)
- N36 No workspace.package.edition (NEW)

## LOW
- N34 bmc not in CLAUDE.md (NEW)

## Top 3 Actions
1. Replace anyhow with thiserror in library APIs
2. Add CI + workspace.lints
3. Add workspace.dependencies + workspace.package.edition = "2024"

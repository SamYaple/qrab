# Best Practices Audit Report

Generated: 2026-04-13 | Round 8

## Changes This Round
- N33 (edition mismatch) — NEW: codegen=2024, all others=2021. Should unify via workspace.package.edition.

## HIGH (unchanged)
| # | Issue |
|---|-------|
| 1 | No thiserror — anyhow in public APIs |
| 3 | syn v1 in qapi-macros |
| 4 | No CI pipeline |
| N2 | No `#[must_use]` |
| N3 | Duplicated execute logic |
| N23 | No error taxonomy |

## MEDIUM (key items, unchanged)
- #5 No workspace.dependencies
- #8 Qga no Debug
- #11 Public fields bypass typestate
- N29 pub use serde leaks impl deps
- N30 No `#[non_exhaustive]`
- N33 Edition mismatch (NEW)

## Top 3 Actions
1. Replace anyhow with thiserror in library APIs
2. Add CI — clippy + fmt + test + deny
3. Add workspace.dependencies + workspace.lints + unify edition 2024

# Best Practices Audit Report

Generated: 2026-04-13 | Round 10

## Changes: N2 (#[must_use]) IMPROVED (manager has ~10). N37 (pub use serde_json leaks) NEW.

## HIGH
| # | Issue |
|---|-------|
| 1 | No thiserror — anyhow in 4 crates |
| 3 | syn v1 in qapi-macros |
| 4 | No CI pipeline |
| N3 | Duplicated execute logic |
| N23 | No error taxonomy |

## MEDIUM
- #5 No workspace.dependencies
- #8 Qga no Debug
- #11 Public fields bypass typestate
- N29 pub use serde leaks impl dep
- N30 No `#[non_exhaustive]`
- N33 Edition mismatch (5/6 still 2021)
- N35 No deny(unsafe_code)
- N36 No workspace.package.edition
- N37 pub use serde_json also leaks (NEW)

## LOW
- N2 `#[must_use]` partial (manager only)
- N34 bmc not in CLAUDE.md

## Top 3 Actions
1. Replace anyhow with thiserror in library APIs
2. Add CI + workspace.lints
3. workspace.dependencies + workspace.package.edition = "2024"

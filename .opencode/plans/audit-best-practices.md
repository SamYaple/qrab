# Best Practices Audit Report

Generated: 2026-04-13 | Round 6

## Changes This Round
- **N1 (unwrap in library code) → FIXED** — zero `.unwrap()` in protocol/manager/qapi
- **N27 (QGA sync no timeout) → FIXED** — 10s timeout wraps entire handshake
- **#13 (connection timeout) → PARTIAL** — QGA has timeouts, QMP connect still unguarded

## HIGH
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
- #13 No QMP connect timeout — STILL PRESENT (QGA fixed)
- N28 No connect timeout on QMP — NEW
- N29 pub use serde leaks impl deps — NEW
- N30 No `#[non_exhaustive]` — NEW

## Top 3 Actions
1. **Add CI** — clippy + fmt + test + deny
2. **Replace anyhow with thiserror** in library public APIs
3. **Add `[workspace.dependencies]` + `[workspace.lints]` + edition 2024**

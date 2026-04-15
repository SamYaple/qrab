# Best Practices Audit Report

Generated: 2026-04-13 | Round 6

## Changes This Round

| # | Issue | Change |
|---|-------|--------|
| N20 | stderr piped to null | FIXED (confirmed r5) |
| N2 | No `#[must_use]` | REGRESSION — 18 annotations in manager, but zero in qapi-protocol/qapi (was 33 prior) |
| N33 | Edition inconsistency | PARTIAL FIX — codegen 2024, root resolver 3, but 7 crates still 2021 |

## NEW Issues (Round 6)

| # | Severity | Issue |
|---|----------|-------|
| N42 | HIGH | bmc crate uses anyhow — now 5 library crates without thiserror |
| N43 | MEDIUM | No workspace.dependencies — duplicate dep versions across 7 crates |
| N44 | MEDIUM | No `#![deny(unsafe_code)]` in crates that don't need unsafe |
| N45 | LOW | qrab-init uses `Result<(), String>` instead of proper error type |
| N46 | LOW | bmc pulls dioxus unconditionally (not feature-gated) |

## Top 3 Actions
1. **Add `workspace.dependencies`** — centralizes all dep versions, fixes N43
2. **Replace anyhow with thiserror** in library crates — fixes #1, N23, N42
3. **Add SAFETY comments to qrab-init unsafe blocks + deny(unsafe_code) elsewhere** — fixes N38, N44

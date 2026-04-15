# Best Practices Audit Report

Generated: 2026-04-13 | Round 11

## Changes
- **N3 (duplicated execute logic) → FIXED**
- **N2 (#[must_use]) → IMPROVED** (33 annotations, protocol + manager covered)
- **N33 (edition mismatch) → WORSE** (6/7 on 2021, qrab-init added)

## NEW Issues
| # | Severity | Issue |
|---|----------|-------|
| N38 | HIGH | unsafe blocks in namespace.rs (10+) + qrab-init — no `// SAFETY:` comments |
| N39 | MEDIUM | qrab-init missing from CLAUDE.md workspace table |
| N40 | MEDIUM | namespace.rs 235-line god-module (fd plumbing + mount + fork) |
| N41 | LOW | Hand-rolled unsafe fd conversion — should use tokio::fs::File::from_std |

## HIGH
| # | Issue |
|---|-------|
| 1 | No thiserror — anyhow in 4 crates |
| 3 | syn v1 in qapi-macros |
| 4 | No CI pipeline |
| N23 | No error taxonomy |
| N38 | Unsafe without safety docs (NEW) |

## Top 3 Actions
1. **Add `// SAFETY:` comments + `#![deny(unsafe_code)]` in non-namespace crates**
2. **Replace anyhow with thiserror** in library APIs
3. **Add CI + workspace.dependencies + edition 2024**

# Best Practices Audit Report

Generated: 2026-04-13 | Re-audited: 2026-04-13 (x3)

## HIGH
| # | Issue | Status |
|---|-------|--------|
| 1 | No thiserror — anyhow in public APIs | STILL PRESENT |
| 2 | Cancellation safety | CONFIRMED NOT A BUG |
| 3 | syn v1 in qapi-macros | STILL PRESENT |
| 4 | No CI pipeline | STILL PRESENT |
| N1 | unwrap in library code (qga.rs:45) | STILL PRESENT |
| N2 | No `#[must_use]` anywhere | STILL PRESENT |
| N3 | Duplicated execute logic (Qmp/Qga) | STILL PRESENT |

## MEDIUM
| # | Issue | Status |
|---|-------|--------|
| 5 | No `[workspace.dependencies]` | STILL PRESENT (anyhow version inconsistent: "1.0.102" vs "1") |
| 6 | Inconsistent editions (codegen=2024, rest=2021) | STILL PRESENT |
| 7 | No `[workspace.lints]` | STILL PRESENT |
| 8 | Missing C-COMMON traits (Qga no Debug) | STILL PRESENT |
| 9 | Version = raw tuple | STILL PRESENT |
| 10 | No Drop on Qmp | STILL PRESENT |
| 11 | Public fields bypass typestate | STILL PRESENT |
| 12 | No unit tests in library crates (only integration) | PARTIALLY FIXED |
| 13 | No connection timeout | STILL PRESENT |
| 14 | No cargo-deny | STILL PRESENT |
| N5 | No Send/Sync assertions | STILL PRESENT |
| N10 | No Drop on Qga | NEW |
| N11 | Event channel backpressure (bounded 100, silent drop) | NEW |
| N12 | Blocking `exists()` in async context (vm.rs:266) | NEW |
| N13 | anyhow version inconsistency across crates | NEW |

## LOW
| # | Issue | Status |
|---|-------|--------|
| 15 | Silent `.ok()` on OnceLock | STILL PRESENT |
| 16 | Tracing inconsistent (35 eprintln in codegen) | STILL PRESENT |
| 17 | No property-based tests | STILL PRESENT |
| N4 | `pub use generated::*` glob re-export | STILL PRESENT |
| N7 | Dead code allows in generated code | DOWNGRADED to LOW |
| N8 | No Debug on Qga | STILL PRESENT |
| N9 | No doc-tests | STILL PRESENT |
| N14 | BMC crate not in CLAUDE.md | STILL PRESENT |

## Top 3 Actions
1. **Add CI** — catches half these automatically
2. **Replace anyhow with thiserror** in library crate public APIs
3. **Upgrade qapi-macros to syn 2**

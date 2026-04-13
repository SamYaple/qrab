# Best Practices Audit Report

Generated: 2026-04-13 | Round 4

## HIGH
| # | Issue | Status |
|---|-------|--------|
| 1 | No thiserror — anyhow in public APIs | STILL PRESENT |
| 3 | syn v1 in qapi-macros | STILL PRESENT |
| 4 | No CI pipeline | STILL PRESENT |
| N1 | unwrap in library code | PARTIALLY FIXED (qga.rs:45 fixed, 4 guarded unwraps remain) |
| N2 | No `#[must_use]` anywhere | STILL PRESENT |
| N3 | Duplicated execute logic (Qmp/Qga) | STILL PRESENT |

## MEDIUM
| # | Issue | Status |
|---|-------|--------|
| 5 | No `[workspace.dependencies]` | STILL PRESENT |
| 6 | Inconsistent editions | STILL PRESENT |
| 7 | No `[workspace.lints]` | STILL PRESENT |
| 8 | Missing C-COMMON traits (Qga no Debug) | STILL PRESENT |
| 9 | Version = raw tuple | STILL PRESENT |
| 10 | No Drop on Qmp | **FIXED** |
| 11 | Public fields bypass typestate | STILL PRESENT |
| 12 | No unit tests in library crates | IMPROVED (codegen 22 modules, but protocol/manager zero unit tests) |
| 13 | No connection timeout | STILL PRESENT |
| 14 | No cargo-deny | STILL PRESENT |
| N5 | No Send/Sync assertions | STILL PRESENT |
| N10 | No Drop on Qga | STILL PRESENT |
| N11 | Event channel backpressure | Events silently lost when channel full |
| N12 | Blocking `exists()` in async | STILL PRESENT |
| N15 | resolver = "3" with edition 2021 | NEW |
| N16 | bmc pulls dioxus unconditionally | NEW |
| N17 | Qga has no event handling | NEW |
| N18 | No `#[non_exhaustive]` on public types | NEW |
| N19 | `kill_on_drop` but no graceful QEMU quit | NEW |
| N20 | stderr piped to null | NEW |

## LOW
| # | Issue | Status |
|---|-------|--------|
| 15 | Silent `.ok()` on OnceLock | STILL PRESENT |
| 16 | Tracing inconsistent (25 eprintln in codegen) | STILL PRESENT |
| 17 | No property-based tests | STILL PRESENT |
| N4 | `pub use generated::*` glob | STILL PRESENT |
| N8 | No Debug on Qga | STILL PRESENT |
| N9 | No doc-tests | STILL PRESENT |
| N14 | BMC crate not in CLAUDE.md | STILL PRESENT |
| N21 | No Cargo.toml publish metadata | NEW |
| N22 | VmBuilder fields not validated | NEW |

## Top 3 Actions
1. **Add CI** — clippy + fmt + test + deny
2. **Replace anyhow with thiserror** in library public APIs
3. **Upgrade qapi-macros to syn 2**

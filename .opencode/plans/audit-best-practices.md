# Best Practices Audit Report

Generated: 2026-04-13 | Round 5

## HIGH
| # | Issue | Status |
|---|-------|--------|
| 1 | No thiserror — anyhow in public APIs | STILL PRESENT |
| 3 | syn v1 in qapi-macros | STILL PRESENT |
| 4 | No CI pipeline | STILL PRESENT |
| N1 | unwrap in library code (4 guarded remain) | STILL PRESENT |
| N2 | No `#[must_use]` anywhere | STILL PRESENT |
| N3 | Duplicated execute logic (Qmp/Qga) | STILL PRESENT |
| N23 | No error type taxonomy — all stringly-typed | NEW |

## MEDIUM
| # | Issue | Status |
|---|-------|--------|
| 5 | No `[workspace.dependencies]` | STILL PRESENT |
| 6 | Inconsistent editions | STILL PRESENT |
| 7 | No `[workspace.lints]` | STILL PRESENT |
| 8 | Missing C-COMMON traits (Qga no Debug) | STILL PRESENT |
| 9 | Version = raw tuple | STILL PRESENT |
| 10 | No Drop on Qmp | FIXED |
| 11 | Public fields bypass typestate | STILL PRESENT |
| 12 | No unit tests in library crates | STILL PRESENT |
| 13 | No connection timeout | STILL PRESENT |
| 14 | No cargo-deny | STILL PRESENT |
| N20 | stderr piped to null | **FIXED** (now piped to tracing) |
| N24 | qga_version returns String, qemu_version returns tuple | NEW |
| N25 | No Clone on Qmp | NEW |
| N26 | SOCKET_COUNTER global prevents deterministic testing | NEW |
| N27 | Qga sync handshake no timeout | NEW |

## LOW
| # | Issue | Status |
|---|-------|--------|
| 15 | Silent `.ok()` on OnceLock | STILL PRESENT |
| 16 | Tracing inconsistent (eprintln in codegen) | STILL PRESENT |
| 17 | No property-based tests | STILL PRESENT |
| N4 | `pub use generated::*` glob | STILL PRESENT |
| N8 | No Debug on Qga | STILL PRESENT |
| N9 | No doc-tests | STILL PRESENT |
| N14 | BMC crate not in CLAUDE.md | STILL PRESENT |

## Top 3 Actions
1. **Add CI** — clippy + fmt + test + deny
2. **Replace anyhow with thiserror** in library public APIs
3. **Add `[workspace.dependencies]` + `[workspace.lints]`**

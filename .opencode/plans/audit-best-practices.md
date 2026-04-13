# Best Practices Audit Report

Generated: 2026-04-13 | Re-audited: 2026-04-13

## Prior Issues Status

### HIGH
| # | Issue | Status |
|---|-------|--------|
| 1 | No `thiserror` — anyhow in public APIs | STILL PRESENT |
| 2 | Cancellation safety in `tokio::select!` | **CORRECTED: NOT A BUG** — `Lines::next_line()` is cancellation-safe per tokio docs |
| 3 | syn v1 unmaintained in qapi-macros | STILL PRESENT |
| 4 | No CI pipeline | STILL PRESENT |

### MEDIUM
| # | Issue | Status |
|---|-------|--------|
| 5 | No `[workspace.dependencies]` | STILL PRESENT |
| 6 | Inconsistent editions (codegen=2024, rest=2021) | STILL PRESENT |
| 7 | No `[workspace.lints]` | STILL PRESENT |
| 8 | Missing C-COMMON traits | STILL PRESENT |
| 9 | Version = raw tuple | STILL PRESENT |
| 10 | No `Drop` on `Qmp` | STILL PRESENT |
| 11 | Public fields bypass typestate | STILL PRESENT |
| 12 | No unit tests in library crates | PARTIALLY FIXED (codegen has 24 test modules; protocol/manager still zero) |
| 13 | No connection timeout | STILL PRESENT |
| 14 | No cargo-deny | STILL PRESENT |

### LOW
| # | Issue | Status |
|---|-------|--------|
| 15 | Silent `.ok()` on OnceLock | STILL PRESENT |
| 16 | Tracing inconsistent | STILL PRESENT (codegen: 29 eprintln! calls) |
| 17 | No property-based tests | STILL PRESENT |

## NEW Issues Found

### HIGH
- **N1: `unwrap()` in library code** — `qga.rs:45` `.unwrap()` on `duration_since(UNIX_EPOCH)` panics if clock before epoch
- **N2: No `#[must_use]` anywhere** — `qemu_version()`, `execute()` return values silently discardable

### MEDIUM
- **N3: Duplicated execute logic** — `Qmp::execute()` and `Qga::execute_raw()` nearly identical JSON-RPC handling
- **N4: `pub use generated::*` glob re-export** — dumps ~500+ types into crate root namespace
- **N5: No `Send`/`Sync` assertions** — async types need compile-time verification
- **N6: `resolver = "3"` but edition 2021** — inconsistent
- **N7: Dead code allows in generated code too broad** — could mask real issues

### LOW
- **N8: No `Debug` on `Qga`** — `Qmp` has Debug derive, `Qga` doesn't
- **N9: No doc-tests** — zero runnable examples in rustdoc

## Priority Summary

| Priority | Issues |
|----------|--------|
| **High** | anyhow public APIs, syn v1, no CI, unwrap in lib, no must_use |
| **Medium** | workspace.deps, lints, traits, version type, Drop, typestate bypass, timeout, cargo-deny, execute dedup, glob re-export, Send/Sync |
| **Low** | OnceLock, tracing, property tests, Qga Debug, doc-tests |

**Top 3 actions:** Add CI (catches half these), replace anyhow with thiserror, upgrade syn v1→v2.

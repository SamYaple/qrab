# Best Practices Audit Report

Generated: 2026-04-13

## 1. Workspace Structure

- **No `[workspace.dependencies]`** (Medium) — Duplicated dep versions across crates
- **Inconsistent editions** (Low) — codegen=2024, rest=2021
- **No `[workspace.lints]`** (Medium) — No clippy/lint enforcement

## 2. Error Handling

- **No `thiserror`** (High) — Library crates expose `anyhow::Error` in public APIs
- **Silent `.ok()` on OnceLock** (Low) — `set_qemu_version` drops errors silently

## 3. API Design

- **Missing C-COMMON traits** (Medium) — QmpEvent missing PartialEq/Eq, Timestamp missing Ord/Hash
- **Version = raw tuple** (Medium) — Should be `QemuVersion` struct
- **No `Drop` on `Qmp`** (Medium) — Background tasks leak
- **Public fields bypass typestate** (Low) — `pub qmp` on PreconfigVm/Vm

## 4. CI/CD

- **No CI pipeline** (High) — No clippy, fmt, or test automation

## 5. Tracing

- **Inconsistent** (Low) — manager uses tracing, protocol has none, codegen uses eprintln

## 6. Testing

- **No unit tests in library crates** (Medium) — Only codegen has #[cfg(test)]
- **No property-based tests** (Low)

## 7. Async

- **Cancellation safety** (High) — `Lines::next_line()` not cancellation-safe in `tokio::select!`
- **No connection timeout** (Medium) — `Qmp::new()` blocks forever if QEMU hangs

## 8. Dependencies

- **No cargo-deny** (Medium) — No license/advisory auditing
- **syn v1 unmaintained** (High) — qapi-macros still on syn 1

## Priority Summary

| Priority | Issues |
|----------|--------|
| **High** | No typed errors, cancellation safety, syn v1, no CI |
| **Medium** | workspace.dependencies, lints, traits, version type, Drop, unit tests, timeout, cargo-deny |
| **Low** | Editions, OnceLock, public fields, tracing, property tests |

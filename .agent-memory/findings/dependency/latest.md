[0m
> build · claude-sonnet-4-6
[0m
[0m# [0mTodos
[ ] Run cargo audit for security advisories
[ ] Run cargo update --dry-run for available updates
[ ] Run cargo tree and cargo tree --duplicates
[ ] Read Cargo.toml files for manual analysis
[ ] Compile and output findings report
[0m
[0m→ [0mRead ../../Cargo.toml
[0m
[0m$ [0mcd /home/bob/workspace/qrab && cargo update --dry-run 2>&1
Updating crates.io index
     Locking 6 packages to latest Rust 1.92.0 compatible versions
    Updating fastrand v2.3.0 -> v2.4.1
    Updating hashbrown v0.16.1 -> v0.17.0
    Updating indexmap v2.13.1 -> v2.14.0
    Updating libredox v0.1.15 -> v0.1.16
    Updating redox_syscall v0.7.3 -> v0.7.4
    Updating tokio v1.51.0 -> v1.51.1
note: pass `--verbose` to see 3 unchanged dependencies behind latest
warning: not updating lockfile due to dry run
[0m
[0m$ [0mcd /home/bob/workspace/qrab && cargo audit 2>&1
[0m[0m[1m[32m    Fetching[0m advisory database from `https://github.com/RustSec/advisory-db.git`
[0m[0m[1m[32m      Loaded[0m 1042 security advisories (from /home/bob/.cargo/advisory-db)
[0m[0m[1m[32m    Updating[0m crates.io index
[0m[0m[1m[32m    Scanning[0m Cargo.lock for vulnerabilities (79 crate dependencies)
[0m[0m[1m[33mCrate:    [0m paste
[0m[0m[1m[33mVersion:  [0m 1.0.15
[0m[0m[1m[33mWarning:  [0m unmaintained
[0m[0m[1m[33mTitle:    [0m paste - no longer maintained
[0m[0m[1m[33mDate:     [0m 2024-10-07
[0m[0m[1m[33mID:       [0m RUSTSEC-2024-0436
[0m[0m[1m[33mURL:      [0m https://rustsec.org/advisories/RUSTSEC-2024-0436
[0m[0m[1m[33mDependency tree:
[0mpaste 1.0.15
├── qapi-protocol 0.1.0
│   └── manager 0.1.0
│       ├── simple-vm 0.1.0
│       ├── qapi-protocol 0.1.0
│       ├── preconfig-vm 0.1.0
│       └── events 0.1.0
└── manager 0.1.0

[0m[0m[1m[33mwarning:[0m 1 allowed warning found
[0m
Good, got those results. Now reading the member Cargo.toml files and running `cargo tree --duplicates`.
[0m$ [0mcd /home/bob/workspace/qrab && cargo tree --duplicates 2>&1
syn v1.0.109
└── qapi-macros v0.1.0 (proc-macro) (/home/bob/workspace/qrab/projects/qapi-macros)
    └── qapi v0.1.0 (/home/bob/workspace/qrab/projects/qapi)
        ├── events v0.1.0 (/home/bob/workspace/qrab/examples/events)
        ├── manager v0.1.0 (/home/bob/workspace/qrab/projects/manager)
        │   ├── events v0.1.0 (/home/bob/workspace/qrab/examples/events)
        │   ├── preconfig-vm v0.1.0 (/home/bob/workspace/qrab/examples/preconfig-vm)
        │   └── simple-vm v0.1.0 (/home/bob/workspace/qrab/examples/simple-vm)
        │   [dev-dependencies]
        │   └── qapi-protocol v0.1.0 (/home/bob/workspace/qrab/projects/qapi-protocol)
        │       └── manager v0.1.0 (/home/bob/workspace/qrab/projects/manager) (*)
        ├── preconfig-vm v0.1.0 (/home/bob/workspace/qrab/examples/preconfig-vm)
        ├── qapi-protocol v0.1.0 (/home/bob/workspace/qrab/projects/qapi-protocol) (*)
        └── simple-vm v0.1.0 (/home/bob/workspace/qrab/examples/simple-vm)

syn v2.0.117
├── prettyplease v0.2.37
│   └── qapi-codegen v0.1.0 (/home/bob/workspace/qrab/projects/qapi-codegen)
├── qapi-codegen v0.1.0 (/home/bob/workspace/qrab/projects/qapi-codegen)
├── serde_derive v1.0.228 (proc-macro)
│   └── serde v1.0.228
│       ├── qapi v0.1.0 (/home/bob/workspace/qrab/projects/qapi) (*)
│       └── qapi-protocol v0.1.0 (/home/bob/workspace/qrab/projects/qapi-protocol) (*)
│       [dev-dependencies]
│       └── manager v0.1.0 (/home/bob/workspace/qrab/projects/manager) (*)
└── tokio-macros v2.7.0 (proc-macro)
    └── tokio v1.51.0
        ├── events v0.1.0 (/home/bob/workspace/qrab/examples/events)
        ├── manager v0.1.0 (/home/bob/workspace/qrab/projects/manager) (*)
        ├── preconfig-vm v0.1.0 (/home/bob/workspace/qrab/examples/preconfig-vm)
        ├── qapi-protocol v0.1.0 (/home/bob/workspace/qrab/projects/qapi-protocol) (*)
        └── simple-vm v0.1.0 (/home/bob/workspace/qrab/examples/simple-vm)
        [dev-dependencies]
        └── qapi-protocol v0.1.0 (/home/bob/workspace/qrab/projects/qapi-protocol) (*)
[0m
[0m$ [0mcd /home/bob/workspace/qrab && cargo tree 2>&1 | tail -5 && echo "---" && cargo tree 2>&1 | wc -l
simple-vm v0.1.0 (/home/bob/workspace/qrab/examples/simple-vm)
├── anyhow v1.0.102
├── manager v0.1.0 (/home/bob/workspace/qrab/projects/manager) (*)
├── qapi v0.1.0 (/home/bob/workspace/qrab/projects/qapi) (*)
└── tokio v1.51.0 (*)
---
119
[0m
[0m→ [0mRead ../../projects/qapi/Cargo.toml
[0m→ [0mRead ../../projects/qapi-macros/Cargo.toml
[0m→ [0mRead ../../projects/qapi-codegen/Cargo.toml
[0m→ [0mRead ../../projects/qapi-protocol/Cargo.toml
[0m→ [0mRead ../../projects/manager/Cargo.toml
I now have all the data needed. Let me compile the report.
[0m
[0m# [0mTodos
[x] Run cargo audit for security advisories
[x] Run cargo update --dry-run for available updates
[x] Run cargo tree and cargo tree --duplicates
[x] Read Cargo.toml files for manual analysis
[ ] Compile and output findings report
[0m
# Dependency Findings — qrab

## Last Run
2026-04-11T04:40:00Z

## Summary
The workspace is in reasonable health with no critical security vulnerabilities; the main concerns are an unmaintained dev-dependency (`paste`), a dual `syn` version caused by `qapi-macros` pinning the obsolete v1, and inconsistent over-pinning of versions in `qapi-codegen`.

---

## Critical
_None._

---

## Warnings

- **[DEP-W001]** `paste@1.0.15` — unmaintained (RUSTSEC-2024-0436)
  - **Advisory**: RUSTSEC-2024-0436 (2024-10-07)
  - **Used in**: `qapi-protocol` dev-dependencies, `manager` dev-dependencies
  - **Fix**: Replace usages with [`paste2`](https://crates.io/crates/paste2) (maintained fork) or restructure test macros to avoid identifier concatenation. Since this is dev-only, runtime risk is zero, but the crate will accumulate bit-rot.

- **[DEP-W002]** `syn` — two versions in tree (v1.0.109 + v2.0.117)
  - `qapi-macros` declares `syn = "1"` while `qapi-codegen` and serde/tokio proc-macros all use `syn = "2"`. Both compile into every build, increasing build times significantly (`syn` is one of the heaviest compile-time crates).
  - **Fix**: Upgrade `qapi-macros` to `syn = "2"`. The syn 1→2 migration guide is well-documented; the main changes are `parse_quote!` / `Error` API adjustments.

- **[DEP-W003]** `tokio@1.51.0` — patch update available (`1.51.1`)
  - Patch releases often carry bug and soundness fixes.
  - **Fix**: Run `cargo update tokio`.

---

## Suggestions

- **[DEP-S001]** `qapi-codegen` — over-pinned dependency versions
  - Several deps are pinned to exact minor+patch versions, which prevents `cargo update` from pulling in bug fixes:
    - `syn = "2.0.117"` → `"2"`
    - `proc-macro2 = "1.0.106"` → `"1"`
    - `quote = "1.0.45"` → `"1"`
    - `anyhow = "1.0.102"` → `"1"`
    - `prettyplease = "0.2.37"` → `"0.2"`
  - For a library/binary that isn't publishing to crates.io with stability guarantees, `Cargo.lock` handles reproducibility; `Cargo.toml` should use caret constraints.

- **[DEP-S002]** No `[workspace.dependencies]` table
  - `anyhow`, `serde`, `serde_json`, `tokio`, `proc-macro2`, `quote`, and `syn` are each declared independently across multiple crates with no central version coordination. A workspace-level `[workspace.dependencies]` table would ensure all members use the same version spec and simplify future upgrades.

- **[DEP-S003]** `qapi-protocol` — tokio features duplicated between `[dependencies]` and `[dev-dependencies]`
  - The regular dep declares `["net", "io-util", "sync", "rt", "macros"]`; the dev dep re-declares all of those plus `["rt-multi-thread", "process", "time", "fs"]`. Cargo merges feature sets, so the dev entry could simply list only the additional features not already in the regular dep, reducing noise.

- **[DEP-S004]** Minor/patch updates available for transitive deps
  - `fastrand 2.3.0 → 2.4.1` (minor)
  - `hashbrown 0.16.1 → 0.17.0` (minor)
  - `indexmap 2.13.1 → 2.14.0` (minor)
  - `libredox 0.1.15 → 0.1.16` (patch)
  - `redox_syscall 0.7.3 → 0.7.4` (patch)
  - All are transitive; run `cargo update` to pull them in.

- **[DEP-S005]** `qapi-macros` and `qapi-protocol` — different `edition` values in workspace
  - `qapi-codegen` uses `edition = "2024"`; all other members use `edition = "2021"`. This is not an error, but inconsistency may cause surprise. Consider aligning or documenting the intentional divergence.

---

## Metrics

| Metric | Value |
|--------|-------|
| direct external dependencies | 15 |
| total transitive dependencies | 79 |
| security advisories (critical) | 0 |
| security advisories (warning) | 1 |
| outdated — minor | 3 |
| outdated — patch | 3 |
| duplicate crate versions | 1 (`syn` v1 + v2) |
| workspace dep inheritance | none |

# Audit Agent Workflow

Pattern for cleanup, refactor, quality passes on crate. Parallel exploration agents with focused review lenses, then phased sequential implementation.

## When to Use

- After feature lands fast with shortcuts needing cleanup
- Before "production-ready" milestone
- User asks multi-perspective review
- Scope wider than single targeted bug fix

Not for: targeted bug fixes, single-file edits, simple feature adds.

## The Pattern

### Phase A — Parallel exploration (audit lenses)

Spawn 3 `Explore` subagents parallel (single message, multiple tool calls). Each reviews target crate through different lens. Read-only — report findings, no code.

| Lens | Looks for | Reports as |
|---|---|---|
| **Capability inventory** | APIs/commands/features underlying lib exposes that crate could use but isn't. Maps "current CLI/static config" → "available dynamic config" | `[CAPABILITY]` |
| **Bugs + performance** | Race conditions, lock ordering, resource leaks, missing cleanup, unbounded memory, hot-path inefficiencies, error path correctness, dead code | `[BUG]` `[PERF]` `[CLEANUP]` `[TEST-GAP]` |
| **UI / best practices** | Component patterns, idiomatic framework usage, type safety (string-typed config → enums), state management, CSS responsive issues, accessibility, deployment workarounds | `[DESIGN]` `[PRACTICE]` `[UI]` `[PATTERN]` |

Each agent prompt should:
- State crate path and purpose
- List specific files to review with line numbers if known
- Specify categorization tags for findings
- Request `file:line` refs for every finding

### Phase B — Synthesis & plan

Collect findings. Spawn one `Plan` agent for phased implementation:

- Order phases so corrective fixes (correctness, lifecycle) land first
- Group structural changes (e.g., type system, removing workarounds) middle
- Defer parallel-safe polish (frontend, tests, hardening) to end
- Disjoint file ownership for parallel phases — no merge conflicts

After plan, ask user to clarify load-bearing decisions via `AskUserQuestion` before writing final plan file. Decisions to surface:

- Compatibility cutoffs (e.g., min dependency version vs. fallback path)
- Polling vs. event-driven update strategies
- Keep diagnostic code as debug-only or delete entirely
- Sequential vs. parallel agent execution

### Phase C — Phased implementation

Execute phases in order. Sequential phases (correctness, structural) primary agent does itself. Parallel phases (frontend polish, tests, server hardening) dispatch to multiple `general-purpose` cleanup agents simultaneously, each owning disjoint file set.

After each phase: run affected crate's tests. No advance until green.

### Phase D — Final verification

- `cargo test -p <crate> --features <features>` — all tests pass
- `cargo clippy -p <crate> -p <other-crates>` — no warnings
- `cargo check -p <crate> --features web` if applicable
- Review post-cleanup invariants (listed in plan)

## Cleanup-Phase Recipe

Phase 1 — **Correctness & lifecycle**: state machine errors, error paths leaving invalid state, lock ordering bugs, leaked tasks, missing cleanup. Most disruptive; lands first.

Phase 2 — **Workaround removal**: temp debugging hacks that turned out unnecessary (separate ports, polling endpoints used as diagnostics, etc.). Independent of phase 3.

Phase 3 — **Architectural shift**: big change audit identified — e.g., moving CLI args to dynamic API calls, replacing strings with enums, swapping storage backends. Touches type system, must precede phases 4-6.

Phase 4 — **Server / API hardening**: parallel with 5/6. Single-lock atomicity, bounded memory, deletion of diagnostic-only paths, tighter contracts.

Phase 5 — **Frontend polish**: parallel with 4/6. Idiomatic UI patterns, extracting inline JS/CSS to assets, responsive fixes.

Phase 6 — **Test gaps**: parallel with 4/5. Concurrency tests, error paths, lifecycle assertions, anything original test suite missed.

## Subagent Types

| Type | When | Notes |
|---|---|---|
| `Explore` | Phase A read-only audits | Single thorough pass, no edits |
| `Plan` | Phase B architecture design | Returns ordered plan with file refs |
| `general-purpose` | Phase C parallel implementation | Each gets disjoint file ownership |

## Examples in Repo History

- `docs/bmc-plan.md` — initial BMC feature plan
- `docs/video-console-plan.md` — VNC console feature plan
- BMC cleanup pass that produced this doc used 3 parallel `Explore` agents (capability/bugs/UI), one `Plan` agent, then 2 parallel `general-purpose` agents for phases 5+6 after sequential 1-4.

## Key Principles

- **Parallel exploration, sequential synthesis.** Multiple lenses save time; one synthesis avoids contradictory plans.
- **Disjoint file sets for parallel implementation.** Different agents touching same file = merge conflicts. Plan file ownership before dispatch.
- **Tests after every phase, not just at end.** Catches regressions while change context fresh.
- **`cargo test -p <crate>` not `--workspace`.** Workspace tests pull in slow unrelated integration tests. Scope to crate being modified.
- **Verify by running test, not reading rust-analyzer diagnostics.** Diagnostics lag edits, produce false errors after large refactors.
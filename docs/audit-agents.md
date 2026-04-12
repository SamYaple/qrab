# Audit Agent Workflow

A pattern for cleanup, refactor, and quality-improvement passes on a crate using
parallel exploration agents with focused review lenses, followed by phased
sequential implementation.

## When to Use

- After a feature lands quickly with shortcuts that need cleanup
- Before a "production-ready" milestone
- When the user asks for a multi-perspective review
- When the scope is wider than a single targeted bug fix

Not appropriate for: targeted bug fixes, single-file edits, simple feature
additions.

## The Pattern

### Phase A — Parallel exploration (audit lenses)

Spawn 3 `Explore` subagents in parallel (single message, multiple tool calls).
Each agent reviews the target crate through a different lens. Agents are
read-only — they report findings, not write code.

| Lens | Looks for | Reports as |
|---|---|---|
| **Capability inventory** | What APIs/commands/features the underlying library exposes that the crate could use but isn't. Maps "current CLI/static config" → "available dynamic config" | `[CAPABILITY]` |
| **Bugs + performance** | Race conditions, lock ordering, resource leaks, missing cleanup, unbounded memory, hot-path inefficiencies, error path correctness, dead code | `[BUG]` `[PERF]` `[CLEANUP]` `[TEST-GAP]` |
| **UI / best practices** | Component patterns, idiomatic framework usage, type safety (string-typed config → enums), state management, CSS responsive issues, accessibility, deployment workarounds | `[DESIGN]` `[PRACTICE]` `[UI]` `[PATTERN]` |

Each agent prompt should:
- State the crate path and purpose
- List specific files to review with line numbers if known
- Specify the categorization tags for findings
- Request `file:line` references for every finding

### Phase B — Synthesis & plan

Collect all findings. Spawn one `Plan` agent to design phased implementation:

- Order phases so corrective fixes (correctness, lifecycle) land first
- Group structural changes (e.g., type system, removing workarounds) into the middle
- Defer parallel-safe polish (frontend, tests, hardening) to the end
- Disjoint file ownership for parallel phases — no merge conflicts

After receiving the plan, ask the user to clarify load-bearing decisions via
`AskUserQuestion` before writing the final plan file. Examples of decisions to
surface:

- Compatibility cutoffs (e.g., minimum dependency version vs. fallback path)
- Polling vs. event-driven update strategies
- Whether to keep diagnostic code as debug-only or delete entirely
- Sequential vs. parallel agent execution

### Phase C — Phased implementation

Execute phases in order. Sequential phases (correctness, structural) the
primary agent does itself. Parallel phases (frontend polish, tests, server
hardening) get dispatched to multiple `general-purpose` cleanup agents at the
same time, each owning a disjoint file set.

After each phase: run the affected crate's tests. Do not advance until green.

### Phase D — Final verification

- `cargo test -p <crate> --features <features>` — all tests pass
- `cargo clippy -p <crate> -p <other-crates>` — no warnings
- `cargo check -p <crate> --features web` if applicable
- Review post-cleanup invariants (listed in plan)

## Cleanup-Phase Recipe

Phase 1 — **Correctness & lifecycle**: state machine errors, error paths that
leave invalid state, lock ordering bugs, leaked tasks, missing cleanup. Most
disruptive; must land first.

Phase 2 — **Workaround removal**: temporary debugging hacks that turned out
unnecessary (separate ports, polling endpoints used as diagnostics, etc.).
Independent of phase 3.

Phase 3 — **Architectural shift**: the big change the audit identified — e.g.,
moving CLI args to dynamic API calls, replacing strings with enums, swapping
storage backends. Touches type system, so must precede phases 4-6.

Phase 4 — **Server / API hardening**: in parallel with 5/6. Single-lock
atomicity, bounded memory, deletion of diagnostic-only paths, tighter
contracts.

Phase 5 — **Frontend polish**: in parallel with 4/6. Idiomatic UI patterns,
extracting inline JS/CSS to assets, responsive fixes.

Phase 6 — **Test gaps**: in parallel with 4/5. Concurrency tests, error
paths, lifecycle assertions, anything the original test suite missed.

## Subagent Types

| Type | When | Notes |
|---|---|---|
| `Explore` | Phase A read-only audits | Single thorough pass, no edits |
| `Plan` | Phase B architecture design | Returns ordered plan with file refs |
| `general-purpose` | Phase C parallel implementation | Each gets disjoint file ownership |

## Examples in Repo History

- `docs/bmc-plan.md` — initial BMC feature plan
- `docs/video-console-plan.md` — VNC console feature plan
- The BMC cleanup pass that produced this document used 3 parallel `Explore`
  agents (capability/bugs/UI), one `Plan` agent, then 2 parallel
  `general-purpose` agents for phases 5+6 after sequential 1-4.

## Key Principles

- **Parallel exploration, sequential synthesis.** Multiple lenses save time;
  one synthesis avoids contradictory plans.
- **Disjoint file sets for parallel implementation.** Different agents touching
  the same file = merge conflicts. Plan file ownership before dispatch.
- **Tests after every phase, not just at the end.** Catches regressions while
  the change context is fresh.
- **`cargo test -p <crate>` not `--workspace`.** Workspace tests pull in slow
  unrelated integration tests. Scope to the crate being modified.
- **Verify by running the test, not by reading rust-analyzer diagnostics.**
  Diagnostics lag edits and produce false errors after large refactors.

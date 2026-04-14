# qrab

Typed Rust bindings for QEMU QMP/QGA protocols. Types generated from QEMU QAPI JSON schemas across all release tags (v1.0+). Unified type system carry version lifecycle metadata (`since`, `until`, `deprecated`) — one type set work against any QEMU version at runtime.

## Workspace

| Crate | Path | Role |
|---|---|---|
| `qapi-codegen` | `projects/qapi-codegen/` | Parse QEMU QAPI JSON schemas from git history, generate Rust types |
| `qapi-macros` | `projects/qapi-macros/` | Proc macro: `#[qapi]` → serde/trait impls |
| `qapi` | `projects/qapi/` | Generated types + runtime traits (consumer-facing) |
| `qapi-protocol` | `projects/qapi-protocol/` | Async QMP/QGA clients + mock servers |
| `manager` | `projects/manager/` | Typestate VM lifecycle wrapper |

Examples: `examples/simple-vm/`, `examples/preconfig-vm/`, `examples/events/`

## Critical Rules

- **Schemas golden.** Never preprocess/strip/transform QEMU schema files. Parsers handle raw format.
- **Generated files = outputs.** `generated/` and `qga/` dirs (`mod.rs`, `commands.rs`, `events.rs`) made by `qapi-codegen`. Edit codegen, not generated files.
- **All QEMU release tags processed.** Tag discovery auto. No hardcoded version lists.
- **Version gating dual-layer.** Compile-time (`QEMU_MIN_VERSION`/`QEMU_MAX_VERSION` env vars) + runtime (`set_qemu_version()` OnceLock). Both stay correct.

Full rationale: [docs/design-constraints.md](docs/design-constraints.md)

## Worktree Workflow

**Main checkout sacred.** Never let agents write directly. All work in worktrees.

### Rules

1. **Every writing agent gets worktree.** Before spawning agent that creates/edits/commits, create dedicated worktree:
   ```sh
   git worktree add ../qrab-<agent-name> -b agent/<agent-name>
   ```
   Agent works entirely inside worktree. Main checkout stays clean.

2. **Read-only agents can read main checkout.** Explore/audit agents that only read (no writes, no commits) may use main checkout. If agent needs write reports/results, gets worktree.

3. **One worktree per agent.** Never share worktree between parallel agents. Each agent gets own branch and directory.

4. **Merge back sequentially.** When agent finishes, merge branch into current working branch from main checkout:
   ```sh
   # From main checkout
   git merge agent/<agent-name> --no-ff -m "Merge <agent-name> work"
   ```
   Resolve conflicts before merging next agent.

5. **Cleanup immediately after merge.** Remove worktree and delete branch:
   ```sh
   git worktree remove ../qrab-<agent-name>
   git branch -d agent/<agent-name>
   ```

6. **Verify clean state.** After all merges:
   ```sh
   git worktree list  # Only main checkout should remain
   git branch | grep agent/  # Should return nothing
   ```

### When spawning parallel agents

```
# Before spawning
for each agent:
  git worktree add ../qrab-{name} -b agent/{name}

# Spawn agents, each pointed at its worktree path

# After all agents complete
for each agent (sequentially):
  git merge agent/{name} --no-ff
  git worktree remove ../qrab-{name}
  git branch -d agent/{name}
```

### Never do

- Write files in main checkout from sub-agent
- Run two writing agents against same worktree
- Leave orphan worktrees or stale `agent/*` branches
- Commit directly to main checkout from parallel agent

## Docs

| Doc | When |
|---|---|
| [docs/architecture.md](docs/architecture.md) | Orienting — deps, build phases, toolchain |
| [docs/qapi-crate.md](docs/qapi-crate.md) | Work with generated types + runtime traits |
| [docs/codegen.md](docs/codegen.md) | Modify parser, IR, or codegen pipeline |
| [docs/macro.md](docs/macro.md) | Debug/extend `#[qapi]` proc macro |
| [docs/protocol.md](docs/protocol.md) | Async QMP/QGA clients or mock servers |
| [docs/manager.md](docs/manager.md) | VM lifecycle manager |
| [docs/design-constraints.md](docs/design-constraints.md) | Invariants to preserve |
| [docs/testing.md](docs/testing.md) | Write tests or CI |
| [docs/audit-agents.md](docs/audit-agents.md) | Multi-perspective cleanup/refactor passes via parallel review agents |

## Quick Commands

```sh
cargo test --workspace

# Regen QMP types
cargo run -p qapi-codegen -- vendor/qemu --output-dir projects/qapi/src/generated

# Regen QGA types
cargo run -p qapi-codegen -- vendor/qemu --qga --output-dir projects/qapi/src/qga
```
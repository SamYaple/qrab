# qrab maintenance

The current refactor starts with the QAPI/QMP/QGA library foundation and moves
the launcher toward Kubernetes. Work tracking and durable operational notes
belong to the `qrab` project in towl.

- Read the crate's `AGENTS.md` before editing it.
- Generated files under `projects/qapi/src/generated/` and `projects/qapi/src/qga/`
  are outputs. Change the parser, IR, generator or macros and regenerate them.
- `vendor/qemu` is pinned upstream source. Update its gitlink and the release
  lockfile together when advancing the schema baseline.
- Use `contrib/check-qapi.sh` for the focused core tests and regeneration check.
  See `docs/testing.md` for the explicit real-QEMU smoke test.
- Keep protocol tests independent of manager, systemd, KVM, guest downloads and
  confidential-computing hardware. Put launcher and boot tests with those components.
- Source changes, image builds and cluster rollouts are separate operations.
  No deployment follows automatically from a library refresh.

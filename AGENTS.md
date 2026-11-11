# qrab maintenance

The active path is the per-VM OIDC BMC, pod-native manager and QAPI/QMP/QGA libraries. The
default workspace and manager features select this path; host/SEV APIs are
explicitly opt-in. Work tracking and durable operational notes
belong to the `qrab` project in towl.

- Read the crate's `AGENTS.md` before editing it.
- Generated files under `projects/qapi/src/generated/` and `projects/qapi/src/qga/`
  are outputs. Change the parser, IR, generator or macros and regenerate them.
- `vendor/qemu` is pinned source from our local Gitea `mirrors/qemu` repository.
  Fetch QEMU only from that local remote, never directly from upstream. Update
  its gitlink and the release lockfile together when advancing the baseline.
- Use `contrib/check-runtime.sh` for normal core development. Use
  `contrib/check-qapi.sh` for parser/generator work and regeneration checks.
  See `docs/testing.md` for the explicit real-QEMU smoke test.
- Keep protocol tests independent of manager, systemd, KVM, guest downloads and
  confidential-computing hardware. Put launcher and boot tests with those components.
  Target current QEMU in protocol changes; older-release compatibility is deferred.
- Source changes, image builds and cluster rollouts are separate operations.
  No deployment follows automatically from a library refresh.

The operator authorizes direct merges and temporary breakage during this
refactor. Keep changes moving without adding review or approval gates.

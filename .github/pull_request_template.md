## What this changes and why

<!-- What the change does, the problem it solves, and anything a reviewer
should look at closely. Link related issues (Fixes #123). -->

## How it was tested

<!-- Commands you ran and their results; live QEMU or cluster tests if any. -->

## Checklist

- [ ] Every commit is signed off (`git commit -s`) under the
      [DCO](https://developercertificate.org/); the `DCO` check verifies this.
- [ ] Commits are signed (`git commit -S`) if I can, and "Allow edits by
      maintainers" is on: a maintainer re-signs any commit without a
      maintainer signature before landing it by fast-forward.
- [ ] The branch is rebased on the current `main` (landing is a
      fast-forward).
- [ ] `bash contrib/check-runtime.sh` passes locally (fmt, clippy, tests,
      generated-file drift, license check).
- [ ] Commit subjects are in the imperative mood, and bodies say what changed
      and why.
- [ ] Generated files were regenerated, not edited by hand.
- [ ] Docs are updated for any user-visible change.
- [ ] Significant AI assistance, if any, is mentioned above, and I have
      reviewed and tested the generated code.

See [CONTRIBUTING.md](https://github.com/qrab-dev/qrab/blob/main/CONTRIBUTING.md) for the full flow.

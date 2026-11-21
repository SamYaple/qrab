# Contributing to qrab

Thank you for your interest in qrab. This guide covers building and testing,
the pull request flow, the Developer Certificate of Origin (DCO) sign-off and
how to report bugs. Everyone who takes part follows the
[Code of Conduct](CODE_OF_CONDUCT.md).

## Build and test

You need:

- A Rust toolchain from [rustup](https://rustup.rs/), Rust 1.88 or newer,
  with the `rustfmt` and `clippy` components. CI pins a stable toolchain,
  currently 1.97.1, in [.github/workflows/ci.yml](.github/workflows/ci.yml).
- Optionally [cargo-deny](https://github.com/EmbarkStudios/cargo-deny), for
  the dependency license check that CI runs.

From the repository root:

```sh
cargo build --locked
cargo test --locked
bash contrib/check-runtime.sh
```

`contrib/check-runtime.sh` is the check to run before you push. It covers
the same ground as the CI jobs:

- `cargo fmt --check`;
- strict Clippy (`-D warnings`);
- the tests;
- the license check, when `cargo-deny` is installed.

## Pull request flow

Every change to `main` goes through a pull request, including the
maintainers' own changes. GitHub rulesets enforce this.
[docs/contributing-review.md](docs/contributing-review.md) has the details.

1. **Fork and branch.** Fork `qrab-dev/qrab` and make your change on a
   branch of your fork. Every change comes from a fork, the maintainers' own
   included: no branch other than `main` is pushed to this repository.
2. **Sign off every commit** (see below), and preferably sign it too (see
   "Signing commits").
3. **Open a pull request** against `main`. Fill in the template: what the
   change does, why, and how you tested it. Keep a pull request to one
   logical change; split unrelated fixes into separate pull requests.
4. **Pass the required checks.** CI runs `fmt`, `clippy`, `test` and
   `cargo-deny`, and the `DCO` check verifies every commit's sign-off. The
   branch must be up to date with `main`.
5. **Get a code owner's review.** A maintainer listed in
   [.github/CODEOWNERS](.github/CODEOWNERS) must approve the latest push, and
   every review thread must be resolved. Pushing new commits dismisses earlier
   approvals.
6. **Landing.** Maintainers land approved pull requests by signed
   fast-forward: once the PR is approved and green, a maintainer moves `main`
   to the PR's head commit with a push authenticated by their hardware
   security key. The merge buttons are not used. Your commits reach `main`
   unchanged, GitHub marks the PR merged, and `main` keeps a linear history
   with no merge commits.

Address review feedback with new commits or by amending; either is fine,
as long as every commit is signed off. Keep the branch rebased on `main`: a
fast-forward is only possible from the current tip.

## Signing commits

Every commit on `main` carries a signature that GitHub verifies, made with a
maintainer's security key. You SHOULD sign your own commits with an SSH or
GPG key registered on your GitHub account (`git commit -S`, or
`commit.gpgsign = true`), so that reviewers see them as verified.

Before landing, a maintainer re-signs every commit that is not signed by a
maintainer key, including unsigned ones: the maintainer becomes the
committer, and your authorship, author date, message and sign-off stay as
they are. The maintainer pushes the re-signed commits to your PR branch (so
leave "Allow edits by maintainers" on), CI runs again on them, and the
maintainer approves that head. Re-signing changes the commit SHAs; when
`main` reaches the new head, GitHub marks the PR merged as usual. Without
maintainer edits the maintainer opens a new PR with the re-signed commits and
closes yours with a link to it.

## Developer Certificate of Origin

qrab uses the [Developer Certificate of Origin](https://developercertificate.org/)
(DCO) instead of a contributor license agreement. By signing off a commit you
certify that you wrote the change or otherwise have the right to submit it
under the project's licenses.

Sign off by adding a `Signed-off-by` line with your real name and an email
address you can be reached at:

```text
Signed-off-by: Jane Doe <jane@example.com>
```

`git commit -s` adds the line for you. To sign off commits you already made:

```sh
git commit --amend -s --no-edit        # the last commit
git rebase --signoff main              # every commit on your branch
```

Commits made in the GitHub web editor must also be signed off. The `DCO`
check fails on any commit without a sign-off, and a pull request cannot land
until it passes.

## Commit messages

Commit messages follow the 50/72 rules. The maintainers hold their own
commits to them strictly; for contributions they are recommended, and a
maintainer may reword a message when re-signing it.

- Write the subject in the imperative mood ("Add Redfish virtual media
  eject", not "Added" or "Adds"), at most 50 characters, starting with an
  uppercase letter, with no trailing period and no `area:` prefix.
- Leave a blank line, then a body that explains what changed and why. The
  diff shows how; the body records the reasoning a reviewer or a later
  reader needs.
- Wrap the body at 72 columns. Only a line that holds a single unbreakable
  token, such as a long URL, path or command, may be longer; put it on a
  line of its own, optionally indented or after a `- ` bullet.
- Use no tabs, trailing whitespace or Markdown `#` headings (git drops lines
  starting with `#` as comments when you edit a message).
- Reference related issues in the body (`Fixes #123`).
- End with your `Signed-off-by` line. The trailers, the `Key: value` lines
  of the last paragraph, are exempt from the 72-column limit.

## Reporting bugs and requesting features

Use [GitHub issues](https://github.com/qrab-dev/qrab/issues) and pick the
bug report or feature request template.

- For a bug, include the qrab version or commit, the Kubernetes and QEMU
  versions, what you did, what you expected and what happened, with relevant
  logs (operator, `qrab-manager`, BMC or Redfish sidecar). Remove secrets and
  tokens first.
- For a feature, describe the problem you want solved before the solution
  you have in mind.

**Do not report security vulnerabilities in public issues.** Follow
[SECURITY.md](SECURITY.md).

## AI-assisted contributions

Much of qrab is written with coding agents, and AI-assisted contributions are
welcome under the same rules as any other:

- The person who submits a change is responsible for it. Review what a tool
  generated, test it, and make sure you have the right to submit it before
  you sign off. The DCO sign-off is yours, not the tool's.
- Mention significant AI assistance in the pull request description.
- The project's own agents push branches and open pull requests as the
  `qrab-dev-agent[bot]` GitHub App, never with a maintainer's credentials.
  Their pull requests take the same path as yours: required CI, the DCO
  check and approval by a human code owner.
- Automated reviewers may leave advisory comments, but they are not code
  owners and never replace the human approval.

The reasons for this policy are in
[docs/contributing-review.md](docs/contributing-review.md).

## Licensing

qrab's code is licensed under the [Apache License 2.0](LICENSE) and its
documentation under [CC-BY-4.0](LICENSE-docs). Contributions are accepted
under the same licenses. New Cargo dependencies must use a license on the
CNCF allowed list; `cargo deny --locked check licenses` enforces this.

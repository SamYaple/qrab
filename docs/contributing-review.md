# How changes reach main on GitHub

Every change to `main` in a public `qrab-dev` repository goes through a pull
request, and maintainers land approved pull requests by signed
fast-forward. GitHub rulesets enforce the parts GitHub can enforce, and the
maintainer's landing helper checks the rest.

## The path of a change

1. **Sign off every commit.** Use `git commit -s` to add `Signed-off-by: Name
   <email>`, which certifies the [Developer Certificate of
   Origin](https://developercertificate.org/). The `DCO` check fails on any
   commit without it. Commits made in the GitHub web editor must also be
   signed off. Contributors should also sign their commits; see
   [CONTRIBUTING.md](../CONTRIBUTING.md#signing-commits).
2. **Open a pull request** against `main` from a fork. No branch other than
   `main` can be created in the repository, for anyone (`fork-only`).
3. **Pass the required checks.** `fmt`, `clippy`, `test`, `cargo-deny` (the
   `ci` workflow; qrab-qapi adds `check-qapi`) and `DCO` must all pass on the
   PR's latest commit, and the branch must be up to date with `main`. Run
   `bash contrib/check-runtime.sh` locally first; it covers the same ground
   as the CI jobs.
4. **Get a CODEOWNER approval.** [.github/CODEOWNERS](../.github/CODEOWNERS)
   lists who can approve. The approval has to be for the latest push, and
   every review thread must be resolved.
5. **Landing.** The maintainer's landing helper checks the approval, the
   checks, the signatures, the sign-offs and that the head is a fast-forward
   of `main`, then pushes the head to `main` with an SSH push authenticated
   by the maintainer's security key. GitHub marks the PR
   merged; the commit SHAs do not change.

## What the rulesets enforce

On the default branch, `main-integrity`, with no bypass for anyone:

- Every commit must carry a signature GitHub verifies.
- Linear history, no force pushes, no deletion of the branch.

On the default branch, `main-review-gate`:

- A pull request with 1 approving review from a CODEOWNER. New pushes dismiss
  older approvals, and the last push must be approved by someone else.
- All review conversations resolved.
- The required status checks above, on an up-to-date branch. Each check is
  pinned to the app that must report it, so another app or a hand-posted
  status cannot satisfy it.
- The repository admin role bypasses this ruleset. That is what allows the
  maintainer's fast-forward push, since GitHub has no fast-forward merge
  button. The landing helper refuses to push unless the same conditions
  hold.

On release tags `v*`, `release-tags-integrity`, with no bypass for anyone: a
tag must point at a signed commit, and nobody, admins included, can move or
delete it. `release-tags`: only repository admins can create them.

On every other branch, `fork-only`, with no bypass for anyone: no branch
can be created. Every change comes from a fork, the maintainers' and the
bot's included, and the repository holds only `main`.

## Decision: signed fast-forward landing

Decided by the maintainer on 2026-11-19, after testing the alternatives on a
throwaway repository:

- **Rebase merge** puts unsigned commits on `main`; GitHub becomes the
  committer and drops any signature.
- **Squash merge** is signed by GitHub (web-flow), but on a bot-opened PR the
  bot becomes the author and the real author is demoted to a
  `Co-authored-by` trailer.
- **Merge commits** leave the PR's commits unsigned in history.
- **Signed fast-forward** keeps the commits exactly as reviewed: the author,
  the sign-off and the maintainer's security-key signature, which GitHub shows
  as verified. The PR is marked merged and no SHA changes, so a stacked series
  needs no restack after each landing.

GitHub cannot restrict which keys satisfy "require signed commits": any key
on the pushing account counts, and so does GitHub's own web-flow signature.
So the merge buttons that would produce web-flow signatures are disabled, and
the landing helper additionally requires every commit to be signed by a
maintainer's key.

## Threat model

The goal is that no automation, compromised or misbehaving, can put a change
on `main`, and that every change on `main` is one a human maintainer
deliberately approved and landed.

- **No agent holds owner credentials.** Agents authenticate only as the
  `qrab-dev-agent[bot]` GitHub App, with Contents, Pull requests and Workflows
  write and no Administration permission. They cannot change rulesets or
  repository settings. Owner actions (creating repositories, applying the
  rulesets) run in an attended session with a short-lived owner token that
  is revoked afterwards.
- **The bot only proposes.** It pushes to its fork and opens pull requests. It
  is not an admin, so it cannot use the `main-review-gate` bypass, and it
  holds no maintainer signing key, so `main-integrity` refuses anything it
  could push to `main`.
- **Every landing needs two touches** of the maintainer's hardware security
  key: one to sign each commit and one to authenticate the push to `main`.
  Neither key leaves the device, and the key cannot be used without someone
  physically present. A remote attacker or an agent on the maintainer's host
  can at most ask for a touch.
- **The checks run twice.** GitHub's rulesets and the landing helper check
  independently: the ruleset bypass that the fast-forward needs is backed by
  the helper refusing to push an unapproved, red, unsigned or
  non-fast-forward head.
- **Residual risks.** A maintainer who touches the key for a push they did
  not intend, or who approves a change without reading it, defeats the gate;
  the review is the human part. An owner token left unrevoked would let its
  holder change the rules, which is why it is short-lived.

## Reviewing agent-authored changes

Much of qrab is written by coding agents. An agent's pull request takes the
same path as any other: a human CODEOWNER reads and approves every pull
request, the required CI and DCO checks must pass, and the maintainer lands
it. GitHub never lets an author approve their own pull request, so a
separate identity for the agents is also what lets a human approve their
work. Automated reviewers, including agent reviewers that check a branch
before it is pushed, may post advisory comments, but they are not CODEOWNERs
and their verdict never replaces the human approval.

Two alternatives were considered and rejected:

- **An automated reviewer as CODEOWNER.** Reviews would run at agent speed
  and be public on the pull request, but agents would approve agents with no
  human in the loop.
- **Review outside GitHub, forward only approved branches.** GitHub would see
  only reviewed changes, but the substantive review would be invisible on the
  public pull request, and outside contributions would still need a second
  review path.

The cost of the chosen policy is the maintainer's review time and two
touches per landing, which limits throughput; every change on `main` carries
a visible human approval and a hardware-backed signature in exchange.

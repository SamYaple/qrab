# qrab Governance

qrab uses a maintainer model sized for a young project. It currently has one
maintainer, and this document describes how that changes as more people
contribute.

## Scope

This governance covers the qrab repository and its subproject
[qrab-qapi](https://github.com/qrab-dev/qrab-qapi), the QMP/QGA libraries and
QAPI code generator that qrab builds on. qrab-qapi has no separate
governance: this document applies to it as written, its maintainers are qrab
maintainers, and its own GOVERNANCE.md, MAINTAINERS.md and CODEOWNERS name
who reviews and approves changes there. Larger changes to the qrab-qapi
crates start as an issue in that repository, under the rules below.

## Principles

- **Open.** Anyone can take part. Discussion, design and review happen in
  public on GitHub issues and pull requests.
- **Vendor-neutral.** qrab is not controlled by any company. Roles are held
  by individuals, earned through contributions, and keep no weight from an
  employer. No single organization may hold a majority of maintainer seats
  once the project has three or more maintainers. qrab depends on open
  standards (Kubernetes, QEMU, Redfish, OIDC, SPIFFE, TPM 2.0) and does not
  favor any vendor's cluster, CNI, storage or identity provider over another.
- **Written down.** Decisions and the reasons for them are recorded where
  others can find them later: in pull requests, issues and the docs.

All participants follow the [CNCF Code of Conduct](CODE_OF_CONDUCT.md).

## Roles

### Contributors

Anyone who files an issue, reviews a pull request, improves the docs or
submits code. Contributors follow [CONTRIBUTING.md](CONTRIBUTING.md) and sign
off their commits under the DCO.

### Reviewers

Contributors with a track record in one or more areas of the codebase.
Reviewers review pull requests in their areas and help triage issues. Their
approval is not binding on its own, but maintainers weigh it heavily.
Reviewers are listed in [MAINTAINERS.md](MAINTAINERS.md) with their areas.

### Maintainers

Maintainers are responsible for the project as a whole. They:

- review and merge pull requests (they are the code owners in
  [.github/CODEOWNERS](.github/CODEOWNERS));
- set the [roadmap](ROADMAP.md) and cut releases;
- handle security reports under [SECURITY.md](SECURITY.md) and Code of
  Conduct reports under [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md);
- administer the `qrab-dev` GitHub organization and project accounts;
- uphold this governance.

The current maintainers are listed in [MAINTAINERS.md](MAINTAINERS.md).

### Automation

Coding agents act through the `qrab-dev-agent[bot]` GitHub App. Automation
holds no role: it cannot approve pull requests, and every change it proposes
needs a human code owner's approval, as for any contributor.

## Decision making

qrab uses **lazy consensus**. A proposal (a pull request, or an issue for
larger changes) that draws no sustained objection after a reasonable time for
review is accepted. Most decisions are made this way in the course of
reviewing pull requests.

- Routine changes need the approval of one maintainer who did not author
  them, as the review rules on `main` require.
- Larger changes (new components, API changes to the custom resources,
  removing features, changes to the security model) start as an issue that
  states the problem and the proposed design, and stay open for at least one
  week for comment.
- When consensus cannot be reached, the maintainers decide by a simple
  majority vote. On a tie, the proposal is not accepted.

Decisions are recorded in the pull request or issue where they were made, and
lasting design decisions in the docs.

## Becoming a reviewer or maintainer

A contributor becomes a **reviewer** after sustained, good-quality
contributions and reviews in an area, typically several merged pull requests
and reviews over a few months. Anyone can nominate someone, including
themselves, by opening a pull request that adds them to
[MAINTAINERS.md](MAINTAINERS.md). A maintainer approves it under lazy
consensus.

A reviewer becomes a **maintainer** after showing judgement across the
project, not just one area, and a commitment to the project's health:
reviewing others' work, helping users and following through on releases and
reports. Nomination works the same way, and the existing maintainers must
approve it with no maintainer objecting.

Reviewers and maintainers can step down at any time by opening a pull request
that moves them to an emeritus section of [MAINTAINERS.md](MAINTAINERS.md).
A reviewer or maintainer who has been inactive for six months may be moved
to emeritus by the other maintainers, after asking them first. In the rare
case of a Code of Conduct violation, a role can be removed by a vote of the
other maintainers.

## Changes to this governance

Changes to this document are made by pull request, stay open for at least
one week, and need the approval of the maintainers with no maintainer
objecting.

## CNCF

qrab is preparing to apply to the [CNCF](https://www.cncf.io/) as a Sandbox
project. If accepted, the project follows the CNCF's policies, including the
[CNCF Code of Conduct](https://github.com/cncf/foundation/blob/main/code-of-conduct.md),
the [IP policy](https://github.com/cncf/foundation/blob/main/charter.md#11-ip-policy)
and the trademark and license requirements of the CNCF charter.

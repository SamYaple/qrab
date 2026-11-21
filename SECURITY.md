# Security Policy

## Supported versions

qrab is pre-1.0. Security fixes go to `main` and to the latest release only.
There are no long-term support branches; upgrade to the latest release to get
a fix.

| Version | Supported |
| --- | --- |
| `main` | Yes |
| Latest release | Yes |
| Older releases | No |

## Reporting a vulnerability

**Do not open a public issue, pull request or discussion for a security
problem.** Report it privately in one of these ways:

1. **GitHub private vulnerability reporting (preferred).** On the
   [qrab repository](https://github.com/qrab-dev/qrab), open the **Security**
   tab and choose **Report a vulnerability**, or go directly to
   <https://github.com/qrab-dev/qrab/security/advisories/new>. The report is
   visible only to you and the maintainers, and the fix can be prepared in a
   private fork attached to the advisory.
2. **Email** [security@qrab.dev](mailto:security@qrab.dev).

Please include:

- the affected component (for example the operator, `qrab-manager`, the BMC
  or Redfish sidecar, `qrab-tpm-issuer` or `qrab-node-agent`) and version or commit;
- a description of the problem and its impact;
- steps or a proof of concept to reproduce it, and the deployment
  configuration it depends on;
- whether you want to be credited, and under what name.

## What to expect

| Step | Target |
| --- | --- |
| Acknowledge the report | Within 3 business days |
| Initial assessment: confirm or reject, and a severity | Within 10 business days |
| Fix and release | Depends on severity and complexity; we keep you updated at least every 2 weeks |

These are targets, not guarantees. If you get no acknowledgement within 3
business days, send a reminder by the other channel.

## Disclosure policy

qrab follows coordinated disclosure:

1. The maintainers confirm the problem, determine the affected versions and
   prepare a fix in private.
2. A disclosure date is agreed with the reporter. The default is when a fixed
   release is available, and no later than 90 days after the report unless
   both sides agree to extend it, for example while an upstream fix is
   pending.
3. The fix is released and a GitHub security advisory is published, with a
   CVE where one applies, crediting the reporter unless they ask otherwise.

If a vulnerability is already public or actively exploited, the maintainers
release a fix and advisory as soon as possible.

## Scope

In scope is the code in the qrab repositories and the artifacts the project
publishes from them:

- the Rust components: operator, `qrab-manager`, BMC, Redfish sidecar,
  dashboard, `qrab-tpm-issuer`, `qrab-node-agent`, `qrab-spiffe`, OIDC bearer
  verification, guest agent, SPIRE relay, KVM device plugin and
  observability bridge;
- the generic Kubernetes manifests and policies under `contrib/kubernetes`;
- container images released by the project, including how they are built
  and configured;
- qrab's changes to bundled third-party code, such as its noVNC audio patch.

Out of scope, and best reported to the upstream project:

- **QEMU** itself, including the QEMU binary in qrab's runtime image, unless
  the problem comes from how qrab configures or launches it. Report QEMU bugs
  through the [QEMU security process](https://www.qemu.org/contribute/security-process/).
- **noVNC** upstream code, as opposed to qrab's patch to it.
- Other upstream software qrab uses: Kubernetes, the CNI, CSI drivers, SPIRE,
  swtpm, OVMF/EDK II, your OIDC provider, and the Rust crates qrab depends
  on.
- Weaknesses that follow from qrab's documented trust model, for example that
  swtpm is a software TPM whose state is protected only by the node, pod and
  storage it runs on, or that the node and its kubelet are trusted.
- Problems that need an already-compromised cluster administrator, node or
  host, and misconfiguration of a specific deployment.

If you are unsure whether something is in scope, report it privately anyway.
We would rather triage a report that turns out to be out of scope than miss
one.

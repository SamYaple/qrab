# Roadmap

qrab is pre-alpha. This roadmap lists the themes the project is working
toward next. It carries no dates: items land when they are ready, in roughly
the order below. Larger items are tracked as GitHub issues; to propose a
change to the roadmap, open an issue or a pull request against this file.

## Recently completed

- **CSI disks end to end.** VM disks are block PersistentVolumeClaims on any
  CSI driver with snapshot support. Base images are imported into a block PVC
  and snapshot, and new VMs clone from it, from both the operator and the
  dashboard.
- **Stock CNI by default.** Guest NICs work on unmodified Cilium. Only live
  migration of VMs with NICs still needs the opt-in handoff mode (below).
- **Any OIDC provider.** BMC, Redfish and dashboard login accept any
  OIDC-compliant issuer; group management for a specific identity provider
  is an optional add-on.
- **Neutral API group.** The custom resources, labels and finalizers use the
  `qrab.dev` API group.

## Near term

- **Getting started on a vanilla cluster.** A quickstart that installs qrab
  on a [kind](https://kind.sigs.k8s.io/) cluster and boots a VM with a
  Redfish endpoint, a SPIFFE identity and a vTPM, run as an end-to-end test
  in CI.
- **Public releases.** Tagged releases with signed container images and the
  install manifests for that release.
- **Helm chart.** A chart for the operator and its supporting components, as
  an alternative to the Kustomize bundles.
- **Documentation for outside users.** An overview, architecture, concepts
  and per-component reference, published on <https://qrab.dev>.
- **CNCF Sandbox application.** Apply to the CNCF as a Sandbox project once
  the public repository, governance and getting-started path are in place.

## Next

- **Live migration with a stock CNI.** Today, live migration of a VM with
  guest NICs needs an opt-in Cilium patch that hands the endpoint over to the
  target node. The goal is to upstream that endpoint handoff to Cilium, so
  that NIC-bearing VMs migrate on an unmodified CNI.
- **Redfish completeness.** Fill the remaining gaps: virtual media from
  remote sources (today only local, pre-approved images), persistent boot
  order settings (today only a one-time `BootNext`), firmware rollback,
  resetting Secure Boot keys to defaults, and reporting a guest that powered
  itself off in the VirtualMachine status.
- **Transport encryption.** Make encryption of storage traffic and of
  node-to-node links a supported, documented part of a qrab install, not
  something each cluster arranges for itself. Control traffic between qrab
  components already uses SPIFFE mTLS.

## Later

- Community growth toward CNCF incubation: more maintainers from more than one organization,
  more adopters, and a third-party security review.

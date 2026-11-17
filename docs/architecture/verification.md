# What the presentation can prove

Use the strongest claim supported by the particular evidence collected. Code
review, a manifest and a real-peer fixture establish different things; adding
them together does not automatically establish a live workload outcome.

![Different evidence classes](diagrams/rendered/light/34-evidence-classes.svg)

## Evidence gathered during this investigation

| Check | Actual scope | Useful presentation claim | Remaining limit |
| --- | --- | --- | --- |
| Default runtime check | Formatting, workspace tests, generated CRD/policy comparisons and strict Clippy | Repaired source passes the repository's normal local checks | No guest boot, cluster, ZFS/LIO or real Cilium datapath acceptance |
| Pinned QEMU image fixture | QEMU 11.1.3 qemu-img, benign local files, raw/standalone qcow2 and external dependency cases | The importer refuses extra image inputs before invoking QEMU | No production image publication or live import/export test |
| Real QMP smoke | Local pinned 11.1.3, machine none/TCG, private monitor sockets; compact/pretty and legacy/object monitor frontends | Generated schema/types and protocol behavior work against that real peer | No firmware, KVM, guest, disk, network or migration workload |
| Real QGA smoke | Local qemu-ga 11.1.3, private listener/temp files and explicit RPC allowlist | Sync, info/ping, a 2 MiB typed file read and allowed exec/status work against that peer | No in-guest agent rollout or application consistency proof |
| Original socket-peer fixture | Private child process and Unix connection on a supporting local kernel | Peer authority does not become a replacement numeric process after exit | No live VM grant or node rollout |
| Kernel networking fixtures | Throwaway user/network namespaces, real TAP/FD transfer, routes, DHCP and namespace replacement | The exercised local kernel mechanisms work in isolated fixtures | Dummy uplink replaces Cilium; VPN cases skipped without WireGuard tools |
| Manager NIC fixture | Stand-in netd and QMP peers, actual manager reconciliation | Namespace token change causes ordered NIC-only detach/attach with the same MAC/root port | Guest hot-unplug and traffic continuity require their own acceptance |
| Private netd socket faults | Fragmented JSON, descriptor timing/count/truncation, malformed input, dribble and full listen queue | Frame bounds and descriptor cleanup are regression-tested without a node daemon | No production endpoint handoff or remote packet test |
| Limited native observation | netd host cgroup view, node kernel metadata and no-op status server dry-runs | The recorded prerequisites and API conditional-write semantics matched at that time | Not a fresh health audit or proof of every feature/image |
| OpenStack comparison baseline | The user's description plus primary upstream service documentation | The diagrams compare the supplied ownership model and general service duties | The other cluster's release, backend and workload behavior were not inspected |

The final combined runtime gate used `RUST_TEST_THREADS=4` on the busy local
host. The normal command and its assertions were unchanged. The audit records
the isolated timing rerun and the separately repaired provider setup.

Source/test links and upstream references are in [the evidence register](evidence.md).
The [audit](audit.md) identifies which repair each regression supports.

## Reproduce the explicit peer checks

The maintained [testing guide](../testing.md) supplies the full commands and
requirements. The essential protocol checks are:

```sh
QRAB_TEST_QEMU=/path/to/pinned/qemu-system-x86_64 \
  cargo test --locked -p qapi-protocol --test live_qemu -- --ignored --nocapture
QRAB_TEST_QGA=/path/to/pinned/qemu-ga \
  cargo test --locked -p qapi-protocol --test live_qga -- --ignored --nocapture
```

The binaries used here came from the locked local Gitea QEMU checkout,
canonical source preparation, verified dependency archives and the locked
local patch. Downloads were disabled during configuration. This audit did
not advance the QEMU source baseline.

The networking fixtures can run in throwaway user/network namespaces where
the kernel permits it. Read the emitted skip reasons: a test function that
returns early is not evidence that its privileged or optional path ran.
The current record distinguishes four exercised kernel fixtures from two
VPN fixtures that skipped for missing tooling.

## Reject findings that fail the real contract

The audit initially suspected that `ingress: [{}]` in two CiliumNetworkPolicy
examples allowed all incoming traffic, by analogy with Kubernetes
NetworkPolicy. Tests of the exact examples through the local Cilium fork's
production policy repository denied unrelated ingress. Both examples were
preserved. Its auxiliary step-by-step simulator disagreed for those inputs,
so that simulator was not treated as the sole oracle.

This was a rejected finding, not a repaired production defect. The distinction
matters when replacing familiar OpenStack mechanisms: a similar field name
or diagram shape does not establish the same contract in another system.
Detailed source/reproduction notes are in qrab towl
`work/cilium-default-deny-ingress-policy`.

## Desired workload acceptance remains explicit

Before moving a real workload, prove its boot/firmware identity, required
management roles, network policy and route behavior, persistent disk recovery,
backup/restore and application consistency. Exercise the intended node-loss,
migration and deletion procedures using the chosen release/configuration.
Measure performance or cost before making a quantitative improvement claim.

Those are desired acceptance criteria. The local tests above make particular
implementation contracts reviewable; they do not turn unperformed acceptance
work into a completed result.

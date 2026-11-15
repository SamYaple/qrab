# Redfish/Secure Boot POC rollout — 2026-11-14

> Historical record of the 2026-11-14 release; the images named here are no longer the live ones. Current pins are in [images.lock.json](../contrib/images/images.lock.json) and [redfish-poc/vm.yaml](../contrib/kubernetes/redfish-poc/vm.yaml).

The integrated release is deployed in `qrab`. The public console/Redfish hostname
remains `https://qrab-demo.yapplesauce.com`, explicitly routed to the fresh
`qrab-redfish-poc` VM. This environment requires private DNS/routing; tests used
the real TLS hostname/SNI with the Ingress IP `10.203.1.80`, not disabled TLS.

## Release and identity

| Component | Immutable image |
| --- | --- |
| QEMU 11.1.3 | `zot.yapplesauce.com/qrab/qemu@sha256:771ce49b1185dc38986a67e7cafbd3de4fa5100f80661e681f4f3add012c66b5` |
| Manager/BMC/Redfish | `zot.yapplesauce.com/qrab/manager@sha256:3e218436dc9e9903e4414ef12fe1f30eb8a53797836d0ae4d2461b233f9e420e` |
| Operator | `zot.yapplesauce.com/qrab/operator@sha256:19a38f40ffc5723ff5cbf945ecc2722e72db6290e0da940862800a65bc83a877` |

Manager source is `625c7710cef27ee4a52cfa375a1c2d4236ea2391`; operator source is
`e1e76f554850cfe877559d0ba83874bc159c04f3`. The latter integrated parallel
Prometheus/Grafana export and the default-off risky-console admin bucket.
Build provenance is also recorded in `contrib/images/images.lock.json`.
The OCI publication guard caught a stale cached revision label during a build;
explicit build-time source labels now prevent publishing that mismatch.

GitOps PRs 433, 434 and 435 passed repository CI and independent review before
merge. Their exact operator SPIFFE selectors were reconciled by Flux before
each operator rollout. The storage-client overlay and token-free VM isolation
were preserved. Old retained claims were never adopted or reset: there were no
VM CRs/Pods at the start, so the POC received new, explicit state claims.

- VM UID: `8f3ea37c-899e-43e3-a37f-a7e87365e54e`.
- State PVC UID: `4280b205-1032-46b6-822f-66ab55ed6ed5`.
- Public TPM EK SHA256: `8dc07a7288ae2a3568560c221abfff81c57cb017f8d0d68a7ea86a70c6af759c`.

Those values remained unchanged through image updates, native PowerCycle and
controlled authorization-policy restarts. No guest NIC was introduced. The
current VM/Pod and operator are Ready, with zero container restarts after their
latest intentional Pod replacements.

## Tests and fixes from real deployment

- Exact published QEMU/guest artifacts booted under 11.1.3, without rebuilding
  the image initrd or replacing its agent. Signed Linux booted; the same unsigned
  loader was rejected when enabled. Real EFI databases and BootOptions answered.
- The demo image initially lacked the efivarfs module/mount. Both are now included
  before agent startup; the guest emits `QRAB_EFIVARFS_READY`.
- Actual CPU queries exposed a generator defect: valid `x86_64` discriminators
  with implicit empty union branches were rejected. The IR now implements the
  pinned QEMU schema rule, outputs were regenerated from local Gitea only, and
  complete current-wire fixtures replace the incomplete mock. Unknown targets
  and invalid explicit s390x payloads still fail. `contrib/check-qapi.sh` passed.
- Linux's conservative immutable inode guard initially blocked authenticated
  database writes before firmware authorization. The fixed-name transaction
  validates AUTH2 first, preserves/restores exact flags, and never exposes an
  arbitrary file/ioctl API. Native initial db/KEK/PK creation and protected writes
  then passed, including integrity-lockdown boots. Restoration failures report
  unknown outcome, not success.
- Approved signed media initially failed the operator's old single-path enum.
  CRD and Rust policy now allow only safe media basenames on a separate read-only
  PVC; manager canonical validation rejects escape to identity/Secrets.
- Real Kanidm PKCE over the public TLS endpoint passed 92 admin resource GETs,
  seven harmless validation checks, three signed database updates, staging and
  diagnostics. With the same real ID token and a controlled viewer-only VM ACL,
  89 resource GETs passed, sensitive configuration/UUID were redacted and all
  seven mutation checks returned403. Admin policy was restored. The token stayed
  in memory only and was explicitly discarded.
- Native SecureBootEnable staging left the current boot Disabled. Redfish
  PowerCycle changed the Pod UID and returned with actual current boot Enabled.
  Signed updates also passed over authenticated HTTPS while Enabled. Tampered
  and replayed timestamped replacements were rejected; public trust material was
  unchanged on rejection. No offline VARS reseeding was used for live enrollment.
- Read-only live acceptance passed 66 missing/malformed-token rejections,
  generated-resource shapes, live hardware/metrics, published image IDs, protected
  VNC/control paths, UEFI/TPM/eventlog evidence and identity continuity.
- Prometheus target `qrab/qrab-vm-redfish` is Up; `qrab_vm_vcpus` for this VM is1.
  Non-monitoring TCP access to9181 was denied/timed out while Prometheus could
  scrape it. Grafana's native dashboard watcher loaded `/tmp/dashboards/qrab-vm.json`.
- Final integrated source validation: 545 tests passed, zero failed, 12 explicitly
  ignored; formatting, all generated comparisons and strict Clippy passed.

The guest is an explicitly signed external POC media asset, not an automatic
upgrade of a tenant OS. Its SHA256 is
`613f10a207a74e7ded996f0412d9deb27cdc4f0cc35738ad73df3505ed8c0253`.
Only this public image was copied to the media PVC. The throwaway signing key
remains private in ignored local proof artifacts and never entered an image,
manifest or API payload. This is a test trust profile, not production enrollment.

## Resource observation

A 10-second final acceptance sample measured independent cgroups:

| Container | CPU | memory.current | working-set estimate |
| --- | ---: | ---: | ---: |
| Manager/QEMU/swtpm | 126m | 503.5MiB | 461.8MiB |
| BMC | 3.4m | 2.64MiB | 2.64MiB |
| Redfish/exporter | 4.3m | 2.80MiB | 2.80MiB |

No throttling occurred in that sample; OOM counters were zero. Existing limits
remain manager2cores/1GiB and each sidecar500m/128MiB, with separate requests.
These are acceptance observations, not an old/new performance comparison: no VM
existed at the beginning. No Redis, firmware helper daemon or one-off metrics
backend was added. See `contrib/experiments/redfish-performance.md` for the
separate repeatable allocation/checkpoint experiment and its explicit tradeoffs.

## Reproduction and remaining scope

Use `contrib/kubernetes/redfish-poc` after the documented signed-media bootstrap,
`redfish-rollout-test.py --connect-ip 10.203.1.80`, and
`redfish-authenticated-poc.py` for interactive real-issuer tests. Baselines and
final summaries live in ignored `target/redfish-poc-*.json`; no raw serial,
configuration, token or private key is stored in those reports.

Temporary copy Pods, the stray namespace-only test CR and authentication helper
were cleaned up; earlier user PVCs/worktrees remain untouched. The active source
worktree is `.worktrees/redfish-rollout`. A linked-worktree/container Git metadata
limitation in the unrelated local GitOps delivery gate is tracked in towl;
independent real PR CI passed in its self-contained checkout.

The full-stack towl container remains open for native ForceOff/On intent,
approved default-key reset/rollback, persistent task/history semantics and full
firmware-policy migration transfer. Migration currently refuses unsupported
retained management policy/media rather than silently losing it. No audio or USB
redirection device was enabled by the default-off admin bucket.

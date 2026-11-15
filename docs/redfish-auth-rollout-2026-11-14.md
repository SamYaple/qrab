# Redfish authentication release — 2026-11-14

> Historical record of the 2026-11-14 release; the images named here are no longer the live ones. Current pins are in [images.lock.json](../contrib/images/images.lock.json) and [redfish-poc/vm.yaml](../contrib/kubernetes/redfish-poc/vm.yaml).

The validated authentication release is merged and deployed. Build source is
`b2966828407ec22b03d58f4cd44293263596dd02`; immutable pins were merged as
`372fcee`. The coordinated release includes the dashboard’s native Redfish
integration and the BMC console layout already merged by parallel work.

## Routing and authentication

| Request | Destination |
| --- | --- |
| Public `/redfish/*` | Ingress → Redfish sidecar, port 8081; bypasses BMC |
| Browser UI `/bmc/redfish/*` | BMC session adapter → loopback Redfish |
| Other console paths | BMC, port 8080 |

The browser adapter retains OIDC session, Origin and CSRF checks and keeps the
login token server-side. Its prefix is removed before forwarding native resource
URIs. Requests, advertised action URIs and SSE use the browser prefix. A direct
request to the BMC’s old `/redfish/v1/` returns 404; the public native endpoint
returns Redfish 401. The deployed console asset matches the release source.

OIDC remains required. Local Basic accounts and anonymous discovery are disabled
in this deployment. Standard sessions and the explicit OIDC-to-session exchange
are available in the image. Redfish SPIFFE enforcement remains deferred: no
workload listener or new Redfish Workload API mount was enabled. No Redis was
added. The existing operator/storage SPIRE image selector was maintained.

## Published images

- manager: `zot.yapplesauce.com/qrab/manager@sha256:99a29308fe52cd5d433e2ebbc39b24ce4e992e3d16e3f028c4fc33e6d6b8ca1f`.
- operator: `zot.yapplesauce.com/qrab/operator@sha256:6d9ad275139d14ee1fba34111319ad180a919850a6f63d5a895ef2a9b0f15203`.
- dashboard: `zot.yapplesauce.com/qrab/dashboard@sha256:9e0ce7a3067133e283b7a35dd5bda38c5e9273f85c8e87b94199bdc19d59f849`.

GitOps [PR 438](https://gitea.yapplesauce.com/sam/kubernetes-gitops/pulls/438)
passed required CI and review, merged as `7f0b6c999128b71adbae49cc524ac0107ed18e17`,
and Flux reconciled the exact operator selector before its rollout.

## Live acceptance

- VM UID unchanged: `8f3ea37c-899e-43e3-a37f-a7e87365e54e`.
- State PVC UID unchanged: `4280b205-1032-46b6-822f-66ab55ed6ed5`.
- New Pod UID: `56dd6474-d868-4eb3-9e69-adc3fceaba34`.
- Public TPM EK SHA256 unchanged: `8dc07a7288ae2a3568560c221abfff81c57cb017f8d0d68a7ea86a70c6af759c`.
- Actual Secure Boot remains Enabled; QEMU remains 11.1.3; guest NIC remains absent.
- VM Pod 3/3 Ready, dashboard and operator Ready, zero container restarts.
- Kubernetes acceptance checks passed, including the actual Ingress backend
  targeting 8081, measured guest boot, private QMP/VNC, protected console and OIDC
  login redirect. The live harness passed 66 unauthenticated/malformed-token
  rejections and image/identity continuity assertions.
- Dashboard-to-Redfish access on 8081 is reachable and requires authentication.
- Prometheus reports the replacement Pod target Up and one vCPU.
- Authenticated client/session behavior was tested by the independent spike and
  automated suite; no fresh human Kanidm login was requested for this final rollout.

## Resource observation

A 10-second post-rollout sample measured each container’s own cgroup. These are
acceptance observations, not a controlled performance comparison.

| Container | CPU | memory.current | Working set |
| --- | ---: | ---: | ---: |
| manager | 104.9m | 496.69 MiB | 455.00 MiB |
| bmc | 3.3m | 2.59 MiB | 2.59 MiB |
| redfish | 3.8m | 4.79 MiB | 4.78 MiB |

There were zero OOM events and no throttling in the sample. Existing separate
requests/limits remain manager 2 CPU/1 GiB, BMC 500m/128 MiB and Redfish
500m/128 MiB. Optional password hashing was not enabled in this measurement.

## Verification and cleanup

The integrated `contrib/check-runtime.sh` passed: 586 tests, zero failures,
13 explicit ignores; formatting, generated comparisons and strict Clippy passed.
The preceding independent spike passed 233 HTTPS protocol assertions, with
55 untested; full conformance still requires guest-backed validation and published
OEM schemas. See [the experiment report](../contrib/experiments/redfish-interop.md).

Temporary port forwards and the GitOps worktree were removed. Old PVCs and
unrelated worktrees were preserved. Sanitized evidence is retained only in ignored
`target/redfish-auth-*.json`; no new login token or signing key was created.

# Platform evidence and external service boundaries

The [atlas](../connections.md) is based on qrab source at `86a504d`. This
appendix adds **read-only live configuration observations on 2026-11-15,
approximately 19:12–19:15 UTC**, and source checks in external repositories.
No Secret data, private keys, bearer tokens, packet captures or traffic
instrumentation were collected. No workload, policy, route, image or deployment
was changed. Configuration/status evidence is not a captured TLS handshake or
proof that a guest exercised an optional feature.

The snapshot precedes the NAT-PMP source commit `86a504d` at 19:16 UTC. Its
mapping/renewal and broker status paths are documented from source; no deployment
of that new feature was verified by these earlier observations.

## Live qrab configuration

The context was `kubernetes-admin@kubernetes`. The namespace had the dashboard,
operator, Proton broker and `qrab-redfish-poc` running; there were **no NIC
anchor Pods**. The POC VM spec had `nics: []`, no `disks`/`iscsiDisks`, and
separate state, media and Redfish-state claims. Thus the TAP, VPN, iSCSI and
live-migration inventories describe supported/configured paths, not simultaneous
observed POC traffic.

| Object | Observed image digest / relevant configuration |
| --- | --- |
| Dashboard | `qrab/dashboard@sha256:b688afbc9a374891673d8dbd9c5cf4c68e8c31ac47e9e8817684507bfbe2b0f4` |
| Operator | `qrab/operator@sha256:5e920da2a3a681d2361badbc465e936177da46a59dda532ed0aa37a10d5a4516` |
| Proton broker | `qrab/operator@sha256:62b96ae7a8bc16286ef4b8cbb46e422d22bd3ae973f9620d792305386338a2f6` |
| `qrab-redfish-poc` manager/BMC/Redfish image | `qrab/manager@sha256:4522193142cd2ba484c4826bc5d1a043c9fbd9ef68bd1fd09e1fdd2eee9d56be` |
| Node netd and KVM plugin on kube01, kube02, kube03 | Same operator digest `5e920da2…`; **different** ServiceAccounts `qrab-netd` and `qrab-kvm-device-plugin` in `qrab-node` |
| POC BMC | Dashboard bearer issuer/audience `https://idm.yaple.net/oauth2/openid/qrab-dashboard` / `qrab-dashboard`; Redfish upstream `http://127.0.0.1:8081` |
| POC Redfish | API 8081, metrics 9181; primary issuer/client `qrab-redfish`, trusted browser pairs `qrab-bmc` and `qrab-dashboard`; no configured workload TLS listener or Workload API mount |
| POC manager | Workload API `unix:/run/spire/agent.sock` on CSI volume `csi.spiffe.io`; private runtime, state, control and downward-API mounts |
| POC ingress | Cilium ingress TLS for `qrab-demo.yapplesauce.com`; `/redfish` goes directly to Service `redfish`, `/` to BMC `http` |
| Policies | Standing POC migration guard, metrics allow/deny policy, Prometheus egress, dashboard→VM ingress and broker policy present; no active migration-specific allow policy in the observed list |
| StorageClass `zfs-block` | Provisioner `zfs-block.csi.yaple.net`, `Retain`, `Immediate`, expansion enabled |

These digests are in `zot.yapplesauce.com`. The POC can intentionally select an
image different from the operator's default VM image; do not use the default
digest alone as evidence of the running VM's source behavior. The digest table
is a dated snapshot, not a release pin update.

### Cilium does not supply encryption here

`cilium-dbg status` reported **`Encryption: Disabled`** and
`Routing: Network: Native, Host: BPF` on **all three** Cilium agents, one each
on kube01, kube02 and kube03. `cilium-config` reported
`mesh-auth-enabled: "false"` and `routing-mode: native`.

Consequently no Cilium WireGuard/IPsec overlay or mesh-authentication claim
can be used to reclassify the atlas's plain HTTP, WS, iSCSI or conntrack hops
as encrypted in this observed deployment. This does not test arbitrary host
firewall/tunnel configuration or capture physical packets. A VPN NIC's own
`wg0` is a separate, explicit encryption mechanism.

### SPIRE registrations and controller

Live `ClusterSPIFFEID/qrab-vm` uses class `kubernetes-tpm`, trust domain
`yapplesauce.com`, TTL 10m and template
`spiffe://yapplesauce.com/kubernetes/qrab/vm/{{ index .PodMeta.Labels "qrab.yapplesauce.com/owner-uid" }}`.
It selects `qrab` VM Pods and workload selectors namespace `qrab`, ServiceAccount
`qrab-manager`, container **manager**, UID 10001. It selected one Pod and
reported zero entry failures at observation time. This supports the per-VM
identity contract; the registration status alone is not a captured SVID or
migration handshake.

`ClusterSPIFFEID/qrab-operator` matched the atlas's exact operator ID, container,
ServiceAccount, UID and deployed image digest, TTL 10m; one selected Pod and
zero entry failures. A retained `qrab-nic` registration still names the old
**nic** container selector and per-NIC URI template, with zero selected Pods.
It is not evidence of an active SPIFFE NIC data channel: current anchors run
container **anchor**, and their datapath is TAP/vhost.

The live controller ConfigMap uses trust domain `yapplesauce.com`, class
`kubernetes-tpm`, local SPIRE server API socket
`/run/spire-server/private/api.sock`, explicit TPM DevID parent mappings for
kube01–03 and leader election via Kubernetes. It reconciles ClusterSPIFFEIDs,
with static-entry and federation reconciliation disabled. The controller's
health listener is :8081 and its own metrics bind is disabled. None of this
enrolls the qrab guest OS in SPIRE.

## External source checks

These immutable source references were read locally; they establish contracts
without claiming that every native host process was verified against that
revision. In particular, the inspected GitOps seed is older than some live
qrab enrollments; live observations above take precedence for current objects.

### Storage ingress and exchange shim: S1–S3, A1

The standalone shim source at `4078a3e200cec8664170e55db9bae6867e83bbc3`
closes two boundaries that cannot be inferred from qrab's HTTPS client alone:

- The storage ingress is TLS 1.3 on host :9477, forwarding to the **fixed
  plain loopback HTTP** daemon origin `http://127.0.0.1:9476`. It forwards
  bearer and JSON content type, not trusted identity headers; paths and
  operation shapes are allowlisted. Thus S3 has a local plaintext TCP leg
  after TLS termination, before daemon→wrapper stdin/local operations.
- The exchange shim calls Kanidm over HTTPS: issuer discovery/JWKS, a directory
  read of subject `may_act` and actor `spn`, then RFC 8693 `/oauth2/token`
  with subject/actor credentials, actor instance nonce and requested scopes.
  Long-lived actor/subject/grant-reader API-token files stay in the native
  shim; they are not issued to the qrab operator. The shim validates the
  resulting delegated token and returns it over the caller's mTLS connection.
- For image-pull policies, the kubelet credential provider receives a
  Pod-bound ServiceAccount token and image reference via local JSON stdin,
  calls the shim over SPIFFE mTLS using its node Workload API identity, and
  returns a zero-cache-duration registry credential on local stdout. The
  shim validates repository and exact caller policy, calls Kubernetes
  TokenReview over HTTPS for audience `spire-kanidm-shim`, and verifies
  current Pod UID, ServiceAccount UID, placement and pinned Node UID through
  API reads before delegation. Runtime→Zot OCI traffic is a separate HTTPS
  registry connection using that returned bearer as a registry credential.

Sources: [storage frontend and loopback origin](https://gitea.yapplesauce.com/infrastructure/spiffe-kanidm-shim/src/commit/4078a3e200cec8664170e55db9bae6867e83bbc3/src/storage.rs),
[Kanidm grant/token exchange](https://gitea.yapplesauce.com/infrastructure/spiffe-kanidm-shim/src/commit/4078a3e200cec8664170e55db9bae6867e83bbc3/src/broker.rs),
[Kubernetes pull validation](https://gitea.yapplesauce.com/infrastructure/spiffe-kanidm-shim/src/commit/4078a3e200cec8664170e55db9bae6867e83bbc3/src/kubernetes.rs),
[credential provider](https://gitea.yapplesauce.com/infrastructure/spiffe-kanidm-shim/src/commit/4078a3e200cec8664170e55db9bae6867e83bbc3/src/client.rs) and
[deployment/TLS contract](https://gitea.yapplesauce.com/infrastructure/spiffe-kanidm-shim/src/commit/4078a3e200cec8664170e55db9bae6867e83bbc3/README.md).

### SPIRE controller and CSI local socket paths: I5/I6

The GitOps source at `eb7fdd3100c2f13cbf9399c4c898759259fa4a74` mounts host
`/run/spire` into the SPIFFE CSI driver. The driver exposes local CSI gRPC at
`/var/lib/kubelet/plugins/csi.spiffe.io/csi.sock`; the node-driver-registrar
connects it to kubelet's plugin registry. Ephemeral Pod volumes expose the
Workload API socket to workloads; this is local Unix/CSI plumbing, not another
remote TLS service. The driver mounts no Kubernetes API token. The controller
has Kubernetes API RBAC and a hostPath mount of the SPIRE server's private
Unix API socket; controller→API is HTTPS/ServiceAccount while controller→SPIRE
admin API is local Unix access. SPIRE native node-agent→server attestation and
renewal networking remains separately owned and was not revalidated here.

Sources: [SPIFFE CSI DaemonSet](https://gitea.yapplesauce.com/sam/kubernetes-gitops/src/commit/eb7fdd3100c2f13cbf9399c4c898759259fa4a74/infrastructure/spiffe-runtime/csi.yaml),
[controller deployment](https://gitea.yapplesauce.com/sam/kubernetes-gitops/src/commit/eb7fdd3100c2f13cbf9399c4c898759259fa4a74/infrastructure/spiffe-runtime/controller.yaml) and
[API RBAC](https://gitea.yapplesauce.com/sam/kubernetes-gitops/src/commit/eb7fdd3100c2f13cbf9399c4c898759259fa4a74/infrastructure/spiffe/foundation.yaml).

### The default retained state PVC: S6

The external `zfs-block-csi` source at
`a2f25ee3a88154d17120ac867ba246e13254e869` uses its own controller SVID
`spiffe://yapplesauce.com/kubernetes/storage-system/zfs-block-controller` to
exchange via :8444 and call the :9477 TLS storage ingress with a delegated
bearer. This repeats the identity/storage API pattern, with its **own** policy
and scopes; it does not borrow the qrab operator credential. The node driver
uses neither exchange nor host API.

Host storage administration attaches a zvol as a SCSI device to the Kubernetes
node VM using that node VM's host QMP hotplug path. Inside the node the CSI
driver verifies SCSI identity, formats/resizes ext4, stages it and bind-mounts
it to the Pod under kubelet's CSI directory. There is no additional CSI NBD or
guest-kernel iSCSI data connection in that described path. CSI gRPC, node
device/filesystem access and host QMP attachment are local boundaries at their
respective layers; the virtual disk is distinct from qrab QEMU's native iSCSI
guest disks. These facts do not prove ZFS or guest-volume encryption at rest.
Other storage classes, especially RWX media/Redfish claims, require their own
driver's transport inventory.

Source: [independent zfs-block CSI architecture and ownership contract](https://gitea.yapplesauce.com/infrastructure/zfs-block-csi/src/commit/a2f25ee3a88154d17120ac867ba246e13254e869/README.md).

## Repeating these observations

Useful read-only queries are `kubectl get pods/qvm/ingress` in `qrab`,
`get pods` in `qrab-node`, `get clusterspiffeids`,
`get configmap cilium-config -n kube-system`, and
`get configmap spire-controller-manager -n spiffe-system`. For each Cilium
Pod, `kubectl exec ... -c cilium-agent -- cilium-dbg status` reports effective
routing/encryption state. Select only transport-related environment names and
mount metadata when inspecting Pods; Secret values are unnecessary.

Remaining unverified boundaries are native host SPIRE agent/server settings,
Kanidm/registry/ingress internal backend legs, deployed storage/shim binary and
host-listener configuration versus the external source, host firewall/BGP auth,
log collection, API-server-to-node transport and encryption at rest. The
[capture guide](capture.md) explains how to extend evidence to those streams
without confusing source, live configuration and actual packet observations.

## Netd peer mTLS rollout: 2026-11-15 22:02–22:09 UTC

The node peer source `b2022f5` was published as
`zot.yapplesauce.com/qrab/operator@sha256:88818c3fb502a0e51c610bc934132ed72f201a457a5c8a50d62e7a4ac93dba2e`
(tag `secure-netd-b2022f5`), with the source revision label verified after push
and immutable pull. [GitOps PR 465](https://gitea.yapplesauce.com/sam/kubernetes-gitops/pulls/465)
passed CI/review and merged as `7d19b6c`. Flux rolled netd and the KVM plugin
(they share the pinned node artifact) to that digest on kube01, kube02 and kube03.
The operator Deployment and Proton broker retained their existing image.

`ClusterSPIFFEID/qrab-netd` selected all three Pods with zero entry failures;
all three daemons logged `peer_identity_ready` for their own node after initial
Workload API synchronization. The DaemonSet has the read-only CSI socket mount,
exact image-bound node registration and Flux identity dependency described in
[the enrollment contract](../networking.md#conntrack-peer-spiffe-enrollment).

The explicit conntrack oracle ran a disposable VM, with TCP 5000 ingress and
no new egress, through two live moves. The client connected once and only read;
its established server-push stream needed imported conntrack on each target:

| Move | Longest observed stream gap | Result |
| --- | --- | --- |
| kube03 → kube01 | 0.54s | precopy and delta imported; stream continued |
| kube01 → kube02 | 0.57s | precopy and delta imported; stream continued |

`contrib/check-nic-ct-handoff.sh mtlsct-20261115` returned PASS. Its VM,
migration objects, policy and claims were cleaned up; the storage system's
Retain semantics still apply to the disposable disk/state data. No existing
VM was moved by this test. This is live functional proof on the TLS-only node
artifact, in addition to the fixture tests for wrong peers, missing certificates,
plaintext refusal, malformed exports and rotation. It is not a recorded packet
capture or a guarantee about unrelated management/metrics/iSCSI streams.

## Management workload mTLS rollout: 2026-11-15 22:56–23:04 UTC

Source `793d404` was published/pulled with matching revision labels for all
three images; release pins are in `images.lock.json` (commit `1799681`):

| Component | Immutable digest |
| --- | --- |
| Operator | `sha256:0414a35b7358b7f295a63cc8b03418acd5aeb6459e2620bcc462fe1e95ea17d4` |
| Dashboard | `sha256:f200cfe745bde411c0202f23c850fb9422c6ea848c655f6dede8bef01957ab3c` |
| Manager/BMC/Redfish | `sha256:2b84545114dd871252623ea9cede97bf017bc410b3d8d8b1072822ad291d9608` |

[GitOps PR 467](https://gitea.yapplesauce.com/sam/kubernetes-gitops/pulls/467)
passed CI/review and merged as `541c302`. Flux enrolled the dashboard image and
per-VM BMC/Redfish roles and updated the operator image selector before rollout.
The scoped kustomized operator Deployment retained its storage client; dashboard
Deployment and Cilium backend policies then rolled to mTLS8443/8444. Both
Deployments became Ready. No inactive POC bundle/retained identity was recreated.

A fresh disposable VM (`secure-mgmt-20261115`, UID
`17d600d0-7178-42bd-8d2a-94744b90b4e3`) booted all three containers, received its
separate service SVIDs and migrated from kube03 to kube01. The deployed dashboard
on kube03 repeated the fixed-path probes against the new Pod on kube01:

- Correct `/vm/<UID>/bmc` and `/vm/<UID>/redfish` peers passed mTLS and returned
  the expected application **401** with no user credential supplied.
- A wrong VM UID failed peer SPIFFE authorization before an HTTP response.
- OpenSSL clients from the actual dashboard Pod, with no client certificate,
  were rejected with `certificate_required` on both8443 and8444.
- An unauthenticated browser-bridge GET through a temporary authenticated
  port-forward returned Redfish's `NoValidSession`401. BMC's actual upstream
  configuration was `https://127.0.0.1:8444` with this VM's exact Redfish ID:
  this verifies the second TLS hop without exporting any workload private key.
- The guest agent remained connected after migration. Public dashboard HTTPS
  returned200 with certificate/SNI verification through the observed ingress IP
  (the admin host needed `--resolve` because its DNS did not answer this private
  hostname).

The diagnostic port-forward, disposable VM/migration and claims were removed;
Retain semantics still preserve storage data. No fresh human OIDC login or
privileged interactive console session was attempted in this live test. Source
fixtures separately exercise actual user-auth/session behavior, the WSS binary
roundtrip, both TLS bridge hops, wrong peers, missing credentials, TLS1.2 refusal,
no plaintext fallback, redirects and resource bounds. This is transport/guest
continuity evidence, not a packet recording or a claim that ingress backends,
metrics, iSCSI or external guest application traffic have also been protected.

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

## Observability workload mTLS rollout: 2026-11-16

Enrollment began on 2026-11-15 at 23:33UTC; final live verification and cleanup
completed on 2026-11-16. The observability paths P6–P8 and new discovery path P15
are now deployed, independently of the earlier read-only snapshot.

| Role | Source revision | Verified deployed image digest |
| --- | --- | --- |
| Operator | `a205be6` | `sha256:e8c7bd9815efccd8a0abddc62c3ec3ae23e583f1c6620361a071bffbd16e6bb1` |
| VM manager/BMC/Redfish | `a205be6` | `sha256:4a147cba51ccfec915f5bece0ffba3016f434e1e0f630bfbf940dd7bcd7252ef` |
| qrab telemetry sidecar (operator repository) | `694f435` | `sha256:3a87af89b6a3fc25ed4166e323ef2212a5d4b126ff69e811bc8f915157e1e6e3` |

[GitOps PR468](https://gitea.yapplesauce.com/sam/kubernetes-gitops/pulls/468)
merged as `77f6cdf`, enrolling the container-specific telemetry SVID, read-only
VM/Pod discovery, fixed history Service and existing Prometheus ServiceAccount's
UID-bound operator-repository pull authority. The scoped native pull policy
update passed on kube01/02/03; all three nodes and SPIRE/shim/kubelet services
remained healthy. Disposable preflight Pods verified the published image,
SVID and API readiness on each node, with an unsatisfied readiness gate keeping
them out of ready Service endpoints. Those Pods were subsequently removed.

[PR469](https://gitea.yapplesauce.com/sam/kubernetes-gitops/pulls/469) merged as
`0e25f39` and activated the sidecar. Live acceptance caught a fail-closed bug in
the initial gateway: discovery checked ordinary containers instead of native
Redfish init containers. The correction and real native-container regression
fixture passed the full runtime gate; [PR470](https://gitea.yapplesauce.com/sam/kubernetes-gitops/pulls/470)
merged as `8f33fe1`, advancing only the telemetry image and its SVID selector.
Flux applied that revision Ready. The operator and updated PodMonitor were
applied separately from their scoped manifests. The final telemetry image is
tracked independently in the [release lock](../../contrib/images/images.lock.json).

The Prometheus Pod became 4/4 Ready with no container restarts. Its existing
20 Gi RWOP claim, UID `d3f38601-aacb-46f9-85d7-74b19f041781`, and volume survived
both replacements; the new Pod could start on kube01. A local query still found
thousands of samples across the preceding hour, rather than only samples since
startup. The existing native SPIFFE adapter remained present.

Two fresh disposable VMs were used: `secure-obs-20261115` (UID
`874878e8-e6a7-4232-abc9-af9682de8cdc`) and `secure-obs-other-20261115` (UID
`5e4f340d-ded6-4efb-a16f-980258a9fe32`). The first migrated successfully from
kube03 to kube02; Prometheus ran on kube01. Before and after migration:

- Production Prometheus scrapes reported `up=1`; the operator reported
  `status.powerState=On` through its exact-VM mTLS metrics reader.
- The actual Redfish container authenticated to the exact telemetry SVID over
  TLS and received a 200 history matrix with three container series. A query
  for the other live VM's Pod returned 403.
- The telemetry container read metrics over exact-VM mTLS. A wrong expected VM
  UID failed TLS. Redfish's own valid SVID was refused as a metrics client on
  loopback, independently of network policy.
- A client without a certificate received `certificate_required` on 9181;
  plain HTTP received a TLS alert rather than a metrics response.
- A bounded TCP connect from the VM to raw Prometheus 9090 timed out under the
  explicit egress deny. The old source Pod IP returned 403 from the production
  scrape adapter after migration.
- Dashboard probes still admitted the exact BMC/Redfish service identities and
  returned the expected application 401 without a user token. The migrated guest
  agent remained connected, reporting version 11.1.3.

Public certificate metadata also demonstrated rotation without process restarts:
the gateway process started at 00:02 UTC, but its observed certificate had
`notBefore=00:16:37`, `notAfter=00:26:47`; the migrated Redfish certificate had
`notBefore=00:17:11`, `notAfter=00:27:21`. Both contained their expected URI SANs.
Discovery and production scrapes remained healthy beyond the 600s projected-token
and 10m SVID lifetimes. No private key or bearer token was exported.

The [read-only diagnostic](../observability.md#release-and-verification) runs in
memory inside the attested container. The initial attempt to stage its executable
in the 1 MiB control volume failed; its partial file was removed, and the helper
was changed to leave runtime storage untouched. Both VMs retained zero container
restarts. The temporary VMs, migration, preflight Pods and claims were removed;
Retain semantics preserve storage data. No fresh human OIDC login or privileged
live console session was attempted. Ingress backend HTTP, native iSCSI and node
link protection remain separate work; this rollout does not establish their
confidentiality.

## Ingress backend mTLS and HTTPS build rollout: 2026-11-16

The deployed release adds M2/M4/M5 backend mTLS, local exec health probes
(P11), and native SPIRE SDS delivery to Envoy (I10). The
[ingress guide and diagram](../ingress.md) describe the final configuration;
this record separates publication, platform enrollment and live acceptance.
Guest application protocols and ordinary LAN/internet connectivity retain the
[user-selected policy](../networking.md#guest-protocol-and-link-protection-policy).

| Artifact | Build source | Published and verified digest |
| --- | --- | --- |
| qrab operator and telemetry | `bc7e422` | `sha256:c75cf38dd6690440e71c1175e5aa6ee872a904273355ebcd21b9de2d3c92d0ad` |
| Dashboard | `bc7e422` | `sha256:3e05ed075b7785bf723177b5e3710baf42a44a116895ae483393ddd71dcd620c` |
| Manager/BMC/Redfish | `bc7e422` | `sha256:e165cfc654ce85fdfeeab57b500471d6a2dc2abc2e55942be1eac175e865f220` |
| QEMU runtime/build | `f97a3de` | `sha256:1187d49e124d53a1c82e8b51c7cdf11c116ccd4fbf821b46f51b1e822eb3e312` |
| Cilium operator | `dc87287dfb` | `sha256:b44bd24f9ad290f90edb7ba8c7c99ce479934056c0ce8e056aba7261b3f10e1e` |

Image revision labels were checked after publication and immutable pull. The
Cilium operator's executable also reported the expected fork revision. Its
agent stayed on `sha256:f102ff20…`; netd/KVM stayed on `sha256:88818c3f…`.
The QEMU source baseline remained `005c61c`, fetched from the local mirror.
Release pins are recorded in `images.lock.json` at `664e6b2`.

All four image recipes now fetch Debian packages over verified HTTPS (A3).
They bootstrap public CA certificates from the immutable Rust builder, set
APT's CA path explicitly, retain package signatures, reject plaintext sources
and disable redirects. `apt-get update --error-on=any` refuses partial-fetch
failure. The minimal Debian HTTPS check succeeded, then the QEMU and all three
Rust artifacts were built and published. The deployed guest boot and migration
used this QEMU runtime, including its `qemu-img` tools. No QEMU baseline advance
was needed.

Platform changes passed the protected GitOps CI/review process:

| PR | Merge | Effect |
| --- | --- | --- |
| [474](https://gitea.yapplesauce.com/sam/kubernetes-gitops/pulls/474) | `6c5c09b` | Image-bound ingress SVID, CSI mount, fixed SDS bootstrap and optional Flux overlay |
| [477](https://gitea.yapplesauce.com/sam/kubernetes-gitops/pulls/477) | `8d72b31` | Preload the published Cilium operator through the node alias |
| [478](https://gitea.yapplesauce.com/sam/kubernetes-gitops/pulls/478) | `f15b09c` | Activate the operator translator and exact qrab image identities; pair telemetry image/identity |
| [479](https://gitea.yapplesauce.com/sam/kubernetes-gitops/pulls/479) | `afc3d76` | Correct Envoy's selector to its attested digest-only ImageID |

The preload reached 3/3 Ready, and read-only node inspection verified the exact
image and node-wide kubelet pull records on kube01/02/03. Temporary one-image
preload Pods held the new image while an older all-image Pod took several
minutes to terminate; those temporary Pods were removed after the DaemonSet
completed. Flux's initially stale reader credential was renewed through its
normal attested exchange path and the original renewer Deployment restarted;
no credential was exported. The diagnostic Pod used for that repair was removed.

Live acceptance exposed two independent admission prerequisites. SPIRE's image
selector uses ContainerStatus.Image/ImageID, so the spec's `tag@digest` string
was replaced with the exact runtime ImageID. The native Unix attestor also
refused Envoy's 75,395,768-byte executable under the local 64 MiB cap. Each
agent's inline configuration was validated, backed up root-only on that node,
and changed to a finite 128 MiB limit with executable hashing enabled. Agents
were restarted sequentially. Their preserved public socket directory kept its
inode, agent health recovered, and existing CSI clients continued to work.
See the [native prerequisites](../ingress.md#native-agent-and-image-admission-prerequisites).

The installed Envoy lacks the specialized SPIFFE certificate validator. Native
SDS therefore supplies standard X509 validation against only the local SPIRE
bundle, combined with mandatory exact URI SAN checks and TLS 1.3. Active Cilium
cluster resources contained the expected peer, fixed SDS cluster and ingress
client identity. No plaintext fallback was introduced.

The scoped operator/storage-client and dashboard bundles were applied with
matching images and exec probes. A fresh VM, `secure-ingress-20261116`, UID
`5b456b25-ddb6-4aa1-9c65-a5375b371bc3`, booted on kube03 and migrated to kube02
in 27 seconds. Its per-VM public test Service selected that immutable UID;
`/redfish` routed directly to Redfish. Other test Services exercised wrong and
missing backend pins without changing the production dashboard Service.

Requests were directed to each node's HTTPS NodePort with certificate/SNI
verification. The node's own Envoy request counter increased for the test VM;
this verified the chosen ingress process, not just load-balancer availability.
Before and after migration, and again after certificate rotation, all three
nodes returned:

| Request | Result |
| --- | --- |
| Public dashboard | HTTPS 200 |
| BMC `/api/session`, no user credential | 401 after workload TLS admission |
| Native `/redfish/v1/Systems/1`, no user credential | Redfish 401, `Base.1.18.NoValidSession` |
| Valid backend with a wrong expected URI | 503; backend TLS refused |
| Host from an Ingress missing its required identity pin | 404; no backend route |

Local Envoy counters recorded TLS 1.3 handshakes to BMC and Redfish. The missing
pin test used a valid configured SNI with the missing route's HTTP Host, so its
404 was a routing result rather than a frontend certificate failure. A public
WebSocket upgrade without a user credential returned 401.

Additional tests passed against the actual containers before/after migration:

- Dashboard's fixed-path probes admitted the exact BMC/Redfish identities and
  returned application 401. A wrong VM UID failed TLS before HTTP.
- Dashboard, BMC and Redfish rejected missing client certificates with
  `certificate_required`; self-signed clients claiming the exact ingress URI
  also failed TLS. Plain HTTP sent to each TLS port received a seven-byte TLS
  alert and no HTTP response.
- Ordinary dashboard/BMC/Redfish and operator Pod-address health ports refused
  connections. Local `/healthz` returned 200 and application `/` returned 404.
  Dashboard/BMC/Redfish also rejected query-bearing health requests; the
  operator's local health route accepts query strings.
- Production scrapes remained `up=1`, operator power remained `On`, and Redfish
  obtained a 200 scoped history matrix with three container series. The guest
  agent remained connected after migration, reporting QEMU guest agent 11.1.3.
- Prometheus retained its original 20 Gi claim, UID
  `d3f38601-aacb-46f9-85d7-74b19f041781`. Its telemetry container recovered from
  one initial API-inventory deadline failure; the Pod became 4/4 Ready.

Public certificate metadata demonstrated native SDS rotation in all three
unchanged Envoy processes, each with zero restarts:

| Node | First observed SVID notBefore | Rotated SVID notBefore |
| --- | --- | --- |
| kube01 | 02:22:17 UTC | 02:26:47 UTC |
| kube02 | 02:22:46 UTC | 02:27:40 UTC |
| kube03 | 02:22:49 UTC | 02:27:26 UTC |

Serials changed and every certificate retained the exact ingress URI. Public
checks passed after rotation. Only public certificate metadata and counters
were read; SPIFFE private keys and bearer tokens stayed inside their workload boundary.
The full Rust runtime gate and Cilium model tests passed for the published
source. Fixture tests cover successful authorized WSS and HTTP bridge behavior;
a fresh human OIDC login and privileged interactive console were not attempted
in this live run. These observations are functional/transport evidence, not a
packet recording.

The temporary VM, migration, entrypoints, negative-test Services/Ingresses,
certificate and claims were removed; storage Retain semantics preserve their
data. The inactive canonical POC was not recreated. Node-link encryption and
native iSCSI remain separate tracked work; this ingress release establishes
no confidentiality for those paths.

## Strict Pod WireGuard rollout: 2026-11-16

The [Pod link guide](../node-encryption.md) and
[public proof metadata](evidence/2026-11-16-wireguard.json) record the deployed
N14 data channel and I11 key-distribution channel. This step protects managed
IPv4 Pod-to-Pod links, including guest NIC anchors, while retaining guest
application protocols. Host-network, ordinary external and world-origin
forwarding traffic still require separate protection.

The Cilium fork at `b62a4ec4f00c17b37f411ebd2ed589be7b3bb222` was built,
published and immutable-pulled as
`zot.yapplesauce.com/infrastructure/cilium@sha256:6575d06b830d70814399c686a14ade8bebb1b021dac01f9da45fc21fa51b85e7`
(tag `v1.20.2-towl.11-b62a4ec4f0`). Its executable revision and OCI label were
verified. Functional and race tests cover repeated stable-IP moves, release and
reuse, unknown peers, key refresh, failed kernel updates, rollback and partially
applied updates. Before the fix, A → B → A kept sending to B; a stale failed
retry could also reclaim an IP after rollback. The fix tracks applied ownership
and latest intent, retaining the kernel's atomic prefix-transfer behavior.

[GitOps PR480](https://gitea.yapplesauce.com/sam/kubernetes-gitops/pulls/480)
merged as `a661798`, preserving all preload references and adding this exact
node alias. All three nodes held the image and matching node-wide kubelet pull
records. The preload reached 3/3 Ready; temporary preflight Pods were removed.
[PR481](https://gitea.yapplesauce.com/sam/kubernetes-gitops/pulls/481) merged as
`129ca32`, activating only the agent image and encryption configuration. The
Cilium operator stayed on `b44bd24f…`, preserving ingress SPIRE SDS behavior.

The exact pinned chart digest was rendered with checksum-verified Helm before
activation. The deployed ConfigMap independently confirmed:

- WireGuard enabled, with strict ingress and egress encryption.
- Egress CIDR `10.244.0.0/16`, including guest `10.244.128.0/17`.
- Dynamic remote-node exceptions disabled; IPv6 disabled.
- `nodeEncryption` false and the default control-plane exclusion preserved.

All three agents became Ready on the expected image, each with a
`cilium_wg0` interface and two peers. Public dashboard and forge HTTPS returned
200 after the rolling transition. Application SPIFFE mTLS remains independent
of this node-key tunnel.

The live oracle used a Python client Pod on kube01 and a CirrOS guest with
stable address `10.244.128.1`. A 4096-byte padding option exercised TCP payloads
larger than the tunnel MTU. The client connected once and only read the guest's
counter; the guest security group admitted TCP5000 in and no new egress, so
continuity required imported conntrack state.

The first run passed kube03 → kube02 with a 0.55s gap, then exposed a manager
supervision bug on the return. During the authenticated target's preparation,
migration progress callbacks were silent and the normal heartbeat was not
driven. Kubelet killed the source after three stale-status liveness failures.
A hardware-free delayed-target test reproduced the failure beyond the five-second
freshness limit. Source `c357503` adds QMP-backed heartbeat supervision throughout
that wait; the regression also requires multiple actual QMP replies. Probe
freshness and process-identity checks are unchanged. The full runtime gate passed.

The corrected manager was published/pulled with its revision label as
`zot.yapplesauce.com/qrab/manager@sha256:480be89f821a799d375a099d631e7c7a16e5605e07dd165d18129be564d8245a`
(tag `secure-migration-c357503`).
[PR482](https://gitea.yapplesauce.com/sam/kubernetes-gitops/pulls/482), merge
`7d4833a`, retained it. Release pin `4c787ad` records both artifacts. The scoped
operator Deployment retained its `c75cf38d…` executable and changed its default
VM image to the new manager. No inactive VM was created by that rollout.

The repeated oracle then passed both moves on one connection:

| Move | Maximum observed gap | Conntrack evidence |
| --- | --- | --- |
| kube03 → kube02 | 0.58s | precopy 3, delta 2 |
| kube02 → kube03 | 0.74s | precopy imports recorded, delta 1 |

There were zero liveness-probe failures across the repeat run's 53 Kubernetes
events. The disposable VM and migration objects were removed afterward.

A separate fixed-endpoint test read a 14-byte synthetic reply successfully,
cleared only that endpoint's encryption marker in the client node's BPF entry,
and observed connection timeout plus two `Traffic is unencrypted` egress drops.
The exact original entry was restored, and the same read succeeded again. This
changed no other endpoint or peer key. Paired captures had zero kernel drops:
16 inner synthetic TCP packets were visible on each WireGuard interface, with
zero matching plaintext TCP packets on either physical uplink.

A UDP query to a CoreDNS Pod on kube02 from the kube01 client returned three
successful answers. Paired captures saw six inner DNS packets and zero matching
physical plaintext packets, also with zero kernel drops. No VPN profile was
active during these captures. A later [VPN NIC lab run](../networking.md#vpn-nics)
verified the deployed manager's tunnel, kill switch and migration separately.

The repeated-migration inner captures span both moves with zero drops. Physical
captures observed WireGuard records and no matching plaintext, but two reached
the packet cap and one reported 13,328 drops. They are bounded supporting
samples, not a complete loss-free recording. The independent fixed-endpoint and
DNS captures provide the zero-drop negative/positive comparison. Capture
lengths, counts, hashes and limitations are retained in the JSON proof; payload
files stay separate with restricted access. Bulk migration traffic was why the
initial unfiltered capture exhausted its budget; later captures used kernel
socket filters to isolate test/client-node traffic.

All test entrypoints and workload data were synthetic. No SPIFFE private key,
bearer token or ordinary application payload was exported. The remaining
native iSCSI, BGP, host/uplink and world-origin forwarding links remain open
work under the transport hardening effort.

# VM identity foundation

qrab boots VMs through OVMF and a UEFI boot medium and exposes a swtpm-backed
TPM 2.0 device to Linux. The VM's retained state volume holds its firmware
variables and TPM state. This prepares the guest for TPM-based SPIRE enrollment;
the minimal operator does not provision a CA, issue DevID certificates or install
a guest SPIRE agent. Guest NICs are optional; the bundled demo has none.
This guest identity foundation is separate from the manager Pod's active
SPIFFE identity for migration. See the
[identity connection guide](connections/identity.md) for SVID delivery, exact
peer matching, OIDC roles and the boundaries not enrolled by qrab.

swtpm is a software TPM. Its identity depends on the protection of the hosting
pod, hypervisor and state volume, consistent with the existing native deployment's
accepted trust model. A TPM device, nonzero PCRs and a firmware event log establish
usable interfaces and boot measurements; they do not establish hardware-backed
identity, Secure Boot enforcement or an approved measured-boot policy.

## Offline checks

The RAM-only demo guest in the manager image (`contrib/images/guest-init`)
reports these markers on its serial console:

| Marker | Evidence |
| --- | --- |
| `QRAB_UEFI_BOOTED` | Linux exposes `/sys/firmware/efi` |
| `QRAB_TPM2_READY` | `/dev/tpmrm0` answers capability and SHA-256 PCR 0/4/7/11 requests |
| `QRAB_TPM_EVENTLOG_PRESENT` | `tpm2_eventlog` parses the firmware's binary event log |
| `QRAB_TPM_EK_SHA256=<hex>` | SHA-256 of the deterministic ECC endorsement public blob |

Inside the disposable guest's BMC console:

```sh
test -d /sys/firmware/efi
TPM2TOOLS_TCTI=device:/dev/tpmrm0 tpm2_getcap properties-fixed
TPM2TOOLS_TCTI=device:/dev/tpmrm0 tpm2_pcrread sha256:0,4,7,11
tpm2_eventlog /sys/kernel/security/tpm0/binary_bios_measurements
ls /sys/class/net
```

Compare the EK marker across replacement pods using the same VM state PVC. It
must remain unchanged; a fresh VM must have a different identity. In the no-NIC
demo the network listing should contain only `lo`; a configured guest NIC adds
its own interfaces. Keep the entire TPM state, not just the public
fingerprint. Clearing or replacing it invalidates TPM-bound credentials; copying
it to another VM duplicates that identity.

## Enrollment contract

SPIRE's `tpm_devid` attestor validates an LDevID key and certificate, including
proof that the key resides in an endorsed TPM. It does not enforce the guest's
PCR values. Both sides must be configured, as described by the official
[agent plugin](https://github.com/spiffe/spire/blob/v1.15.3/doc/plugin_agent_nodeattestor_tpm_devid.md)
and [server plugin](https://github.com/spiffe/spire/blob/v1.15.3/doc/plugin_server_nodeattestor_tpm_devid.md).

Before enabling an agent in a guest:

1. Provision an EK certificate matching that virtual TPM and establish an
   explicitly approved endorsement-CA chain at the server. The smoke test's EK
   fingerprint is not an endorsement certificate or an enrollment credential.
2. Generate the guest's LDevID key in its TPM using the pinned SPIRE version's
   storage-parent template, and obtain a certificate from the trusted DevID CA.
   Preserve the certificate, its TPM-bound public/private blobs and the guest's
   agent state across boots. The current RAM-only fixture does not provide that
   guest-side persistence. The agent expects raw `TPMT_PUBLIC`/`TPMT_PRIVATE`
   blobs, rather than the two-byte-length-prefixed TPM2B files emitted by
   `tpm2_create`; follow the
   [provisioning walkthrough](https://github.com/spiffe/spire/blob/main/doc/tpm_deployment_walkthrough.md)
   matched against the deployed agent before creating keys.
3. Configure `/dev/tpmrm0`, the LDevID certificate/blobs and the real owner/key
   authorization values in the agent. The server must trust both the DevID CA
   and the EK's endorsement CA. CA private keys belong outside VM images and CRs.
4. Provide authenticated server transport and a trusted SPIRE bundle, then add
   scoped workload registrations under the new attested agent identity. The
   agent's SPIFFE ID derives from its LDevID certificate fingerprint. A guest
   with no transport cannot complete enrollment or renew SVIDs.

The native deployment was checked read-only on 2026-11-11: SPIRE 1.15.3 uses trust
domain `yapplesauce.com`, server VIP `10.203.0.10:8081`, and `tpm_devid`. Its agents
use `/etc/spire/agent/devid.{pem,pub,priv}` and `/etc/spire/bundle.pem`; servers
trust `/etc/spire/ca/devid-ca.pem` and `/etc/spire/ca/endorsement-ca.pem`. Existing
virtual TPM endorsements were issued by `swtpm-localca`. Those existing trust
bundles do not automatically authorize a newly generated qrab VM identity.

Operational history is in towl `towl/work/deploy-spire-three-kubernetes-vms` and
`towl/work/complete-spire-ha-and-zot-delegation-rollout`. Kubernetes GitOps keeps
the native-node TPM parent mappings explicit. The node/shim's pod-bound image
pull authorization authenticates the host and pod; it does not attest the guest.

## Guest workload identity (design, 2026-11-15)

Status: decided, not yet built. The towl container is
`qrab/container/vm-guest-spiffe-identity`; its implementation tasks are blocked
on `work/guest-identity-mechanism-design`.

### What a guest identity proves

A guest SVID proves two things:
- **Pod-vouched.** The SPIRE-attested VM manager Pod, holding
  `spiffe://yapplesauce.com/kubernetes/qrab/vm/<vm-uid>`, vouched for the
  swtpm it runs.
- **TPM-bound.** The guest agent's key cannot leave that vTPM.

It does **not** prove measured boot, guest integrity, or isolation from node
or hypervisor administrators. The issuer checks no PCR values, and SPIRE's
`tpm_devid` attestor does not either. The guest identity is a different
SPIFFE ID from the Pod's, so the Pod boundary (migration, storage) and the
guest boundary (what code in the VM may do) stay separate.

### SPIFFE IDs

| Subject | ID | Issued to |
| --- | --- | --- |
| Guest SPIRE agent | `spiffe://yapplesauce.com/spire/agent/tpm_devid/<SHA1 of the DevID cert>` | in-guest `spire-agent`, by `tpm_devid` attestation |
| Guest node alias | `spiffe://yapplesauce.com/qrab/guest-node/<vm-uid>` | every agent whose DevID subject CN is `qrab-vm-<vm-uid>` from the qrab DevID CA |
| Guest workload | `spiffe://yapplesauce.com/qrab/guest/<vm-uid>/<name>` | guest processes matching the entry's `unix:`/`systemd:` selectors, under the alias |

### Mechanism: an in-guest SPIRE agent with `tpm_devid` on the vTPM

1. **Issuer.** `qrab-tpm-issuer` is a Rust Deployment in its own namespace.
   - It holds two CAs, the qrab endorsement CA and the qrab DevID CA. Their
     keys live in a Kubernetes Secret that only the issuer's ServiceAccount
     may read. They never go into VM images, VirtualMachine CRs or the manager.
   - It listens only for SPIFFE mTLS. It admits peers whose ID is exactly
     `spiffe://yapplesauce.com/kubernetes/qrab/vm/<uuid>` and signs only for
     that uuid, taken from the peer certificate, never from the request body.
   - A CiliumNetworkPolicy admits only `component=vm` Pods.
2. **Provisioning before first boot.** The manager owns swtpm and works on
   its control socket before QEMU attaches. Only on a TPM with no existing
   identity, it:
   - creates the EK;
   - asks the issuer for an EK certificate and writes it to NV index
     `0x01c00002`;
   - creates the LDevID signing key under the SPIRE `tpm_devid` SRK template
     (RSA 2048, fixedTPM/fixedParent);
   - asks the issuer for a DevID certificate with CN `qrab-vm-<vm-uid>`.

   It stores the key's raw `TPMT_PUBLIC`/`TPMT_PRIVATE` blobs and both
   certificates in the VM's retained state volume. They reach the guest on a
   read-only `qrab-identity` virtio-blk disk. The private blob is useless off
   this vTPM.

   The trust root here is the manager's attested SVID. A credential-activation
   handshake inside the guest would add nothing, because the manager already
   controls the swtpm the guest would use.
3. **SPIRE server.** The native servers on kube01–03 trust the qrab
   endorsement and DevID CA certificates. They are appended to
   `endorsement_ca_path` and `devid_ca_path`, separate from the node CAs, so
   the issuer CN distinguishes guests from nodes. This is an attended native
   change.
4. **Transport, without a guest network.**
   - The manager adds a second virtio-serial port,
     `org.yapplesauce.spire.0`.
   - It splices that port to a TCP connection to the SPIRE server VIP
     `10.203.0.10:8081`; the VM Pod's network policy gains that egress only.
   - In the guest, `qrab-spire-relay`, a separate binary rather than part of
     the allowlist-only guest agent, listens on `127.0.0.1:8081` and pipes to
     the port.
   - The agent→server channel is SPIRE's own mTLS end to end, so the manager
     only moves bytes. The guest's bootstrap bundle comes from the identity
     disk.
5. **Registration.**
   - The qrab operator writes `ClusterStaticEntry` objects for a VM whose spec
     asks for guest identity: one node alias entry
     (`parentID spiffe://yapplesauce.com/spire/server`, selectors
     `tpm_devid:subject:cn:qrab-vm-<vm-uid>` and
     `tpm_devid:issuer:cn:<qrab DevID CA CN>`), plus one entry per requested
     workload under that alias.
   - spire-controller-manager reconciles them; GitOps sets
     `reconcile.clusterStaticEntries: true`.
   - The operator gains create/update/delete on ClusterStaticEntries.
   - A ValidatingAdmissionPolicy bounds that power to the operator's own
     prefixes: spiffeID under `/qrab/guest/<uid>/` or `/qrab/guest-node/<uid>`,
     the parent either the server or that VM's alias, `admin` and `downstream`
     false, and `<uid>` an existing VirtualMachine.
   - The entries stay visible to the IDM dashboard through the Kubernetes API.
6. **Live migration.** TPM state, the identity disk and the guest's agent data
   ride the migration. The agent ID and alias are unchanged. The relay's TCP
   connection is re-opened by the target manager, and the agent reconnects as
   on any server failover. No re-enrollment and no entry change.
7. **Deletion.** The operator removes the VM's ClusterStaticEntries. The guest
   agent's SVIDs lapse at their TTL, and the agent is evicted from SPIRE.

### The qrab CAs (created 2026-11-16)

| CA | Subject | SHA-256 fingerprint |
| --- | --- | --- |
| Endorsement | `CN=qrab-endorsement-ca-1,O=qrab` | `e9a9b4f57875c2e3822aef9c9e28af71c9253a68f3f8eaebc945e8c1bd4eee05` |
| DevID | `CN=qrab-devid-ca-1,O=qrab` | `0cd4e428954c70e631d58d189760093c0496154e13d84fc46101bec486525022` |

Both are ECDSA P-256, self-signed, `pathlen:0`, valid for ten years.

- `contrib/qrab-tpm-ca-create.sh` made them. It generated the keys in tmpfs,
  wrote Secret `qrab-tpm-issuer/qrab-tpm-issuer-ca` (`ek-ca.pem`, `ek-ca.key`,
  `devid-ca.pem`, `devid-ca.key`) and ConfigMap `qrab-tpm-issuer-ca-certs`
  (the two certificates only), then shredded the keys. It refuses to replace
  an existing Secret: rotating a CA invalidates every guest DevID.
- `contrib/spire-trust-qrab-cas.sh <ek sha256> <devid sha256>` appended both
  certificates to `/etc/spire/ca/endorsement-ca.pem` and `devid-ca.pem` on
  kube01–03.
  - It verifies that the ConfigMap's certificates match these fingerprints.
  - It restarts one server at a time, and only while the others are healthy.
  - It keeps `*.before-qrab-<UTC>` backups.
  - Re-running it is a no-op.
- The workload selector for guests is therefore
  `tpm_devid:issuer:cn:qrab-devid-ca-1`.
- Cluster-wide Secret readers can read the CA keys: Flux, cert-manager,
  cilium-operator, the kube-state-metrics and Grafana roles, and the Kanidm
  `kubernetes:admin` group. This is the custody the in-cluster Secret was
  chosen with.

### Rejected alternatives

- **SPIRE Delegated Identity API from a node-level helper.** The agent returns
  an SVID for any selectors the delegate presents. The helper could therefore
  mint every identity on its node, and the guest would be attested only by
  the host's word.
- **Join tokens.** These are bearer secrets minted through the server admin
  API. They need re-minting on agent state loss and bind nothing to the vTPM.
- **Guest NIC egress to the SPIRE VIP.** It exposes the server to guest
  networking and fails for NIC-less VMs. It remains a possible later option.
- **Reusing the node DevID CA.** Guest and node identities would share one
  root and could not be told apart by issuer.
- **PCR-gated issuance.** Deferred: it needs a per-image measured-boot policy.

### Boundaries the IDM dashboard shows

The two columns of a VM's page show:
- **Pod:** the `vm/<uid>` SVID, its peer==self migration tunnel, temporary
  `qrab-migration-*` policies and storage authority.
- **Guest:** the agent ID (L0), alias and workload entries (L1), and what the
  guest workloads may call (L2–L4), with this section's trust statement.

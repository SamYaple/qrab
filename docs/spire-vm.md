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

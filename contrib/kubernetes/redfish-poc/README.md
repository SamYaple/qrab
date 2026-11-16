# Redfish/Secure Boot POC

`qrab-redfish-poc` is the cluster's only VM. Apply the platform (the
digest-pinned operator with its storage client, plus monitoring) first, from a
checkout at current origin/main ([../README.md](../README.md)):

```sh
kubectl apply -k contrib/kubernetes
```

This POC uses fresh identity and Redfish-state claims. Deleted earlier VMs'
retained claims are recovery artifacts and are never adopted, reset or deleted.
The separate `apply-console.sh` binds `console.yaml` to the VM's current immutable
UID and serves it at `qrab-demo.yapplesauce.com`; its Service,
Ingress and CiliumNetworkPolicy keep the name `qrab-demo` (Certificate
`qrab-demo-tls`) from the retired qrab-demo VM, but belong to this bundle.
Private DNS must resolve the host to the Cilium Ingress address.

Before applying the VM, populate its separate read-only media claim with the
signed public guest image produced by
`contrib/experiments/secure-boot-guest-proof.py` against the published image.
Use a temporary restricted copy Pod with explicit CPU/memory limits,
`imagePullPolicy: Always`, and the cluster's control-plane toleration. Delete
that Pod before mounting the RWOP claim into the VM. Copy **only** signed media,
never the private test signing key. Verify its SHA256 before use. The VM points
to `/var/lib/qrab-media/signed-boot.img`; the operator and manager both enforce
that approved media namespace and reject escape to identity or Secrets.

```sh
kubectl apply -k contrib/kubernetes/redfish-poc
bash contrib/kubernetes/redfish-poc/apply-console.sh
```

Start in SetupMode, enroll the public test profile through fixed authenticated
database updates (`db`, `KEK`, `PK`), stage SecureBootEnable, then PowerCycle.
The protocol validates signatures through real firmware; no offline identity
reseed is involved. Keep the test signing key private outside containers/images.
Capture CR/PVC UIDs and public TPM EK before and after each controlled cold cycle.

Read-only acceptance:

```sh
python3 contrib/experiments/redfish-rollout-test.py --vm qrab-redfish-poc \
  --origin http://127.0.0.1:18081 --expect-secure-boot enabled --expect-no-nic \
  --report target/redfish-poc-final.json
```

Use a deliberate loopback port-forward, or the HTTPS origin with private DNS.
`redfish-authenticated-poc.py` performs real Kanidm PKCE checks and holds its
ID token in memory only. Its explicit commands do not retry mutations.

The console helper requires an existing `qrab-redfish-poc` and never creates a
VM or identity claim. It binds both the Service selector and SPIFFE backend IDs
to the observed UID. Run it again after intentionally replacing the VM resource.
BMC8443 and direct native Redfish8444 use ingress mTLS; ordinary HTTP health is
local and probed with exec. [Ingress transport](../../../docs/ingress.md) describes
platform enrollment and the fixed Envoy SDS bootstrap dependency.

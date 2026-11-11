# Native iSCSI guest disks

QEMU connects directly to an iSCSI target through libiscsi and presents the LUN
as a virtio disk. There is no kernel initiator, block PVC, host filesystem or disk
image file in this guest-disk path. The storage host exports an owned ZFS zvol
through Linux LIO. The small filesystem PVC for TPM/UEFI state stays separate.
Guest NICs remain disabled; only the QEMU Pod connects to the storage endpoint.

## VM configuration

Provision the target and its exact VM-UID initiator ACL first. Then create a
namespace Secret containing the exact CHAP password bytes, without a trailing
newline. Passwords never belong in CRs, ConfigMaps, image layers or argv:

```yaml
spec:
  storageNode: kube03
  iscsiDisks:
  - id: data
    portal: 10.203.0.1:3260
    target: iqn.2026-11.com.yapplesauce:qrab.disk.VM-UID
    lun: 0
    username: CHAP-ACCOUNT
    passwordSecret:
      name: vm-disk-password
      key: password
```

Replace the example target/account with the provisioner's report. The operator
derives the initiator as `iqn.2026-11.com.yapplesauce:qrab.vm.<CR UID>`; a CR cannot
supply another initiator. It mounts a readonly Secret file under
`/run/qrab-auth/<id>/password` (0440, fsGroup 10001). QEMU uses a secret object and
`password-secret`; it never receives a credential-bearing URL. `username` is an
explicit nonsecret CHAP account identifier.

IDs are stable guest serials of 1..20 lowercase letters/digits/hyphens. Portal
must be literal IP:port, including brackets for IPv6. Disk endpoint identity is
immutable per ID; the runtime also retains append-only ID bindings beside the
TPM state. Readonly/presentation/credential changes do not repurpose that ID.
Rotate the Secret and change `restartNonce` to load new credentials: existing
QEMU processes do not hot-reload secret objects.

Optional `bootIndex: 0` requests UEFI priority ahead of the bundled ESP; the LUN
must already contain a valid UEFI-bootable OS. Index 1 is reserved for the demo
ESP; other boot indexes must be unique. Ordinary data disks omit it. The initial
live proof uses a disposable 128MiB data LUN; OS installation and generic volume
provisioning are outside the demo installer.

## Ownership and recovery

This first version requires immutable `storageNode` and scheduler nodeSelector.
It does not automatically move a native-disk VM to another node. The target's
exact initiator ACL and CHAP credentials authorize a VM; they alone do not limit
the number of sessions using those credentials. MaxConnections is per session.

Native Pods start with an ownership scheduling gate. The operator removes it
only after the Pod UID is durably recorded, so an untracked process cannot start.
Native VMs and Pods receive drain finalizers. Before replacing/deleting a Pod,
the operator records its confirmed clean exit UID in status, then drops the Pod
finalizer. Parent deletion waits for this cleanup before dropping its own
finalizer. The manager reports forced/nonzero native child termination as a
failure and retains its state lock through actual child reaping. Failed,
vanished or unknown old process state blocks replacement. Timeouts, Pod deletion
and lease expiry are not treated as storage fences.

Use the bounded root target procedure in
[contrib/storage/README.md](../contrib/storage/README.md) to disable/drain the old
export and rotate CHAP before recovery. After verifying that hard fence and
updating the Secret, an administrator can acknowledge the exact old Pod UID:

```sh
kubectl -n qrab patch qvm VM --subresource=status --type=merge \
  -p '{"status":{"storageRecovery":"EXACT-OLD-POD-UID"}}'
```

This field is an operator acknowledgment, not a signed fence receipt. It cannot
be reused for another Pod UID. A failed orphan whose parent was forcibly removed
remains a visible finalizer tombstone; hard-fence first, then explicitly remove
its Pod finalizer. The periodic sweep only collects genuinely VM-owned clean
orphans. There is no general migration workflow in this release.

PVC state locks and RWOP still protect retained identity. Keep the target,
credentials, guest disk and TPM/NVRAM identity together through deliberate
recovery; never copy a TPM volume to make another identity.

## Checks

The image includes native libiscsi; authentication errors fail startup rather
than falling back to a local disk. The normal suite includes config, Secret,
immutable binding, clean/failed/unknown cleanup and orphan-order tests.

`projects/manager/tests/live_iscsi.py` and `contrib/images/iscsi-test-fixture.py`
provide isolated protocol acceptance with a disposable userspace target. That
file-backed fixture is not production ZFS or kernel LIO fencing evidence.

The guest's explicit `qrab-disk-probe initialize|verify [--serial]` helper only
accepts the reserved `qrab-probe` serial and a 64..256MiB disposable LUN.
Initialize refuses existing signatures and requires a full zero scan before
formatting ext4. Verify mounts readonly without journal replay and reports the
filesystem UUID and persisted token hash. No disk is autoformatted at boot.
The test-only `qrab-operator` example `vnc_probe` sends this fixed lowercase
command over the private Unix VNC socket when run through authenticated
`kubectl exec`; it is not an anonymous BMC endpoint or a shipped service.

The deployment-specific demo patch is
`contrib/kubernetes/iscsi-demo.patch.json`. Apply it only after the reviewed root
installer has prepared that exact VM UID's target, fencing has been tested and
the matching `qrab-demo-iscsi` Secret has been created. It is deliberately not
included in the default kustomization: installing runtime capability must not
connect an unprovisioned or unfenced LUN.

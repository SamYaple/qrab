# Pod manager

`manager::pod` runs QEMU and its swtpm child inside an existing container sandbox.
`qrab-bmc` embeds it for authenticated browser access; `qrab-manager` remains a
standalone CLI. [operator.md](operator.md) describes the default CR-driven
Kubernetes deployment, and [bmc.md](bmc.md) describes OIDC and the console.
There is no systemd, host namespace helper or Kubernetes client in the manager.
The older confidential-guest API is opt-in through `legacy-host`.

## Images

- `qrab/qemu`: pinned local-Gitea QEMU 11.1.3, TCG/KVM, VNC/pixman/TPM, swtpm and
  OVMF CODE/VARS under `/opt/qrab/firmware`. QEMU source/provenance stays local.
- `qrab/manager`: Rust supervisor and BMC plus the readonly GPT/FAT ESP demo UKI,
  kernel, initramfs and TPM2 guest probes under `/opt/qrab/demo`.
- `qrab/operator`: the small namespace-scoped Rust reconciler without QEMU or
  guest assets. Its Kubernetes API token is separate from the token-free VM Pod.

All run as UID/GID 10001. The default VM uses TCG, read-only container rootfs,
RuntimeDefault seccomp and zero capabilities. KVM remains a standalone explicit
option requiring provisioned `/dev/kvm`; the operator does not expose it yet.
Published immutable references and source revision are in
`contrib/images/images.lock.json`.

## Runtime configuration

Config is strict JSON. The example `contrib/kubernetes/vm.json` uses:

```json
{
  "name": "qrab-demo",
  "memory_mib": 512,
  "cpus": 1,
  "accelerator": "tcg",
  "console": true,
  "uefi": {},
  "tpm": {},
  "state_dir": "/var/lib/qrab",
  "runtime_dir": "/run/qrab"
}
```

`uefi` defaults to OVMF CODE/VARS templates and `/opt/qrab/demo/boot.img`.
`uefi.boot_disk` can select another readonly image. UEFI mode rejects direct
`kernel`, `initrd` and `kernel_args`; the UKI contains the kernel command line and
initramfs. Existing standalone direct-kernel JSON remains supported when UEFI is
absent. An optional separate `disk` accepts an absolute file or block device,
`format` raw/qcow2 and `read_only`. Blockdev JSON handles commas/spaces safely.
Guest networking is always disabled with `-nic none`.

`tpm` defaults to `/usr/bin/swtpm`, with private Unix control and TPM2 CRB in
QEMU. `state_dir` must be writable persistent storage; its owner lock prevents
concurrent supervisors. `identity/OVMF_VARS.fd`, `identity/tpm/` and a manifest
are initialized atomically, then reused. State binds the VM name and `state_id`
(defaults to name); the operator sets the immutable CR UID. Private directories
are 0700 and files 0600. Corrupt/missing initialized TPM state or a mismatched
manifest fails closed instead of silently resetting identity.

The readonly image's ESP is separate from mutable VARS/TPM state. See
[spire-vm.md](spire-vm.md) before enrolling a VM: presence of TPM2 and an event log
alone is not trusted SPIRE node authentication.

## CLI, probes and lifecycle

```sh
qrab-manager check --config /etc/qrab/vm.json
qrab-manager run --config /etc/qrab/vm.json
qrab-manager status --config /etc/qrab/vm.json
qrab-manager ready --config /etc/qrab/vm.json
qrab-manager live --config /etc/qrab/vm.json
```

Use persistent `/var/lib/qrab` and a writable `/run/qrab` mount in standalone
containers. A private runtime session owns QMP, VNC and swtpm sockets. QEMU starts
paused, the manager verifies QMP, then resumes it. Serial output goes to stdout;
manager/BMC lifecycle records go to stderr. No raw QMP/VNC TCP listener is opened.

A one-second QMP heartbeat atomically writes `status.json`. Probes validate
freshness and process start times for manager, QEMU and swtpm. The CLI's `ready`
requires QMP running; `live` permits paused guests. BMC HTTP `/readyz` uses live
semantics so an operator can resume a paused VM. Process readiness does not
prove the guest has finished booting; the demo serial probes provide that
separate evidence.

SIGTERM/SIGINT or BMC powerdown attempts ACPI, waits the configured guest grace,
then QMP quit and bounded kill/reap. Both children are owned and reaped. A TPM
crash fails the VM and reaps QEMU. Cancellation uses kill-on-drop cleanup. Pod
grace must exceed guest grace plus command/kill bounds; the example uses 20s.
The tiny init has no ACPI handler, so normal shutdown may use QMP quit. The
operator's `running:true` policy creates another instance after exit; use the CR
to keep a VM off.

## Build and publish

From clean committed source, with `vendor/qemu` initialized from local Gitea:

```sh
tag=$(git rev-parse --short=12 HEAD)
contrib/images/build.sh qemu "11.1.3-$tag"
contrib/images/publish.sh qemu "11.1.3-$tag"
contrib/images/build.sh manager "$tag"
contrib/images/publish.sh manager "$tag"
contrib/images/build.sh operator "$tag"
contrib/images/publish.sh operator "$tag"
```

QEMU preparation verifies the mirror/revision and vendored Meson input snapshots.
Configure uses `--disable-download`. Base images are digest-pinned; apt package
versions are recorded in image provenance. Publication mints fresh scoped Zot
credentials immediately before each push and verifies the immutable remote
manifest. Credentials never enter images/manifests. Pin the resulting refs and
operator default VM image in a subsequent data-only commit; unchanged code need
not be rebuilt for that pin commit. Namespace/ServiceAccount pull identities
remain separately managed by Kubernetes GitOps.

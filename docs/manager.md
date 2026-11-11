# Pod manager

`qrab-manager` runs one x86-64 QEMU VM inside an existing container sandbox.
It uses `manager::pod` and `qapi-protocol` directly: no systemd, host namespace
helper, Kubernetes API client, guest networking or VM controller is involved.
The older confidential-guest `VmBuilder` API remains separate from this path.

## Images

- `qrab/qemu`: QEMU 11.1.3 built from the pinned local Gitea source. TCG and KVM
  are compiled in; only the x86-64 system emulator is built. Firmware and source
  provenance are included under `/opt/qemu` and `/usr/share/doc/qrab-qemu`.
- `qrab/manager`: extends an immutable QEMU image and adds the Rust supervisor.
  A tiny, RAM-only Linux guest is included as a reproducible boot fixture.

Both run as UID/GID 10001. The manager is the container's main process and owns
QEMU directly. The sample pod has a read-only root filesystem, drops all
capabilities, uses RuntimeDefault seccomp, mounts no API token, and needs only
writable `emptyDir` volumes. It uses TCG so it can run without a privileged pod
or a KVM device plugin. KVM is an explicit config option and requires separately
provisioned access to `/dev/kvm`; the sample does not configure that access.

The demo contains a Debian cloud kernel and a static BusyBox initramfs. It
prints `QRAB_GUEST_BOOTED` and the guest kernel version to the serial console,
then remains running. It has no guest NIC or persistent root filesystem.
Its package versions are recorded in `/opt/qrab/demo/guest-packages.txt`.

## Configuration and commands

See `contrib/kubernetes/vm.json` for a complete example. Config is strict JSON;
unknown fields fail. Required `kernel` and optional `initrd` paths must be
absolute existing files. Memory is in MiB, CPUs are explicit, and the default
machine is q35. `accelerator` is `tcg` or `kvm`. Optional `disk` accepts an
absolute regular-file or block-device path, `format` (`raw` or `qcow2`) and
`read_only`. Blockdev JSON keeps filenames containing commas/spaces unambiguous.
`firmware_dir` optionally overrides QEMU's data directory for local testing.

```sh
qrab-manager check --config /etc/qrab/vm.json
qrab-manager run --config /etc/qrab/vm.json
qrab-manager status --config /etc/qrab/vm.json
qrab-manager ready --config /etc/qrab/vm.json
qrab-manager live --config /etc/qrab/vm.json
```

The runtime directory has an exclusive kernel-backed owner lock and a private
per-run QMP socket directory. Stale lock files do not block a container restart;
active concurrent managers are refused. QEMU starts paused, the manager checks
its QMP version and resumes it. Serial output goes to container stdout; lifecycle
records go to stderr as JSON.

A one-second QMP heartbeat writes `status.json` by atomic replacement. Probes
read that snapshot and verify freshness plus manager/child PID start times;
they never compete for the monitor's single connection. Readiness means QMP
reports running, not that guest applications have started. Liveness permits a
paused VM. Guest boot is checked separately through the serial marker.

SIGTERM/SIGINT attempts ACPI powerdown, waits `shutdown_grace_secs` (default 5),
then uses QMP quit and finally SIGKILL if needed. Every explicit path waits and
reaps the child. Cancellation of the library future uses kill-on-drop cleanup.
The pod termination grace must exceed the configured guest grace plus roughly
ten seconds for command/final-reap bounds; the example uses 20 seconds. The
minimal demo init has no ACPI userspace handler, so its normal manager shutdown
falls back to QMP quit. Real guests can handle the ACPI request themselves.
Unexpected QEMU failure makes the manager exit nonzero. The sample pod uses
`restartPolicy: Never` so failures stay visible instead of looping.

## Build and publish

From a clean committed checkout with initialized `vendor/qemu`:

```sh
tag=$(git rev-parse --short=12 HEAD)
contrib/images/build.sh qemu "11.1.3-$tag"
contrib/images/publish.sh qemu "11.1.3-$tag"
contrib/images/build.sh manager "$tag"
contrib/images/publish.sh manager "$tag"
```

QEMU preparation verifies the local mirror, git revision, three small source
snapshot checksums, and their matching Meson wrap revisions. Compilation uses
`--disable-download`; no QEMU or subproject clone happens during the image build.
The dependency source archives and their licenses live in `vendor/qemu-build`.
Base Debian/Rust images are digest-pinned; distribution build packages are
resolved by apt. Published artifact digests are the deployment authority.

Publication exchanges a fresh delegated Zot credential immediately before each
individual push and verifies the remote manifest. No credentials enter images
or manifests. Manager builds require a digest-pinned QEMU base, taken from
`target/images/qemu.ref` or `QEMU_IMAGE`. The release pin commit records the
published references in `contrib/images/images.lock.json` and the pod manifest;
it does not require rebuilding unchanged application code.

## Kubernetes launch

The `qrab` namespace and stable `qrab-manager` ServiceAccount are enrolled through
Kubernetes GitOps. The cluster's normal pull-audience/reviewer RBAC and UID-bound
native policies must be installed first; do not replace that path with static
image pull passwords. The manifest uses `imagePullPolicy: Always` so every start
checks its pod-bound pull authorization.

```sh
kubectl apply -k contrib/kubernetes
kubectl -n qrab wait --for=condition=Ready pod/qrab-demo --timeout=120s
kubectl -n qrab logs qrab-demo
kubectl -n qrab exec qrab-demo -- qrab-manager status --config /etc/qrab/vm.json
kubectl -n qrab delete pod qrab-demo --timeout=30s
```

Reapply to launch a new VM. The namespace/ServiceAccount must retain their UIDs;
they are separate, never-pruned GitOps identity resources. There is no Deployment,
VM CRD, service or controller in the demo. To use a different kernel/initramfs or
disk, mount the files and update the ConfigMap; configuration is read at launch,
not hot-reloaded. Networking, production storage policy, device plugins and a
controller remain subsequent work.

## Validation

`cargo test -p manager --test pod_runtime` exercises config/argument handling,
early exits, startup timeout, cancellation, ownership and child reaping. The
ignored live CLI test boots the same fixture, validates readiness, attaches a
read-only disk whose filename contains commas/spaces, sends SIGTERM and checks
that both manager and child exit. Set `QRAB_TEST_QEMU`, `QRAB_TEST_QEMU_DATA` and
`QRAB_TEST_GUEST` to extracted image artifacts, then run with `--ignored`.

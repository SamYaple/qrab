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

`tpm` defaults to `/usr/bin/swtpm`, with private Unix control and TPM2 TIS in
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

## Control API

`manager::pod::Control` is the source control API; the in-process BMC and the
control socket are thin transports over it. It provides `status()`,
`config_view()` (use `ConfigView::redacted()` for less-privileged users),
`events(after, limit)`, `wait_events(cursor)`, `action(..)` and `console()`.
`control_channel_for(&config, Deployment::from_env())` makes the configuration
view available before QEMU starts; `Deployment::from_env()` reads the optional
`QRAB_NODE_NAME`, `QRAB_POD_NAME`, `QRAB_POD_NAMESPACE` and `QRAB_IMAGE`.

The supervisor records every QMP event in a bounded history: 256 events,
oldest evicted first, payloads over 8 KiB replaced by `null` with
`truncated: true`, and names that are not QAPI-shaped rejected and counted.
Sequence numbers start at 1 and increase by one per event for the channel's
lifetime, so `missed` in a page reports eviction between a cursor and the
oldest retained event. `VmEvent::decode::<qapi::events::ResetEvent>()` and the
other generated event types give typed payloads.

### Control socket

With `"control_socket": true` the supervisor serves the API on
`{runtime_dir}/control/control.sock` while it owns the runtime lock, so a BMC
or Redfish sidecar can share the pod's runtime emptyDir. The directory is 0700
and the socket 0600; peers must have the manager's UID. Eight connections
(four console streams) are served at once; further connections receive a
`busy` error and are closed. Requests are limited to 64 KiB and responses to
4 MiB; an idle connection closes after 60 seconds and a blocked write after
5 seconds. The socket is removed when the supervisor exits. It never exposes
QMP or a socket path.

The protocol is newline-delimited JSON, version 1 (`pod::socket` has the
types and a typed `Client`). On connect the server sends
`{"protocol":"qrab-control","version":1,"vm":"<name>","operations":[...]}`.
Each request is `{"v":1,"id":<u64>,"op":"<op>",...}` and each reply
`{"v":1,"id":<id>,"result":...}` or
`{"v":1,"id":<id>,"error":{"code":"<code>","message":"..."}}` with codes
`bad_request`, `unsupported_version`, `frame_too_large`, `busy` and
`unavailable`. A connection may carry any number of requests.

| `op` | Arguments | Result |
| --- | --- | --- |
| `status` | | The `status.json` record |
| `config` | `redacted` (default false) | `ConfigView` |
| `events` | `after`, `limit` (≤256) | `EventPage` |
| `action` | `action`: `pause`/`resume`/`reset`/`powerdown` | `null` |
| `follow` | `after` (default: latest) | `{"after":n}`, then the connection streams `{"v":1,"event":{...}}` and `{"v":1,"missed":n}` frames until the client closes or sends anything |
| `console` | | `null`, then the connection is the raw VNC (RFB) byte stream |

The socket trusts its UID-restricted peer. A sidecar serving users enforces
its own authentication and requests `redacted` views for less-privileged
users, as the BMC does.

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

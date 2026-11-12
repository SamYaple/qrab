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
  kernel, initramfs (with `qrab-guest-agent`) and TPM2 guest probes under `/opt/qrab/demo`.
- `qrab/operator`: the small namespace-scoped Rust reconciler without QEMU or
  guest assets. Its Kubernetes API token is separate from the token-free VM Pod.

All run as UID/GID 10001. The default VM uses TCG, read-only container rootfs,
RuntimeDefault seccomp and zero capabilities. KVM requires an openable
`/dev/kvm`; operator VMs opt in with `spec.accelerator: kvm` and receive it from
the qrab KVM device plugin through CDI (see operator.md).
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

### In-place growth (`hotplug`)

An optional `hotplug` object gives QEMU boot-time headroom and lets the
supervisor grow a running guest. It has no Kubernetes client; it learns the
desired size from files and gates on the container's own cgroup:

```json
"hotplug": {"max_cpus": 4, "max_memory_mib": 2048, "limits_dir": "/run/qrab-size",
            "cpu_overhead_millis": 1000, "memory_overhead_mib": 512}
```

- QEMU starts with `-smp cpus=N,maxcpus=max_cpus` and, when `max_memory_mib`
  exceeds `memory_mib`, `-m size=M,maxmem=…` plus a `virtio-mem-pci` device
  (`hotplug-memory0`, 2 MiB blocks, `requested-size=0`) on a `memory-backend-ram`.
  Unplugged virtio-mem blocks are reserved address space, not RSS. QEMU 11.1.3
  q35 accepts `maxmem` without DIMM slots.
- `limits_dir` holds the downward-API files `cpu_limit_millis` and
  `memory_limit_mib`. Desired guest size = limit − overhead (`cpu_limit/1000 − 1`
  vCPUs, `memory_limit − 512` MiB in operator Pods).
- Each one-second heartbeat also reads `cgroup_dir` (default `/sys/fs/cgroup`)
  `cpu.max` and `memory.max`. Growth is `min(desired, applied cgroup, maxima)`
  and never shrinks. vCPUs are added with `query-hotpluggable-cpus` +
  `device_add` (free slots in topology order, id `qrab-vcpuN`); memory with
  `qom-set /machine/peripheral/hotplug-memory0 requested-size`. `qom-get size`
  reports what the guest driver actually plugged.
- `status.json` carries `resize`: boot, maximum, desired, permitted, current,
  `plugged_memory_mib`, a `phase` (`idle`, `waiting_for_limits`, `retrying`,
  `restart_required`, `limits_unavailable`) and a `detail` such as waiting for
  cgroup limits or "exceeds the boot-time maximum; a restart is required". Each
  growth logs a `guest_resized` stderr record. Hot-plug failures do not stop
  the VM.
- A failed `device_add`/`qom-set` step is retried with capped exponential
  backoff (1 s, 2 s, 4 s ... 60 s) rather than on every heartbeat. `resize`
  reports `failures`, `last_error` and `retry_after_unix_ms`. Each failure logs
  a `guest_resize_failed` record. A change in the container limits retries
  immediately, and a successful step clears the failure state.
- `Control::config_view()` includes `hotplug`: the boot-time maxima plus the
  last published phase, current and desired size and last error. It is not
  sensitive, so the redacted viewer view and the BMC show it too.

The kubelet updates downward-API files before cgroup limits (and even while a
resize is `Deferred`), so the cgroup check is the safety gate. The guest must
online the new resources: the demo initramfs loads `virtio_mem` (Debian 6.12
auto-onlines its memory) and, having no udev, onlines hot-added CPUs and
memory blocks in a background loop. It onlines only devices that appear after
boot, once each, so a CPU or block an administrator offlines inside the guest
stays offline. The loop prints
`QRAB_GUEST_RESOURCES cpus=N memory_kib=M` on serial.

**vCPU hot-plug requires a hypervisor-identified guest.** Linux 6.12.116
(`acpi_is_processor_usable`) only registers QEMU's disabled MADT entries as
hot-pluggable when the guest is not on bare metal; under TCG the hypervisor
type is NATIVE (QEMU's `TCGTCGTCGTCG` signature is unknown), so the guest logs
`Allowing 1 present CPUs plus 0 hotplug CPUs` and ignores `device_add`
(`hypervisor=on` does not help). With KVM (`Hypervisor detected: KVM`) it
allows the hot-plug CPUs and onlines them. Memory hot-plug works under both.
The operator therefore renders `max_cpus = cpus` for its TCG VMs and vCPU
headroom only for `accelerator: kvm`.

Verified 2026-11-11 with the published QEMU 11.1.3/OVMF/demo-guest image, the
updated demo initramfs and `qrab-manager run` (direct kernel boot, KVM) with
simulated limit/cgroup files; the same QMP sequence was also checked by hand on
the OVMF/UKI `boot.img` under KVM. Limits asking for 3 vCPUs/1024 MiB while
`cpu.max=200000 100000` held the guest at 1/512 with "waiting for the kubelet";
raising `cpu.max` to `400000 100000` and `memory.max` to 1536 MiB produced
`guest_resized cpus=3 memory_mib=1024`, `plugged_memory_mib: 512` and guest
serial `QRAB_GUEST_RESOURCES cpus=3 memory_kib=930352` (from 406064). Under TCG
the same flow grew memory (`memory_kib` 400948 → 925236) but not CPUs.

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

Events are drained by their own task as QMP delivers them, independent of
request dispatch and the heartbeat, through a 1024-event client queue. Should
QMP's client still discard events (full queue or an event over 1 MiB), the
count is reported as `qmp_dropped` in every page, refreshed after each
dispatched request and on the heartbeat.

`status()` returns the record the supervisor last published, from memory
(`watch_status()` follows changes); before this channel's supervisor publishes,
it reads `status.json`. Once the supervisor starts stopping QEMU, every request
fails at once with `Error::ShuttingDown` (a repeated powerdown succeeds)
instead of waiting for the 5 s request timeout.

### Control socket

With `"control_socket": true` the supervisor serves the API on
`{control_dir}/control.sock` while it owns the runtime lock. `control_dir`
defaults to `{runtime_dir}/control`; operator Pods use
`/run/qrab-control/manager` on a volume shared only with sidecars. The manager
creates the directory itself and refuses one owned by another UID. The directory is 0700
and the socket 0600; peers must have the manager's UID. Sixteen connections
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
`bad_request`, `unsupported_version`, `frame_too_large`, `busy`,
`unavailable` and `shutting_down`; clients must accept unknown codes from
newer managers. A connection may carry any number of requests.

| `op` | Arguments | Result |
| --- | --- | --- |
| `status` | | The `status.json` record |
| `probe` | `ready` (default false) | `null` when the manager's live (or ready) probe passes |
| `config` | `redacted` (default false) | `ConfigView` |
| `events` | `after`, `limit` (≤256) | `EventPage` |
| `action` | `action`: `pause`/`resume`/`reset`/`powerdown` | `null` |
| `follow` | `after` (default: latest) | `{"after":n}`, then the connection streams `{"v":1,"event":{...}}`, `{"v":1,"missed":n}` and `{"v":1,"qmp_dropped":n}` (on change) frames until the client closes or sends anything |
| `console` | | `null`, then the connection is the raw VNC (RFB) byte stream |
| `agent` | `agent`: `{"command":"status"\|"ping"\|"freeze_status"\|"thaw"\|"update_status"}`, `{"command":"freeze","mountpoints":[..]}` (optional list) or `{"command":"update_trigger","args":[..]}` | The matching `Control::agent_*` result (see [Guest agent](#guest-agent)) |
| `agent_status` | | shorthand for `agent` `status`: cached `AgentStatus` (`enabled`, `connected`, `version`, timestamps, `last_error`); never queues |
| `agent_freeze_status` | | `"frozen"` or `"thawed"` |
| `agent_freeze` | `mountpoints` (default: all) | number of frozen filesystems |
| `agent_thaw` | | number of thawed filesystems |
| `agent_update_trigger` | `args` (≤16 tokens) | the guest `UpdateStatus`; invalid arguments are `bad_request` |
| `agent_update_status` | | the guest `UpdateStatus` |

The flat `agent_*` operations are equivalent to the nested `agent` form
(`qrab-manager agent` uses the nested form; `pod::socket::Client` has typed
methods for both). Guest-agent commands other than status fail at once while the agent is
disabled or not connected (for example before QEMU starts).

`pod::Remote` is the sidecar client. It keeps a mirror of the event history,
with the manager's sequence numbers, over one `follow` connection that
reconnects every second. `pod::Backend` puts either an in-process `Control` or
a `Remote` behind one API for the BMC and Redfish.

The socket trusts its UID-restricted peer. A sidecar serving users enforces
its own authentication and requests `redacted` views for less-privileged
users, as the BMC does.

## Guest agent

`guest_agent` (default `true`) adds `virtio-serial-pci` and a
`virtserialport` named `org.qemu.guest_agent.0` backed by
`socket,id=qga0,path=<session>/qga.sock,server=on,wait=off`. The socket is in
the private 0700 session directory beside QMP; it is never on TCP. The
channel is harmless when no agent runs in the guest.

After QMP connects, the supervisor starts one guest-agent task owning a
`qapi-protocol` QGA connection to that socket. QEMU accepts the socket even with
no agent, so absence shows as a 3-second handshake timeout; the task retries
every 2 seconds, pings every 10 seconds, and resynchronizes with
`guest-sync-delimited` after any timeout or desynchronizing failure. A valid
agent error leaves the session connected. The task ends with the QEMU child.

`manager::pod::Control` exposes it:

| Method | Effect |
|---|---|
| `agent_status()` | Cached `AgentStatus`: `enabled`, `connected`, `version`, connect/last-seen times, `last_error`. Never waits on the guest. |
| `agent_ping()` | `guest-ping`. |
| `agent_freeze(None \| Some(mountpoints))` | `guest-fsfreeze-freeze` or `-freeze-list`; returns the number frozen. |
| `agent_thaw()`, `agent_freeze_status()` | Thaw all, or report `frozen`/`thawed`. |
| `agent_update_trigger(args)`, `agent_update_status()` | Start or inspect the guest's fixed update hook. |

With `control_socket` enabled, the same requests are available as the
socket's `agent` operation and from inside the manager container:

```sh
qrab-manager agent status --config /etc/qrab/vm.json
qrab-manager agent freeze --config /etc/qrab/vm.json [MOUNTPOINT...]
qrab-manager agent thaw --config /etc/qrab/vm.json
qrab-manager agent update-trigger --config /etc/qrab/vm.json channel=stable
qrab-manager agent update-status --config /etc/qrab/vm.json
```

Requests fail immediately when the agent is not connected, its four-slot
queue is full, or the channel is disabled. They have a 30-second control
deadline (a 20-second QGA command bound plus one reconnect attempt). Update
arguments are validated host-side with the agent's own bound before reaching
the guest. As with QMP, a timeout never proves a command did not run: the guest
watchdog thaws an abandoned freeze. See [guest-agent.md](guest-agent.md) for
the guest daemon, its allowlist and the update-hook contract.

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

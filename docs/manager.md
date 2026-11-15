# Pod manager

`manager::pod` runs QEMU and its swtpm child inside an existing container sandbox.
`qrab-bmc` embeds it for authenticated browser access; `qrab-manager` remains a
standalone CLI. [operator.md](operator.md) describes the default CR-driven
Kubernetes deployment, and [bmc.md](bmc.md) describes OIDC and the console.
There is no systemd, host namespace helper or Kubernetes client in the manager.
The retired host/confidential-guest API is archived; see [legacy.md](legacy.md).

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
Without `nics` QEMU runs with `-nic none`. Each entry of the optional
`nics: [{"id","mac","host","port"}]` becomes a JSON
`-netdev {"type":"stream","id":"nic-<id>","server":false,"addr":{"type":"inet",...},"reconnect-ms":1000}`
and `-device virtio-net-pci,id=nic-<id>,netdev=nic-<id>,mac=<mac>,romfile=`.
`id` is a DNS label, `mac` is unicast and locally administered, and `host` is
an IP literal. The relay at `host:port` is `qrab-manager nic` in a separate
NIC Pod. The config view shows each NIC's id and MAC, and its endpoint to
admins only; `guest_network` is true when NICs exist. See
[networking.md](networking.md).

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

### cloud-init seed (`seed`)

An optional `seed` gives the guest a cloud-init
[NoCloud](https://docs.cloud-init.io/en/latest/reference/datasources/nocloud.html)
volume, so it configures itself (hostname, user-data, later static network)
without DHCP or a metadata service:

```json
"seed": {"meta_data": "{\"instance-id\":\"<uid>\",\"local-hostname\":\"web-1\"}\n",
         "user_data": "#cloud-config\n...", "network_config": "version: 2\n..."}
```

- On every start the manager writes a 1 MiB FAT12 image labelled `cidata`
  with the files `meta-data`, `user-data` (an empty `#cloud-config` when
  `user_data` is absent) and, only when present, `network-config`. It lives in
  the private per-start session directory under `runtime_dir` (tmpfs in
  operator Pods), mode 0600 and owned by the manager user, and disappears with
  the session. The image is a pure function of the input: fixed FAT
  timestamps, no clock.
- QEMU attaches it read-only, with JSON `-blockdev` file/raw nodes `seed-file`
  and `seed` and `-device virtio-blk-pci,drive=seed,id=seed`, with no
  `bootindex`. The guest finds it by label. Without `seed` the argv is
  unchanged.
- `meta_data` is required. The three files total at most 64 KiB; validation
  refuses a larger seed before QEMU starts.
- The config view reports `seed: {hostname, user_data_bytes, network_config}`.
  `hostname` comes from `local-hostname` in meta-data, and `network_config` is
  a boolean. user-data may hold secrets, so no view shows it, admin views
  included; only its size is reported.

The label is lowercase `cidata`. cloud-init accepts either case, but CirrOS
matches `blkid -t LABEL=cidata` exactly. CirrOS also parses meta-data as
JSON only, which is why the operator writes JSON.

Local proof, 2026-11-13: CirrOS 0.6.3 booted under OVMF/KVM in the manager
image, with a seed built by `Seed::write` and attached with the arguments
above. It mounted the seed through its `nocloud` datasource and set the
hostname from `local-hostname`: the VGA console showed `seedtest login:` and
`/etc/hostname` was `seedtest`. It recorded instance `0b7c4e6a-seed-proof`
from meta-data and ran the `#!/bin/sh` user-data
(`/var/lib/cirros/sem/instance.0b7c4e6a-seed-proof.userdata`). An identical
image labelled `CIDATA` was ignored (`cirros login:`). CirrOS has no NIC
configured from the seed, so it still waits about three minutes for dhcpcd to
time out before it continues.

## CLI, probes and lifecycle

```sh
qrab-manager check --config /etc/qrab/vm.json
qrab-manager run --config /etc/qrab/vm.json
qrab-manager status --config /etc/qrab/vm.json
qrab-manager ready --config /etc/qrab/vm.json
qrab-manager live --config /etc/qrab/vm.json
qrab-manager nic --listen 0.0.0.0:7700 --socket /run/nic/passt.sock
qrab-manager nic-ready --socket /run/nic/passt.sock
```

`nic` and `nic-ready` are the NIC Pod's relay and readiness probe
([networking.md](networking.md)). The manager image carries a patched passt
at `/usr/bin/passt` for them.

Use persistent `/var/lib/qrab` and a writable `/run/qrab` mount in standalone
containers. A private runtime session owns QMP, VNC and swtpm sockets. QEMU starts
paused, the manager verifies QMP, then resumes it. Serial output goes to stdout;
manager/BMC lifecycle records go to stderr. No raw QMP/VNC TCP listener is opened.

A one-second QMP heartbeat atomically writes `status.json`. Probes validate
freshness and process start times for manager, QEMU and swtpm. The CLI's `ready`
requires QMP running; `live` permits paused guests. BMC HTTP `/readyz` uses live
semantics so an operator can resume a paused VM. A powered-off VM (phase `off`)
and one being powered on, off or cycled in place (`starting`/`stopping` with
`relaunch: true`) is `live` while its manager heartbeat is fresh, but never
`ready`: its Pod stays Ready, so the BMC and Redfish remain reachable to power
it on. Process readiness does not prove the guest has finished booting; the
demo serial probes provide that separate evidence.

### Power lifecycle

The manager is the VM's init. The runtime lock, control socket, persistent
state lease and event numbering (one epoch) last for the whole manager
process, across every QEMU it starts. Each power-on cycle creates a fresh
session directory (QMP, VNC, QGA and swtpm sockets), applies staged firmware
and Secure Boot settings, starts swtpm and waits for it, then runs QEMU.
A live-migration target receives its guest only in the first cycle.

| Event | Result |
| --- | --- |
| `powerdown` (Redfish GracefulShutdown) | ACPI, the configured guest grace, then QMP quit and bounded kill; the VM rests `off` |
| The guest powers itself off (QEMU exits 0) | `off` |
| `forceoff` | QMP quit, SIGKILL after 2 s; `off` |
| `on` while `off` | a new cycle: new QEMU and swtpm PIDs, same TPM identity, NVRAM and disks |
| `coldrestart` (PowerCycle) | QMP quit and reap, then a new cycle at once; while `off` it powers on |
| SIGTERM/SIGINT (Pod deletion, `spec.running: false`) | stop as for `powerdown` (or at once while `off`), then the manager exits |
| QEMU exits non-zero or by a signal we did not send, swtpm dies, or QMP is lost while QEMU keeps running | the manager fails and exits; the operator replaces the Pod |
| A completed outgoing live migration | the manager exits (`migrated`) |

While `off`, `status` reports phase `off` with no QEMU or swtpm PID and the
heartbeat continues. Requests that need QEMU (metrics, inventory, media,
console, guest agent, pause/resume/reset/NMI/reboot) fail at once with
`Error::PoweredOff` (`powered_off` on the socket); `powerdown` and `forceoff`
succeed as no-ops, and `on` while QEMU runs is refused. Firmware and Secure
Boot staging work while off and take effect at the next power on. Each
transition records a `QRAB_POWER_STATE` event (data
`{"power_state": "Off"|"PoweringOn"|"On", "reason": ...}`, no QEMU
timestamp) in the same history as QMP events. A native iSCSI VM whose QEMU
needed SIGKILL still fails the manager, since clean disk release is not
proven; a clean stop logs out and the next QEMU logs in again with the same
initiator name, so the Pod, its exported LUNs and the operator's storage
guard are untouched. A powered-off VM cannot live-migrate.

All children are owned and reaped. Cancellation uses kill-on-drop cleanup. Pod
grace must exceed guest grace plus command/kill bounds; the example uses 20s.
The tiny init has no ACPI handler, so a graceful shutdown may use QMP quit.

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
oldest retained event. Every page also carries the channel's `epoch`: 16
lowercase hex digits chosen at random when the channel is created (once per
manager process), so sequences are comparable only within one epoch and a
restarted manager is recognised whatever its sequence values. The field is
optional on the wire: a manager without it omits it, and readers treat a
missing epoch as unknown. `VmEvent::decode::<qapi::events::ResetEvent>()` and the
other generated event types give typed payloads.

Events are drained by their own task as QMP delivers them, independent of
request dispatch and the heartbeat, through a 1024-event client queue. Should
QMP's client still discard events (full queue or an event over 1 MiB), the
count is reported as `qmp_dropped` in every page, refreshed after each
dispatched request and on the heartbeat.

`status()` returns the record the supervisor last published, from memory
(`watch_status()` follows changes); before this channel's supervisor publishes,
it reads `status.json`. While the supervisor stops QEMU, every request fails
at once with `Error::ShuttingDown` (a repeated power off succeeds) instead of
waiting for the 5 s request timeout; while the VM is off, requests that need
QEMU fail with `Error::PoweredOff`. A new power-on cycle accepts requests again.
`qmp_dropped` counts drops across every QEMU the manager has run.

### Control socket

With `"control_socket": true` the supervisor serves the API on
`{control_dir}/control.sock` while it owns the runtime lock. `control_dir`
defaults to `{runtime_dir}/control`; operator Pods use
`/run/qrab-control/manager` on a volume shared only with sidecars. The manager
creates the directory itself and refuses one owned by another UID. The directory is 0700
and the socket 0600; peers must have the manager's UID. Sixteen connections
(four console streams) are served at once; further connections receive a
`busy` error and are closed. Requests are limited to 384KiB (bounded authenticated EFI updates) and responses to
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
newer managers. A connection may carry any number of requests, answered in
order. A client that closes its connection (including only its write side)
while a read-only request is pending abandons that request, and the manager
frees the connection slot at once. Actions, guest writes, staging and media
changes run to their outcome whether or not the client is still waiting.

| `op` | Arguments | Result |
| --- | --- | --- |
| `status` | | The `status.json` record |
| `probe` | `ready` (default false) | `null` when the manager's live (or ready) probe passes |
| `config` | `redacted` (default false) | `ConfigView` |
| `metrics` | | `VmMetrics`: timestamp, logical processor count, configured RAM bytes and aggregate guest-facing block counters; one-second cache, two-second QMP deadline |
| `events` | `after`, `limit` (≤256) | `EventPage` |
| `action` | `action`: `pause`/`resume`/`reset`/`powerdown`/`nmi`/`reboot`/`coldrestart`/`on`/`forceoff` | `null` |
| `follow` | `after` (default: latest) | `{"after":n}`, then the connection streams `{"v":1,"event":{...}}`, `{"v":1,"missed":n}` and `{"v":1,"qmp_dropped":n}` (on change) frames until the client closes or sends anything |
| `console` | | `null`, then the connection is the raw VNC (RFB) byte stream |
| `agent_status` | | cached `AgentStatus` (`enabled`, `connected`, `version`, timestamps, `last_error`); never queues |
| `agent_ping` | | `null` once the guest answers `guest-ping` |
| `agent_freeze_status` | | `"frozen"` or `"thawed"` |
| `agent_freeze` | `mountpoints` (default: all) | number of frozen filesystems |
| `agent_thaw` | | number of thawed filesystems |
| `agent_update_trigger` | `args` (≤16 tokens) | the guest `UpdateStatus`; invalid arguments are `bad_request` |
| `agent_update_status` | | the guest `UpdateStatus` |

These `agent_*` operations are the only guest-agent wire form; each maps to
one `Control::agent_*` method (see [Guest agent](#guest-agent)), and
`pod::socket::Client` and `qrab-manager agent` use them. Guest-agent commands
other than status fail at once while the agent is disabled or not connected
(for example before QEMU starts).

`pod::Remote` is the sidecar client. It keeps a mirror of the event history,
with the manager's sequence numbers and epoch, over one `follow` connection
that reconnects every second. When a reconnected manager reports another
epoch (or, from a manager without epochs, a lower latest sequence) it
restarted its numbering: the mirror resets, adopts the new epoch and resyncs,
and waiters holding old cursors wake. A mirror that has not yet synchronized
reports no epoch. Requests reuse up to four idle connections
younger than 30 s (the manager closes idle ones after 60 s); a failed pooled
connection is dropped, read-only requests retry once on a fresh one, and
actions are never retried. `pod::Backend` puts either an in-process `Control` or
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
socket's `agent_*` operations and from inside the manager container (output is
the pretty-printed JSON result; `null` for `ping`):

```sh
qrab-manager agent status --config /etc/qrab/vm.json
qrab-manager agent freeze --config /etc/qrab/vm.json [MOUNTPOINT...]
qrab-manager agent thaw --config /etc/qrab/vm.json
qrab-manager agent update-trigger --config /etc/qrab/vm.json channel=stable
qrab-manager agent update-status --config /etc/qrab/vm.json
```

Requests fail immediately when the agent is not connected, its four-slot
queue is full, or the channel is disabled. They have a 30-second control
deadline (a 20-second QGA command bound plus one reconnect attempt).

Read-only queries (`agent_ping`, `agent_freeze_status`, `agent_update_status`,
`agent_boot`, `agent_network`, `agent_firmware`) are coalesced: concurrent
callers of the same query share one in-flight guest call, so status polls
never queue behind a hung agent or fill its queue. A caller that gives up (for
example because its control client disconnected) does not cancel the call for
the others; once every caller has gone the call is dropped and the worker
skips it if it has not started. Ping, freeze status, update status and boot
state are bounded at 5 seconds in the guest (a 10-second control deadline), as
is the periodic ping. An expired read drops the QGA session: the guest may
still answer it, so the task reconnects and resynchronizes with a fresh
`guest-sync-delimited` nonce, which discards the late reply. Update
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

### Redfish inventory and lifecycle extensions

The additive control operations `inventory`, `resource_usage`, and `serial`
return sanitized hardware topology/drives, this manager's process/container
usage, and a bounded 64KiB guest serial tail. Only admin-facing HTTP routes may
serve serial text. The serial log mirror uses a 64KiB bounded queue and 128KiB
thread stack so container-log backpressure cannot block the manager runtime.
The in-memory tail remains authoritative for retained history.

`agent_network`, `agent_firmware`, `agent_boot`, and `agent_boot_next` execute on
the existing bounded QGA worker. They read actual guest networking/EFI state;
the only writable EFI variable is validated BootNext, with readback. `reboot`
preflights enabled guest shutdown and thawed filesystems, then sends native QGA
reboot without a response or automatic retry. Delivery is not completion.

`media` takes a tagged `operation` object (`status`, `insert` with local
`image` basename, or `eject`). `QRAB_MEDIA_ROOT` opts into a trusted read-only
local image mount, cold-provisioned CD device and bounded image policy.
`QRAB_MEDIA_MAX_BYTES` defaults to 8GiB. No URL credentials or arbitrary paths
are accepted.

`firmware_catalog`, `firmware_stage_status` and `firmware_stage` expose the
approved immutable firmware catalog and identity-bound staging selection.
`QRAB_FIRMWARE_ROOT` enables that catalog; staging retains the supervisor's
exclusive persistent-state lease even if the caller stops waiting. Cold-start
application follows PersistentState preparation, revalidates the catalog,
changes only selected CODE and republishes the effective configuration.
NVRAM/TPM identity is retained. See [Redfish](redfish.md) for catalog format,
authorization, task semantics and separate service budgets.

### Secure Boot cold settings and signed updates

`uefi.secure_boot_capable=true` is an explicit secure-firmware build contract;
it selects q35 SMM and protected flash. It is separate from enablement and
key enrollment. `secure_boot_settings` observes the next-boot flag and staged
policy; `stage_secure_boot` records a boolean under retained identity and lease.
Cold startup applies it only before a QEMU child exists, using the bounded
native FV editor and atomic backup/replacement. No Python runtime helper is used.

`agent_secure_boot_databases` reads fixed public PK/KEK/db/dbx inventory;
`agent_secure_boot_update` accepts a bounded authenticated update request.
Guest/manager frames are 384KiB maximum, database aggregate bytes 256KiB,
certificate/signature count 512, decoded AUTH2 payload 128KiB. The existing 16
control connections and 32 Redfish response-lifetime admissions bound memory.
Read caches last five seconds; mutation failure never preserves stale trust data.

`coldrestart` quits QEMU and starts a fresh one in the same manager, applying
staged CODE and Secure Boot settings; a normal `reset` upgrades to it if cold
settings or CODE activation is pending. This no longer depends on
`restart_on_exit` (still set by the operator, and still accepted), so a
standalone manager power-cycles too. QGA reboot remains a warm guest
operation. See [Redfish](redfish.md) for API semantics and measured experiments.

### In-pod power rollout (2026-11-15)

The manager, BMC and Redfish ship in one image, so the Pod's containers always
agree on phase `off`, `Error::PoweredOff` and the new actions. Rolling the
manager image replaces each VM Pod once (the operator's template check), as
any image update does. An older manager answers Redfish `On` on an off VM with
409 and still exits on shutdown; an older operator needs no change, since it
only reads Pod readiness and a powered-off VM's Pod stays Running and Ready.
Its `Running`/`VMRunning` status then means the Pod is live: Redfish
`PowerState` says whether the guest is on. Live checks: GracefulShutdown then
On keeps the Pod UID, restart count and TPM EK and changes `qemu_pid`;
PowerCycle changes `qemu_pid` in the same Pod; Pod deletion, `spec.running:
false` and a killed QEMU behave as before.

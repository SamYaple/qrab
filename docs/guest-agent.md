# Guest agent

`qrab-guest-agent` is a small static guest daemon that speaks the QEMU guest
agent (QGA) wire protocol on the virtio-serial port `org.qemu.guest_agent.0`.
It gives the manager a narrow control path into the guest: liveness,
filesystem quiesce for consistent disk snapshots, and one entry point for
guest updates. It is not qemu-ga: it serves a fixed allowlist and has no
arbitrary execution, file access, user, password or network-configuration
commands.

## Host access

`manager::pod::Control` exposes the agent; its connection status lives in the
control channel's shared state, so `agent_status` answers before QEMU starts
and never queues behind the supervisor. Other commands fail at once while the
agent is disabled or disconnected. The control socket carries the same
operations (`agent_status`, `agent_freeze_status`, `agent_freeze`,
`agent_thaw`, `agent_update_trigger`, `agent_update_status`; see
[manager.md](manager.md#control-socket)). The BMC shows the status to viewers
and offers freeze, thaw and update to admins ([bmc.md](bmc.md)); Redfish has
`Oem.Qrab.GuestAgent` and admin-only OEM actions ([redfish.md](redfish.md)).

## Commands

| Command | Notes |
|---|---|
| `guest-sync`, `guest-sync-delimited` | The delimited reply is preceded by `0xFF`. |
| `guest-ping`, `guest-info` | `guest-info` lists exactly the served commands and their current `enabled` state. |
| `guest-get-osinfo`, `guest-get-host-name`, `guest-get-time` | os-release(5) fields and uname(2); time in ns since the epoch. |
| `guest-fsfreeze-status`, `guest-fsfreeze-freeze`, `guest-fsfreeze-freeze-list`, `guest-fsfreeze-thaw` | `FIFREEZE`/`FITHAW` on mounted block-backed filesystems. |
| `guest-fstrim` | `FITRIM` per filesystem; per-path errors are reported, not fatal. |
| `guest-shutdown` | `powerdown` (default), `halt` or `reboot`; no reply, like QEMU. |
| `qrab-update-trigger` | Start the fixed update hook asynchronously. |
| `qrab-update-status` | Report the current or last hook run. |
| `guest-network-get-interfaces`, `qrab-network-get-interfaces` | Read-only interface inventory ([below](#redfish-inventory-and-boot-commands)). |
| `qrab-firmware-state`, `qrab-boot-state`, `qrab-boot-next` | Fixed EFI variables; `qrab-boot-next` writes only BootNext. |
| `qrab-secure-boot-databases`, `qrab-secure-boot-update` | Public PK/KEK/db/dbx lists and authenticated updates ([below](#authenticated-secure-boot-databases)). |

Everything else, including `guest-exec`, `guest-file-*` and
`guest-set-user-password`, returns `CommandNotFound`. Standard request and
reply shapes use the generated `qapi::qga` types. The `qrab-*` types live in
`qrab_guest_agent::proto` and implement `qapi::QapiCommand`, so a host calls them
through the ordinary typed `qapi_protocol::qga::Qga::execute`.

`guest-info` reports version `11.1.3-qrab`: the QGA schema level the agent
implements, which clients use for version-aware serialization. The agent's own
build version is in `qrab-update-status` as `agent-version`.

## Protocol behaviour

Requests are top-level JSON objects with `execute`, optional object
`arguments` and an optional `id` that is echoed. Line breaks are optional. A
`0xFF` byte discards all partial input, so a client that sends `0xFF` before
`guest-sync-delimited` recovers from a torn previous session; stale replies are
ignored by the client's nonce check. Requests over 384KiB are rejected and the
input is discarded to the next line feed or `0xFF`. Unknown members and
unknown arguments are errors rather than being silently ignored.

A virtio-serial port reads EOF while no host is connected; the agent sleeps
100 ms and keeps the port open, as qemu-ga does. Without udev it finds the port
through `/sys/class/virtio-ports/*/name`, then opens `/dev/vportNpM`.

## Freeze and thaw

Freezable filesystems come from `/proc/self/mountinfo`. Pseudo filesystems
(device major 0: proc, sysfs, tmpfs, devtmpfs, overlay, network mounts) are
skipped, except btrfs; bind mounts are deduplicated by device. Children freeze
before parents. Filesystems that report `EOPNOTSUPP` are skipped; any other
failure thaws what was already frozen and returns an error.

While frozen, only `guest-sync*`, `guest-ping`, `guest-info`,
`guest-fsfreeze-status`, `guest-fsfreeze-thaw` and `qrab-update-status` are
enabled, matching qemu-ga's allowlist plus the read-only update status; other
commands fail with `CommandNotFound` "has been disabled: the agent is in frozen
state". Freeze is refused while an update hook runs.

A watchdog thaws automatically if the host does not thaw within the freeze
timeout (default 120 s, `--freeze-timeout 1..3600`), so a crashed manager or a
lost connection cannot wedge guest I/O. Thaw repeats `FITHAW` on every mounted
filesystem until it fails, counting each filesystem thawed. It does not rely on
recorded state, and the agent thaws everything at startup: no watchdog covers
a freeze left by a previous agent instance.

## Update hook

`qrab-update-trigger` runs only `/etc/qrab/update-hook`; the path is compiled
in and cannot be changed by the host or the command line. The hook must be a
regular executable file, not a symlink, owned by root and not group/world
writable, in a root-owned directory that is not group/world writable. Without
such a hook the command is reported disabled.

Arguments are optional: at most 16 tokens of 1–256 bytes from
`[A-Za-z0-9._:/=+@,-]`, none starting with `-`. They are passed as argv, never
through a shell. The hook runs in its own process group with a cleared
environment (`PATH`, `QRAB_UPDATE_RUN_ID`), `/` as working directory and stdin
from `/dev/null`. One run is active at a time. It is killed (whole process
group) after the update timeout (default 1 h, `--update-timeout 1..86400`). The trigger
reply and `qrab-update-status` carry `state`
(`idle|running|succeeded|failed`), `run-id`, arguments, start/finish times,
exit code or signal, an error and the last 8 KiB of combined stdout/stderr.

The demo image installs `contrib/images/guest-update-hook`, an example that
accepts `channel=*` and `version=*` and only reports. A real image replaces it
with its own updater, which should validate every argument it accepts.

## Guest image

`contrib/images/Containerfile.manager` builds the agent for
`x86_64-unknown-linux-musl` and installs it as `/usr/sbin/qrab-guest-agent`
with the example hook in the RAM-only demo initramfs. `guest-init` restarts it
if it exits and prints `QRAB_GUEST_AGENT_STARTED`. The Debian kernel has
`virtio_console` built in; `prepare-guest.py` still lists it so a modular kernel
loads it.

The manager side is described in [manager.md](manager.md#guest-agent).

## Tests

```sh
cargo test --locked -p qrab-guest-agent
cargo test --locked -p manager
```

The agent's protocol tests drive it with the real `qapi-protocol` QGA client over
a socketpair, with filesystem effects behind a fake platform: sync and `0xFF`
recovery, info, freeze/status/thaw and gating, watchdog auto-thaw, fstrim,
shutdown, unknown and disabled commands, and the update hook. Manager tests
use the qapi-protocol mock QGA server and the real agent library.

The explicit live test boots a direct-kernel guest whose initramfs contains the
agent and hook, then drives ping, freeze/thaw and an update through
`manager::pod::Control`:

```sh
QRAB_TEST_QEMU=/path/to/qemu-system-x86_64 QRAB_TEST_FIRMWARE_DIR=/path/to/share/qemu \
QRAB_TEST_KERNEL=/path/to/vmlinuz QRAB_TEST_INITRD=/path/to/initramfs.gz \
  cargo test --locked -p manager --test pod_guest_agent -- --ignored --nocapture
```

The host needs `mke2fs`. The test formats only its private 64 MiB scratch
file, with a reserved fixture label/UUID, and opts into the demo's test mount
through an exact kernel argument. The demo verifies the size, ext4 type,
label and UUID before mounting that disk, and never formats guest disks.
It mounts before starting the agent. The test freezes that exact mountpoint,
requires a count of one, checks the frozen-operation gate, thaws it and then
tests the update hook. The initramfs must include this opt-in fixture support;
an older RAM-only fixture that freezes zero filesystems fails the test.
Ordinary demo boots do not mount this filesystem.

A glibc `+crt-static` build linked against a host-optimized static libc can
fault with `SIGILL` under TCG `-cpu max`. Use the musl target.

## Redfish inventory and boot commands

Standard `guest-network-get-interfaces` reports the qemu-ga shape;
`qrab-network-get-interfaces` adds real Linux interface/carrier flags. Names, MACs, addresses/prefixes and counters come from
the guest; the manager bounds and validates them before matching configured MACs.

`qrab-firmware-state` reads the fixed EFI SecureBoot/SetupMode/AuditMode/
DeployedMode variables. Absent/inaccessible/malformed values remain unknown.
This does not expose arbitrary EFI variable or filesystem access.

`qrab-boot-state` reads actual EFI BootOrder/BootNext and bounded Boot####
entries. `qrab-boot-next` selects an existing active reference or clears it.
Writes are limited to the fixed BootNext GUID, with native NV/BS/RT attributes
and readback confirmation; kernel permission/immutable-file errors are real
failures. There is no raw pflash access or arbitrary EFI variable API. Signed Secure Boot
database updates use the fixed authenticated command described below.

Manager graceful restart uses the existing native `guest-shutdown` reboot mode,
with admission/thaw preflight and no success response. A successful send means
delivery, while later guest/QMP observations establish the resulting state.

## Authenticated Secure Boot databases

`qrab-secure-boot-databases` returns bounded public PK/KEK/db/dbx signature lists.
`qrab-secure-boot-update` accepts an AUTH2 descriptor for one of those fixed names
and GUIDs, validated before Platform dispatch and denied while frozen. It never
accepts a private key or arbitrary variable/path. The static guest adds a small
pure-Rust DER/X.509 decoder, with no OpenSSL library, helper process or async
runtime dependency. Firmware authorization remains intact. For an existing fixed-name database,
the agent pins its regular NOFOLLOW inode (or atomically creates a missing fixed
name with exclusive 0600 semantics after validating the signed input), temporarily removes only the Linux
immutable bit if necessary, writes once to the same verified inode, then restores
and verifies the exact prior flags on success/error (with an unwind guard).
Malformed input is rejected before any flag change. No generic chattr/ioctl
interface exists, and restoration failure is an explicit unknown outcome; a successful write is followed by inventory readback. Unknown
outcomes must be inspected, not automatically replayed. Native SecureBootEnable
is boot-service-only on the tested OVMF, so enable/disable uses the manager's cold
policy editor instead of pretending Linux runtime can write that flag.

The checked musl release binary is static PIE: 1,328,312 bytes unstripped
(981,318 bytes text). This is executable size, not process RSS; measurement
was taken after the Secure Boot parser/update additions on 2026-11-14.

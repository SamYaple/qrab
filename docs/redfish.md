# Redfish sidecar

`qrab-redfish` is an optional sidecar that serves a native DMTF Redfish API for
one VM. It reaches the VM only through the manager's control socket
([manager.md](manager.md#control-api)); it never sees QMP, VNC, the runtime
directory, persistent state or Secrets. Enable it per VM with
`spec.components.redfish` ([operator.md](operator.md#components)).

```sh
qrab-redfish --socket /run/qrab-control/manager/control.sock
```

| Variable | Meaning |
| --- | --- |
| `REDFISH_OIDC_ISSUER` | Required HTTPS OIDC issuer, e.g. `https://idm.yaple.net/oauth2/openid/qrab-redfish` |
| `REDFISH_OIDC_AUDIENCE` | Required token audience: the OAuth2 client ID, e.g. `qrab-redfish` |
| `REDFISH_TRUSTED_TOKENS` | Optional further token sources: comma-separated `issuer\|audience` pairs, e.g. `https://idm.yaple.net/oauth2/openid/qrab-bmc\|qrab-bmc` |
| `REDFISH_ADMIN_SUBJECTS` | Comma-separated subject UUIDs that may read and reset the VM |
| `REDFISH_VIEWER_SUBJECTS` | Comma-separated read-only subject UUIDs |
| `REDFISH_ADMIN_GROUPS` | Comma-separated Kanidm group UUIDs whose members may read and reset the VM |
| `REDFISH_VIEWER_GROUPS` | Comma-separated Kanidm group UUIDs with read-only access |
| `REDFISH_LISTEN` | Listener; defaults to `0.0.0.0:8081` |
| `REDFISH_CA_FILE` | Optional PEM root certificate for an isolated test IdP |
| `RUST_LOG` | Default `qrab_redfish=info` |

At least one subject or group is required. The admin and viewer lists of each
kind must be disjoint. The
listener is plain HTTP inside the Pod; expose it only through a TLS Ingress, as
for the BMC.

### Exposure

`contrib/kubernetes/console.yaml` routes `https://qrab-demo.yapplesauce.com/redfish`
(Redfish's own URI root, which the BMC does not serve) to Service port
`redfish` (8081). The rest of the host still reaches the BMC, behind the same
certificate. The VM's CiliumNetworkPolicy admits 8081 only from the ingress
and host entities. With `spec.components.redfish.enabled`,
`contrib/kubernetes/check.sh` requires a native `redfish` sidecar with a
subject allowlist and no access to the runtime directory. It also requires
401 with `WWW-Authenticate: Bearer` on `/redfish`, the service root,
`Systems/1` and `Reset`, with no token or a malformed one.

`contrib/kubernetes/redfish-live.py` is the authenticated half, run by an
allowlisted person. It obtains an ID token by PKCE through the loopback
redirect and keeps it in memory only. As admin, it reads `Systems/1` and
cycles `Reset` Pause and then Resume. With `--expect viewer`, it reads and
requires 403 for `Reset`.

## Authentication

Every request, including `/redfish` and the service root, needs
`Authorization: Bearer <OIDC ID token>`. This deliberately departs from the
DMTF convention that the service root is anonymous: no VM data or version
information is served without a token. Like Kubernetes API-server OIDC
authentication, the token is an ID token (a signed JWT) issued to the
`REDFISH_OIDC_AUDIENCE` client or to a `REDFISH_TRUSTED_TOKENS` pair. Each
request verifies the signature against the issuer's discovered JWKS (ES256 or
RS256 only), the issuer, the audience (and `azp` when several audiences are
present) and expiry. The subject must be
listed, or the token's `groups` claim (Kanidm scope `groups_uuid`) must contain
a configured group UUID; admin wins when both apply. Tokens are limited to
16 KiB, about 300 group memberships. Keys are refreshed every ten minutes, and at most every 30 seconds
after a signature fails, so rotated keys are picked up. This verification lives
in `projects/oidc-bearer`, which the BMC's
[dashboard bearer mode](bmc.md#dashboard-bearer-tokens) shares.

### Trusted token sources

Each configured issuer is discovered at startup and keeps its own JWKS and
refresh. The token's `iss`, read before verification, selects the single
verifier for that issuer; a token from any other issuer gets 401 and no other
source is tried. Several audiences of one issuer share its verifier, and a
token is checked against the one audience it names (its `azp`, or its only
`aud`), so `iss` and `aud` must form a configured pair. Repeated pairs are
ignored. A trusted source grants no role: the subject and group lists above
still decide admin, viewer or 403. The operator adds the BMC browser client
and the dashboard client
([operator.md](operator.md#dashboard-access-and-publicurl)) so their users'
ID tokens reach Redfish unchanged.

There are no Redfish sessions, static tokens, Basic authentication, stored
passwords or anonymous routes other than `/healthz` and `/readyz`. Missing,
malformed, expired, wrong-issuer, wrong-audience or badly signed tokens get 401
with `WWW-Authenticate: Bearer`; a valid token for an unlisted subject without a
configured group gets 403.
Bearer tokens cannot be revoked here: their lifetime is the IdP's ID-token
lifetime, and event streams close when the token expires.

### Kanidm provisioning

Use a separate public client for direct Redfish callers. BMC (`qrab-bmc`) and
dashboard tokens are accepted only where the operator lists them in
`REDFISH_TRUSTED_TOKENS`:

```sh
kanidm system oauth2 create-public qrab-redfish 'qrab Redfish' https://qrab-demo.yapplesauce.com -D idm_admin
kanidm system oauth2 add-redirect-url qrab-redfish http://localhost:8400/callback -D idm_admin
kanidm system oauth2 enable-localhost-redirects qrab-redfish -D idm_admin
kanidm system oauth2 update-scope-map qrab-redfish qrab_operators openid profile -D idm_admin
```

An operator obtains an ID token with any PKCE authorization-code client using
the loopback redirect, then calls e.g.
`curl -H "Authorization: Bearer $ID_TOKEN" https://<redfish-host>/redfish/v1/Systems/1`.

## Resources

| URI | Type | Notes |
| --- | --- | --- |
| `/redfish` | version object | `{"v1":"/redfish/v1/"}` |
| `/redfish/v1/` | `ServiceRoot.v1_17_0` | `Systems`, `Managers`, `EventService`; `UUID` (the CR UID) for admins |
| `/redfish/v1/Systems` | `ComputerSystemCollection` | one member |
| `/redfish/v1/Systems/1` | `ComputerSystem.v1_22_0` | `PowerState`, `Status`, `ProcessorSummary`, `MemorySummary`, `Actions`, `Links.ManagedBy`; `Oem.Qrab.Configuration` is the BMC configuration view (redacted for viewers) |
| `/redfish/v1/Systems/1/Actions/ComputerSystem.Reset` | action (POST) | admin only |
| `/redfish/v1/Systems/1/Actions/Oem/QrabComputerSystem.{Freeze,Thaw,Update}` | OEM actions (POST) | admin only; `{"Mountpoints":[...]}` (optional) for Freeze, `{"Args":[...]}` for Update; return `FrozenFilesystems`, `ThawedFilesystems` or `Update`. `Oem.Qrab.GuestAgent` on `Systems/1` is the cached agent status |
| `/redfish/v1/Managers`, `/Managers/1` | `ManagerCollection`, `Manager.v1_19_0` | `ManagerType: BMC` |
| `/redfish/v1/Managers/1/LogServices/Events` | `LogService.v1_7_0` | 256 records, `WrapsWhenFull`; `Oem.Qrab.QmpDropped` counts events QMP discarded, with `Health: Warning` when nonzero |
| `.../Events/Entries`, `.../Entries/{seq}` | `LogEntryCollection`, `LogEntry.v1_17_0` | one `Oem` entry per QMP event; QAPI name and data under `Oem.Qrab` |
| `/redfish/v1/EventService` | `EventService.v1_10_0` | `ServerSentEventUri` and opt-in HTTPS push subscriptions |
| `/redfish/v1/EventService/SSE` | SSE of `Event.v1_10_0` | SSE `id` is the QMP sequence; `Last-Event-ID` resumes |

`PowerState` follows the manager status: `PoweringOn` while starting, `On`
running, `Paused` when QEMU is paused or otherwise not running, `PoweringOff`
while stopping, `Off` after exit; `null` with `UnavailableOffline` when the
manager cannot be reached. Panicked or I/O-errored guests and failed managers
report `Health: Critical`.

`ComputerSystem.Reset` (`{"ResetType": ...}`) maps honestly to manager actions:

| ResetType | Effect |
| --- | --- |
| `On` | resume a paused VM; no-op while on; 409 when off (start/stop is `spec.running`) |
| `GracefulShutdown` | ACPI powerdown with the bounded quit/kill fallback; ends this VM instance, and the operator starts another when `spec.running` is true |
| `ForceRestart` | QMP `system_reset` |
| `Pause`, `Resume` | QMP `stop` / `cont` |
| `Nmi` | QMP `inject-nmi` (all virtual CPUs; admin only) |
| `PowerCycle` | Operator-managed VMs only: clean QEMU exit and fresh manager instance under the same retained identity |
| `GracefulRestart` | QGA `guest-shutdown` in reboot mode; requires connected agent, enabled command and thawed filesystems. Success acknowledges delivery, subsequent observations establish restart |

`ForceOff`, `ForceOn`, `PushPowerButton`,
`Suspend` and `FullPowerCycle` get 400
`ActionParameterNotSupported`; other values get `ActionParameterValueNotInList`.
A successful reset returns 204.

Errors use the Redfish error format with `Base.1.18` message IDs
(`NoValidSession`, `InsufficientPrivilege`, `ResourceNotFound`,
`OperationNotAllowed`, `MalformedJSON`, `ActionParameterMissing`,
`ServiceTemporarilyUnavailable`, ...). All responses carry `OData-Version: 4.0`
and `Cache-Control: no-store`. Ordinary request bodies are limited to 4 KiB (signed EFI updates have a separate
192 KiB bound) and eight
event streams are served at once.

## Tests

`cargo test --locked -p qrab-redfish` covers resource shapes, the authentication
rejection matrix (missing, Basic, malformed, expired, wrong audience, wrong
issuer, foreign signing key, unlisted subject, viewer reset), trusted sources
(each pair accepted with its listed role, cross-source audiences and keys and
unconfigured issuers 401, unlisted subjects 403), reset mapping,
power-state mapping, the event stream and its token-expiry bound. The tests sign
tokens with the test-only keys under `projects/redfish/tests/fixtures`.

## Native inventory and telemetry

Resource mappings follow [DMTF schemas](https://redfish.dmtf.org/schemas/).
All routes, including discovery, require OIDC. Viewers may read sanitized
inventory and metrics; mutations, diagnostic attachments and serial history
require admin. Native protocol support does not imply physical sensors or an
unimplemented power-management path.

| URI below `/redfish/v1` | Mapping |
| --- | --- |
| `/Chassis`, `/Chassis/1` | Virtual chassis with live power/status and system/manager/TPM links |
| `/Systems/1/Processors`, `/{socket-id}` | Live QMP sockets, cores, threads and CPU model; topology unknown when QMP does not supply it |
| `/Systems/1/Memory`, `/Memory/1` | Aggregate configured RAM following manager hotplug progress |
| `/Systems/1/Storage`, `/Storage/1` | Guest-facing virtual storage and drive links |
| `/Systems/1/Storage/1/Volumes`, `/{drive-id}` | Guest-visible logical block volumes with authoritative virtual capacity |
| `/Chassis/1/Drives/{drive-id}`, `/Metrics` | Capacity, read-only/media/I/O state and per-drive byte/operation counters; firmware flash and backing nodes excluded |
| `/Systems/1/EthernetInterfaces`, `/{id}` | Configured MACs enriched by guest IP/prefix, real administrative/carrier state and counters when agent data matches the MAC |
| `/Chassis/1/TrustedComponents`, `/TPM` | Configured virtual TPM2; no invented swtpm firmware version |
| `/UpdateService/FirmwareInventory`, `/{id}` | Known service/QEMU versions and UEFI/TPM presence; unavailable versions omitted |
| `/Systems/1/ResetActionInfo` | Supported reset types and required parameter |
| `/Systems/1/BootOptions`, `/Boot####` | Actual guest EFI boot variables; no synthetic QEMU bootindex references |
| `/Systems/1/SecureBoot` | Actual current guest EFI SecureBoot/SetupMode/AuditMode/DeployedMode observations |
| `/TelemetryService/MetricDefinitions`, `/{metric-id}` | Units, integer type, gauge/counter semantics and metric-property references |
| `/TelemetryService/MetricReportDefinitions/VM` | `OnRequest` by default; explicitly configured `Periodic` cadence |
| `/TelemetryService/MetricReports/VM` | Current report; MetricProperty links address `/Systems/1/Oem/Qrab/Metrics` |
| `/TelemetryService/MetricReports/History-{sequence}` | Up to 128 successful samples retained in memory |
| `/Managers/1/Oem/Qrab/Resources` | Independent Redfish and manager process/container usage |

QMP queries are fixed, typed and read-only: `query-cpus-fast`, `query-block`,
`query-memory-size-summary`, and guest-facing `query-blockstats`
(`query-nodes=false`). Inventory and metrics each coalesce successful reads
for one second, with a two-second query deadline and bounded manager admission.
Backing chains, host paths, storage credentials and host thread IDs are not
exposed. A failed sample returns 503 rather than invented zeroes. Historical
reports retain their sample timestamp; stale history never substitutes for a
successful current query.

RAM metrics are configured capacity, not guest used RAM or QEMU RSS. Block
counters reset with the QEMU instance; rates require successive samples. Drive
native KiB counters use whole KiB; exact byte counts remain in OEM fields.
The snapshots use sequential queries and are not atomic VM-wide measurements.
Physical temperature, power and fan readings are not synthesized.

### Numeric thresholds

`/TelemetryService/Triggers` supports admin POST and per-trigger GET/PATCH/DELETE.
Up to 16 triggers watch a single canonical VM metric property with integer
UpperWarning/UpperCritical/LowerWarning/LowerCritical thresholds. Increasing
and Decreasing activation are supported with zero dwell/hysteresis. The first
sample establishes a baseline; duplicate samples are ignored and cumulative
counter resets reestablish it. Only `LogToLogService` is advertised. Enabled triggers sample every five seconds; inactive triggers do no sampling. The
`/Managers/1/LogServices/Telemetry` log retains128 alerts; threshold webhook
forwarding is not yet offered. Triggers and report history are volatile.

## Tasks, diagnostics and serial history

`/TaskService/Tasks` holds at most 32 records, with at most 2 active operations.
Terminal tasks expire after 60 minutes or are evicted oldest-first when capacity
is needed. Monitors live under `/TaskService/TaskMonitors/{id}`. Cancellation
is not advertised; losing monitoring never claims that a guest operation was
cancelled. Restart loses this task registry.

Admin `POST /TaskService/Actions/Oem/QrabTaskService.StartGuestUpdate`
(`{"Args":[]}`) wraps the existing fixed guest update hook with task tracking.
The original guest-agent OEM action remains compatible. Monitoring is bounded
to 300 seconds and reports unknown outcome when the guest disconnects or a run
cannot be identified; it never silently resubmits an update.

Admin `POST /Managers/1/LogServices/Diagnostics/Actions/LogService.CollectDiagnosticData`
creates a diagnostic task. Its completed monitor links a LogEntry with an
attachment. Bundles contain redacted configuration, summarized status/events
and best-effort metrics; raw QMP payloads and free-form errors are omitted.
Each attachment is bounded to 128KiB and remains admin-only.

`/Managers/1/LogServices/Serial` exposes the current 64KiB guest serial tail as
an admin-only LogEntry attachment, with retained/dropped byte counts. It is
volatile and mirrors the existing container serial log on a bounded best-effort
queue; log backpressure cannot stall VM control. Serial text may
contain guest secrets and is intentionally excluded from redacted diagnostics.

## Push subscriptions

`/EventService/Subscriptions` supports admin POST and per-subscription GET/DELETE.
At most 32 destinations are retained. Creation accepts only exact operator
allowlisted HTTPS URLs, no credentials/query/fragment or custom HTTP headers.
Payloads contain event names/sequence metadata; raw QMP data is omitted.
Viewers cannot see destinations or user Context. Subscriptions expire at the
earlier of creator bearer expiry and 24 hours; tokens are never stored.

One worker uses event notifications and allows two in-flight deliveries, each with a five-second timeout,
no redirects and no environment proxy. There are three attempts per event,
two seconds between failures, then subscription termination. Successful cursors
are checkpointed in batches per delivery cycle; an interrupted cycle can replay
a previously delivered event, so receivers should deduplicate by event ID. Sequence cursors,
missed-event counters and manager restart handling bound replay behavior.
A persistence failure pauses delivery instead of producing a retry storm.
Disk writes run outside the async runtime and reader lock, with one admitted
persistent writer; concurrent mutations return503 instead of growing a queue.

Standalone environment:

| Variable | Meaning |
| --- | --- |
| `REDFISH_EVENT_DESTINATIONS` | Comma-separated exact allowed HTTPS receiver URLs; empty disables creation/delivery |
| `REDFISH_SUBSCRIPTIONS_FILE` | Optional absolute private JSON state file,128KiB maximum; atomic0600 writes. Without it subscriptions are volatile |

Operator `spec.components.redfish.eventDestinations` configures exact HTTPS port 443
receivers and creates an owned additive Cilium policy for their hosts/IPs.
`spec.components.redfish.stateVolumeClaim` names a separate, explicit PVC for
subscription state; it must differ from VM identity and artifact claims. It is
mounted only into Redfish, never manager/BMC. Existing PVC ownership and
retention stay with the user; the operator does not create or delete it.

## Boot and virtual media

Admin PATCH of `/Systems/1` can set an existing active EFI boot reference:

```json
{"Boot":{"BootSourceOverrideEnabled":"Once","BootSourceOverrideTarget":"UefiBootNext","BootNext":"Boot0001"}}
```

`{"Boot":{"BootSourceOverrideEnabled":"Disabled"}}` clears it. The guest
agent writes only the fixed EFI BootNext variable, validates the selected
Boot#### and verifies readback. Actual EFI permission/immutable-variable errors
are propagated. Firmware consumes BootNext on its next boot; no raw NVRAM or
pflash edit is used. Persistent BootOrder changes and synthetic HDD/CD/PXE
aliases are not offered.

`/Systems/1/VirtualMedia` exposes a read-only CD when the manager has an explicit
media policy. Native InsertMedia accepts `{"Image":"installer.iso"}`; EjectMedia
never forces a guest-locked tray. Local .iso/.img basenames only: no remote URLs,
paths or caller credentials. Images must be nonempty regular files under a
trusted read-only media mount; canonical escape and size checks apply.

Manager `QRAB_MEDIA_ROOT` enables an empty q35 IDE CD-ROM at startup;
`QRAB_MEDIA_MAX_BYTES` defaults to 8GiB and cannot exceed64GiB. Existing VMs
without the cold-provisioned device may reject hotplug, which is reported as
failure. Media capacity is bounded independently of task/telemetry memory.
Use operator `spec.mediaVolumeClaim` for the approved read-only media PVC.

## Firmware staging and Secure Boot

Current Secure Boot state is read from the guest EFI interface. Missing or
unreachable values remain unknown. The current SecureBoot flag is distinct from
the next-boot SecureBootEnable setting; neither is inferred from a filename.

`spec.secureBootCapable: true` selects explicit secure-capable OVMF CODE and
SMM-protected pflash. Standalone config uses `uefi.secure_boot_capable: true`
with approved secure-capable CODE. This capability flag does not enroll keys or
enable Secure Boot. The manager image source now includes
`OVMF_CODE_4M.secboot.fd`; publishing/deploying that image is a separate step.

Admin PATCH `/Systems/1/SecureBoot` or `/SecureBoot/Settings` accepts exactly
`{"SecureBootEnable":true}` or false and returns a staging task. Enablement
requires an enrolled valid X.509 PK and a valid allow database. Staging changes
only private VM-identity-bound metadata, not a running VM's VARS. The ordinary
resource retains its actual current setting; `@Redfish.Settings.SettingsObject`
links the requested value and OEM pending/generation fields.

At cold manager startup, while holding the exclusive persistent-state lease,
a bounded native EDK2 authenticated-FV editor validates headers/checksum,
record states, bounds and databases. It changes an existing enable flag by one
byte, or appends into erased space if absent; no compaction or key deletion.
Unsupported/corrupt/full stores fail closed. Atomic replacement retains the
previous complete store as `identity/OVMF_VARS.before-secure-boot.fd`; keys,
boot options, authenticated metadata and TPM identity are preserved.

For operator-managed VMs, `ResetType: PowerCycle` starts a fresh VM instance
under the same identity after clean QEMU/storage release. `ForceRestart`
automatically takes this cold path when Secure Boot or CODE staging is pending.
A guest/QGA graceful reboot does not reload manager configuration. Standalone
managers must be cold-restarted explicitly; no restart guarantee is invented.
Current-boot enabled status still comes from observed firmware, never from a
completed staging task or applied-generation marker.

`/Systems/1/SecureBoot/SecureBootDatabases` exposes fixed PK/KEK/db/dbx
collections with native Signatures and Certificates. Public DER certificates
are fully parsed before PEM/fingerprint output; private-key ASN.1 containers,
malformed lists and oversized replies are rejected. Inventory is cached for
five seconds with its observation timestamp; a busy query is rejected rather
than retaining an unbounded waiter queue.

Admin `POST /Systems/1/SecureBoot/Actions/Oem/QrabSecureBoot.UpdateAuthenticatedDatabase`
accepts `{"DatabaseId":"db","PayloadBase64":"<EFI_AUTHENTICATION_2 .auth>","Append":false}`.
Only PK/KEK/db/dbx are writable. Payloads are capped at 128KiB decoded and 192KiB
HTTP; all other route bodies retain 4KiB limits. The command validates AUTH2,
UTC time/header lengths, PKCS#7 envelope and signature-list/DER structure, then
performs one fixed-variable efivarfs write. Firmware validates trust, signature
and timestamp authorization. No private signing key is sent or stored, and no ambiguous mutation is
replayed. An existing fixed-variable inode may have Linux's conservative
immutable guard temporarily lifted for exactly one authenticated write; every
other flag is preserved and the exact original flags are restored and verified
on success or failure. A missing fixed-name variable may be created atomically
with NOFOLLOW/exclusive0600 semantics after validation, enabling initial
SetupMode enrollment; a concurrent existing inode is never truncated. A rejected
creation is reported for inspection rather than blindly unlinked. No arbitrary
path/flag operation is exposed. Firmware
continues enforcing PKCS#7 trust/signature/time policy. A restoration failure is
an unknown outcome requiring inspection, never a successful update.
The cache is invalidated before dispatch, including errors after a possible
write. Re-read actual inventory after an unknown outcome. Append follows UEFI
semantics and does not provide timestamp replay protection; ordinary replacement
does. ResetKeys-to-default and unauthenticated key deletion are not advertised.

The authenticated database route requires a running, connected guest agent and
actual firmware support. Permissions/firmware policy can deny writes. It is not
a raw EFI variable or filesystem API.

`/UpdateService/Oem/Qrab/Artifacts` lists approved firmware artifacts;
`/UpdateService/Oem/Qrab/StagedFirmware` reports pending/active selection.
Admin `POST /UpdateService/Actions/Oem/QrabUpdateService.StageFirmware`
with `{"ArtifactId":"ovmf-v2"}` returns a task. Completion means staging
succeeded; activation remains `PendingRestart` until a cold manager start
selects the validated CODE and a live QEMU reaches Running. A QMP system_reset
alone does not reload CODE.

`QRAB_FIRMWARE_ROOT` points to an immutable root-owned directory (0555), with
0444 manifest/CODE files, no symlinks. Operator mode uses
`spec.firmwareCatalogVolumeClaim`, mounted read-only only into the manager.
A catalog has schema 1 and at most 64 artifacts:

```json
{"schema":1,"artifacts":[{"id":"ovmf-v2","version":"approved-build-version","file":"OVMF_CODE.fd","sha256":"<64 lowercase hex>","size_bytes":3653632,"vars_size_bytes":540672,"vars_template_sha256":"<64 lowercase hex>"}]}
```

Sizes shown are examples; use the actual approved build dimensions/digests.
Artifacts for secure-capable targets must additionally declare
`"secure_boot_capable":true` in their sealed catalog metadata.
Each CODE is bounded to 64MiB. Staging validates SHA256, image size, retained
VARS size and the immutable VARS template digest, then atomically records a
VM-identity-bound selection under the existing private identity directory.
Application occurs under the manager's exclusive persistent-state lease and
revalidates the catalog. Existing NVRAM keys and TPM identity are preserved.
Arbitrary URL/capsule upload, automatic rollback orchestration and default-key
reset remain separate lifecycle work.

## Separate CPU and memory accounting

| Container | CPU request | CPU limit | Memory request | Memory limit |
| --- | --- | --- | --- | --- |
| manager: QEMU/swtpm/supervisor | guest vCPUs | guest vCPUs+1 | guest MiB+256Mi | guest MiB+512Mi |
| bmc, enabled |20m|500m|32Mi|128Mi|
| redfish, enabled |20m|500m|32Mi|128Mi|

With both services enabled, requests total guest CPU+40m and guest memory+320Mi;
limits total guest CPU+2 and guest memory+768Mi. These are reservations/limits,
not measurements. Manager CPU overhead has limit headroom rather than an
additional CPU request. Guest resize never absorbs sidecar resources.

The BMC page and authenticated `/api/resources` report BMC/manager separately.
Redfish's OEM resource endpoint reports Redfish/manager separately. Process CPU
seconds and RSS are exact local-process observations. Container usage is
reported only with `QRAB_RESOURCE_SCOPE=container` (set by operator) and verified
self-membership in the resolved cgroup v2; unavailable fields are omitted.
Container CPU includes children such as QEMU/swtpm; process CPU does not.
Standalone embedded BMC shares the manager process and cannot give independent
container accounting. Configured guest RAM must not be added to QEMU's cgroup
memory usage because it is already accounted there.

New state has explicit bounds:64KiB serial tail in manager;128 metric reports,
16 triggers/128 alerts,32 subscriptions,32 tasks/two active and 128KiB per
attachment in Redfish. All workers remain inside the Redfish allocation.
Subscription disk state lives on its separate optional PVC. No Redis was added;
these bounded stores do not yet justify another per-VM service. Redis would
require an explicit independent container/PVC budget if future durable history
or task requirements justify it.

With kubelet/cAdvisor scraping, retain container identity in usage queries:

```promql
sum by (namespace,pod,container) (rate(container_cpu_usage_seconds_total{namespace="qrab",container=~"manager|bmc|redfish|redis"}[5m]))
sum by (namespace,pod,container) (container_memory_working_set_bytes{namespace="qrab",container=~"manager|bmc|redfish|redis"})
```

No Prometheus credentials or Kubernetes API tokens are introduced into VM Pods.

## Remaining full-stack lifecycle work

The `redfish-full-stack` container in towl retains the complete scope. Further
work covers operator-coordinated ForceOff/On (without unintended respawn),
durable task/history semantics, threshold event forwarding, remote media
acquisition, firmware rollback, persistent BootOrder settings and approved
default-key reset. PowerCycle/cold activation, Secure Boot settings and signed
key-database updates are now implemented; deployment/live-baseline validation
remain separate.
Read-only discovery and bounded frameworks do not advertise these operations as
implemented. Image builds, baseline-QEMU smoke tests and cluster rollouts stay
separate from source changes.

## Performance and verification experiments

Telemetry defaults to OnRequest, with no idle metric RPCs from Redfish.
`REDFISH_TELEMETRY_INTERVAL_SECS=5..3600`, or operator
`spec.components.redfish.telemetryIntervalSeconds`, opts into the exact advertised
periodic interval. Only explicit periodic sampling retains background history;
on-demand reads retain successful samples, while trigger-only sampling does
not pretend to be periodic history. The manager's existing VM heartbeat remains.

History holds 128 compact typed samples and formats JSON only for reads.
Duplicate/stale samples allocate no report JSON. Diagnostic attachments use
shared byte buffers, task listings avoid cloning output, and subscription /
diagnostic metadata projections omit raw QMP data before cloning. The Redfish
service admits at most 32 requests across routes and holds admission until the
response body completes or is dropped, including SSE; overload returns 503.
Cheap probes remain available under load. Firmware status hashes are cached
for five seconds and invalidated before staging.

[Performance experiment](../contrib/experiments/redfish-performance.md) documents
three debug runs and limitations:20,000 duplicate recordings dropped from 6.64M
allocations to zero, 128 event acknowledgments from 128 checkpoints to four,
and modeled process peak RSS from 8,608KiB to 6,468KiB. Historical JSON reads
cost more (~66us each) because rendering moves to the read path. These are
isolated representations/current mock transport and tmpfs checkpoint timings,
not promised end-to-end or PVC speedups.

`contrib/experiments/secure-boot-probe.py` performs disposable TCG EFI tests,
including signed acceptance, unsigned rejection, cold flag roundtrip and the
actual flag's disappearance after ExitBootServices. The native editor matches
all active variables from virt-fw-vars and preserves unrelated bytes; measured
debug edits were 0.76ms/128KiB and 1.78ms/528KiB versus ~120ms/~32MiB for the Python
reference tool. Production has no Python helper or new daemon.

`contrib/experiments/secure-boot-auth-update.py` uses real throwaway PKCS#7
signatures to prove trusted append/replacement acceptance, untrusted signer and
tampered-signature rejection, replacement replay rejection, and unchanged db
on each rejection, before/after ExitBootServices. Host EFI is never accessed.
Current recorded experiments use installed QEMU 10.2.3/OVMF; they demonstrate
firmware enforcement and editor parity, not pinned-QEMU 11.1 cluster validation.
See script help and ignored target result JSON for reproduction. Primary
references: [EDK2 enrollment](https://github.com/tianocore/edk2/blob/master/OvmfPkg/EnrollDefaultKeys/EnrollDefaultKeys.c),
[UEFI authenticated variables](https://uefi.org/specs/UEFI/2.10/08_Services_Runtime_Services.html),
and [DMTF SecureBoot](https://redfish.dmtf.org/schemas/v1/SecureBoot.v1_1_0.json).

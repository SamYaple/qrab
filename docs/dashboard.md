# VM dashboard

See the [connection atlas](connections.md) and
[OIDC forwarding diagram](connections/identity.md#oidc-browser-login-and-downstream-user-tokens)
for transport encryption, user-token forwarding to VM Pods and the separate
Kubernetes and Kanidm service-account connections.

`qrab-dashboard` (`projects/dashboard`) is the web entry point for qrab VMs at
**https://vms.yapplesauce.com**. Signed-in users see the VMs shared with them,
with their size, disks and live usage, and open a VM's status, guest agent,
QMP events, configuration and console. Members of `qrab_vm_launchers` can
launch new VMs; VM admins can send power actions and delete VMs.

The updated dashboard uses SPIFFE mTLS for all per-VM management and console
requests, with the user ID token still verified by the receiving service. BMC
8443 and Redfish8444 have separate service IDs bound to immutable VM UID and
container, rather than DNS/IP or the manager's migration identity. The dashboard
has its own image-bound SVID and read-only CSI socket; no plaintext fallback is
provided. Public ingress-to-dashboard and monitoring connections are separate
hardening tasks, not implicitly encrypted by these service listeners.

## Architecture

- **One OIDC client for the cluster.** The dashboard is the only browser OIDC
  client for VM access, with one callback,
  `https://vms.yapplesauce.com/auth/callback`. Per-VM console hosts,
  certificates and callbacks are no longer needed for new VMs: the dashboard
  creates VMs without `console.publicUrl`, so their BMCs are bearer-only.
- **VM calls are server-side.** For a VM page the dashboard resolves the VM's
  Pod from `status.podName`, checks that this exact VM controls the Pod (a
  controller ownerReference of kind VirtualMachine with the VM's UID, as the
  operator checks) and that the Pod carries the operator's labels for it
  (`app.kubernetes.io/instance` and `qrab.yapplesauce.com/owner-uid`), and
  calls that Pod with `Authorization: Bearer <the user's ID token>`: its
  Redfish service at `https://<pod IP>:8444/redfish/v1` for status, guest
  agent, configuration, events and power, and its BMC at
  `wss://<pod IP>:8443/api/console` for the console only. Both verify the
  token themselves (issuer `https://idm.yaple.net/oauth2/openid/qrab-dashboard`,
  audience `qrab-dashboard`; see [bmc.md](bmc.md#dashboard-bearer-tokens) and
  [redfish.md](redfish.md#trusted-token-sources)) and apply the VM's roles:
  while the BMC is enabled the operator merges the console's admin and viewer
  lists into Redfish's ([operator.md](operator.md#bmc-users-in-redfish)), so a
  dashboard user holds the same role in Redfish, and the dashboard cannot
  widen access to anything either serves. Redfish calls need only the Redfish
  component (always present with the BMC, or `components.redfish.enabled:
  true` without it); the console needs the BMC. Launch and delete use the
  dashboard's own ServiceAccount; the dashboard gates them itself
  ([below](#roles)). Redfish responses larger than 1 MiB are refused (4 MiB
  for the whole event log).
- **The ID token never reaches the browser.** It lives in the server-side
  session with the subject, `preferred_username`, the token's group UUIDs and
  the expiry. The browser holds only an opaque `__Host-` cookie.
- **Pages are Dioxus SSR with HTML forms.** The console and full BMC pages
  use JavaScript: the vendored noVNC (`projects/dashboard/assets/novnc`, a copy
  of the BMC's) connects to `/vms/{namespace}/{name}/console/ws`, which the
  dashboard proxies to the BMC's `/api/console` WebSocket, and its power
  controls call `/vms/{namespace}/{name}/power`
  ([below](#console-power-controls)).

### The BMC panel

The VM page keeps status, usage, networking and health visible, with local links
to jump between sections. Infrastructure identity, Pod budgets, effective
configuration and recent event payloads use native HTML disclosures. Event-drop
counts remain visible in the closed event heading, and a configuration with a
resize error starts expanded. These controls need no JavaScript.

The detail page's BMC panel reads the VM's Redfish service with the user's
token (viewers and admins alike), for any VM whose Pod runs the Redfish
sidecar, with or without the BMC. A VM without Redfish shows that the Redfish
component is disabled instead:

- `Systems/1`: `PowerState`, `Status` (`State`, `Health`), the guest agent
  status (`Oem.Qrab.GuestAgent`) and the effective configuration
  (`Oem.Qrab.Configuration`, which Redfish redacts for viewers);
- `Managers/1/LogServices/Events` for the QMP drop count, and its `Entries`
  for the newest 20 QMP events. Redfish implements no `$top`/`$skip`, so the
  dashboard reads the whole bounded log (at most 256 entries) and keeps the
  newest 20;
- `TelemetryService/Oem/Qrab/MetricHistory` with a fixed `range=1h` for each
  metric (`container-cpu`, `container-memory`, `block-read`, `block-write`,
  `nic-rx`, `nic-tx`), drawn as server-side SVG sparklines in a *Metric
  history* card: one SVG path per series scaled to the hour and the peak, with
  gaps between missing samples, a
  legend with the latest values and an `aria-label` summary. Series colours
  are stylesheet classes, so the page keeps its script-free CSP without
  inline styles. A metric with no samples says so, and a failed read shows
  the Redfish error. The six reads run concurrently with the others; Redfish
  answers them from Prometheus ([redfish.md](redfish.md#metric-history)).

While the agent is connected, the guest-agent card also shows the filesystem
freeze state and the last update-hook run (state, run id, exit code or
signal, end time). Update errors and output appear only for admins, because
Redfish sends them only to admins. Freeze, thaw and update themselves are
Redfish admin actions offered on the full BMC page's Guest agent tab.

| Route | Purpose |
| --- | --- |
| `GET /` | VMs visible to the user: phase/ready (`PoweredOff` or `Paused` for a guest off or paused inside its Pod), node, the first NIC's IP, vCPUs and memory (current, with maxima labelled), disks, CPU/memory snapshot from `metrics.k8s.io`, role |
| `GET /vms/{ns}/{name}` | Spec and status, cloud-init summary, disks, `status.resize`, Pod requests/limits, live usage, guest NICs, and the BMC panel ([below](#the-bmc-panel)) |
| `POST /vms/{ns}/{name}/action` | Admin form power action (`action`, plus `confirm=yes` for disruptive ones): one Redfish ResetType from `Systems/1`'s allowable values, or `on` / `off` as for the JSON route |
| `POST /vms/{ns}/{name}/delete` | Admin deletion of a VM launched by the dashboard; the PVC is kept |
| `GET /vms/{ns}/{name}/power` | Admin JSON power state: `running` (spec), operator `phase`, `pod`, Redfish `power_state` and `health` from `Systems/1`, the allowed in-pod `reset_types` from `ResetActionInfo`, `redfish_error` |
| `POST /vms/{ns}/{name}/power` | Admin JSON power request `{"action": ..., "confirm": bool}`: `on`, `off`, or a Redfish ResetType (`Resume`, `Pause`, `GracefulRestart`, `ForceRestart`, `PowerCycle`, `Nmi`, `GracefulShutdown`, `ForceOff`) |
| `GET /vms/{ns}/{name}/console` | noVNC page with power controls (admin) |
| `GET /vms/{ns}/{name}/bmc` | Full BMC page with live status, metrics, configuration, events and role-appropriate controls |
| `GET /vms/{ns}/{name}/console/ws` | Console WebSocket proxy (admin, exact Origin) |
| `GET`/`POST /launch` | Launch form (launchers) |
| `GET /images` | Base-image catalog and how images are staged (any signed-in user) |
| `GET /vpn` | ProtonVPN profiles and the server browser (launchers; [below](#vpn)) |
| `POST /vpn`, `POST /vpn/{name}`, `POST /vpn/use`, `POST /vpn/{name}/delete` | Create a profile; change its server (`action=server`, a server name or `auto`) or settings (`action=settings`); use a listed server for a profile; delete an unused profile |
| `GET /auth/login`, `GET /auth/callback`, `POST /auth/logout` | OIDC |
| `GET /healthz`, `GET /readyz` | Probes |

### Full BMC page

**Open BMC** on a VM's detail page opens `/vms/{namespace}/{name}/bmc` on
`vms.yapplesauce.com`. This uses the same HTML, stylesheet and JavaScript as
the per-VM BMC page: console, Overview, Metrics, Events, Guest agent and
Configuration. Viewers can inspect the page; console and management actions
require admin access. The existing **Open console** page remains available.

The dashboard supplies the selected VM's session information, proxies the
console through its existing WebSocket route and sends management requests
to that VM's Redfish sidecar through `/vms/{namespace}/{name}/bmc/redfish/*`.
Every request checks VM visibility and Pod ownership. Mutations require the
exact dashboard Origin and session CSRF token, and Redfish independently
authorizes the user's server-side ID token. Live event streams are bounded
and close when the dashboard session ends. No per-VM ingress, certificate,
public URL or OIDC callback is needed. BMC assets are embedded in the dashboard
binary, so interface changes require a dashboard image build and rollout.

The interface shows access loading separately from live VM data. A failed live
read labels the last successful snapshot and disables state-dependent actions
until a read succeeds. Management requests show pending/accepted/error feedback;
polling cannot unlock a pending request, and other management controls remain
disabled until the action and its follow-up read settle. Acceptance does not
claim that a guest reboot or shutdown has finished.

Status and metric polls pause in a hidden tab and resume when it becomes visible.
Overlapping reads are coalesced. Session expiry closes the event stream, stops
polling and offers sign-in back to the same VM's BMC page. Only the VM detail,
BMC and console paths are allowed as VM sign-in return destinations. Fetch
routes return JSON errors; an HTML error from an ingress is shown as a short
availability message, never as a document dumped into the error banner.
Action URLs advertised by Redfish are normalized and restricted to the selected
VM's Redfish bridge; they cannot select a different dashboard VM or origin.
Rare power actions are grouped under **More power actions**, with the same
two-step confirmations. On narrow screens, **VM panels ↓** moves focus past the
console to the details. Event rows expand with Enter or a tap to show the full
timestamp, sequence and wrapped payload; they do not depend on hover tooltips.
The full BMC updates known power controls from the allowable values on each
existing System poll. Newly removed actions become hidden and disabled; newly
advertised known actions become available without a page reload. Empty groups
disappear with an explicit no-actions message. Controls keep their DOM nodes,
so an availability change does not rebuild a pending action. The initial System
list avoids a separate discovery request when present; otherwise ResetActionInfo
provides the fallback. If discovery fails, the power area explains how to retry,
and a later System list can restore controls through the existing poll. Missing
inline values use the initial successful list. No extra polling request or new
action type is added. Server authorization is unchanged.
While discovery is pending, the power area says it is loading; known controls
can recover from System polling before that request finishes. Metric polling
also starts independently of optional startup reads.
If initial access metadata fails to load, the BMC offers a same-page reload
instead of implying that sign-in expired. A 401 still shows the sign-in path.
Event-history failures stay in the Events panel, independently of the live
stream's status. Incoming events do not hide the warning about missing past
records. Empty history, pending history and waiting for live events have distinct
messages. Queued stream callbacks are ignored after session cleanup.
Power and guest-agent requests retain keyboard focus while pending, using
`aria-disabled` with the existing request guard. A focused action that becomes
unavailable moves to the VM state or guest-agent tab. Blocked held keys cannot
activate after settlement, and Enter autorepeat is not a confirmation gesture.
Closing the optional power group clears its armed confirmation. Expiry stops
pending feedback, exits application full-screen mode and moves dashboard focus
to visible sign-in; unrelated VM navigation focus is preserved. Late action
refusals cannot reopen an error after the signed-out view appears.
Full-screen ownership also counts as console focus if a disconnected socket
has already removed the focused canvas. The real-router browser check enters
full-screen before natural expiry; `QRAB_UI_EARLY_DISCONNECT=1` additionally
closes the console first to exercise this recovery race.
Metrics, Events and Guest agent name the browser timezone used by the page.
Date formatters use that same zone for the page lifetime and account for
daylight-saving transitions; expanded event timestamps remain explicitly UTC.
Formatters are cached for repeated chart/event rendering. Browser checks cover
Los Angeles winter/summer times and Kolkata's half-hour offset.

### Console power controls

The dedicated console keeps routine controls visible and groups Reset, Power
cycle, NMI and Force off in a native “More power actions” disclosure. It hides
the group when none of those resets are offered. Closing the group, or losing
an offered action, clears that action's armed confirmation. Unoffered controls
are hidden and disabled, and the click handler rechecks their eligibility.
If the group disappears while focused, focus stays near the power state. If
the session ends while a console control is focused, focus moves to sign-in
after leaving full-screen mode. Background expiry does not steal focus from
navigation outside the console.
Pending actions show “Sending…” and then “Updating state…” on the active
button, with `aria-busy` and a status line outside the optional menu. Controls
stay locked until the request and follow-up read settle. “Request accepted”
describes delivery, not completion of a guest transition, and does not replace
the console connection status. Action failures have a separate alert so a
failed state read cannot overwrite the command's outcome. A new request clears
the previous action error; session expiry clears pending feedback and late
responses cannot restore it.
Both console views allow Tab entry into the display and show keyboard focus.
Ordinary Tab and Escape remain guest keys. Ctrl+Alt+Shift with left Alt returns
to console controls, leaving console full screen first; Right Alt/AltGr remains
available to the guest. Leaving display focus releases held keys. Shared static
keyboard handling is served at `/assets/console-keyboard.js`; it adds no management
endpoint and uses no private noVNC API.
An ordinary disconnect restores the connection prompt in the display; powered-off
and expired-session states keep their own guidance.
The full BMC also reflects known guest-agent prerequisites for freeze and update,
with visible action-specific reasons. Thaw remains available for connected-agent
recovery. These checks use the existing System snapshot; missing optional status
does not become a denial, and the backend remains authoritative.
The active pending control uses `aria-disabled` with the existing busy guard,
so keyboard focus survives the request. Other unavailable controls remain
natively disabled. If the focused action becomes unavailable after settling,
focus moves to the power state; unrelated navigation focus is preserved.
Power-state and sign-in recovery targets are brought into view, including in
a scrolled 320×320 window.
Activation keys pressed while blocked remain suppressed through key release,
even if the request settles in between. Enter autorepeat does not count as
another confirmation gesture.

Dashboard HTML errors keep their HTTP status and provide a next step: sign-in
after expiry, a VM/list link for failed requests, and an explanation for access
denials. VM sign-in destinations pass the same allowlist as normal login.
Unknown pages have a readable 404; invisible and absent VMs remain identical.

The console page's toolbar has **Power on**, the in-pod Redfish resets
**Resume**, **Pause**, **Restart guest** (`GracefulRestart`), **Reset**
(`ForceRestart`), **Power cycle** (`PowerCycle`), **NMI** (`Nmi`), **Shut
down** (`GracefulShutdown`) and **Force off** (`ForceOff`), and **Power off**,
enabled from a state poll every 3 seconds while the tab is visible. An in-pod
button is shown only while the VM's `Systems/1/ResetActionInfo` lists its
ResetType (Restart guest also needs a connected guest agent at action time),
and enabled from `Systems/1` `PowerState`: Resume while `Paused`; Pause,
Restart guest and NMI while `On`; Reset and Shut down while `On` or `Paused`;
Power cycle also while `Off`; Force off unless `Off`. The full BMC and detail
page use the same rules for these existing operations. Every button except
Power on, Resume and Pause arms on the first click ("Confirm reset") and sends
only on a second click within 5 seconds; the server also refuses them without
`"confirm": true`.

The BMC, detail page and dedicated console disable guest restart when their
current snapshot reports a disconnected/disabled guest agent or frozen
filesystems, and show the reason beside the controls. Other actions retain
their existing rules. The console's admin-only power snapshot includes an
optional `guest_restart_unavailable` explanation derived from the same System
read; it adds no upstream request. Missing/unknown agent information is not a
permission grant, and the backend still performs its action-time checks.

- The in-pod buttons send `ComputerSystem.Reset` with the matching ResetType
  to the running VM's Redfish service with the user's ID token, for any VM
  with the Redfish component. On a VM without it they are refused with a 409
  that says in-pod actions need `components.redfish` and that Power on and
  Power off still work. Shut down and Force off leave the VM powered off in
  its Pod (`PowerState` `Off`).
- **Power on** is enabled while `spec.running` is false or `PowerState` is
  `Off`. While the VM is set to run, has Redfish and its Pod exists, it sends
  Redfish `On` to that Pod, which starts QEMU again in place (a no-op while
  on). Otherwise it sets `spec.running: true`.
- **Power off** sets `spec.running: false`. Both `spec.running` patches are
  merge patches that carry the VM's UID (so they cannot reach a recreated VM
  of the same name). Power off is the operator's stop: it ends the Pod and
  keeps the VM off.
- After Power on, Reset, Restart guest, Power cycle or Shut down, the console
  reconnects by itself, backing off from 1 to 30 seconds, for two minutes.
  Disconnect or Power off cancels that.
- While the VM is off (`PowerState` `Off`, or `spec.running` false with no
  Pod) the console shows a powered-off state, stops retrying and disables
  **Connect console**; the power buttons stay as above. A console that was in
  use (open or reconnecting) when the VM went off, or after Power on, starts
  that reconnect once the state poll reads `On` or `Paused`.
- Force off during a Shut down's ACPI grace cuts the grace short; the VM
  rests `Off` with reason `force-off`.

`POST /vms/{ns}/{name}/power` takes JSON with `X-CSRF-Token` and the exact
Origin, and is admin-only (viewers 403, invisible VMs 404), like the console.
Redfish enforces the role again: its 403 (for example a grant removed since
sign-in) is returned as a 403 naming the refused action, a 409 as a 409, and
other failures as 502.

The dedicated console keeps its existing plain-text power API errors and shows
their messages above the display. On expiry it stops polling and reconnecting,
closes the socket, clears the display, exits the display's full-screen mode and
offers sign-in back to the same console. Pending actions remain locked through
their follow-up state read.

The detail page keeps a no-script power form for the same in-pod resets,
rendered from the allowable values in the `Systems/1` it already read and
posted to `POST /vms/{ns}/{name}/action`; its result, including a Redfish
refusal, comes back as the page notice. A VM with the BMC has Power on and
off on its console page. A VM without the BMC has no console page, so its
detail form also offers **Power on** and **Power off** (confirmed), routed as
above: a Redfish-only VM gets Redfish `On` while its Pod is up, and a VM with
neither component gets only the `spec.running` patches and a note that
in-pod actions need `components.redfish`.

### Roles

The dashboard grants each VM the same roles as its BMC (and, through the
operator's merged lists, its Redfish service):

- **admin**: the subject is in `spec.console.allowedSubjects`, or a token
  group is in `console.adminGroups`;
- **viewer**: the subject is in `console.viewerSubjects` or equals the
  `qrab.yapplesauce.com/creator-subject` annotation (which the operator adds
  to the BMC's viewers), or a token group is in `console.viewerGroups`;
- otherwise the VM is invisible: it is not listed and its pages are 404, as for
  a VM that does not exist. `components.redfish`'s own lists grant nothing in
  the dashboard.

On a Redfish-only VM (`bmc.enabled: false`, `redfish.enabled: true`) the
operator does not merge the console's lists into Redfish's, so Redfish decides
by `components.redfish`'s lists alone. A dashboard admin there must also be a
Redfish admin, or Redfish refuses the resets (shown as a refused action); the
dashboard role still comes from `spec.console` (or the creator annotation), so
such a VM is visible only with a `console` grant.

Admin wins when both apply. Deleting additionally requires the VM to carry
`app.kubernetes.io/managed-by: qrab-dashboard`, which only dashboard launches
set, so qrab-redfish-poc and other hand-made VMs cannot be deleted here. Kanidm group changes reach the dashboard, the
BMC and Redfish at the next sign-in; sessions last at most 15 minutes.

The VM directory shows configured disk counts separately from reported
capacity. A missing capacity report says “Capacity not reported”; it does not
classify a pending provisioned disk as static or count an unreported size as
zero. Reported capacity may cover only some of a VM's configured disks.
The images page explains the independent writable VM disks first. Detailed
verification and staging instructions remain available in a native disclosure.

### Launching a VM

The form shows the effective name limit for its namespace, including the generated
access-group names (43 characters in `qrab`). The server's existing validation
still enforces the limit; the browser no longer invites a longer name that will
be rejected after submission. A namespace that leaves no room for a VM name gets
an unavailable message instead of an unusable form.

The image catalogue shows names and generations separately, matching the optional
data-disk fields. Its form link opens a new tab so in-progress values stay in place.
Leave every field of an unused data-disk row empty; a clone needs both image fields.
Within the optional disk section, the first row starts open and additional rows
have native disclosures. Submitted values and field errors reopen their own row.
Case-sensitive identifier and cloud-init controls disable automatic capitalization
and spellchecking hints.
Hot-plug limits and additional data disks are optional disclosures. Submitted
values reopen their section when the form is rejected. Numeric controls inside
closable sections use a numeric keyboard and server validation: native browser
constraint validation cannot focus an invalid input in a closed disclosure.
The server checks all ranges before provisioning anything. Required fields
outside those sections retain native validation too.

The form takes an operating system (see [Base images](#base-images)): "None:
demo ESP only" or one catalog image, with a root disk size in GiB (default 1,
at least the image size rounded up to whole GiB, at most 1024). It also takes a
name (DNS label, at most 50 characters, and
`qrab_vm_<namespace>_<name>_viewers` at most 64), vCPUs and an optional
maximum, memory and an optional maximum, the accelerator (KVM or TCG) and
up to four data disks (ID, GiB, optional base image name
and generation; they use `DASHBOARD_STORAGE_NODE`), a network interface and
cloud-init (below). The chosen image must
still be in the catalog when the form is submitted; an unknown or removed
choice, a root disk below the minimum, a data disk named `root` alongside an
image, or an image without a configured storage node is a 400 and creates
nothing. On submit:

1. Preflight: the VirtualMachine, the PVC `<name>-state` and both Kanidm
   groups must not exist. An existing group is never adopted, because its
   members would gain access. A `<name>-state` PVC is reused only if the
   dashboard created it and it is still `Pending` (never bound, so it holds
   no TPM identity); this is checked again just before step 3.
2. Kanidm, with the service-account token: create
   `qrab_vm_<ns>_<name>_admins` and `_viewers`, nest both in `qrab_vm_users`,
   and add the launcher's subject to `_admins`.
3. Create the PVC `<name>-state` (`zfs-block`, `ReadWriteOncePod`,
   `Filesystem`, 1Gi).
4. Create the VirtualMachine with `bootDisk: /opt/qrab/demo/boot.img`,
   `running: true`, `console: {adminGroups, viewerGroups}` (no `publicUrl`),
   `nics` and `cloudInit` as described below,
   `storageNode` and `disks` when disks are requested (an OS image adds
   `{id: root, sizeBytes, baseImage: {name, generation}, bootIndex: 0}`
   first, so OVMF boots the clone and falls back to the demo ESP at boot
   index 1), the creator annotation
   set to the launcher and the label `app.kubernetes.io/managed-by:
   qrab-dashboard`. The form has no Redfish options: the Redfish sidecar that
   comes with the BMC serves the dashboard, and the launcher is its admin
   through the console lists.

If a step fails, the result page names the step, what was created, and what
was rolled back: groups created by that request are deleted again, by name
if their UUID could not be read back after one retry. The PVC
cannot be rolled back (the dashboard has no PVC delete permission); a retry with
the same name reuses it while it is unbound.

### Network and cloud-init on the launch form

Existing VPN-backed interfaces are marked **VPN tunnel only** on the VM detail
page. This is a routing-mode summary, not a handshake/connection-health claim.
The Secret reference and tunnel keys are not part of the dashboard view model.
Default-allow guidance applies only to direct interfaces; the launch form still
offers the same direct-interface configuration as before.

- **Network.** "Attach a network interface (eth0)" is checked by default and
  renders `spec.nics: [{name: eth0, securityGroups}]`. Security groups are
  comma-separated DNS labels, at most 8, distinct. Each becomes the NIC Pod
  label `sg.qrab.yapplesauce.com/<group>: ""`, which CiliumNetworkPolicies
  select. A NIC that no policy selects is default-allow, as the form says
  ([networking.md](networking.md#security-groups)). Unchecked, the VM has no
  NIC; security groups without the NIC are a 400. The form offers one NIC
  (`launch::FORM_NICS`); raising it adds `eth1`... rows with fields
  `nic<i>_enabled` and `nic<i>_security_groups`.
- **Cloud-init.** "Hostname" (DNS label, default: the VM name, so it is left
  out of the CR) and "User-data" (monospace, at most 32 KiB after the
  browser's CRLF line breaks become LF). `spec.cloudInit` is rendered when
  either is given, and always with an OS image (cloud images expect a seed;
  then `cloudInit: {}` means hostname = VM name and an empty
  `#cloud-config`). A VM with a NIC gets a seed with static network-config
  from the operator in any case. The launch route accepts bodies up to
  128 KiB for this; every other route keeps 16 KiB.
- **user-data is never shown or logged.** It is stored in the VM's ConfigMap,
  which the form says, so it must not hold secrets. The dashboard never
  renders it: a rejected form is shown again without it and asks for it
  again, validation errors give only its size, the request's Debug output
  shows only its size, and the detail page shows "hostname X, user-data N
  bytes".

The launcher's current ID token predates the new `_admins` group. Until they
sign in again they are the VM's viewer as its creator; after that, its admin.

Deleting a VM (console admin of a dashboard-launched VM, type its name to
confirm) deletes the VirtualMachine
with a UID precondition. Its Pod, ConfigMap and disk Secrets follow. The
state PVC stays: it holds the firmware variables and TPM identity, and an
administrator deletes it deliberately. Provisioned guest disks retain their data
after their connections are closed (see [retained storage](iscsi.md#destroying-retained-data)).
The Kanidm groups stay too. The confirmation form repeats the exact VM name and
explains these retained resources; an administrator must clean up the existing
PVC and groups before the dashboard can reuse the name. Power confirmation
checkboxes identify both their action and VM to assistive technology, and their
click targets include the full label height. These changes do not alter the
server's exact-name and explicit-confirmation requirements.
The detail page keeps routine power controls visible and puts Reset, Power
cycle, NMI and Force off under “More power actions,” matching the BMC. This is
a native disclosure; each action still has its own confirmed POST form, and
only resets currently offered by the VM's Redfish service are rendered. An
empty offered-action list has an explicit message instead of an empty toolbar.
The detail page also disables actions incompatible with its guest-power
snapshot (for example Resume while On, or Pause while Paused/Off). It shows
the snapshot state beside the controls and says to reload after a state
change. Persistent Power on/off retain their separate desired-power/Pod rules;
the server and Redfish still validate every submitted action independently.

### The detail page

The Overview card adds "Cloud-init": `hostname <name>, user-data <N> bytes`
(or "no user-data"); a VM with NICs but no `spec.cloudInit` shows "seed for
the NICs only", and a VM with neither "none". The Network card lists
`spec.nics` joined with `status.nics` by name: NIC, MAC, IP, security groups,
NIC Pod and ready, as stacked cells on phones. Guest traffic leaves from the
NIC Pod's IP; the VM Pod (BMC) never carries it. A VM without NICs shows "No
guest NIC." The VM list shows the first NIC's IP, or "—".

## Base images

A base image is a sealed, read-only ZFS snapshot
`tank/qrab-iscsi/image-<name>-g<generation>@sealed` on the storage node. A VM
launched with it gets `spec.disks[] = {id: root, baseImage: {name,
generation}, bootIndex: 0}`; the operator and storage service make that disk
a private copy-on-write clone of the snapshot and export it over iSCSI, and
OVMF boots its EFI System partition. The image never changes; a new release
or rebuild is a new generation. The VM detail page shows "Boots from:
CirrOS 0.6.3 (cirros g1) clone on root" for such a VM, else "demo ESP".

The dashboard reads the catalog from ConfigMap `qrab/qrab-base-images`, key
`images.json`, on every Images page and launch request (RBAC: `get` on that
one ConfigMap). It is a JSON array of objects with exactly these fields:

| Field | Meaning |
| --- | --- |
| `name`, `generation` | The snapshot `image-<name>-g<generation>`; name is 1-32 of `[a-z0-9-]`, generation 1-999999 |
| `title`, `os`, `version` | Display text, e.g. `CirrOS 0.6.3`, `cirros`, `0.6.3` |
| `size_bytes` | Raw image size; the root disk minimum is this rounded up to whole GiB |
| `sha256` | SHA-256 of the imported raw image |
| `upstream_sha256`, `source_url` | The downloaded release file and where it came from (HTTPS) |
| `boot` | `uefi` |

Entries with missing, extra or invalid fields, and duplicates of an earlier
`(name, generation)`, are skipped with a logged warning.

### Staging an image

Staging is an administrator task with three parts: an unprivileged download
and conversion, a root import on the storage node, and the catalog update.
`contrib/images/stage-base-image.sh` drives all three. Its built-in table
knows CirrOS 0.6.3 (x86_64, GPT with an EFI System partition, 117440512 bytes
virtual size).

```sh
bash contrib/images/stage-base-image.sh cirros 0.6.3 1
```

1. It downloads
   `https://download.cirros-cloud.net/0.6.3/cirros-0.6.3-x86_64-disk.img`
   (`curl -fL`, HTTPS only; the URL redirects to GitHub) into a private
   temporary directory, checks the upstream SHA-256
   `7d6355852aeb6dbcd191bcda7cd74f1536cfe5cbf8a10495a7283a8396e4b75b`,
   converts it with `qemu-img convert -O raw` and checks the raw size against
   the image's virtual size.
2. It copies the raw image to `/var/tmp/qrab-image-cirros-g1.raw` (0644) and
   prints the root command with the raw SHA-256, for example:

   ```sh
   sudo /usr/local/sbin/qrab-iscsi-target vm-image-import --image cirros --image-generation 1 \
     --source /var/tmp/qrab-image-cirros-g1.raw --sha256 <raw sha256>
   ```

   A local run on 2026-11-12 produced raw SHA-256
   `809dc3e0e993cd8ec736e28d55b8356ccc4ee9051cad9ad1be66fad9a79470f1` for
   CirrOS 0.6.3; use the value the script prints. The import verifies that digest, writes the zvol and seals
   `image-cirros-g1@sealed`, exactly as `vm-image-publish` does. Run it on
   the storage node (`DASHBOARD_STORAGE_NODE`), then press Enter.
3. It merges the entry into `images.json` by `(name, generation)`, keeps the
   array sorted, and applies the ConfigMap with `kubectl apply --server-side
   --field-manager=qrab-stage-base-image`. The image then appears on
   `/images` and the launch form.

`--catalog-only` repeats only step 3 from the staged raw file, for example
after the import had to be retried. The script handles no credentials. Do
not add a catalog entry for an image that has not been imported and sealed:
VMs launched with it would wait for a snapshot that does not exist.

## VPN

`/vpn` manages ProtonVPN profiles (docs/networking.md, "ProtonVPN"). Profiles
are cluster-wide, so the page, its forms and its nav link exist only for
members of the launcher group; everyone else gets a 404.

- **Profiles** list each profile's server (Auto or fixed), location, load,
  entry IP, forwarded port (or why there is none), the VM NICs using it, the
  broker's Ready condition and the certificate's expiry. Each has forms to
  pick a server (`auto` or a name such as `CH#12`), change its settings
  (country and city for Auto, server features, NetShield, port forwarding,
  Moderate NAT, VPN Accelerator) and delete it, which is refused while a NIC
  uses it.
- **Servers** come from ConfigMap `proton-catalog`, which the broker
  publishes (no addresses or keys). The list applies the broker's rules (up,
  within the plan's tier, Secure Core and Tor only when that feature is
  chosen), filters by country, feature, maximum load and name, and shows up
  to 100, best score first; each row can be used for any profile.
- A NIC uses a profile through `vpn: {secretName: vpn-<profile>}`. Changing a
  profile's server moves its tunnel within a minute and changes the exit
  address, so open connections drop.

The dashboard writes only `VpnProfile` specs (patches carry the profile's
UID, deletes a UID precondition). It cannot read the profile Secrets, the
Proton session or the keys: its Role adds `vpnprofiles` get/list/create/patch/
delete and `get` on ConfigMap `proton-catalog` only.

## Configuration

| Variable | Value |
| --- | --- |
| `DASHBOARD_PUBLIC_URL` | `https://vms.yapplesauce.com` |
| `DASHBOARD_OIDC_ISSUER` | `https://idm.yaple.net/oauth2/openid/qrab-dashboard` |
| `DASHBOARD_OIDC_CLIENT_ID` | `qrab-dashboard` |
| `DASHBOARD_OIDC_CLIENT_SECRET_FILE` | File with the confidential client's secret (client_secret_basic; PKCE as well) |
| `DASHBOARD_LAUNCHER_GROUP` | `7617e8d9-74ce-4525-a70b-80235206b06a` (`qrab_vm_launchers`) |
| `DASHBOARD_VM_USERS_GROUP` | `3d40e2a1-3968-43ea-b4a4-154cbd26d88b` (`qrab_vm_users`) |
| `DASHBOARD_KANIDM_URL` | `https://idm.yaple.net` |
| `DASHBOARD_KANIDM_TOKEN_FILE` | File with the API token of service account `qrab-dashboard-groups` |
| `DASHBOARD_NAMESPACE` | Default `qrab` |
| `DASHBOARD_STORAGE_NODE` | Storage node for data disks, e.g. `kube03`; without it, launches with disks are refused |
| `DASHBOARD_LISTEN` | Default `0.0.0.0:8080` |
| `DASHBOARD_CA_FILE` | Optional PEM CA for an isolated test Kanidm |
| `DASHBOARD_ALLOW_LOOPBACK_HTTP` | `true` permits HTTP only for a loopback dashboard origin (development) |
| `DASHBOARD_NOVNC_DIR` | noVNC directory; the image sets `/opt/qrab/dashboard/novnc` |

The login requests `openid profile groups_uuid`, so the `groups` claim holds
group UUIDs. Per-VM groups are created at run time, so the session keeps every
UUID in the claim (at most 1,024).

## Kanidm provisioning

Production is provisioned (client `qrab-dashboard`, group `qrab_vm_launchers`,
service account `qrab-dashboard-groups`, Secret `qrab/qrab-dashboard`). For a
new installation, as a Kanidm administrator (`kanidm login -D idm_admin`):

```sh
# Confidential client with one strict callback; PKCE stays enabled.
kanidm system oauth2 create qrab-dashboard 'qrab VMs' https://vms.yapplesauce.com/ -D idm_admin
kanidm system oauth2 add-redirect-url qrab-dashboard https://vms.yapplesauce.com/auth/callback -D idm_admin
kanidm system oauth2 enable-strict-redirect-url qrab-dashboard -D idm_admin
kanidm group create qrab_vm_launchers -D idm_admin
# update-scope-map replaces the set: always pass every scope.
for group in qrab_vm_users qrab_vm_launchers qrab_operators; do
  kanidm system oauth2 update-scope-map qrab-dashboard "$group" openid profile groups_uuid -D idm_admin
done
# Group administration for the launch flow.
kanidm service-account create qrab-dashboard-groups 'qrab dashboard group administration' idm_admins -D idm_admin
kanidm group add-members idm_group_admins qrab-dashboard-groups -D idm_admin
kanidm service-account api-token generate qrab-dashboard-groups qrab-dashboard --readwrite -D idm_admin
kanidm system oauth2 show-basic-secret qrab-dashboard -D idm_admin
```

The kanidm CLI exits 0 even when a command fails. Read its output, then check
the result explicitly: `kanidm system oauth2 get qrab-dashboard` (redirect,
scope maps, `oauth2_rs_basic_secret` present) and
`kanidm group list-members idm_group_admins`. Users get in when they are members
of `qrab_vm_users` (directly or through a per-VM group), `qrab_vm_launchers`
or `qrab_operators`.

Copy the client secret and the API token straight into
`contrib/kubernetes/dashboard/install-secret.py`. It reads both with hidden
prompts, renders Secret `qrab/qrab-dashboard` (keys `oidc-client-secret` and
`kanidm-token`) with `kubectl create secret --dry-run=client` from inherited
pipes, and applies it with `kubectl apply --server-side` from stdin, so no
`last-applied-configuration` annotation copies the credentials. It never
prints them or puts them in arguments or files.

## Deployment

`contrib/kubernetes/dashboard` holds a namespaced Role
(VirtualMachines get/list/create/delete/patch, Pods get, PVCs get/create,
ConfigMap `qrab-base-images` get, `metrics.k8s.io` pods list), the Deployment (one replica, `Recreate`,
non-root UID 10001, read-only root, no capabilities, credentials mounted as
files), the Service, the Ingress and Certificate for `vms.yapplesauce.com`,
and two CiliumNetworkPolicies. The first lets the dashboard receive from the
ingress and host, and reach the API server, DNS, Kanidm (192.168.0.251:443)
and VM Pods on 8443 (BMC workload console) and 8444 (Redfish workload API). The second,
`qrab-vm-from-dashboard`, selects every VM Pod and admits the dashboard on 8443
and 8444. Cilium policies are additive, so per-VM policies such as
`qrab-demo` (the qrab-redfish-poc console's) keep working and gain only the
dashboard. A VM Pod
without a policy of its own, such as every dashboard-launched VM, becomes
ingress default-deny once this policy selects it: only the dashboard to 8443
and 8444 is admitted. Its egress is not
restricted by this policy.

```sh
bash contrib/images/build.sh dashboard <tag>
bash contrib/images/publish.sh dashboard <tag>   # pin the printed digest in deployment.yaml
kubectl apply -k contrib/kubernetes/dashboard
kubectl -n qrab rollout status deployment/qrab-dashboard
```

Record the same digest in `contrib/images/images.lock.json`, as for the other
images.
Bearer access also needs the operator's `QRAB_BMC_BEARER_ISSUER` and
`QRAB_BMC_BEARER_AUDIENCE`, and the admission policy that trusts only
`system:serviceaccount:qrab:qrab-dashboard` to set the creator annotation
([operator.md](operator.md#the-creator-is-a-viewer)). The private image pull
identity belongs to Kubernetes GitOps, like the operator's: the
`qrab-dashboard` ServiceAccount (never pruned, because node pull policies bind
its UID), its pull-audience and `use` RBAC, the protected-account admission
guards, and its `spiffe-node-pulls.py` entry for `zot.yapplesauce.com/qrab/dashboard`.

## Security notes

- OIDC: authorization code with S256 PKCE, 32-byte state and nonce,
  client_secret_basic at the token endpoint, ID token signature, issuer,
  audience, nonce, expiry and access-token hash checks. Discovery and
  endpoints must be HTTPS; redirects are not followed.
- Sessions: a bounded in-memory store (4,096), at most 15 minutes and never
  past the token `exp`, rotated at sign-in, `__Host-qrab-dashboard` cookie
  (Secure, HttpOnly, SameSite=Lax). A restart signs everyone out. When the
  store is full, a new record evicts the oldest pending (not yet signed-in)
  login, so a flood of `/auth/login` cannot block sign-ins; signed-in sessions
  are never evicted.
- Every POST (logout, launch, power, delete) needs the session's CSRF token
  (a form field, or `X-CSRF-Token` for the console page's JSON power
  requests) and the exact Origin before anything else happens.
- VM `patch` is for power on/off only. The ValidatingAdmissionPolicy
  `qrab-dashboard-vm-updates` (`contrib/kubernetes/operator/dashboard-update-policy.json`,
  generated by `qrab-operator dashboard-update-policy` from the CRD and
  compared by `contrib/check-runtime.sh`) refuses any update by the
  dashboard's ServiceAccount that changes a spec field other than `running`,
  or any label or annotation. A compromised dashboard therefore cannot grant
  itself a VM's console. `check-dashboard-update-policy.sh` checks it live
  with dry runs. The console WebSocket
  needs a session, the exact Origin and the admin role, and closes within a
  second of sign-out or expiry; it rechecks the VM's grants every 30 seconds.
  At most 16 consoles are open at once, and at most 4 per subject.
- BMC and Redfish calls go only to the requested VM's own Pod IP. The
  dashboard and VM Pods talk plain HTTP inside the cluster network,
  restricted by the policies above; the bearer is an ID token for the
  `qrab-dashboard` audience, which the VM's BMC and Redfish accept only
  because the operator lists that pair (`QRAB_BMC_BEARER_*`).
- Guest- and VM-controlled strings (agent versions, errors, event payloads)
  are rendered as escaped text. Pages have a no-script CSP; the console page
  allows only same-origin scripts and WebSocket.
- The dashboard uses its Kanidm token only to create `qrab_vm_<ns>_<name>_*`
  groups, nest them in `qrab_vm_users`, add the launcher, and delete groups it
  created in the same failed launch.
- **Accepted risk: the Kanidm token's blast radius.** Service account
  `qrab-dashboard-groups` is in `idm_group_admins`, the same as the
  idm-dashboard writer. Whoever obtains its token can create and delete groups
  and change the membership of any unprotected group, including the groups
  mapped to Kubernetes roles, not only the per-VM groups. The token is mounted
  as a file readable only by the Pod's group, never in env, argv, logs or
  HTML; `install-secret.py` applies it server-side so no
  `last-applied-configuration` annotation copies it. Rotate it after any
  suspected exposure.

## Testing

`cargo test --locked -p qrab-dashboard` (part of `contrib/check-runtime.sh`)
runs offline against an in-memory cluster, a fake Kanidm and two fake VM Pods
(BMC console and Redfish) on 127.0.0.1 and 127.0.0.2. It covers role resolution, list filtering, launch
authorization, the rendered names, PVC and VirtualMachine, rollback, CSRF and
Origin on every POST, Redfish and console calls with the bearer only to the
selected Pod, the panel from `Systems/1`, the event log and a sparkline per
metric history answer, each power action
as its ResetType, Redfish reads and power for a Redfish-only VM, the
console kept BMC-only, the `spec.running`-only path and in-pod refusal for a
VM with neither, Redfish refusals as refused actions, the response bounds,
escaping of guest strings, the base-image catalog parser, the Images page,
OS launches (root clone, stale or unknown images, root minimum, a data disk
named root), NIC and cloud-init rendering and validation (user-data never
in a page, an error or a log line, and the larger launch body limit), OIDC
state handling and the console WebSocket's
refusals, proxying and revocation. A live Kanidm login and a browser console
session are part of the rollout check.

## Rollout record (2026-11-12)

A dated record; the qrab-demo VM it mentions has since been retired.

- Kanidm: confidential client `qrab-dashboard` (one strict callback,
  `https://vms.yapplesauce.com/auth/callback`), scope maps `openid profile
  groups_uuid` for `qrab_vm_users`, `qrab_vm_launchers` (7617e8d9…, member
  sam) and `qrab_operators`; service account `qrab-dashboard-groups` in
  `idm_group_admins`. Secret `qrab/qrab-dashboard` was installed without
  printing either credential and carries no last-applied annotation.
- kubernetes-gitops `c990160`: ServiceAccount `qrab/qrab-dashboard` (UID
  e368e4b5…), pull and `use` RBAC, the protected-account guards, and the
  operator re-pinned to `02c56cd9`. `spiffe-node-pulls.py --apply` enrolled
  `qrab/dashboard` on kube01–03.
- Images from `2ed767d`: manager `373b51bb…`, operator `02c56cd9…`. The
  dashboard `98cfeff4…` comes from `c208c2e`, which fixes the independent
  security review. Every node is a control-plane node, so the Deployment
  tolerates that taint.
- `creator-policy.yaml` applied. `check-creator-policy.sh` PASS, including the
  dashboard cases. The ServiceAccount can create, list and delete VMs, create
  PVCs and get Pods and metrics. It cannot delete PVCs, read Secrets or patch
  VMs.
- qrab-demo was replaced once for the bearer settings (Pod 3f9ce1e7, 0
  restarts). `check.sh` PASS with EK `ec61ceb9…e844` unchanged.
- `https://vms.yapplesauce.com`: certificate Ready; `/healthz` 200; login
  redirects to Kanidm with `client_id=qrab-dashboard`, S256 PKCE and
  `groups_uuid`.
- Not yet exercised: a browser login, the BMC panel and console through the
  proxy, and a real launch.

## Interface checks without a cluster

Dashboard data tables become labelled cards below 1000px. Capacity maxima are
labelled explicitly; exact whole memory capacities use GiB/TiB where applicable.
At phone widths up to 480px, table cards stack labels above values so addresses,
VM names and checksums can use the full row.
Empty VM lists explain account visibility and access refresh without exposing
hidden VMs; only launch-enabled accounts get the empty-state launch action.
Empty image catalogues point to administrator staging rather than leaving the
user with an unexplained blank list.
CPU usage is shown in cores and larger memory snapshots in GiB/TiB. Positive
sub-millicore samples remain distinct from zero, and invalid samples show no data.
SSR usage is a page-load snapshot, not a poll; the BMC provides continuing updates.
CPU history retains fractional millicores below one millicore; very small positive
samples say `<0.01 mcores`, distinct from zero. Both history views break lines
across missing intervals longer than two sample steps and visibly mark isolated
samples instead of losing them or connecting through missing data.
Byte-rate history also preserves fractions below 1 B/s, with `<0.01 B/s` for
smaller positive samples; true zero remains distinct. Light-theme faint text
has extra contrast on raised console surfaces.
The BMC Metrics tab keeps service-resource counters in a native disclosure so
the history charts are easier to reach; opening it explains cumulative CPU totals.
VM-report disk counters are labelled as totals. Current byte values use adaptive
binary units, keeping small values visible; tiny positive time totals use a
threshold label instead of rounding to zero.
Missing BMC configuration or guest-agent data is labelled unavailable; it is
distinct from an explicitly empty disk/NIC list or a disabled channel. A successful
System response without configuration clears the previous configuration values.
Polling updates individual BMC list values so a changing timestamp preserves
unchanged text selections and update-output focus and scroll, even when optional
neighboring fields appear or disappear. Long guest update
output has a bounded, labelled keyboard-scrollable region. If that output is
removed, focus returns to the Guest agent panel. SSR multiline diagnostic output
also has a labelled keyboard focus target.
Long VM names wrap in the BMC header, which scrolls normally in short windows so
it does not cover the useful panel area.
The signed-in account remains visible beside its role and Sign out on narrow
screens; long account names truncate without increasing the header height.
Names and status chips can wrap onto separate header rows at larger text sizes.
Dashboard cards wrap long event names, identifiers and other unbroken text.

The dashboard and BMC follow the system light/dark preference. Dashboard pages
remain server-rendered; the BMC uses the shared interface assets. Keyboard users
can skip the header, and the navigation identifies the current dashboard page.
Coarse-pointer devices get larger controls, navigation links and disclosure
targets, including tablets with wide viewports. VM directory name links have a
44px minimum touch target; precise-pointer layouts keep their compact links.

For a visual check, generate representative pages and run the isolated browser
checks. The Python environment needs `playwright`; use an existing Chromium
binary with `--browser` if it is not at the default path:

```sh
mkdir -p target/ui-preview
cargo run --locked -p qrab-dashboard --example preview -- target/ui-preview
python3 contrib/tests/ui-browser.py --previews target/ui-preview \
  --browser /path/to/chromium --output target/ui-browser
# Optional second engine in the same Python environment:
python3 -m playwright install firefox
python3 contrib/tests/ui-browser.py --engine firefox --previews target/ui-preview \
  --output target/ui-browser-firefox
# WebKit in an environment with its platform libraries installed:
python3 -m playwright install webkit
python3 contrib/tests/ui-browser.py --engine webkit --previews target/ui-preview \
  --output target/ui-browser-webkit
# Browser emulation of system high-contrast colors:
python3 contrib/tests/ui-browser.py --forced-colors --previews target/ui-preview \
  --output target/ui-browser-forced-colors
```

This checks desktop, tablet and mobile layouts down to 320px in light and dark themes, shared
BMC admin/viewer controls, keyboard tabs, event filtering and two-step power
confirmation. It also checks loading, stale reads and recovery, pending requests
across polls, denied actions, empty history/filters, powered-off layouts and
session expiry. `--states-only` runs just the BMC and dedicated-console state
checks during iteration. Console expiry also checks native WebSocket closure,
canvas cleanup, full-screen exit and the same-console sign-in destination.
Dedicated-console checks cover expanded controls down to 320px, confirmation
reset on close/availability loss, and recovery-link focus after expiry.
The event checks include malformed/missing timestamps and long payload text that
must render without becoming HTML.
Long names, large capacities, namespace-specific name limits and a 320×320
CSS-pixel viewport at 2× density are included in the layout checks.
Header reflow checks also use 24px and 32px text with a valid long VM name and
the longer power/health labels, checking both useful viewport space and panel access.
Metric fixtures include CPU, memory and I/O series as well as an empty series;
the checks inspect keyboard sample selection and chart-label fit. Chart labels
and readouts use larger text, with fewer time ticks on narrow plots. The history
panel explains the existing pointer and keyboard controls.
Focused charts keep their position through empty/error responses and recovery;
an unchanged message keeps its DOM and focus. Availability live regions announce
source failures and recovery, while routine sample timestamps remain quiet.
The browser checks observe these live-region mutations; they do not substitute
for testing with an actual screen reader.
Unchanged control titles and descriptions are not rewritten by ordinary polls.
Chart resize handling responds to width changes outside ResizeObserver's
delivery cycle, avoiding a WebKit resize loop. Resizing keeps a keyboard-selected
sample; a stationary mouse event from layout does not replace that selection.
The checks resize an already-focused chart and then verify genuine pointer input.
Pending chart resize work and its observer are stopped when the session ends.
Chart series use matching line patterns in their plots and SVG legend keys,
so color is not the only distinction. The keys are decorative to assistive
technology. Forced-colors mode uses the system chart colors and preserves
current-navigation, selected-tab and pressed-range indicators. The browser
checks compare plot/key styles and include an optional forced-colors run;
this emulation does not substitute for testing an actual OS accessibility setup.
It writes screenshots to the output directory. BMC requests use
in-memory browser fixtures. Lifecycle checks use a temporary loopback WebSocket
held during RFB negotiation; keyboard checks complete a minimal RFB 3.8 handshake
and capture key-down/key-up pairs. These fixtures send no guest framebuffer data.
The checks need no credentials,
cluster, VM or guest, and sends no action to a live VM.
The separate `bmc-browser.py` test covers real
OIDC and guest-console delivery.

### Browser checks through the real router

An ignored Rust test serves the actual dashboard router on loopback with the
in-memory cluster, bounded session store and fake Pod upstreams. Python receives
opaque test-session cookies on stdin; no test login route is added to the
application. It uses the real response headers, CSP, HTTP proxies and WebSocket
proxy rather than browser HTTP interception. It checks admin/viewer pages,
keyboard chart focus across redraws, rejected launch-form feedback, peer logout
and real server-time expiry.
Its fake console upstream holds RFB negotiation open, so this check proves
authenticated admission and browser lifecycle behavior rather than guest input
or framebuffer delivery.

```sh
QRAB_UI_PYTHON=/path/to/python-with-playwright QRAB_UI_ENGINE=firefox \
  cargo test --locked -p qrab-dashboard actual_router_browser -- --ignored --nocapture
# The same check over a full 15-minute session:
QRAB_UI_PYTHON=/path/to/python-with-playwright QRAB_UI_SESSION_SECONDS=900 \
  cargo test --locked -p qrab-dashboard actual_router_browser -- --ignored --nocapture
```

`QRAB_UI_OUTPUT` selects the screenshot directory (default `target/ui-router`).
The check has no guest, KVM, cluster or production credentials. Its invalid
launch submission must create no resources; it sends no action to a live VM.
The favicon is a shared same-origin SVG; SSR CSP permits
same-origin images for it while retaining the script-free policy. A chart's
keyboard focus and selected timestamp survive an ordinary history redraw.

Rejected launch forms retain non-secret values and identify each invalid field
with an inline message and an error-summary link. The summary receives native
browser focus after submission; linked fields in optional sections remain
visible. User-data is cleared instead of being echoed into the response.

## Workload TLS enrollment and verification

Stage [workload identities](../contrib/kubernetes/workload-identities/) through
kubernetes-gitops before deploying the new clients/servers. The dashboard's
registration pins its reviewed image; BMC/Redfish registrations bind namespace,
SA, container, UID and the controller-added Pod UID while preserving per-VM
identity across image updates and migrations. The operator mounts the same CSI
socket into each service; SPIRE issues only that container's selected identity.
No sidecar receives the manager's SVID or private runtime volume.

Release matching manager/operator images to enable BMC8443 and Redfish8444,
then release dashboard and its Cilium policy (VM backend egress/ingress only
8443/8444). The BMC's own browser adapter uses Redfish mTLS over loopback.
Do not infer that old explicit manager image overrides start the new BMC
listener: update such VM image pins deliberately and verify the resulting Pod.

For a running test VM, an operator can run the deployed dashboard's fixed
read-only transport probe with `kubectl exec deploy/qrab-dashboard --
qrab-dashboard workload-probe qrab VM-UID POD-IP bmc` (or `redfish`). It always
uses the selected service's fixed TLS port/path, sends no user token and prints
only expected service identity plus the resulting **401**. This proves workload
TLS admission while confirming application auth is still required; it is not
a user-authenticated console session or a general URL proxy. A wrong UID must
fail TLS rather than returning any HTTP response. No key material is printed.

The HTTPS clients require `qrab_spiffe::http::install_tls13_provider()` before
other TLS clients are built. This restricts cipher suites even for preconfigured
reqwest/rustls configs; an incompatible already-installed provider refuses
startup, rather than silently allowing TLS1.2. Redirects and environment proxies
are disabled, and per-service client scopes bind both the trusted VM ID and Pod IP.

The first management mTLS rollout completed on 2026-11-15 from source `793d404`:
operator0414a35b, dashboardf200cfe7 and manager2b845451. A fresh VM migrated
between nodes and the deployed dashboard verified exact service identities,
application401 without a user token, wrong-UID refusal and mandatory client
certificates. See the [dated evidence](connections/platform-evidence.md#management-workload-mtls-rollout-2026-11-15-22562304-utc)
for scope and verification limits. This does not disposition the separate
plaintext ingress/metrics/storage/link work.

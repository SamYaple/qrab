# Per-VM BMC

`qrab-bmc` serves one VM's browser console. Operator Pods run it as an optional
sidecar, `qrab-bmc --socket /run/qrab-control/manager/control.sock`, next to the
`qrab-manager` container that owns the VM ([operator.md](operator.md#components)).
`qrab-bmc --config /etc/qrab/vm.json` still runs the VM in-process for
standalone use. The manager image contains `qrab-manager`, `qrab-bmc` and
`qrab-redfish`. The BMC uses the same Kanidm authorization-code OIDC flow as
`idm-dashboard`, with S256 PKCE, state/nonce verification, signature/issuer/audience
and expiry validation. `/api/session` and the `/api/console` WebSocket can
also accept the dashboard's Kanidm ID tokens as bearer credentials ([below](#dashboard-bearer-tokens)). There are no
static tokens and no anonymous development bypass.

The demo is at **https://qrab-demo.yapplesauce.com**, using the cluster's private
DNS and Cilium ingress, like `idm-dashboard`. It requires LAN/private-DNS access;
this manifest does not add a public edge route. Sign in with Kanidm, then choose
**Connect console**. Video, keyboard, USB tablet input, Ctrl Alt Del and full
screen run through the vendored noVNC client. There are no CDN dependencies.
Everything else on the page (power state, power actions, guest-agent actions,
the live QMP event history, the effective configuration and metrics)
is read from and sent to the VM's in-pod [Redfish](redfish.md) service through
the BMC's [`/redfish` proxy](#redfish-proxy).

Shutdown ends the current VM instance. With spec.running=true, the operator
starts another; set spec.running=false to keep it off.

The RAM-only fixture has an interactive root shell on its virtual terminal.
Keyboard and absolute pointer devices are exposed to the guest; the fixture's
terminal is not a desktop environment. A graphical guest can consume the same
USB tablet events. The fixture has no persistent root filesystem or NIC.
Clipboard sharing, sound and guest networking remain outside this stage.
The minimal [operator](operator.md) now creates the per-VM Pod from a CR.

## Browser UI

The page is console-first (`projects/bmc/assets`, plain HTML, CSS and ES
modules):

- **Status header** (sticky): VM name, a `PowerState · Health` chip from
  `Systems/1`, the role badge and **Sign out**. The chip is re-read every 3 s
  and shortly after every `EventService/SSE` event, so pause, resume or a
  guest panic shows without a reload.
- **Console toolbar**, attached to the display: **Connect console** /
  **Disconnect**, **Ctrl Alt Del**, a **Fit** / **1:1** scaling toggle (noVNC
  `scaleViewport`; 1:1 scrolls inside the display), **Full screen**, the
  admin-bucket **Audio** toggle when enabled, and the **Power** group. The power
  buttons are the `ResetType` values Redfish's `ResetActionInfo` allows (Pause,
  Resume, Reset = `ForceRestart`, Restart guest = `GracefulRestart`, Shut down,
  Power cycle, NMI). Disruptive ones use an inline two-step confirm: the first
  click turns the button into **Confirm <action>** for 5 s and only a second
  click sends it. Pressing a toolbar or tab button while the console has
  keyboard focus keeps that focus; clicking anywhere else releases it.
- **Side rail** beside the console (stacked below it at 1000 px and narrower),
  with keyboard-accessible tabs (arrow keys, Home, End). The selected tab is
  remembered per user in the browser's `localStorage`.
  - *Overview*: power, health, vCPUs, memory, disks, NICs, guest network and
    guest-agent state.
  - *Metrics*: current values from `TelemetryService/MetricReports/VM` and
    `Managers/1/Oem/Qrab/Resources`, polled every 5 s only while the tab is
    shown (each report read takes a fresh QMP sample). The `#metric-history`
    slot is reserved for the history graphs of `work/bmc-metric-history-graphs`.
  - *Events*: the QMP event log and live stream, a name filter and the
    dropped-events warning.
  - *Guest agent*: agent status and the freeze (two-step confirm), thaw and
    update actions.
  - *Configuration*: `Oem.Qrab.Configuration`, redacted for viewers.

Viewers get the same header and rail, but the console toolbar, power group,
keyboard hint and guest-agent actions are not in the page at all. The palette
is dark by default and switches to a light one with the system preference.

## Runtime configuration

In sidecar mode the BMC needs the control socket and the Redfish sidecar on the
Pod's loopback. It keeps a local mirror of the manager's event history and
opens a short socket connection for each console request. The VM config,
which must
set `console: true`, belongs to the manager container. In embedded mode
(`--config`) the BMC runs the supervisor itself. See [manager.md](manager.md).
On SIGTERM the sidecar closes live streams within two seconds and exits 0.

| Variable | Meaning |
| --- | --- |
| `BMC_PUBLIC_URL` | Exact HTTPS origin for this VM, without a path/query/fragment. Optional: without it there is no browser login or UI |
| `BMC_OIDC_ISSUER` | `https://idm.yaple.net/oauth2/openid/qrab-bmc`; required with `BMC_PUBLIC_URL` |
| `BMC_OIDC_CLIENT_ID` | Public PKCE client `qrab-bmc`; no client secret; required with `BMC_PUBLIC_URL` |
| `BMC_BEARER_ISSUER` | HTTPS issuer of accepted bearer ID tokens, e.g. `https://idm.yaple.net/oauth2/openid/qrab-dashboard` |
| `BMC_BEARER_AUDIENCE` | Their audience, the dashboard's client ID `qrab-dashboard`; set with `BMC_BEARER_ISSUER` |
| `BMC_ALLOWED_SUBJECTS` | Comma-separated subject UUIDs with full (admin) access; kept for compatibility |
| `BMC_ADMIN_SUBJECTS` | Additional comma-separated admin subject UUIDs |
| `BMC_VIEWER_SUBJECTS` | Comma-separated read-only (viewer) subject UUIDs |
| `BMC_ADMIN_GROUPS` | Comma-separated Kanidm group UUIDs whose members have admin access |
| `BMC_VIEWER_GROUPS` | Comma-separated Kanidm group UUIDs whose members have viewer access |
| `BMC_ADMIN_BUCKET` | Comma-separated [admin-bucket](#admin-bucket) features enabled for this VM: `audio`, `usb-redirect`. Unset means none; an unknown name stops startup |
| `QRAB_NODE_NAME`, `QRAB_POD_NAME`, `QRAB_POD_NAMESPACE`, `QRAB_IMAGE` | Optional deployment metadata shown to admins |
| `BMC_REDFISH_URL` | Origin of the in-pod Redfish sidecar behind `/redfish`; defaults to `http://127.0.0.1:8081` |
| `BMC_LISTEN` | Listener; defaults to `0.0.0.0:8080` |
| `BMC_ASSETS_DIR` | Defaults to the source assets directory; image sets `/opt/qrab/bmc/assets` |
| `BMC_CA_FILE` | Optional PEM root certificate for an isolated test IdP |
| `RUST_LOG` | Default `bmc=info,qrab_bmc=info` |

`BMC_ALLOW_LOOPBACK_HTTP=true` permits HTTP only when **both** the public origin
and listener are loopback. OIDC issuer/endpoints still require verified HTTPS.
Never use that option in the Kubernetes pod.

At least one of `BMC_PUBLIC_URL` (browser console) and the
`BMC_BEARER_ISSUER`/`BMC_BEARER_AUDIENCE` pair (dashboard tokens) is required;
both may be set. The BMC discovers each configured issuer at startup.

Sessions are opaque server-side records, bounded to 2,048 per VM and lost on
restart. Each keeps the login's verified ID token (at most 16 KiB; a larger one
is refused at login) for the Redfish proxy. The token never appears in a
cookie or a response. Cookies are host-only, Secure, HttpOnly and SameSite=Lax under the
`__Host-qrab-session` name. A session expires at the earlier of the ID-token
expiry and 15 minutes after login. There is no refresh token in the browser.
Signing out revokes active consoles across tabs within one second; expiry also
closes them. Kanidm group changes affect new logins; existing sessions retain
their authorization until expiry or logout, at most 15 minutes. Changing the
VM's subject or group lists requires recreating its pod; changing a group's
members does not.

At least one admin or viewer subject or group is required. No subject, and no
group, may be in both the admin and the viewer set. With groups configured, the
login also requests Kanidm's `groups_uuid` scope. The session keeps only the
configured group UUIDs listed in the verified ID token. Roles are resolved from
this configuration and those groups on every request and on every live-stream
recheck; admin wins when both apply. The role itself is never stored in the
session.

| Role | Access |
| --- | --- |
| viewer | `/api/session` |
| admin | `/api/session` and `/api/console`, plus the VM's enabled [admin-bucket](#admin-bucket) features when requested |

Through `/redfish` a user's role is Redfish's, from the same Kanidm subject
and groups ([below](#redfish-proxy)).

### Routes

Redfish is the VM's single management plane. The BMC serves only the login
(`/auth/*`), the UI shell and its `/assets`, `GET /api/session`, the
`/api/console` WebSocket, the [`/redfish` proxy](#redfish-proxy) and the
`/healthz` and `/readyz` probes. Status, power, events, the guest agent and the
(role-redacted) configuration are Redfish's `Systems/1`,
`ComputerSystem.Reset`, `LogServices/Events`, `EventService/SSE`,
`Oem.Qrab.GuestAgent`, `Oem.Qrab.Configuration`,
`Managers/1/Oem/Qrab/Resources` and the `Oem/QrabComputerSystem` actions
([redfish.md](redfish.md)). The BMC page reads them through the proxy and the
dashboard calls each VM's Redfish on port 8081 directly
([dashboard.md](dashboard.md)). The former `/api/status`, `/api/action`,
`/api/events`, `/api/agent`, `/api/config`, `/api/events/stream`,
`/api/resources` and `POST /api/agent/{freeze,thaw,update}` routes are gone
(404). Add new management capabilities to Redfish, never to the BMC.

The console is admin-only: a server-side view-only RFB filter is not
implemented, so viewers are refused at WebSocket admission (403) and the UI
leaves the console and power controls out of the page.

`/api/session` and `/api/console` require a valid session or, in bearer mode,
a valid bearer token: missing, expired or unlisted sessions get 401, and an
authenticated viewer at console admission gets 403.
WebSocket upgrades require the exact configured Origin, and cookie-session
mutations (logout and proxied Redfish writes) also require the session's
`X-CSRF-Token`. There is no browser raw-QMP endpoint. Four simultaneous
consoles and a bounded manager queue limit resource use. Binary WebSocket
messages are limited to 64 KiB and blocked writes time out. 503 responses
name what is unavailable (console, Redfish, or too many consoles/streams) and
say so when the VM is shutting down.

## Redfish proxy

`/redfish` and everything below it is a reverse proxy to the in-pod Redfish
sidecar at `BMC_REDFISH_URL` (GET, HEAD, POST, PATCH and DELETE, including the
streaming `EventService/SSE`). Redfish verifies every token and decides the
role from the VM's lists; the BMC never grants a Redfish role itself. The
operator gives Redfish the console's subjects and groups
([operator.md](operator.md#bmc-users-in-redfish)), so a BMC admin or viewer
has the same role through the proxy.

- **Browser sessions.** A request with a valid session cookie is forwarded
  with `Authorization: Bearer <the login's ID token>`. Mutations (POST, PATCH,
  DELETE) need the exact Origin and the session's `X-CSRF-Token`, and a read
  with a foreign Origin is refused, before a single byte is sent upstream.
  The response is cut off at logout, expiry or role loss, rechecked at least
  once a second, so a proxied event stream ends within two seconds. Redfish
  refusing the token of a session the BMC accepted is 502, not 401, so the
  browser does not mistake a configuration fault for a sign-out.
- **Bearer requests.** A request with `Authorization: Bearer` is forwarded
  with exactly that token and never touches the cookie session; Redfish
  verifies it (its own, BMC and dashboard clients are trusted,
  [redfish.md](redfish.md#trusted-token-sources)). Redfish ends such streams
  at the token's expiry. No CSRF token or Origin is needed.
- **No credential.** The request is forwarded without one and Redfish answers
  401 with `WWW-Authenticate: Bearer`.
- Only `Authorization`, `Content-Type`, `Accept` and `Last-Event-ID` are
  forwarded, never a cookie, and only `Content-Type`, `OData-Version`,
  `Location`, `ETag`, `Allow`, `Retry-After`, `Link` and `WWW-Authenticate`
  come back. Bodies are bounded like Redfish's (4 KiB; 192 KiB for the signed
  Secure Boot database update) and refused with 413 before forwarding; paths
  with dot segments get 400. Sixteen event streams are proxied at once.
  Redfish unreachable is 503 (`The VM's Redfish service is unavailable.`) and
  a non-streaming exchange is bounded to 60 seconds (504).

Redfish is reached over the Pod's loopback, which network policy does not
filter; the BMC needs no other route to it. The embedded `--config` mode has no
Redfish sidecar, so its page shows the console and the session only.

QMP and VNC listen only on sockets under the supervisor's private session
directory. A sidecar BMC receives an established VNC stream through the manager's control
socket (`console` operation), and an embedded BMC receives it through the
in-process channel. The BMC still authenticates the WebSocket at admission and
rechecks the session every second, so logout, expiry or role loss closes the
tunnel. The manager trusts its same-UID socket peer. The sidecar mounts only the
control-socket volume, never the directory holding QMP and VNC sockets; see
[manager.md](manager.md#control-api). For a token-authenticated machine API, see
[redfish.md](redfish.md). It never accepts a socket path from HTTP. QEMU always starts
with `-nic none`; opening the BMC's pod port does not create guest networking.
The sample Cilium policy admits ingress/host probes on 8080 and permits only DNS
and Kanidm HTTPS outbound from the pod. It mounts no Kubernetes API token.

## Admin bucket

The admin bucket holds console features with a concrete data-exfiltration or
device-injection risk. During the proof-of-concept phase they are available
only as admin-only actions. A feature leaves the bucket only when it meets its
exit criteria below and the user records that decision as a note on
`container/bmc-experience-2026-11`.

| Feature | Name | Risk |
| --- | --- | --- |
| Guest audio playback (no microphone) | `audio` | Data exfiltration: whatever the guest plays (calls, media, screen readers, a covert audio channel) leaves the VM to the browser |
| Browser USB redirection into the guest | `usb-redirect` | Device injection into the guest (a malicious or keystroke-injecting device), and data exfiltration through a host-attached USB storage device |

The gate, `admin_bucket::admit(feature, principal, requested)`, admits a
feature on a console connection only when all of these hold:

- **Enabled for this VM.** The CR opts in per feature with
  `spec.console.adminBucket: {audio: true, usbRedirect: true}`; both default to
  false ([operator.md](operator.md#admin-bucket)). The operator renders the
  enabled names into `BMC_ADMIN_BUCKET` and the manager's `vm.json`
  `admin_bucket` flags only when at least one is enabled and the BMC is.
- **Admin role.** Viewers are refused at admission, not merely hidden in the UI.
- **Explicitly requested on this connection**, with
  `GET /api/console?features=audio,usb-redirect` (the parameter may repeat).
  Unknown names are 400. If any requested feature is refused, the whole
  admission is refused with 403; a console without `features` is admitted by
  the existing console rules.

An admitted feature is part of that connection's grant state
(`admin_bucket::Grants`). It is rechecked at least once a second like the
console: logout, expiry, or loss of the admin role (subject or group removal)
revokes the feature within two seconds, and closing the console revokes it too.
Revocation is final for the connection. A feature channel checks
`Grants::is_active(feature)` before it starts and ends when
`Grants::revoked(feature)` resolves. The console itself follows its own rules.
Every admission, denial and revocation is logged at info with the subject,
feature, outcome and, for denials, the reason. `GET /api/session` lists the
VM's enabled features as `admin_bucket` for admins only.

The dashboard forwards `features` on its console proxy only for admins and
only for features that VM enables; it drops the others and refuses unknown
names with 400.

### Audio playback

Audio uses QEMU's upstream VNC audio extension and travels in-band on the
existing console stream; there is no new socket or manager operation, and
QEMU is not patched. With `adminBucket.audio` (and the console) enabled, the
manager adds `-audiodev none,id=snd0 -device intel-hda,id=hda -device
hda-output,audiodev=snd0` and `,audiodev=snd0` on `-vnc`. The guest sees an
Intel HDA controller with an output-only codec; there is no microphone. A VM
without the flag keeps its exact command line, and QEMU then ignores the audio
encoding on every connection. Turning the flag on or off changes the QEMU
arguments, which take effect when the VM's Pod next starts.

The enforcement point is the BMC's client-to-server RFB filter
(`rfb_filter`), applied to every console connection. It parses the
ProtocolVersion (3.3, 3.7 or 3.8), the None security choice and ClientInit,
then every client message noVNC sends: SetPixelFormat (0), SetEncodings (2),
FramebufferUpdateRequest (3), KeyEvent (4), PointerEvent (5), ClientCutText
(6, including the extended clipboard's negative length), EnableContinuousUpdates
(150), ClientFence (248), xvp (250), SetDesktopSize (251), the QEMU extended key
event (255/0) and QEMU audio (255/1). Any other type, an unknown QEMU
sub-message or audio operation, more than 1024 encodings or a fence payload over
64 bytes closes the console. Unless the connection's grants admit `audio`, the
filter removes the audio pseudo-encoding (-259) from SetEncodings and drops
every 255/1 message, so QEMU never acknowledges audio or sends any on that
connection; other viewers of the same VM are unaffected because QEMU captures
audio per connection. When an admitted connection loses its audio grant, the BMC
closes the console: stopping audio already in flight would mean parsing the
server-to-client stream.

Both console pages show an **Audio** toggle only to admins of a VM that enables
audio (the BMC page reads `admin_bucket` from `/api/session`; the dashboard page
reads `spec.console.adminBucket`). It is off on every page load. Clicking it
creates the page's AudioContext (browsers allow playback only from a user
gesture) and, if the console is open, reconnects it with `?features=audio`;
otherwise the next connect asks for audio. Clicking it again closes the
AudioContext and reconnects without audio. The vendored noVNC carries
`contrib/novnc-audio.patch` (adapted from novnc/noVNC#1952, MPL-2.0) in both
identical copies: it advertises -259 only when constructed with an
`audioContext`, requests S16 stereo at 44.1 kHz on the acknowledgement, and
plays through an AudioWorklet (`core/audio.js`, served same-origin) with a
60-100 ms jitter buffer that drops frames on overflow. One listening console
costs about 176 KB/s (1.4 Mbit/s).

USB redirection is not built (decision note on
`container/bmc-experience-2026-11`); its flag still changes nothing.

Exit criteria, all required for a feature to leave the bucket:

- **audio**: a security review of the audio path; a server-side protocol filter
  that passes only playback (never capture) from the guest (the client-side
  `rfb_filter` gates negotiation; the server stream is not yet parsed); per-feature RBAC
  separate from console admin; rate and size limits on the audio stream; and
  the user's decision recorded on `container/bmc-experience-2026-11`.
- **usb-redirect**: a security review of the redirection path; a server-side
  protocol filter enforcing a device-class allowlist (for example, refusing
  HID and mass storage unless separately granted); per-feature RBAC separate
  from console admin; rate and size limits on transfers; and the user's
  decision recorded on `container/bmc-experience-2026-11`.

Known bucket concern: Prometheus in this cluster
(`kube-prometheus-stack-prometheus:9090`) has no query authentication. The
planned VM-pod egress to it for metric history graphs
(`work/bmc-metric-history-graphs`) would let a compromised BMC or Redfish
sidecar query every series in the cluster, not only its own VM's.

## Dashboard bearer tokens

The dashboard is the cluster's single browser OIDC client (Kanidm client
`qrab-dashboard`). It proxies a VM's console server-side from the BMC's
`/api/console` at the Pod IP on port 8080 and forwards the signed-in user's
ID token as `Authorization: Bearer <JWT>`; status, events and power go to the
VM's Redfish on port 8081 with the same token
([dashboard.md](dashboard.md)). The BMC verifies that token itself, so a
dashboard cannot forge identities. Bearer mode is on when both
`BMC_BEARER_ISSUER` and `BMC_BEARER_AUDIENCE` are set; the operator renders
them into every BMC sidecar when it has its own `QRAB_BMC_BEARER_*` settings
([operator.md](operator.md#dashboard-access-and-publicurl)).

In bearer mode, an `/api/session` request or `/api/console` WebSocket
admission with an `Authorization: Bearer` header is
authenticated only by that token. It never falls back to, mixes with or
creates a cookie session. The token is verified like Redfish's (the shared
`oidc-bearer` crate): ES256 or RS256 signature against the issuer's discovered
JWKS, issuer, audience (and `azp` when several audiences are present) and
expiry. Keys are refreshed every ten minutes, and at most every 30 seconds
after a signature fails. Tokens are limited to 16 KiB. The role then comes from
the subject and the configured groups in the token's `groups` claim (Kanidm
scope `groups_uuid`), exactly as for a browser session; admin wins.

- An empty, oversized, malformed, expired, wrong-issuer, wrong-audience or
  badly signed token gets 401. A valid token whose subject and groups grant
  nothing gets 403, and a bearer viewer at console admission gets 403.
- Bearer requests need no `X-CSRF-Token` and no matching Origin. Those checks
  protect ambient cookie credentials; a browser never attaches a bearer token
  by itself.
- A console admitted by a bearer token ends at the token's
  `exp`. A bearer token cannot be revoked at the BMC: its lifetime is the
  IdP's ID-token lifetime, so keep that short. Group changes take effect with
  the next token.
- Without a bearer header, cookie sessions behave exactly as described above.
  Other `Authorization` schemes are ignored, and without bearer mode, an
  `Authorization` header grants nothing.
- `GET /api/session` returns the bearer user with an empty `csrf`.

Without `BMC_PUBLIC_URL` the BMC is bearer-only: it registers no `/auth/*`
routes and serves no browser UI or `/assets` (404). It needs no Service,
Ingress, certificate or Kanidm callback, and only the dashboard reaches it.
Its network policy must admit the dashboard's Pods to port 8080 and permit
HTTPS to the token issuer for discovery and JWKS.

Kanidm must issue `qrab-dashboard` ID tokens with the `groups_uuid` scope for
group grants to work. Keep it a separate client from `qrab-bmc` and
`qrab-redfish`, so BMC browser tokens and Redfish tokens are never accepted as
dashboard tokens, or the reverse.

## Kanidm provisioning

Production client `qrab-bmc` and group `qrab_operators` were created for this
stage. Sam is in that group and the sample per-VM allowlist. For a **new**
installation, use an existing privileged operator session:

```sh
kanidm group create qrab_operators idm_admins -D idm_admin
kanidm group add-members qrab_operators sam -D idm_admin
kanidm system oauth2 create-public qrab-bmc 'qrab VM Console' https://qrab-demo.yapplesauce.com -D idm_admin
kanidm system oauth2 add-redirect-url qrab-bmc https://qrab-demo.yapplesauce.com/auth/callback -D idm_admin
kanidm system oauth2 enable-strict-redirect-url qrab-bmc -D idm_admin
kanidm system oauth2 update-scope-map qrab-bmc qrab_operators openid profile -D idm_admin
```

Keep PKCE enabled. For another VM, give it a unique Pod/ConfigMap/Service/Ingress
and certificate, an exact callback entry on the client, its own selector and
public URL, and an explicit subject allowlist. Host-only cookies and per-pod
session stores isolate browser sessions. The shared group permits login to the
client; the per-VM allowlist decides which VM an operator can access.

## Build, apply and verify

Use the image build/publication procedure in [manager.md](manager.md).
QEMU must include VNC and pixman; the image build enables them explicitly. No
additional registry identity is needed: the BMC ships in `qrab/manager`.

```sh
# Install the operator/CRD and preserve namespace/SA identity.
kubectl apply -k contrib/kubernetes/operator
kubectl wait --for=condition=Established crd/virtualmachines.qrab.yapplesauce.com
kubectl apply -k contrib/kubernetes
kubectl -n qrab wait --for=condition=Ready certificate/qrab-demo-tls --timeout=180s
kubectl -n qrab wait --for=condition=Ready pod/qrab-demo --timeout=120s
bash contrib/kubernetes/check.sh
```

The per-VM Service/Ingress and certificate are part of qrab's launch manifests.
The namespace and private-image-pull ServiceAccount stay in Kubernetes GitOps.
Pod readiness uses HTTP `/readyz`, which validates a live manager/QEMU heartbeat
and permits paused VMs so the operator can resume them. It is separate from the
standalone manager's `ready` command, which requires the guest to be running.
A failed QEMU child makes the pod process fail; clean guest exit/shutdown ends it.

`contrib/check-runtime.sh` includes BMC HTTP/WebSocket authentication tests and
strict Clippy. `contrib/tests/bmc-browser.py` performs real Kanidm login against
an isolated test client and the disposable guest, checks the framebuffer,
physical keyboard delivery, USB input events, absence of a guest NIC,
pause/resume and logout across tabs. It takes a private credentials JSON file,
a captured guest serial log, and a Chromium executable; it never saves cookies
or passwords. Screenshots go to ignored `target/bmc-browser`.

It observes pause/resume through Redfish `Systems/1` `PowerState` on the BMC's
`/redfish` proxy. For example, against an isolated client/guest already
running locally:

```sh
python3 contrib/tests/bmc-browser.py --origin http://localhost:28080 \
  --credentials /private/test-credentials.json --serial-log /tmp/bmc-serial.log \
  --browser /path/to/chromium
```

The real-VNC test checks expiry, server-side revocation and bounded shutdown
without browser polling or client cooperation. With extracted image artifacts:

```sh
QRAB_TEST_QEMU=/path/to/qemu/bin/qemu-system-x86_64 \
QRAB_TEST_QEMU_DATA=/path/to/qemu/share/qemu \
QRAB_TEST_GUEST=/path/to/demo \
  cargo test --locked -p bmc --test http real_vnc_streams -- --ignored
```

See [Redfish resource accounting](redfish.md#separate-cpu-and-memory-accounting)
for independent BMC/Redfish container budgets and per-container usage queries.

The Metrics tab's Service resources list shows Redfish's `Managers/1/Oem/Qrab/Resources`:
Redfish and manager process/cgroup usage, separately. The BMC's own container
usage is in the cluster's cAdvisor metrics. The guest-restart power action
(`GracefulRestart`) requires a connected, thawed guest; delivery does not
prove reboot completion.

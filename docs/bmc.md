# Per-VM BMC

`qrab-bmc` serves one VM's browser console. Operator Pods run it as an optional
sidecar, `qrab-bmc --socket /run/qrab-control/manager/control.sock`, next to the
`qrab-manager` container that owns the VM ([operator.md](operator.md#components)).
`qrab-bmc --config /etc/qrab/vm.json` still runs the VM in-process for
standalone use. The manager image contains `qrab-manager`, `qrab-bmc` and
`qrab-redfish`. The BMC uses the same Kanidm authorization-code OIDC flow as
`idm-dashboard`, with S256 PKCE, state/nonce verification, signature/issuer/audience
and expiry validation. The JSON API can also accept the dashboard's Kanidm ID
tokens as bearer credentials ([below](#dashboard-bearer-tokens)). There are no
static tokens and no anonymous development bypass.

The demo is at **https://qrab-demo.yapplesauce.com**, using the cluster's private
DNS and Cilium ingress, like `idm-dashboard`. It requires LAN/private-DNS access;
this manifest does not add a public edge route. Sign in with Kanidm, then choose
**Connect console**. Video, keyboard, USB tablet input, Ctrl Alt Del and full
screen run through the vendored noVNC client. There are no CDN dependencies.
Pause, resume, reset and bounded shutdown use a small typed manager control API.
The dashboard also shows the VM's live QMP event history and its effective
configuration.
Shutdown ends the current VM instance. With spec.running=true, the operator
starts another; set spec.running=false to keep it off.

The RAM-only fixture has an interactive root shell on its virtual terminal.
Keyboard and absolute pointer devices are exposed to the guest; the fixture's
terminal is not a desktop environment. A graphical guest can consume the same
USB tablet events. The fixture has no persistent root filesystem or NIC.
Clipboard sharing, sound and guest networking remain outside this stage.
The minimal [operator](operator.md) now creates the per-VM Pod from a CR.

## Runtime configuration

In sidecar mode the BMC needs only the control socket. It keeps a local mirror
of the manager's event history and opens a short socket connection for each
status, configuration, action or console request. The VM config, which must
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
| `QRAB_NODE_NAME`, `QRAB_POD_NAME`, `QRAB_POD_NAMESPACE`, `QRAB_IMAGE` | Optional deployment metadata shown to admins |
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
restart. Cookies are host-only, Secure, HttpOnly and SameSite=Lax under the
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
| viewer | `/api/session`, `/api/status`, `/api/events`, `/api/events/stream`, redacted `/api/config` |
| admin | Everything a viewer has, the full `/api/config`, `/api/action`, `/api/console` and `POST /api/agent/{freeze,thaw,update}` |

`GET /api/agent` (both roles) returns the manager's cached guest-agent status
and, while connected, the freeze state and last update run; viewers do not see
update arguments, output or errors. Admin `POST /api/agent/freeze`,
`/api/agent/thaw` and `/api/agent/update` (`{"args":[...]}`) need the same
Origin and CSRF checks as power actions. The dashboard's guest-agent panel
shows the status and offers the admin buttons.

A viewer's configuration keeps sizing, boot mode, TPM/console flags, disk kinds,
IDs, buses and read-only flags. It omits iSCSI portals, targets, LUNs,
initiator names, CHAP usernames and password-file references, file paths,
kernel arguments, the state ID, node, pod and image. Secret contents are never
read into any view. The console is admin-only: a server-side view-only RFB
filter is not implemented, so viewers are refused at WebSocket admission
(403) and the UI hides the console and power controls.

Every `/api/*` route requires a valid session or, in bearer mode, a valid
bearer token: missing, expired or unlisted sessions get 401, and an
authenticated viewer on an admin route gets 403.
WebSocket upgrades require the exact configured Origin. Mutations also
require the session's `X-CSRF-Token`. The control endpoint accepts only JSON
`"pause"`, `"resume"`, `"reset"` or `"powerdown"`; there is no browser raw-QMP
endpoint. Four simultaneous consoles and a bounded manager queue limit resource
use. Binary WebSocket messages are limited to 64 KiB and blocked writes time out.

`GET /api/events?after=<seq>&limit=<n>` returns a bounded page of the
manager's QMP event history (`events`, `next`, `latest`, `missed`,
`rejected`, `qmp_dropped`); without `after` it returns the newest events. Each event has a
sequence number, receive time, QEMU timestamp, QAPI name and payload.
`GET /api/events/stream` is a Server-Sent Events stream of `qmp` events (SSE
`id` is the sequence number, so a reconnecting browser resumes with
`Last-Event-ID`), `missed` events when the history evicted events first, and
`qmp_dropped` events when the manager's QMP client count of discarded events
changes; the events panel shows that warning. 503 responses name what is
unavailable (status, configuration, console, power action, or too many
consoles/streams) and say so when the VM is shutting down.
Its session is authorized at admission and rechecked at least once a second;
logout, expiry or removal from the subject or group lists end the stream. A request
carrying a foreign Origin is refused. Sixteen concurrent streams are allowed
per BMC; further streams get 503.

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

## Dashboard bearer tokens

The dashboard is the cluster's single browser OIDC client (Kanidm client
`qrab-dashboard`). It calls a VM's BMC JSON API server-side at the Pod IP on
port 8080 and forwards the signed-in user's ID token as
`Authorization: Bearer <JWT>`. The BMC verifies that token itself, so a
dashboard cannot forge identities. Bearer mode is on when both
`BMC_BEARER_ISSUER` and `BMC_BEARER_AUDIENCE` are set; the operator renders
them into every BMC sidecar when it has its own `QRAB_BMC_BEARER_*` settings
([operator.md](operator.md#dashboard-access-and-publicurl)).

In bearer mode, any `/api/*` request with an `Authorization: Bearer` header,
including `/api/console` WebSocket admission and `/api/events/stream`, is
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
  nothing gets 403, and a bearer viewer on an admin route gets 403.
- Bearer requests need no `X-CSRF-Token` and no matching Origin. Those checks
  protect ambient cookie credentials; a browser never attaches a bearer token
  by itself.
- A console or event stream admitted by a bearer token ends at the token's
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

For example, against an isolated client/guest already running locally:

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

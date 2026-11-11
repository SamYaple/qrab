# Per-VM BMC

`qrab-bmc` owns one QEMU VM through `manager::pod` and serves its browser console.
The manager image contains both `qrab-manager` and `qrab-bmc`; the demo pod selects
`qrab-bmc` as PID 1. The BMC uses the same Kanidm authorization-code OIDC flow as
`idm-dashboard`, with S256 PKCE, state/nonce verification, signature/issuer/audience
and expiry validation. No bearer-token console or anonymous development bypass
is provided.

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

Run `qrab-bmc --config /etc/qrab/vm.json`. The VM config is shared with the
standalone manager and must set `console: true`. See [manager.md](manager.md).

| Variable | Meaning |
| --- | --- |
| `BMC_PUBLIC_URL` | Exact HTTPS origin for this VM, without a path/query/fragment |
| `BMC_OIDC_ISSUER` | `https://idm.yaple.net/oauth2/openid/qrab-bmc` |
| `BMC_OIDC_CLIENT_ID` | Public PKCE client `qrab-bmc`; no client secret |
| `BMC_ALLOWED_SUBJECTS` | Comma-separated subject UUIDs with full (admin) access; kept for compatibility |
| `BMC_ADMIN_SUBJECTS` | Additional comma-separated admin subject UUIDs |
| `BMC_VIEWER_SUBJECTS` | Comma-separated read-only (viewer) subject UUIDs |
| `QRAB_NODE_NAME`, `QRAB_POD_NAME`, `QRAB_POD_NAMESPACE`, `QRAB_IMAGE` | Optional deployment metadata shown to admins |
| `BMC_LISTEN` | Listener; defaults to `0.0.0.0:8080` |
| `BMC_ASSETS_DIR` | Defaults to the source assets directory; image sets `/opt/qrab/bmc/assets` |
| `BMC_CA_FILE` | Optional PEM root certificate for an isolated test IdP |
| `RUST_LOG` | Default `bmc=info,qrab_bmc=info` |

`BMC_ALLOW_LOOPBACK_HTTP=true` permits HTTP only when **both** the public origin
and listener are loopback. OIDC issuer/endpoints still require verified HTTPS.
Never use that option in the Kubernetes pod.

Sessions are opaque server-side records, bounded to 2,048 per VM and lost on
restart. Cookies are host-only, Secure, HttpOnly and SameSite=Lax under the
`__Host-qrab-session` name. A session expires at the earlier of the ID-token
expiry and 15 minutes after login. There is no refresh token in the browser.
Signing out revokes active consoles across tabs within one second; expiry also
closes them. Kanidm group changes affect new logins; existing sessions retain
their authorization until expiry or logout. Changing the VM's allowlist requires
recreating its pod.

At least one admin or viewer subject is required, and a subject cannot be in
both sets. Roles are resolved from this configuration on every request and on
every live-stream recheck; they are never stored in the session.

| Role | Access |
| --- | --- |
| viewer | `/api/session`, `/api/status`, `/api/events`, `/api/events/stream`, redacted `/api/config` |
| admin | Everything a viewer has, the full `/api/config`, `/api/action` and `/api/console` |

A viewer's configuration keeps sizing, boot mode, TPM/console flags, disk kinds,
IDs, buses and read-only flags. It omits iSCSI portals, targets, LUNs,
initiator names, CHAP usernames and password-file references, file paths,
kernel arguments, the state ID, node, pod and image. Secret contents are never
read into any view. The console is admin-only: a server-side view-only RFB
filter is not implemented, so viewers are refused at WebSocket admission
(403) and the UI hides the console and power controls.

Every `/api/*` route requires a valid session: missing, expired or unlisted
sessions get 401, and an authenticated viewer on an admin route gets 403.
WebSocket upgrades require the exact configured Origin. Mutations also
require the session's `X-CSRF-Token`. The control endpoint accepts only JSON
`"pause"`, `"resume"`, `"reset"` or `"powerdown"`; there is no browser raw-QMP
endpoint. Four simultaneous consoles and a bounded manager queue limit resource
use. Binary WebSocket messages are limited to 64 KiB and blocked writes time out.

`GET /api/events?after=<seq>&limit=<n>` returns a bounded page of the
manager's QMP event history (`events`, `next`, `latest`, `missed`,
`rejected`); without `after` it returns the newest events. Each event has a
sequence number, receive time, QEMU timestamp, QAPI name and payload.
`GET /api/events/stream` is a Server-Sent Events stream of `qmp` events (SSE
`id` is the sequence number, so a reconnecting browser resumes with
`Last-Event-ID`) and `missed` events when the history evicted events first.
Its session is authorized at admission and rechecked at least once a second;
logout, expiry or removal from the subject lists end the stream. A request
carrying a foreign Origin is refused. Sixteen concurrent streams are allowed
per BMC; further streams get 503.

QMP and VNC listen only on sockets under the supervisor's private session
directory. The BMC receives an established VNC stream through the manager's
in-process channel. The same typed API is available to a sidecar over the
manager's optional control socket; see [manager.md](manager.md#control-api). It never accepts a socket path from HTTP. QEMU always starts
with `-nic none`; opening the BMC's pod port does not create guest networking.
The sample Cilium policy admits ingress/host probes on 8080 and permits only DNS
and Kanidm HTTPS outbound from the pod. It mounts no Kubernetes API token.

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

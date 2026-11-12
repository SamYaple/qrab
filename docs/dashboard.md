# VM dashboard

`qrab-dashboard` (`projects/dashboard`) is the web entry point for qrab VMs at
**https://vms.yapplesauce.com**. Signed-in users see the VMs shared with them,
with their size, disks and live usage, and open a VM's status, guest agent,
QMP events, configuration and console. Members of `qrab_vm_launchers` can
launch new VMs; VM admins can send power actions and delete VMs.

## Architecture

- **One OIDC client for the cluster.** The dashboard is the only browser OIDC
  client for VM access, with one callback,
  `https://vms.yapplesauce.com/auth/callback`. Per-VM console hosts,
  certificates and callbacks are no longer needed for new VMs: the dashboard
  creates VMs without `console.publicUrl`, so their BMCs are bearer-only.
- **BMC calls are server-side.** For a VM page the dashboard resolves the VM's
  Pod from `status.podName`, checks that the Pod carries the operator's labels
  for exactly that VM (`app.kubernetes.io/instance` and
  `qrab.yapplesauce.com/owner-uid`), and calls `http://<pod IP>:8080` with
  `Authorization: Bearer <the user's ID token>`. The BMC verifies that token
  itself (issuer `https://idm.yaple.net/oauth2/openid/qrab-dashboard`,
  audience `qrab-dashboard`, see [bmc.md](bmc.md#dashboard-bearer-tokens)) and
  applies its own roles, so the dashboard cannot widen access.
- **The ID token never reaches the browser.** It lives in the server-side
  session with the subject, `preferred_username`, the token's group UUIDs and
  the expiry. The browser holds only an opaque `__Host-` cookie.
- **Pages are Dioxus SSR with HTML forms.** The only client script is the
  console page: the vendored noVNC (`projects/dashboard/assets/novnc`, a copy
  of the BMC's) connects to `/vms/{namespace}/{name}/console/ws`, which the
  dashboard proxies to the BMC's `/api/console` WebSocket.

| Route | Purpose |
| --- | --- |
| `GET /` | VMs visible to the user: phase/ready, node, vCPUs and memory (current / hot-plug maximum), disks, live CPU and memory from `metrics.k8s.io`, role |
| `GET /vms/{ns}/{name}` | Spec and status, disks, `status.resize`, Pod requests/limits, live usage, network, and the BMC panel |
| `POST /vms/{ns}/{name}/action` | Admin power action (pause, resume, reset, shut down) |
| `POST /vms/{ns}/{name}/delete` | Admin deletion of the VirtualMachine; the PVC is kept |
| `GET /vms/{ns}/{name}/console` | noVNC page (admin) |
| `GET /vms/{ns}/{name}/console/ws` | Console WebSocket proxy (admin, exact Origin) |
| `GET`/`POST /launch` | Launch form (launchers) |
| `GET /auth/login`, `GET /auth/callback`, `POST /auth/logout` | OIDC |
| `GET /healthz`, `GET /readyz` | Probes |

### Roles

The dashboard mirrors the BMC for each VM:

- **admin**: the subject is in `spec.console.allowedSubjects` or
  `spec.components.redfish.adminSubjects`, or a token group is in
  `console.adminGroups` or `redfish.adminGroups`;
- **viewer**: the subject is in `console.viewerSubjects` or
  `redfish.viewerSubjects`, equals the `qrab.yapplesauce.com/creator-subject`
  annotation, or a token group is in `console.viewerGroups` or
  `redfish.viewerGroups`;
- otherwise the VM is invisible: it is not listed and its pages are 404, as for
  a VM that does not exist.

Admin wins when both apply. Kanidm group changes reach the dashboard and the
BMC at the next sign-in; sessions last at most 15 minutes.

### Launching a VM

The form takes a name (DNS label, at most 50 characters, and
`qrab_vm_<namespace>_<name>_viewers` at most 64), vCPUs and an optional
maximum, memory and an optional maximum, the accelerator (KVM or TCG),
Redfish on/off and up to four data disks (ID, GiB, optional base image name
and generation; they use `DASHBOARD_STORAGE_NODE`). On submit:

1. Preflight: the VirtualMachine, the PVC `<name>-state` and both Kanidm
   groups must not exist. An existing group is never adopted, because its
   members would gain access. A `<name>-state` PVC is reused only if the
   dashboard created it and it is still `Pending` (never bound, so it holds
   no TPM identity).
2. Kanidm, with the service-account token: create
   `qrab_vm_<ns>_<name>_admins` and `_viewers`, nest both in `qrab_vm_users`,
   and add the launcher's subject to `_admins`.
3. Create the PVC `<name>-state` (`zfs-block`, `ReadWriteOncePod`,
   `Filesystem`, 1Gi).
4. Create the VirtualMachine with `bootDisk: /opt/qrab/demo/boot.img`,
   `running: true`, `console: {adminGroups, viewerGroups}` (no `publicUrl`),
   Redfish with the same groups when selected, `storageNode` and `disks` when
   disks are requested, and the creator annotation set to the launcher.

If a step fails, the result page names the step, what was created, and what
was rolled back: groups created by that request are deleted again. The PVC
cannot be rolled back (the dashboard has no PVC delete permission); a retry with
the same name reuses it while it is unbound.

The launcher's current ID token predates the new `_admins` group. Until they
sign in again they are the VM's viewer as its creator; after that, its admin.

Deleting a VM (admin, type its name to confirm) deletes the VirtualMachine
with a UID precondition. Its Pod, ConfigMap and disk Secrets follow. The
state PVC stays: it holds the firmware variables and TPM identity, and an
administrator deletes it deliberately. The Kanidm groups stay too.

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
prompts and pipes Secret `qrab/qrab-dashboard` (keys `oidc-client-secret` and
`kanidm-token`) to `kubectl apply -f -`; it never prints them or puts them in
arguments or files.

## Deployment

`contrib/kubernetes/dashboard` holds the ServiceAccount, a namespaced Role
(VirtualMachines get/list/watch/create/delete, Pods get/list, PVCs get/create,
`metrics.k8s.io` pods get/list), the Deployment (one replica, `Recreate`,
non-root UID 10001, read-only root, no capabilities, credentials mounted as
files), the Service, the Ingress and Certificate for `vms.yapplesauce.com`,
and two CiliumNetworkPolicies. The first lets the dashboard receive from the
ingress and host, and reach the API server, DNS, Kanidm (192.168.0.251:443)
and VM Pods on 8080. The second selects every VM Pod and admits the dashboard
on 8080; Cilium policies are additive, so per-VM policies such as qrab-demo's
keep working and gain only the dashboard.

```sh
bash contrib/images/build.sh dashboard <tag>
bash contrib/images/publish.sh dashboard <tag>   # pin the printed digest in deployment.yaml
kubectl apply -k contrib/kubernetes/dashboard
kubectl -n qrab rollout status deployment/qrab-dashboard
```

The Deployment's image is a placeholder digest until the first publication.
Bearer access also needs the operator's `QRAB_BMC_BEARER_ISSUER` and
`QRAB_BMC_BEARER_AUDIENCE`, and the admission policy that trusts only
`system:serviceaccount:qrab:qrab-dashboard` to set the creator annotation
([operator.md](operator.md#the-creator-is-a-viewer)). The private image pull
identity for the ServiceAccount belongs to Kubernetes GitOps, like the
operator's.

## Security notes

- OIDC: authorization code with S256 PKCE, 32-byte state and nonce,
  client_secret_basic at the token endpoint, ID token signature, issuer,
  audience, nonce, expiry and access-token hash checks. Discovery and
  endpoints must be HTTPS; redirects are not followed.
- Sessions: a bounded in-memory store (4,096), at most 15 minutes and never
  past the token `exp`, rotated at sign-in, `__Host-qrab-dashboard` cookie
  (Secure, HttpOnly, SameSite=Lax). A restart signs everyone out.
- Every POST (logout, launch, power, delete) needs the session's CSRF token
  and the exact Origin before anything else happens. The console WebSocket
  needs a session, the exact Origin and the admin role, and closes within a
  second of sign-out or expiry; it rechecks the VM's grants every 30 seconds.
- BMC calls go only to the requested VM's own Pod IP. The dashboard and VM
  Pods talk plain HTTP inside the cluster network, restricted by the
  policies above; the bearer is an ID token for the `qrab-dashboard` audience
  only, which Redfish (audience `qrab-redfish`) does not accept.
- Guest- and VM-controlled strings (agent versions, errors, event payloads)
  are rendered as escaped text. Pages have a no-script CSP; the console page
  allows only same-origin scripts and WebSocket.
- The service-account token can create and delete groups and change members
  (`idm_group_admins`). The dashboard only creates `qrab_vm_<ns>_<name>_*`
  groups, nests them in `qrab_vm_users`, adds the launcher, and deletes
  groups it created in the same failed launch.

## Testing

`cargo test --locked -p qrab-dashboard` (part of `contrib/check-runtime.sh`)
runs offline against an in-memory cluster, a fake Kanidm and two fake BMCs on
127.0.0.1 and 127.0.0.2. It covers role resolution, list filtering, launch
authorization, the rendered names, PVC and VirtualMachine, rollback, CSRF and
Origin on every POST, BMC calls with the bearer only to the selected Pod,
escaping of guest strings, OIDC state handling and the console WebSocket's
refusals, proxying and revocation. A live Kanidm login and a browser console
session are part of the rollout check.

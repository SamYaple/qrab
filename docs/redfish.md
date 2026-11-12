# Redfish sidecar

`qrab-redfish` is an optional sidecar that serves a minimal DMTF Redfish API for
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
| `REDFISH_ADMIN_SUBJECTS` | Comma-separated subject UUIDs that may read and reset the VM |
| `REDFISH_VIEWER_SUBJECTS` | Comma-separated read-only subject UUIDs |
| `REDFISH_LISTEN` | Listener; defaults to `0.0.0.0:8081` |
| `REDFISH_CA_FILE` | Optional PEM root certificate for an isolated test IdP |
| `RUST_LOG` | Default `qrab_redfish=info` |

At least one subject is required and the two lists must be disjoint. The
listener is plain HTTP inside the Pod; expose it only through a TLS Ingress, as
for the BMC.

## Authentication

Every request, including `/redfish` and the service root, needs
`Authorization: Bearer <OIDC ID token>`. This deliberately departs from the
DMTF convention that the service root is anonymous: no VM data or version
information is served without a token. Like Kubernetes API-server OIDC
authentication, the token is an ID token (a signed JWT) issued to the
`REDFISH_OIDC_AUDIENCE` client. Each request verifies the signature against
the issuer's discovered JWKS (ES256 or RS256 only), the issuer, the audience
(and `azp` when several audiences are present) and expiry. The subject must be
listed. Keys are refreshed every ten minutes, and at most every 30 seconds
after a signature fails, so rotated keys are picked up.

There are no Redfish sessions, static tokens, Basic authentication, stored
passwords or anonymous routes other than `/healthz` and `/readyz`. Missing,
malformed, expired, wrong-issuer, wrong-audience or badly signed tokens get 401
with `WWW-Authenticate: Bearer`; a valid token for an unlisted subject gets 403.
Bearer tokens cannot be revoked here: their lifetime is the IdP's ID-token
lifetime, and event streams close when the token expires.

### Kanidm provisioning

Use a separate public client so BMC browser tokens (audience `qrab-bmc`) are
never accepted by Redfish:

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
| `/redfish/v1/Managers`, `/Managers/1` | `ManagerCollection`, `Manager.v1_19_0` | `ManagerType: BMC` |
| `/redfish/v1/Managers/1/LogServices/Events` | `LogService.v1_7_0` | 256 records, `WrapsWhenFull`; `Oem.Qrab.QmpDropped` counts events QMP discarded, with `Health: Warning` when nonzero |
| `.../Events/Entries`, `.../Entries/{seq}` | `LogEntryCollection`, `LogEntry.v1_17_0` | one `Oem` entry per QMP event; QAPI name and data under `Oem.Qrab` |
| `/redfish/v1/EventService` | `EventService.v1_10_0` | `ServerSentEventUri` only; push subscriptions are not offered |
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

`ForceOff`, `GracefulRestart`, `ForceOn`, `Nmi`, `PushPowerButton`,
`PowerCycle`, `Suspend` and `FullPowerCycle` get 400
`ActionParameterNotSupported`; other values get `ActionParameterValueNotInList`.
A successful reset returns 204.

Errors use the Redfish error format with `Base.1.18` message IDs
(`NoValidSession`, `InsufficientPrivilege`, `ResourceNotFound`,
`OperationNotAllowed`, `MalformedJSON`, `ActionParameterMissing`,
`ServiceTemporarilyUnavailable`, ...). All responses carry `OData-Version: 4.0`
and `Cache-Control: no-store`. Request bodies are limited to 4 KiB and eight
event streams are served at once.

## Tests

`cargo test --locked -p qrab-redfish` covers resource shapes, the authentication
rejection matrix (missing, Basic, malformed, expired, wrong audience, wrong
issuer, foreign signing key, unlisted subject, viewer reset), reset mapping,
power-state mapping, the event stream and its token-expiry bound. The tests sign
tokens with the test-only keys under `projects/redfish/tests/fixtures`.

# BMC

The native `qrab-bmc` binary embeds `manager::pod` to own one VM per pod. Its
Axum server authenticates browser requests with Kanidm OIDC and bridges the
private VNC Unix socket to noVNC. There is no host launcher or WASM build.

Keep QMP and VNC off TCP, keep the guest NIC disabled, and use the manager's
bounded control channel. Never add a static token or anonymous console fallback.
The only bearer credentials are OIDC ID tokens from the configured dashboard
issuer, verified per request by `oidc-bearer` and authorized by the same
subject/group lists; they never mix with cookie sessions. Without
`BMC_PUBLIC_URL` the BMC is bearer-only and serves no login or UI.

Redfish is the VM's management plane. The BMC UI uses it through the
`/bmc/redfish` browser-session adapter (`src/proxy.rs`): a session's request carries the login's
server-side ID token after the Origin/CSRF checks, a bearer request carries
its own token, and Redfish verifies and authorizes both. Public `/redfish` routes directly to the
Redfish sidecar through Kubernetes Ingress. Never add BMC
`/api/*` management routes; the remaining ones serve the dashboard until
work/dashboard-uses-redfish.
Only explicit loopback HTTP development may use non-Secure cookies. Tests must
cover authentication at HTTP and WebSocket admission and throughout a session
or token lifetime.

See `docs/bmc.md` for configuration, OIDC provisioning and Kubernetes delivery.
Run `cargo test --locked -p bmc` and `contrib/check-runtime.sh`.

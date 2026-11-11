# BMC

The native `qrab-bmc` binary embeds `manager::pod` to own one VM per pod. Its
Axum server authenticates browser requests with Kanidm OIDC and bridges the
private VNC Unix socket to noVNC. There is no host launcher or WASM build.

Keep QMP and VNC off TCP, keep the guest NIC disabled, and use the manager's
bounded control channel. Never add a bearer token or anonymous console fallback.
Only explicit loopback HTTP development may use non-Secure cookies. Tests must
cover authentication at HTTP and WebSocket admission and throughout a session.

See `docs/bmc.md` for configuration, OIDC provisioning and Kubernetes delivery.
Run `cargo test --locked -p bmc` and `contrib/check-runtime.sh`.

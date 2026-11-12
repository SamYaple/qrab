# VM dashboard

`qrab-dashboard` is the cluster's single browser OIDC client for VM access. It
lists, launches, inspects and deletes VirtualMachines, and proxies each VM's
BMC (JSON and console) server-side with the signed-in user's ID token as a
bearer. Pages are Dioxus SSR with HTML forms; the only client script is the
noVNC console page.

Keep the ID token server-side, CSRF + exact Origin on every POST, roles equal
to the BMC's console roles (admin wins; invisible VMs are 404; Redfish lists
grant nothing), delete only for dashboard-launched VMs, and BMC calls only to
the selected VM's own controlled Pod. Never adopt existing Kanidm groups or PVCs that may
hold another VM's identity. See docs/dashboard.md.

Run `cargo test --locked -p qrab-dashboard` and `contrib/check-runtime.sh`.

# VM dashboard

`qrab-dashboard` is the cluster's single browser OIDC client for VM access. It
lists, launches, inspects and deletes VirtualMachines. It reads and powers
each VM through the VM's Redfish service (port 8081) and proxies its BMC
console (port 8080), server-side with the signed-in user's ID token as a
bearer; never add calls to BMC `/api/*` management routes. Pages are Dioxus SSR with HTML forms, except the console and full BMC
interface. The BMC interface embeds the shared assets from `projects/bmc/assets`
and uses the VM-scoped dashboard Redfish bridge.

Keep the ID token server-side, CSRF + exact Origin on every POST, roles equal
to the BMC's console roles (admin wins; invisible VMs are 404; Redfish lists
grant nothing), delete only for dashboard-launched VMs, and Redfish and BMC
calls only to the selected VM's own controlled Pod. Never adopt existing Kanidm groups or PVCs that may
hold another VM's identity. See docs/dashboard.md.

Run `cargo test --locked -p qrab-dashboard` and `contrib/check-runtime.sh`.

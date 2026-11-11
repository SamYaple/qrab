# manager

The pod-native path is `src/pod/` and `src/bin/qrab-manager.rs`. It supervises
QEMU directly inside the container sandbox. See [../../docs/manager.md](../../docs/manager.md)
for config, images, probes, shutdown and Kubernetes launch.

The default feature set is pod-only. The older typestate/namespace/confidential-guest
API in `src/vm.rs` requires `legacy-host`. BMC uses the pod control channel.
Keep it separate from the pod path; basic pod launch must not require qrab-init,
SEV/SVSM, host namespaces or Kubernetes API access.

Run `bash contrib/check-runtime.sh` for the default core or `cargo test -p manager`.
Preserve host API/unit coverage with `cargo test -p manager --features legacy-host --lib`. Image definitions and the
RAM-only guest fixture are under `contrib/images`; pod manifests are under
`contrib/kubernetes`. Image source must stay pinned to our local Gitea QEMU.

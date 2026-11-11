# manager

The pod-native path is `src/pod.rs` and `src/bin/qrab-manager.rs`. It supervises
QEMU directly inside the container sandbox. See [../../docs/manager.md](../../docs/manager.md)
for config, images, probes, shutdown and Kubernetes launch.

The older typestate/namespace/confidential-guest API remains in `src/vm.rs`.
Keep it separate from the pod path; basic pod launch must not require qrab-init,
SEV/SVSM, host namespaces or Kubernetes API access.

Run `cargo test -p manager --lib --test pod_runtime`. Image definitions and the
RAM-only guest fixture are under `contrib/images`; pod manifests are under
`contrib/kubernetes`. Image source must stay pinned to our local Gitea QEMU.

# manager

The pod-native path is `src/pod/` and `src/bin/qrab-manager.rs`. It supervises
QEMU directly inside the container sandbox. See [../../docs/manager.md](../../docs/manager.md)
for config, images, probes, shutdown and Kubernetes launch.

The manager supports the pod runtime. The retired host namespace/SEV/SVSM API
is archived under `eol/confidential-computing-svsm-2026-11-15`.

Run `bash contrib/check-runtime.sh` for the default core or `cargo test -p manager`.
Image definitions and the
RAM-only guest fixture are under `contrib/images`; pod manifests are under
`contrib/kubernetes`. Image source must stay pinned to our local Gitea QEMU.

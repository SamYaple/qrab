# Live migration

A running VM moves to another node without a reboot. The guest's memory,
devices, TPM state and UEFI variables travel in QEMU's migration stream over a
SPIFFE mTLS tunnel between the two managers of the same VM. Disks are shared
iSCSI LUNs, so no disk data is copied, and NIC pods stay where they are, so the
guest keeps its NIC addresses. Design and work items: towl
`container/vm-live-migration`.

## Identity and isolation

- Every VM pod's manager holds `spiffe://yapplesauce.com/kubernetes/qrab/vm/<vm-uid>`.
  The source and target pods of one VM therefore hold the same ID, and each
  side of the tunnel accepts exactly that ID (`projects/qrab-spiffe`, TLS 1.3,
  local trust domain only). Another VM's pod, or any other workload, fails the
  handshake.
- The source listens on TCP 7800 (`migration.port`). The operator opens that
  port with a CiliumNetworkPolicy from the target pod to the source pod only for
  the migration's lifetime; otherwise it is default-denied.
- The target's hello names the VM (`state_id`) and the migration UID; the source
  refuses another VM, a protocol mismatch, local disks and hot-plugged growth.

## Manager protocol

`projects/manager/src/pod/migration.rs`.

1. The target manager (config `incoming: {migration, source}`) dials the
   source, sends `Hello`, and receives an `Offer`: the source's exact machine
   type (e.g. `pc-q35-11.1`) and every iSCSI binding it has recorded.
2. The target prepares **empty** state (it refuses a retained identity, which
   would be a second, divergent copy), starts swtpm with a fresh directory and
   QEMU with the offered machine type and `-incoming defer`, calls
   `migrate-incoming unix:<session>/migrate-in.sock`, connects to that socket
   and sends `{"ready":true}`.
3. The source sets `return-path` and `events`, caps bandwidth at 1 GiB/s and
   runs `migrate unix:<session>/migrate-out.sock`. Each manager splices its
   QEMU's Unix socket with the TLS stream, so QEMU never sees a TCP socket.
4. Both managers poll `query-migrate` every 250 ms and keep publishing status,
   so probes stay green. On `completed` the target runs `cont`; the source
   QEMU (paused, disks inactive) is quit, and its manager exits 0 with outcome
   `{"reason":"migrated","shutdown_method":"migration"}`. On failure the source
   keeps running the guest, records `migration failed: ...` in its status
   detail, and accepts the next attempt.

TPM state rides the stream as swtpm state blobs; the target's swtpm writes them
to its own state directory. UEFI variables ride as pflash RAM and QEMU's
`pflash_post_load` writes them back to the target's `OVMF_VARS.fd`.

Limits: one migration at a time per VM; local `disk` images and hot-plugged
vCPUs/memory are refused (the target would need a different device layout);
SIGTERM during an outgoing migration cancels it and stops the VM normally.

## Testing

Unit tests in `migration.rs` cover the hello/offer frames, refusals and a
different VM's SVID being refused, with fixture SVIDs from
`qrab-spiffe`'s `fixture` feature.

`projects/manager/tests/pod_migration.rs` migrates a real UEFI + swtpm guest
between two managers (TCG), checks the guest agent answers on the target
without a reboot, then cold-boots the target's state and compares the TPM
endorsement key with the source's. It needs QEMU 11.1, swtpm, OVMF and the demo
guest, so run it inside the manager image, building with the image's Rust
toolchain:

```sh
T=target/migration-test
podman run --rm -v "$PWD":/src:ro -v "$PWD/$T":/target \
  -v "$HOME/.cargo/registry":/usr/local/cargo/registry:ro \
  -e CARGO_TARGET_DIR=/target -e CARGO_NET_OFFLINE=true -w /src \
  "$(sed -n 's/^ARG RUST_IMAGE=//p' contrib/images/Containerfile.manager)" \
  cargo test --release --locked -p manager --test pod_migration --no-run
podman run --rm -v "$PWD/$T":/target:ro \
  --entrypoint "$(ls $T/release/deps/pod_migration-* | grep -v '\.d$' | head -1 | sed "s|^$T|/target|")" \
  zot.yapplesauce.com/qrab/manager:<tag> --ignored --nocapture
```

First run, 2026-11-14 on manager `fe93197`: migration completed in under a
second for a 512 MiB TCG guest; the guest agent answered on the target, and the
cold-booted target reported the source's endorsement key.

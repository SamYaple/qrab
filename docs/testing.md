# Core-library validation

The normal developer path is the default workspace (`default-members` in the
root `Cargo.toml`): the QAPI and protocol libraries, the pod-native `manager`,
`bmc`, `qrab-redfish`, `oidc-bearer`, `qrab-operator`, `qrab-dashboard`,
`qrab-iscsi-target`, `qrab-guest-agent`, `qrab-kvm-device-plugin` and
`qrab-spiffe`. No QEMU checkout, Python, firmware, privileged hardware or
cluster is required for these tests:

```sh
cargo test --locked
bash contrib/check-runtime.sh
```

The script also checks formatting, runs strict Clippy on those crates (the
generated `qapi`/`qapi-macros` aside), and verifies that the committed VM and
migration CRDs and the dashboard update policy match `qrab-operator`'s output.
Provisioned-disk coverage is hermetic: `qrab-iscsi-target` drives its disk,
image, export/rotation, fence, budget and argv/stdin contracts against an
in-memory host, and the operator converges `spec.disks`, auto-fences and
fences on deletion against a fake storage API and fake Kubernetes server.
Neither proves LIO session drain, ZFS clone behaviour or the SPIFFE/Kanidm
exchange; the live `contrib/kubernetes/operator-storage-api/check.sh` and
`clone-check.sh` do ([contrib/kubernetes/README.md](../contrib/kubernetes/README.md#checks)).
Generated QAPI modules are excluded from rustfmt traversal; update them through
codegen, not manual formatting.

Parser/generator changes have a separate, explicit check:

```sh
bash contrib/check-qapi.sh
```

It requires Git, Python 3 and the initialized local QEMU submodule with pinned
release tags. It compares definition coverage with QEMU's own parser, tests the
generator, and regenerates QMP/QGA to verify committed outputs byte for byte.
Historical schemas remain tooling inputs; their compatibility work is not a
mandatory runtime development step.

Retired host and confidential-guest experiments are archived; see [legacy.md](legacy.md).
BMC authentication and browser acceptance are described in [bmc.md](bmc.md).

Protocol cleanup can be checked directly with:

```sh
cargo test --locked -p qapi-protocol
cargo clippy --locked -p qapi-protocol --all-targets --no-deps -- -D warnings
```

The timeout fault tests use Tokio's test clock or short explicit
deadlines; no 30-second stall test is required.

`contrib/check-qapi.sh` also compiles `qapi` with `QEMU_MIN_VERSION` and
`QEMU_MAX_VERSION` bounds for every maintained release series, selecting its
latest pinned patch. The final check restores the full development API surface.

The default suite intentionally leaves the real-QEMU smoke tests ignored. Run
it explicitly against an installed binary, or a locally built current release:

```sh
QRAB_TEST_QEMU=/path/to/qemu-system-x86_64 \
  cargo test --locked -p qapi-protocol --test live_qemu -- --ignored --nocapture
```

It uses `-machine none -accel tcg` and a temporary QMP socket: no KVM, guest
image, network download, root privileges, systemd or cluster access. It checks
the greeting, typed query-version/query-status, query-commands against generated
metadata, typed query-qmp-schema decoding, concurrent and OOB commands, error
recovery, and clean quit. Both compact and pretty monitor output are tested
through `-mon`. If the binary advertises QEMU 11.1's `monitor-qmp` object,
the same checks also run through that frontend. This keeps the smoke test
usable across the recent release matrix.

A minimal current-QEMU build for this test is:

```sh
mkdir -p target/qemu-11.1.3
cd target/qemu-11.1.3
../../vendor/qemu/configure --target-list=x86_64-softmmu \
  --without-default-features --without-default-devices --enable-tcg \
  --disable-docs --disable-werror
ninja -j 8 qemu-system-x86_64
```

QEMU's build requires its own C/Python dependencies and may fetch its build
subprojects. Pod launcher and UEFI/TPM boot tests have separate requirements and remain
outside this core-library check.

The real guest-agent test uses a private Unix listener, temporary files and an
explicit RPC allowlist. It checks sync, guest-info, ping, a typed 2 MiB file read
and typed guest-exec/status. Build and run it with:

```sh
# Include --enable-guest-agent in the QEMU configure command above.
ninja -C target/qemu-11.1.3 -j 8 qga/qemu-ga
QRAB_TEST_QGA="$PWD/target/qemu-11.1.3/qga/qemu-ga" \
  cargo test --locked -p qapi-protocol --test live_qga -- --ignored --nocapture
```

It runs as the current user and reads no host agent configuration. No guest VM,
root privileges, systemd or cluster is required for either protocol smoke test.

The guest agent's protocol tests need none of these.

The manager's real-guest tests are ignored by default and need the built guest
assets (`contrib/images`):

| Test | Checks | Inputs |
| --- | --- | --- |
| `pod_runtime` | CLI boot, probes and SIGTERM shutdown | `QRAB_TEST_QEMU`, `QRAB_TEST_QEMU_DATA`, `QRAB_TEST_GUEST` |
| `pod_uefi` | UEFI/TPM boot and retained identity ([operator.md](operator.md#build-and-verification)) | the above plus `QRAB_TEST_FIRMWARE`, optional `QRAB_TEST_SWTPM` |
| `pod_guest_agent` | ping, freeze/thaw and an update ([guest-agent.md](guest-agent.md#tests)) | `QRAB_TEST_QEMU`, `QRAB_TEST_FIRMWARE_DIR`, `QRAB_TEST_KERNEL`, `QRAB_TEST_INITRD` |
| `pod_migration` | live migration between two managers ([migration.md](migration.md#testing)) | run inside the manager image |

Run one with `cargo test --locked -p manager --test <name> -- --ignored --nocapture`.

For an explicit scan of every locally mirrored schema-era stable release up to
the pinned baseline, independent of the normal locked input selection:

```sh
cargo test --locked --release -p qapi-codegen --test mirrored_releases \
  -- --ignored --nocapture
```

It lowers QMP inputs from 1.0 and QGA inputs from 0.15, including historical
four-component release tags. The normal check stays deterministic when extra
local tags are present.

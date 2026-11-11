# Core-library validation

Rust/Cargo, Git and a C toolchain build the core libraries. Parser integration
tests also require the initialized QEMU submodule, its pinned release tags and
Python 3 for QEMU's independent parser oracle. They do not need QEMU binaries.

From the repository root:

```sh
contrib/check-qapi.sh
```

This runs parser/generator, macro, type and protocol tests, then generates QMP
and QGA into a temporary directory and compares every file with the committed
outputs. Ordinary builds do not regenerate schemas. For just the transport and
type tests (no submodules or Python required):

```sh
cargo test --locked -p qapi-macros -p qapi -p qapi-protocol
```

Protocol cleanup can be checked directly with:

```sh
cargo test --locked -p qapi-protocol
cargo clippy --locked -p qapi-protocol --all-targets --no-deps -- -D warnings
```

The timeout fault tests use Tokio's test clock or short explicit
deadlines; no 30-second stall test is required.

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
using QEMU 11.1's monitor-qmp object.

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
subprojects. Launcher, confidential guest and boot tests have separate
requirements and remain outside this core-library check.

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

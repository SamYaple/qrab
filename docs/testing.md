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

The default suite intentionally leaves the real-QEMU smoke test ignored. Run
it explicitly against an installed binary, or a locally built current release:

```sh
QRAB_TEST_QEMU=/path/to/qemu-system-x86_64 \
  cargo test --locked -p qapi-protocol --test live_qemu -- --ignored --nocapture
```

It uses `-machine none -accel tcg` and a temporary QMP socket: no KVM, guest
image, network download, root privileges, systemd or cluster access. It checks
the greeting, typed query-version/query-status, query-commands against generated
metadata, typed query-qmp-schema decoding, and clean quit.

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

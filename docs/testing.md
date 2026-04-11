# Testing

## Running Tests

```sh
# All tests
cargo test --workspace

# Specific crate
cargo test -p qapi
cargo test -p qapi-protocol
cargo test -p manager
```

## Test Locations

| Location | What It Tests |
|---|---|
| `projects/qapi/tests/serialization.rs` | Serde round-trips for generated types (unit enums, renamed variants, flat unions, alternates) |
| `projects/qapi-protocol/tests/mock_server.rs` | QMP/QGA clients against mock servers |
| `projects/qapi-protocol/tests/minimal.rs` | Basic QMP session against real QEMU |
| `projects/qapi-protocol/tests/boot.rs` | VM boot sequences |
| `projects/qapi-protocol/tests/block.rs` | Block device operations |
| `projects/qapi-protocol/tests/events.rs` | Async event handling |
| `projects/qapi-protocol/tests/schema.rs` | Schema-level validation |
| `projects/qapi-codegen/tests/integration.rs` | Codegen integration tests |

## Integration Tests Requiring QEMU

Tests in `qapi-protocol` that spawn real QEMU processes will **auto-skip** if
the QEMU binary is not available. No special setup is needed — missing binaries
result in skipped tests, not failures.

## Multi-Version Container Testing

`tests/containers/` provides infrastructure for testing against multiple QEMU
versions without installing them on the host.

| File | Purpose |
|---|---|
| `Dockerfile.debian` | Builds minimal containers with `qemu-system-x86` from Debian repos |
| `build.sh` | Builds container images: `qrab-qemu-3.1`, `qrab-qemu-5.2`, `qrab-qemu-7.2` |
| `qemu-wrapper.sh` | Drop-in `qemu-system-x86_64` replacement that runs inside a container. Auto-mounts file paths from QEMU args as volumes |
| `run-tests.sh` | Runs `cargo test -p qapi-protocol --test minimal` against each container version. Reports PASS/FAIL/SKIP per version |

### Usage

```sh
# Build all container images (requires podman)
tests/containers/build.sh

# Run tests against all versions
tests/containers/run-tests.sh

# Build/test specific versions
tests/containers/build.sh 3.1 5.2
```

Set `QEMU_CONTAINER_IMAGE` and `QEMU_TEST_BIN_X86_64` to use the container
wrapper in custom test runs.

## Serialization Tests

`projects/qapi/tests/serialization.rs` uses `set_qemu_version(99, 0, 0)` to
unlock all version-gated fields, then tests:

- Unit enum round-trips
- Renamed variant serialization
- Leading-digit variant handling
- Alternate types (`StrOrNull`)
- Flat union encoding
- Struct field serialization

## Coverage Status

Test coverage needs significant improvement in two areas:

1. **Functional tests** — more end-to-end scenarios covering command execution,
   error handling, event processing, and version-gating behavior.
2. **Parser coverage** — the nom parser handles schemas from v1.0 to latest,
   but edge cases and format variations across versions need more targeted
   tests.

# qapi-protocol

Async clients and mock servers for QEMU's [QMP](https://www.qemu.org/docs/master/interop/qmp-intro.html) and [QGA](https://www.qemu.org/docs/master/interop/qemu-ga.html) protocols.

## Clients

- **`Qmp`** — connects to a QMP unix socket, negotiates capabilities, provides typed `execute()`
- **`Qga`** — connects to a QGA socket, performs `guest-sync-delimited` handshake, provides typed `execute()`

Both use the `QapiCommand` trait from the `qapi` crate for type-safe command execution and response deserialization.

## Mock servers

- **`QmpSession`** — simulates a QMP endpoint (greeting, capability negotiation, command dispatch)
- **`QgaSession`** — simulates a QGA endpoint (sync handshake, command dispatch)

Mock servers are first-class library types, not test utilities.

## Tests

| File | Scope |
|------|-------|
| `minimal.rs` | Query commands with `-machine none`, parameterized across architectures |
| `events.rs` | Event capture: STOP, RESUME, RESET, SHUTDOWN, POWERDOWN |
| `boot.rs` | Cirros guest boot with serial console verification |
| `block.rs` | Block device lifecycle (blockdev-add, query, deletion) |
| `schema.rs` | Trait metadata, enum serialization, QMP schema introspection |
| `mock_server.rs` | Round-trip tests against mock QMP and QGA servers |

Tests that require unavailable QEMU binaries or firmware skip automatically. Boot tests download a cirros image on first run and cache it in `target/test-fixtures/`.

# qapi-protocol

Async QMP and QGA clients built on tokio, plus mock servers for testing.

## Files

| File | Lines | Content |
|---|---|---|
| `src/lib.rs` | ~249 | `Qmp` client |
| `src/qga.rs` | ~142 | `Qga` client |
| `src/server.rs` | ~216 | Mock `QmpSession` and `QgaSession` |

## Qmp Client

```rust
let (qmp, events) = Qmp::new(socket_path).await?;
let status: StatusInfo = qmp.execute(&QueryStatus {}).await?;
```

### Connection lifecycle

1. Connect to UNIX socket
2. Read QMP greeting — extracts QEMU version, calls `qapi::set_qemu_version()`
3. Send `qmp_capabilities` to complete negotiation
4. Spawn 3 background tokio tasks:
   - **read_loop** — reads JSON lines, routes responses vs events
   - **write_loop** — sends commands, pairs each with a `oneshot` reply channel
   - **event_worker** — deserializes raw event JSON into `QmpEvent`

### Key methods

| Method | Description |
|---|---|
| `Qmp::new(path)` | Connect, negotiate, return `(Qmp, Receiver<QmpEvent>)` |
| `qmp.execute(&cmd)` | Send a typed `QapiCommand`, await typed response |
| `qmp.shutdown()` | Graceful teardown of background tasks |

### Types

- `QmpEvent { event: String, timestamp: Timestamp, data: Option<Value> }`
- `Timestamp { seconds: u64, microseconds: u64 }`

## Qga Client

Simpler sequential model (no background tasks — QGA has no async events).

```rust
let mut qga = Qga::new(socket_path).await?;
let info: GuestAgentInfo = qga.execute(&GuestInfo {}).await?;
```

### Connection lifecycle

1. Connect to UNIX socket
2. Perform `guest-sync-delimited` handshake (0xFF sentinel byte)
3. Query `guest-info` to get QGA version, calls `qapi::set_qga_version()`

## Mock Servers

For testing without a real QEMU instance.

### QmpSession

| Method | Description |
|---|---|
| `QmpSession::accept(listener)` | Accept connection |
| `send_greeting(major, minor, micro)` | Emit QMP greeting |
| `negotiate()` | Handle `qmp_capabilities` exchange |
| `recv_command()` | Read next command as `(name, args)` |
| `send_return(value)` | Send success response |
| `send_error(class, desc)` | Send error response |
| `send_event(name, data)` | Push async event |

### QgaSession

Same pattern: `accept()`, `handle_sync()`, `recv_command()`, `send_return()`,
`send_error()`.

## Dependencies

`tokio` (net, io-util, sync, rt, macros), `serde`, `serde_json`, `anyhow`,
`qapi` (path dependency).

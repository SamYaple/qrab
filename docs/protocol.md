# Protocol clients

`qapi-protocol` provides Tokio Unix-socket clients without a dependency on the
VM manager, systemd, Kubernetes or confidential-computing hardware. Its test
servers support protocol and framing tests without a QEMU process.

`Qmp::new(path)` reads the greeting and negotiates capabilities, including OOB
when advertised. It returns a client and an event receiver. Typed requests use
`qapi::commands` and `QapiCommand::Returns`; raw requests remain available for
out-of-tree or newer commands. Responses are correlated by ID, and the normal
command queue has eight permits. Explicit shutdown ends the worker tasks.

`Qga::new(path)` performs guest-sync-delimited, then queries guest-info. Typed
requests use `qapi::qga::commands`. The guest-agent version can differ from the
host's QEMU version. QGA requests are sequential through a mutable client.

Each client exposes `version()` and uses its own peer's version during request
serialization. Different QEMU and QGA versions can coexist in one process.
`qapi::with_version(version, || serde_json::to_value(value))` provides the same
synchronous, nested, thread-local scope for direct serialization; its previous
value is restored even when the closure panics. Serialize inside the closure,
not in a future returned from it.

The old `set_qemu_version` / `qemu_version` and QGA counterparts remain as
process-default compatibility APIs. Connections still initialize these defaults
for existing callers, but typed protocol requests use the connection's version.
New consumers should use the client's `version()` rather than the process
default. The manager's preconfig check also uses the connected peer's version.

Connection, handshake and command waits are bounded. JSON input lines are
bounded to 1 MiB; oversized lines are drained to the next newline. Protocol
errors and QEMU error responses are exposed through the crate's `Error` type.

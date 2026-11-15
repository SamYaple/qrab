# Protocol clients

`qapi-protocol` 0.2 targets the locally vendored QEMU **11.1.3** protocol. It
provides Tokio Unix-socket QMP/QGA clients and bounded mock sessions without
depending on manager, systemd, Kubernetes or guest hardware. Older-release
compatibility work is deferred; historical schemas remain codegen inputs.

## Connecting and configuration

`Qmp::new(path)` returns a client and an event receiver. `Qga::new(path)` returns
a sequential guest-agent client. Both also support `from_stream(stream)` for an
already-connected Unix socket and `*_with_options` constructors accepting
`ClientOptions`. Options set connect, handshake and command timeouts, maximum
JSON message size, and the QMP event buffer capacity.

Defaults are 5 seconds for connect, 10 seconds for the complete handshake,
30 seconds for commands, 128 buffered events with a 1 MiB per-event bound, and
64 MiB + 1 KiB per JSON message. The size accommodates the base64 expansion of QGA's documented
48 MiB guest-file-read result; it is a configurable resource bound, not a
promise that every peer accepts equally large requests. JSON size excludes the
final line terminator. Both compact and pretty-printed QMP objects are accepted,
with LF or CRLF termination. Invalid UTF-8, malformed envelopes, truncated EOF
and oversized frames close the connection; the client never drains an endless
oversized frame waiting for a newline.

QMP validates the typed greeting and sends a correlated `qmp_capabilities`
request. It enables OOB only if advertised. A missing, malformed or incorrectly
correlated capabilities acknowledgement fails the handshake. Events arriving
during negotiation are retained within the configured buffer. `greeting()`
exposes the peer's version, package string and advertised capabilities.

QGA sends an OS-random 64-bit guest-sync-delimited nonce. It flushes parser state
at every `0xFF` sentinel and ignores stale, malformed or mismatched replies until
the nonce matches. The same handshake deadline also bounds guest-info discovery.
An invalid guest-info version fails connection establishment.

## QMP requests and lifecycle

One task owns the socket and routing table. Reads and partial writes progress
independently, so a large upload cannot prevent reading an earlier reply.
Request IDs belong to the connection. Typed calls use `qapi::commands` and
`QapiCommand::Returns`; `execute_raw` accepts an `execute` envelope with optional
object arguments, while `execute_raw_oob` requires `exec-oob`. Caller IDs,
conflicting verbs and malformed arguments fail before any socket I/O. Unknown
response IDs of any JSON type are dropped as required by QMP. An ID-less server
parser error closes the connection while preserving its error class and description.

`execute_raw_with_fds` (and its `getfd(name, fd)` shorthand) attaches file
descriptors to an in-band command as SCM_RIGHTS, the way QEMU's `getfd` and
`add-fd` read them: the driver sends them with the frame's first bytes in one
`sendmsg`, then the rest with ordinary writes. The descriptors are moved in and
closed once sent, or when the request is refused or expires. The manager uses it
to hand a hot-attached NIC's tap and vhost-net descriptors to QEMU.

Admission is bounded to eight unanswered in-band commands, following QEMU's
recommendation for keeping OOB responsive. A separate eight-command bound
limits OOB requests. Permits belong to dispatched requests, not their callers:
cancelling a future cannot free a slot while QEMU may still be executing it.
Requests cancelled while still queued are skipped. `queue_depth()` and
`subscribe_queue()` count dispatched requests, including OOB and requests whose
callers were cancelled; `queue_capacity()` reports the in-band capacity.

Command deadlines cover admission, queued writes and waiting for a reply. They
can be configured per connection, or per call through `execute_with_timeout`
and `execute_raw_with_timeout`. If a dispatched command expires, the connection
closes because its remote outcome and queue occupancy are unknown. A timeout
before dispatch does not invalidate a healthy connection. No automatic retry
occurs: cancellation, timeout or disconnect never proves that a command did not
execute.

`shutdown(&self)` works through `Arc<Qmp>`, wakes both admitted callers and
callers waiting for capacity, closes the socket and ends the driver. Dropping
the owner aborts the driver with the same cleanup. `is_closed()`,
`close_reason()` and `closed().await` expose connection lifecycle. Pending
callers share the original failure via `Error::Disconnected`.

Events are parsed once and delivered without blocking replies. Dropping the
event receiver does not disconnect QMP. Buffer overflow or an event exceeding `max_event_bytes` drops that event;
`dropped_events()` counts these losses, so consumers can detect loss and refresh
state. QEMU itself also rate-limits some events: this stream is not a durable
lifecycle journal. Event timestamps support QEMU's `-1` unavailable-time values.

## QGA requests and lifecycle

QGA calls use `&mut self` and `qapi::qga::commands`. The session is taken out of
the client before socket I/O and restored only after a complete valid response.
Timeout, cancellation, malformed JSON or I/O failure therefore closes the
socket instead of letting a late reply satisfy the next command. Reconnect to
perform synchronization again. Closing a socket does not roll back guest-side
resources or effects; a known file handle may require explicit cleanup through
a newly synchronized connection. A valid agent error or a typed payload decoding
error leaves an aligned session usable. `is_closed()` and `shutdown()` expose
this state explicitly.

Commands with `success-response: false`, such as guest-shutdown and guest-suspend,
use `send_no_response` instead of `execute`. Success means only that the request
bytes were written; no remote success acknowledgement exists. This operation
closes the socket so a possible error reply cannot contaminate a later call.
Typed QMP and QGA calls evaluate OOB and reply flags for their connected peer's
release, including historical additions and removals of those flags.

## Types and errors

Each connection serializes requests and decodes typed replies using its own
peer's `version()`, including guest
agents whose version differs from QEMU. Protocol connections no longer mutate
process-global QAPI version defaults. Direct serialization can use the
synchronous `qapi::with_version(version, || ...)` scope or `qapi::to_value(version, &value)`; outside any scope a version-gated type fails with an error naming `qapi::with_version`; the legacy global APIs
remain only for direct callers. BMC validation and version display use the
connected VM's version.

Responses require exactly one of `return` or a well-formed `error`. Missing
return values are never success. An empty object maps to Rust `()` only when
that is the actual return type; other failed decodes are not retried as null.
`QmpError` retains the exact string `class` and `desc`, including new error
classes; `known_class()` optionally maps it to the generated enum.

The 0.2 API changes are deliberate: `QmpError.class` is a string,
`Timestamp.microseconds` is signed, deserialized return types are owned and
`'static`, and connection shutdown takes `&self`. Consumers should use the
connection's version and lifecycle rather than a process-global snapshot.

See [testing.md](testing.md) for fault tests, the core checks and live QMP/QGA
smoke tests. Mock `server` sessions share the bounded framing and validation
helpers, including explicit response IDs for out-of-order tests.

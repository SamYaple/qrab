# qrab-spire-relay

Guest relay from `127.0.0.1:8081` to the SPIRE server over the virtio-serial
port `org.yapplesauce.spire.0`. See [../../docs/manager.md](../../docs/manager.md#spire-relay)
and [../../docs/spire-vm.md](../../docs/spire-vm.md).

- The listen address is fixed and must be loopback; never add a flag that
  widens it. The relay only moves bytes and never terminates TLS.
- Keep it separate from `qrab-guest-agent`, whose allowlist forbids network
  functions.
- Keep it static, small and std-only: synchronous threads, no async runtime
  or dependencies. The image builds it for `x86_64-unknown-linux-musl`.
- `wire` is shared with the manager's `pod::spire_relay`; changing it changes
  the host/guest contract. Run `cargo test --locked -p qrab-spire-relay` and
  `cargo test --locked -p manager`.

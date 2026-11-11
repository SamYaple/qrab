# qrab-guest-agent

Guest-side QGA daemon. See [../../docs/guest-agent.md](../../docs/guest-agent.md).

- The command table in `src/agent.rs` is the allowlist. Never add arbitrary
  execution, file read/write, account or network-configuration commands; the
  update hook path stays fixed at `proto::UPDATE_HOOK`.
- Keep it static and small: synchronous std threads, `libc`, `serde_json` and the
  generated `qapi` types. No tokio or async runtime in the agent itself.
- OS effects go through `sys::Platform`, so protocol tests run unprivileged over
  a socketpair with the real `qapi-protocol` client. Run
  `cargo test --locked -p qrab-guest-agent`.
- `proto` is shared with the manager; changing a `qrab-*` wire type changes the
  host/guest contract.

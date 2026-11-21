# qrab

qrab runs QEMU virtual machines on Kubernetes and gives each VM the
management and identity surface of a physical server. A Rust operator
reconciles namespaced `VirtualMachine` and `VirtualMachineMigration` custom
resources into VM pods; in each pod, `qrab-manager` supervises QEMU, and BMC
and Redfish sidecars expose the VM the way a server's baseboard management
controller would.

What sets qrab apart:

- **A per-VM Redfish service and BMC.** Every VM gets its own DMTF Redfish
  endpoint (power, inventory, boot, virtual media, firmware and Secure Boot,
  events and telemetry) and a browser console (display, keyboard and mouse
  through noVNC), so existing bare-metal tooling and people manage VMs as
  they would servers.

**Status: pre-alpha.** APIs, resource formats and manifests change without
notice and there are no releases yet. qrab is preparing to apply to the
[CNCF](https://www.cncf.io/) as a Sandbox project. See the
[roadmap](ROADMAP.md) for what comes next. Website: <https://qrab.dev>.

## Project

| | |
| --- | --- |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Building, testing, the pull request flow and the DCO sign-off |
| [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) | The CNCF Code of Conduct and how to report a violation |
| [GOVERNANCE.md](GOVERNANCE.md) | Roles, decision making and how to become a maintainer |
| [MAINTAINERS.md](MAINTAINERS.md) | Current maintainers and how to reach them |
| [SECURITY.md](SECURITY.md) | Supported versions and how to report a vulnerability privately |
| [ROADMAP.md](ROADMAP.md) | Near-term themes |
| [ADOPTERS.md](ADOPTERS.md) | Who runs qrab; add yourself |

Questions, bugs and feature requests go to
[GitHub issues](https://github.com/qrab-dev/qrab/issues). Report security
problems privately, as described in [SECURITY.md](SECURITY.md).

## Development

You need a Rust toolchain, Rust 1.88 or newer, with `rustfmt` and `clippy`.
Then:

```sh
cargo build --locked
cargo test --locked
bash contrib/check-runtime.sh
```

[CONTRIBUTING.md](CONTRIBUTING.md) describes the pull request flow.

## License

qrab's code is licensed under the [Apache License 2.0](LICENSE). Documentation
is licensed under [CC-BY-4.0](LICENSE-docs).
`cargo deny --locked check licenses` holds Cargo dependencies to the CNCF
allowed license list.

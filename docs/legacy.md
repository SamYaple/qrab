# Legacy host and confidential-guest tooling

The active deployment path is the pod manager in [manager.md](manager.md).
The following code remains available for explicit development, outside the
normal `cargo build` / `cargo test` package selection:

| Component | Purpose |
| --- | --- |
| `manager` with `legacy-host` | Host namespaces, firmware discovery and the SEV/SVSM `VmBuilder` API |
| `bmc --features server` | Earlier BMC server; explicitly enables the legacy manager feature |
| `qrab-init` | Host namespace helper used by that earlier manager |
| `qrab-boot`, `qrab-real-uki-demo` | Standalone UEFI targets with their own toolchains |
| `qrab-boot-*`, `qrab-snp-proto`, `qrab-attest-verifier`, `attest-server`, `qrab-guest-init` | Confidential-guest boot, attestation and policy experiments |
| `contrib/build-*`, `matrix-*`, `smoke-snp-*` | Specialized firmware/hardware build and smoke procedures |

The old API keeps its module names and re-exports when enabled:

```toml
manager = { path = "../manager", features = ["legacy-host"] }
```

Safe host API/unit and BMC compilation checks are:

```sh
cargo test --locked -p manager --features legacy-host --lib
cargo check --locked -p bmc --features server
```

The legacy manager integration targets require `legacy-host`. Selecting that
feature and running all integration tests is an explicit hardware/firmware test
operation; it is not part of pod-runtime validation. A passing TCG guest smoke
does not validate confidential-guest security or firmware behavior.

UEFI crates are excluded from the Linux workspace and must be built from their
own directories. Other experiment crates remain workspace members and can be
selected with `-p`; `--workspace` intentionally selects these as well. See each
component's source and build script for its prerequisites before running it.

Historical `llm-rag` plans, audit IDs and absolute workspace paths in older
comments are provenance, not self-contained runbooks. Those external documents
are not required for the pod runtime and are not shipped here. Old phase-status
claims should not be treated as evidence of current firmware validation.

Do not initialize every vendor submodule for ordinary development. In particular,
the historical `qemu-svsm` pin is not available in the current local SVSM mirror;
that lane needs deliberate source reconciliation before it can be restored.
Do not silently switch it to another revision or fetch QEMU from upstream.

The obsolete Debian QEMU 3.1/5.2/7.2 container wrapper matrix was retired. Current
images are built from pinned local source under `contrib/images`, and current
live tests are documented in [testing.md](testing.md). Older-version
compatibility will be revisited separately.

# qapi-codegen

Parses QEMU's [QAPI](https://www.qemu.org/docs/master/devel/qapi-code-gen.html) JSON schema files and generates Rust type definitions.

The codegen extracts schemas from every release tag in a QEMU git repository, parses each with version-appropriate nom parsers, and merges the results into a single type registry. The output carries precise `added`, `deprecated`, and `removed` annotations derived from the full version history.

## Usage

```sh
# QMP types
cargo run -p qapi-codegen -- vendor/qemu > projects/qapi/src/generated.rs

# QGA types
cargo run -p qapi-codegen -- vendor/qemu --qga > projects/qapi/src/qga.rs
```

The binary accepts a path to any QEMU git repository. Version tags are discovered automatically.

## Structure

- **`parsers/`** — nom parsers with version-dispatched variants (`parse_v1`, `parse_v2`, etc.) that handle schema format changes across QEMU releases without preprocessing
- **`qapi_ir/`** — intermediate representation and Rust code generation
- **`unified.rs`** — multi-version merge that builds a single type registry from all parsed versions
- **`versions.rs`** — git tag discovery and parallel schema extraction

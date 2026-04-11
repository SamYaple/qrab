# Codegen Pipeline (qapi-codegen)

Parses QEMU's QAPI JSON schemas from every release tag in the QEMU git repo
and generates a single unified set of Rust types with version lifecycle
metadata.

## Pipeline Overview

1. **Tag discovery** — Scan `vendor/qemu` for all release tags (v1.0+).
2. **Schema extraction** — For each tag, `git archive` + `tar` extracts the
   `.json` schema files into a tempdir.
3. **Nom parsing** — Each schema is parsed into `QapiSchema(Vec<QapiSchemaToken>)`.
   A thread-local `PARSER_VERSION` controls version-specific parse behavior.
4. **IR conversion** — Parser AST types (`QapiStruct`, `QapiEnum`, etc.) are
   converted to intermediate representation (`Struct`, `Enum`, `StructField`,
   `EnumVariant`) with extracted metadata (docs, conditions, features, `@since`).
5. **Unified merge** — All versions are merged into a single type registry.
   Each type, field, and variant carries a `VersionRange` with `since`, `until`,
   `deprecated`, and `unstable` annotations.
6. **Code emission** — IR types generate `TokenStream` with `#[qapi(...)]`
   attributes. `prettyplease` formats the output. Result goes to stdout.

## Key Files

| File | Stage | Entry Point |
|---|---|---|
| `src/main.rs` | CLI | `main()` — arg parsing, orchestration |
| `src/versions.rs` | 1-2 | `discover_version_tags()`, `extract_schema_from_tag()` |
| `src/parsers/mod.rs` | 3 | Module root, `PARSER_VERSION` thread-local |
| `src/parsers/common.rs` | 3 | Shared nom combinators (`qstring`, `qtag`, `take_list`, etc.) |
| `src/parsers/qapi_schema.rs` | 3 | `take_qapi_schema()` — top-level parser |
| `src/parsers/qapi_struct.rs` | 3 | Struct definition parser |
| `src/parsers/qapi_enum.rs` | 3 | Enum definition parser |
| `src/parsers/qapi_command.rs` | 3 | Command definition parser |
| `src/parsers/qapi_event.rs` | 3 | Event definition parser |
| `src/parsers/qapi_union.rs` | 3 | Union definition parser |
| `src/parsers/qapi_documentation.rs` | 3 | `##`-doc comment parser |
| `src/qapi_ir/mod.rs` | 4 | Type mapping (`qapi_to_rust_type()`), name mangling |
| `src/qapi_ir/process_qapi.rs` | 4 | AST to IR conversion (911 lines) |
| `src/qapi_ir/rust_struct.rs` | 6 | Struct `TokenStream` generation |
| `src/qapi_ir/rust_enum.rs` | 6 | Enum `TokenStream` generation |
| `src/unified.rs` | 5-6 | `generate_unified_code()` — merge + emit (1465 lines) |

## Parser Architecture

One parser file per QAPI construct, each exporting a type and `take_*`
function. The parsers directory also contains files for sub-elements:
`qapi_member.rs`, `qapi_members.rs`, `qapi_branch.rs`, `qapi_branches.rs`,
`qapi_variant.rs`, `qapi_alternative.rs`, `qapi_alternatives.rs`,
`qapi_cond.rs`, `qapi_feature.rs`, `qapi_features.rs`, `qapi_type_ref.rs`,
`qapi_include.rs`, `qapi_pragma.rs`.

The parser handles all QEMU schema format variations from v1.0 to latest.
Source schemas are treated as golden — see
[design-constraints.md](design-constraints.md).

## Dependencies

| Crate | Purpose |
|---|---|
| `nom 8.0` | Parser combinators |
| `quote` + `proc-macro2` + `syn` | Rust code generation via TokenStream |
| `prettyplease` | Formats generated code |
| `heck` | Case conversion (PascalCase, snake_case) |
| `tar` + `tempfile` | Extract schemas from git archive |
| `anyhow` | Error handling |

## Regeneration

```sh
# QMP types
cargo run -p qapi-codegen -- vendor/qemu > projects/qapi/src/generated.rs

# QGA types
cargo run -p qapi-codegen -- vendor/qemu --qga > projects/qapi/src/qga.rs
```

The `vendor/qemu` submodule must point to a QEMU git repository with full
tag history.

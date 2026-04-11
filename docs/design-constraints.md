# Design Constraints

Hard rules for all work in this repository. Violating these breaks the
architecture or loses information.

## 1. Source Schemas Are Golden

QEMU's QAPI JSON schema files are the single source of truth. Parsers must
extract **all** information from the raw format — doc comments, field
descriptions, version annotations, notes, examples — without preprocessing
or transforming the input.

If the parser cannot handle a schema format or version, add parser support.
Never preprocess schemas or skip problematic types/fields.

## 2. Generated Files Are Outputs

`projects/qapi/src/generated.rs` and `projects/qapi/src/qga.rs` are produced
by `qapi-codegen` and checked in for convenience. Never hand-edit them — edits
are overwritten on the next codegen run. Fix bugs in the codegen pipeline, then
regenerate.

## 3. All QEMU Release Tags Are Processed

The codegen discovers tags automatically from `vendor/qemu` (v1.0 onward).
Never hardcode version lists. Use `discover_version_tags()` for enumeration.
Updating the submodule is sufficient when QEMU releases a new version.

## 4. Dual-Layer Version Gating

Both layers must remain correct:

| Layer | Mechanism | Effect |
|---|---|---|
| Compile-time | `QEMU_MIN_VERSION` / `QEMU_MAX_VERSION` env vars | Prunes types/fields outside window from compilation |
| Runtime | `qapi::set_qemu_version(major, minor, patch)` (`OnceLock`) | Skips fields outside `since`/`until` range during serde |

When modifying version annotations, verify both layers. A field must be
reachable at compile time for any version in its `since..until` range and
correctly gated at runtime.

## 5. Parser Handles All Format Variations

QEMU's QAPI schema format has changed across releases (syntax, comment styles,
new features). The nom parser is the adaptation layer — it adapts to QEMU, not
the other way around.

The `PARSER_VERSION` thread-local in `parsers/mod.rs` controls version-specific
parse behavior. Test parser changes against schemas from multiple QEMU versions.

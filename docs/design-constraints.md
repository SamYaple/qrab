# Design Constraints

Hard rules that apply to all work in this repository. Violating these will
break the architecture or lose information.

## 1. Source Schemas Are Golden

QEMU's QAPI JSON schema files are the single source of truth. Parsers must
extract **all** information from the raw format without preprocessing or
transforming the input.

**Why:** The goal is to expose the full QEMU API history through Rust types.
Losing doc comments, field descriptions, version annotations, notes, examples,
or any other content is unacceptable.

**In practice:** If the parser cannot handle a particular schema format or
version, add parser support for that format. Never work around parse
difficulties by preprocessing the schema files or skipping problematic
types/fields.

## 2. Generated Files Are Outputs

`projects/qapi/src/generated.rs` and `projects/qapi/src/qga.rs` are produced
by running `qapi-codegen`. They are checked into the repo for convenience but
are not hand-edited.

**Why:** Any manual edits will be overwritten on the next codegen run. Bugs in
generated code must be fixed in the codegen pipeline.

**In practice:** To change generated output, modify `qapi-codegen` (parsers,
IR, unified merger, or code emission), then regenerate.

## 3. All QEMU Release Tags Are Processed

The codegen discovers release tags automatically from the `vendor/qemu` git
repo. Every release tag from v1.0 onward is processed and merged into the
unified type registry.

**Why:** The type system must cover the full QEMU API history so that one
compiled binary can work against any QEMU version. Skipping tags creates gaps
in version lifecycle metadata.

**In practice:** Never hardcode a list of versions. Use `discover_version_tags()`
for tag enumeration. When QEMU releases a new version, updating the submodule
is sufficient — the codegen picks it up automatically.

## 4. Dual-Layer Version Gating

Version awareness operates at two levels and both must remain correct:

**Compile-time:** The `#[qapi]` macro reads `QEMU_MIN_VERSION` and
`QEMU_MAX_VERSION` environment variables. Types and fields entirely outside
this window are excluded from compilation, reducing binary size for
applications targeting a known QEMU range.

**Runtime:** `qapi::set_qemu_version(major, minor, patch)` sets a global
`OnceLock`. During serde operations, fields with `since`/`until` annotations
check this version and are skipped if outside range. This allows a single
binary to adapt to whatever QEMU version it connects to.

**In practice:** When adding or modifying version annotations, verify both
layers. A field must be reachable at compile time for any version in its
`since..until` range, and must be correctly gated at runtime.

## 5. Parser Handles All Format Variations

QEMU's QAPI schema format has changed over its release history (new features,
syntax changes, different comment styles). The nom parser must handle all
variations from v1.0 to the latest release.

**Why:** Combined with rule 1 (schemas are golden), this means the parser is
the adaptation layer. It adapts to QEMU, not the other way around.

**In practice:** The `PARSER_VERSION` thread-local in `parsers/mod.rs`
controls version-specific parse behavior. Test parser changes against schemas
from multiple QEMU versions.

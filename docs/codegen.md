# QAPI generation

`qapi-codegen` uses nom **8.0.0** and parses QEMU's QAPI language into borrowed
schema tokens, lowers these to Rust IR, then merges releases to annotate
introductions, removals, flags and fields. QMP and QGA use the same pipeline.

## Inputs

`vendor/qemu` points at locally vendored QEMU **v11.1.3**, commit
`005c61c0ef6f6ae615283440b4bb7be51b2c6ddd`. The source remote is
<https://gitea.yapplesauce.com/mirrors/qemu.git>.

`projects/qapi-codegen/qemu-versions.lock` pins every selected stable release
from v0.15.0 through v11.1.3 by tag and peeled commit. QGA input starts at
0.15; QMP has no schema before 1.0. The generator checks all
pins before parsing. Extra local tags and release candidates cannot alter the
default output. Missing or moved pins fail generation. QEMU source and tag
fetches use this local Gitea repository exclusively; do not fetch directly from
upstream. Library functions may still accept explicit tag lists for testing;
the CLI always uses a lockfile. Stable release tags may have two, three or
four numeric components. Historical fourth-component releases retain their
full ordering in discovery and pins, while lifecycle metadata and peer versions
use the three components carried by QMP greetings. The four mirrored subpatch
releases have schema-equivalence regression checks against their base tags.

`vendor/qemu` is the only submodule. Initialize it and fetch the release tags:

```sh
git submodule sync -- vendor/qemu
git submodule update --init vendor/qemu
git -C vendor/qemu fetch origin --tags
```

## Compatibility policy

The maintained QEMU matrix covers release series **9.2, 10.0, 10.1, 10.2,
11.0 and 11.1**. The latest pinned patch in each series is the representative
schema input. The current baseline remains 11.1.3; see `docs/testing.md` for
explicit live-QEMU checks. Schema coverage does not imply live runtime testing
of every release.

Older QEMU is **untested, not unsupported** below this matrix. Historical
schemas remain generation inputs and regression fixtures back to QEMU 1.0,
where the QMP schema first appears, and QGA 0.15. Keep version-specific grammar where the
language changed, and accept explicit older input sets through the library or
`--versions-file`; do not add a minimum-version rejection to the parser.

Lexing follows QEMU's Python parser, including Unicode/C0 whitespace and
LF, CRLF or CR line endings. Shared nom 8 helpers borrow the original source;
parse errors report its line and column. A differential lexical corpus checks
acceptance against the parser in the pinned QEMU tree.
Legacy comments naming a later definition in the same file follow that
definition across intervening helpers; existing attached documentation wins.

## Regeneration

Run from the repository root:

```sh
cargo run --release --locked -p qapi-codegen -- vendor/qemu \
  --output-dir projects/qapi/src/generated
cargo run --release --locked -p qapi-codegen -- vendor/qemu --qga \
  --output-dir projects/qapi/src/qga
```

Each invocation writes `mod.rs`, `commands.rs`, `events.rs` and `metadata.rs`
after all selected inputs have parsed and generated valid Rust. `--output-dir`
is required. For an explicit alternative release set, pass
`--versions-file /path/to/lockfile` containing the same `tag commit` format.
Normal builds consume these files directly and perform no network access or
code generation. Use release mode for generation and parser integration tests;
unoptimized parsing across the full release history is much slower.

When advancing QEMU, select a stable release available in the local mirror,
advance the submodule, and add the newly selected tag/commit pairs in numeric
release order. Regenerate both protocols and run
`contrib/check-qapi.sh`. Keep old pins to preserve lifecycle metadata.

## Correctness boundaries

Modern schemas require commas and reject duplicate dictionary keys. The
permissive separator grammar remains restricted to pre-2.4 schemas for legacy
compatibility. Includes are canonicalized, aliases are read once, and include
cycles fail. Complete-file parsing rejects any unconsumed input. Strings follow QEMU's
printable-ASCII grammar, including escaped backslashes in include paths.
Conditions before 6.2 retain C expressions and lists of expressions; newer
conditions require configuration identifiers and nonempty boolean expressions.
Empty enums and structs are valid, but their `data` key is still required.
Referenced predefined `QType` and `q_empty` definitions are synthesized from
QEMU 2.6 onward. The QType number variants follow the 2.10 `qint`/`qfloat` to
`qnum` transition. Implicit-definition versions participate in schema hashes,
so identical source files across that boundary still retain both wire shapes.
Implicit `ElementList` references normalize to ordinary arrays. Scalar builtin
lists are predefined; named lists require an explicit array use somewhere in
the schema graph. Nested arrays and declarations with reserved `List` suffixes
are rejected before lowering.
Alternates reject overlapping JSON-type branches, including broad `any` values.
Before 2.10 integer and floating-point branches remain distinct; newer schemas
also apply QEMU's string and enum scalar-conversion conflict rules.
Boxed command and event data must name an object type. Missing, anonymous,
scalar and false-boxing forms fail before lowering; valid named objects keep
their fields flat on the wire.
Named union argument types require boxing for both commands and events;
scalars, enums and alternates cannot serve as argument objects.
Command flags enforce QEMU's true-only execution flags, false-only modern
generation/reply flags and incompatible OOB/coroutine modes. Legacy string
generation and reply flags retain their historical handling, including
`success-response: 'no'` before 2.4.
Generated command constants describe the newest schema flags. The
`allows_oob`, `allows_preconfig` and `sends_success_response` trait methods
query their added/removed ranges for a particular peer release.
Flags that disappear and return retain separate ranges, including historical
simple-union layouts. Repeated macro ranges combine independently of order.
Instability also tracks stabilization and later reintroduction for types and
members. Command `is_unstable(version)` queries the peer release; reflection
and the `UNSTABLE` constant describe the newest declaration.
From 8.1, conditional members in command/event argument objects require boxing,
including members inherited from a base. Earlier schemas retain the older rules.
Type references resolve against exact QAPI names across all source files before
Rust name mapping. Undefined member, branch and return types report their source
and definition; historical inline objects include the nested member path.
Definition names share one namespace across files and declaration kinds;
duplicates report both sources before Rust symbol planning.
Member clash checks include inherited fields and each flat-union branch against
its base. Alternative branches retain independent member namespaces. Names
that collapse to the same QAPI C member identifier fail with source context.
Historical anonymous structs lower recursively, including optional and empty
objects. Extracted type names avoid declared types and generated role names,
so anonymous objects cannot overwrite unrelated schema definitions.
QEMU's positive schema fixtures are checked through parsing, IR lowering and
compilation of complete generated Rust modules, including its schema stress test.
Flat unions may omit every payload branch when their discriminator enum is
nonempty. Generated branch enums carry `union_branch` even when all variants
have no payload, preserving discriminator dispatch and empty-object wire shape.
Discriminators must be required scalar enums; unknown tags and empty discriminator
enums fail IR processing. Historical non-enum discriminators retain legacy handling.
Derived structs resolve bases independently of definition and include order;
named bases of structs and unions must refer to plain structs.
Missing bases, inheritance cycles and conflicting Rust type names fail with
schema context.

A missing QMP schema is allowed only before 1.0, and a missing QGA schema only
before 0.15. These exceptions reflect source availability, not minimum supported
peer versions. Present schemas that fail parsing or IR processing are errors,
never skipped releases. Early guest-agent `qapi-schema-guest.json` files are
extracted alongside the later `qga/qapi-schema.json` layout. Duplicate emitted Rust names and invalid generated Rust also
fail generation. Historical documentation-name warnings are listed in
`projects/qapi-codegen/KNOWN_ISSUES.md`.

Commands sharing a Rust name with their referenced argument struct retain the
existing merged representation, including the exact command wire name. A
command colliding with a distinct response type gets a `Command` suffix:
`qapi::qga::commands::GuestFileReadCommand` returns
`qapi::qga::GuestFileRead`. The same distinction applies to guest-file-write,
guest-file-seek, guest-exec, guest-exec-status and guest-fsfreeze-status.

A command's `Returns` type is the newest pinned release's by default, since a
Rust associated type cannot vary by peer version. A `QEMU_MAX_VERSION` build
selects the newest return type within that ceiling. Earlier return types stay
in the registry as `UnifiedStruct::returns_history`. The changes across pins are
pinned by a test: `device-list-properties` (`DevicePropertyInfo` until 2.12,
then `ObjectPropertyInfo`), `query-target` (`TargetInfo` until 10.1, then
`QemuTargetInfo`) and `guest-fstrim` (unit until 2.4, then
`GuestFilesystemTrimResponse`).

The test suite compares definition sets with QEMU's own Python parser for both
current QMP and QGA schemas. Live QMP validation additionally checks that every
advertised command appears under its exact wire name in generated metadata.

Generated enum serializers enforce both type and variant lifetimes. The same
checks apply to alternate values and union branch payloads, including scalar
payloads whose Rust types carry no QEMU metadata. Invalid outgoing values name
the unavailable type or variant and peer version. Enum deserialization does not
enforce serialization lifetimes, so recorded and newer replies can still be
inspected.

Definition documentation is parsed as complete comment blocks. A closing `##`
is never treated as prose, and malformed modern blocks cannot fall back to
ordinary comments. Member and feature text is compared with QEMU's own parser
for the production schemas and its documentation fixture. Generated comments
retain ordered prose, Returns/Errors sections and literal examples; member
continuation indentation is normalized separately from literal-block indentation.
Legacy headers, optional closing markers and the historical `###` terminator
remain accepted for older inputs.

Historical simple unions retain their nested `{"type": ..., "data": ...}` wire
shape, including scalar, array and null payloads. The `simple_union` property
tracks transitions to flat unions per type rather than using a global cutoff.
Use `qapi::with_version` when decoding a recorded historical layout; unscoped
decoding selects the newest layout. Protocol clients scope typed reply decoding
to each peer, including OOB replies.

When a branch payload type or wire spelling changes, the latest Rust variant
name remains stable and earlier representations are emitted as lifetime-gated
`Legacy` variants. Repeated changes receive distinct suffixes without colliding
with schema-defined names. Discriminator dispatch selects the representation
for the scoped peer version; unscoped inspection uses the newest shape and
does not fall back to an obsolete permissive shape after a decoding error.
Generated metadata carries each representation's original wire tag and lifetime.
Fields and enum members that disappear and later return retain separate
lifetimes, so the intervening releases do not inherit the restored member.
String enum decoding selects the active representation of a shared wire tag.

Field payload changes, optionality changes and wire renames retain `_legacy`
representations with separate lifetimes. Shared wire names populate only the
selected representation;
serializing a value in an inactive representation fails instead of discarding it.
A required wire field remains required in metadata even when version merging
needs a Rust `Option`. Explicit peer contexts enforce active required fields;
unscoped record inspection remains lenient. Present nullable values retain
`Some(null)` separately from absent fields.

When a named type changes between a string enum, alternate and object, earlier
definitions receive lifetime-gated `Legacy` names. References follow each
release's definition; a return to an earlier kind starts a separate lifetime.
Structs and tagged unions retain their shared object field history.

QAPI build conditions describe the remote QEMU or guest-agent build. They are
retained in generated documentation, never translated into Rust host OS gates.
Conditional fields use `Option` because a peer can omit them even when their
schema declaration is required when enabled. Metadata retains that declaration's
wire optionality; version checks cannot determine the peer's build configuration.
Condition presence changes retain field history so later unconditional fields
become required. Generated conditions describe the newest declaration rather
than an obsolete condition inherited from an earlier release.

`QEMU_MIN_VERSION`/`QEMU_MAX_VERSION` builds prune representations outside the
requested range. Command associated return types use the newest shape within
the configured maximum, or the newest pinned shape without a maximum.
`contrib/check-qapi.sh` compiles all maintained release series and restores an
unbounded development build afterwards. Recursive-type analysis ignores `Vec`
and existing `Box` indirections, which already make recursive values sized.

The historical `union` form with an empty discriminator dictionary is an
untagged alternate, not a simple tagged union. It lowers to the same bare-value
Rust enum as modern `alternate` syntax, preserving the original introduction
version and feature metadata.

Serde macro expansions allocate helper names against user fields, referenced
types and other generated helpers. Fields such as `map` and internal-looking
identifiers cannot capture JSON storage, visitors or serde generic parameters.
These cases have compile-and-round-trip regressions.

Rust identifiers are escaped by role. Reserved type names use a `Qapi` prefix;
enum variants keep ordinary names such as `String` and `None`, with `Self`
escaped. Fields `self`, `crate` and `super` receive a trailing underscore and
retain their wire-name attributes. Type declarations use type-name mapping,
not builtin-value mapping: a declared `Visitor` remains a real struct, while
the historical lowercase `visitor` opaque reference remains a JSON value.
A QEMU-validated fixture is generated, compiled offline and round-tripped to
check declarations, references, commands and metadata together.
Generated union branches, simple-union tags, event types and command fallbacks
avoid declared symbols, adding `Generated` or numbered suffixes when needed.
References and wire names remain attached to the correct definitions.
Exact QAPI type references resolve before Rust conversion. Coexisting declarations
such as `Option`/`QapiOption` or `FooBAR`/`FooBar` receive distinct Rust symbols;
mutually exclusive historical spellings can retain a shared identity.
Coexisting field names that normalize alike receive stable `_alt` names across
the selected history. Allocation reserves future declared fields and distinguishes
virtual flatten receivers from wire fields, preserving keys such as `pc` and `PC`.
Enum, alternate and branch variants use the same history-aware reservation,
with `Alt`/`AltN` names for collisions. Original tags remain in wire attributes,
including keyword escapes such as `self` and numeric names such as `a-1`.
Name planning covers every selected release before IR lowering, so a newer
declaration cannot overwrite an older generated branch or anonymous object.
Extraction, name collection and lowering use bounded workers; schema hashes
still deduplicate identical releases during merging.

Metadata resolves field and variant references as QAPI names, and command
returns as already mapped Rust names. A declared `String` is therefore distinct
from builtin `str`. Enum variants expose `payload_type` indices into `TYPE_REFS`
for scalar, null, enum, struct and list payloads; `payload_struct` is retained
for direct struct payloads.
`EnumMeta::name` is the Rust lookup identity; `wire_name` retains the QAPI
spelling through alias and legacy-type allocation. Synthesized enums fall back
to their generated name.
Union metadata selects the newest observed discriminator and branch fields.
Historical discriminator representations remain ordinary versioned fields;
every generated branch enum retains its `FlatUnionBranch` classification.

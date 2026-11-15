# QAPI generation

`qapi-codegen` uses nom **8.0.0** and parses QEMU's QAPI language into borrowed
schema tokens, lowers these to Rust IR, then merges releases to annotate
introductions, removals, flags and fields. QMP and QGA use the same pipeline.

## Inputs

`vendor/qemu` points at locally vendored QEMU **v11.1.3**, commit
`005c61c0ef6f6ae615283440b4bb7be51b2c6ddd`. The source remote is
<https://gitea.yapplesauce.com/mirrors/qemu.git>.

`projects/qapi-codegen/qemu-versions.lock` pins every selected stable release
from v1.0 through v11.1.3 by tag and peeled commit. The generator checks all
pins before parsing. Extra local tags and release candidates cannot alter the
default output. Missing or moved pins fail generation. QEMU source and tag
fetches use this local Gitea repository exclusively; do not fetch directly from
upstream. Library functions may still accept explicit tag lists for testing;
the CLI always uses a lockfile.

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
where the QMP schema first appears. Keep version-specific grammar where the
language changed, and accept explicit older input sets through the library or
`--versions-file`; do not add a minimum-version rejection to the parser.

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
Derived structs resolve bases independently of definition and include order;
missing bases, inheritance cycles and conflicting Rust type names fail with
schema context.

QGA releases predating a guest-agent schema are allowed to have no QGA input.
A present schema that fails parsing or IR processing is an error, never a
skipped release. Duplicate emitted Rust names and invalid generated Rust also
fail generation. Historical documentation-name warnings are listed in
`projects/qapi-codegen/KNOWN_ISSUES.md`.

Commands sharing a Rust name with their referenced argument struct retain the
existing merged representation, including the exact command wire name. A
command colliding with a distinct response type gets a `Command` suffix:
`qapi::qga::commands::GuestFileReadCommand` returns
`qapi::qga::GuestFileRead`. The same distinction applies to guest-file-write,
guest-file-seek, guest-exec, guest-exec-status and guest-fsfreeze-status.

A command's `Returns` type is the newest pinned release's, since a Rust
associated type cannot vary by peer version. Earlier return types stay in the
registry as `UnifiedStruct::returns_history`. The changes across pins are
pinned by a test: `device-list-properties` (`DevicePropertyInfo` until 2.12,
then `ObjectPropertyInfo`), `query-target` (`TargetInfo` until 10.1, then
`QemuTargetInfo`) and `guest-fstrim` (unit until 2.4, then
`GuestFilesystemTrimResponse`).

The test suite compares definition sets with QEMU's own Python parser for both
current QMP and QGA schemas. Live QMP validation additionally checks that every
advertised command appears under its exact wire name in generated metadata.

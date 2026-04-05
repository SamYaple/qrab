# Known Parser / Codegen Issues

## Warnings (non-blocking)

### Doc section name mismatches

QEMU renamed some types between versions but kept old documentation
section names. The doc `@Name` in the `##` block doesn't match the
actual type definition name.

| Doc Name | Actual Type | Versions |
|---|---|---|
| `ChardevBackend` | `ChardevDummy` | v2.0.0–v3.1.0 |
| `InputButton` | `InputAxis` | v2.0.0–v4.2.0 |
| `Rocker` | `RockerSwitch` | v2.4.0–v5.2.0 |
| `TpmInfo` | `TPMInfo` | v2.5.0–v10.2.0 |
| `VncPriAuth` | `VncPrimaryAuth` | v2.7.0–v4.2.0 |

These are harmless — the doc content is still attached to the correct
type via the unified merge. The warning fires because the parser's
`add_docs!` macro checks `doc.name != type_name`.

### BlockdevOptions discriminator type 'str'

In QEMU v1.7.0–v2.0.0, `BlockdevOptions` used `'discriminator': 'str'`
(a bare string type) instead of a named enum type. The IR processing
expects the discriminator to reference an enum. This is handled by
skipping the enum lookup, but the union struct may have a less precise
discriminator type.

### CpuInfo field 'PC' renamed to 'PC_alt'

The `CpuInfo` union (removed in v9.0.0) had a base field `'CPU': 'int'`
in all versions, but an older version (v1.0) also had a `'PC': 'int'`
field. Both `CPU` and `PC` rustify to names that don't collide (`Cpu`
and `Pc`), but the `PC` field from v1.0 was renamed to `PC_alt` to
avoid a collision with `pc` from `CpuInfoX86`. This is a version-merge
artifact and correct behavior.

## Recursive type boxing

The codegen detects recursive types (where a struct field references
the containing struct, directly or transitively) and wraps them in
`Box<>`. This is correct and necessary for Rust's type system.
32 fields are currently boxed across 15 struct families (mostly in
the block layer: `BlockdevOptions*`, `ImageInfo*`, `BlockStats`).

## Type merges

When a command transitioned from inline data to referencing an existing
struct (e.g., `drive-mirror` changed from inline `'data': {...}` to
`'data': 'DriveMirror'` with `'boxed': true`), the codegen merges the
command attributes onto the existing struct and removes the duplicate.
4 types are currently merged:

- `blockdev-snapshot-sync` → `BlockdevSnapshotSync`
- `drive-mirror` → `DriveMirror`
- `migrate-set-parameters` → `MigrateSetParameters`
- `SgxInfo` → `SGXInfo` (renamed in v10.1.0)

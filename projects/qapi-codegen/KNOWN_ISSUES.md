# Historical schema diagnostics

The pinned history parses and generates both protocols. These diagnostics
describe old source quirks, not a minimum supported QEMU release. The maintained
release matrix and generation rules are in [the pipeline guide](../../docs/codegen.md).

## Mislabeled legacy documentation

Some pre-2.9 comments name a symbol that does not exist in that file:

- `Rocker` above `RockerSwitch`
- `VncPriAuth` above `VncPrimaryAuth`

The generator warns and retains the attached text. When the named definition
does exist later in the same file, legacy documentation follows it without
overwriting its own comment. Formal documentation from 2.9 requires the exact
definition name. Case-only differences do not trigger the legacy warning.

## String discriminators

QEMU 1.7–2.0 used `str` for the `BlockdevOptions.driver` discriminator.
The generator retains that string field and the declared payload branches,
and warns that the discriminator is not an enum. Its Rust type cannot restrict
the driver to the declared branch tags as newer enum discriminators do.

## Generated representation choices

Recursive object references use `Box`; existing `Vec` indirection needs no
additional box. This is normal Rust sizing, not a missing schema feature.

History-wide name allocation preserves distinct wire fields such as `PC` and
`pc` with `_alt` Rust names. Safe command/argument sharing preserves the command
wire name; requests receive separate types when data lifetimes or name
collisions make sharing incorrect. These choices are covered by compiled
generation fixtures.

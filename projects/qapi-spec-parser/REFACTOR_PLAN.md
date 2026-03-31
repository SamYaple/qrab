# qapi-spec-parser Refactoring & Testing Plan

## Phase 1: Idiomatic Rust 2024 Cleanups (R1) ✓ COMPLETE

Most items were already applied in prior work. Remaining fix applied:
- Field init shorthand (`name: name` -> `name`) in `qapi_ir/mod.rs:129`

All 50 tests pass.

## Phase 2: nom 8 Idiomatic Migration (R2)

Replace legacy nom patterns with nom 8 idioms.

**Pattern 1: `map(parser, closure)` -> `parser.map(closure)`**

All parser files use `map(take_kv(...), |v| ...)` inside `alt()`. nom 8 supports `.map()` directly on `Parser`. This affects every file in `src/parsers/` — estimated 50+ call sites.

Example:
```rust
// Before
map(take_kv("command", qstring), |v| s.name = v)
// After
take_kv("command", qstring).map(|v| s.name = v)
```

**Pattern 2: Old-style function call syntax**

`qapi_documentation.rs:258-259` uses `tag("##")(input)?` instead of `tag("##").parse(input)?`. Update to `.parse()` style.

**Note:** Remove unused `map` import from `nom::combinator` in files where all `map` calls are converted.

## Phase 3: Parser Code Deduplication (R3)

### 3a. Remove identical `take_multiline_text` / `take_doc_text`
- `qapi_documentation.rs` — these two functions are identical
- Delete `take_doc_text`, replace all its call sites with `take_multiline_text`

### 3b. Unify `QapiBranch` and `QapiAlternative`
Both are structurally identical (name, type ref, optional condition). Per the QAPI spec:
- A **branch** appears in unions (discriminated by enum value)
- An **alternative** appears in alternates (discriminated by JSON type)

They share identical structure. Create a single generic type (e.g., `QapiVariant<'i>`) and type-alias:
```rust
pub type QapiBranch<'i> = QapiVariant<'i>;
pub type QapiAlternative<'i> = QapiVariant<'i>;
```
This preserves QAPI spec naming while eliminating duplication. The parser implementation can be shared.

### 3c. Macro for Vec newtype boilerplate
`QapiBranches`, `QapiAlternatives`, `QapiFeatures`, `QapiMembers`, `QapiSchema` all implement identical `Deref + DerefMut + IntoIterator`. Create a `vec_newtype!` macro:
```rust
vec_newtype!(QapiBranches, QapiBranch<'i>);
vec_newtype!(QapiAlternatives, QapiAlternative<'i>);
// etc.
```

## Phase 4: IR/Codegen Deduplication (R4)

### 4a. Unify `process_members_or_ref` and `command_process_members_or_ref`
In `process_qapi.rs`, these two functions (~70 lines each) differ only in how `MembersOrRef::Ref` is handled:
- Regular: looks up the struct and inlines its fields
- Command: creates a single flatten field

Unify into one function with an enum parameter:
```rust
enum RefStrategy { InlineFields, Flatten }
fn process_members_or_ref(..., strategy: RefStrategy) -> Vec<StructField> { ... }
```

### 4b. Fix `.expect(format!(...))` pattern
Replace `format!` inside `.expect()` (lines 205-207, 247-249, 335-336) with `.unwrap_or_else(|| panic!(...))` to avoid allocation on the success path.

### 4c. Replace `todo!()` in `add_docs!` macro
Lines in the macro body use `todo!()` for validation errors. Replace with `eprintln!` + skip, or return a `Result` and propagate.

## Phase 5: Cleanup Placeholders & Dead Code (R5)

| Item | File | Action |
|------|------|--------|
| `"NULL_REPLACE_NULL"` | `qapi_ir/mod.rs:36` | Replace with `()` or `Option<()>` — QAPI `null` maps to Rust's unit type |
| `QEMU_VERSIONS` unused at runtime | `versions.rs` | Wire into `find_schema_entry` as a validation lookup, or remove entirely |
| Hardcoded `"5.1"` in preamble | `lib.rs:145` | Remove or make dynamic |
| Dead functions `docstr_to_string`, `docstr_to_string_keep_structure` | `qapi_documentation.rs:46,65` | Remove (compiler warns they're unused) |
| Dead fields in `QapiPragma` | `qapi_pragma.rs:23-27` | Add `#[allow(dead_code)]` with comment explaining they're parsed but not yet used, OR prefix with `_` |

## Phase 6: Eliminate Redundant Schema Parsing (R6)

`lib.rs` currently parses each schema file up to 3 times:
1. In `read_schema` to find includes
2. In `generate_qapi_code` first loop (lines 89-117) to collect types
3. In `generate_qapi_code` second loop (lines 147-188) to generate code

Refactor to parse once and store the AST. The `read_schema` function can return `Vec<(PathBuf, String, QapiSchema)>` and both processing passes can iterate over the pre-parsed schemas.

## Phase 7: Integration Tests (T1)

Create `projects/qapi-spec-parser/tests/integration.rs`:

```rust
#[test]
fn test_parse_current_qemu_schema() {
    // Parse vendor/qemu at current checkout (v9.1.0)
    // Assert no errors, assert expected number of types parsed
}

#[test]
fn test_generate_code_current_qemu() {
    // Run generate_qapi_code against vendor/qemu
    // Assert output is valid Rust (parse with syn)
}
```

The tests will use the current vendor/qemu checkout only (no tag switching for now). They'll reference the vendor submodule path relative to the workspace root.

## Phase 8: Unit Tests for Untested Parsers (T2)

Add `#[cfg(test)] mod tests` to:
- `qapi_schema.rs` — test full schema parsing with a multi-entry input
- `qapi_alternative.rs` — valid/invalid alternative entries
- `qapi_alternatives.rs` — list parsing
- `qapi_branches.rs` — list parsing
- `qapi_members.rs` — member list parsing
- `qapi_enum_value.rs` — simple string + dict enum values

Follow the pattern from `qapi_feature.rs`: test both valid inputs and expected failure cases.

## Phase 9: IR/Codegen Tests (T3)

Add tests in `qapi_ir/`:
- Test `process_enum` with a simple `QapiEnum` input, assert correct `Enum` IR output
- Test `process_struct` with members and base type
- Test `process_command` with returns and features
- Test code generation: parse a small QAPI input -> process to IR -> generate tokens -> prettyplease format -> assert output matches expected Rust code (snapshot test)

## Execution Order

1. **R1** — Idiomatic cleanups (run tests after)
2. **R2** — nom 8 migration (run tests after)
3. **T1** — Integration tests (establishes safety net)
4. **R3** — Parser deduplication (run tests after)
5. **R4** — IR deduplication (run tests after)
6. **R5** — Cleanup placeholders (run tests after)
7. **R6** — Eliminate redundant parsing (run tests after)
8. **T2** — Unit tests for untested parsers
9. **T3** — IR/codegen tests

Tests run after every refactoring phase to ensure nothing breaks.

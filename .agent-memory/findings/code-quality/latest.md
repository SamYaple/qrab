[0m
> build · claude-opus-4-6
[0m
[0m# [0mTodos
[ ] Read .agent-memory/ and project context
[ ] Run cargo clippy and cargo build to capture warnings
[ ] Manual analysis: complexity, duplication, dead code, maintainability
[ ] Auto-fix: remove dead code, fix simple warnings, apply straightforward refactors
[ ] Output findings report
[0m
Let me start by gathering project context and running the automated tools.
[0m→ [0mRead ../..
[0m→ [0mRead ../../../../.claude/projects/-home-bob-workspace-qrab/memory/project_qapi_macros.md
[0m→ [0mRead ../../../../.claude/projects/-home-bob-workspace-qrab/memory/project_manager.md
Good, I have the project context. Now let me run the automated tools and explore the source code.
[0m
[0m$ [0mcd /home/bob/workspace/qrab && cargo clippy -- -W clippy::all -W clippy::complexity 2>&1
Compiling qapi v0.1.0 (/home/bob/workspace/qrab/projects/qapi)
   Compiling qapi-macros v0.1.0 (/home/bob/workspace/qrab/projects/qapi-macros)
    Checking qapi-codegen v0.1.0 (/home/bob/workspace/qrab/projects/qapi-codegen)
warning: this loop could be written as a `while let` loop
   --> projects/qapi-codegen/src/parsers/qapi_documentation.rs:309:17
    |
309 | /                 loop {
310 | |                     if let Ok((n, line)) = take_line_continuation(next) {
311 | |                         field_vals.push(line);
312 | |                         next = n;
...   |
316 | |                 }
    | |_________________^ help: try: `while let Ok((n, line)) = take_line_continuation(next) { .. }`
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#while_let_loop
    = note: `-W clippy::while-let-loop` implied by `-W clippy::complexity`
    = help: to override `-W clippy::complexity` add `#[allow(clippy::while_let_loop)]`

warning: this manual char comparison can be written more succinctly
   --> projects/qapi-codegen/src/parsers/qapi_documentation.rs:499:33
    |
499 |             .trim_start_matches(|c: char| c == '#' || c == ' ')
    |                                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ help: consider using an array of `char`: `['#', ' ']`
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#manual_pattern_char_comparison
    = note: `-W clippy::manual-pattern-char-comparison` implied by `-W clippy::all`
    = help: to override `-W clippy::all` add `#[allow(clippy::manual_pattern_char_comparison)]`

warning: this `impl` can be derived
  --> projects/qapi-codegen/src/parsers/qapi_type_ref.rs:23:1
   |
23 | / impl Default for QapiTypeRef<'_> {
24 | |     fn default() -> Self {
25 | |         Self::Unset
26 | |     }
27 | | }
   | |_^
   |
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#derivable_impls
   = note: `-W clippy::derivable-impls` implied by `-W clippy::complexity`
   = help: to override `-W clippy::complexity` add `#[allow(clippy::derivable_impls)]`
help: replace the manual implementation with a derive attribute and mark the default variant
   |
15 + #[derive(Default)]
16 | pub enum QapiTypeRef<'i> {
17 ~     #[default]
18 ~     Unset,
   |

warning: this `if` statement can be collapsed
   --> projects/qapi-codegen/src/unified.rs:169:9
    |
169 | /         if let Some(ref ref_name) = data_ref_name {
170 | |             if let Some(existing) = structs_lookup.get_mut(ref_name.as_str()) {
171 | |                 // Command references an existing struct — merge command attributes onto it.
172 | |                 for attr in &processed.meta.attributes {
...   |
208 | |         }
    | |_________^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#collapsible_if
    = note: `-W clippy::collapsible-if` implied by `-W clippy::all`
    = help: to override `-W clippy::all` add `#[allow(clippy::collapsible_if)]`
help: collapse nested if block
    |
169 ~         if let Some(ref ref_name) = data_ref_name
170 ~             && let Some(existing) = structs_lookup.get_mut(ref_name.as_str()) {
171 |                 // Command references an existing struct — merge command attributes onto it.
...
206 |                 continue;
207 ~             }
    |

warning: this `if` statement can be collapsed
    --> projects/qapi-codegen/src/unified.rs:1220:9
     |
1220 | /         if let Some(uvr) = us.attr_versions.get("unstable") {
1221 | |             if let Some(until) = uvr.until {
1222 | |                 inner.push(Attribute::with_value("removed", version_str(until)));
1223 | |             }
1224 | |         }
     | |_________^
     |
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#collapsible_if
help: collapse nested if block
     |
1220 ~         if let Some(uvr) = us.attr_versions.get("unstable")
1221 ~             && let Some(until) = uvr.until {
1222 |                 inner.push(Attribute::with_value("removed", version_str(until)));
1223 ~             }
     |

warning: `qapi-codegen` (lib) generated 5 warnings (run `cargo clippy --fix --lib -p qapi-codegen` to apply 4 suggestions)
warning: large size difference between variants
     --> projects/qapi/src/generated.rs:14552:1
      |
14552 | / pub enum ObjectOptionsBranch {
14553 | |     #[qapi(name = "acpi-generic-initiator")]
14554 | |     #[qapi(added = "9.0.0")]
14555 | |     AcpiGenericInitiator(AcpiGenericInitiatorProperties),
...     |
14648 | |     TdxGuest(TdxGuestProperties),
      | |     ---------------------------- the second-largest variant contains at least 192 bytes
...     |
14653 | |     ThrottleGroup(ThrottleGroupProperties),
      | |     -------------------------------------- the largest variant contains at least 608 bytes
...     |
14666 | |     XVfioUserServer(VfioUserServerProperties),
14667 | | }
      | |_^ the entire enum is at least 608 bytes
      |
      = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#large_enum_variant
      = note: `-W clippy::large-enum-variant` implied by `-W clippy::all`
      = help: to override `-W clippy::all` add `#[allow(clippy::large_enum_variant)]`
help: consider boxing the large fields or introducing indirection in some other way to reduce the total size of the enum
      |
14653 -     ThrottleGroup(ThrottleGroupProperties),
14653 +     ThrottleGroup(Box<ThrottleGroupProperties>),
      |

warning: `qapi` (lib) generated 1 warning
    Checking qapi-protocol v0.1.0 (/home/bob/workspace/qrab/projects/qapi-protocol)
    Checking manager v0.1.0 (/home/bob/workspace/qrab/projects/manager)
warning: enclosing `Ok` and `?` operator are unneeded
   --> projects/manager/src/vm.rs:291:9
    |
291 |         Ok(self.qmp.execute(cmd).await?)
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#needless_question_mark
    = note: `-W clippy::needless-question-mark` implied by `-W clippy::complexity`
    = help: to override `-W clippy::complexity` add `#[allow(clippy::needless_question_mark)]`
help: remove the enclosing `Ok` and `?` operator
    |
291 -         Ok(self.qmp.execute(cmd).await?)
291 +         self.qmp.execute(cmd).await
    |

warning: enclosing `Ok` and `?` operator are unneeded
   --> projects/manager/src/vm.rs:337:9
    |
337 |         Ok(self.qmp.execute(cmd).await?)
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#needless_question_mark
help: remove the enclosing `Ok` and `?` operator
    |
337 -         Ok(self.qmp.execute(cmd).await?)
337 +         self.qmp.execute(cmd).await
    |

warning: `manager` (lib) generated 2 warnings (run `cargo clippy --fix --lib -p manager` to apply 2 suggestions)
    Checking events v0.1.0 (/home/bob/workspace/qrab/examples/events)
    Checking preconfig-vm v0.1.0 (/home/bob/workspace/qrab/examples/preconfig-vm)
    Checking simple-vm v0.1.0 (/home/bob/workspace/qrab/examples/simple-vm)
warning: this loop could be written as a `while let` loop
  --> examples/events/src/main.rs:53:5
   |
53 | /     loop {
54 | |         match tokio::time::timeout_at(deadline, vm.events.recv()).await {
55 | |             Ok(Some(evt)) => {
56 | |                 println!("    {evt:?}");
...  |
60 | |     }
   | |_____^ help: try: `while let Ok(Some(evt)) = tokio::time::timeout_at(deadline, vm.events.recv()).await { .. }`
   |
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#while_let_loop
   = note: `-W clippy::while-let-loop` implied by `-W clippy::complexity`
   = help: to override `-W clippy::complexity` add `#[allow(clippy::while_let_loop)]`

warning: `events` (bin "events") generated 1 warning
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 14.70s
[0m
[0m$ [0mcd /home/bob/workspace/qrab && cargo build 2>&1
Blocking waiting for file lock on build directory
   Compiling qapi-codegen v0.1.0 (/home/bob/workspace/qrab/projects/qapi-codegen)
   Compiling preconfig-vm v0.1.0 (/home/bob/workspace/qrab/examples/preconfig-vm)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 15.00s
[0m
Good results. Let me now do the manual analysis by exploring the source code.
[0m# [0mTodos
[x] Read .agent-memory/ and project context
[x] Run cargo clippy and cargo build to capture warnings
[ ] Manual analysis: complexity, duplication, dead code, maintainability
[ ] Auto-fix: remove dead code, fix simple warnings, apply straightforward refactors
[ ] Output findings report
[0m
Now let me explore the source files for manual analysis. I'll check module sizes and look at the key files.
[0m$ [0mcd /home/bob/workspace/qrab && find projects examples -name '*.rs' -exec wc -l {} + | sort -rn | head -30
36139 total
  22294 projects/qapi/src/generated.rs
   1465 projects/qapi-codegen/src/unified.rs
   1343 projects/qapi-macros/src/lib.rs
   1255 projects/qapi/src/qga.rs
    913 projects/qapi-protocol/tests/minimal.rs
    911 projects/qapi-codegen/src/qapi_ir/process_qapi.rs
    614 projects/qapi-codegen/src/parsers/qapi_documentation.rs
    434 projects/qapi-codegen/tests/integration.rs
    377 projects/manager/src/vm.rs
    320 projects/qapi-protocol/tests/common/fixtures.rs
    300 projects/qapi-protocol/tests/block.rs
    269 projects/qapi-codegen/src/parsers/qapi_feature.rs
    267 projects/qapi-protocol/tests/schema.rs
    249 projects/qapi-protocol/src/lib.rs
    232 examples/preconfig-vm/src/main.rs
    227 projects/qapi-codegen/src/parsers/qapi_cond.rs
    220 projects/qapi/tests/serialization.rs
    218 projects/qapi-codegen/src/qapi_ir/mod.rs
    216 projects/qapi-protocol/src/server.rs
    191 projects/qapi-codegen/src/parsers/qapi_schema.rs
    182 projects/qapi-codegen/src/parsers/qapi_union.rs
    181 projects/qapi-protocol/tests/boot.rs
    179 projects/qapi-protocol/tests/events.rs
    172 projects/qapi-codegen/src/parsers/qapi_pragma.rs
    170 projects/qapi-protocol/tests/mock_server.rs
    163 projects/qapi-codegen/src/parsers/qapi_features.rs
    147 projects/manager/tests/integration.rs
    143 projects/qapi-codegen/src/versions.rs
    143 projects/qapi-codegen/src/parsers/qapi_struct.rs
[0m
[0m$ [0mcd /home/bob/workspace/qrab && find projects examples -name '*.rs' ! -path '*/generated*' -exec grep -l 'fn ' {} + | xargs -I{} sh -c 'echo "=== {} ===" && grep -n "^\s*\(pub \)\?\(async \)\?fn " {} | head -30'
=== projects/qapi-macros/src/lib.rs ===
22:    fn bare() -> Self {
67:fn get_version_bounds() -> VersionBounds {
89:fn excluded_by_bounds(bounds: &VersionBounds, since: Option<&str>, until: Option<&str>) -> bool {
109:fn rust_deprecated_by_bounds(bounds: &VersionBounds, deprecated: Option<&str>) -> bool {
126:fn parse_attr_range(
149:fn parse_type_attrs(args: &[NestedMeta]) -> TypeAttrs {
220:fn parse_item_attrs(meta_list: &MetaList) -> ItemAttrs {
257:fn extract_all_item_attrs(syn_attrs: &[syn::Attribute]) -> ItemAttrs {
280:fn wire_name(attrs_name: &Option<String>, ident: &syn::Ident) -> String {
286:fn parse_version(version: &str) -> (u32, u32, u32) {
294:fn version_check(version: &str) -> TokenStream2 {
299:fn version_range_check(since: Option<&str>, until: Option<&str>) -> Option<TokenStream2> {
316:fn deprecation_warning(
346:fn removed_field_warning(type_name: &str, field_name: &str, removed_version: &str) -> TokenStream2 {
362:fn condition_cfg(condition: &str) -> TokenStream2 {
374:fn condition_cfg_attr(condition: &str) -> Option<syn::Attribute> {
383:fn is_option_type(ty: &Type) -> bool {
409:pub fn qapi(args: TokenStream, input: TokenStream) -> TokenStream {
490:fn gen_enum(input_ast: &mut DeriveInput, ta: &TypeAttrs, bounds: &VersionBounds) -> TokenStream2 {
588:                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
608:fn gen_unit_enum_ser(name: &syn::Ident, variants: &[VariantInfo], ta: &TypeAttrs) -> TokenStream2 {
649:            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
657:fn gen_unit_enum_de(name: &syn::Ident, variants: &[VariantInfo]) -> TokenStream2 {
679:            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
691:fn gen_alt_ser(name: &syn::Ident, variants: &[VariantInfo]) -> TokenStream2 {
708:            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
716:fn gen_alt_de(name: &syn::Ident, variants: &[VariantInfo]) -> TokenStream2 {
746:            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
758:fn gen_struct(input_ast: &mut DeriveInput, ta: &TypeAttrs, bounds: &VersionBounds) -> TokenStream2 {
889:                    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
=== projects/qapi/src/lib.rs ===
42:pub fn set_qemu_version(major: u32, minor: u32, patch: u32) {
47:pub fn qemu_version() -> (u32, u32, u32) {
60:pub fn set_qga_version(version: String) {
65:pub fn qga_version() -> &'static str {
=== projects/qapi/tests/serialization.rs ===
4:fn init_version() {
9:fn unit_enum_serialize() {
21:fn unit_enum_deserialize() {
31:fn unit_enum_roundtrip() {
42:fn enum_with_rename() {
53:fn enum_leading_digit_variant() {
64:fn alternate_string_roundtrip() {
78:fn alternate_null_roundtrip() {
89:fn struct_serialize_with_rename() {
100:fn struct_deserialize_with_rename() {
108:fn struct_roundtrip() {
122:fn command_trait_query_status() {
127:    fn _check_returns(_: <QueryStatus as QapiCommand>::Returns) {}
128:    fn _check_is_status_info(_: StatusInfo) {}
132:fn command_trait_yank() {
140:fn event_trait() {
147:fn command_serialize() {
163:fn event_serialize() {
176:fn event_deserialize() {
185:fn version_gating_rejects_old_version() {
192:fn union_struct_serialize() {
206:fn error_class_roundtrip() {
216:fn command_version_info() {
=== projects/qapi/build.rs ===
1:fn main() {
=== projects/qapi-codegen/src/parsers/mod.rs ===
33:pub fn set_parser_version(v: (u32, u32, u32)) {
38:pub fn parser_version() -> (u32, u32, u32) {
47:            fn deref(&self) -> &Self::Target {
52:            fn deref_mut(&mut self) -> &mut Self::Target {
59:            fn into_iter(self) -> Self::IntoIter {
=== projects/qapi-codegen/src/parsers/qapi_alternative.rs ===
6:pub fn take_alternative(input: &str) -> IResult<&str, QapiAlternative<'_>> {
=== projects/qapi-codegen/src/parsers/qapi_branch.rs ===
6:pub fn take_branch(input: &str) -> IResult<&str, QapiBranch<'_>> {
22:    fn test_valid() {
=== projects/qapi-codegen/src/parsers/common.rs ===
13:pub fn qcomment(input: &str) -> IResult<&str, &str> {
22:pub fn clean_lines(input: &str) -> IResult<&str, &str> {
30:pub fn qtag<'i>(t: &'static str) -> impl Parser<&'i str, Output = &'i str, Error = Error<&'i str>> {
34:pub fn qstring(input: &str) -> IResult<&str, &str> {
38:pub fn qbool(input: &str) -> IResult<&str, &str> {
45:pub fn take_list<'i, I, O>(
57:pub fn take_dict<'i, I, O>(
70:fn separated_list_flexible<'i, I, O>(
116:pub fn take_kv<'i, I, O>(
=== projects/qapi-codegen/src/parsers/qapi_alternate.rs ===
15:pub fn take_alternate(input: &str) -> IResult<&str, QapiAlternate<'_>> {
33:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
37:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
71:    fn test_valid() {
=== projects/qapi-codegen/src/parsers/qapi_alternatives.rs ===
8:pub fn take_alternatives(input: &str) -> IResult<&str, QapiAlternatives<'_>> {
16:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
20:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
32:    fn test_empty_alternatives() {
39:    fn test_single_alternative() {
48:    fn test_multiple_alternatives() {
57:    fn test_alternative_with_condition() {
66:    fn test_invalid_input() {
=== projects/qapi-codegen/src/parsers/qapi_branches.rs ===
7:pub fn take_branches(input: &str) -> IResult<&str, QapiBranches<'_>> {
15:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
19:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
31:    fn test_empty_branches() {
38:    fn test_single_branch() {
47:    fn test_multiple_branches() {
56:    fn test_branch_with_condition() {
65:    fn test_invalid_input() {
=== projects/qapi-codegen/src/parsers/qapi_command.rs ===
14:pub fn take_command(input: &str) -> IResult<&str, QapiCommand<'_>> {
35:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
45:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
50:    pub fn parse_v2(input: &'i str) -> IResult<&'i str, Self> {
54:    fn parse_common(input: &'i str, gen_string_as_bool: bool) -> IResult<&'i str, Self> {
102:    fn test_valid_v2() {
113:    fn test_gen_string_accepted_in_v1() {
123:    fn test_gen_string_rejected_in_v2() {
131:    fn test_gen_bool_accepted_in_v2() {
=== projects/qapi-codegen/src/parsers/qapi_cond.rs ===
13:pub fn take_cond(input: &str) -> IResult<&str, QapiCond<'_>> {
32:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
36:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
57:    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
96:    fn test_parse_with_whitespace() {
114:    fn test_parse_invalid_inputs() {
133:    fn test_parse_config_name() {
142:    fn test_parse_not_condition() {
151:    fn test_parse_any_condition_single() {
160:    fn test_parse_any_condition_multiple() {
172:    fn test_parse_all_condition() {
184:    fn test_display_config_name() {
190:    fn test_display_not_condition() {
196:    fn test_display_any_condition() {
205:    fn test_display_all_condition() {
214:    fn test_display_complex_nested_conditions() {
=== projects/qapi-codegen/src/parsers/qapi_documentation.rs ===
13:fn not_name_break(c: char) -> bool {
17:fn take_line_start(input: &str) -> IResult<&str, &str> {
22:fn take_line_end(input: &str) -> IResult<&str, &str> {
26:fn take_empty_line(input: &str) -> IResult<&str, &str> {
31:fn take_empty_line_ws(input: &str) -> IResult<&str, &str> {
36:fn take_name_ws(input: &str) -> IResult<&str, &str> {
44:fn take_value(input: &str) -> IResult<&str, Vec<&str>> {
52:fn take_line_continuation(input: &str) -> IResult<&str, &str> {
74:fn take_multiline_text(input: &str) -> IResult<&str, &str> {
78:fn is_since_without_colon(input: &str) -> IResult<&str, &str> {
86:fn take_name(input: &str) -> IResult<&str, &str> {
94:fn take_name_optional_colon(input: &str) -> IResult<&str, &str> {
98:fn take_namedkey<'i>(
107:pub fn extract_since_from_comment(desc: &Vec<&str>) -> Option<String> {
144:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
157:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
216:    pub fn parse_v2(input: &'i str) -> IResult<&'i str, Self> {
249:    pub fn parse_v3(input: &'i str) -> IResult<&'i str, Self> {
283:    fn parse_body(mut input: &'i str, doc: &mut Self) -> Result<&'i str, nom::Err<Error<&'i str>>> {
394:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
406:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
411:    pub fn parse_v2(input: &'i str) -> IResult<&'i str, Self> {
416:    pub fn parse_v3(input: &'i str) -> IResult<&'i str, Self> {
420:    fn parse_common(input: &'i str, _allow_leading_ws: bool) -> IResult<&'i str, Self> {
458:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
470:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
475:    pub fn parse_v2(input: &'i str) -> IResult<&'i str, Self> {
480:    pub fn parse_v3(input: &'i str) -> IResult<&'i str, Self> {
484:    fn parse_common(input: &'i str, _allow_leading_ws: bool) -> IResult<&'i str, Self> {
537:    fn test_doc_v3_basic() {
=== projects/qapi-codegen/src/parsers/qapi_enum.rs ===
15:pub fn take_enum(input: &str) -> IResult<&str, QapiEnum<'_>> {
35:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
39:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
70:    fn test_valid() {
=== projects/qapi-codegen/src/parsers/qapi_enum_value.rs ===
11:pub fn take_enum_value(input: &str) -> IResult<&str, QapiEnumValue<'_>> {
27:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
31:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
41:    fn simple_parser(input: &'i str) -> IResult<&'i str, Self> {
50:    fn complex_parser(input: &'i str) -> IResult<&'i str, Self> {
67:    fn test_simple_string() {
76:    fn test_dict_name_only() {
85:    fn test_dict_with_condition() {
94:    fn test_dict_with_features() {
103:    fn test_dict_with_all_fields() {
113:    fn test_empty_name_fails() {
118:    fn test_missing_name_in_dict_fails() {
123:    fn test_invalid_input_fails() {
128:    fn test_unknown_field_fails() {
=== projects/qapi-codegen/src/parsers/qapi_event.rs ===
15:pub fn take_event(input: &str) -> IResult<&str, QapiEvent<'_>> {
39:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
43:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
80:    fn test_valid() {
=== projects/qapi-codegen/src/parsers/qapi_feature.rs ===
9:pub fn take_feature(input: &str) -> IResult<&str, QapiFeature<'_>> {
24:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
28:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
38:    fn string_parser(input: &'i str) -> IResult<&'i str, Self> {
45:    fn dict_parser(input: &'i str) -> IResult<&'i str, Self> {
59:    fn test_parse_string_feature() {
71:    fn test_parse_dict_feature_minimal() {
83:    fn test_parse_dict_feature_with_if() {
95:    fn test_parse_dict_feature_with_complex_if() {
109:    fn test_parse_dict_feature_with_any_if() {
124:    fn test_parse_dict_feature_with_all_if() {
139:    fn test_parse_feature_with_nested_conditions() {
155:    fn test_parse_feature_with_whitespace() {
167:    fn test_parse_invalid_missing_name() {
177:    fn test_parse_invalid_unknown_field() {
187:    fn test_parse_invalid_unexpected_format() {
197:    fn test_parse_invalid_incorrect_quotes() {
214:    fn test_parse_invalid_missing_quotes() {
230:    fn test_parse_trailing_comma_accepted() {
246:    fn test_parse_invalid_missing_wrapping_quotes() {
261:    fn test_parse_invalid_extra_fields() {
=== projects/qapi-codegen/src/parsers/qapi_features.rs ===
8:pub fn take_features(input: &str) -> IResult<&str, QapiFeatures<'_>> {
18:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
22:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
34:    fn test_parse_features_single_dict() {
46:    fn test_parse_features_single_string() {
58:    fn test_parse_features_with_if() {
70:    fn test_parse_features_mixed() {
88:    fn test_parse_feature_dict() {
100:    fn test_parse_feature_string() {
112:    fn test_parse_feature_with_if() {
124:    fn test_parse_features_invalid() {
145:    fn test_parse_feature_invalid() {
=== projects/qapi-codegen/src/parsers/qapi_include.rs ===
6:pub fn take_include(input: &str) -> IResult<&str, QapiInclude<'_>> {
14:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
18:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
37:    fn test_valid() {
=== projects/qapi-codegen/src/parsers/qapi_member.rs ===
14:pub fn take_member(input: &str) -> IResult<&str, QapiMember<'_>> {
32:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
36:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
50:    fn simple_parser(input: &'i str) -> IResult<&'i str, Self> {
61:    fn complex_parser(input: &'i str) -> IResult<&'i str, Self> {
89:    fn test_valid() {
=== projects/qapi-codegen/src/parsers/qapi_members.rs ===
8:pub fn take_members(input: &str) -> IResult<&str, QapiMembers<'_>> {
16:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
20:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
26:pub fn take_members_or_ref(input: &str) -> IResult<&str, MembersOrRef<'_>> {
40:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
44:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
55:    fn test_empty_members() {
62:    fn test_single_member() {
71:    fn test_multiple_members() {
83:    fn test_members_or_ref_string() {
90:    fn test_members_or_ref_members() {
100:    fn test_invalid_members() {
=== projects/qapi-codegen/src/parsers/qapi_pragma.rs ===
9:pub fn take_pragma(input: &str) -> IResult<&str, QapiPragma<'_>> {
31:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
41:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
46:    pub fn parse_v2(input: &'i str) -> IResult<&'i str, Self> {
50:    fn parse_common(input: &'i str, old_keys: bool) -> IResult<&'i str, Self> {
107:    fn from_tokens(tokens: Vec<ParserToken<'i>>) -> Self {
138:    fn test_valid_v2() {
150:    fn test_old_keys_accepted_in_v1() {
158:    fn test_old_keys_rejected_in_v2() {
166:    fn test_doc_required() {
=== projects/qapi-codegen/src/parsers/qapi_schema.rs ===
22:pub fn take_schema(input: &str) -> IResult<&str, QapiSchema<'_>> {
49:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
53:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
77:fn bare_doc_separator(input: &str) -> IResult<&str, &str> {
88:    fn test_empty_input() {
95:    fn test_single_enum() {
103:    fn test_single_struct() {
111:    fn test_single_command() {
119:    fn test_include() {
127:    fn test_comment_and_whitespace() {
138:    fn test_multi_entry_schema() {
161:    fn test_freeform_doc_direct() {
174:    fn test_freeform_doc_in_schema() {
=== projects/qapi-codegen/src/parsers/qapi_struct.rs ===
13:pub fn take_struct(input: &str) -> IResult<&str, QapiStruct<'_>> {
29:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
39:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
62:    pub fn parse_v2(input: &'i str) -> IResult<&'i str, Self> {
97:    fn test_valid_v2() {
109:    fn test_type_keyword_accepted_in_v1() {
123:    fn test_struct_keyword_also_accepted_in_v1() {
137:    fn test_type_keyword_rejected_in_v2() {
=== projects/qapi-codegen/src/parsers/qapi_type_ref.rs ===
9:pub fn take_type_ref(input: &str) -> IResult<&str, QapiTypeRef<'_>> {
24:    fn default() -> Self {
31:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
41:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
53:    pub fn parse_v2(input: &'i str) -> IResult<&'i str, Self> {
65:    fn test_string_ref() {
73:    fn test_array_ref() {
81:    fn test_inline_struct_not_parsed_in_v2() {
89:    fn test_inline_struct_parsed_in_v1() {
=== projects/qapi-codegen/src/parsers/qapi_union.rs ===
15:pub fn take_union(input: &str) -> IResult<&str, QapiUnion<'_>> {
32:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
44:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
73:    pub fn parse_v2(input: &'i str) -> IResult<&'i str, Self> {
103:    pub fn parse_v3(input: &'i str) -> IResult<&'i str, Self> {
136:    fn test_flat_union_v3() {
146:    fn test_simple_union_rejected_in_v3() {
154:    fn test_simple_union_accepted_in_v2() {
165:    fn test_empty_discriminator_v1() {
175:    fn test_flat_union_accepted_in_v2() {
=== projects/qapi-codegen/src/parsers/qapi_variant.rs ===
12:pub fn take_variant(input: &str) -> IResult<&str, QapiVariant<'_>> {
21:    fn test_simple_string_type() {
30:    fn test_simple_array_type() {
38:    fn test_complex_with_condition() {
48:    fn test_complex_array_with_condition() {
58:    fn test_whitespace_tolerance() {
66:    fn test_missing_name_fails() {
71:    fn test_missing_type_fails() {
76:    fn test_invalid_format() {
93:    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
97:    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
107:    fn simple_parser(input: &'i str) -> IResult<&'i str, Self> {
116:    fn complex_parser(input: &'i str) -> IResult<&'i str, Self> {
=== projects/qapi-codegen/src/qapi_ir/mod.rs ===
20:fn qapi_to_rust_type(qapi_type: &str) -> String {
46:fn fix_leading_digit(input: &str) -> String {
68:fn rustify_field_name(input: &str) -> String {
73:fn rustify_type(input: &str) -> String {
88:pub fn generate_attribute(attribute: &Attribute) -> TokenStream {
101:pub fn generate_options(attribute: &Attribute) -> Vec<TokenStream> {
136:    pub fn new<N: ToString>(name: N) -> Self {
140:    pub fn with_value<N: ToString, V: ToString>(name: N, value: V) -> Self {
147:    pub fn with_values<N: ToString, V: ToString>(values: Vec<(N, Option<V>)>) -> Self {
166:    pub fn nested<N: ToString>(name: N, inner: Vec<Attribute>) -> Self {
183:pub fn doc_to_tokens(doc: &str) -> proc_macro2::TokenStream {
=== projects/qapi-codegen/src/qapi_ir/process_qapi.rs ===
20:fn normalize_since_version(s: &str) -> String {
110:fn field_name_attr(name: &str) -> Option<Attribute> {
119:fn name_attr(name: &str) -> Option<Attribute> {
128:fn process_type_ref<'a>(q: &'a QapiTypeRef<'a>) -> (&'a str, bool) {
141:fn process_alternative(q: QapiAlternative) -> EnumVariant {
161:pub fn process_alternate(q: QapiAlternate) -> Enum {
186:fn process_enum_value(q: QapiEnumValue) -> EnumVariant {
205:pub fn process_enum(q: QapiEnum) -> Enum {
231:pub fn process_member(q: QapiMember) -> StructField {
255:fn process_member_with_inline(q: QapiMember, parent_name: &str) -> (StructField, Option<Struct>) {
292:pub fn process_struct(
332:pub fn process_union(
461:fn process_members_or_ref(q: MembersOrRef, strategy: RefStrategy<'_>) -> Vec<StructField> {
493:pub fn process_event(q: QapiEvent, structs_lookup: &HashMap<String, Struct>) -> Struct {
526:    fn make_enum_value(name: &str) -> QapiEnumValue<'_> {
535:    fn test_process_enum_simple() {
553:    fn test_process_enum_with_features() {
582:    fn test_process_struct_simple() {
606:    fn test_process_struct_with_base() {
646:    fn test_process_member_optional_and_array() {
662:    fn test_process_struct_with_inline_struct() {
728:    fn test_process_command_simple() {
754:    fn test_process_command_with_flags() {
785:    fn test_process_alternate() {
812:    fn test_enum_codegen_roundtrip() {
832:    fn test_struct_codegen_roundtrip() {
867:pub fn process_command(q: QapiCommand) -> Struct {
=== projects/qapi-codegen/src/qapi_ir/rust_enum.rs ===
25:    fn cmp(&self, other: &Self) -> Ordering {
31:    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
44:    fn cmp(&self, other: &Self) -> Ordering {
50:    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
56:    pub fn generate(&self) -> TokenStream {
=== projects/qapi-codegen/src/qapi_ir/rust_struct.rs ===
16:    fn cmp(&self, other: &Self) -> Ordering {
22:    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
35:    fn cmp(&self, other: &Self) -> Ordering {
41:    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
46:    pub fn generate(&self) -> TokenStream {
=== projects/qapi-codegen/src/lib.rs ===
41:pub fn parse_schema(input: &str) -> Result<QapiSchema<'_>> {
54:pub fn read_schema_with_version(
66:fn read_schema_file(
=== projects/qapi-codegen/src/main.rs ===
5:fn main() -> Result<()> {
=== projects/qapi-codegen/src/versions.rs ===
6:pub fn find_schema_entry(qemu_root: &Path) -> Option<PathBuf> {
19:pub fn parse_version_tag(tag: &str) -> Option<(u32, u32, u32)> {
35:pub fn extract_schema_from_tag(qemu_repo: &Path, tag: &str) -> Result<tempfile::TempDir> {
99:pub fn find_extracted_schema_entry(dir: &Path) -> Option<PathBuf> {
112:pub fn find_extracted_qga_schema_entry(dir: &Path) -> Option<PathBuf> {
122:pub fn discover_version_tags(qemu_repo: &Path) -> Result<Vec<String>> {
=== projects/qapi-codegen/src/unified.rs ===
19:fn version_str(v: Version) -> String {
31:fn clean_doc(doc: &str) -> Option<String> {
48:    pub fn new(since: Version) -> Self {
94:fn process_version_schema(
228:fn process_single_version(qemu_repo: &Path, tag: &str) -> Result<VersionResult> {
268:fn process_single_version_qga(qemu_repo: &Path, tag: &str) -> Result<Option<VersionResult>> {
310:pub fn build_unified_registry(qemu_repo: &Path, version_tags: &[&str]) -> Result<UnifiedRegistry> {
485:fn has_deprecated_feature(meta: &Metadata) -> bool {
497:fn has_unstable_feature(meta: &Metadata) -> bool {
512:fn extract_bool_attr_versions(meta: &Metadata, version: Version) -> BTreeMap<String, VersionRange> {
528:fn merge_bool_attrs(existing: &mut UnifiedStruct, new_meta: &Metadata, version: Version) {
559:fn variant_rust_name(v: &EnumVariant) -> String {
563:fn merge_enum(registry: &mut UnifiedRegistry, name: &str, e: &Enum, version: Version) {
679:fn field_rust_name(f: &StructField) -> String {
683:fn merge_struct(registry: &mut UnifiedRegistry, name: &str, s: &Struct, version: Version) {
843:fn fix_recursive_types(registry: &mut UnifiedRegistry) {
897:fn is_reachable(
920:pub fn generate_unified_code(qemu_repo: &Path, version_tags: &[&str]) -> Result<String> {
1010:fn separate_items(code: &str) -> String {
1027:fn generate_unified_enum(
1118:fn generate_unified_struct(
1310:pub fn build_qga_registry(qemu_repo: &Path, version_tags: &[&str]) -> Result<UnifiedRegistry> {
1404:pub fn generate_qga_code(qemu_repo: &Path, version_tags: &[&str]) -> Result<String> {
=== projects/qapi-codegen/tests/integration.rs ===
10:fn qemu_root() -> &'static Path {
21:fn representative_tags() -> Vec<String> {
34:fn test_find_schema_entry() {
42:fn test_parse_all_schema_files() {
58:fn test_parse_schema_returns_types() {
77:fn test_discover_version_tags() {
114:fn test_parse_all_discovered_versions() {
149:fn try_parse_version(tag: &str, repo: &Path) -> Result<usize, String> {
173:fn test_build_unified_registry() {
209:fn test_unified_codegen_produces_valid_rust() {
239:fn test_unified_codegen_has_doc_comments() {
256:fn test_spice_types_from_v2_1() {
277:fn test_era_a_v1_1_type_keyword() {
310:fn test_era_b_v2_1_includes_and_events() {
331:fn test_era_c_v2_5_struct_and_alternate() {
348:fn test_era_d_v4_0_conditions_and_features() {
374:fn test_era_e_v10_2_modern_format() {
413:fn test_doc_preservation_in_ir() {
=== projects/qapi-protocol/src/lib.rs ===
45:    pub async fn new(socket_path: &Path) -> Result<(Self, mpsc::Receiver<QmpEvent>)> {
123:    pub async fn execute<C: QapiCommand>(&self, cmd: &C) -> Result<C::Returns> {
169:    pub async fn shutdown(&mut self) {
182:    async fn read_loop(
209:    async fn write_loop(
233:    async fn event_worker(
=== projects/qapi-protocol/src/qga.rs ===
20:    pub async fn new(socket_path: &Path) -> Result<Self> {
42:    async fn sync(&mut self) -> Result<()> {
90:    pub async fn execute<C: QapiCommand>(&mut self, cmd: &C) -> Result<C::Returns> {
96:    async fn execute_raw<T: serde::de::DeserializeOwned>(
=== projects/qapi-protocol/src/server.rs ===
17:    pub async fn accept(listener: &UnixListener) -> Result<Self> {
27:    pub fn from_stream(stream: UnixStream) -> Self {
36:    pub async fn send_greeting(&mut self, major: u32, minor: u32, micro: u32) -> Result<()> {
55:    pub async fn negotiate(&mut self) -> Result<()> {
70:    pub async fn recv_command(&mut self) -> Result<(String, serde_json::Value)> {
89:    pub async fn send_return(&mut self, value: serde_json::Value) -> Result<()> {
100:    pub async fn send_error(&mut self, class: &str, desc: &str) -> Result<()> {
111:    pub async fn send_event(&mut self, name: &str, data: serde_json::Value) -> Result<()> {
134:    pub async fn accept(listener: &UnixListener) -> Result<Self> {
145:    pub async fn handle_sync(&mut self) -> Result<i64> {
177:    pub async fn recv_command(&mut self) -> Result<(String, serde_json::Value)> {
196:    pub async fn send_return(&mut self, value: serde_json::Value) -> Result<()> {
207:    pub async fn send_error(&mut self, class: &str, desc: &str) -> Result<()> {
=== projects/qapi-protocol/tests/block.rs ===
18:async fn query_block_empty() {
60:async fn blockdev_add_via_cli_and_query() {
129:async fn blockdev_del_typed() {
176:async fn drive_arg_queries() {
234:async fn block_roundtrip_after_boot() {
=== projects/qapi-protocol/tests/boot.rs ===
16:async fn cirros_builder(arch: Arch) -> VmBuilder {
=== projects/qapi-protocol/tests/common/fixtures.rs ===
8:fn fixtures_dir() -> PathBuf {
22:pub async fn cirros_image(arch: Arch) -> PathBuf {
60:pub async fn cirros_image_copy(arch: Arch) -> PathBuf {
76:pub async fn cirros_kernel(arch: Arch) -> PathBuf {
86:pub async fn cirros_initramfs(arch: Arch) -> PathBuf {
95:async fn download_cirros_file(filename: &str) -> PathBuf {
119:pub async fn nocloud_cidata() -> PathBuf {
160:fn add_files_to_fat(img_path: &std::path::Path, files: &[(&str, &str)]) {
237:pub async fn blank_qcow2(name: &str, size_mb: u64) -> PathBuf {
266:pub fn find_firmware(arch: Arch) -> Option<PathBuf> {
312:pub fn firmware_code(arch: Arch) -> PathBuf {
=== projects/qapi-protocol/tests/common/mod.rs ===
15:pub async fn wait_for_event(
32:pub async fn wait_for_typed_event<T: qapi::QapiEvent + serde::de::DeserializeOwned>(
46:pub fn event_timeout() -> Duration {
55:pub fn boot_timeout() -> Duration {
64:pub fn ensure_version_set() {
77:                async fn [<$name _ $arch:lower>]() {
98:                async fn [<$name _ $arch:lower>]() {
=== projects/qapi-protocol/tests/common/serial.rs ===
14:    pub fn new(log_path: PathBuf) -> Self {
19:    pub async fn wait_for(&self, pattern: &str, timeout: Duration) -> Result<String> {
48:    pub async fn wait_for_boot(&self, timeout: Duration) -> Result<()> {
54:    pub fn path(&self) -> &Path {
=== projects/qapi-protocol/tests/mock_server.rs ===
10:fn unique_socket() -> String {
18:async fn mock_qmp_negotiate() {
54:async fn mock_qmp_error_response() {
86:async fn mock_qmp_event() {
131:async fn mock_qga_sync_and_ping() {
=== projects/qapi-protocol/tests/schema.rs ===
14:fn command_trait_metadata() {
44:fn event_trait_metadata() {
60:fn enum_run_state_all_variants() {
93:fn enum_on_off_auto() {
108:fn enum_io_operation_type() {
121:fn enum_error_class() {
130:fn enum_migration_capability_all_variants() {
170:fn enum_dump_guest_memory_format() {
193:fn alternate_str_or_null() {
211:fn struct_field_renames() {
=== projects/manager/src/arch.rs ===
13:    pub fn all() -> &'static [Arch] {
19:    pub fn binary(&self) -> PathBuf {
32:    fn default_binary_name(&self) -> &'static str {
42:    pub fn is_available(&self) -> bool {
47:    pub fn bios_machine(&self) -> Option<&'static str> {
55:    pub fn uefi_machine(&self) -> &'static str {
63:    pub fn cpu(&self) -> Option<&'static str> {
72:    pub fn label(&self) -> &'static str {
83:    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
99:pub fn detect_version(arch: Arch) -> Option<(u32, u32, u32)> {
=== projects/manager/src/vm.rs ===
34:    async fn cleanup(&mut self) {
48:    fn drop(&mut self) {
81:    pub fn new(arch: Arch, config: MachineConfig) -> Self {
96:    pub fn kernel(mut self, kernel: PathBuf, initrd: PathBuf, append: &str) -> Self {
104:    pub fn serial(mut self) -> Self {
110:    pub fn no_shutdown(mut self) -> Self {
116:    pub fn firmware(mut self, path: PathBuf) -> Self {
122:    pub fn arg(mut self, a: impl Into<String>) -> Self {
132:    pub async fn spawn(self) -> Result<PreconfigVm> {
278:    pub fn serial_log(&self) -> Option<&PathBuf> {
287:    pub async fn execute<C: QapiCommand + qapi::PreconfigSafe>(
298:    pub async fn start(self) -> Result<Vm> {
308:    pub async fn shutdown(mut self) {
331:    pub fn serial_log(&self) -> Option<&PathBuf> {
336:    pub async fn execute<C: QapiCommand>(&self, cmd: &C) -> Result<C::Returns> {
343:    pub async fn stop(&self) -> Result<()> {
349:    pub async fn cont(&self) -> Result<()> {
355:    pub async fn reset(&self) -> Result<()> {
361:    pub async fn powerdown(&self) -> Result<()> {
367:    pub async fn quit(&self) -> Result<()> {
373:    pub async fn shutdown(mut self) {
=== projects/manager/tests/integration.rs ===
4:fn require_x86_64() -> bool {
13:async fn spawn_preconfig_and_start() {
38:async fn preconfig_shutdown_without_start() {
52:async fn preconfig_query_commands() {
70:async fn execute_multiple_commands_after_start() {
89:async fn events_received_on_stop() {
122:async fn detect_qemu_version() {
133:async fn arch_availability() {
144:async fn preconfig_min_version_constant() {
=== examples/simple-vm/src/main.rs ===
5:async fn main() -> Result<()> {
=== examples/preconfig-vm/src/main.rs ===
9:fn find_ovmf() -> Result<PathBuf> {
43:async fn main() -> Result<()> {
=== examples/events/src/main.rs ===
6:async fn main() -> Result<()> {
50:async fn drain_events(vm: &mut manager::Vm, timeout: Duration) {
[0m
Now let me read the specific files with clippy warnings and the largest non-generated source files for manual analysis.
[0m→ [0mRead ../../projects/qapi-codegen/src/parsers/qapi_documentation.rs[90m [offset=300, limit=25][0m
[0m→ [0mRead ../../projects/qapi-codegen/src/parsers/qapi_documentation.rs[90m [offset=490, limit=20][0m
[0m→ [0mRead ../../projects/qapi-codegen/src/parsers/qapi_type_ref.rs
[0m→ [0mRead ../../projects/qapi-codegen/src/unified.rs[90m [offset=160, limit=55][0m
[0m→ [0mRead ../../projects/qapi-codegen/src/unified.rs[90m [offset=1210, limit=20][0m
[0m→ [0mRead ../../projects/manager/src/vm.rs[90m [offset=280, limit=65][0m
[0m→ [0mRead ../../examples/events/src/main.rs[90m [offset=48, limit=20][0m
Good, I have all the clippy warnings context. Now let me check for large functions and do deeper analysis on the biggest files.
[0m
[0m$ [0mcd /home/bob/workspace/qrab && find projects examples -name '*.rs' ! -path '*/generated*' ! -path '*/target/*' -exec awk '/^[[:space:]]*(pub )?(async )?fn /{name=FILENAME":"NR" "$0; start=NR} name && /^[[:space:]]*\}[[:space:]]*$/{len=NR-start; if(len>50) print len" lines: "name}' {} +  | sort -rn | head -20
1257 lines: projects/qapi/src/lib.rs:1408 pub fn qga_version() -> &'static str {
1241 lines: projects/qapi/src/lib.rs:1408 pub fn qga_version() -> &'static str {
1227 lines: projects/qapi/src/lib.rs:1408 pub fn qga_version() -> &'static str {
1217 lines: projects/qapi/src/lib.rs:1408 pub fn qga_version() -> &'static str {
1209 lines: projects/qapi/src/lib.rs:1408 pub fn qga_version() -> &'static str {
1180 lines: projects/qapi/src/lib.rs:1408 pub fn qga_version() -> &'static str {
1163 lines: projects/qapi/src/lib.rs:1408 pub fn qga_version() -> &'static str {
1150 lines: projects/qapi/src/lib.rs:1408 pub fn qga_version() -> &'static str {
1132 lines: projects/qapi/src/lib.rs:1408 pub fn qga_version() -> &'static str {
1114 lines: projects/qapi/src/lib.rs:1408 pub fn qga_version() -> &'static str {
1106 lines: projects/qapi/src/lib.rs:1408 pub fn qga_version() -> &'static str {
1094 lines: projects/qapi-protocol/tests/common/serial.rs:11350     pub fn path(&self) -> &Path {
1092 lines: projects/qapi-protocol/tests/common/serial.rs:11350     pub fn path(&self) -> &Path {
1086 lines: projects/qapi-protocol/tests/common/serial.rs:11350     pub fn path(&self) -> &Path {
1080 lines: projects/qapi/src/lib.rs:1408 pub fn qga_version() -> &'static str {
1067 lines: projects/qapi-protocol/tests/common/serial.rs:11350     pub fn path(&self) -> &Path {
1055 lines: projects/qapi/src/lib.rs:1408 pub fn qga_version() -> &'static str {
1047 lines: projects/qapi-protocol/tests/common/serial.rs:11350     pub fn path(&self) -> &Path {
1045 lines: projects/qapi-protocol/tests/common/serial.rs:11350     pub fn path(&self) -> &Path {
1038 lines: projects/qapi/src/lib.rs:1408 pub fn qga_version() -> &'static str {
[0m
[0m$ [0mcd /home/bob/workspace/qrab && find projects examples -name '*.rs' ! -path '*/generated*' ! -path '*/target/*' -exec grep -c '' {} + | awk -F: '$2>500{print $2" "$1}' | sort -rn
1465 projects/qapi-codegen/src/unified.rs
1343 projects/qapi-macros/src/lib.rs
1255 projects/qapi/src/qga.rs
913 projects/qapi-protocol/tests/minimal.rs
911 projects/qapi-codegen/src/qapi_ir/process_qapi.rs
614 projects/qapi-codegen/src/parsers/qapi_documentation.rs
[0m
Now let me look at the key large functions for complexity, then start auto-fixing.
[0m→ [0mRead ../../projects/qapi-macros/src/lib.rs[90m [offset=409, limit=100][0m
[0m→ [0mRead ../../projects/qapi-macros/src/lib.rs[90m [offset=758, limit=150][0m
[0m→ [0mRead ../../projects/qapi-codegen/src/unified.rs[90m [offset=683, limit=170][0m
Good, I have a thorough view of the codebase. Now let me apply the auto-fixes for all clippy warnings.
[0m
[0m# [0mTodos
[x] Read .agent-memory/ and project context
[x] Run cargo clippy and cargo build to capture warnings
[x] Manual analysis: complexity, duplication, dead code, maintainability
[ ] Auto-fix: remove dead code, fix simple warnings, apply straightforward refactors
[ ] Output findings report
[0m
Now applying all 8 clippy fixes. Let me start with the straightforward ones:

**Fix 1**: `qapi_documentation.rs:309` — while_let_loop
[0m← [0mEdit ../../projects/qapi-codegen/src/parsers/qapi_documentation.rs
Index: /home/bob/workspace/qrab/projects/qapi-codegen/src/parsers/qapi_documentation.rs
===================================================================
--- /home/bob/workspace/qrab/projects/qapi-codegen/src/parsers/qapi_documentation.rs
+++ /home/bob/workspace/qrab/projects/qapi-codegen/src/parsers/qapi_documentation.rs
@@ -1,13 +1,13 @@
 use super::parser_version;
-use nom::IResult;
-use nom::Parser;
 use nom::bytes::complete::{is_a, tag, tag_no_case, take_while1};
 use nom::character::complete::{line_ending, not_line_ending, space0};
 use nom::combinator::{not, opt, peek, recognize};
 use nom::error::{Error, ErrorKind};
 use nom::multi::{many0, many1};
 use nom::sequence::{preceded, terminated};
+use nom::IResult;
+use nom::Parser;
 
 // ─── Shared helpers ────────────────────────────────────────────────
 
 fn not_name_break(c: char) -> bool {
@@ -305,15 +305,11 @@
                     field_vals.push(first_line);
                 }
                 // Consume continuation lines (indented, not starting with @, not a section keyword)
                 let mut next = next;
-                loop {
-                    if let Ok((n, line)) = take_line_continuation(next) {
-                        field_vals.push(line);
-                        next = n;
-                    } else {
-                        break;
-                    }
+                while let Ok((n, line)) = take_line_continuation(next) {
+                    field_vals.push(line);
+                    next = n;
                 }
                 // Skip trailing empty lines
                 while let Ok((n, _)) = take_empty_line(next) {
                     next = n;

[0m
**Fix 2**: `qapi_documentation.rs:499` — manual_pattern_char_comparison
[0m← [0mEdit ../../projects/qapi-codegen/src/parsers/qapi_documentation.rs
Index: /home/bob/workspace/qrab/projects/qapi-codegen/src/parsers/qapi_documentation.rs
===================================================================
--- /home/bob/workspace/qrab/projects/qapi-codegen/src/parsers/qapi_documentation.rs
+++ /home/bob/workspace/qrab/projects/qapi-codegen/src/parsers/qapi_documentation.rs
@@ -490,12 +490,9 @@
         tmp = next;
     }
     tmp
 };
-if check_input
-    .trim_start_matches(|c: char| c == '#' || c == ' ')
-    .starts_with('@')
-{
+if check_input.trim_start_matches(['#', ' ']).starts_with('@') {
     return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
 }
 // Consume lines starting with # until ##
 let mut input = input;

[0m
**Fix 3**: `qapi_type_ref.rs:23` — derivable_impls (replace manual Default with derive)
[0m← [0mEdit ../../projects/qapi-codegen/src/parsers/qapi_type_ref.rs
Index: /home/bob/workspace/qrab/projects/qapi-codegen/src/parsers/qapi_type_ref.rs
===================================================================
--- /home/bob/workspace/qrab/projects/qapi-codegen/src/parsers/qapi_type_ref.rs
+++ /home/bob/workspace/qrab/projects/qapi-codegen/src/parsers/qapi_type_ref.rs
@@ -1,19 +1,20 @@
 use super::common::{qstring, qtag};
 use super::parser_version;
 use super::qapi_members::QapiMembers;
-use nom::IResult;
-use nom::Parser;
 use nom::branch::alt;
 use nom::sequence::delimited;
+use nom::IResult;
+use nom::Parser;
 
 pub fn take_type_ref(input: &str) -> IResult<&str, QapiTypeRef<'_>> {
     use super::common::take_kv;
     take_kv("type", QapiTypeRef::parse).parse(input)
 }
 
-#[derive(Debug, Clone, PartialEq)]
+#[derive(Debug, Clone, PartialEq, Default)]
 pub enum QapiTypeRef<'i> {
+    #[default]
     Unset,
     Ref(&'i str),
     ArrayRef(&'i str),
     /// Inline anonymous struct definition (v1.0–v2.3.0)

[0m
Now remove the manual `impl Default`:
[0m← [0mEdit ../../projects/qapi-codegen/src/parsers/qapi_type_ref.rs
Index: /home/bob/workspace/qrab/projects/qapi-codegen/src/parsers/qapi_type_ref.rs
===================================================================
--- /home/bob/workspace/qrab/projects/qapi-codegen/src/parsers/qapi_type_ref.rs
+++ /home/bob/workspace/qrab/projects/qapi-codegen/src/parsers/qapi_type_ref.rs
@@ -20,14 +20,8 @@
     /// Inline anonymous struct definition (v1.0–v2.3.0)
     InlineStruct(QapiMembers<'i>),
 }
 
-impl Default for QapiTypeRef<'_> {
-    fn default() -> Self {
-        Self::Unset
-    }
-}
-
 impl<'i> QapiTypeRef<'i> {
     /// Dispatches to the correct versioned parser.
     pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
         let v = parser_version();

[0m
**Fix 4**: `unified.rs:169` — collapsible_if
[0m← [0mEdit ../../projects/qapi-codegen/src/unified.rs
Index: /home/bob/workspace/qrab/projects/qapi-codegen/src/unified.rs
===================================================================
--- /home/bob/workspace/qrab/projects/qapi-codegen/src/unified.rs
+++ /home/bob/workspace/qrab/projects/qapi-codegen/src/unified.rs
@@ -165,10 +165,11 @@
 };
 
 let processed = process_command(v.clone());
 
-if let Some(ref ref_name) = data_ref_name {
-    if let Some(existing) = structs_lookup.get_mut(ref_name.as_str()) {
+if let Some(ref ref_name) = data_ref_name
+    && let Some(existing) = structs_lookup.get_mut(ref_name.as_str())
+{
         // Command references an existing struct — merge command attributes onto it.
         for attr in &processed.meta.attributes {
             match attr {
                 Attribute::Unit(n)

[0m
Now fix the closing braces — need to remove one level of nesting:
[0m→ [0mRead ../../projects/qapi-codegen/src/unified.rs[90m [offset=195, limit=20][0m
I see — need to remove the extra closing brace from the old nested `if`:
[0m
[0m← [0mEdit ../../projects/qapi-codegen/src/unified.rs
Index: /home/bob/workspace/qrab/projects/qapi-codegen/src/unified.rs
===================================================================
--- /home/bob/workspace/qrab/projects/qapi-codegen/src/unified.rs
+++ /home/bob/workspace/qrab/projects/qapi-codegen/src/unified.rs
@@ -204,9 +204,8 @@
             // Remove old command entry that was created from inline data in earlier versions
             let old_cmd_name = processed.name.clone();
             structs_lookup.remove(&old_cmd_name);
             continue;
-        }
     }
 
     structs_lookup.insert(processed.name.clone(), processed);
 }

[0m
**Fix 5**: `unified.rs:1220` — collapsible_if
[0m← [0mEdit ../../projects/qapi-codegen/src/unified.rs
Index: /home/bob/workspace/qrab/projects/qapi-codegen/src/unified.rs
===================================================================
--- /home/bob/workspace/qrab/projects/qapi-codegen/src/unified.rs
+++ /home/bob/workspace/qrab/projects/qapi-codegen/src/unified.rs
@@ -1216,12 +1216,12 @@
 }
 if let Some(unstable) = us.versions.unstable {
     let mut inner = vec![Attribute::with_value("added", version_str(unstable))];
     // Check if unstable was removed (stabilized) via attr_versions
-    if let Some(uvr) = us.attr_versions.get("unstable") {
-        if let Some(until) = uvr.until {
-            inner.push(Attribute::with_value("removed", version_str(until)));
-        }
+    if let Some(uvr) = us.attr_versions.get("unstable")
+        && let Some(until) = uvr.until
+    {
+        inner.push(Attribute::with_value("removed", version_str(until)));
     }
     ir.meta
         .attributes
         .push(Attribute::nested("unstable", inner));

[0m
**Fix 6 & 7**: `vm.rs:291` and `vm.rs:337` — needless_question_mark (both identical patterns)
[0m← [0mEdit ../../projects/manager/src/vm.rs
Index: /home/bob/workspace/qrab/projects/manager/src/vm.rs
===================================================================
--- /home/bob/workspace/qrab/projects/manager/src/vm.rs
+++ /home/bob/workspace/qrab/projects/manager/src/vm.rs
@@ -287,9 +287,9 @@
 pub async fn execute<C: QapiCommand + qapi::PreconfigSafe>(
     &self,
     cmd: &C,
 ) -> Result<C::Returns> {
-    Ok(self.qmp.execute(cmd).await?)
+    self.qmp.execute(cmd).await
 }
 
 /// Exit preconfig mode and start guest execution.
 ///

[0m
Now fix the second one in `Vm::execute`:
[0m← [0mEdit ../../projects/manager/src/vm.rs
Index: /home/bob/workspace/qrab/projects/manager/src/vm.rs
===================================================================
--- /home/bob/workspace/qrab/projects/manager/src/vm.rs
+++ /home/bob/workspace/qrab/projects/manager/src/vm.rs
@@ -333,9 +333,9 @@
 }
 
 /// Execute any QMP command and return the typed response.
 pub async fn execute<C: QapiCommand>(&self, cmd: &C) -> Result<C::Returns> {
-    Ok(self.qmp.execute(cmd).await?)
+    self.qmp.execute(cmd).await
 }
 
 // -- Ergonomic lifecycle methods --
 

[0m
Good, both `vm.rs` fixes are done. **Fix 8**: `events/src/main.rs:53` — while_let_loop
[0m← [0mEdit ../../examples/events/src/main.rs
Index: /home/bob/workspace/qrab/examples/events/src/main.rs
===================================================================
--- /home/bob/workspace/qrab/examples/events/src/main.rs
+++ /home/bob/workspace/qrab/examples/events/src/main.rs
@@ -49,13 +49,8 @@
 
 async fn drain_events(vm: &mut manager::Vm, timeout: Duration) {
     sleep(Duration::from_millis(50)).await; // let events arrive
     let deadline = tokio::time::Instant::now() + timeout;
-    loop {
-        match tokio::time::timeout_at(deadline, vm.events.recv()).await {
-            Ok(Some(evt)) => {
-                println!("    {evt:?}");
-            }
-            _ => break,
-        }
+    while let Ok(Some(evt)) = tokio::time::timeout_at(deadline, vm.events.recv()).await {
+        println!("    {evt:?}");
     }
 }

[0m
All 8 clippy fixes applied. Now let me also check that the first `vm.rs` edit (Fix 6) applied correctly:
[0m→ [0mRead ../../projects/manager/src/vm.rs[90m [offset=287, limit=10][0m
All fixes confirmed. Now let me verify the build is clean:
[0m
[0m$ [0mcd /home/bob/workspace/qrab && cargo clippy -- -W clippy::all -W clippy::complexity 2>&1
Checking qapi-codegen v0.1.0 (/home/bob/workspace/qrab/projects/qapi-codegen)
warning: large size difference between variants
     --> projects/qapi/src/generated.rs:14552:1
      |
14552 | / pub enum ObjectOptionsBranch {
14553 | |     #[qapi(name = "acpi-generic-initiator")]
14554 | |     #[qapi(added = "9.0.0")]
14555 | |     AcpiGenericInitiator(AcpiGenericInitiatorProperties),
...     |
14648 | |     TdxGuest(TdxGuestProperties),
      | |     ---------------------------- the second-largest variant contains at least 192 bytes
...     |
14653 | |     ThrottleGroup(ThrottleGroupProperties),
      | |     -------------------------------------- the largest variant contains at least 608 bytes
...     |
14666 | |     XVfioUserServer(VfioUserServerProperties),
14667 | | }
      | |_^ the entire enum is at least 608 bytes
      |
      = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.92.0/index.html#large_enum_variant
      = note: `-W clippy::large-enum-variant` implied by `-W clippy::all`
      = help: to override `-W clippy::all` add `#[allow(clippy::large_enum_variant)]`
help: consider boxing the large fields or introducing indirection in some other way to reduce the total size of the enum
      |
14653 -     ThrottleGroup(ThrottleGroupProperties),
14653 +     ThrottleGroup(Box<ThrottleGroupProperties>),
      |

warning: `qapi` (lib) generated 1 warning
    Checking manager v0.1.0 (/home/bob/workspace/qrab/projects/manager)
    Checking simple-vm v0.1.0 (/home/bob/workspace/qrab/examples/simple-vm)
    Checking events v0.1.0 (/home/bob/workspace/qrab/examples/events)
    Checking preconfig-vm v0.1.0 (/home/bob/workspace/qrab/examples/preconfig-vm)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.93s
[0m
All 8 hand-written clippy warnings are fixed. The only remaining warning is in `generated.rs` (large_enum_variant) which is auto-generated code — not something to fix by hand.

Now let me finalize the report.
[0m# [0mTodos
[x] Read .agent-memory/ and project context
[x] Run cargo clippy and cargo build to capture warnings
[x] Manual analysis: complexity, duplication, dead code, maintainability
[x] Auto-fix: remove dead code, fix simple warnings, apply straightforward refactors
[ ] Output findings report
[0m
# Code Quality Findings — qrab

## Last Run
2026-04-11T04:41:44Z

## Summary
The codebase is in good shape with no dead code or build warnings outside generated files. All 8 clippy warnings in hand-written code have been auto-fixed. The main quality concerns are two god modules (`unified.rs` at 1465 lines, `qapi-macros/src/lib.rs` at 1343 lines) and the `merge_struct` function at 155 lines with deep nesting — these are candidates for decomposition.

## Auto-Fixes Applied (8 total)

| # | File | Line | Warning | Change |
|---|------|------|---------|--------|
| 1 | `projects/qapi-codegen/src/parsers/qapi_documentation.rs` | 309 | `while_let_loop` | `loop { if let Ok(...) { } else { break } }` → `while let Ok(...) { }` |
| 2 | `projects/qapi-codegen/src/parsers/qapi_documentation.rs` | 499 | `manual_pattern_char_comparison` | `\|c: char\| c == '#' \|\| c == ' '` → `['#', ' ']` |
| 3 | `projects/qapi-codegen/src/parsers/qapi_type_ref.rs` | 23 | `derivable_impls` | Manual `impl Default` → `#[derive(Default)]` + `#[default]` on variant |
| 4 | `projects/qapi-codegen/src/unified.rs` | 169 | `collapsible_if` | Nested `if let` → chained `if let ... && let ...` |
| 5 | `projects/qapi-codegen/src/unified.rs` | 1220 | `collapsible_if` | Nested `if let` → chained `if let ... && let ...` |
| 6 | `projects/manager/src/vm.rs` | 291 | `needless_question_mark` | `Ok(self.qmp.execute(cmd).await?)` → `self.qmp.execute(cmd).await` |
| 7 | `projects/manager/src/vm.rs` | 337 | `needless_question_mark` | `Ok(self.qmp.execute(cmd).await?)` → `self.qmp.execute(cmd).await` |
| 8 | `examples/events/src/main.rs` | 53 | `while_let_loop` | `loop { match ... { _ => break } }` → `while let Ok(Some(evt)) = ... { }` |

## Critical
_None._

## Warnings

- **[CQ-W001]** `projects/qapi-codegen/src/unified.rs:683` — `merge_struct()` is ~155 lines with nesting depth 5
  - **Impact**: Hard to reason about, high cognitive load. The function handles: field existence checks, rename detection, collision disambiguation, version tracking, and initial insertion — all in one function.
  - **Recommendation**: Extract into helpers: `merge_existing_fields()`, `handle_field_rename()`, `handle_field_collision()`, `insert_new_struct()`. Each would be <40 lines.

- **[CQ-W002]** `projects/qapi-codegen/src/unified.rs` — god module at 1465 lines
  - **Impact**: Difficult to navigate; mixes registry building, merging logic, code generation, and recursive type fixing.
  - **Recommendation**: Split into `unified/registry.rs` (build + merge), `unified/codegen.rs` (generate_unified_*), `unified/recursive.rs` (fix_recursive_types).

- **[CQ-W003]** `projects/qapi-macros/src/lib.rs` — god module at 1343 lines
  - **Impact**: Single file contains all proc-macro logic: attribute parsing, enum codegen, struct codegen, serialization, deserialization, Display, command/event impls.
  - **Recommendation**: Split into `attrs.rs` (parse_type_attrs, parse_item_attrs, version helpers), `enum_gen.rs` (gen_enum, gen_unit_enum_ser/de, gen_alt_ser/de), `struct_gen.rs` (gen_struct, gen_struct_ser/de).

- **[CQ-W004]** `projects/qapi/src/generated.rs:14552` — `ObjectOptionsBranch` enum is 608 bytes due to `ThrottleGroupProperties`
  - **Impact**: All variants pay the cost of the largest. Stack-allocated 608-byte values are expensive to move.
  - **Recommendation**: Fix in the codegen (`unified.rs`) to emit `Box<T>` for large variants. Not a hand-edit — requires updating `generate_unified_enum()` to detect and box variants above a threshold.

## Suggestions

- **[CQ-S001]** `projects/qapi-macros/src/lib.rs:758` — `gen_struct()` is ~150 lines
  - Already partially decomposed (calls `gen_struct_ser`, `gen_struct_de`, etc.) but the Display generation block (lines 849–920) could be extracted to `gen_struct_display()`.

- **[CQ-S002]** `projects/qapi-codegen/src/unified.rs:563` — `merge_enum()` is ~115 lines
  - Similar pattern to `merge_struct` — handles variant merging, rename detection, collision. Could benefit from the same helper extraction approach.

- **[CQ-S003]** `projects/qapi-codegen/src/unified.rs:920` — `generate_unified_code()` is ~90 lines
  - Reasonable size but mixes registry building with code generation. Consider separating the two phases more clearly.

- **[CQ-S004]** Duplication between `merge_enum()` and `merge_struct()`
  - Both follow the same pattern: check existing → update versions → merge fields/variants → detect renames → handle collisions → mark removed. A trait or generic merge function could reduce the ~270 combined lines.

- **[CQ-S005]** Duplication between `build_unified_registry()` (line 310) and `build_qga_registry()` (line 1310)
  - Nearly identical structure. Both iterate version tags, process schemas, merge enums/structs. Parameterize with a schema extraction function.

- **[CQ-S006]** `projects/qapi-codegen/src/unified.rs:1027` and `:1118` — `generate_unified_enum()` (90 lines) and `generate_unified_struct()` (190 lines)
  - `generate_unified_struct()` is the largest codegen function. The attribute generation section (lines 1170–1240) could be extracted.

## Metrics

| Metric | Value |
|--------|-------|
| total clippy warnings (before fix) | 8 hand-written + 1 generated |
| total clippy warnings (after fix) | 1 (generated code only) |
| dead code instances | 0 |
| functions >50 lines | 5 (`merge_struct`, `merge_enum`, `gen_struct`, `gen_enum`, `generate_unified_struct`) |
| max nesting depth | 5 (`merge_struct`) |
| largest module (lines) | `unified.rs`: 1465 |
| second largest module | `qapi-macros/lib.rs`: 1343 |
| generated code | `generated.rs`: 22,294 lines; `qga.rs`: 1,255 lines |
[0m# [0mTodos
[x] Read .agent-memory/ and project context
[x] Run cargo clippy and cargo build to capture warnings
[x] Manual analysis: complexity, duplication, dead code, maintainability
[x] Auto-fix: remove dead code, fix simple warnings, apply straightforward refactors
[x] Output findings report
[0m
Analysis complete. Here's the summary of what was done:

**8 auto-fixes applied** — all clippy warnings in hand-written code are resolved. The build is clean except for 1 warning in generated code (`large_enum_variant` in `generated.rs`).

**Key findings** (no critical issues):
- **CQ-W001**: `merge_struct()` in `unified.rs:683` — 155 lines, nesting depth 5. Decompose into helpers.
- **CQ-W002**: `unified.rs` — 1465-line god module. Split into registry/codegen/recursive submodules.
- **CQ-W003**: `qapi-macros/src/lib.rs` — 1343-line god module. Split into attrs/enum_gen/struct_gen.
- **CQ-W004**: `ObjectOptionsBranch` enum is 608 bytes. Fix in codegen to box large variants.
- **CQ-S004/S005**: Duplication between `merge_enum`/`merge_struct` and `build_unified_registry`/`build_qga_registry` — candidates for generalization.
Files changed:
- edited /home/bob/workspace/qrab/projects/qapi-codegen/src/parsers/qapi_documentation.rs
- edited /home/bob/workspace/qrab/projects/qapi-codegen/src/parsers/qapi_type_ref.rs
- edited /home/bob/workspace/qrab/projects/qapi-codegen/src/unified.rs
- edited /home/bob/workspace/qrab/projects/manager/src/vm.rs
- edited /home/bob/workspace/qrab/examples/events/src/main.rs

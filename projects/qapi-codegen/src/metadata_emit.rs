//! Emit compile-time schema metadata from the unified registry.
//!
//! Walks `UnifiedRegistry` and produces a Rust source file containing static
//! arrays / `phf::Map`s describing every command, event, struct, enum, and
//! field. Consumers (notably the BMC web UI) use this to render forms
//! schema-driven without hard-coding any QAPI type information.
//!
//! The emitted file is `projects/qapi/src/generated/metadata.rs`. It is
//! never hand-edited — regenerate via the `qapi-codegen` binary.
//!
//! Shape highlights:
//! - `TYPE_REFS`: deduplicated pool of every type referenced by a field or
//!   command return. `FieldMeta::ty` and `CommandMeta::returns` are indices
//!   into this pool. `List` entries point at their inner element by index.
//! - `STRUCTS` / `ENUMS`: `phf::Map` keyed by Rust type name.
//! - `COMMANDS`: ordered slice of `CommandMeta`, each carrying a pointer to
//!   its arg `StructMeta`.

use crate::qapi_ir::{
    Attribute, EnumVariantKind, Struct, StructField, rustify_field, rustify_name,
};
use crate::unified::{
    UnifiedEnum, UnifiedRegistry, UnifiedStruct, UnifiedType, Version, VersionRange,
};
use std::collections::{BTreeMap, BTreeSet};

// ── helpers ────────────────────────────────────────────────────────────────

fn ver_tuple(v: Version) -> String {
    format!("({}u32, {}u32, {}u32)", v.0, v.1, v.2)
}

fn ver_opt(v: Option<Version>) -> String {
    match v {
        Some(t) => format!("Some({})", ver_tuple(t)),
        None => "None".to_string(),
    }
}

/// Escape a string for inclusion as a Rust string literal.
fn rs_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x00'..='\x1f' => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Find a `Unit` attribute by name.
fn has_unit_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs
        .iter()
        .any(|a| matches!(a, Attribute::Unit(n) if n == name))
}

/// Find an `Item` attribute and return its value.
fn item_attr<'a>(attrs: &'a [Attribute], name: &str) -> Option<&'a str> {
    attrs.iter().find_map(|a| match a {
        Attribute::Item { name: n, value } if n == name => Some(value.as_str()),
        _ => None,
    })
}

/// Detect a `feature = "deprecated"` (or "unstable") flag living inside a
/// `List(...)` attribute bundle on the IR node.
fn has_feature(attrs: &[Attribute], feat: &str) -> bool {
    attrs.iter().any(|a| match a {
        Attribute::List(items) => items.iter().any(
            |i| matches!(i, Attribute::Item { name, value } if name == "feature" && value == feat),
        ),
        _ => false,
    })
}

fn field_deprecated(f: &StructField) -> bool {
    has_feature(&f.meta.attributes, "deprecated")
}

fn field_unstable(f: &StructField) -> bool {
    has_feature(&f.meta.attributes, "unstable")
}

fn field_recursive(f: &StructField) -> bool {
    has_unit_attr(&f.meta.attributes, "boxed")
}

/// Wire name for a field. The `name` attribute (if present) overrides the
/// snake-cased ident; otherwise the ident itself is the wire name.
fn field_wire_name(f: &StructField) -> String {
    item_attr(&f.meta.attributes, "name")
        .map(String::from)
        .unwrap_or_else(|| f.name.clone())
}

/// Wire name for a struct/enum (used for both data types and commands).
fn type_wire_name(name: &str, attrs: &[Attribute]) -> String {
    item_attr(attrs, "name")
        .map(String::from)
        .unwrap_or_else(|| name.to_string())
}

/// Map QAPI primitive type names to the `Prim` enum variant.
/// Returns `None` for non-primitive (struct / enum / list) types.
fn qapi_prim_for_type(t: &str) -> Option<&'static str> {
    Some(match t {
        "str" => "Str",
        "bool" => "Bool",
        "int" | "int64" => "I64",
        "int8" => "I8",
        "int16" => "I16",
        "int32" => "I32",
        "uint8" => "U8",
        "uint16" => "U16",
        "uint32" => "U32",
        "uint64" | "size" => "U64",
        "number" => "F64",
        "null" => "Null",
        "any" | "visitor" | "dict" | "**" => "Any",
        _ => return None,
    })
}

// ── type-ref pool ──────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Eq, Hash)]
struct TypeKey {
    /// "Primitive", "Enum", "Struct", or "List".
    kind: &'static str,
    /// Display name. For lists, "List<inner>".
    name: String,
    /// Variant of `Prim`. "" for non-primitives.
    prim: &'static str,
    /// Index of the inner element (lists only). 0 otherwise.
    inner: u32,
}

/// Deduplicated pool of every type referenced from any field or command.
struct TypePool {
    entries: Vec<TypeKey>,
    index: BTreeMap<String, u32>,
}

impl TypePool {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: BTreeMap::new(),
        }
    }

    /// Pool key composed of kind+name+prim+inner. Inner index is part of the
    /// key so two `List<T>` referring to different inner types stay distinct.
    fn key_for(k: &TypeKey) -> String {
        format!("{}|{}|{}|{}", k.kind, k.name, k.prim, k.inner)
    }

    fn intern(&mut self, k: TypeKey) -> u32 {
        let key = Self::key_for(&k);
        if let Some(&idx) = self.index.get(&key) {
            return idx;
        }
        let idx = self.entries.len() as u32;
        self.entries.push(k);
        self.index.insert(key, idx);
        idx
    }

    /// Resolve a (rust-type-name, array-flag) pair into a pool index.
    fn intern_field(
        &mut self,
        rust_type: &str,
        array: bool,
        struct_names: &BTreeSet<String>,
        enum_names: &BTreeSet<String>,
    ) -> u32 {
        let inner_idx = self.intern_scalar(rust_type, struct_names, enum_names);
        if array {
            let inner_name = self.entries[inner_idx as usize].name.clone();
            self.intern(TypeKey {
                kind: "List",
                name: format!("List<{}>", inner_name),
                prim: "Null",
                inner: inner_idx,
            })
        } else {
            inner_idx
        }
    }

    fn intern_scalar(
        &mut self,
        rust_type: &str,
        struct_names: &BTreeSet<String>,
        enum_names: &BTreeSet<String>,
    ) -> u32 {
        if let Some(prim) = qapi_prim_for_type(rust_type) {
            return self.intern(TypeKey {
                kind: "Primitive",
                name: rust_type.to_string(),
                prim,
                inner: 0,
            });
        }
        // Map QAPI type name → rust name (mirrors qapi_to_rust_type for non-primitives)
        let rust = rustify_name(rust_type);
        if enum_names.contains(&rust) {
            return self.intern(TypeKey {
                kind: "Enum",
                name: rust,
                prim: "Null",
                inner: 0,
            });
        }
        if struct_names.contains(&rust) {
            return self.intern(TypeKey {
                kind: "Struct",
                name: rust,
                prim: "Null",
                inner: 0,
            });
        }
        // Unknown — likely a runtime opaque type (HashMap<...>, serde_json::Value, etc.).
        // Treat as `Any` primitive so the UI can render a free-form JSON editor.
        self.intern(TypeKey {
            kind: "Primitive",
            name: rust,
            prim: "Any",
            inner: 0,
        })
    }

    /// Resolve a command's `#[qapi(returns = "...")]` attribute string into a
    /// pool index. The string is the already-rustified Rust type name (e.g.
    /// `"AddfdInfo"`, `"Vec<String>"`, `"()"`).
    fn intern_returns(
        &mut self,
        rust_returns: &str,
        struct_names: &BTreeSet<String>,
        enum_names: &BTreeSet<String>,
    ) -> Option<u32> {
        let rust_returns = rust_returns.trim();
        if rust_returns == "()" {
            return None;
        }
        if let Some(inner) = rust_returns
            .strip_prefix("Vec<")
            .and_then(|s| s.strip_suffix('>'))
        {
            let inner_idx = self
                .intern_returns(inner.trim(), struct_names, enum_names)
                .unwrap_or_else(|| self.intern_scalar("null", struct_names, enum_names));
            let inner_name = self.entries[inner_idx as usize].name.clone();
            return Some(self.intern(TypeKey {
                kind: "List",
                name: format!("List<{inner_name}>"),
                prim: "Null",
                inner: inner_idx,
            }));
        }
        // Return attributes already contain Rust names; field and variant IR
        // retain QAPI names. Do not confuse a declared String with builtin str.
        let schema_type = match rust_returns {
            "String" => "str",
            "i8" => "int8",
            "i16" => "int16",
            "i32" => "int32",
            "i64" => "int64",
            "u8" => "uint8",
            "u16" => "uint16",
            "u32" => "uint32",
            "u64" => "uint64",
            "f64" => "number",
            "serde_json::Value" => "any",
            other => other,
        };
        Some(self.intern_field(schema_type, false, struct_names, enum_names))
    }

    fn emit(&self) -> String {
        let mut s = String::new();
        s.push_str("pub static TYPE_REFS: &[TypeRef] = &[\n");
        for e in &self.entries {
            s.push_str(&format!(
                "    TypeRef {{ kind: TypeKind::{}, name: {}, prim: Prim::{}, inner: {} }},\n",
                e.kind,
                rs_str(&e.name),
                e.prim,
                e.inner,
            ));
        }
        s.push_str("];\n");
        s
    }
}

// ── classification ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
    Command,
    Event,
    Data,
}

fn classify(us: &UnifiedStruct) -> Category {
    let attrs = &us.ir.meta.attributes;
    if has_unit_attr(attrs, "command") {
        Category::Command
    } else if has_unit_attr(attrs, "event") {
        Category::Event
    } else {
        Category::Data
    }
}

/// Detect a `gen: false` command — currently encoded as a flattened
/// `arguments: HashMap<String, serde_json::Value>` field.
fn is_free_form(s: &Struct) -> bool {
    s.fields.iter().any(|f| {
        f.name == "arguments"
            && f.r#type.starts_with("HashMap<")
            && has_unit_attr(&f.meta.attributes, "flatten")
    })
}

/// Detect a flat-union struct: contains exactly one `#[qapi(discriminator)]`
/// field plus a sibling field named `u` with `#[qapi(flatten)]` whose type is
/// `<X>Branch`. Returns `(discriminator_rust_field, branch_enum_name)`.
fn detect_union(s: &Struct) -> Option<(String, String)> {
    let mut disc: Option<&StructField> = None;
    let mut branch: Option<String> = None;
    for f in &s.fields {
        if has_unit_attr(&f.meta.attributes, "discriminator") {
            if disc.is_some() {
                return None; // more than one discriminator
            }
            disc = Some(f);
        }
        if f.name == "u" && has_unit_attr(&f.meta.attributes, "flatten") {
            // Field type is the enum (or Box<enum>), but the IR stores the
            // raw type name — `XBranch`. Strip any `Box<...>` wrappers.
            let mut t = f.r#type.as_str();
            if let Some(inner) = t.strip_prefix("Box<").and_then(|s| s.strip_suffix('>')) {
                t = inner.trim();
            }
            branch = Some(rustify_name(t));
        }
    }
    match (disc, branch) {
        (Some(d), Some(b)) => Some((rustify_field(&d.name), b)),
        _ => None,
    }
}

/// Walk every struct, collect the set of branch-enum type names that any
/// flat-union references via its `u` field. Used to classify enums as
/// `FlatUnionBranch` vs `Alternate`.
fn collect_branch_enums(registry: &UnifiedRegistry) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for ut in registry.types.values() {
        if let UnifiedType::Struct(us) = ut {
            if let Some((_, branch)) = detect_union(&us.ir) {
                out.insert(branch);
            }
        }
    }
    out
}

// ── version helpers for fields/variants ────────────────────────────────────

fn field_versions(us: &UnifiedStruct, field_name: &str) -> (Version, Option<Version>) {
    match us.field_versions.get(field_name) {
        Some(vr) => (vr.since, vr.until),
        None => (us.versions.since, us.versions.until),
    }
}

fn variant_versions(ue: &UnifiedEnum, variant_name: &str) -> (Version, Option<Version>) {
    match ue.variant_versions.get(variant_name) {
        Some(vr) => (vr.since, vr.until),
        None => (ue.versions.since, ue.versions.until),
    }
}

// ── struct emission ───────────────────────────────────────────────────────

struct StructEmit {
    /// Rust name (used as map key + identifier).
    rust_name: String,
    /// Wire name (overridden by `name` attr if present).
    wire_name: String,
    doc: String,
    fields_var: String,
    is_union: bool,
    discriminator_field: Option<String>,
    branch_enum: Option<String>,
    free_form: bool,
    versions: VersionRange,
}

fn emit_struct(us: &UnifiedStruct) -> StructEmit {
    let rust_name = rustify_name(&us.ir.name);
    let wire_name = type_wire_name(&us.ir.name, &us.ir.meta.attributes);
    let doc = us.ir.meta.doc.clone().unwrap_or_default();

    let union = detect_union(&us.ir);
    let is_union = union.is_some();
    let (disc, branch) = union.unzip();
    let free_form = is_free_form(&us.ir);

    let fields_var = format!("FIELDS_{}", rust_name);

    StructEmit {
        rust_name,
        wire_name,
        doc,
        fields_var,
        is_union,
        discriminator_field: disc,
        branch_enum: branch,
        free_form,
        versions: us.versions.clone(),
    }
}

fn emit_field_array(
    us: &UnifiedStruct,
    var_name: &str,
    pool: &mut TypePool,
    struct_names: &BTreeSet<String>,
    enum_names: &BTreeSet<String>,
) -> String {
    let mut s = String::new();
    s.push_str(&format!("static {}: &[FieldMeta] = &[\n", var_name));
    for f in &us.ir.fields {
        // Skip the synthetic `arguments: HashMap<...>` flatten field on free-form
        // commands and the `u: Option<XBranch>` flatten field on unions; the BMC
        // doesn't render them as input fields.
        if f.name == "arguments" && f.r#type.starts_with("HashMap<") {
            continue;
        }
        if f.name == "u" && has_unit_attr(&f.meta.attributes, "flatten") {
            continue;
        }

        let rust_field = rustify_field(&f.name);
        let wire = field_wire_name(f);
        let ty_idx = pool.intern_field(&f.r#type, f.array, struct_names, enum_names);
        let optional = f.optional;
        let is_disc = has_unit_attr(&f.meta.attributes, "discriminator");
        let recursive = field_recursive(f);
        let (since, until) = field_versions(us, &f.name);
        let dep = field_deprecated(f);
        let unstable = field_unstable(f);
        let doc = f.meta.doc.clone().unwrap_or_default();

        s.push_str(&format!(
            "    FieldMeta {{ name: {}, wire_name: {}, ty: {}, optional: {}, is_discriminator: {}, recursive: {}, since: {}, until: {}, deprecated: {}, unstable: {}, doc: {} }},\n",
            rs_str(&rust_field),
            rs_str(&wire),
            ty_idx,
            optional,
            is_disc,
            recursive,
            ver_tuple(since),
            ver_opt(until),
            dep,
            unstable,
            rs_str(&doc),
        ));
    }
    s.push_str("];\n");
    s
}

fn emit_struct_meta(decl_name: &str, e: &StructEmit) -> String {
    format!(
        "static {}: StructMeta = StructMeta {{\n\
         \x20   name: {},\n\
         \x20   doc: {},\n\
         \x20   fields: {},\n\
         \x20   is_union: {},\n\
         \x20   discriminator_field: {},\n\
         \x20   branch_enum: {},\n\
         \x20   free_form: {},\n\
         \x20   since: {},\n\
         \x20   until: {},\n\
         }};\n",
        decl_name,
        rs_str(&e.wire_name),
        rs_str(&e.doc),
        format!("&{}", e.fields_var),
        e.is_union,
        match &e.discriminator_field {
            Some(d) => format!("Some({})", rs_str(d)),
            None => "None".to_string(),
        },
        match &e.branch_enum {
            Some(b) => format!("Some({})", rs_str(b)),
            None => "None".to_string(),
        },
        e.free_form,
        ver_tuple(e.versions.since),
        ver_opt(e.versions.until),
    )
}

// ── enum emission ──────────────────────────────────────────────────────────

struct EnumEmit {
    rust_name: String,
    doc: String,
    variants_var: String,
    kind: &'static str,
    versions: VersionRange,
}

/// Intern payloads with the same scalar/list rules as struct fields.
fn emit_enum_variants(
    ue: &UnifiedEnum,
    var_name: &str,
    pool: &mut TypePool,
    struct_names: &BTreeSet<String>,
    enum_names: &BTreeSet<String>,
) -> String {
    let mut s = String::new();
    s.push_str(&format!("static {}: &[EnumVariantMeta] = &[\n", var_name));
    for v in &ue.ir.variants {
        let rust = crate::qapi_ir::rustify_variant(&v.name);
        let wire = item_attr(&v.meta.attributes, "name")
            .map(String::from)
            .unwrap_or_else(|| v.name.clone());
        let payload_type = if let EnumVariantKind::Tuple(ty) = &v.kind {
            Some(pool.intern_field(ty, v.array, struct_names, enum_names))
        } else {
            None
        };
        let payload = payload_type.and_then(|index| {
            let ty = &pool.entries[index as usize];
            (ty.kind == "Struct").then(|| ty.name.clone())
        });
        let (since, until) = variant_versions(ue, &v.name);
        let dep = has_feature(&v.meta.attributes, "deprecated");
        let unstable = has_feature(&v.meta.attributes, "unstable");
        let doc = v.meta.doc.clone().unwrap_or_default();
        s.push_str(&format!(
            "    EnumVariantMeta {{ name: {}, wire_name: {}, payload_struct: {}, payload_type: {}, since: {}, until: {}, deprecated: {}, unstable: {}, doc: {} }},\n",
            rs_str(&rust),
            rs_str(&wire),
            match payload {
                Some(p) => format!("Some({})", rs_str(&p)),
                None => "None".to_string(),
            },
            payload_type.map(|index| format!("Some({index})")).unwrap_or_else(|| "None".into()),
            ver_tuple(since),
            ver_opt(until),
            dep,
            unstable,
            rs_str(&doc),
        ));
    }
    s.push_str("];\n");
    s
}

fn emit_enum_meta(decl_name: &str, e: &EnumEmit) -> String {
    format!(
        "static {}: EnumMeta = EnumMeta {{\n\
         \x20   name: {},\n\
         \x20   doc: {},\n\
         \x20   variants: &{},\n\
         \x20   kind: EnumKind::{},\n\
         \x20   since: {},\n\
         \x20   until: {},\n\
         }};\n",
        decl_name,
        rs_str(&e.rust_name),
        rs_str(&e.doc),
        e.variants_var,
        e.kind,
        ver_tuple(e.versions.since),
        ver_opt(e.versions.until),
    )
}

// ── command emission ───────────────────────────────────────────────────────

struct CommandEmit {
    rust_name: String,
    wire_name: String,
    doc: String,
    args_static: String,
    returns_idx: Option<u32>,
    allow_oob: bool,
    allow_preconfig: bool,
    success_response: bool,
    deprecated: bool,
    unstable: bool,
    versions: VersionRange,
    category: String,
}

fn emit_command(
    us: &UnifiedStruct,
    pool: &mut TypePool,
    struct_names: &BTreeSet<String>,
    enum_names: &BTreeSet<String>,
) -> CommandEmit {
    let attrs = &us.ir.meta.attributes;
    let rust_name = rustify_name(&us.ir.name);
    let wire_name = type_wire_name(&us.ir.name, attrs);
    let doc = us.ir.meta.doc.clone().unwrap_or_default();
    let allow_oob = has_unit_attr(attrs, "allow_oob");
    let allow_preconfig = has_unit_attr(attrs, "allow_preconfig");
    let success_response = !has_unit_attr(attrs, "no_success_response");
    let deprecated = has_feature(attrs, "deprecated");
    let unstable = has_feature(attrs, "unstable");
    let returns = item_attr(attrs, "returns").unwrap_or("()");
    let returns_idx = pool.intern_returns(returns, struct_names, enum_names);
    let category = wire_name.split('-').next().unwrap_or("misc").to_string();

    CommandEmit {
        rust_name: rust_name.clone(),
        wire_name,
        doc,
        args_static: format!("STRUCT_{}", rust_name),
        returns_idx,
        allow_oob,
        allow_preconfig,
        success_response,
        deprecated,
        unstable,
        versions: us.versions.clone(),
        category,
    }
}

fn emit_command_meta(c: &CommandEmit) -> String {
    format!(
        "    CommandMeta {{ wire_name: {}, rust_name: {}, args: &{}, returns: {}, allow_oob: {}, allow_preconfig: {}, success_response: {}, since: {}, until: {}, deprecated: {}, unstable: {}, doc: {}, category: {} }},\n",
        rs_str(&c.wire_name),
        rs_str(&c.rust_name),
        c.args_static,
        match c.returns_idx {
            Some(i) => format!("Some({})", i),
            None => "None".to_string(),
        },
        c.allow_oob,
        c.allow_preconfig,
        c.success_response,
        ver_tuple(c.versions.since),
        ver_opt(c.versions.until),
        c.deprecated,
        c.unstable,
        rs_str(&c.doc),
        rs_str(&c.category),
    )
}

// ── top-level emit ────────────────────────────────────────────────────────

/// Emit a complete `metadata.rs` source file from the unified registry.
pub fn emit(registry: &UnifiedRegistry) -> String {
    // Build the name sets needed for type-pool resolution.
    let mut struct_names: BTreeSet<String> = BTreeSet::new();
    let mut enum_names: BTreeSet<String> = BTreeSet::new();
    for (_, ut) in &registry.types {
        match ut {
            UnifiedType::Struct(us) => {
                struct_names.insert(rustify_name(&us.ir.name));
            }
            UnifiedType::Enum(ue) => {
                enum_names.insert(rustify_name(&ue.ir.name));
            }
        }
    }

    // Discover every enum that some struct uses as a flat-union branch.
    let branch_enums = collect_branch_enums(registry);

    // Stable, deterministic order: case-insensitive alphabetical by Rust name.
    let mut entries: Vec<(&String, &UnifiedType)> = registry.types.iter().collect();
    entries.sort_by(|(_, a), (_, b)| {
        let an = match a {
            UnifiedType::Struct(us) => rustify_name(&us.ir.name),
            UnifiedType::Enum(ue) => rustify_name(&ue.ir.name),
        };
        let bn = match b {
            UnifiedType::Struct(us) => rustify_name(&us.ir.name),
            UnifiedType::Enum(ue) => rustify_name(&ue.ir.name),
        };
        an.to_lowercase().cmp(&bn.to_lowercase())
    });

    let mut pool = TypePool::new();

    // ── pass 1: emit all field/variant arrays + per-type metas ────────────
    let mut bodies = String::new();
    let mut struct_metas: Vec<(String, String)> = Vec::new(); // (rust_name, decl_name)
    let mut enum_metas: Vec<(String, String)> = Vec::new();
    let mut commands: Vec<CommandEmit> = Vec::new();
    // Track Rust names already emitted so duplicates (matching the qapi-codegen
    // dedup logic) don't produce duplicate static items.
    let mut seen_rust_names: BTreeSet<String> = BTreeSet::new();

    for (_, ut) in &entries {
        let rust_name = match ut {
            UnifiedType::Struct(us) => rustify_name(&us.ir.name),
            UnifiedType::Enum(ue) => rustify_name(&ue.ir.name),
        };
        if !seen_rust_names.insert(rust_name.clone()) {
            continue;
        }

        match ut {
            UnifiedType::Struct(us) => {
                let cat = classify(us);
                let se = emit_struct(us);
                bodies.push_str(&emit_field_array(
                    us,
                    &se.fields_var,
                    &mut pool,
                    &struct_names,
                    &enum_names,
                ));
                let decl = format!("STRUCT_{}", se.rust_name);
                bodies.push_str(&emit_struct_meta(&decl, &se));
                struct_metas.push((se.rust_name.clone(), decl));
                if cat == Category::Command {
                    commands.push(emit_command(us, &mut pool, &struct_names, &enum_names));
                }
                let _ = cat; // events stay only as StructMeta entries; CommandMeta is QMP-only.
            }
            UnifiedType::Enum(ue) => {
                let kind = if branch_enums.contains(&rust_name) {
                    "FlatUnionBranch"
                } else if ue
                    .ir
                    .variants
                    .iter()
                    .all(|v| matches!(v.kind, EnumVariantKind::Unit))
                {
                    "Unit"
                } else {
                    "Alternate"
                };
                let var_name = format!("VARIANTS_{}", rust_name);
                bodies.push_str(&emit_enum_variants(
                    ue,
                    &var_name,
                    &mut pool,
                    &struct_names,
                    &enum_names,
                ));
                let decl = format!("ENUM_{}", rust_name);
                let ee = EnumEmit {
                    rust_name: rust_name.clone(),
                    doc: ue.ir.meta.doc.clone().unwrap_or_default(),
                    variants_var: var_name,
                    kind,
                    versions: ue.versions.clone(),
                };
                bodies.push_str(&emit_enum_meta(&decl, &ee));
                enum_metas.push((rust_name.clone(), decl));
            }
        }
    }

    // ── header + type definitions ─────────────────────────────────────────
    let mut out = String::new();
    out.push_str("// AUTO-GENERATED by qapi-codegen — do not edit by hand.\n");
    out.push_str("#![allow(non_upper_case_globals, dead_code)]\n\n");
    out.push_str("use phf::phf_map;\n\n");
    out.push_str(METADATA_TYPES);
    out.push_str("\n");

    // ── type-ref pool ─────────────────────────────────────────────────────
    out.push_str(&pool.emit());
    out.push_str("\n");

    // ── per-type bodies ───────────────────────────────────────────────────
    out.push_str(&bodies);
    out.push_str("\n");

    // ── COMMANDS ──────────────────────────────────────────────────────────
    out.push_str("pub static COMMANDS: &[CommandMeta] = &[\n");
    let mut sorted_cmds: Vec<&CommandEmit> = commands.iter().collect();
    sorted_cmds.sort_by(|a, b| a.wire_name.cmp(&b.wire_name));
    for c in &sorted_cmds {
        out.push_str(&emit_command_meta(c));
    }
    out.push_str("];\n\n");

    // ── STRUCTS map ───────────────────────────────────────────────────────
    out.push_str("pub static STRUCTS: phf::Map<&'static str, &'static StructMeta> = phf_map! {\n");
    let mut struct_metas_sorted: Vec<(String, String)> = struct_metas;
    struct_metas_sorted.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, decl) in &struct_metas_sorted {
        out.push_str(&format!("    {} => &{},\n", rs_str(name), decl));
    }
    out.push_str("};\n\n");

    // ── ENUMS map ─────────────────────────────────────────────────────────
    out.push_str("pub static ENUMS: phf::Map<&'static str, &'static EnumMeta> = phf_map! {\n");
    let mut enum_metas_sorted: Vec<(String, String)> = enum_metas;
    enum_metas_sorted.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, decl) in &enum_metas_sorted {
        out.push_str(&format!("    {} => &{},\n", rs_str(name), decl));
    }
    out.push_str("};\n");

    out
}

/// Static type definitions that prefix the generated metadata file.
const METADATA_TYPES: &str = r#"pub type Ver = (u32, u32, u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prim { Str, Bool, I8, I16, I32, I64, U8, U16, U32, U64, F64, Null, Any }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind { Primitive, Enum, Struct, List }

#[derive(Debug)]
pub struct TypeRef {
    pub kind: TypeKind,
    pub name: &'static str,
    pub prim: Prim,
    pub inner: u32,
}

#[derive(Debug)]
pub struct FieldMeta {
    pub name: &'static str,
    pub wire_name: &'static str,
    pub ty: u32,
    /// Whether the active wire field may be absent, independently of Rust's version-merged Option.
    pub optional: bool,
    pub is_discriminator: bool,
    pub recursive: bool,
    pub since: Ver,
    pub until: Option<Ver>,
    pub deprecated: bool,
    pub unstable: bool,
    pub doc: &'static str,
}

#[derive(Debug)]
pub struct EnumVariantMeta {
    pub name: &'static str,
    pub wire_name: &'static str,
    /// Direct struct payload, if any; scalar, enum and list payloads use payload_type.
    pub payload_struct: Option<&'static str>,
    /// Index into TYPE_REFS, including null payloads; None means a unit variant.
    pub payload_type: Option<u32>,
    pub since: Ver,
    pub until: Option<Ver>,
    pub deprecated: bool,
    pub unstable: bool,
    pub doc: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnumKind { Unit, Alternate, FlatUnionBranch }

#[derive(Debug)]
pub struct EnumMeta {
    pub name: &'static str,
    pub doc: &'static str,
    pub variants: &'static [EnumVariantMeta],
    pub kind: EnumKind,
    pub since: Ver,
    pub until: Option<Ver>,
}

#[derive(Debug)]
pub struct StructMeta {
    pub name: &'static str,
    pub doc: &'static str,
    pub fields: &'static [FieldMeta],
    pub is_union: bool,
    pub discriminator_field: Option<&'static str>,
    pub branch_enum: Option<&'static str>,
    pub free_form: bool,
    pub since: Ver,
    pub until: Option<Ver>,
}

#[derive(Debug)]
pub struct CommandMeta {
    pub wire_name: &'static str,
    pub rust_name: &'static str,
    pub args: &'static StructMeta,
    pub returns: Option<u32>,
    pub allow_oob: bool,
    pub allow_preconfig: bool,
    pub success_response: bool,
    pub since: Ver,
    pub until: Option<Ver>,
    pub deprecated: bool,
    pub unstable: bool,
    pub doc: &'static str,
    pub category: &'static str,
}

"#;

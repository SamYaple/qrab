//! Multi-version QAPI schema merge.
//!
//! Parses schemas from multiple QEMU version tags, merges them into a single
//! unified type registry where each type, field, and variant has version range
//! annotations (since/until).

use crate::QapiSchemaToken;
use crate::qapi_ir::*;
use crate::versions::{
    extract_schema_from_tag, find_extracted_qga_schema_entry, find_extracted_schema_entry,
    parse_version_tag,
};
use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

pub type Version = (u32, u32, u32);

fn version_str(v: Version) -> String {
    format!("{}.{}.{}", v.0, v.1, v.2)
}

/// Classify a unified type as a command, event, or plain data type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeCategory {
    Command,
    Event,
    DataType,
}

/// Determine the category of a unified type by inspecting its IR attributes.
fn classify_type(ut: &UnifiedType) -> TypeCategory {
    let attrs = match ut {
        UnifiedType::Struct(us) => &us.ir.meta.attributes,
        UnifiedType::Enum(_) => return TypeCategory::DataType,
    };
    for attr in attrs {
        match attr {
            Attribute::Unit(n) if n == "command" => return TypeCategory::Command,
            Attribute::Unit(n) if n == "event" => return TypeCategory::Event,
            _ => {}
        }
    }
    TypeCategory::DataType
}

fn clean_doc(doc: &str) -> Option<String> {
    let doc = doc.trim_matches('\n');
    if doc.trim().is_empty() {
        return None;
    }
    Some(doc.to_string())
}

/// Clean a doc string field in place. If `doc` is `Some`, replaces it with
/// the cleaned value (which may become `None` when only whitespace remained).
/// `None` inputs stay `None`.
fn clean_doc_in_place(doc: &mut Option<String>) {
    *doc = doc.as_deref().and_then(clean_doc);
}

/// Update `dst` from `src` when `src` has a doc, applying [`clean_doc`].
/// If `src` is `None`, leaves `dst` unchanged (lets older docs survive
/// version merges where the newer schema dropped the documentation).
fn merge_doc_from(dst: &mut Option<String>, src: Option<&str>) {
    if let Some(s) = src {
        *dst = clean_doc(s);
    }
}

#[derive(Debug, Clone)]
pub struct VersionRange {
    pub since: Version,
    pub until: Option<Version>,
    pub deprecated: Option<Version>,
    pub unstable: Option<Version>,
}

impl VersionRange {
    pub fn new(since: Version) -> Self {
        Self {
            since,
            until: None,
            deprecated: None,
            unstable: None,
        }
    }
}

/// An enum with version-annotated variants.
#[derive(Debug, Clone)]
pub struct UnifiedEnum {
    pub ir: Enum,
    pub versions: VersionRange,
    /// Per-variant version ranges (keyed by variant name)
    pub variant_versions: BTreeMap<String, VersionRange>,
    /// Earlier payload shapes, retained before synthetic legacy variants are emitted.
    pub variant_history: Vec<(EnumVariant, VersionRange)>,
}

/// A struct with version-annotated fields.
#[derive(Debug, Clone)]
pub struct UnifiedStruct {
    pub ir: Struct,
    pub versions: VersionRange,
    /// Per-field version ranges (keyed by field name)
    pub field_versions: BTreeMap<String, VersionRange>,
    /// Earlier field payload types and wire spellings.
    pub field_history: Vec<(StructField, VersionRange)>,
    /// Per-attribute version ranges for command/event flags that can appear/disappear
    /// across versions (e.g. allow_preconfig, allow_oob, no_success_response, unstable).
    pub attr_versions: BTreeMap<String, VersionRange>,
    /// Command return types in version order: each entry is the first merged
    /// version that returned that Rust type. Generated code uses the newest
    /// entry, the current QEMU contract; older entries record the lifecycle.
    pub returns_history: Vec<(Version, String)>,
}

impl UnifiedStruct {
    /// The return type served by the newest merged version, for commands.
    pub fn returns(&self) -> Option<&str> {
        returns_attr(&self.ir.meta)
    }
}

fn returns_attr(meta: &Metadata) -> Option<&str> {
    meta.attributes.iter().find_map(|a| match a {
        Attribute::Item { name, value } if name == "returns" => Some(value.as_str()),
        _ => None,
    })
}

/// A command's return type changed: the newest version is authoritative,
/// and the change is recorded in `returns_history`.
fn merge_returns(existing: &mut UnifiedStruct, new_meta: &Metadata, version: Version) {
    let Some(new) = returns_attr(new_meta) else {
        return;
    };
    if existing.returns_history.last().map(|(_, r)| r.as_str()) != Some(new) {
        existing.returns_history.push((version, new.to_owned()));
    }
    // A struct first seen as plain data gains `returns` through the
    // ordinary attribute union, keeping attribute order stable.
    for attr in &mut existing.ir.meta.attributes {
        if let Attribute::Item { name, value } = attr {
            if name == "returns" {
                *value = new.to_owned();
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum UnifiedType {
    Enum(UnifiedEnum),
    Struct(UnifiedStruct),
}

/// Registry of all types seen across all versions.
#[derive(Debug, Default)]
pub struct UnifiedRegistry {
    pub types: BTreeMap<String, UnifiedType>,
    pub versions: Vec<Version>,
}

/// Return true when two attributes represent the "same" directive. For
/// directive attributes that can only appear once per item (`Unit`, `Item`,
/// `Nested`) same means "same name/key". For `List` attributes we compare by
/// structural equality since a list may carry multiple semantically distinct
/// bundles (e.g. multiple feature markers). Used to dedupe attribute sets
/// while merging struct metadata across versions.
fn attrs_equivalent(a: &Attribute, b: &Attribute) -> bool {
    match (a, b) {
        (Attribute::Unit(a), Attribute::Unit(b)) => a == b,
        (Attribute::Item { name: a, .. }, Attribute::Item { name: b, .. }) => a == b,
        (Attribute::Nested { name: a, .. }, Attribute::Nested { name: b, .. }) => a == b,
        (Attribute::List(a), Attribute::List(b)) => a == b,
        _ => false,
    }
}

fn condition(meta: &Metadata) -> Option<&str> {
    meta.attributes.iter().find_map(|attr| match attr {
        Attribute::Item { name, value } if name == "condition" => Some(value.as_str()),
        _ => None,
    })
}

fn refresh_condition(existing: &mut Metadata, latest: &Metadata) {
    if condition(existing) == condition(latest) {
        return;
    }
    existing
        .attributes
        .retain(|attr| !matches!(attr, Attribute::Item { name, .. } if name == "condition"));
    if let Some(value) = condition(latest) {
        existing
            .attributes
            .push(Attribute::with_value("condition", value));
    }
}

/// Merge command-specific attributes (command, returns, allow_oob, etc.) from
/// a processed command struct onto an existing data struct.
fn merge_command_attrs_onto(source: &Struct, target: &mut Struct) {
    // The argument struct's QAPI name is not the command's wire name.
    // Preserve the command spelling when the two share a Rust identifier.
    target
        .meta
        .attributes
        .retain(|attr| !matches!(attr, Attribute::Item { name, .. } if name == "name"));
    target
        .meta
        .attributes
        .push(Attribute::with_value("name", source.name.clone()));
    merge_doc_from(&mut target.meta.doc, source.meta.doc.as_deref());
    for attr in &source.meta.attributes {
        match attr {
            Attribute::Unit(n)
                if n == "command"
                    || n == "allow_oob"
                    || n == "allow_preconfig"
                    || n == "no_success_response" =>
            {
                if !target.meta.attributes.contains(attr) {
                    target.meta.attributes.push(attr.clone());
                }
            }
            Attribute::Item { name, .. } if name == "returns" => {
                if !target
                    .meta
                    .attributes
                    .iter()
                    .any(|a| matches!(a, Attribute::Item { name: n, .. } if n == "returns"))
                {
                    target.meta.attributes.push(attr.clone());
                }
            }
            _ => {}
        }
    }
    if !target
        .meta
        .attributes
        .iter()
        .any(|a| matches!(a, Attribute::Unit(n) if n == "command"))
    {
        target.meta.attributes.push(Attribute::new("command"));
    }
}

/// Process a single version's schema through the IR pipeline.
/// Returns maps of enum name → Enum IR and struct name → Struct IR.
#[cfg(test)]
fn process_version_schema(
    sources: &[(std::path::PathBuf, String)],
) -> Result<(HashMap<String, Enum>, HashMap<String, Struct>)> {
    process_version_schema_with_names(sources, None)
}

fn process_version_schema_with_names(
    sources: &[(std::path::PathBuf, String)],
    shared: Option<&crate::symbols::SchemaNames>,
) -> Result<(HashMap<String, Enum>, HashMap<String, Struct>)> {
    let mut enums_lookup = HashMap::new();
    let mut structs_lookup = HashMap::new();
    let mut unprocessed_structs = Vec::new();
    let mut unprocessed_unions = Vec::new();
    let mut unprocessed_commands = Vec::new();
    let mut unprocessed_events = Vec::new();
    let mut definitions = HashMap::new();
    let schemas = crate::builtins::parse_sources(sources)?;
    crate::schema_validation::validate(&schemas)?;
    let declared = crate::symbols::SchemaNames::declared_names(&schemas);
    let source_paths: HashMap<_, _> = schemas
        .iter()
        .flat_map(|(path, schema)| {
            schema.0.iter().filter_map(move |token| match token {
                QapiSchemaToken::Struct(value) => Some((value.name, *path)),
                QapiSchemaToken::Union(value) => Some((value.name, *path)),
                _ => None,
            })
        })
        .collect();
    let mut local = crate::symbols::SchemaNames::default();
    let symbols = if let Some(shared) = shared {
        shared
    } else {
        for (_, schema) in &schemas {
            local.collect(schema);
        }
        local.finish();
        &local
    };
    let mut names = symbols.names.clone();
    let mut union_names = HashMap::new();
    let mut event_names = HashMap::new();
    let mut command_names = HashMap::new();
    for (_, schema) in &schemas {
        for token in &schema.0 {
            match token {
                QapiSchemaToken::Union(value) => {
                    let root = symbols.alias_name(value.name);
                    let branch = symbols.role(&format!("{root}Branch"));
                    let discriminator = symbols.role(&format!("{root}Type"));
                    union_names.insert(value.name, (branch, discriminator));
                }
                QapiSchemaToken::Event(value) => {
                    let name = symbols.role(&rustify_name(&format!("{}Event", value.name)));
                    event_names.insert(value.name, name);
                }
                QapiSchemaToken::Command(value) => {
                    let name = symbols.role(&format!("{}Command", rustify_name(value.name)));
                    command_names.insert(value.name, name);
                }
                _ => {}
            }
        }
    }

    // Resolve exact QAPI names before assigning Rust identities for merging.
    for (path, schema) in schemas {
        for token in schema.0 {
            let definition = match &token {
                QapiSchemaToken::Enum(value) => Some(("type", value.name)),
                QapiSchemaToken::Alternate(value) => Some(("type", value.name)),
                QapiSchemaToken::Struct(value) => Some(("type", value.name)),
                QapiSchemaToken::Union(value) => Some(("type", value.name)),
                QapiSchemaToken::Event(value) => Some(("event", value.name)),
                QapiSchemaToken::Command(value) => Some(("command", value.name)),
                _ => None,
            };
            if let Some((namespace, name)) = definition {
                let key = (
                    namespace,
                    if namespace == "type" {
                        name.to_owned()
                    } else {
                        rustify_name(name)
                    },
                );
                if let Some(previous) = definitions.insert(key.clone(), name) {
                    anyhow::bail!(
                        "Conflicting QAPI definitions {previous} and {name} share a {namespace} symbol {} in {}",
                        key.1,
                        path.display()
                    );
                }
            }
            match token {
                QapiSchemaToken::Enum(v) => {
                    let processed = process_enum(v);
                    enums_lookup.insert(processed.name.clone(), processed);
                }
                QapiSchemaToken::Alternate(v) => {
                    let processed = process_alternate(v);
                    enums_lookup.insert(processed.name.clone(), processed);
                }
                QapiSchemaToken::Struct(v) => {
                    if v.base.is_some() {
                        unprocessed_structs.push(v);
                        continue;
                    }
                    let (processed, extracted) =
                        process_struct_with_names(v, &structs_lookup, &mut names);
                    for value in std::iter::once(&processed).chain(extracted.iter()) {
                        crate::schema_validation::ir_members(&value.fields).with_context(|| {
                            format!("Invalid QAPI struct '{}' in {}", value.name, path.display())
                        })?;
                    }
                    structs_lookup.insert(processed.name.clone(), processed);
                    for e in extracted {
                        structs_lookup.insert(e.name.clone(), e);
                    }
                }
                QapiSchemaToken::Union(value) if value.legacy_alternate => {
                    let processed = process_alternate(crate::QapiAlternate {
                        name: value.name,
                        data: crate::QapiAlternatives(value.data.0),
                        r#if: value.r#if,
                        features: value.features,
                        doc: value.doc,
                    });
                    enums_lookup.insert(processed.name.clone(), processed);
                }
                QapiSchemaToken::Union(v) => {
                    unprocessed_unions.push(v);
                }
                QapiSchemaToken::Command(v) => {
                    unprocessed_commands.push(v);
                }
                QapiSchemaToken::Event(v) => {
                    unprocessed_events.push(v);
                }
                _ => continue,
            }
        }
    }

    // A derived struct may precede its base, including across include files.
    while !unprocessed_structs.is_empty() {
        let count = unprocessed_structs.len();
        let mut pending = Vec::new();
        for value in unprocessed_structs {
            let base = value.base.expect("only derived structs are deferred");
            if !structs_lookup.contains_key(base) {
                pending.push(value);
                continue;
            }
            let path = source_paths[value.name];
            let (processed, extracted) =
                process_struct_with_names(value, &structs_lookup, &mut names);
            for value in std::iter::once(&processed).chain(extracted.iter()) {
                crate::schema_validation::ir_members(&value.fields).with_context(|| {
                    format!("Invalid QAPI struct '{}' in {}", value.name, path.display())
                })?;
            }
            structs_lookup.insert(processed.name.clone(), processed);
            for value in extracted {
                structs_lookup.insert(value.name.clone(), value);
            }
        }
        anyhow::ensure!(
            pending.len() < count,
            "Unresolved QAPI struct bases (missing definition or inheritance cycle): {}",
            pending
                .iter()
                .map(|value| format!("{} -> {}", value.name, value.base.unwrap()))
                .collect::<Vec<_>>()
                .join(", ")
        );
        unprocessed_structs = pending;
    }
    for v in unprocessed_unions {
        if let crate::MembersOrRef::Ref(base) = &v.base {
            anyhow::ensure!(
                structs_lookup.contains_key(*base),
                "QAPI union {} references missing base {base}",
                v.name
            );
        }
        if let Some(discriminator) = v.discriminator {
            let base_fields = match &v.base {
                crate::MembersOrRef::Ref(base) => structs_lookup[*base].fields.clone(),
                crate::MembersOrRef::Members(members) => {
                    members.iter().cloned().map(process_member).collect()
                }
                crate::MembersOrRef::Unset => Vec::new(),
            };
            for branch in &v.data.0 {
                if let crate::QapiTypeRef::Ref(name) = &branch.r#type {
                    if let Some(payload) = structs_lookup.get(*name) {
                        crate::schema_validation::ir_members(
                            base_fields.iter().chain(&payload.fields),
                        )
                        .with_context(|| {
                            format!(
                                "Invalid QAPI union '{}' branch '{}' in {}",
                                v.name,
                                branch.name,
                                source_paths[v.name].display()
                            )
                        })?;
                    }
                }
            }
            let (discriminator_type, optional, array) = match &v.base {
                crate::MembersOrRef::Ref(base) => structs_lookup[*base]
                    .fields
                    .iter()
                    .find(|field| field.name == discriminator)
                    .map(|field| (field.r#type.clone(), field.optional, field.array)),
                crate::MembersOrRef::Members(members) => members
                    .iter()
                    .find(|field| field.name == discriminator)
                    .map(|field| {
                        let field = process_member(field.clone());
                        (field.r#type, field.optional, field.array)
                    }),
                crate::MembersOrRef::Unset => None,
            }
            .with_context(|| {
                format!(
                    "QAPI union {} has no discriminator field {discriminator} in its base",
                    v.name
                )
            })?;
            anyhow::ensure!(
                !optional && !array,
                "QAPI union {} discriminator {discriminator} must be a required scalar enum",
                v.name
            );
            for branch in &v.data.0 {
                anyhow::ensure!(
                    !matches!(branch.r#type, crate::QapiTypeRef::ArrayRef(_)),
                    "QAPI flat union {} branch {} must reference a struct, not an array",
                    v.name,
                    branch.name
                );
                if crate::parser_version() >= (6, 2, 0) {
                    anyhow::ensure!(
                        matches!(&branch.r#type, crate::QapiTypeRef::Ref(name) if structs_lookup.contains_key(*name)),
                        "QAPI flat union {} branch {} must reference a struct",
                        v.name,
                        branch.name
                    );
                }
            }
            if let Some(tags) = enums_lookup.get(&discriminator_type) {
                anyhow::ensure!(
                    !tags.variants.is_empty(),
                    "QAPI union {} has no branches",
                    v.name
                );
                for branch in &v.data.0 {
                    anyhow::ensure!(
                        tags.variants.iter().any(|tag| tag.name == branch.name),
                        "QAPI union {} branch {} is not a value of discriminator enum {discriminator_type}",
                        v.name,
                        branch.name
                    );
                }
            } else {
                anyhow::ensure!(
                    crate::parser_version() < (6, 2, 0),
                    "QAPI union {} discriminator {discriminator} has non-enum type {discriminator_type}",
                    v.name
                );
            }
        }
        let (branch_name, discriminator_name) = &union_names[v.name];
        let (mut s, mut e, disc_enum) = process_union(v, &structs_lookup, &enums_lookup);
        e.name.clone_from(branch_name);
        for field in &mut s.fields {
            if field.meta.attributes.contains(&Attribute::new("flatten")) {
                field.r#type.clone_from(branch_name);
            }
        }
        if let Some(mut de) = disc_enum {
            for field in &mut s.fields {
                if field
                    .meta
                    .attributes
                    .contains(&Attribute::new("discriminator"))
                {
                    field.r#type.clone_from(discriminator_name);
                }
            }
            de.name.clone_from(discriminator_name);
            enums_lookup.insert(de.name.clone(), de);
        }
        enums_lookup.insert(e.name.clone(), e);
        structs_lookup.insert(s.name.clone(), s);
    }
    for v in unprocessed_events {
        if let Some(crate::MembersOrRef::Ref(data)) = &v.data {
            anyhow::ensure!(
                structs_lookup.contains_key(*data),
                "QAPI event {} references missing argument type {data}",
                v.name
            );
        }
        let event_name = &event_names[v.name];
        let mut processed = process_event(v, &structs_lookup);
        processed.name.clone_from(event_name);
        structs_lookup.insert(processed.name.clone(), processed);
    }
    let symbol_name = |name: &str| {
        if declared.contains(name) {
            symbols.type_name(name)
        } else {
            rustify_name(name)
        }
    };
    let mut occupied = symbols.data_symbols();
    occupied.extend(
        structs_lookup
            .values()
            .map(|value| symbol_name(&value.name)),
    );
    occupied.extend(enums_lookup.values().map(|value| symbol_name(&value.name)));
    for v in unprocessed_commands {
        if let Some(crate::MembersOrRef::Ref(data)) = &v.data {
            anyhow::ensure!(
                structs_lookup.contains_key(*data),
                "QAPI command {} references missing argument type {data}",
                v.name
            );
        }
        // Check if this command references an existing struct via data
        let data_ref_name = match &v.data {
            Some(crate::MembersOrRef::Ref(name)) => Some(name.to_string()),
            _ => None,
        };

        let command_name = &command_names[v.name];
        let returns = v.returns.as_ref().and_then(|value| {
            let (name, array) = match value {
                crate::QapiTypeRef::Ref(name) => (*name, false),
                crate::QapiTypeRef::ArrayRef(name) => (*name, true),
                _ => return None,
            };
            let name = if declared.contains(name) {
                symbols.type_name(name)
            } else {
                rustify_type(name)
            };
            Some(if array { format!("Vec<{name}>") } else { name })
        });
        let mut processed = process_command(v, &structs_lookup);
        if let Some(returns) = returns {
            for attr in &mut processed.meta.attributes {
                if let Attribute::Item { name, value } = attr {
                    if name == "returns" {
                        value.clone_from(&returns);
                    }
                }
            }
        }
        let mut cmd_key = rustify_name(&processed.name);

        if let Some(ref ref_name) = data_ref_name {
            let ref_key = rustify_name(ref_name);
            if cmd_key == ref_key {
                // Command and its data ref share the same rustified name
                // (e.g. command `blockdev-snapshot-sync` → struct
                // `BlockdevSnapshotSync`). Merge command attributes onto the
                // existing data struct -- a single entry represents both.
                if let Some(existing) = structs_lookup.get_mut(ref_name) {
                    merge_command_attrs_onto(&processed, existing);
                    continue;
                }
            }
            // Different rustified names (e.g. `netdev_add` → `NetdevAdd`,
            // data ref `Netdev` → `Netdev`): the command gets its own entry
            // under its rustified key. The data struct stays under its key.
        }

        // A command and its *response* can rustify to the same identifier
        // (guest-file-read / GuestFileRead, guest-exec / GuestExec). Preserve
        // the data type and give the request an explicit Command suffix.
        // Otherwise Returns resolves to the request itself and decoding
        // silently loses response fields or fails on missing arguments.
        if occupied.contains(&cmd_key) {
            processed
                .meta
                .attributes
                .retain(|attr| !matches!(attr, Attribute::Item { name, .. } if name == "name"));
            processed
                .meta
                .attributes
                .push(Attribute::with_value("name", processed.name.clone()));
            processed.name.clone_from(command_name);
            cmd_key = rustify_name(&processed.name);
            anyhow::ensure!(
                !occupied.contains(&cmd_key),
                "Conflicting command type: {cmd_key}"
            );
        }
        occupied.insert(cmd_key.clone());
        structs_lookup.insert(cmd_key, processed);
    }

    symbols.rewrite_types(&declared, &mut enums_lookup, &mut structs_lookup);
    Ok((
        enums_lookup
            .into_values()
            .map(|value| (rustify_name(&value.name), value))
            .collect(),
        structs_lookup
            .into_values()
            .map(|value| (rustify_name(&value.name), value))
            .collect(),
    ))
}

/// Result of extracting and parsing a single version's schema.
struct VersionResult {
    tag: String,
    version: Version,
    /// Hash of the raw schema content (for dedup of identical patch releases).
    schema_hash: u64,
    enums: HashMap<String, Enum>,
    structs: HashMap<String, Struct>,
}

fn normalize_member_names(versions: &mut [VersionResult]) {
    #[derive(Default)]
    struct Group {
        identities: Vec<(String, bool)>,
        collision: bool,
    }
    #[derive(Default)]
    struct Groups {
        groups: BTreeMap<(String, String), Group>,
        reserved: BTreeMap<String, BTreeSet<String>>,
    }
    impl Groups {
        fn observe(&mut self, owner: &str, members: impl Iterator<Item = (String, String, bool)>) {
            let mut local: BTreeMap<String, Vec<_>> = BTreeMap::new();
            for (raw, preferred, virtual_field) in members {
                local
                    .entry(preferred)
                    .or_default()
                    .push((raw, virtual_field));
            }
            for (preferred, identities) in local {
                self.reserved
                    .entry(owner.to_owned())
                    .or_default()
                    .insert(preferred.clone());
                let group = self
                    .groups
                    .entry((owner.to_owned(), preferred))
                    .or_default();
                group.collision |= identities.len() > 1;
                for identity in identities {
                    if !group.identities.contains(&identity) {
                        group.identities.push(identity);
                    }
                }
            }
        }
        fn allocate(mut self, suffix: &str) -> BTreeMap<(String, (String, bool)), String> {
            let mut aliases = BTreeMap::new();
            for ((owner, preferred), mut group) in self.groups {
                if !group.collision {
                    continue;
                }
                // Declared wire fields keep priority over virtual flatten receivers.
                group
                    .identities
                    .sort_by_key(|(_, virtual_field)| *virtual_field);
                let used = self.reserved.get_mut(&owner).unwrap();
                for identity in group.identities.into_iter().skip(1) {
                    let base = format!("{preferred}{suffix}");
                    let mut name = base.clone();
                    let mut number = 2;
                    while !used.insert(name.clone()) {
                        name = format!("{base}{number}");
                        number += 1;
                    }
                    aliases.insert((owner.clone(), identity), name);
                }
            }
            aliases
        }
    }
    let mut fields = Groups::default();
    let mut variants = Groups::default();
    let mut ordered: Vec<_> = versions.iter().collect();
    ordered.sort_by_key(|version| {
        std::cmp::Reverse(crate::versions::parse_version_tag_full(&version.tag).unwrap())
    });
    for version in ordered {
        for (owner, value) in &version.structs {
            fields.observe(
                owner,
                value.fields.iter().map(|field| {
                    (
                        field.name.clone(),
                        field_rust_name(field),
                        field.meta.attributes.contains(&Attribute::new("flatten")),
                    )
                }),
            );
        }
        for (owner, value) in &version.enums {
            variants.observe(
                owner,
                value
                    .variants
                    .iter()
                    .map(|variant| (variant.name.clone(), variant_rust_name(variant), false)),
            );
        }
    }
    let fields = fields.allocate("_alt");
    let variants = variants.allocate("Alt");
    if fields.is_empty() && variants.is_empty() {
        return;
    }
    let rename = |raw: &mut String, meta: &mut Metadata, assigned: &String| {
        if !meta
            .attributes
            .iter()
            .any(|attr| matches!(attr, Attribute::Item { name, .. } if name == "name"))
        {
            meta.attributes.push(Attribute::with_value("name", &*raw));
        }
        raw.clone_from(assigned);
    };
    for version in versions {
        for (owner, value) in &mut version.structs {
            for field in &mut value.fields {
                let identity = (
                    field.name.clone(),
                    field.meta.attributes.contains(&Attribute::new("flatten")),
                );
                if let Some(name) = fields.get(&(owner.clone(), identity)) {
                    rename(&mut field.name, &mut field.meta, name);
                }
            }
        }
        for (owner, value) in &mut version.enums {
            for variant in &mut value.variants {
                if let Some(name) = variants.get(&(owner.clone(), (variant.name.clone(), false))) {
                    rename(&mut variant.name, &mut variant.meta, name);
                }
            }
        }
    }
}

/// Bound concurrent git processes and parser workers even on hosts with large
/// CPU counts. Keep input order so errors and merges are reproducible.
fn parse_versions<T: Send>(
    tags: &[&str],
    parse: impl Fn(&str) -> Result<T> + Sync,
) -> Result<Vec<T>> {
    let workers = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(8);
    let mut results = Vec::with_capacity(tags.len());
    for batch in tags.chunks(workers) {
        std::thread::scope(|scope| -> Result<()> {
            let handles: Vec<_> = batch
                .iter()
                .map(|tag| {
                    let parse = &parse;
                    (*tag, scope.spawn(move || parse(tag)))
                })
                .collect();
            for (tag, handle) in handles {
                let result = handle
                    .join()
                    .map_err(|_| anyhow::anyhow!("Parser worker panicked for {tag}"))?;
                results.push(result.with_context(|| format!("Failed to process {tag}"))?);
            }
            Ok(())
        })?;
    }
    Ok(results)
}

struct VersionSources {
    tag: String,
    version: Version,
    schema_hash: u64,
    sources: Vec<(std::path::PathBuf, String)>,
}

fn extract_version_sources(
    qemu_repo: &Path,
    tag: &str,
    qga: bool,
) -> Result<Option<VersionSources>> {
    let version = parse_version_tag(tag).with_context(|| format!("Invalid version tag: {tag}"))?;
    let tmpdir = extract_schema_from_tag(qemu_repo, tag)
        .with_context(|| format!("Failed to extract schema for {tag}"))?;
    let entry = if qga {
        find_extracted_qga_schema_entry(tmpdir.path())
    } else {
        find_extracted_schema_entry(tmpdir.path())
    };
    let first_schema = if qga { (0, 15, 0) } else { (1, 0, 0) };
    let schema_entry = match entry {
        Some(entry) => entry,
        None if version < first_schema => return Ok(None),
        None => anyhow::bail!(
            "No {} schema entry point found for {tag}",
            if qga { "QGA" } else { "QMP" }
        ),
    };
    let sources = crate::read_schema_with_version(&schema_entry, Some(version))
        .with_context(|| format!("Failed to read schema for {tag}"))?;
    let schema_hash = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        crate::builtins::model(version).hash(&mut hasher);
        if qga {
            hasher.write(b"qga");
        }
        for (path, content) in &sources {
            path.file_name().hash(&mut hasher);
            content.hash(&mut hasher);
        }
        hasher.finish()
    };
    Ok(Some(VersionSources {
        tag: tag.into(),
        version,
        schema_hash,
        sources,
    }))
}

fn load_versions(
    qemu_repo: &Path,
    tags: &[&str],
    qga: bool,
) -> Result<(Vec<VersionResult>, usize)> {
    let extracted = parse_versions(tags, |tag| extract_version_sources(qemu_repo, tag, qga))?;
    let absent = extracted.iter().filter(|version| version.is_none()).count();
    let sources: Vec<_> = extracted.into_iter().flatten().collect();
    let by_tag: HashMap<_, _> = sources
        .iter()
        .map(|source| (source.tag.as_str(), source))
        .collect();
    let plans = parse_versions(tags, |tag| {
        let Some(source) = by_tag.get(tag) else {
            return Ok(None);
        };
        crate::set_parser_version(source.version);
        let mut symbols = crate::symbols::SchemaNames::default();
        for (_, schema) in crate::builtins::parse_sources(&source.sources)? {
            symbols.collect(&schema);
        }
        Ok(Some(symbols))
    })?;
    let mut symbols = crate::symbols::SchemaNames::default();
    for plan in plans.into_iter().flatten() {
        symbols.merge(plan);
    }
    symbols.finish();
    let results = parse_versions(tags, |tag| {
        let Some(source) = by_tag.get(tag) else {
            return Ok(None);
        };
        crate::set_parser_version(source.version);
        let (enums, structs) = process_version_schema_with_names(&source.sources, Some(&symbols))
            .with_context(|| format!("Failed to process IR for {tag}"))?;
        Ok(Some(VersionResult {
            tag: source.tag.clone(),
            version: source.version,
            schema_hash: source.schema_hash,
            enums,
            structs,
        }))
    })?;
    let mut results: Vec<_> = results.into_iter().flatten().collect();
    normalize_member_names(&mut results);
    Ok((results, absent))
}

/// Parse schemas from multiple versions and build a unified registry.
/// Extraction and parsing run in parallel across versions; merging is sequential.
pub fn build_unified_registry(qemu_repo: &Path, version_tags: &[&str]) -> Result<UnifiedRegistry> {
    use std::time::Instant;
    let start = Instant::now();

    // Phase 1: Extract + parse all versions in parallel
    eprintln!(
        "Extracting and parsing {} versions in parallel...",
        version_tags.len()
    );
    let (mut version_results, _) = load_versions(qemu_repo, version_tags, false)?;

    eprintln!(
        "  Extraction + parsing: {:.1}s",
        start.elapsed().as_secs_f64()
    );

    // Sort by version to ensure deterministic merge order
    version_results
        .sort_by_key(|result| crate::versions::parse_version_tag_full(&result.tag).unwrap());

    // Phase 2: Merge sequentially in version order
    let merge_start = Instant::now();
    let mut registry = UnifiedRegistry::default();
    let mut prev_hash: Option<u64> = None;
    let mut skipped = 0;

    for vr in &version_results {
        registry.versions.push(vr.version);

        // Skip versions with identical schemas (patch releases)
        if prev_hash == Some(vr.schema_hash) {
            skipped += 1;
            continue;
        }
        prev_hash = Some(vr.schema_hash);

        let mut seen_types: BTreeSet<&str> = BTreeSet::new();

        for (name, e) in &vr.enums {
            seen_types.insert(name.as_str());
            merge_enum(&mut registry, name, e, vr.version);
        }

        for (name, s) in &vr.structs {
            seen_types.insert(name.as_str());
            merge_struct(&mut registry, name, s, vr.version);
        }

        for (name, ut) in registry.types.iter_mut() {
            if !seen_types.contains(name.as_str()) {
                let vr_inner = match ut {
                    UnifiedType::Enum(e) => &mut e.versions,
                    UnifiedType::Struct(s) => &mut s.versions,
                };
                if vr_inner.until.is_none() && vr_inner.since < vr.version {
                    vr_inner.until = Some(vr.version);
                }
            }
        }
    }

    if skipped > 0 {
        eprintln!("  Skipped {} versions with identical schemas", skipped);
    }
    eprintln!("  Merge loop: {:.1}s", merge_start.elapsed().as_secs_f64());
    let fix_start = Instant::now();
    materialize_variant_history(&mut registry);
    materialize_field_history(&mut registry);
    fix_recursive_types(&mut registry);
    eprintln!(
        "  fix_recursive_types: {:.1}s",
        fix_start.elapsed().as_secs_f64()
    );

    let total = registry.types.len();
    let removed = registry
        .types
        .values()
        .filter(|t| match t {
            UnifiedType::Enum(e) => e.versions.until.is_some(),
            UnifiedType::Struct(s) => s.versions.until.is_some(),
        })
        .count();
    eprintln!("  Merge: {:.1}s", merge_start.elapsed().as_secs_f64());
    eprintln!(
        "  Total: {:.1}s — {} types, {} removed",
        start.elapsed().as_secs_f64(),
        total,
        removed
    );

    Ok(registry)
}

/// Check if an IR node has a named feature (e.g. `deprecated`, `unstable`)
/// in its QAPI `features` list.
fn has_feature(meta: &Metadata, feature_name: &str) -> bool {
    meta.attributes.iter().any(|a| match a {
        Attribute::List(items) => items.iter().any(|item| {
            matches!(item,
                Attribute::Item { name, value } if name == "feature" && value == feature_name
            )
        }),
        _ => false,
    })
}

/// Boolean schema properties whose presence changes between releases.
const TRACKED_BOOL_ATTRS: &[&str] = &[
    "allow_preconfig",
    "allow_oob",
    "no_success_response",
    "simple_union",
];

/// Extract version ranges for boolean command attributes present in metadata.
fn extract_bool_attr_versions(meta: &Metadata, version: Version) -> BTreeMap<String, VersionRange> {
    let mut map = BTreeMap::new();
    for attr_name in TRACKED_BOOL_ATTRS {
        if meta
            .attributes
            .iter()
            .any(|a| matches!(a, Attribute::Unit(n) if n == *attr_name))
        {
            map.insert(attr_name.to_string(), VersionRange::new(version));
        }
    }
    map
}

/// Merge boolean command attributes from a new version's struct into an existing UnifiedStruct.
/// Adds newly-appearing attributes, marks removed ones with `until`.
fn merge_bool_attrs(existing: &mut UnifiedStruct, new_meta: &Metadata, version: Version) {
    for attr_name in TRACKED_BOOL_ATTRS {
        let present_in_new = new_meta
            .attributes
            .iter()
            .any(|a| matches!(a, Attribute::Unit(n) if n == *attr_name));

        if present_in_new {
            if existing.attr_versions.contains_key(*attr_name) {
                // Already tracked — clear until (it's still present)
                existing.attr_versions.get_mut(*attr_name).unwrap().until = None;
            } else {
                // Newly appeared in this version — add to IR and track
                existing
                    .attr_versions
                    .insert(attr_name.to_string(), VersionRange::new(version));
                let attr = Attribute::new(attr_name);
                if !existing.ir.meta.attributes.contains(&attr) {
                    existing.ir.meta.attributes.push(attr);
                }
            }
        } else if let Some(vr) = existing.attr_versions.get_mut(*attr_name) {
            // Was present before but not in this version — mark removed
            if vr.until.is_none() && vr.since < version {
                vr.until = Some(version);
            }
        }
    }
}

/// Return the Rust name for a `UnifiedType`, regardless of whether it's an
/// enum or struct. Used as a sort key and for other name-based lookups.
fn unified_type_rust_name(ut: &UnifiedType) -> String {
    match ut {
        UnifiedType::Enum(ue) => rustify_name(&ue.ir.name),
        UnifiedType::Struct(us) => rustify_name(&us.ir.name),
    }
}

/// Map from QAPI variant name to its Rust name (for detecting renames).
fn variant_rust_name(v: &EnumVariant) -> String {
    crate::qapi_ir::rustify_variant(&v.name)
}

fn same_variant_shape(left: &EnumVariant, right: &EnumVariant) -> bool {
    left.array == right.array
        && match (&left.kind, &right.kind) {
            (EnumVariantKind::Tuple(left), EnumVariantKind::Tuple(right)) => {
                rustify_type(left) == rustify_type(right)
            }
            (left, right) => left == right,
        }
}

/// Materialize old payloads only after merging, so synthetic names never look like schema renames.
fn materialize_variant_history(registry: &mut UnifiedRegistry) {
    for value in registry.types.values_mut() {
        let UnifiedType::Enum(value) = value else {
            continue;
        };
        let mut used: BTreeSet<_> = value.ir.variants.iter().map(variant_rust_name).collect();
        for (historical, versions) in &value.variant_history {
            let mut variant = historical.clone();
            let wire = variant
                .meta
                .attributes
                .iter()
                .find_map(|attr| match attr {
                    Attribute::Item { name, value } if name == "name" => Some(value.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| variant.name.clone());
            let base = format!("{}Legacy", variant_rust_name(&variant));
            let mut name = base.clone();
            let mut suffix = 2;
            while !used.insert(name.clone()) {
                name = format!("{base}{suffix}");
                suffix += 1;
            }
            variant.name = name.clone();
            variant
                .meta
                .attributes
                .retain(|attr| !matches!(attr, Attribute::Item { name, .. } if name == "name"));
            variant
                .meta
                .attributes
                .push(Attribute::with_value("name", wire));
            value.variant_versions.insert(name, versions.clone());
            value.ir.variants.push(variant);
        }
    }
}

fn merge_enum(registry: &mut UnifiedRegistry, name: &str, e: &Enum, version: Version) {
    if let Some(UnifiedType::Enum(existing)) = registry.types.get_mut(name) {
        existing.versions.until = None;
        // Track type-level deprecation
        if has_feature(&e.meta, "deprecated") && existing.versions.deprecated.is_none() {
            existing.versions.deprecated = Some(version);
        }
        // Track type-level unstable
        if has_feature(&e.meta, "unstable") && existing.versions.unstable.is_none() {
            existing.versions.unstable = Some(version);
        }
        // Update doc from latest version (latest has best docs)
        merge_doc_from(&mut existing.ir.meta.doc, e.meta.doc.as_deref());
        refresh_condition(&mut existing.ir.meta, &e.meta);
        // Union enum-level attributes across versions.
        for new_attr in &e.meta.attributes {
            if !existing
                .ir
                .meta
                .attributes
                .iter()
                .any(|a| attrs_equivalent(a, new_attr))
            {
                existing.ir.meta.attributes.push(new_attr.clone());
            }
        }

        let mut seen_variants = BTreeSet::new();
        for variant in &e.variants {
            seen_variants.insert(variant.name.clone());
            if existing.variant_versions.contains_key(&variant.name) {
                let old = existing
                    .ir
                    .variants
                    .iter()
                    .find(|old| old.name == variant.name)
                    .unwrap();
                if !same_variant_shape(old, variant)
                    || existing.variant_versions[&variant.name].until.is_some()
                {
                    let mut old_versions = existing.variant_versions[&variant.name].clone();
                    old_versions.until = old_versions.until.or(Some(version));
                    existing.variant_history.push((old.clone(), old_versions));
                    existing
                        .variant_versions
                        .insert(variant.name.clone(), VersionRange::new(version));
                }
                // Variant still exists — clear until, track deprecation.
                let vr = existing.variant_versions.get_mut(&variant.name).unwrap();
                vr.until = None;
                if has_feature(&variant.meta, "deprecated") && vr.deprecated.is_none() {
                    vr.deprecated = Some(version);
                }
                // Promote the variant to the latest version's definition:
                // tuple payload type and array marker track the newest
                // schema; attributes are unioned with the existing set;
                // doc keeps existing unless newer has one.
                if let Some(existing_variant) = existing
                    .ir
                    .variants
                    .iter_mut()
                    .find(|v| v.name == variant.name)
                {
                    existing_variant.kind = variant.kind.clone();
                    existing_variant.array = variant.array;
                    merge_doc_from(&mut existing_variant.meta.doc, variant.meta.doc.as_deref());
                    refresh_condition(&mut existing_variant.meta, &variant.meta);
                    for new_attr in &variant.meta.attributes {
                        if !existing_variant
                            .meta
                            .attributes
                            .iter()
                            .any(|a| attrs_equivalent(a, new_attr))
                        {
                            existing_variant.meta.attributes.push(new_attr.clone());
                        }
                    }
                }
            } else {
                // Check if this is a rename (same Rust name, different QAPI name)
                let rust_name = variant_rust_name(variant);
                let renamed_from = existing
                    .ir
                    .variants
                    .iter()
                    .position(|v| variant_rust_name(v) == rust_name && v.name != variant.name);

                if let Some(old_idx) = renamed_from {
                    let old = existing.ir.variants[old_idx].clone();
                    let mut old_versions = existing.variant_versions.remove(&old.name).unwrap();
                    old_versions.until = old_versions.until.or(Some(version));
                    existing.variant_history.push((old, old_versions));
                    let mut value = variant.clone();
                    clean_doc_in_place(&mut value.meta.doc);
                    existing.ir.variants[old_idx] = value;
                    existing
                        .variant_versions
                        .insert(variant.name.clone(), VersionRange::new(version));
                } else {
                    // Genuinely new variant
                    existing
                        .variant_versions
                        .insert(variant.name.clone(), VersionRange::new(version));
                    if !existing.ir.variants.iter().any(|v| v.name == variant.name) {
                        let mut v = variant.clone();
                        clean_doc_in_place(&mut v.meta.doc);
                        existing.ir.variants.push(v);
                    }
                }
            }
        }

        // Mark variants NOT seen in this version
        for (vname, vr) in existing.variant_versions.iter_mut() {
            if !seen_variants.contains(vname) && vr.until.is_none() && vr.since < version {
                vr.until = Some(version);
            }
        }
    } else {
        let variant_versions: BTreeMap<String, VersionRange> = e
            .variants
            .iter()
            .map(|v| (v.name.clone(), VersionRange::new(version)))
            .collect();
        let mut ir = e.clone();
        clean_doc_in_place(&mut ir.meta.doc);
        for v in &mut ir.variants {
            clean_doc_in_place(&mut v.meta.doc);
        }
        let mut versions = VersionRange::new(version);
        if has_feature(&ir.meta, "unstable") {
            versions.unstable = Some(version);
        }
        if has_feature(&ir.meta, "deprecated") {
            versions.deprecated = Some(version);
        }
        registry.types.insert(
            name.to_string(),
            UnifiedType::Enum(UnifiedEnum {
                ir,
                versions,
                variant_versions,
                variant_history: Vec::new(),
            }),
        );
    }
}

fn field_rust_name(f: &StructField) -> String {
    crate::qapi_ir::rustify_field(&f.name)
}

fn same_field_shape(left: &StructField, right: &StructField) -> bool {
    rustify_type(&left.r#type) == rustify_type(&right.r#type)
        && left.array == right.array
        && left.optional == right.optional
        && (left.optional || condition(&left.meta).is_some() == condition(&right.meta).is_some())
}

fn materialize_field_history(registry: &mut UnifiedRegistry) {
    for value in registry.types.values_mut() {
        let UnifiedType::Struct(value) = value else {
            continue;
        };
        let mut used: BTreeSet<_> = value.ir.fields.iter().map(field_rust_name).collect();
        for (historical, versions) in &value.field_history {
            let mut field = historical.clone();
            let wire = field
                .meta
                .attributes
                .iter()
                .find_map(|attr| match attr {
                    Attribute::Item { name, value } if name == "name" => Some(value.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| field.name.clone());
            let base = format!(
                "{}_legacy",
                field_rust_name(&field).trim_start_matches("r#")
            );
            let mut name = base.clone();
            let mut suffix = 2;
            while !used.insert(name.clone()) {
                name = format!("{base}{suffix}");
                suffix += 1;
            }
            field.name = name.clone();
            field
                .meta
                .attributes
                .retain(|attr| !matches!(attr, Attribute::Item { name, .. } if name == "name"));
            field
                .meta
                .attributes
                .push(Attribute::with_value("name", wire));
            value.field_versions.insert(name, versions.clone());
            value.ir.fields.push(field);
        }
    }
}

fn merge_struct(registry: &mut UnifiedRegistry, name: &str, s: &Struct, version: Version) {
    if let Some(UnifiedType::Struct(existing)) = registry.types.get_mut(name) {
        existing.versions.until = None;
        // Track type-level deprecation
        if has_feature(&s.meta, "deprecated") && existing.versions.deprecated.is_none() {
            existing.versions.deprecated = Some(version);
        }
        // Track type-level unstable
        if has_feature(&s.meta, "unstable") && existing.versions.unstable.is_none() {
            existing.versions.unstable = Some(version);
        }
        // Update doc from latest version (latest has best docs)
        merge_doc_from(&mut existing.ir.meta.doc, s.meta.doc.as_deref());
        refresh_condition(&mut existing.ir.meta, &s.meta);
        // Merge version-tracked boolean command attributes (allow_preconfig, allow_oob, etc.)
        merge_bool_attrs(existing, &s.meta, version);
        merge_returns(existing, &s.meta, version);
        // Union struct-level attributes across versions. We preserve the
        // older version's attributes (e.g. `name = "SGXInfo"` wire override
        // from pre-10.1) and add any new attributes the newer version
        // introduced. Version-tracked attrs and bool attrs are handled
        // separately above.
        for new_attr in &s.meta.attributes {
            if !existing
                .ir
                .meta
                .attributes
                .iter()
                .any(|a| attrs_equivalent(a, new_attr))
            {
                existing.ir.meta.attributes.push(new_attr.clone());
            }
        }

        let mut seen_fields = BTreeSet::new();
        for field in &s.fields {
            seen_fields.insert(field.name.clone());
            if existing.field_versions.contains_key(&field.name) {
                let old = existing
                    .ir
                    .fields
                    .iter()
                    .find(|old| old.name == field.name)
                    .unwrap();
                if !same_field_shape(old, field)
                    || existing.field_versions[&field.name].until.is_some()
                {
                    let mut old_versions = existing.field_versions[&field.name].clone();
                    old_versions.until = old_versions.until.or(Some(version));
                    existing.field_history.push((old.clone(), old_versions));
                    existing
                        .field_versions
                        .insert(field.name.clone(), VersionRange::new(version));
                }
                // Field still exists — clear until, track deprecation.
                let fr = existing.field_versions.get_mut(&field.name).unwrap();
                fr.until = None;
                if has_feature(&field.meta, "deprecated") && fr.deprecated.is_none() {
                    fr.deprecated = Some(version);
                }
                // Promote the field to the latest version's definition.
                // Type / optional / array always use the newer value (the
                // latest schema is authoritative for the field shape).
                // Attributes get unioned with the existing set so version-
                // specific directives (e.g. deprecated feature) survive.
                // Doc falls through: newer version wins if it has one,
                // otherwise the older doc is preserved.
                if let Some(existing_field) =
                    existing.ir.fields.iter_mut().find(|f| f.name == field.name)
                {
                    existing_field.r#type = field.r#type.clone();
                    existing_field.optional = field.optional;
                    existing_field.array = field.array;
                    merge_doc_from(&mut existing_field.meta.doc, field.meta.doc.as_deref());
                    refresh_condition(&mut existing_field.meta, &field.meta);
                    for new_attr in &field.meta.attributes {
                        if !existing_field
                            .meta
                            .attributes
                            .iter()
                            .any(|a| attrs_equivalent(a, new_attr))
                        {
                            existing_field.meta.attributes.push(new_attr.clone());
                        }
                    }
                }
            } else {
                // Check for rename (same Rust field name, different QAPI name)
                let rust_name = field_rust_name(field);
                let renamed_from = existing
                    .ir
                    .fields
                    .iter()
                    .position(|f| field_rust_name(f) == rust_name && f.name != field.name);

                if let Some(old_idx) = renamed_from {
                    let old = existing.ir.fields[old_idx].clone();
                    let mut old_versions = existing.field_versions.remove(&old.name).unwrap();
                    old_versions.until = old_versions.until.or(Some(version));
                    existing.field_history.push((old, old_versions));
                    existing.ir.fields[old_idx] = field.clone();
                    existing
                        .field_versions
                        .insert(field.name.clone(), VersionRange::new(version));
                } else {
                    // Check for Rust name collision with existing fields
                    let rust_name = field_rust_name(field);
                    let has_collision = existing
                        .ir
                        .fields
                        .iter()
                        .any(|f| field_rust_name(f) == rust_name && f.name != field.name);

                    let mut field = field.clone();
                    if has_collision {
                        // Append a suffix to disambiguate
                        let new_name = format!("{}_alt", field.name);
                        eprintln!(
                            "WARNING: Renaming field '{}' to '{}' in '{}' to avoid collision",
                            field.name, new_name, name
                        );
                        // Store original QAPI name in the name attribute
                        if !field
                            .meta
                            .attributes
                            .iter()
                            .any(|a| matches!(a, Attribute::Item { name, .. } if name == "name"))
                        {
                            field
                                .meta
                                .attributes
                                .push(Attribute::with_value("name", &field.name));
                        }
                        field.name = new_name.clone();
                        existing
                            .field_versions
                            .insert(new_name, VersionRange::new(version));
                    } else {
                        existing
                            .field_versions
                            .insert(field.name.clone(), VersionRange::new(version));
                    }
                    if !existing.ir.fields.iter().any(|f| f.name == field.name) {
                        existing.ir.fields.push(field);
                    }
                }
            }
        }

        for (fname, fr) in existing.field_versions.iter_mut() {
            if !seen_fields.contains(fname) && fr.until.is_none() && fr.since < version {
                fr.until = Some(version);
            }
        }
    } else {
        // Detect field name collisions within the initial version
        let mut ir = s.clone();
        clean_doc_in_place(&mut ir.meta.doc);
        let mut field_versions = BTreeMap::new();
        let mut seen_rust_names: BTreeSet<String> = BTreeSet::new();
        for field in &mut ir.fields {
            clean_doc_in_place(&mut field.meta.doc);
            let rust_name = field_rust_name(field);
            if !seen_rust_names.insert(rust_name) {
                let new_name = format!("{}_alt", field.name);
                eprintln!(
                    "WARNING: Renaming field '{}' to '{}' in '{}' to avoid collision",
                    field.name, new_name, name
                );
                if !field
                    .meta
                    .attributes
                    .iter()
                    .any(|a| matches!(a, Attribute::Item { name, .. } if name == "name"))
                {
                    field
                        .meta
                        .attributes
                        .push(Attribute::with_value("name", &field.name));
                }
                field.name = new_name.clone();
                field_versions.insert(new_name, VersionRange::new(version));
            } else {
                field_versions.insert(field.name.clone(), VersionRange::new(version));
            }
        }
        // Track version-sensitive boolean attributes from initial version
        let attr_versions = extract_bool_attr_versions(&ir.meta, version);
        let returns_history = returns_attr(&ir.meta)
            .map(|r| vec![(version, r.to_owned())])
            .unwrap_or_default();
        let mut versions = VersionRange::new(version);
        if has_feature(&ir.meta, "unstable") {
            versions.unstable = Some(version);
        }
        if has_feature(&ir.meta, "deprecated") {
            versions.deprecated = Some(version);
        }
        registry.types.insert(
            name.to_string(),
            UnifiedType::Struct(UnifiedStruct {
                ir,
                versions,
                field_versions,
                field_history: Vec::new(),
                attr_versions,
                returns_history,
            }),
        );
    }
}

/// Detect and fix recursive type references by marking fields that need Box wrapping.
/// A field is recursive if its type (directly or through an enum) refers back to the
/// containing struct.
fn fix_recursive_types(registry: &mut UnifiedRegistry) {
    // Collect (type_name, field_name) pairs that need Box wrapping, then
    // apply mutations after releasing the immutable borrow on registry.
    let needs_box: BTreeSet<(String, String)> = {
        // Only inline references affect Rust size; Vec and Box already break cycles.
        let mut deps: HashMap<String, BTreeSet<String>> = HashMap::new();
        for (name, ut) in &registry.types {
            let mut referenced = BTreeSet::new();
            match ut {
                UnifiedType::Struct(us) => {
                    for field in &us.ir.fields {
                        if !field.array && !field.meta.attributes.contains(&Attribute::new("boxed"))
                        {
                            referenced.insert(rustify_type(&field.r#type));
                        }
                    }
                }
                UnifiedType::Enum(ue) => {
                    for variant in &ue.ir.variants {
                        if !variant.array {
                            if let EnumVariantKind::Tuple(t) = &variant.kind {
                                referenced.insert(rustify_type(t));
                            }
                        }
                    }
                }
            }
            deps.insert(name.clone(), referenced);
        }

        // Find types that are part of a cycle
        let mut result: BTreeSet<(String, String)> = BTreeSet::new();
        for (name, ut) in &registry.types {
            if let UnifiedType::Struct(us) = ut {
                for field in &us.ir.fields {
                    let referenced = rustify_type(&field.r#type);
                    if !field.array
                        && !field.meta.attributes.contains(&Attribute::new("boxed"))
                        && is_reachable(&referenced, name, &deps, &mut BTreeSet::new())
                    {
                        result.insert((name.clone(), field.name.clone()));
                    }
                }
            }
        }
        result
    };

    // Apply Box wrapping by setting a flag on the field
    for (type_name, field_name) in &needs_box {
        if let Some(UnifiedType::Struct(us)) = registry.types.get_mut(type_name) {
            for field in &mut us.ir.fields {
                if field.name == *field_name {
                    eprintln!(
                        "INFO: Boxing field '{}.{}' to break recursive type",
                        type_name, field_name
                    );
                    // Use a special marker that the codegen will detect.
                    // The codegen checks field.meta for "boxed" and wraps in Box<>.
                    field.meta.attributes.push(Attribute::new("boxed"));
                }
            }
        }
    }
}

/// Check if `target` is reachable from `start` through the dependency graph.
fn is_reachable<'a>(
    start: &'a str,
    target: &str,
    deps: &'a HashMap<String, BTreeSet<String>>,
    visited: &mut BTreeSet<&'a str>,
) -> bool {
    if start == target {
        return true;
    }
    if !visited.insert(start) {
        return false;
    }
    if let Some(refs) = deps.get(start) {
        for r in refs {
            if is_reachable(r, target, deps, visited) {
                return true;
            }
        }
    }
    false
}

/// Knobs that differ between the QMP and QGA code-emission paths.
struct CodegenFlavor {
    /// Prefix in log messages, e.g. "" for QMP and "QGA " for QGA.
    label: &'static str,
    /// Emit verbose timing output (Token generation / Formatting / Codegen total).
    /// When false, a single aggregated timing line is emitted instead.
    verbose_timing: bool,
}

/// Shared code-emission core used by both QMP (`generate_unified_code`) and
/// QGA (`generate_qga_code`). Classifies each type, partitions into
/// types/commands/events token streams, formats each as a Rust file, and
/// returns the three-file output.
fn emit_unified_output(
    registry: &UnifiedRegistry,
    flavor: &CodegenFlavor,
) -> Result<GeneratedOutput> {
    use std::time::Instant;
    let codegen_start = Instant::now();

    // Track Rust names to detect collisions (e.g., struct BlockDirtyBitmapAdd vs
    // command block-dirty-bitmap-add both produce the same Rust name).
    // Pre-populate with Rust reserved/builtin type names that should never be shadowed.
    let seen_rust_names: BTreeSet<String> = ["String", "Option", "Vec", "Box", "Result"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    // Generate types in a stable order.
    // Sort all types by Rust name (case-insensitive) for deterministic, readable output.
    let mut all_entries: Vec<_> = registry.types.iter().collect();
    all_entries.sort_by(|(_, a), (_, b)| {
        unified_type_rust_name(a)
            .to_lowercase()
            .cmp(&unified_type_rust_name(b).to_lowercase())
    });

    // Pass 1: Classify types and build a set of command/event Rust names.
    // Then find which command/event types are referenced by data types' fields
    // or enum variants — those must stay in the data types module.
    let mut command_names: BTreeSet<String> = BTreeSet::new();
    let mut event_names: BTreeSet<String> = BTreeSet::new();
    for (_, ut) in &all_entries {
        let rust_name = match ut {
            UnifiedType::Enum(ue) => rustify_name(&ue.ir.name),
            UnifiedType::Struct(us) => rustify_name(&us.ir.name),
        };
        match classify_type(ut) {
            TypeCategory::Command => {
                command_names.insert(rust_name);
            }
            TypeCategory::Event => {
                event_names.insert(rust_name);
            }
            TypeCategory::DataType => {}
        }
    }

    // Find command/event types referenced by data type fields or enum variants
    let mut referenced_by_data: BTreeSet<String> = BTreeSet::new();
    for (_, ut) in &all_entries {
        if classify_type(ut) != TypeCategory::DataType {
            continue;
        }
        match ut {
            UnifiedType::Struct(us) => {
                for field in &us.ir.fields {
                    let rust_type = rustify_name(&field.r#type);
                    if command_names.contains(&rust_type) || event_names.contains(&rust_type) {
                        referenced_by_data.insert(rust_type);
                    }
                }
            }
            UnifiedType::Enum(ue) => {
                for variant in &ue.ir.variants {
                    if let crate::qapi_ir::EnumVariantKind::Tuple(ref type_name) = variant.kind {
                        let rust_type = rustify_name(type_name);
                        if command_names.contains(&rust_type) || event_names.contains(&rust_type) {
                            referenced_by_data.insert(rust_type);
                        }
                    }
                }
            }
        }
    }
    if flavor.verbose_timing && !referenced_by_data.is_empty() {
        eprintln!(
            "  {} command/event types also referenced by data types (kept in types module): {:?}",
            referenced_by_data.len(),
            referenced_by_data
        );
    }

    // Pass 2: Emit into partitioned token streams, overriding classification
    // for types that are cross-referenced by data types.
    let mut types_tokens = proc_macro2::TokenStream::new();
    let mut commands_tokens = proc_macro2::TokenStream::new();
    let mut events_tokens = proc_macro2::TokenStream::new();

    let mut seen_type_names = seen_rust_names.clone();
    let mut seen_command_names = seen_rust_names.clone();
    let mut seen_event_names = seen_rust_names;

    for (name, ut) in &all_entries {
        let rust_name = match ut {
            UnifiedType::Enum(ue) => rustify_name(&ue.ir.name),
            UnifiedType::Struct(us) => rustify_name(&us.ir.name),
        };

        let mut category = classify_type(ut);
        // Override: if a command/event type is referenced by data types, keep it
        // in the data types module so cross-references resolve.
        if referenced_by_data.contains(&rust_name) {
            category = TypeCategory::DataType;
        }

        let (tokens, seen) = match category {
            TypeCategory::DataType => (&mut types_tokens, &mut seen_type_names),
            TypeCategory::Command => (&mut commands_tokens, &mut seen_command_names),
            TypeCategory::Event => (&mut events_tokens, &mut seen_event_names),
        };

        anyhow::ensure!(
            !seen.contains(&rust_name),
            "{}duplicate Rust name '{}' (QAPI name: '{}', category: {:?})",
            flavor.label,
            rust_name,
            name,
            category
        );
        seen.insert(rust_name);

        let code = match ut {
            UnifiedType::Enum(ue) => generate_unified_enum(ue, registry),
            UnifiedType::Struct(us) => generate_unified_struct(us, registry),
        };
        tokens.extend(code);
    }

    if flavor.verbose_timing {
        eprintln!(
            "  Token generation: {:.1}s",
            codegen_start.elapsed().as_secs_f64()
        );
    }

    // Format each part as a separate file
    let fmt_start = Instant::now();

    // mod.rs: data types + module declarations
    let mod_tokens = quote::quote! {
        #types_tokens

        pub mod commands;
        pub mod events;
        pub mod metadata;
    };
    let types_file = format_rust_file(mod_tokens, "")?;

    // commands.rs: command types with super::* import
    let commands_file = format_rust_file(commands_tokens, "use super::*;\n")?;

    // events.rs: event types with super::* import
    let events_file = format_rust_file(events_tokens, "use super::*;\n")?;

    if flavor.verbose_timing {
        eprintln!("  Formatting: {:.1}s", fmt_start.elapsed().as_secs_f64());
        eprintln!(
            "  Codegen total: {:.1}s",
            codegen_start.elapsed().as_secs_f64()
        );
    } else {
        eprintln!(
            "  {}codegen: {:.1}s",
            flavor.label,
            codegen_start.elapsed().as_secs_f64()
        );
    }

    let metadata_file = crate::metadata_emit::emit(registry);

    Ok(GeneratedOutput {
        types: types_file,
        commands: commands_file,
        events: events_file,
        metadata: metadata_file,
    })
}

pub fn generate_unified_code(qemu_repo: &Path, version_tags: &[&str]) -> Result<GeneratedOutput> {
    let registry = build_unified_registry(qemu_repo, version_tags)?;
    emit_unified_output(
        &registry,
        &CodegenFlavor {
            label: "",
            verbose_timing: true,
        },
    )
}

/// The four files produced by code generation.
pub struct GeneratedOutput {
    /// Data types (structs, enums, unions) — goes into `mod.rs`.
    pub types: String,
    /// Command types — goes into `commands.rs`.
    pub commands: String,
    /// Event types — goes into `events.rs`.
    pub events: String,
    /// Compile-time schema metadata — goes into `metadata.rs`.
    pub metadata: String,
}

impl GeneratedOutput {
    /// Concatenate the three code parts into a single string.
    /// Useful for tests that search across all generated code.
    /// Strips inner attributes (`#![...]`) and duplicate headers from
    /// commands/events so the result is valid as a single Rust file.
    /// The `metadata` part is excluded — it lives in its own module and
    /// uses an entirely separate type system.
    pub fn combined(&self) -> String {
        let mut result =
            String::with_capacity(self.types.len() + self.commands.len() + self.events.len());
        result.push_str(&self.types);
        // Append commands and events, stripping file-level headers
        for part in [&self.commands, &self.events] {
            for line in part.lines() {
                // Skip inner attributes, use/crate lines that duplicate the header
                if line.starts_with("#![")
                    || line.starts_with("use crate as qapi;")
                    || line.starts_with("use qapi_macros::qapi;")
                    || line.starts_with("use std::collections::HashMap;")
                    || line.starts_with("use super::*;")
                {
                    continue;
                }
                result.push_str(line);
                result.push('\n');
            }
        }
        result
    }
}

/// Format a token stream as a complete Rust file with the standard header.
fn format_rust_file(tokens: proc_macro2::TokenStream, extra_imports: &str) -> Result<String> {
    let mut output = String::new();
    output.push_str("#![allow(unused_imports, non_camel_case_types, dead_code)]\n");
    output.push_str("use crate as qapi;\n");
    output.push_str("use qapi_macros::qapi;\n");
    output.push_str("use std::collections::HashMap;\n");
    if !extra_imports.is_empty() {
        output.push_str(extra_imports);
    }
    output.push('\n');

    let combined = format!("{}{}", output, tokens);
    let syntax = syn::parse_file(&combined).context("Generated invalid Rust")?;
    Ok(separate_items(&prettyplease::unparse(&syntax)))
}

/// Insert a blank line between top-level items for readability.
fn separate_items(code: &str) -> String {
    let mut result = String::with_capacity(code.len() + code.len() / 20);
    let mut prev_line = "";
    for line in code.lines() {
        // Insert blank line after a top-level item ends (line is or ends with `}`)
        let prev_ends_item = prev_line == "}" || prev_line.ends_with(" {}");
        if prev_ends_item && !line.is_empty() {
            result.push('\n');
        }
        result.push_str(line);
        result.push('\n');
        prev_line = line;
    }
    result
}

/// Drop any version-lifecycle attrs already attached to a type so the
/// registry-derived values are the single source of truth. Always removes
/// `added`/`since`/`removed`/`until`/`deprecated` items; when
/// `strip_nested_bundles` is true, also removes nested `deprecated`/`unstable`
/// bundles (only relevant at the type level — variants and fields never carry
/// these nested forms).
fn strip_version_attrs(attrs: &mut Vec<Attribute>, strip_nested_bundles: bool) {
    attrs.retain(|a| !matches!(a, Attribute::Item { name, .. } if name == "added" || name == "since" || name == "removed" || name == "until" || name == "deprecated"));
    if strip_nested_bundles {
        attrs.retain(|a| !matches!(a, Attribute::Nested { name, .. } if name == "deprecated" || name == "unstable"));
    }
}

/// Generate code for a unified enum, adding since/until attributes from the registry.
fn generate_unified_enum(
    ue: &UnifiedEnum,
    _registry: &UnifiedRegistry,
) -> proc_macro2::TokenStream {
    let mut ir = ue.ir.clone();

    // Strip any existing since from doc comments — always use registry version
    strip_version_attrs(&mut ir.meta.attributes, true);
    ir.meta.attributes.push(Attribute::with_value(
        "added",
        version_str(ue.versions.since),
    ));
    if let Some(dep) = ue.versions.deprecated {
        ir.meta.attributes.push(Attribute::nested(
            "deprecated",
            vec![Attribute::with_value("added", version_str(dep))],
        ));
    }
    if let Some(unstable) = ue.versions.unstable {
        ir.meta.attributes.push(Attribute::nested(
            "unstable",
            vec![Attribute::with_value("added", version_str(unstable))],
        ));
        // Remove redundant feature = "unstable" since unstable(added=...) covers it
        ir.meta.attributes.retain(|a| {
            !matches!(a,
                Attribute::Item { name, value } if name == "feature" && value == "unstable"
            )
        });
    }
    if let Some(until) = ue.versions.until {
        ir.meta
            .attributes
            .push(Attribute::with_value("removed", version_str(until)));
    }

    // Add per-variant added/deprecated/removed
    for variant in &mut ir.variants {
        strip_version_attrs(&mut variant.meta.attributes, false);
        if let Some(vr) = ue.variant_versions.get(&variant.name) {
            if vr.since > ue.versions.since {
                variant
                    .meta
                    .attributes
                    .push(Attribute::with_value("added", version_str(vr.since)));
            }
            if let Some(dep) = vr.deprecated {
                variant.meta.attributes.push(Attribute::nested(
                    "deprecated",
                    vec![Attribute::with_value("added", version_str(dep))],
                ));
            }
            if let Some(until) = vr.until {
                variant
                    .meta
                    .attributes
                    .push(Attribute::with_value("removed", version_str(until)));
            }
        }
    }

    // Clean docs
    clean_doc_in_place(&mut ir.meta.doc);
    for variant in &mut ir.variants {
        clean_doc_in_place(&mut variant.meta.doc);
    }

    // Sort variants alphabetically by name
    ir.variants
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    ir.generate()
}

/// Generate code for a unified struct, adding since/until and making removed/added fields optional.
fn generate_unified_struct(
    us: &UnifiedStruct,
    _registry: &UnifiedRegistry,
) -> proc_macro2::TokenStream {
    let mut ir = us.ir.clone();

    // Strip existing since from doc comments — always use registry version
    strip_version_attrs(&mut ir.meta.attributes, true);
    ir.meta.attributes.push(Attribute::with_value(
        "added",
        version_str(us.versions.since),
    ));
    if let Some(dep) = us.versions.deprecated {
        ir.meta.attributes.push(Attribute::nested(
            "deprecated",
            vec![Attribute::with_value("added", version_str(dep))],
        ));
    }
    if let Some(unstable) = us.versions.unstable {
        let mut inner = vec![Attribute::with_value("added", version_str(unstable))];
        // Check if unstable was removed (stabilized) via attr_versions
        if let Some(uvr) = us.attr_versions.get("unstable")
            && let Some(until) = uvr.until
        {
            inner.push(Attribute::with_value("removed", version_str(until)));
        }
        ir.meta
            .attributes
            .push(Attribute::nested("unstable", inner));
        // Remove redundant feature = "unstable" since unstable(added=...) covers it
        ir.meta.attributes.retain(|a| {
            !matches!(a,
                Attribute::Item { name, value } if name == "feature" && value == "unstable"
            )
        });
    }
    if let Some(until) = us.versions.until {
        ir.meta
            .attributes
            .push(Attribute::with_value("removed", version_str(until)));
    }

    // Add version-tracked boolean command attributes (allow_preconfig, allow_oob, etc.)
    // using nested syntax: #[qapi(allow_preconfig(added = "7.1.0", removed = "9.0.0"))]
    // "unstable" is handled specially since it's also a top-level version attribute.
    for (attr_name, vr) in &us.attr_versions {
        // Skip "unstable" — already handled above as a top-level attribute
        if attr_name == "unstable" {
            continue;
        }
        // Remove any existing forms of this attribute (bare, old-style, nested)
        ir.meta
            .attributes
            .retain(|a| !matches!(a, Attribute::Unit(n) if n == attr_name));
        ir.meta.attributes.retain(|a| !matches!(a, Attribute::Item { name, .. } if name == &format!("{}_since", attr_name) || name == &format!("{}_until", attr_name)));
        ir.meta
            .attributes
            .retain(|a| !matches!(a, Attribute::Nested { name, .. } if name == attr_name));

        let mut inner = Vec::new();
        if vr.since > us.versions.since {
            inner.push(Attribute::with_value("added", version_str(vr.since)));
        }
        if let Some(until) = vr.until {
            inner.push(Attribute::with_value("removed", version_str(until)));
        }
        if inner.is_empty() {
            ir.meta.attributes.push(Attribute::new(attr_name));
        } else {
            ir.meta.attributes.push(Attribute::nested(attr_name, inner));
        }
    }

    if us.returns_history.len() > 1 {
        for (version, value) in &us.returns_history {
            ir.meta.attributes.push(Attribute::nested(
                "returns",
                vec![
                    Attribute::with_value("added", version_str(*version)),
                    Attribute::with_value("value", value),
                ],
            ));
        }
    }

    // Add per-field added/deprecated/removed and make fields optional if needed
    for field in &mut ir.fields {
        let required = !field.optional;
        strip_version_attrs(&mut field.meta.attributes, false);
        if let Some(fr) = us.field_versions.get(&field.name) {
            if fr.since > us.versions.since {
                field
                    .meta
                    .attributes
                    .push(Attribute::with_value("added", version_str(fr.since)));
                if !field.optional {
                    field.optional = true;
                }
            }
            if let Some(dep) = fr.deprecated {
                field.meta.attributes.push(Attribute::nested(
                    "deprecated",
                    vec![Attribute::with_value("added", version_str(dep))],
                ));
            }
            if let Some(until) = fr.until {
                field
                    .meta
                    .attributes
                    .push(Attribute::with_value("removed", version_str(until)));
                if !field.optional {
                    field.optional = true;
                }
            }
        }
        let conditional = field
            .meta
            .attributes
            .iter()
            .any(|attr| matches!(attr, Attribute::Item { name, .. } if name == "condition"));
        if conditional {
            field.optional = true;
        }
        if required
            && field.optional
            && !conditional
            && !field.meta.attributes.contains(&Attribute::new("flatten"))
        {
            field.meta.attributes.push(Attribute::new("required"));
        }
    }

    // Clean docs
    clean_doc_in_place(&mut ir.meta.doc);
    for field in &mut ir.fields {
        clean_doc_in_place(&mut field.meta.doc);
    }

    // Sort fields alphabetically by name
    ir.fields
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    ir.generate()
}

/// Build a unified registry from QGA schemas across versions.
pub fn build_qga_registry(qemu_repo: &Path, version_tags: &[&str]) -> Result<UnifiedRegistry> {
    use std::time::Instant;
    let start = Instant::now();

    eprintln!(
        "Extracting and parsing QGA schemas from {} versions...",
        version_tags.len()
    );
    let (mut version_results, absent) = load_versions(qemu_repo, version_tags, true)?;

    eprintln!(
        "  QGA extraction + parsing: {:.1}s",
        start.elapsed().as_secs_f64()
    );

    version_results
        .sort_by_key(|result| crate::versions::parse_version_tag_full(&result.tag).unwrap());
    eprintln!(
        "  QGA: {} versions parsed, {} without a guest-agent schema",
        version_results.len(),
        absent
    );

    let merge_start = Instant::now();
    let mut registry = UnifiedRegistry::default();
    let mut prev_hash: Option<u64> = None;
    let mut skipped = 0;

    for vr in &version_results {
        registry.versions.push(vr.version);
        if prev_hash == Some(vr.schema_hash) {
            skipped += 1;
            continue;
        }
        prev_hash = Some(vr.schema_hash);

        let mut seen_types: BTreeSet<&str> = BTreeSet::new();
        for (name, e) in &vr.enums {
            seen_types.insert(name.as_str());
            merge_enum(&mut registry, name, e, vr.version);
        }
        for (name, s) in &vr.structs {
            seen_types.insert(name.as_str());
            merge_struct(&mut registry, name, s, vr.version);
        }
        for (name, ut) in registry.types.iter_mut() {
            if !seen_types.contains(name.as_str()) {
                let vr_inner = match ut {
                    UnifiedType::Enum(e) => &mut e.versions,
                    UnifiedType::Struct(s) => &mut s.versions,
                };
                if vr_inner.until.is_none() && vr_inner.since < vr.version {
                    vr_inner.until = Some(vr.version);
                }
            }
        }
    }

    if skipped > 0 {
        eprintln!("  QGA skipped {} identical schemas", skipped);
    }
    materialize_variant_history(&mut registry);
    materialize_field_history(&mut registry);
    fix_recursive_types(&mut registry);
    eprintln!(
        "  QGA merge: {:.1}s — {} types",
        merge_start.elapsed().as_secs_f64(),
        registry.types.len()
    );

    Ok(registry)
}

/// Generate unified QGA code from the registry.
pub fn generate_qga_code(qemu_repo: &Path, version_tags: &[&str]) -> Result<GeneratedOutput> {
    let registry = build_qga_registry(qemu_repo, version_tags)?;
    emit_unified_output(
        &registry,
        &CodegenFlavor {
            label: "QGA ",
            verbose_timing: false,
        },
    )
}

#[cfg(test)]
mod schema_resolution_tests {
    use super::*;

    #[test]
    fn non_object_and_unboxed_union_arguments_are_rejected() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/qemu/tests/qapi-schema");
        for name in ["args-union", "args-alternate"] {
            let path = root.join(format!("{name}.json"));
            let sources = crate::read_schema_with_version(&path, Some((11, 1, 3))).unwrap();
            let error = format!("{:#}", process_version_schema(&sources).unwrap_err());
            assert!(error.contains(name), "{error}");
        }
        let definitions = concat!(
            "{'enum':'Kind','data':['one']}\n",
            "{'struct':'Payload','data':{'value':'str'}}\n",
            "{'union':'Choice','base':{'kind':'Kind'},'discriminator':'kind','data':{'one':'Payload'}}\n",
        );
        process(&format!("{definitions} {{'command':'good','boxed':true,'data':'Choice'}} {{'event':'GOOD','boxed':true,'data':'Choice'}} {{'command':'plain','data':'Payload'}}")).unwrap();
        let error =
            process(&format!("{definitions} {{'event':'BAD','data':'Choice'}}")).unwrap_err();
        assert!(format!("{error:#}").contains("requires 'boxed': true"));
    }

    #[test]
    fn conditional_unboxed_argument_fixtures_are_rejected() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/qemu/tests/qapi-schema");
        for name in ["args-if-unboxed", "event-args-if-unboxed"] {
            let path = root.join(format!("{name}.json"));
            let sources = crate::read_schema_with_version(&path, Some((11, 1, 3))).unwrap();
            let error = format!("{:#}", process_version_schema(&sources).unwrap_err());
            assert!(error.contains(name), "{error}");
            assert!(error.contains("boxed"), "{error}");
        }
    }

    #[test]
    fn boxed_inherited_conditions_and_older_unboxed_forms_remain_valid() {
        let definitions = concat!(
            "{'struct':'Base','data':{'optional-build':{'type':'str','if':'CONFIG_TEST'}}}\n",
            "{'struct':'Args','base':'Base','data':{'value':'str'}}\n",
        );
        process(&format!("{definitions} {{'command':'good','boxed':true,'data':'Args'}} {{'event':'GOOD','boxed':true,'data':'Args'}}")).unwrap();
        let bad = format!("{definitions} {{'command':'bad','data':'Args'}}");
        assert!(
            format!("{:#}", process(&bad).unwrap_err()).contains("Conditional arguments require")
        );
        crate::set_parser_version((8, 0, 0));
        process_version_schema(&[("old.json".into(), bad)]).unwrap();
    }

    #[test]
    fn command_flags_reject_invalid_values_and_incompatible_execution_modes() {
        for input in [
            "{'command':'bad','allow-oob':false}",
            "{'command':'bad','allow-preconfig':false}",
            "{'command':'bad','coroutine':false}",
            "{'command':'bad','gen':true}",
            "{'command':'bad','success-response':true}",
            "{'command':'bad','allow-oob':true,'coroutine':true}",
        ] {
            let error = format!("{:#}", process(input).unwrap_err());
            assert!(error.contains("fixture.json"), "{error}");
            assert!(
                error.contains("Flag") || error.contains("incompatible"),
                "{error}"
            );
        }
        process(concat!(
            "{'command':'oob','allow-oob':true}\n",
            "{'command':'preconfig','allow-preconfig':true}\n",
            "{'command':'coroutine','coroutine':true}\n",
            "{'command':'manual','gen':false,'success-response':false}",
        ))
        .unwrap();
        crate::set_parser_version((1, 0, 0));
        process_version_schema(&[("old.json".into(), "{'command':'manual','gen':'no'}".into())])
            .unwrap();
    }

    #[test]
    fn upstream_invalid_boxed_arguments_are_rejected() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/qemu/tests/qapi-schema");
        for name in [
            "args-bad-boxed",
            "args-boxed-anon",
            "args-boxed-string",
            "event-boxed-empty",
        ] {
            let path = root.join(format!("{name}.json"));
            let sources = crate::read_schema_with_version(&path, Some((11, 1, 3))).unwrap();
            let error = format!("{:#}", process_version_schema(&sources).unwrap_err());
            assert!(error.contains(name), "{name}: {error}");
        }
    }

    #[test]
    fn boxed_commands_and_events_keep_named_object_fields_flat() {
        let (_, structs) = process(concat!(
            "{'struct':'Payload','data':{'value':'str'}}\n",
            "{'command':'boxed-command','boxed':true,'data':'Payload'}\n",
            "{'event':'BOXED_EVENT','boxed':true,'data':'Payload'}\n",
            "{'command':'boxed-empty','boxed':true,'data':'q_empty'}",
        ))
        .unwrap();
        assert_eq!(structs["BoxedCommand"].fields[0].name, "value");
        assert_eq!(structs["BoxedEventEvent"].fields[0].name, "value");
        assert!(structs["BoxedEmpty"].fields.is_empty());
    }

    #[test]
    fn upstream_ambiguous_alternate_fixtures_are_rejected() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/qemu/tests/qapi-schema");
        for name in [
            "alternate-conflict-dict",
            "alternate-conflict-bool-string",
            "alternate-conflict-num-string",
            "alternate-conflict-string",
            "alternate-conflict-enum-int",
            "alternate-conflict-enum-bool",
            "alternate-conflict-lists",
            "alternate-any",
        ] {
            let path = root.join(format!("{name}.json"));
            let sources = crate::read_schema_with_version(&path, Some((11, 1, 3))).unwrap();
            let error = format!("{:#}", process_version_schema(&sources).unwrap_err());
            assert!(error.contains(name), "{name}: {error}");
            assert!(
                error.contains("distinguished") || error.contains("cannot use"),
                "{error}"
            );
        }
    }

    #[test]
    fn alternate_scalar_conflicts_follow_the_historical_qobject_model() {
        for input in [
            "{'alternate':'Choice','data':{'integer':'int','floating':'number'}}",
            "{'alternate':'Choice','data':{'text':'str','boolean':'bool'}}",
        ] {
            crate::set_parser_version((2, 9, 0));
            process_version_schema(&[("old.json".into(), input.into())]).unwrap();
            crate::set_parser_version((2, 10, 0));
            let error = process_version_schema(&[("new.json".into(), input.into())]).unwrap_err();
            assert!(format!("{error:#}").contains("cannot be distinguished"));
        }
        process(concat!(
            "{'enum':'Kind','data':['yes','no']}\n",
            "{'struct':'Payload','data':{'value':'str'}}\n",
            "{'alternate':'Choice','data':{'kind':'Kind','number':'int','object':'Payload','empty':'null'}}",
        )).unwrap();
    }

    #[test]
    fn implicit_arrays_require_creation_and_reject_nested_arrays() {
        for (input, expected) in [
            (
                "{'struct':'Holder','data':{'values':'QTypeList'}}",
                "Unknown QAPI type 'QTypeList'",
            ),
            (
                "{'struct':'Item','data':{}} {'struct':'Holder','data':{'values':'ItemList'}}",
                "Unknown QAPI type 'ItemList'",
            ),
            (
                "{'struct':'Holder','data':{'values':['intList']}}",
                "Nested QAPI arrays",
            ),
            (
                "{'struct':'Holder','data':{'direct':['intList'],'alias':'intListList'}}",
                "Nested QAPI arrays",
            ),
            ("{'struct':'ItemList','data':{}}", "reserved List suffix"),
        ] {
            let error = format!("{:#}", process(input).unwrap_err());
            assert!(error.contains(expected), "{expected}: {error}");
            assert!(error.contains("fixture.json"), "{error}");
        }
    }

    #[test]
    fn implicit_arrays_resolve_across_source_files() {
        crate::set_parser_version((11, 1, 3));
        let (_, structs) = process_version_schema(&[
            ("first.json".into(), "{'struct':'Holder','data':{'values':'ItemList'}}".into()),
            ("second.json".into(), "{'struct':'Item','data':{'value':'str'}} {'struct':'Seed','data':{'direct':['Item']}}".into()),
        ]).unwrap();
        let field = &structs["Holder"].fields[0];
        assert!(field.array);
        assert_eq!(field.r#type, "Item");
    }

    #[test]
    fn predefined_types_are_reserved_and_synthesized_only_when_used() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/qemu/tests/qapi-schema");
        let path = root.join("redefined-predefined.json");
        let sources = crate::read_schema_with_version(&path, Some((11, 1, 3))).unwrap();
        let error = format!("{:#}", process_version_schema(&sources).unwrap_err());
        assert!(error.contains("redefined-predefined.json"), "{error}");
        assert!(error.contains("Predefined QAPI type 'QType'"), "{error}");
        let (enums, structs) = process("{'struct':'Plain','data':{}}").unwrap();
        assert!(!enums.contains_key("QType"));
        assert!(!structs.contains_key("QEmpty"));
        crate::set_parser_version((2, 6, 0));
        let (enums, structs) = process_version_schema(&[(
            "builtin.json".into(),
            "{'struct':'Holder','data':{'kind':'QType','empty':'q_empty'}}".into(),
        )])
        .unwrap();
        assert!(
            enums["QType"]
                .variants
                .iter()
                .any(|variant| variant.name == "qint")
        );
        assert!(structs.contains_key("QEmpty"));
        crate::set_parser_version((2, 5, 0));
        let error = process_version_schema(&[(
            "old.json".into(),
            "{'struct':'Holder','data':{'kind':'QType'}}".into(),
        )])
        .unwrap_err();
        assert!(format!("{error:#}").contains("Unknown QAPI type 'QType'"));
    }

    #[test]
    fn duplicate_definitions_fail_across_kinds_and_source_files() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/qemu/tests/qapi-schema");
        for name in ["redefined-type", "redefined-command", "redefined-event"] {
            let sources = crate::read_schema_with_version(
                &root.join(format!("{name}.json")),
                Some((11, 1, 3)),
            )
            .unwrap();
            let error = format!("{:#}", process_version_schema(&sources).unwrap_err());
            assert!(error.contains(name), "{name}: {error}");
            assert!(error.contains("already defined"), "{name}: {error}");
        }
        let sources = [
            (
                "first.json".into(),
                "{'struct':'Repeated','data':{}}".into(),
            ),
            (
                "included.json".into(),
                "{'enum':'Repeated','data':['one']}".into(),
            ),
        ];
        let error = format!("{:#}", process_version_schema(&sources).unwrap_err());
        for expected in ["Repeated", "first.json", "included.json"] {
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn primitive_type_names_cannot_be_redefined() {
        for name in [
            "str", "number", "int", "int8", "int16", "int32", "int64", "uint8", "uint16", "uint32",
            "uint64", "size", "bool", "any", "null",
        ] {
            let input = format!("{{'struct':'{name}','data':{{}}}}");
            let error = format!("{:#}", process(&input).unwrap_err());
            assert!(error.contains("fixture.json"), "{error}");
            assert!(error.contains("Predefined QAPI type"), "{error}");
        }
        let (_, structs) = process("{'struct':'Str','data':{'value':'str'}}").unwrap();
        assert!(structs.contains_key("Str"));
        crate::set_parser_version((1, 0, 0));
        let error =
            process_version_schema(&[("old.json".into(), "{'type':'str','data':{}}".into())])
                .unwrap_err();
        assert!(format!("{error:#}").contains("Predefined QAPI type 'str'"));
    }

    #[test]
    fn upstream_member_clash_fixtures_fail_before_codegen() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/qemu/tests/qapi-schema");
        for name in [
            "struct-base-clash",
            "struct-base-clash-deep",
            "struct-member-name-clash",
            "union-clash-member",
            "enum-clash-member",
            "alternate-clash",
        ] {
            let path = root.join(format!("{name}.json"));
            let sources = crate::read_schema_with_version(&path, Some((11, 1, 3))).unwrap();
            let error = format!("{:#}", process_version_schema(&sources).unwrap_err());
            assert!(error.contains(name), "{name}: {error}");
            assert!(error.contains("collid"), "{name}: {error}");
        }
    }

    #[test]
    fn inheritance_requires_plain_struct_bases() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/qemu/tests/qapi-schema");
        for name in ["bad-base", "union-base-union", "union-base-any"] {
            let sources = crate::read_schema_with_version(
                &root.join(format!("{name}.json")),
                Some((11, 1, 3)),
            )
            .unwrap();
            let error = format!("{:#}", process_version_schema(&sources).unwrap_err());
            assert!(error.contains(name), "{name}: {error}");
            assert!(error.contains("requires a struct type"), "{name}: {error}");
        }
        for base in ["str", "Kind", "Choice"] {
            let definitions = concat!(
                "{'enum':'Kind','data':['one']}\n",
                "{'alternate':'Choice','data':{'flag':'bool','integer':'int'}}\n",
            );
            for definition in [
                format!("{{'struct':'Derived','base':'{base}','data':{{}}}}"),
                format!("{{'union':'Derived','base':'{base}','discriminator':'kind','data':{{}}}}"),
            ] {
                let error = format!(
                    "{:#}",
                    process(&format!("{definitions}{definition}")).unwrap_err()
                );
                assert!(error.contains("fixture.json"), "{error}");
                assert!(error.contains("requires a struct type"), "{error}");
            }
        }
        let (_, structs) = process(concat!(
            "{'struct':'Derived','base':'Base','data':{'extra':'bool'}}\n",
            "{'struct':'Base','data':{'value':'str'}}",
        ))
        .unwrap();
        assert_eq!(structs["Derived"].fields.len(), 2);
    }

    #[test]
    fn separate_union_branches_can_reuse_member_names() {
        let (_, structs) = process(concat!(
            "{'enum':'Kind','data':['left','right']}\n",
            "{'struct':'Base','data':{'kind':'Kind'}}\n",
            "{'struct':'Left','data':{'value':'str'}}\n",
            "{'struct':'Right','data':{'value':'int'}}\n",
            "{'union':'Choice','base':'Base','discriminator':'kind','data':{'left':'Left','right':'Right'}}",
        )).unwrap();
        assert!(structs.contains_key("Choice"));
    }

    #[test]
    fn upstream_positive_schema_fixtures_lower() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/qemu/tests/qapi-schema");
        let mut fixtures = Vec::new();
        for entry in std::fs::read_dir(&root).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|extension| extension == "err")
                && std::fs::read_to_string(&path).unwrap().trim().is_empty()
                && path.with_extension("json").exists()
            {
                fixtures.push(path.with_extension("json"));
            }
        }
        fixtures.sort();
        assert!(!fixtures.is_empty());
        let mut failures = Vec::new();
        for path in fixtures {
            let sources = crate::read_schema_with_version(&path, Some((11, 1, 3))).unwrap();
            match std::panic::catch_unwind(|| process_version_schema(&sources)) {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => failures.push(format!("{}: {error:#}", path.display())),
                Err(_) => failures.push(format!("{}: lowering panicked", path.display())),
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    fn process(input: &str) -> Result<(HashMap<String, Enum>, HashMap<String, Struct>)> {
        crate::set_parser_version((11, 1, 3));
        process_version_schema(&[("fixture.json".into(), input.into())])
    }

    #[test]
    fn undefined_type_references_report_the_source_and_definition() {
        for (input, definition, member) in [
            (
                "{'struct':'Record','data':{'value':'Missing'}}",
                "Record",
                "value",
            ),
            (
                "{'struct':'Record','data':{'values':['Missing']}}",
                "Record",
                "values",
            ),
            (
                "{'command':'query-record','returns':'Missing'}",
                "query-record",
                "return",
            ),
            (
                "{'event':'RECORD','data':{'value':'Missing'}}",
                "RECORD",
                "value",
            ),
            (
                "{'alternate':'Choice','data':{'value':'Missing'}}",
                "Choice",
                "value",
            ),
            (
                "{'command':'query-record','data':{'value':'Missing'}}",
                "query-record",
                "value",
            ),
        ] {
            let error = format!("{:#}", process(input).unwrap_err());
            for expected in ["fixture.json", definition, member, "Missing"] {
                assert!(error.contains(expected), "{expected}: {error}");
            }
        }
    }

    #[test]
    fn type_references_use_schema_names_not_rust_names_or_command_names() {
        for input in [
            "{'struct':'FooBAR','data':{}} {'struct':'Record','data':{'value':'FooBar'}}",
            "{'command':'Example'} {'struct':'Record','data':{'value':'Example'}}",
            "{'event':'EXAMPLE'} {'struct':'Record','data':{'value':'EXAMPLE'}}",
            "{'struct':'Record','data':{'value':'String'}}",
        ] {
            let error = format!("{:#}", process(input).unwrap_err());
            assert!(error.contains("Unknown QAPI type"), "{error}");
        }
    }

    #[test]
    fn forward_member_references_resolve_across_source_files() {
        crate::set_parser_version((11, 1, 3));
        let (_, structs) = process_version_schema(&[
            (
                "first.json".into(),
                "{'struct':'First','data':{'value':'Second'}}".into(),
            ),
            (
                "second.json".into(),
                "{'struct':'Second','data':{'value':'str'}}".into(),
            ),
        ])
        .unwrap();
        assert_eq!(structs["First"].fields[0].r#type, "Second");
    }

    #[test]
    fn historical_inline_members_validate_nested_references() {
        crate::set_parser_version((1, 0, 0));
        let error = process_version_schema(&[(
            "old.json".into(),
            "{'type':'Record','data':{'outer':{'inner':'Missing'}}}".into(),
        )])
        .unwrap_err();
        let error = format!("{error:#}");
        for expected in ["old.json", "Record", "outer", "inner", "Missing"] {
            assert!(error.contains(expected), "{expected}: {error}");
        }
    }

    #[test]
    fn invalid_historical_inline_positions_return_contextual_errors() {
        crate::set_parser_version((2, 3, 0));
        for input in [
            "{'event':'BAD','data':{'object':{'value':'str'}}}",
            "{'command':'bad','data':{'object':{'value':'str'}}}",
            "{'command':'bad','returns':{'value':'str'}}",
            "{'union':'Bad','discriminator':{},'data':{'object':{'value':'str'}}}",
        ] {
            let error = process_version_schema(&[("old.json".into(), input.into())]).unwrap_err();
            let error = format!("{error:#}");
            assert!(error.contains("old.json"), "{error}");
            assert!(error.contains("Anonymous struct"), "{error}");
        }
    }

    #[test]
    fn legacy_alternate_lowers_to_a_bare_value_enum_and_keeps_features() {
        crate::set_parser_version((1, 7, 0));
        let input = "{ 'union': 'Ref', 'discriminator': {}, 'data': {'reference':'str'}, 'features':['deprecated'] }";
        let (enums, structs) =
            process_version_schema(&[("fixture.json".into(), input.into())]).unwrap();
        assert!(!structs.contains_key("Ref"));
        let value = &enums["Ref"];
        assert!(has_feature(&value.meta, "deprecated"));
        assert!(
            !value
                .meta
                .attributes
                .contains(&Attribute::new("union_branch"))
        );
    }

    #[test]
    fn all_implicit_union_branches_are_preserved_as_branch_enums() {
        let (enums, structs) = process(concat!(
            "{ 'enum': 'Kind', 'data': ['first', 'second'] }\n",
            "{ 'union': 'EmptyPayload', 'base': {'kind': 'Kind'}, 'discriminator': 'kind', 'data': {} }",
        )).unwrap();
        assert!(structs.contains_key("EmptyPayload"));
        let branches = &enums["EmptyPayloadBranch"];
        assert_eq!(branches.variants.len(), 2);
        assert!(
            branches
                .variants
                .iter()
                .all(|branch| branch.kind == EnumVariantKind::Unit)
        );
        assert!(
            branches
                .meta
                .attributes
                .iter()
                .any(|attr| matches!(attr, Attribute::Unit(name) if name == "union_branch"))
        );
        let code = branches.generate().to_string();
        assert!(code.contains("union_branch"));
    }

    #[test]
    fn invalid_discriminator_shapes_and_tags_are_rejected() {
        for (base, branches, message) in [
            ("{'*kind': 'Kind'}", "{}", "required scalar enum"),
            ("{'kind': ['Kind']}", "{}", "required scalar enum"),
            ("{'kind': 'str'}", "{}", "non-enum type"),
            ("{'kind': 'Empty'}", "{}", "has no branches"),
            ("{'kind': 'Kind'}", "{'known': ['Payload']}", "not an array"),
            (
                "{'kind': 'Kind'}",
                "{'known': 'str'}",
                "must reference a struct",
            ),
            (
                "{'kind': 'Kind'}",
                "{'unknown': 'Payload'}",
                "not a value of discriminator enum",
            ),
        ] {
            let input = format!(
                "{{ 'enum': 'Kind', 'data': ['known'] }}\n{{ 'enum': 'Empty', 'data': [] }}\n{{ 'struct': 'Payload', 'data': {{}} }}\n{{ 'union': 'Invalid', 'base': {base}, 'discriminator': 'kind', 'data': {branches} }}"
            );
            let error = process(&input).unwrap_err().to_string();
            assert!(error.contains(message), "{error}");
        }
    }

    #[test]
    fn conflicting_schema_definitions_do_not_silently_overwrite_types() {
        for input in [
            "{ 'struct': 'Example', 'data': {} }\n{ 'struct': 'Example', 'data': {} }",
            "{ 'command': 'foo-bar' }\n{ 'command': 'foo_bar' }",
        ] {
            let error = process(input).unwrap_err().to_string();
            assert!(error.contains("Conflicting QAPI definitions"), "{error}");
        }
    }

    #[test]
    fn distinct_raw_type_names_keep_separate_rust_identities() {
        let (enums, structs) =
            process("{ 'struct':'FooBAR', 'data':{} } { 'enum':'FooBar', 'data':['one'] }")
                .unwrap();
        assert!(enums.contains_key("FooBar"));
        assert!(structs.contains_key("FooBarGenerated"));
    }

    #[test]
    fn missing_argument_structs_return_an_error() {
        for input in [
            "{ 'command': 'example', 'data': 'Missing' }",
            "{ 'event': 'EXAMPLE', 'data': 'Missing' }",
        ] {
            let error = format!("{:#}", process(input).unwrap_err());
            assert!(error.contains("Unknown QAPI type 'Missing'"), "{error}");
        }
    }

    #[test]
    fn inherited_structs_resolve_forward_base_chains() {
        let (_, structs) = process(concat!(
            "{ 'struct': 'Leaf', 'base': 'Middle', 'data': { 'leaf': 'int' } }\n",
            "{ 'struct': 'Middle', 'base': 'Root', 'data': { 'middle': 'int' } }\n",
            "{ 'struct': 'Root', 'data': { 'root': 'int' } }\n",
        ))
        .unwrap();
        let fields: Vec<_> = structs["Leaf"]
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect();
        assert_eq!(fields, ["root", "middle", "leaf"]);
    }

    #[test]
    fn unresolved_inheritance_returns_an_error() {
        for (input, expected) in [
            (
                "{ 'struct': 'Leaf', 'base': 'Missing', 'data': {} }",
                "Unknown QAPI type 'Missing'",
            ),
            (
                "{ 'struct': 'First', 'base': 'Second', 'data': {} }\n{ 'struct': 'Second', 'base': 'First', 'data': {} }",
                "Unresolved QAPI struct bases",
            ),
        ] {
            let error = format!("{:#}", process(input).unwrap_err());
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn invalid_union_bases_return_an_error() {
        for (input, message) in [
            (
                "{ 'union': 'ExampleUnion', 'base': 'Missing', 'discriminator': 'kind', 'data': { 'one': 'Missing' } }",
                "Unknown QAPI type 'Missing'",
            ),
            (
                "{ 'struct': 'Base', 'data': {} }\n{ 'union': 'ExampleUnion', 'base': 'Base', 'discriminator': 'kind', 'data': { 'one': 'Base' } }",
                "no discriminator field kind",
            ),
        ] {
            let error = format!("{:#}", process(input).unwrap_err());
            assert!(error.contains(message), "{error}");
        }
    }
}

#[cfg(test)]
mod enum_history_tests {
    use super::*;

    fn enumeration(wire: &str, payload: EnumVariantKind, array: bool) -> Enum {
        Enum {
            name: "Example".into(),
            meta: Metadata::default(),
            variants: vec![EnumVariant {
                name: wire.into(),
                kind: payload,
                array,
                meta: Metadata {
                    doc: None,
                    attributes: vec![Attribute::with_value("name", wire)],
                },
            }],
        }
    }

    #[test]
    fn latest_type_and_variant_conditions_replace_or_clear_old_conditions() {
        let mut registry = UnifiedRegistry::default();
        let mut value = enumeration("value", EnumVariantKind::Unit, false);
        value
            .meta
            .attributes
            .push(Attribute::with_value("condition", "CONFIG_A"));
        value.variants[0]
            .meta
            .attributes
            .push(Attribute::with_value("condition", "CONFIG_A"));
        merge_enum(&mut registry, "Example", &value, (1, 0, 0));
        value.meta.attributes = vec![Attribute::with_value("condition", "CONFIG_B")];
        value.variants[0]
            .meta
            .attributes
            .retain(|attr| !matches!(attr, Attribute::Item {name,..} if name == "condition"));
        merge_enum(&mut registry, "Example", &value, (2, 0, 0));
        let UnifiedType::Enum(value) = &registry.types["Example"] else {
            panic!()
        };
        assert_eq!(condition(&value.ir.meta), Some("CONFIG_B"));
        assert_eq!(condition(&value.ir.variants[0].meta), None);
        let mut value = Struct {
            name: "Record".into(),
            fields: vec![],
            meta: Metadata::default(),
        };
        value
            .meta
            .attributes
            .push(Attribute::with_value("condition", "CONFIG_A"));
        merge_struct(&mut registry, "Record", &value, (1, 0, 0));
        value.meta.attributes.clear();
        merge_struct(&mut registry, "Record", &value, (2, 0, 0));
        let UnifiedType::Struct(value) = &registry.types["Record"] else {
            panic!()
        };
        assert_eq!(condition(&value.ir.meta), None);
    }

    #[test]
    fn reintroduced_variants_keep_the_gap_between_lifetimes() {
        let mut registry = UnifiedRegistry::default();
        let present = enumeration("returning", EnumVariantKind::Unit, false);
        let mut absent = present.clone();
        absent.variants.clear();
        for (major, value) in [(1, &present), (2, &absent), (3, &present)] {
            merge_enum(&mut registry, "Example", value, (major, 0, 0));
        }
        materialize_variant_history(&mut registry);
        let UnifiedType::Enum(value) = &registry.types["Example"] else {
            panic!()
        };
        assert_eq!(value.variant_versions["returning"].since, (3, 0, 0));
        assert_eq!(value.variant_history.len(), 1);
        assert_eq!(value.variant_history[0].1.until, Some((2, 0, 0)));
    }

    #[test]
    fn payload_history_keeps_each_shape_and_avoids_real_variant_name_collisions() {
        let mut registry = UnifiedRegistry::default();
        for (major, payload, array) in [
            (1, "int", false),
            (2, "str", false),
            (3, "int", true),
            (4, "int64", true),
        ] {
            let mut input = enumeration("value", EnumVariantKind::Tuple(payload.into()), array);
            input.variants.push(EnumVariant {
                name: "value-legacy".into(),
                kind: EnumVariantKind::Unit,
                array: false,
                meta: Metadata {
                    doc: None,
                    attributes: vec![Attribute::with_value("name", "value-legacy")],
                },
            });
            merge_enum(&mut registry, "Example", &input, (major, 0, 0));
        }
        materialize_variant_history(&mut registry);
        materialize_field_history(&mut registry);
        let UnifiedType::Enum(value) = &registry.types["Example"] else {
            panic!()
        };
        assert_eq!(
            value.variant_history.len(),
            2,
            "Rust-equivalent int/int64 must not create another representation"
        );
        assert_eq!(value.ir.variants.len(), 4);
        assert_eq!(value.variant_versions["value"].since, (3, 0, 0));
        assert_eq!(
            value.variant_versions["ValueLegacy2"].until,
            Some((2, 0, 0))
        );
        assert_eq!(
            value.variant_versions["ValueLegacy3"].until,
            Some((3, 0, 0))
        );
        assert!(
            value
                .ir
                .variants
                .iter()
                .filter(|variant| variant.name.starts_with("ValueLegacy"))
                .all(|variant| variant
                    .meta
                    .attributes
                    .contains(&Attribute::with_value("name", "value")))
        );
    }

    #[test]
    fn wire_renames_with_the_same_rust_name_keep_both_spellings() {
        let mut registry = UnifiedRegistry::default();
        merge_enum(
            &mut registry,
            "Example",
            &enumeration("foo-bar", EnumVariantKind::Unit, false),
            (1, 0, 0),
        );
        merge_enum(
            &mut registry,
            "Example",
            &enumeration("foo_bar", EnumVariantKind::Unit, false),
            (2, 0, 0),
        );
        materialize_variant_history(&mut registry);
        materialize_field_history(&mut registry);
        let UnifiedType::Enum(value) = &registry.types["Example"] else {
            panic!()
        };
        assert_eq!(value.variant_versions["foo_bar"].since, (2, 0, 0));
        assert_eq!(
            value.variant_versions["FooBarLegacy"].until,
            Some((2, 0, 0))
        );
        let old = value
            .ir
            .variants
            .iter()
            .find(|variant| variant.name == "FooBarLegacy")
            .unwrap();
        assert!(
            old.meta
                .attributes
                .contains(&Attribute::with_value("name", "foo-bar"))
        );
    }
}

#[cfg(test)]
mod field_history_tests {
    use super::*;

    fn field(name: &str, ty: &str, array: bool) -> StructField {
        StructField {
            name: name.into(),
            r#type: ty.into(),
            array,
            optional: false,
            meta: Metadata::default(),
        }
    }

    fn record(fields: Vec<StructField>) -> Struct {
        Struct {
            name: "Record".into(),
            fields,
            meta: Metadata::default(),
        }
    }

    #[test]
    fn optional_fields_keep_one_lifetime_when_conditions_change() {
        let mut registry = UnifiedRegistry::default();
        let mut value = field("value", "str", false);
        value.optional = true;
        value
            .meta
            .attributes
            .push(Attribute::with_value("condition", "CONFIG_A"));
        merge_struct(
            &mut registry,
            "Record",
            &record(vec![value.clone()]),
            (1, 0, 0),
        );
        value.meta.attributes.clear();
        merge_struct(&mut registry, "Record", &record(vec![value]), (2, 0, 0));
        let UnifiedType::Struct(value) = &registry.types["Record"] else {
            panic!()
        };
        assert!(value.field_history.is_empty());
        assert_eq!(value.field_versions["value"].since, (1, 0, 0));
        assert_eq!(condition(&value.ir.fields[0].meta), None);
    }

    #[test]
    fn conditional_field_presence_preserves_requiredness_and_latest_conditions() {
        let mut registry = UnifiedRegistry::default();
        for (major, condition) in [(1, Some("CONFIG_A")), (2, None), (3, Some("CONFIG_B"))] {
            let mut value = field("value", "str", false);
            if let Some(condition) = condition {
                value
                    .meta
                    .attributes
                    .push(Attribute::with_value("condition", condition));
            }
            merge_struct(&mut registry, "Record", &record(vec![value]), (major, 0, 0));
        }
        materialize_field_history(&mut registry);
        let UnifiedType::Struct(value) = &registry.types["Record"] else {
            panic!()
        };
        assert_eq!(value.field_history.len(), 2);
        assert!(
            value.ir.fields[0]
                .meta
                .attributes
                .contains(&Attribute::with_value("condition", "CONFIG_B"))
        );
        assert!(
            !value.field_history[1]
                .0
                .meta
                .attributes
                .iter()
                .any(|attr| matches!(attr, Attribute::Item {name,..} if name == "condition"))
        );
    }

    #[test]
    fn reintroduced_fields_keep_the_gap_between_lifetimes() {
        let mut registry = UnifiedRegistry::default();
        let present = record(vec![field("returning", "str", false)]);
        let absent = record(vec![]);
        for (major, value) in [(1, &present), (2, &absent), (3, &present)] {
            merge_struct(&mut registry, "Record", value, (major, 0, 0));
        }
        materialize_field_history(&mut registry);
        let UnifiedType::Struct(value) = &registry.types["Record"] else {
            panic!()
        };
        assert_eq!(value.field_versions["returning"].since, (3, 0, 0));
        assert_eq!(value.field_history.len(), 1);
        assert_eq!(value.field_history[0].1.until, Some((2, 0, 0)));
    }

    #[test]
    fn field_optionality_changes_preserve_each_wire_contract() {
        let mut registry = UnifiedRegistry::default();
        for (major, optional) in [(1, true), (2, false), (3, true)] {
            let mut value = field("value", "str", false);
            value.optional = optional;
            merge_struct(&mut registry, "Record", &record(vec![value]), (major, 0, 0));
        }
        materialize_field_history(&mut registry);
        let UnifiedType::Struct(value) = &registry.types["Record"] else {
            panic!()
        };
        assert_eq!(value.field_history.len(), 2);
        assert_eq!(value.field_versions["value"].since, (3, 0, 0));
        assert!(value.field_history[0].0.optional);
        assert!(!value.field_history[1].0.optional);
    }

    #[test]
    fn field_type_history_preserves_arrays_and_does_not_collide_with_real_fields() {
        let mut registry = UnifiedRegistry::default();
        for (major, ty, array) in [
            (1, "int", false),
            (2, "str", false),
            (3, "int", true),
            (4, "int64", true),
        ] {
            let value = record(vec![
                field("value", ty, array),
                field("value_legacy", "bool", false),
            ]);
            merge_struct(&mut registry, "Record", &value, (major, 0, 0));
        }
        materialize_field_history(&mut registry);
        let UnifiedType::Struct(value) = &registry.types["Record"] else {
            panic!()
        };
        assert_eq!(value.field_history.len(), 2);
        assert_eq!(value.field_versions["value"].since, (3, 0, 0));
        assert_eq!(value.field_versions["value_legacy2"].until, Some((2, 0, 0)));
        assert_eq!(value.field_versions["value_legacy3"].until, Some((3, 0, 0)));
        assert_eq!(value.ir.fields.len(), 4);
    }

    #[test]
    fn field_wire_renames_preserve_original_keys() {
        let mut registry = UnifiedRegistry::default();
        merge_struct(
            &mut registry,
            "Record",
            &record(vec![field("foo-bar", "str", false)]),
            (1, 0, 0),
        );
        merge_struct(
            &mut registry,
            "Record",
            &record(vec![field("foo_bar", "str", false)]),
            (2, 0, 0),
        );
        materialize_field_history(&mut registry);
        let UnifiedType::Struct(value) = &registry.types["Record"] else {
            panic!()
        };
        assert_eq!(value.field_versions["foo_bar"].since, (2, 0, 0));
        let old = value
            .ir
            .fields
            .iter()
            .find(|field| field.name == "foo_bar_legacy")
            .unwrap();
        assert!(
            old.meta
                .attributes
                .contains(&Attribute::with_value("name", "foo-bar"))
        );
    }

    #[test]
    fn recursive_vectors_break_size_cycles_but_inline_references_require_boxing() {
        for array in [false, true] {
            let mut registry = UnifiedRegistry::default();
            let value = Struct {
                name: "Node".into(),
                fields: vec![field("children", "Node", array)],
                meta: Metadata::default(),
            };
            merge_struct(&mut registry, "Node", &value, (1, 0, 0));
            fix_recursive_types(&mut registry);
            let UnifiedType::Struct(value) = &registry.types["Node"] else {
                panic!()
            };
            assert_eq!(
                value.ir.fields[0]
                    .meta
                    .attributes
                    .contains(&Attribute::new("boxed")),
                !array
            );
        }
        let mut registry = UnifiedRegistry::default();
        let value = Struct {
            name: "Node".into(),
            fields: vec![field("value", "Branch", false)],
            meta: Metadata::default(),
        };
        merge_struct(&mut registry, "Node", &value, (1, 0, 0));
        merge_enum(
            &mut registry,
            "Branch",
            &Enum {
                name: "Branch".into(),
                meta: Metadata::default(),
                variants: vec![EnumVariant {
                    name: "nodes".into(),
                    kind: EnumVariantKind::Tuple("Node".into()),
                    array: true,
                    meta: Metadata::default(),
                }],
            },
            (1, 0, 0),
        );
        fix_recursive_types(&mut registry);
        let UnifiedType::Struct(value) = &registry.types["Node"] else {
            panic!()
        };
        assert!(
            !value.ir.fields[0]
                .meta
                .attributes
                .contains(&Attribute::new("boxed"))
        );
    }
}

//! Multi-version QAPI schema merge.
//!
//! Parses schemas from multiple QEMU version tags, merges them into a single
//! unified type registry where each type, field, and variant has version range
//! annotations (since/until).

use crate::versions::{extract_schema_from_tag, find_extracted_schema_entry, parse_version_tag};
use crate::{read_schema, parse_schema, QapiSchemaToken};
use crate::qapi_ir::*;
use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

pub type Version = (u32, u32, u32);

fn version_str(v: Version) -> String {
    if v.2 == 0 {
        format!("{}.{}", v.0, v.1)
    } else {
        format!("{}.{}.{}", v.0, v.1, v.2)
    }
}

#[derive(Debug, Clone)]
pub struct VersionRange {
    pub since: Version,
    pub until: Option<Version>,
}

impl VersionRange {
    pub fn new(since: Version) -> Self {
        Self { since, until: None }
    }
}

/// An enum with version-annotated variants.
#[derive(Debug, Clone)]
pub struct UnifiedEnum {
    pub ir: Enum,
    pub versions: VersionRange,
    /// Per-variant version ranges (keyed by variant name)
    pub variant_versions: BTreeMap<String, VersionRange>,
}

/// A struct with version-annotated fields.
#[derive(Debug, Clone)]
pub struct UnifiedStruct {
    pub ir: Struct,
    pub versions: VersionRange,
    /// Per-field version ranges (keyed by field name)
    pub field_versions: BTreeMap<String, VersionRange>,
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

/// Process a single version's schema through the IR pipeline.
/// Returns maps of enum name → Enum IR and struct name → Struct IR.
fn process_version_schema(
    sources: &[(std::path::PathBuf, String)],
) -> Result<(HashMap<String, Enum>, HashMap<String, Struct>)> {
    let mut enums_lookup = HashMap::new();
    let mut structs_lookup = HashMap::new();
    let mut unprocessed_structs = Vec::new();
    let mut unprocessed_unions = Vec::new();
    let mut unprocessed_commands = Vec::new();
    let mut unprocessed_events = Vec::new();

    for (_path, content) in sources {
        let schema = parse_schema(content)?;
        for token in &schema.0 {
            match token {
                QapiSchemaToken::Enum(v) => {
                    let processed = process_enum(v.clone());
                    enums_lookup.insert(processed.name.clone(), processed);
                }
                QapiSchemaToken::Alternate(v) => {
                    let processed = process_alternate(v.clone());
                    enums_lookup.insert(processed.name.clone(), processed);
                }
                QapiSchemaToken::Struct(v) => {
                    if v.base.is_some() {
                        unprocessed_structs.push(v.clone());
                        continue;
                    }
                    let processed = process_struct(v.clone(), &structs_lookup);
                    structs_lookup.insert(processed.name.clone(), processed);
                }
                QapiSchemaToken::Union(v) => {
                    unprocessed_unions.push(v.clone());
                }
                QapiSchemaToken::Command(v) => {
                    unprocessed_commands.push(v.clone());
                }
                QapiSchemaToken::Event(v) => {
                    unprocessed_events.push(v.clone());
                }
                _ => continue,
            }
        }
    }

    for v in unprocessed_structs {
        let processed = process_struct(v, &structs_lookup);
        structs_lookup.insert(processed.name.clone(), processed);
    }
    for v in unprocessed_unions {
        let (s, e, disc_enum) = process_union(v, &structs_lookup, &enums_lookup);
        enums_lookup.insert(e.name.clone(), e);
        if let Some(de) = disc_enum {
            enums_lookup.insert(de.name.clone(), de);
        }
        structs_lookup.insert(s.name.clone(), s);
    }
    for v in unprocessed_events {
        let processed = process_event(v, &structs_lookup);
        structs_lookup.insert(processed.name.clone(), processed);
    }
    for v in unprocessed_commands {
        let processed = process_command(v);
        // Commands may collide with structs of the same name (e.g., block-dirty-bitmap-add
        // command uses BlockDirtyBitmapAdd struct as its data). The command wins.
        structs_lookup.insert(processed.name.clone(), processed);
    }

    Ok((enums_lookup, structs_lookup))
}

/// Parse schemas from multiple versions and build a unified registry.
pub fn build_unified_registry(
    qemu_repo: &Path,
    version_tags: &[&str],
) -> Result<UnifiedRegistry> {
    let mut registry = UnifiedRegistry::default();

    for tag in version_tags {
        let version = parse_version_tag(tag)
            .with_context(|| format!("Invalid version tag: {}", tag))?;
        registry.versions.push(version);

        eprintln!("Processing {}...", tag);

        let tmpdir = extract_schema_from_tag(qemu_repo, tag)
            .with_context(|| format!("Failed to extract schema for {}", tag))?;

        let schema_entry = find_extracted_schema_entry(tmpdir.path())
            .with_context(|| format!("No schema entry point found for {}", tag))?;

        let sources = read_schema(&schema_entry)
            .with_context(|| format!("Failed to read schema for {}", tag))?;

        let (enums, structs) = process_version_schema(&sources)
            .with_context(|| format!("Failed to process IR for {}", tag))?;

        // Track which type names appear in this version
        let mut seen_types = BTreeSet::new();

        // Merge enums
        for (name, e) in &enums {
            seen_types.insert(name.clone());
            merge_enum(&mut registry, name, e, version);
        }

        // Merge structs
        for (name, s) in &structs {
            seen_types.insert(name.clone());
            merge_struct(&mut registry, name, s, version);
        }

        // Mark types NOT seen in this version as removed
        for (name, ut) in registry.types.iter_mut() {
            if !seen_types.contains(name) {
                let vr = match ut {
                    UnifiedType::Enum(e) => &mut e.versions,
                    UnifiedType::Struct(s) => &mut s.versions,
                };
                if vr.until.is_none() && vr.since < version {
                    vr.until = Some(version);
                }
            }
        }
    }

    // Post-process: detect and fix recursive types by adding Box wrapping
    fix_recursive_types(&mut registry);

    let total = registry.types.len();
    let removed = registry.types.values().filter(|t| match t {
        UnifiedType::Enum(e) => e.versions.until.is_some(),
        UnifiedType::Struct(s) => s.versions.until.is_some(),
    }).count();
    eprintln!("Unified registry: {} types total, {} removed across versions", total, removed);

    Ok(registry)
}

/// Map from QAPI variant name to its Rust name (for detecting renames).
fn variant_rust_name(v: &EnumVariant) -> String {
    crate::qapi_ir::rustify_name(&v.name)
}

fn merge_enum(registry: &mut UnifiedRegistry, name: &str, e: &Enum, version: Version) {
    if let Some(UnifiedType::Enum(existing)) = registry.types.get_mut(name) {
        existing.versions.until = None;

        let mut seen_variants = BTreeSet::new();
        for variant in &e.variants {
            seen_variants.insert(variant.name.clone());
            if existing.variant_versions.contains_key(&variant.name) {
                // Variant still exists — clear until
                existing.variant_versions.get_mut(&variant.name).unwrap().until = None;
            } else {
                // Check if this is a rename (same Rust name, different QAPI name)
                let rust_name = variant_rust_name(variant);
                let renamed_from = existing.ir.variants.iter().position(|v| {
                    variant_rust_name(v) == rust_name && v.name != variant.name
                });

                if let Some(old_idx) = renamed_from {
                    // It's a rename: update the old variant's wire name to the new one
                    let old_name = existing.ir.variants[old_idx].name.clone();
                    let old_vr = existing.variant_versions.remove(&old_name).unwrap();
                    // Replace old variant IR with new one, keeping the old version range
                    existing.ir.variants[old_idx] = variant.clone();
                    existing.variant_versions.insert(variant.name.clone(), old_vr);
                    // Also mark the old QAPI name as seen so it doesn't get marked as removed
                    seen_variants.insert(old_name);
                } else {
                    // Genuinely new variant
                    existing.variant_versions.insert(
                        variant.name.clone(),
                        VersionRange::new(version),
                    );
                    if !existing.ir.variants.iter().any(|v| v.name == variant.name) {
                        existing.ir.variants.push(variant.clone());
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
        let variant_versions: BTreeMap<String, VersionRange> = e.variants.iter()
            .map(|v| (v.name.clone(), VersionRange::new(version)))
            .collect();
        registry.types.insert(name.to_string(), UnifiedType::Enum(UnifiedEnum {
            ir: e.clone(),
            versions: VersionRange::new(version),
            variant_versions,
        }));
    }
}

fn field_rust_name(f: &StructField) -> String {
    crate::qapi_ir::rustify_field(&f.name)
}

fn merge_struct(registry: &mut UnifiedRegistry, name: &str, s: &Struct, version: Version) {
    if let Some(UnifiedType::Struct(existing)) = registry.types.get_mut(name) {
        existing.versions.until = None;

        let mut seen_fields = BTreeSet::new();
        for field in &s.fields {
            seen_fields.insert(field.name.clone());
            if existing.field_versions.contains_key(&field.name) {
                existing.field_versions.get_mut(&field.name).unwrap().until = None;
            } else {
                // Check for rename (same Rust field name, different QAPI name)
                let rust_name = field_rust_name(field);
                let renamed_from = existing.ir.fields.iter().position(|f| {
                    field_rust_name(f) == rust_name && f.name != field.name
                });

                if let Some(old_idx) = renamed_from {
                    let old_name = existing.ir.fields[old_idx].name.clone();
                    let old_vr = existing.field_versions.remove(&old_name).unwrap();
                    existing.ir.fields[old_idx] = field.clone();
                    existing.field_versions.insert(field.name.clone(), old_vr);
                    seen_fields.insert(old_name);
                } else {
                    // Check for Rust name collision with existing fields
                    let rust_name = field_rust_name(field);
                    let has_collision = existing.ir.fields.iter().any(|f| {
                        field_rust_name(f) == rust_name && f.name != field.name
                    });

                    let mut field = field.clone();
                    if has_collision {
                        // Append a suffix to disambiguate
                        let new_name = format!("{}_alt", field.name);
                        eprintln!("WARNING: Renaming field '{}' to '{}' in '{}' to avoid collision", field.name, new_name, name);
                        // Store original QAPI name in the name attribute
                        if !field.meta.attributes.iter().any(|a| matches!(a, Attribute::Item { name, .. } if name == "name")) {
                            field.meta.attributes.push(Attribute::with_value("name", &field.name));
                        }
                        field.name = new_name.clone();
                        existing.field_versions.insert(
                            new_name,
                            VersionRange::new(version),
                        );
                    } else {
                        existing.field_versions.insert(
                            field.name.clone(),
                            VersionRange::new(version),
                        );
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
        let mut field_versions = BTreeMap::new();
        let mut seen_rust_names: BTreeSet<String> = BTreeSet::new();
        for field in &mut ir.fields {
            let rust_name = field_rust_name(field);
            if !seen_rust_names.insert(rust_name.clone()) {
                let new_name = format!("{}_alt", field.name);
                eprintln!("WARNING: Renaming field '{}' to '{}' in '{}' to avoid collision", field.name, new_name, name);
                if !field.meta.attributes.iter().any(|a| matches!(a, Attribute::Item { name, .. } if name == "name")) {
                    field.meta.attributes.push(Attribute::with_value("name", &field.name));
                }
                field.name = new_name.clone();
                field_versions.insert(new_name, VersionRange::new(version));
            } else {
                field_versions.insert(field.name.clone(), VersionRange::new(version));
            }
        }
        registry.types.insert(name.to_string(), UnifiedType::Struct(UnifiedStruct {
            ir,
            versions: VersionRange::new(version),
            field_versions,
        }));
    }
}

/// Detect and fix recursive type references by marking fields that need Box wrapping.
/// A field is recursive if its type (directly or through an enum) refers back to the
/// containing struct.
fn fix_recursive_types(registry: &mut UnifiedRegistry) {
    // Build a dependency graph: type name → set of types it references
    let mut deps: HashMap<String, BTreeSet<String>> = HashMap::new();
    for (name, ut) in &registry.types {
        let mut referenced = BTreeSet::new();
        match ut {
            UnifiedType::Struct(us) => {
                for field in &us.ir.fields {
                    referenced.insert(field.r#type.clone());
                }
            }
            UnifiedType::Enum(ue) => {
                for variant in &ue.ir.variants {
                    if let EnumVariantKind::Tuple(t) = &variant.kind {
                        referenced.insert(t.clone());
                    }
                }
            }
        }
        deps.insert(name.clone(), referenced);
    }

    // Find types that are part of a cycle
    let mut needs_box: BTreeSet<(String, String)> = BTreeSet::new(); // (struct_name, field_name)

    for (name, ut) in &registry.types {
        if let UnifiedType::Struct(us) = ut {
            for field in &us.ir.fields {
                if is_reachable(&field.r#type, name, &deps, &mut BTreeSet::new()) {
                    needs_box.insert((name.clone(), field.name.clone()));
                }
            }
        }
    }

    // Apply Box wrapping by setting a flag on the field
    for (type_name, field_name) in &needs_box {
        if let Some(UnifiedType::Struct(us)) = registry.types.get_mut(type_name) {
            for field in &mut us.ir.fields {
                if field.name == *field_name {
                    eprintln!("INFO: Boxing field '{}.{}' to break recursive type", type_name, field_name);
                    // Use a special marker that the codegen will detect.
                    // The codegen checks field.meta for "boxed" and wraps in Box<>.
                    field.meta.attributes.push(Attribute::new("boxed"));
                }
            }
        }
    }
}

/// Check if `target` is reachable from `start` through the dependency graph.
fn is_reachable(
    start: &str,
    target: &str,
    deps: &HashMap<String, BTreeSet<String>>,
    visited: &mut BTreeSet<String>,
) -> bool {
    if start == target {
        return true;
    }
    if !visited.insert(start.to_string()) {
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

/// Generate unified code from the registry.
pub fn generate_unified_code(
    qemu_repo: &Path,
    version_tags: &[&str],
) -> Result<String> {
    let registry = build_unified_registry(qemu_repo, version_tags)?;

    let mut output = String::new();
    output.push_str("#![allow(unused_imports, non_camel_case_types, dead_code)]\n");
    output.push_str("use crate as qapi;\n");
    output.push_str("use qapi_macros::qapi;\n");
    output.push_str("use std::collections::HashMap;\n\n");

    // Track Rust names to detect collisions (e.g., struct BlockDirtyBitmapAdd vs
    // command block-dirty-bitmap-add both produce the same Rust name).
    // Pre-populate with Rust reserved/builtin type names that should never be shadowed.
    let mut seen_rust_names: BTreeSet<String> = ["String", "Option", "Vec", "Box", "Result"]
        .iter().map(|s| s.to_string()).collect();

    // Generate types in a stable order.
    // First pass: enums (types often reference enums).
    // Second pass: structs (including commands/events).
    // Within each pass, prefer non-command types over command types for collisions.
    let enum_entries: Vec<_> = registry.types.iter()
        .filter(|(_, ut)| matches!(ut, UnifiedType::Enum(_)))
        .collect();
    let mut struct_entries: Vec<_> = registry.types.iter()
        .filter(|(_, ut)| matches!(ut, UnifiedType::Struct(_)))
        .collect();

    // Sort structs: non-commands first, then commands. This means if a struct
    // and a command have the same Rust name, the struct wins.
    struct_entries.sort_by_key(|(_, ut)| {
        if let UnifiedType::Struct(us) = ut {
            let is_cmd = us.ir.meta.attributes.iter().any(|a| matches!(a, Attribute::Unit(n) if n == "command"));
            if is_cmd { 1 } else { 0 }
        } else {
            0
        }
    });

    for (name, ut) in enum_entries.iter().chain(struct_entries.iter()) {
        let rust_name = match ut {
            UnifiedType::Enum(ue) => rustify_name(&ue.ir.name),
            UnifiedType::Struct(us) => rustify_name(&us.ir.name),
        };

        if !seen_rust_names.insert(rust_name.clone()) {
            eprintln!("WARNING: Skipping duplicate Rust name '{}' (QAPI name: '{}')", rust_name, name);
            continue;
        }

        match ut {
            UnifiedType::Enum(ue) => {
                let code = generate_unified_enum(ue, &registry);
                match syn::parse2::<syn::File>(code.clone()) {
                    Ok(syntree) => output.push_str(&prettyplease::unparse(&syntree)),
                    Err(e) => {
                        eprintln!("WARNING: Failed to parse generated enum '{}': {}", name, e);
                        output.push_str(&format!("// ERROR generating {}: {}\n", name, e));
                    }
                }
            }
            UnifiedType::Struct(us) => {
                let code = generate_unified_struct(us, &registry);
                match syn::parse2::<syn::File>(code.clone()) {
                    Ok(syntree) => output.push_str(&prettyplease::unparse(&syntree)),
                    Err(e) => {
                        eprintln!("WARNING: Failed to parse generated struct '{}': {}", name, e);
                        output.push_str(&format!("// ERROR generating {}: {}\n", name, e));
                    }
                }
            }
        }
    }

    Ok(output)
}

/// Generate code for a unified enum, adding since/until attributes from the registry.
fn generate_unified_enum(ue: &UnifiedEnum, _registry: &UnifiedRegistry) -> proc_macro2::TokenStream {

    let mut ir = ue.ir.clone();

    // Add type-level since from registry if not already present
    let has_since = ir.meta.attributes.iter().any(|a| matches!(a, Attribute::Item { name, .. } if name == "since"));
    if !has_since {
        ir.meta.attributes.push(Attribute::with_value("since", version_str(ue.versions.since)));
    }

    // Add until if the type was removed
    if let Some(until) = ue.versions.until {
        ir.meta.attributes.push(Attribute::with_value("until", version_str(until)));
    }

    // Add per-variant since/until
    for variant in &mut ir.variants {
        if let Some(vr) = ue.variant_versions.get(&variant.name) {
            // Add since if variant appeared after the type
            if vr.since > ue.versions.since {
                let has_since = variant.meta.attributes.iter().any(|a| matches!(a, Attribute::Item { name, .. } if name == "since"));
                if !has_since {
                    variant.meta.attributes.push(Attribute::with_value("since", version_str(vr.since)));
                }
            }
            // Add until if variant was removed
            if let Some(until) = vr.until {
                variant.meta.attributes.push(Attribute::with_value("until", version_str(until)));
            }
        }
    }

    ir.generate()
}

/// Generate code for a unified struct, adding since/until and making removed/added fields optional.
fn generate_unified_struct(us: &UnifiedStruct, _registry: &UnifiedRegistry) -> proc_macro2::TokenStream {

    let mut ir = us.ir.clone();

    // Add type-level since from registry
    let has_since = ir.meta.attributes.iter().any(|a| matches!(a, Attribute::Item { name, .. } if name == "since"));
    if !has_since {
        ir.meta.attributes.push(Attribute::with_value("since", version_str(us.versions.since)));
    }

    if let Some(until) = us.versions.until {
        ir.meta.attributes.push(Attribute::with_value("until", version_str(until)));
    }

    // Add per-field since/until and make fields optional if they don't span the full type range
    for field in &mut ir.fields {
        if let Some(fr) = us.field_versions.get(&field.name) {
            // Field appeared after the type — must be optional
            if fr.since > us.versions.since {
                let has_since = field.meta.attributes.iter().any(|a| matches!(a, Attribute::Item { name, .. } if name == "since"));
                if !has_since {
                    field.meta.attributes.push(Attribute::with_value("since", version_str(fr.since)));
                }
                // Make optional if not already
                if !field.optional {
                    field.optional = true;
                }
            }
            // Field was removed — must be optional
            if let Some(until) = fr.until {
                field.meta.attributes.push(Attribute::with_value("until", version_str(until)));
                if !field.optional {
                    field.optional = true;
                }
            }
        }
    }

    ir.generate()
}

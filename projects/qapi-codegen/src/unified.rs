//! Multi-version QAPI schema merge.
//!
//! Parses schemas from multiple QEMU version tags, merges them into a single
//! unified type registry where each type, field, and variant has version range
//! annotations (since/until).

use crate::versions::{extract_schema_from_tag, find_extracted_schema_entry, parse_version_tag};
use crate::{read_schema_with_version, parse_schema, QapiSchemaToken};
use crate::qapi_ir::*;
use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

pub type Version = (u32, u32, u32);

fn version_str(v: Version) -> String {
    // Always use major.minor.patch format, minimum 1.0.0
    let v = (v.0.max(1), v.1, v.2);
    format!("{}.{}.{}", v.0, v.1, v.2)
}

/// Normalize "Since X.Y" patterns in doc strings to "Since X.Y.0"
/// Strip "since" version information from doc strings.
/// The since/until metadata is already captured in #[qapi(since/until)] attributes,
/// so it should not appear in the generated Rust doc comments.
/// Clean a doc string: strip leading `# ` from each line (QAPI comment syntax)
/// but preserve whitespace structure and all content.
fn clean_doc(doc: &str) -> Option<String> {
    let doc = doc.trim();
    if doc.is_empty() {
        return None;
    }
    Some(doc.to_string())
}

#[derive(Debug, Clone)]
pub struct VersionRange {
    pub since: Version,
    pub until: Option<Version>,
    pub deprecated: Option<Version>,
}

impl VersionRange {
    pub fn new(since: Version) -> Self {
        Self { since, until: None, deprecated: None }
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
                    let (processed, extracted) = process_struct(v.clone(), &structs_lookup);
                    structs_lookup.insert(processed.name.clone(), processed);
                    for e in extracted {
                        structs_lookup.insert(e.name.clone(), e);
                    }
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
        let (processed, extracted) = process_struct(v, &structs_lookup);
        structs_lookup.insert(processed.name.clone(), processed);
        for e in extracted {
            structs_lookup.insert(e.name.clone(), e);
        }
    }
    for v in unprocessed_unions {
        let (s, e, disc_enum) = process_union(v, &structs_lookup, &enums_lookup);
        if let Some(de) = disc_enum {
            enums_lookup.insert(de.name.clone(), de);
        }
        enums_lookup.insert(e.name.clone(), e);
        structs_lookup.insert(s.name.clone(), s);
    }
    for v in unprocessed_events {
        let processed = process_event(v, &structs_lookup);
        structs_lookup.insert(processed.name.clone(), processed);
    }
    for v in unprocessed_commands {
        // Check if this command references an existing struct via data
        let data_ref_name = match &v.data {
            Some(crate::MembersOrRef::Ref(name)) => Some(name.to_string()),
            _ => None,
        };

        let processed = process_command(v.clone());

        if let Some(ref ref_name) = data_ref_name {
            if let Some(existing) = structs_lookup.get_mut(ref_name.as_str()) {
                // Command references an existing struct — merge command attributes onto it.
                for attr in &processed.meta.attributes {
                    match attr {
                        Attribute::Unit(n) if n == "command" || n == "allow_oob" || n == "allow_preconfig" || n == "no_success_response" => {
                            if !existing.meta.attributes.contains(attr) {
                                existing.meta.attributes.push(attr.clone());
                            }
                        }
                        Attribute::Item { name, .. } if name == "returns" => {
                            if !existing.meta.attributes.iter().any(|a| matches!(a, Attribute::Item { name: n, .. } if n == "returns")) {
                                existing.meta.attributes.push(attr.clone());
                            }
                        }
                        _ => {}
                    }
                }
                // Set the command wire name
                if !existing.meta.attributes.iter().any(|a| matches!(a, Attribute::Unit(n) if n == "command")) {
                    existing.meta.attributes.push(Attribute::new("command"));
                }
                // Remove old command entry that was created from inline data in earlier versions
                let old_cmd_name = processed.name.clone();
                structs_lookup.remove(&old_cmd_name);
                continue;
            }
        }

        structs_lookup.insert(processed.name.clone(), processed);
    }

    Ok((enums_lookup, structs_lookup))
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

/// Extract, parse, and process IR for a single version.
/// This is the work done per-thread in parallel.
fn process_single_version(qemu_repo: &Path, tag: &str) -> Result<VersionResult> {
    let version = parse_version_tag(tag)
        .with_context(|| format!("Invalid version tag: {}", tag))?;

    let tmpdir = extract_schema_from_tag(qemu_repo, tag)
        .with_context(|| format!("Failed to extract schema for {}", tag))?;

    let schema_entry = find_extracted_schema_entry(tmpdir.path())
        .with_context(|| format!("No schema entry point found for {}", tag))?;

    // Parser version is thread-local, safe to set per-thread
    crate::set_parser_version(version);

    let sources = crate::read_schema_with_version(&schema_entry, Some(version))
        .with_context(|| format!("Failed to read schema for {}", tag))?;

    // Hash the raw schema content for deduplication (use filename only, not temp dir path)
    let schema_hash = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for (path, content) in &sources {
            path.file_name().hash(&mut hasher);
            content.hash(&mut hasher);
        }
        hasher.finish()
    };

    let (enums, structs) = process_version_schema(&sources)
        .with_context(|| format!("Failed to process IR for {}", tag))?;

    Ok(VersionResult {
        tag: tag.to_string(),
        version,
        schema_hash,
        enums,
        structs,
    })
}

/// Parse schemas from multiple versions and build a unified registry.
/// Extraction and parsing run in parallel across versions; merging is sequential.
pub fn build_unified_registry(
    qemu_repo: &Path,
    version_tags: &[&str],
) -> Result<UnifiedRegistry> {
    use std::time::Instant;
    let start = Instant::now();

    // Phase 1: Extract + parse all versions in parallel
    eprintln!("Extracting and parsing {} versions in parallel...", version_tags.len());
    let results: Vec<Result<VersionResult>> = std::thread::scope(|scope| {
        let handles: Vec<_> = version_tags.iter().map(|tag| {
            scope.spawn(move || process_single_version(qemu_repo, tag))
        }).collect();

        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    eprintln!("  Extraction + parsing: {:.1}s", start.elapsed().as_secs_f64());

    // Collect results, maintaining the original tag order
    let mut version_results: Vec<VersionResult> = Vec::with_capacity(version_tags.len());
    for result in results {
        version_results.push(result?);
    }

    // Sort by version to ensure deterministic merge order
    version_results.sort_by_key(|r| r.version);

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

        let mut seen_types = BTreeSet::new();

        for (name, e) in &vr.enums {
            seen_types.insert(name.clone());
            merge_enum(&mut registry, name, e, vr.version);
        }

        for (name, s) in &vr.structs {
            seen_types.insert(name.clone());
            merge_struct(&mut registry, name, s, vr.version);
        }

        for (name, ut) in registry.types.iter_mut() {
            if !seen_types.contains(name) {
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

    // Deduplicate types that have the same Rust name (e.g., command 'drive-mirror'
    // and struct 'DriveMirror' both rustify to DriveMirror).
    // Keep the one with more fields (the data struct), discard the command wrapper.
    let mut rust_to_qapi: HashMap<String, Vec<String>> = HashMap::new();
    for name in registry.types.keys() {
        let rust_name = rustify_name(name);
        rust_to_qapi.entry(rust_name).or_default().push(name.clone());
    }
    for (rust_name, qapi_names) in &rust_to_qapi {
        if qapi_names.len() > 1 {
            // Find the best one to keep (most fields, or the non-command one)
            let best = qapi_names.iter().max_by_key(|name| {
                match registry.types.get(name.as_str()) {
                    Some(UnifiedType::Struct(s)) => {
                        let is_cmd_only = s.ir.meta.attributes.iter().any(|a| matches!(a, Attribute::Unit(n) if n == "command"))
                            && s.ir.fields.is_empty();
                        if is_cmd_only { 0 } else { s.ir.fields.len() + 100 }
                    }
                    Some(UnifiedType::Enum(e)) => e.ir.variants.len() + 100,
                    None => 0,
                }
            }).unwrap().clone();

            // Merge command attributes from duplicates into the best one
            for name in qapi_names {
                if *name == best { continue; }
                if let Some(dup) = registry.types.remove(name) {
                    if let (UnifiedType::Struct(dup_s), Some(UnifiedType::Struct(best_s))) = (dup, registry.types.get_mut(&best)) {
                        for attr in &dup_s.ir.meta.attributes {
                            match attr {
                                Attribute::Unit(n) if n == "command" || n == "allow_oob" || n == "allow_preconfig" || n == "no_success_response" => {
                                    if !best_s.ir.meta.attributes.contains(attr) {
                                        best_s.ir.meta.attributes.push(attr.clone());
                                    }
                                }
                                Attribute::Item { name: n, .. } if n == "returns" => {
                                    if !best_s.ir.meta.attributes.iter().any(|a| matches!(a, Attribute::Item { name: n2, .. } if n2 == "returns")) {
                                        best_s.ir.meta.attributes.push(attr.clone());
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    eprintln!("INFO: Merged duplicate '{}' into '{}' (Rust name: {})", name, best, rust_name);
                }
            }
        }
    }

    if skipped > 0 {
        eprintln!("  Skipped {} versions with identical schemas", skipped);
    }
    eprintln!("  Merge loop: {:.1}s", merge_start.elapsed().as_secs_f64());
    let fix_start = Instant::now();
    fix_recursive_types(&mut registry);
    eprintln!("  fix_recursive_types: {:.1}s", fix_start.elapsed().as_secs_f64());

    let total = registry.types.len();
    let removed = registry.types.values().filter(|t| match t {
        UnifiedType::Enum(e) => e.versions.until.is_some(),
        UnifiedType::Struct(s) => s.versions.until.is_some(),
    }).count();
    eprintln!("  Merge: {:.1}s", merge_start.elapsed().as_secs_f64());
    eprintln!("  Total: {:.1}s — {} types, {} removed", start.elapsed().as_secs_f64(), total, removed);

    Ok(registry)
}

/// Check if an IR node has the 'deprecated' feature.
fn has_deprecated_feature(meta: &Metadata) -> bool {
    meta.attributes.iter().any(|a| match a {
        Attribute::List(items) => items.iter().any(|item| matches!(item,
            Attribute::Item { name, value } if name == "feature" && value == "deprecated"
        )),
        _ => false,
    })
}

/// Map from QAPI variant name to its Rust name (for detecting renames).
fn variant_rust_name(v: &EnumVariant) -> String {
    crate::qapi_ir::rustify_name(&v.name)
}

fn merge_enum(registry: &mut UnifiedRegistry, name: &str, e: &Enum, version: Version) {
    if let Some(UnifiedType::Enum(existing)) = registry.types.get_mut(name) {
        existing.versions.until = None;
        // Track type-level deprecation
        if has_deprecated_feature(&e.meta) && existing.versions.deprecated.is_none() {
            existing.versions.deprecated = Some(version);
        }
        // Update doc from latest version (latest has best docs)
        if e.meta.doc.is_some() {
            existing.ir.meta.doc = e.meta.doc.as_deref().and_then(clean_doc);
        }

        let mut seen_variants = BTreeSet::new();
        for variant in &e.variants {
            seen_variants.insert(variant.name.clone());
            if existing.variant_versions.contains_key(&variant.name) {
                // Variant still exists — clear until, update doc from latest version
                let vr = existing.variant_versions.get_mut(&variant.name).unwrap();
                vr.until = None;
                // Track variant deprecation
                if has_deprecated_feature(&variant.meta) && vr.deprecated.is_none() {
                    vr.deprecated = Some(version);
                }
                if let (Some(ev), true) = (existing.ir.variants.iter_mut().find(|v| v.name == variant.name), variant.meta.doc.is_some()) {
                    ev.meta.doc = variant.meta.doc.as_deref().and_then(clean_doc);
                }
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
                    let mut v = variant.clone();
                    if let Some(doc) = &v.meta.doc {
                        v.meta.doc = clean_doc(doc);
                    }
                    existing.ir.variants[old_idx] = v;
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
                        let mut v = variant.clone();
                        if let Some(doc) = &v.meta.doc {
                            v.meta.doc = clean_doc(doc);
                        }
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
        let variant_versions: BTreeMap<String, VersionRange> = e.variants.iter()
            .map(|v| (v.name.clone(), VersionRange::new(version)))
            .collect();
        let mut ir = e.clone();
        if let Some(doc) = &ir.meta.doc {
            ir.meta.doc = clean_doc(doc);
        }
        for v in &mut ir.variants {
            if let Some(doc) = &v.meta.doc {
                v.meta.doc = clean_doc(doc);
            }
        }
        registry.types.insert(name.to_string(), UnifiedType::Enum(UnifiedEnum {
            ir,
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
        // Track type-level deprecation
        if has_deprecated_feature(&s.meta) && existing.versions.deprecated.is_none() {
            existing.versions.deprecated = Some(version);
        }
        // Update doc from latest version (latest has best docs)
        if s.meta.doc.is_some() {
            existing.ir.meta.doc = s.meta.doc.as_deref().and_then(clean_doc);
        }

        let mut seen_fields = BTreeSet::new();
        for field in &s.fields {
            seen_fields.insert(field.name.clone());
            if existing.field_versions.contains_key(&field.name) {
                // Field still exists — clear until, update doc from latest version
                let fr = existing.field_versions.get_mut(&field.name).unwrap();
                fr.until = None;
                // Track field deprecation
                if has_deprecated_feature(&field.meta) && fr.deprecated.is_none() {
                    fr.deprecated = Some(version);
                }
                if let (Some(ef), true) = (existing.ir.fields.iter_mut().find(|f| f.name == field.name), field.meta.doc.is_some()) {
                    ef.meta.doc = field.meta.doc.as_deref().and_then(clean_doc);
                }
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
        if let Some(doc) = &ir.meta.doc {
            ir.meta.doc = clean_doc(doc);
        }
        let mut field_versions = BTreeMap::new();
        let mut seen_rust_names: BTreeSet<String> = BTreeSet::new();
        for field in &mut ir.fields {
            if let Some(doc) = &field.meta.doc {
                field.meta.doc = clean_doc(doc);
            }
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
    use std::time::Instant;
    let registry = build_unified_registry(qemu_repo, version_tags)?;
    let codegen_start = Instant::now();

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
    // Sort all types by Rust name (case-insensitive) for deterministic, readable output.
    let mut all_entries: Vec<_> = registry.types.iter().collect();
    all_entries.sort_by(|(_, a), (_, b)| {
        let name_a = match a {
            UnifiedType::Enum(ue) => rustify_name(&ue.ir.name),
            UnifiedType::Struct(us) => rustify_name(&us.ir.name),
        };
        let name_b = match b {
            UnifiedType::Enum(ue) => rustify_name(&ue.ir.name),
            UnifiedType::Struct(us) => rustify_name(&us.ir.name),
        };
        name_a.to_lowercase().cmp(&name_b.to_lowercase())
    });

    // Collect all generated token streams, then format once
    let mut all_tokens = proc_macro2::TokenStream::new();

    for (name, ut) in &all_entries {
        let rust_name = match ut {
            UnifiedType::Enum(ue) => rustify_name(&ue.ir.name),
            UnifiedType::Struct(us) => rustify_name(&us.ir.name),
        };

        if !seen_rust_names.insert(rust_name.clone()) {
            eprintln!("WARNING: Skipping duplicate Rust name '{}' (QAPI name: '{}')", rust_name, name);
            continue;
        }

        let code = match ut {
            UnifiedType::Enum(ue) => generate_unified_enum(ue, &registry),
            UnifiedType::Struct(us) => generate_unified_struct(us, &registry),
        };
        all_tokens.extend(code);
    }

    eprintln!("  Token generation: {:.1}s", codegen_start.elapsed().as_secs_f64());

    // Parse and format the entire output at once
    let fmt_start = Instant::now();
    let combined = format!("{}{}", output, all_tokens);
    let result = match syn::parse_file(&combined) {
        Ok(syntree) => {
            eprintln!("  syn::parse_file: {:.1}s", fmt_start.elapsed().as_secs_f64());
            let pp_start = Instant::now();
            let formatted = prettyplease::unparse(&syntree);
            eprintln!("  prettyplease: {:.1}s", pp_start.elapsed().as_secs_f64());
            Ok(formatted)
        }
        Err(e) => {
            eprintln!("WARNING: Failed to parse combined output: {}", e);
            Ok(combined)
        }
    };
    eprintln!("  Codegen total: {:.1}s", codegen_start.elapsed().as_secs_f64());
    result
}

/// Generate code for a unified enum, adding since/until attributes from the registry.
fn generate_unified_enum(ue: &UnifiedEnum, _registry: &UnifiedRegistry) -> proc_macro2::TokenStream {

    let mut ir = ue.ir.clone();

    // Strip any existing since from doc comments — always use registry version
    ir.meta.attributes.retain(|a| !matches!(a, Attribute::Item { name, .. } if name == "added" || name == "since" || name == "removed" || name == "until" || name == "deprecated"));
    ir.meta.attributes.push(Attribute::with_value("added", version_str(ue.versions.since)));
    if let Some(dep) = ue.versions.deprecated {
        ir.meta.attributes.push(Attribute::with_value("deprecated", version_str(dep)));
    }
    if let Some(until) = ue.versions.until {
        ir.meta.attributes.push(Attribute::with_value("removed", version_str(until)));
    }

    // Add per-variant added/deprecated/removed
    for variant in &mut ir.variants {
        variant.meta.attributes.retain(|a| !matches!(a, Attribute::Item { name, .. } if name == "added" || name == "since" || name == "removed" || name == "until" || name == "deprecated"));
        if let Some(vr) = ue.variant_versions.get(&variant.name) {
            if vr.since > ue.versions.since {
                variant.meta.attributes.push(Attribute::with_value("added", version_str(vr.since)));
            }
            if let Some(dep) = vr.deprecated {
                variant.meta.attributes.push(Attribute::with_value("deprecated", version_str(dep)));
            }
            if let Some(until) = vr.until {
                variant.meta.attributes.push(Attribute::with_value("removed", version_str(until)));
            }
        }
    }

    // Clean docs
    if let Some(doc) = &ir.meta.doc {
        ir.meta.doc = clean_doc(doc);
    }
    for variant in &mut ir.variants {
        if let Some(doc) = &variant.meta.doc {
            variant.meta.doc = clean_doc(doc);
        }
    }

    // Sort variants alphabetically by name
    ir.variants.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    ir.generate()
}

/// Generate code for a unified struct, adding since/until and making removed/added fields optional.
fn generate_unified_struct(us: &UnifiedStruct, _registry: &UnifiedRegistry) -> proc_macro2::TokenStream {

    let mut ir = us.ir.clone();

    // Strip existing since from doc comments — always use registry version
    ir.meta.attributes.retain(|a| !matches!(a, Attribute::Item { name, .. } if name == "added" || name == "since" || name == "removed" || name == "until" || name == "deprecated"));
    ir.meta.attributes.push(Attribute::with_value("added", version_str(us.versions.since)));
    if let Some(dep) = us.versions.deprecated {
        ir.meta.attributes.push(Attribute::with_value("deprecated", version_str(dep)));
    }
    if let Some(until) = us.versions.until {
        ir.meta.attributes.push(Attribute::with_value("removed", version_str(until)));
    }

    // Add per-field added/deprecated/removed and make fields optional if needed
    for field in &mut ir.fields {
        field.meta.attributes.retain(|a| !matches!(a, Attribute::Item { name, .. } if name == "added" || name == "since" || name == "removed" || name == "until" || name == "deprecated"));
        if let Some(fr) = us.field_versions.get(&field.name) {
            if fr.since > us.versions.since {
                field.meta.attributes.push(Attribute::with_value("added", version_str(fr.since)));
                if !field.optional {
                    field.optional = true;
                }
            }
            if let Some(dep) = fr.deprecated {
                field.meta.attributes.push(Attribute::with_value("deprecated", version_str(dep)));
            }
            if let Some(until) = fr.until {
                field.meta.attributes.push(Attribute::with_value("removed", version_str(until)));
                if !field.optional {
                    field.optional = true;
                }
            }
        }
    }

    // Clean docs
    if let Some(doc) = &ir.meta.doc {
        ir.meta.doc = clean_doc(doc);
    }
    for field in &mut ir.fields {
        if let Some(doc) = &field.meta.doc {
            field.meta.doc = clean_doc(doc);
        }
    }

    // Sort fields alphabetically by name
    ir.fields.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    ir.generate()
}

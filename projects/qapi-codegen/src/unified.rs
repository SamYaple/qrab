//! Multi-version QAPI schema merge.
//!
//! Parses schemas from multiple QEMU version tags, merges them into a single
//! unified type registry where each type, field, and variant has version range
//! annotations (since/until).

use crate::qapi_ir::*;
use crate::versions::{
    extract_schema_from_tag, find_extracted_qga_schema_entry, find_extracted_schema_entry,
    parse_version_tag,
};
use crate::{parse_schema, QapiSchemaToken};
use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

pub type Version = (u32, u32, u32);

fn version_str(v: Version) -> String {
    format!("{}.{}.{}", v.0, v.1, v.2)
}

/// Normalize "Since X.Y" patterns in doc strings to "Since X.Y.0"
/// Strip "since" version information from doc strings.
/// The since/until metadata is already captured in #[qapi(since/until)] attributes,
/// so it should not appear in the generated Rust doc comments.
/// Clean a doc string: strip leading `# ` from each line (QAPI comment syntax)
/// but preserve whitespace structure and all content.
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
    let doc = doc.trim();
    if doc.is_empty() {
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
}

/// A struct with version-annotated fields.
#[derive(Debug, Clone)]
pub struct UnifiedStruct {
    pub ir: Struct,
    pub versions: VersionRange,
    /// Per-field version ranges (keyed by field name)
    pub field_versions: BTreeMap<String, VersionRange>,
    /// Per-attribute version ranges for command/event flags that can appear/disappear
    /// across versions (e.g. allow_preconfig, allow_oob, no_success_response, unstable).
    pub attr_versions: BTreeMap<String, VersionRange>,
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

/// Merge command-specific attributes (command, returns, allow_oob, etc.) from
/// a processed command struct onto an existing data struct.
fn merge_command_attrs_onto(source: &Struct, target: &mut Struct) {
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
fn process_version_schema(
    sources: &[(std::path::PathBuf, String)],
) -> Result<(HashMap<String, Enum>, HashMap<String, Struct>)> {
    let mut enums_lookup = HashMap::new();
    let mut structs_lookup = HashMap::new();
    let mut unprocessed_structs = Vec::new();
    let mut unprocessed_unions = Vec::new();
    let mut unprocessed_commands = Vec::new();
    let mut unprocessed_events = Vec::new();

    // All types are keyed by their rustified form so that cross-version renames
    // (e.g. SGXInfo → SgxInfo in v10.1) and command/data-struct pairs that
    // happen to rustify to the same identifier land in a single entry.
    for (_path, content) in sources {
        let schema = parse_schema(content)?;
        for token in schema.0 {
            match token {
                QapiSchemaToken::Enum(v) => {
                    let processed = process_enum(v);
                    enums_lookup.insert(rustify_name(&processed.name), processed);
                }
                QapiSchemaToken::Alternate(v) => {
                    let processed = process_alternate(v);
                    enums_lookup.insert(rustify_name(&processed.name), processed);
                }
                QapiSchemaToken::Struct(v) => {
                    if v.base.is_some() {
                        unprocessed_structs.push(v);
                        continue;
                    }
                    let (processed, extracted) = process_struct(v, &structs_lookup);
                    structs_lookup.insert(rustify_name(&processed.name), processed);
                    for e in extracted {
                        structs_lookup.insert(rustify_name(&e.name), e);
                    }
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

    for v in unprocessed_structs {
        let (processed, extracted) = process_struct(v, &structs_lookup);
        structs_lookup.insert(rustify_name(&processed.name), processed);
        for e in extracted {
            structs_lookup.insert(rustify_name(&e.name), e);
        }
    }
    for v in unprocessed_unions {
        let (s, e, disc_enum) = process_union(v, &structs_lookup, &enums_lookup);
        if let Some(de) = disc_enum {
            enums_lookup.insert(rustify_name(&de.name), de);
        }
        enums_lookup.insert(rustify_name(&e.name), e);
        structs_lookup.insert(rustify_name(&s.name), s);
    }
    for v in unprocessed_events {
        let processed = process_event(v, &structs_lookup);
        structs_lookup.insert(rustify_name(&processed.name), processed);
    }
    for v in unprocessed_commands {
        // Check if this command references an existing struct via data
        let data_ref_name = match &v.data {
            Some(crate::MembersOrRef::Ref(name)) => Some(name.to_string()),
            _ => None,
        };

        let processed = process_command(v, &structs_lookup);
        let cmd_key = rustify_name(&processed.name);

        if let Some(ref ref_name) = data_ref_name {
            let ref_key = rustify_name(ref_name);
            if cmd_key == ref_key {
                // Command and its data ref share the same rustified name
                // (e.g. command `blockdev-snapshot-sync` → struct
                // `BlockdevSnapshotSync`). Merge command attributes onto the
                // existing data struct -- a single entry represents both.
                if let Some(existing) = structs_lookup.get_mut(&ref_key) {
                    merge_command_attrs_onto(&processed, existing);
                    continue;
                }
            }
            // Different rustified names (e.g. `netdev_add` → `NetdevAdd`,
            // data ref `Netdev` → `Netdev`): the command gets its own entry
            // under its rustified key. The data struct stays under its key.
        }

        structs_lookup.insert(cmd_key, processed);
    }

    Ok((enums_lookup, structs_lookup))
}

/// Result of extracting and parsing a single version's schema.
struct VersionResult {
    _tag: String,
    version: Version,
    /// Hash of the raw schema content (for dedup of identical patch releases).
    schema_hash: u64,
    enums: HashMap<String, Enum>,
    structs: HashMap<String, Struct>,
}

/// Extract, parse, and process IR for a single version.
/// This is the work done per-thread in parallel.
fn process_single_version(qemu_repo: &Path, tag: &str) -> Result<VersionResult> {
    let version =
        parse_version_tag(tag).with_context(|| format!("Invalid version tag: {}", tag))?;

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
        _tag: tag.to_string(),
        version,
        schema_hash,
        enums,
        structs,
    })
}

/// Extract, parse, and process IR for a single version's QGA schema.
fn process_single_version_qga(qemu_repo: &Path, tag: &str) -> Result<Option<VersionResult>> {
    let version =
        parse_version_tag(tag).with_context(|| format!("Invalid version tag: {}", tag))?;

    let tmpdir = extract_schema_from_tag(qemu_repo, tag)
        .with_context(|| format!("Failed to extract schema for {}", tag))?;

    let schema_entry = match find_extracted_qga_schema_entry(tmpdir.path()) {
        Some(entry) => entry,
        None => return Ok(None), // No QGA schema in this version
    };

    crate::set_parser_version(version);

    let sources = crate::read_schema_with_version(&schema_entry, Some(version))
        .with_context(|| format!("Failed to read QGA schema for {}", tag))?;

    let schema_hash = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        hasher.write(b"qga"); // Differentiate from QMP hash
        for (path, content) in &sources {
            path.file_name().hash(&mut hasher);
            content.hash(&mut hasher);
        }
        hasher.finish()
    };

    let (enums, structs) = process_version_schema(&sources)
        .with_context(|| format!("Failed to process QGA IR for {}", tag))?;

    Ok(Some(VersionResult {
        _tag: tag.to_string(),
        version,
        schema_hash,
        enums,
        structs,
    }))
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
    let results: Vec<Result<VersionResult>> = std::thread::scope(|scope| {
        let handles: Vec<_> = version_tags
            .iter()
            .map(|tag| scope.spawn(move || process_single_version(qemu_repo, tag)))
            .collect();

        // Propagate worker panics as errors instead of silently unwrapping,
        // which would abort the process and obscure which tag failed.
        handles
            .into_iter()
            .map(|h| {
                h.join().unwrap_or_else(|e| {
                    Err(anyhow::anyhow!("worker thread panicked: {:?}", e))
                })
            })
            .collect()
    });

    eprintln!(
        "  Extraction + parsing: {:.1}s",
        start.elapsed().as_secs_f64()
    );

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

/// The set of boolean Unit attributes on commands/events that must be version-tracked.
const TRACKED_BOOL_ATTRS: &[&str] = &["allow_preconfig", "allow_oob", "no_success_response"];

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
    crate::qapi_ir::rustify_name(&v.name)
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
                    // It's a rename: update the old variant's wire name to the new one
                    let old_name = existing.ir.variants[old_idx].name.clone();
                    let old_vr = existing.variant_versions.remove(&old_name).unwrap();
                    // Replace old variant IR with new one, keeping the old version range
                    let mut v = variant.clone();
                    clean_doc_in_place(&mut v.meta.doc);
                    existing.ir.variants[old_idx] = v;
                    existing
                        .variant_versions
                        .insert(variant.name.clone(), old_vr);
                    // Also mark the old QAPI name as seen so it doesn't get marked as removed
                    seen_variants.insert(old_name);
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
            }),
        );
    }
}

fn field_rust_name(f: &StructField) -> String {
    crate::qapi_ir::rustify_field(&f.name)
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
        // Merge version-tracked boolean command attributes (allow_preconfig, allow_oob, etc.)
        merge_bool_attrs(existing, &s.meta, version);
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
                if let Some(existing_field) = existing
                    .ir
                    .fields
                    .iter_mut()
                    .find(|f| f.name == field.name)
                {
                    existing_field.r#type = field.r#type.clone();
                    existing_field.optional = field.optional;
                    existing_field.array = field.array;
                    merge_doc_from(&mut existing_field.meta.doc, field.meta.doc.as_deref());
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
                    let old_name = existing.ir.fields[old_idx].name.clone();
                    let old_vr = existing.field_versions.remove(&old_name).unwrap();
                    existing.ir.fields[old_idx] = field.clone();
                    existing.field_versions.insert(field.name.clone(), old_vr);
                    seen_fields.insert(old_name);
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
                attr_versions,
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
        // Build a dependency graph borrowing type names from registry.
        let mut deps: HashMap<&str, BTreeSet<&str>> = HashMap::new();
        for (name, ut) in &registry.types {
            let mut referenced = BTreeSet::new();
            match ut {
                UnifiedType::Struct(us) => {
                    for field in &us.ir.fields {
                        referenced.insert(field.r#type.as_str());
                    }
                }
                UnifiedType::Enum(ue) => {
                    for variant in &ue.ir.variants {
                        if let EnumVariantKind::Tuple(t) = &variant.kind {
                            referenced.insert(t.as_str());
                        }
                    }
                }
            }
            deps.insert(name.as_str(), referenced);
        }

        // Find types that are part of a cycle
        let mut result: BTreeSet<(String, String)> = BTreeSet::new();
        for (name, ut) in &registry.types {
            if let UnifiedType::Struct(us) = ut {
                for field in &us.ir.fields {
                    if is_reachable(&field.r#type, name, &deps, &mut BTreeSet::new()) {
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
    deps: &HashMap<&'a str, BTreeSet<&'a str>>,
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

/// Generate unified code from the registry.
pub fn generate_unified_code(qemu_repo: &Path, version_tags: &[&str]) -> Result<GeneratedOutput> {
    use std::time::Instant;
    let registry = build_unified_registry(qemu_repo, version_tags)?;
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
    if !referenced_by_data.is_empty() {
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

        if seen.contains(&rust_name) {
            eprintln!(
                "WARNING: Skipping duplicate Rust name '{}' (QAPI name: '{}', category: {:?})",
                rust_name, name, category
            );
            continue;
        }
        seen.insert(rust_name);

        let code = match ut {
            UnifiedType::Enum(ue) => generate_unified_enum(ue, &registry),
            UnifiedType::Struct(us) => generate_unified_struct(us, &registry),
        };
        tokens.extend(code);
    }

    eprintln!(
        "  Token generation: {:.1}s",
        codegen_start.elapsed().as_secs_f64()
    );

    // Format each part as a separate file
    let fmt_start = Instant::now();

    // mod.rs: data types + module declarations
    let mod_tokens = quote::quote! {
        #types_tokens

        pub mod commands;
        pub mod events;
    };
    let types_file = format_rust_file(mod_tokens, "");

    // commands.rs: command types with super::* import
    let commands_file = format_rust_file(commands_tokens, "use super::*;\n");

    // events.rs: event types with super::* import
    let events_file = format_rust_file(events_tokens, "use super::*;\n");

    eprintln!("  Formatting: {:.1}s", fmt_start.elapsed().as_secs_f64());
    eprintln!(
        "  Codegen total: {:.1}s",
        codegen_start.elapsed().as_secs_f64()
    );
    Ok(GeneratedOutput {
        types: types_file,
        commands: commands_file,
        events: events_file,
    })
}

/// The three files produced by code generation.
pub struct GeneratedOutput {
    /// Data types (structs, enums, unions) — goes into `mod.rs`.
    pub types: String,
    /// Command types — goes into `commands.rs`.
    pub commands: String,
    /// Event types — goes into `events.rs`.
    pub events: String,
}

impl GeneratedOutput {
    /// Concatenate all three parts into a single string.
    /// Useful for tests that search across all generated code.
    /// Strips inner attributes (`#![...]`) and duplicate headers from
    /// commands/events so the result is valid as a single Rust file.
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
fn format_rust_file(tokens: proc_macro2::TokenStream, extra_imports: &str) -> String {
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
    match syn::parse_file(&combined) {
        Ok(syntree) => separate_items(&prettyplease::unparse(&syntree)),
        Err(e) => {
            eprintln!("WARNING: Failed to parse output: {}", e);
            combined
        }
    }
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
        ir.meta.attributes.push(Attribute::nested("unstable", inner));
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

    // Add per-field added/deprecated/removed and make fields optional if needed
    for field in &mut ir.fields {
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
    let results: Vec<Result<Option<VersionResult>>> = std::thread::scope(|scope| {
        let handles: Vec<_> = version_tags
            .iter()
            .map(|tag| scope.spawn(move || process_single_version_qga(qemu_repo, tag)))
            .collect();
        // Propagate worker panics as errors instead of silently unwrapping,
        // which would abort the process and obscure which tag failed.
        handles
            .into_iter()
            .map(|h| {
                h.join().unwrap_or_else(|e| {
                    Err(anyhow::anyhow!("worker thread panicked: {:?}", e))
                })
            })
            .collect()
    });

    eprintln!(
        "  QGA extraction + parsing: {:.1}s",
        start.elapsed().as_secs_f64()
    );

    let mut version_results: Vec<VersionResult> = Vec::new();
    let mut parse_failures = 0;
    for result in results {
        match result {
            Ok(Some(vr)) => version_results.push(vr),
            Ok(None) => {} // No QGA schema in this version
            Err(e) => {
                parse_failures += 1;
                if parse_failures <= 3 {
                    eprintln!("  QGA parse error (skipping): {:#}", e);
                }
            }
        }
    }
    if parse_failures > 3 {
        eprintln!("  ... and {} more QGA parse errors", parse_failures - 3);
    }
    version_results.sort_by_key(|r| r.version);
    eprintln!(
        "  QGA: {} versions parsed, {} skipped (no schema or parse error)",
        version_results.len(),
        parse_failures
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
    use std::time::Instant;
    let registry = build_qga_registry(qemu_repo, version_tags)?;

    let seen_rust_names: BTreeSet<String> = ["String", "Option", "Vec", "Box", "Result"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let mut all_entries: Vec<_> = registry.types.iter().collect();
    all_entries.sort_by(|(_, a), (_, b)| {
        unified_type_rust_name(a)
            .to_lowercase()
            .cmp(&unified_type_rust_name(b).to_lowercase())
    });

    // Pass 1: Find command/event types referenced by data types
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

    // Pass 2: Emit into partitioned token streams
    let mut types_tokens = proc_macro2::TokenStream::new();
    let mut commands_tokens = proc_macro2::TokenStream::new();
    let mut events_tokens = proc_macro2::TokenStream::new();

    let mut seen_type_names = seen_rust_names.clone();
    let mut seen_command_names = seen_rust_names.clone();
    let mut seen_event_names = seen_rust_names;

    for (_name, ut) in &all_entries {
        let rust_name = match ut {
            UnifiedType::Enum(ue) => rustify_name(&ue.ir.name),
            UnifiedType::Struct(us) => rustify_name(&us.ir.name),
        };

        let mut category = classify_type(ut);
        if referenced_by_data.contains(&rust_name) {
            category = TypeCategory::DataType;
        }
        let (tokens, seen) = match category {
            TypeCategory::DataType => (&mut types_tokens, &mut seen_type_names),
            TypeCategory::Command => (&mut commands_tokens, &mut seen_command_names),
            TypeCategory::Event => (&mut events_tokens, &mut seen_event_names),
        };

        if seen.contains(&rust_name) {
            eprintln!("WARNING: QGA skipping duplicate '{}'", rust_name);
            continue;
        }
        seen.insert(rust_name);
        let code = match ut {
            UnifiedType::Enum(ue) => generate_unified_enum(ue, &registry),
            UnifiedType::Struct(us) => generate_unified_struct(us, &registry),
        };
        tokens.extend(code);
    }

    // Format each part as a separate file
    let fmt_start = Instant::now();

    let mod_tokens = quote::quote! {
        #types_tokens

        pub mod commands;
        pub mod events;
    };
    let types_file = format_rust_file(mod_tokens, "");
    let commands_file = format_rust_file(commands_tokens, "use super::*;\n");
    let events_file = format_rust_file(events_tokens, "use super::*;\n");

    eprintln!("  QGA codegen: {:.1}s", fmt_start.elapsed().as_secs_f64());
    Ok(GeneratedOutput {
        types: types_file,
        commands: commands_file,
        events: events_file,
    })
}

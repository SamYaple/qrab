use std::path::Path;

use qapi_codegen::{parse_schema, read_schema_with_version, QapiSchemaToken};
use qapi_codegen::versions::{
    discover_version_tags, extract_schema_from_tag, find_extracted_schema_entry,
    find_schema_entry, parse_version_tag,
};
use qapi_codegen::unified::build_unified_registry;

fn qemu_root() -> &'static Path {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    Box::leak(workspace_root.join("vendor/qemu").into_boxed_path())
}

/// Representative version tags covering all parser eras.
/// Used for integration tests to keep runtime reasonable.
fn representative_tags() -> Vec<String> {
    vec![
        "v1.0", "v1.1.0", "v1.7.0",
        "v2.0.0", "v2.1.0", "v2.5.0",
        "v2.8.0", "v2.12.0",
        "v3.0.0", "v5.0.0",
        "v7.0.0", "v9.0.0", "v10.2.0",
    ].into_iter().map(String::from).collect()
}

// ── Basic schema parsing tests (using latest vendored QEMU) ──

#[test]
fn test_find_schema_entry() {
    let root = qemu_root();
    let entry = find_schema_entry(root).expect("schema entry point not found");
    assert!(entry.exists());
    assert!(entry.ends_with("qapi/qapi-schema.json"));
}

#[test]
fn test_parse_all_schema_files() {
    let root = qemu_root();
    let entry = find_schema_entry(root).unwrap();
    let sources = read_schema_with_version(&entry, None).expect("read_schema failed");
    assert!(sources.len() > 10, "expected >10 schema files, got {}", sources.len());

    for (path, content) in &sources {
        parse_schema(content)
            .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()));
    }
}

#[test]
fn test_parse_schema_returns_types() {
    let root = qemu_root();
    let entry = find_schema_entry(root).unwrap();
    let sources = read_schema_with_version(&entry, None).unwrap();

    let total_tokens: usize = sources.iter().map(|(_, content)| {
        parse_schema(content).unwrap().0.len()
    }).sum();

    assert!(total_tokens > 100, "expected >100 schema tokens total, got {total_tokens}");
}

// ── Version discovery ──

#[test]
fn test_discover_version_tags() {
    let repo = qemu_root();
    let tags = discover_version_tags(repo).expect("discover_version_tags failed");

    // Should find many versions (v1.0 through v10.2.0 including patches)
    assert!(tags.len() > 100, "expected >100 version tags, got {}", tags.len());

    // First tag should be v1.0
    assert_eq!(tags[0], "v1.0", "First tag should be v1.0, got {}", tags[0]);

    // Should include patch versions
    let has_patches = tags.iter().any(|t| {
        if let Some((_, _, patch)) = parse_version_tag(t) {
            patch > 0
        } else {
            false
        }
    });
    assert!(has_patches, "Should include patch versions like v1.0.1");

    // Should be sorted by version
    for w in tags.windows(2) {
        let a = parse_version_tag(&w[0]).unwrap();
        let b = parse_version_tag(&w[1]).unwrap();
        assert!(a <= b, "Tags should be sorted: {} vs {}", w[0], w[1]);
    }

    // Should not include pre-v1.0
    for tag in &tags {
        let (major, _, _) = parse_version_tag(tag).unwrap();
        assert!(major >= 1, "Should not include pre-v1.0 tags, got {}", tag);
    }

    eprintln!("  Discovered {} tags ({} to {})", tags.len(), tags[0], tags.last().unwrap());
}

// ── Multi-version parser compatibility ──
#[test]
fn test_parse_all_discovered_versions() {
    let repo = qemu_root();
    // Representative versions covering all parser eras
    let tags: Vec<String> = vec![
        "v1.0", "v1.1.0", "v1.4.0", "v1.7.0",     // era A: type keyword, simple unions
        "v2.0.0", "v2.1.0", "v2.3.0", "v2.5.0",    // era A/B: includes, alternates
        "v2.7.0", "v2.8.0", "v2.10.0", "v2.12.0",  // era C/D: struct keyword, pragmas
        "v3.0.0", "v4.0.0", "v5.0.0",               // era D: conditions, features
        "v6.0.0", "v7.0.0", "v8.0.0",               // era E: modern format
        "v9.0.0", "v10.2.0",                         // latest
    ].into_iter().map(String::from).collect();
    let mut failures = Vec::new();

    for tag in &tags {
        eprint!("  Parsing {}... ", tag);
        match try_parse_version(tag, repo) {
            Ok(count) => eprintln!("OK ({} tokens)", count),
            Err(e) => {
                eprintln!("FAIL: {}", e);
                failures.push(format!("{}: {}", tag, e));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "Failed to parse {} version(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn try_parse_version(tag: &str, repo: &Path) -> Result<usize, String> {
    let version = parse_version_tag(tag).ok_or_else(|| format!("bad tag: {}", tag))?;
    let tmpdir = extract_schema_from_tag(repo, tag)
        .map_err(|e| format!("extract failed: {}", e))?;

    let entry = find_extracted_schema_entry(tmpdir.path())
        .ok_or_else(|| "no schema entry point found".to_string())?;

    let sources = read_schema_with_version(&entry, Some(version))
        .map_err(|e| format!("read_schema failed: {:#}", e))?;

    let mut total_tokens = 0;
    for (path, content) in &sources {
        let schema = parse_schema(content)
            .map_err(|e| format!("parse failed in {}: {}", path.display(), e))?;
        total_tokens += schema.0.len();
    }

    Ok(total_tokens)
}

// ── Unified codegen ──

#[test]
fn test_build_unified_registry() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs)
        .expect("build_unified_registry failed");

    assert!(registry.types.len() > 100, "expected >100 unified types, got {}", registry.types.len());
    assert_eq!(registry.versions.len(), tags.len());

    let removed_count = registry.types.values().filter(|t| match t {
        qapi_codegen::unified::UnifiedType::Enum(e) => e.versions.until.is_some(),
        qapi_codegen::unified::UnifiedType::Struct(s) => s.versions.until.is_some(),
    }).count();

    eprintln!("  Unified registry: {} types, {} removed, {} versions",
        registry.types.len(), removed_count, registry.versions.len());
    assert!(removed_count > 0, "expected some removed types across versions");
}

#[test]
fn test_unified_codegen_produces_valid_rust() {
    let repo = qemu_root();
    let tags = discover_version_tags(repo).expect("discover_version_tags failed");
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let code = qapi_codegen::unified::generate_unified_code(repo, &tag_refs)
        .expect("generate_unified_code failed");

    assert!(code.len() > 10000, "unified code suspiciously short: {} bytes", code.len());

    let parsed: syn::File = syn::parse_str(&code)
        .expect("unified generated code is not valid Rust");

    eprintln!("  Unified code: {} items, {} bytes", parsed.items.len(), code.len());
    assert!(parsed.items.len() > 100, "expected >100 items, got {}", parsed.items.len());
}

#[test]
fn test_unified_codegen_has_doc_comments() {
    let repo = qemu_root();
    let tags = discover_version_tags(repo).expect("discover_version_tags failed");
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let code = qapi_codegen::unified::generate_unified_code(repo, &tag_refs)
        .expect("Failed to generate unified code");

    let doc_count = code.matches("/**").count() + code.matches("///").count();
    assert!(doc_count > 100, "expected >100 doc comments, got {}", doc_count);
}

#[test]
fn test_spice_types_from_v2_1() {
    let repo = qemu_root();
    let tags = discover_version_tags(repo).expect("discover_version_tags failed");
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let code = qapi_codegen::unified::generate_unified_code(repo, &tag_refs)
        .expect("Failed to generate unified code");

    assert!(code.contains("SpiceServerInfo"), "Should contain SpiceServerInfo");
    assert!(code.contains("SpiceBasicInfo"), "Should contain SpiceBasicInfo");
}

// ── Era-specific parser tests ──

#[test]
fn test_era_a_v1_1_type_keyword() {
    let repo = qemu_root();
    let version = parse_version_tag("v1.1.0").unwrap();
    let tmpdir = extract_schema_from_tag(repo, "v1.1.0").expect("extract v1.1.0");
    let entry = find_extracted_schema_entry(tmpdir.path()).expect("find entry v1.1.0");
    let sources = read_schema_with_version(&entry, Some(version)).expect("read v1.1.0");

    let total: usize = sources.iter().map(|(_, c)| {
        parse_schema(c).unwrap().iter().filter(|t| matches!(t,
            QapiSchemaToken::Struct(_) | QapiSchemaToken::Enum(_) |
            QapiSchemaToken::Command(_) | QapiSchemaToken::Union(_)
        )).count()
    }).sum();
    assert!(total > 30, "v1.1.0 should have >30 type definitions, got {}", total);
}

#[test]
fn test_era_b_v2_1_includes_and_events() {
    let repo = qemu_root();
    let version = parse_version_tag("v2.1.0").unwrap();
    let tmpdir = extract_schema_from_tag(repo, "v2.1.0").expect("extract v2.1.0");
    let entry = find_extracted_schema_entry(tmpdir.path()).expect("find entry v2.1.0");
    let sources = read_schema_with_version(&entry, Some(version)).expect("read v2.1.0");

    let has_events = sources.iter().any(|(_, c)| {
        parse_schema(c).unwrap().iter().any(|t| matches!(t, QapiSchemaToken::Event(_)))
    });
    assert!(has_events, "v2.1.0 should have events");
    assert!(sources.len() > 1, "v2.1.0 should have multiple schema files");
}

#[test]
fn test_era_c_v2_5_struct_and_alternate() {
    let repo = qemu_root();
    let version = parse_version_tag("v2.5.0").unwrap();
    let tmpdir = extract_schema_from_tag(repo, "v2.5.0").expect("extract v2.5.0");
    let entry = find_extracted_schema_entry(tmpdir.path()).expect("find entry v2.5.0");
    let sources = read_schema_with_version(&entry, Some(version)).expect("read v2.5.0");

    let has_alternates = sources.iter().any(|(_, c)| {
        parse_schema(c).unwrap().iter().any(|t| matches!(t, QapiSchemaToken::Alternate(_)))
    });
    assert!(has_alternates, "v2.5.0 should have alternates");
}

#[test]
fn test_era_d_v4_0_conditions_and_features() {
    let repo = qemu_root();
    let version = parse_version_tag("v4.0.0").unwrap();
    let tmpdir = extract_schema_from_tag(repo, "v4.0.0").expect("extract v4.0.0");
    let entry = find_extracted_schema_entry(tmpdir.path()).expect("find entry v4.0.0");
    let sources = read_schema_with_version(&entry, Some(version)).expect("read v4.0.0");

    let has_pragma = sources.iter().any(|(_, c)| {
        parse_schema(c).unwrap().iter().any(|t| matches!(t, QapiSchemaToken::Pragma(_)))
    });
    assert!(has_pragma, "v4.0.0 should have pragma");

    let has_conditions = sources.iter().any(|(_, c)| {
        parse_schema(c).unwrap().iter().any(|t| match t {
            QapiSchemaToken::Command(cmd) => cmd.r#if.is_some(),
            QapiSchemaToken::Struct(s) => s.r#if.is_some(),
            _ => false,
        })
    });
    assert!(has_conditions, "v4.0.0 should have conditional types");
}

#[test]
fn test_era_e_v10_2_modern_format() {
    let repo = qemu_root();
    let version = parse_version_tag("v10.2.0").unwrap();
    let tmpdir = extract_schema_from_tag(repo, "v10.2.0").expect("extract v10.2.0");
    let entry = find_extracted_schema_entry(tmpdir.path()).expect("find entry v10.2.0");
    let sources = read_schema_with_version(&entry, Some(version)).expect("read v10.2.0");

    let all_flat = sources.iter().all(|(_, c)| {
        parse_schema(c).unwrap().iter().all(|t| match t {
            QapiSchemaToken::Union(u) => u.discriminator.is_some(),
            _ => true,
        })
    });
    assert!(all_flat, "v10.2.0 should have no simple unions");

    let total: usize = sources.iter().map(|(_, c)| {
        parse_schema(c).unwrap().iter().filter(|t| matches!(t,
            QapiSchemaToken::Struct(_) | QapiSchemaToken::Enum(_) |
            QapiSchemaToken::Command(_) | QapiSchemaToken::Union(_) |
            QapiSchemaToken::Event(_) | QapiSchemaToken::Alternate(_)
        )).count()
    }).sum();
    assert!(total > 500, "v10.2.0 should have >500 types, got {}", total);
}

#[test]
fn test_doc_preservation_in_ir() {
    let repo = qemu_root();
    let version = parse_version_tag("v9.1.0").unwrap();
    let tmpdir = extract_schema_from_tag(repo, "v9.1.0").expect("extract v9.1.0");
    let entry = find_extracted_schema_entry(tmpdir.path()).expect("find entry v9.1.0");
    let sources = read_schema_with_version(&entry, Some(version)).expect("read v9.1.0");

    let mut found = false;
    for (_, content) in &sources {
        for token in parse_schema(content).unwrap().iter() {
            if let QapiSchemaToken::Struct(s) = token {
                if s.name == "StatusInfo" {
                    if let Some(doc) = &s.doc {
                        assert!(!doc.description.is_empty(), "StatusInfo should have docs");
                        found = true;
                    }
                }
            }
        }
    }
    assert!(found, "Should find StatusInfo with doc in v9.1.0");
}

use std::path::Path;

use qapi_spec_parser::{parse_schema, read_schema, read_schema_with_version, generate_qapi_code, generate_qapi_code_versioned};
use qapi_spec_parser::versions::{find_schema_entry, extract_schema_from_tag, find_extracted_schema_entry, parse_version_tag};
use qapi_spec_parser::unified::build_unified_registry;

fn qemu_root() -> &'static Path {
    // vendor/qemu is at the workspace root
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    // Leak to get 'static — fine for tests
    Box::leak(workspace_root.join("vendor/qemu").into_boxed_path())
}

#[test]
fn test_find_schema_entry() {
    let root = qemu_root();
    let entry = find_schema_entry(root).expect("schema entry point not found");
    assert!(entry.exists(), "schema entry file does not exist: {}", entry.display());
    assert!(entry.ends_with("qapi/qapi-schema.json"));
}

#[test]
fn test_parse_all_schema_files() {
    let root = qemu_root();
    let entry = find_schema_entry(root).unwrap();
    let sources = read_schema(&entry).expect("read_schema failed");

    // The QEMU v9+ schema includes many files
    assert!(
        sources.len() > 10,
        "expected >10 schema files, got {}",
        sources.len()
    );

    // Every source must parse successfully
    for (path, content) in &sources {
        parse_schema(content)
            .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()));
    }
}

#[test]
fn test_parse_schema_returns_types() {
    let root = qemu_root();
    let entry = find_schema_entry(root).unwrap();
    let sources = read_schema(&entry).unwrap();

    let mut total_tokens = 0usize;
    for (_path, content) in &sources {
        let schema = parse_schema(content).unwrap();
        total_tokens += schema.0.len();
    }

    // QEMU schema defines hundreds of types
    assert!(
        total_tokens > 100,
        "expected >100 schema tokens total, got {total_tokens}"
    );
}

#[test]
fn test_generate_code_produces_valid_rust() {
    let root = qemu_root();
    let entry = find_schema_entry(root).unwrap();

    let output = generate_qapi_code(root, &entry)
        .expect("generate_qapi_code failed");

    // Output should be non-trivial
    assert!(
        output.len() > 1000,
        "generated code suspiciously short: {} bytes",
        output.len()
    );

    // Output should parse as valid Rust
    let parsed: syn::File = syn::parse_str(&output)
        .expect("generated code is not valid Rust");

    // Should contain many items (structs, enums, etc.)
    assert!(
        parsed.items.len() > 50,
        "expected >50 top-level items, got {}",
        parsed.items.len()
    );
}

// ── Multi-version parser compatibility tests ──
// These test that the parser can handle QEMU schemas from all target versions.

const TARGET_VERSIONS: &[&str] = &[
    "v1.1.0", "v1.2.0", "v1.3.0", "v1.4.0", "v1.5.0", "v1.6.0", "v1.7.0",
    "v2.0.0", "v2.1.0", "v2.2.0", "v2.3.0", "v2.4.0", "v2.5.0", "v2.6.0", "v2.7.0", "v2.8.0", "v2.9.0", "v2.10.0", "v2.11.0", "v2.12.0",
    "v3.0.0", "v3.1.0",
    "v4.0.0", "v4.1.0", "v4.2.0",
    "v5.0.0", "v5.1.0", "v5.2.0",
    "v6.0.0", "v6.1.0", "v6.2.0",
    "v7.0.0", "v7.1.0", "v7.2.0",
    "v8.0.0", "v8.1.0", "v8.2.0",
    "v9.0.0", "v9.1.0", "v9.2.0",
    "v10.0.0", "v10.1.0", "v10.2.0",
];

fn qemu_repo() -> &'static Path {
    qemu_root()
}

#[test]
fn test_parse_all_target_versions() {
    let repo = qemu_repo();
    let mut failures = Vec::new();

    for tag in TARGET_VERSIONS {
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

#[test]
fn test_process_ir_all_target_versions() {
    let repo = qemu_repo();
    let mut failures = Vec::new();

    for tag in TARGET_VERSIONS {
        eprint!("  Processing IR for {}... ", tag);
        let tmpdir = match extract_schema_from_tag(repo, tag) {
            Ok(d) => d,
            Err(e) => {
                failures.push(format!("{}: extract failed: {}", tag, e));
                continue;
            }
        };

        let entry = match find_extracted_schema_entry(tmpdir.path()) {
            Some(e) => e,
            None => {
                failures.push(format!("{}: no schema entry point", tag));
                continue;
            }
        };

        let version = parse_version_tag(tag);
        match generate_qapi_code_versioned(tmpdir.path(), &entry, version) {
            Ok(output) => {
                // Verify it's valid Rust
                match syn::parse_str::<syn::File>(&output) {
                    Ok(f) => eprintln!("OK ({} items, {} bytes)", f.items.len(), output.len()),
                    Err(e) => {
                        failures.push(format!("{}: invalid Rust: {}", tag, e));
                        eprintln!("FAIL: invalid Rust");
                    }
                }
            }
            Err(e) => {
                failures.push(format!("{}: codegen failed: {}", tag, e));
                eprintln!("FAIL: {}", e);
            }
        }
    }

    assert!(
        failures.is_empty(),
        "Failed IR processing for {} version(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn test_build_unified_registry() {
    let repo = qemu_repo();

    let registry = build_unified_registry(repo, TARGET_VERSIONS)
        .expect("build_unified_registry failed");

    // Should have many types
    assert!(
        registry.types.len() > 100,
        "expected >100 unified types, got {}",
        registry.types.len()
    );

    // Should have tracked all target versions
    assert_eq!(registry.versions.len(), TARGET_VERSIONS.len());

    // Some types should have been removed across versions
    let removed_count = registry.types.values().filter(|t| match t {
        qapi_spec_parser::unified::UnifiedType::Enum(e) => e.versions.until.is_some(),
        qapi_spec_parser::unified::UnifiedType::Struct(s) => s.versions.until.is_some(),
    }).count();

    eprintln!("  Unified registry: {} types, {} removed", registry.types.len(), removed_count);
    assert!(removed_count > 0, "expected some removed types across versions");
}

#[test]
fn test_unified_codegen_produces_valid_rust() {
    let repo = qemu_repo();

    let code = qapi_spec_parser::unified::generate_unified_code(repo, TARGET_VERSIONS)
        .expect("generate_unified_code failed");

    assert!(
        code.len() > 10000,
        "unified code suspiciously short: {} bytes",
        code.len()
    );

    let parsed: syn::File = syn::parse_str(&code)
        .expect("unified generated code is not valid Rust");

    eprintln!("  Unified code: {} items, {} bytes", parsed.items.len(), code.len());
    assert!(
        parsed.items.len() > 100,
        "expected >100 items in unified code, got {}",
        parsed.items.len()
    );
}

// ═══════════════════════════════════════════════════════════
// Phase 7: Version-specific era tests
// ═══════════════════════════════════════════════════════════

use qapi_spec_parser::QapiSchemaToken;

/// Era A (v1.1.0): 'type' keyword, inline structs, simple unions, bare doc blocks
#[test]
fn test_era_a_v1_1_type_keyword() {
    let repo = qemu_root();
    let version = parse_version_tag("v1.1.0").unwrap();
    let tmpdir = extract_schema_from_tag(repo, "v1.1.0")
        .expect("Should extract v1.1.0");
    let entry = find_extracted_schema_entry(tmpdir.path())
        .expect("Should find schema entry for v1.1.0");
    let sources = read_schema_with_version(&entry, Some(version))
        .expect("Should read v1.1.0 schema");

    // v1.1.0 should have at least 30 types
    let total_tokens: usize = sources.iter().map(|(_, content)| {
        let schema = parse_schema(content).unwrap();
        schema.iter().filter(|t| matches!(t,
            QapiSchemaToken::Struct(_) | QapiSchemaToken::Enum(_) |
            QapiSchemaToken::Command(_) | QapiSchemaToken::Union(_)
        )).count()
    }).sum();
    assert!(total_tokens > 30, "v1.1.0 should have >30 type definitions, got {}", total_tokens);
}

/// Era B (v2.1.0): includes, events, multi-file schema
#[test]
fn test_era_b_v2_1_includes_and_events() {
    let repo = qemu_root();
    let version = parse_version_tag("v2.1.0").unwrap();
    let tmpdir = extract_schema_from_tag(repo, "v2.1.0")
        .expect("Should extract v2.1.0");
    let entry = find_extracted_schema_entry(tmpdir.path())
        .expect("Should find schema entry for v2.1.0");
    let sources = read_schema_with_version(&entry, Some(version))
        .expect("Should read v2.1.0 schema");

    // v2.1.0 has events (in event.json)
    let has_events = sources.iter().any(|(_, content)| {
        let schema = parse_schema(content).unwrap();
        schema.iter().any(|t| matches!(t, QapiSchemaToken::Event(_)))
    });
    assert!(has_events, "v2.1.0 should have event definitions");

    // v2.1.0 uses includes (multi-file schema)
    let has_includes = sources.iter().any(|(_, content)| {
        let schema = parse_schema(content).unwrap();
        schema.iter().any(|t| matches!(t, QapiSchemaToken::Include(_)))
    });
    assert!(has_includes, "v2.1.0 should have include directives");

    // v2.1.0 should have multiple source files
    assert!(sources.len() > 1, "v2.1.0 should have multiple schema files, got {}", sources.len());
}

/// Era C (v2.5.0): 'struct' keyword, 'alternate' keyword
#[test]
fn test_era_c_v2_5_struct_and_alternate() {
    let repo = qemu_root();
    let version = parse_version_tag("v2.5.0").unwrap();
    let tmpdir = extract_schema_from_tag(repo, "v2.5.0")
        .expect("Should extract v2.5.0");
    let entry = find_extracted_schema_entry(tmpdir.path())
        .expect("Should find schema entry for v2.5.0");
    let sources = read_schema_with_version(&entry, Some(version))
        .expect("Should read v2.5.0 schema");

    let has_alternates = sources.iter().any(|(_, content)| {
        let schema = parse_schema(content).unwrap();
        schema.iter().any(|t| matches!(t, QapiSchemaToken::Alternate(_)))
    });
    assert!(has_alternates, "v2.5.0 should have alternate definitions");
}

/// Era D (v4.0.0): conditions, features, modern pragma
#[test]
fn test_era_d_v4_0_conditions_and_features() {
    let repo = qemu_root();
    let version = parse_version_tag("v4.0.0").unwrap();
    let tmpdir = extract_schema_from_tag(repo, "v4.0.0")
        .expect("Should extract v4.0.0");
    let entry = find_extracted_schema_entry(tmpdir.path())
        .expect("Should find schema entry for v4.0.0");
    let sources = read_schema_with_version(&entry, Some(version))
        .expect("Should read v4.0.0 schema");

    let has_pragma = sources.iter().any(|(_, content)| {
        let schema = parse_schema(content).unwrap();
        schema.iter().any(|t| matches!(t, QapiSchemaToken::Pragma(_)))
    });
    assert!(has_pragma, "v4.0.0 should have pragma definitions");

    let has_conditions = sources.iter().any(|(_, content)| {
        let schema = parse_schema(content).unwrap();
        schema.iter().any(|t| match t {
            QapiSchemaToken::Command(c) => c.r#if.is_some(),
            QapiSchemaToken::Struct(s) => s.r#if.is_some(),
            _ => false,
        })
    });
    assert!(has_conditions, "v4.0.0 should have conditional types");
}

/// Era E (v10.2.0): modern format, no simple unions
#[test]
fn test_era_e_v10_2_modern_format() {
    let repo = qemu_root();
    let version = parse_version_tag("v10.2.0").unwrap();
    let tmpdir = extract_schema_from_tag(repo, "v10.2.0")
        .expect("Should extract v10.2.0");
    let entry = find_extracted_schema_entry(tmpdir.path())
        .expect("Should find schema entry for v10.2.0");
    let sources = read_schema_with_version(&entry, Some(version))
        .expect("Should read v10.2.0 schema");

    // All unions should have discriminators (no simple unions in v10.2.0)
    let all_unions_have_discriminator = sources.iter().all(|(_, content)| {
        let schema = parse_schema(content).unwrap();
        schema.iter().all(|t| match t {
            QapiSchemaToken::Union(u) => u.discriminator.is_some(),
            _ => true,
        })
    });
    assert!(all_unions_have_discriminator, "v10.2.0 should have no simple unions");

    let total_types: usize = sources.iter().map(|(_, content)| {
        let schema = parse_schema(content).unwrap();
        schema.iter().filter(|t| matches!(t,
            QapiSchemaToken::Struct(_) | QapiSchemaToken::Enum(_) |
            QapiSchemaToken::Command(_) | QapiSchemaToken::Union(_) |
            QapiSchemaToken::Event(_) | QapiSchemaToken::Alternate(_)
        )).count()
    }).sum();
    assert!(total_types > 500, "v10.2.0 should have >500 type definitions, got {}", total_types);
}

/// Test doc comments are preserved in parsed IR
#[test]
fn test_doc_preservation_in_ir() {
    let repo = qemu_root();
    let version = parse_version_tag("v9.1.0").unwrap();
    let tmpdir = extract_schema_from_tag(repo, "v9.1.0")
        .expect("Should extract v9.1.0");
    let entry = find_extracted_schema_entry(tmpdir.path())
        .expect("Should find schema entry for v9.1.0");
    let sources = read_schema_with_version(&entry, Some(version))
        .expect("Should read v9.1.0 schema");

    let mut found_status_doc = false;
    for (_, content) in &sources {
        let schema = parse_schema(content).unwrap();
        for token in schema.iter() {
            if let QapiSchemaToken::Struct(s) = token {
                if s.name == "StatusInfo" {
                    if let Some(doc) = &s.doc {
                        assert!(!doc.description.is_empty(), "StatusInfo should have doc description");
                        found_status_doc = true;
                    }
                }
            }
        }
    }
    assert!(found_status_doc, "Should find StatusInfo with doc comment in v9.1.0");
}

/// Test unified codegen produces doc comments
#[test]
fn test_unified_codegen_has_doc_comments() {
    let repo = qemu_root();
    let code_str = qapi_spec_parser::unified::generate_unified_code(repo, TARGET_VERSIONS)
        .expect("Failed to generate unified code");

    let doc_count = code_str.matches("/**").count() + code_str.matches("///").count();
    assert!(doc_count > 100, "Unified codegen should produce >100 doc comments, got {}", doc_count);
}

/// Test SpiceServerInfo (required parser fixes for v2.1.0 'type' + base struct)
#[test]
fn test_spice_types_from_v2_1() {
    let repo = qemu_root();
    let code_str = qapi_spec_parser::unified::generate_unified_code(repo, TARGET_VERSIONS)
        .expect("Failed to generate unified code");

    assert!(code_str.contains("SpiceServerInfo"), "Generated code should contain SpiceServerInfo");
    assert!(code_str.contains("SpiceBasicInfo"), "Generated code should contain SpiceBasicInfo");
}

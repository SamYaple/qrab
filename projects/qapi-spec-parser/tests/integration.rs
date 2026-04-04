use std::path::Path;

use qapi_spec_parser::{parse_schema, read_schema, generate_qapi_code};
use qapi_spec_parser::versions::{find_schema_entry, extract_schema_from_tag, find_extracted_schema_entry};
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
    "v1.1.0", "v2.0.0", "v2.5.0", "v3.0.0", "v5.0.0", "v7.0.0", "v9.0.0", "v9.1.0", "v10.2.0",
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
    let tmpdir = extract_schema_from_tag(repo, tag)
        .map_err(|e| format!("extract failed: {}", e))?;

    let entry = find_extracted_schema_entry(tmpdir.path())
        .ok_or_else(|| "no schema entry point found".to_string())?;

    let sources = read_schema(&entry)
        .map_err(|e| format!("read_schema failed: {}", e))?;

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

        match generate_qapi_code(tmpdir.path(), &entry) {
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

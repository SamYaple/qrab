use std::path::Path;

use qapi_spec_parser::{parse_schema, read_schema, generate_qapi_code};
use qapi_spec_parser::versions::find_schema_entry;

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

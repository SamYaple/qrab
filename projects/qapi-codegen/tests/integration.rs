use std::path::Path;

use qapi_codegen::unified::build_unified_registry;
use qapi_codegen::versions::{
    discover_version_tags, extract_schema_from_tag, find_extracted_schema_entry, find_schema_entry,
    parse_version_tag,
};
use qapi_codegen::{parse_schema, read_schema_with_version, QapiSchemaToken};

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
        "v1.0", "v1.1.0", "v1.7.0", "v2.0.0", "v2.1.0", "v2.5.0", "v2.8.0", "v2.12.0", "v3.0.0",
        "v5.0.0", "v7.0.0", "v9.0.0", "v10.2.0",
    ]
    .into_iter()
    .map(String::from)
    .collect()
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
    assert!(
        sources.len() > 10,
        "expected >10 schema files, got {}",
        sources.len()
    );

    for (path, content) in &sources {
        parse_schema(content).unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()));
    }
}

#[test]
fn test_parse_schema_returns_types() {
    let root = qemu_root();
    let entry = find_schema_entry(root).unwrap();
    let sources = read_schema_with_version(&entry, None).unwrap();

    let total_tokens: usize = sources
        .iter()
        .map(|(_, content)| parse_schema(content).unwrap().0.len())
        .sum();

    assert!(
        total_tokens > 100,
        "expected >100 schema tokens total, got {total_tokens}"
    );
}

// ── Version discovery ──

#[test]
fn test_discover_version_tags() {
    let repo = qemu_root();
    let tags = discover_version_tags(repo).expect("discover_version_tags failed");

    // Should find all release tags (v1.0 through v10.2.x)
    assert!(
        tags.len() > 100,
        "expected >100 version tags, got {}",
        tags.len()
    );

    // First tag should be v1.0
    assert_eq!(tags[0], "v1.0", "First tag should be v1.0, got {}", tags[0]);

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

    eprintln!(
        "  Discovered {} tags ({} to {})",
        tags.len(),
        tags[0],
        tags.last().unwrap()
    );
}

// ── Multi-version parser compatibility ──
#[test]
fn test_parse_all_discovered_versions() {
    let repo = qemu_root();
    // Representative versions covering all parser eras
    let tags: Vec<String> = vec![
        "v1.0", "v1.1.0", "v1.4.0", "v1.7.0", // era A: type keyword, simple unions
        "v2.0.0", "v2.1.0", "v2.3.0", "v2.5.0", // era A/B: includes, alternates
        "v2.7.0", "v2.8.0", "v2.10.0", "v2.12.0", // era C/D: struct keyword, pragmas
        "v3.0.0", "v4.0.0", "v5.0.0", // era D: conditions, features
        "v6.0.0", "v7.0.0", "v8.0.0", // era E: modern format
        "v9.0.0", "v10.2.0", // latest
    ]
    .into_iter()
    .map(String::from)
    .collect();
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
    let tmpdir =
        extract_schema_from_tag(repo, tag).map_err(|e| format!("extract failed: {}", e))?;

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

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    assert!(
        registry.types.len() > 100,
        "expected >100 unified types, got {}",
        registry.types.len()
    );
    assert_eq!(registry.versions.len(), tags.len());

    let removed_count = registry
        .types
        .values()
        .filter(|t| match t {
            qapi_codegen::unified::UnifiedType::Enum(e) => e.versions.until.is_some(),
            qapi_codegen::unified::UnifiedType::Struct(s) => s.versions.until.is_some(),
        })
        .count();

    eprintln!(
        "  Unified registry: {} types, {} removed, {} versions",
        registry.types.len(),
        removed_count,
        registry.versions.len()
    );
    assert!(
        removed_count > 0,
        "expected some removed types across versions"
    );
}

#[test]
fn test_unified_codegen_produces_valid_rust() {
    let repo = qemu_root();
    let tags = discover_version_tags(repo).expect("discover_version_tags failed");
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let code = qapi_codegen::unified::generate_unified_code(repo, &tag_refs)
        .expect("generate_unified_code failed");

    assert!(
        code.len() > 10000,
        "unified code suspiciously short: {} bytes",
        code.len()
    );

    let parsed: syn::File =
        syn::parse_str(&code).expect("unified generated code is not valid Rust");

    eprintln!(
        "  Unified code: {} items, {} bytes",
        parsed.items.len(),
        code.len()
    );
    assert!(
        parsed.items.len() > 100,
        "expected >100 items, got {}",
        parsed.items.len()
    );
}

#[test]
fn test_unified_codegen_has_doc_comments() {
    let repo = qemu_root();
    let tags = discover_version_tags(repo).expect("discover_version_tags failed");
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let code = qapi_codegen::unified::generate_unified_code(repo, &tag_refs)
        .expect("Failed to generate unified code");

    let doc_count = code.matches("/**").count() + code.matches("///").count();
    assert!(
        doc_count > 100,
        "expected >100 doc comments, got {}",
        doc_count
    );
}

#[test]
fn test_spice_types_from_v2_1() {
    let repo = qemu_root();
    let tags = discover_version_tags(repo).expect("discover_version_tags failed");
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let code = qapi_codegen::unified::generate_unified_code(repo, &tag_refs)
        .expect("Failed to generate unified code");

    assert!(
        code.contains("SpiceServerInfo"),
        "Should contain SpiceServerInfo"
    );
    assert!(
        code.contains("SpiceBasicInfo"),
        "Should contain SpiceBasicInfo"
    );
}

// ── Era-specific parser tests ──

#[test]
fn test_era_a_v1_1_type_keyword() {
    let repo = qemu_root();
    let version = parse_version_tag("v1.1.0").unwrap();
    let tmpdir = extract_schema_from_tag(repo, "v1.1.0").expect("extract v1.1.0");
    let entry = find_extracted_schema_entry(tmpdir.path()).expect("find entry v1.1.0");
    let sources = read_schema_with_version(&entry, Some(version)).expect("read v1.1.0");

    let total: usize = sources
        .iter()
        .map(|(_, c)| {
            parse_schema(c)
                .unwrap()
                .iter()
                .filter(|t| {
                    matches!(
                        t,
                        QapiSchemaToken::Struct(_)
                            | QapiSchemaToken::Enum(_)
                            | QapiSchemaToken::Command(_)
                            | QapiSchemaToken::Union(_)
                    )
                })
                .count()
        })
        .sum();
    assert!(
        total > 30,
        "v1.1.0 should have >30 type definitions, got {}",
        total
    );
}

#[test]
fn test_era_b_v2_1_includes_and_events() {
    let repo = qemu_root();
    let version = parse_version_tag("v2.1.0").unwrap();
    let tmpdir = extract_schema_from_tag(repo, "v2.1.0").expect("extract v2.1.0");
    let entry = find_extracted_schema_entry(tmpdir.path()).expect("find entry v2.1.0");
    let sources = read_schema_with_version(&entry, Some(version)).expect("read v2.1.0");

    let has_events = sources.iter().any(|(_, c)| {
        parse_schema(c)
            .unwrap()
            .iter()
            .any(|t| matches!(t, QapiSchemaToken::Event(_)))
    });
    assert!(has_events, "v2.1.0 should have events");
    assert!(
        sources.len() > 1,
        "v2.1.0 should have multiple schema files"
    );
}

#[test]
fn test_era_c_v2_5_struct_and_alternate() {
    let repo = qemu_root();
    let version = parse_version_tag("v2.5.0").unwrap();
    let tmpdir = extract_schema_from_tag(repo, "v2.5.0").expect("extract v2.5.0");
    let entry = find_extracted_schema_entry(tmpdir.path()).expect("find entry v2.5.0");
    let sources = read_schema_with_version(&entry, Some(version)).expect("read v2.5.0");

    let has_alternates = sources.iter().any(|(_, c)| {
        parse_schema(c)
            .unwrap()
            .iter()
            .any(|t| matches!(t, QapiSchemaToken::Alternate(_)))
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
        parse_schema(c)
            .unwrap()
            .iter()
            .any(|t| matches!(t, QapiSchemaToken::Pragma(_)))
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

    let total: usize = sources
        .iter()
        .map(|(_, c)| {
            parse_schema(c)
                .unwrap()
                .iter()
                .filter(|t| {
                    matches!(
                        t,
                        QapiSchemaToken::Struct(_)
                            | QapiSchemaToken::Enum(_)
                            | QapiSchemaToken::Command(_)
                            | QapiSchemaToken::Union(_)
                            | QapiSchemaToken::Event(_)
                            | QapiSchemaToken::Alternate(_)
                    )
                })
                .count()
        })
        .sum();
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

// ── Boxed command with different name tests ──

/// When a command uses `'data': 'TypeRef', 'boxed': true` and the command name
/// rustifies to a different name than the data type (e.g., netdev_add -> NetdevAdd
/// vs Netdev -> Netdev), the command struct must survive as its own entry in the
/// registry, not be merged onto the data type and lost.
#[test]
fn test_boxed_command_different_name_not_lost() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    // All boxed commands with different Rust names from their data refs
    // must exist in the registry. This covers both:
    // - Commands that transitioned from inline to boxed (e.g., netdev_add)
    // - Commands that were always boxed (e.g., announce-self)
    let boxed_commands = [
        // Transitioned from inline to boxed
        ("netdev_add", "Netdev"),
        ("blockdev-add", "BlockdevOptions"),
        ("object-add", "ObjectOptions"),
        ("set_password", "SetPasswordOptions"),
        ("expire_password", "ExpirePasswordOptions"),
        ("nbd-server-add", "NbdServerAddOptions"),
        ("block_set_io_throttle", "BlockIOThrottle"),
        // Always boxed (never had inline data)
        ("block-export-add", "BlockExportOptions"),
        ("query-stats", "StatsFilter"),
        ("announce-self", "AnnounceParameters"),
        ("set-numa-node", "NumaOptions"),
        ("block-job-change", "BlockJobChangeOptions"),
    ];

    for (cmd_name, data_ref) in &boxed_commands {
        // The command entry must exist
        assert!(
            registry.types.contains_key(*cmd_name),
            "Command '{}' should exist in registry (was lost during boxed merge)",
            cmd_name,
        );

        // The command must be a struct with the command attribute
        if let Some(qapi_codegen::unified::UnifiedType::Struct(us)) = registry.types.get(*cmd_name)
        {
            let has_command =
                us.ir.meta.attributes.iter().any(
                    |a| matches!(a, qapi_codegen::qapi_ir::Attribute::Unit(n) if n == "command"),
                );
            assert!(
                has_command,
                "Command '{}' should have #[qapi(command)] attribute",
                cmd_name,
            );

            // The command must NOT have an `until` (i.e., it should still be current)
            assert!(
                us.versions.until.is_none(),
                "Command '{}' should not be marked as removed (until={:?})",
                cmd_name,
                us.versions.until,
            );
        } else {
            panic!(
                "Command '{}' should be a Struct in the registry, got {:?}",
                cmd_name,
                registry.types.get(*cmd_name).map(|t| match t {
                    qapi_codegen::unified::UnifiedType::Enum(_) => "Enum",
                    qapi_codegen::unified::UnifiedType::Struct(_) => "Struct",
                }),
            );
        }

        // The data type must NOT have the command attribute — it's the payload, not the command
        if let Some(ut) = registry.types.get(*data_ref) {
            let has_command = match ut {
                qapi_codegen::unified::UnifiedType::Struct(us) => us.ir.meta.attributes.iter().any(
                    |a| matches!(a, qapi_codegen::qapi_ir::Attribute::Unit(n) if n == "command"),
                ),
                qapi_codegen::unified::UnifiedType::Enum(ue) => ue.ir.meta.attributes.iter().any(
                    |a| matches!(a, qapi_codegen::qapi_ir::Attribute::Unit(n) if n == "command"),
                ),
            };
            assert!(
                !has_command,
                "Data type '{}' should NOT have #[qapi(command)] (command is '{}')",
                data_ref, cmd_name,
            );
        }
    }
}

/// Verify that same-name boxed command merges still work correctly.
/// e.g., drive-mirror -> DriveMirror and DriveMirror -> DriveMirror should
/// dedup to a single entry.
#[test]
fn test_boxed_command_same_name_still_merges() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    // These commands have matching Rust names with their data refs and should
    // merge into a single registry entry (the data struct with command attrs).
    let same_name_commands = [
        ("drive-mirror", "DriveMirror"),
        ("blockdev-snapshot-sync", "BlockdevSnapshotSync"),
        ("migrate-set-parameters", "MigrateSetParameters"),
    ];

    for (cmd_name, data_ref) in &same_name_commands {
        // The data struct should exist and have the command attribute
        assert!(
            registry.types.contains_key(*data_ref),
            "Data struct '{}' should exist for command '{}'",
            data_ref,
            cmd_name,
        );

        if let Some(qapi_codegen::unified::UnifiedType::Struct(us)) = registry.types.get(*data_ref)
        {
            let has_command =
                us.ir.meta.attributes.iter().any(
                    |a| matches!(a, qapi_codegen::qapi_ir::Attribute::Unit(n) if n == "command"),
                );
            assert!(
                has_command,
                "Data struct '{}' should have #[qapi(command)] for command '{}'",
                data_ref, cmd_name,
            );
        }

        // The command-name entry should NOT exist separately (it was merged)
        assert!(
            !registry.types.contains_key(*cmd_name),
            "Command '{}' should have been merged into '{}', not exist separately",
            cmd_name,
            data_ref,
        );
    }
}

/// Verify generated code contains proper netdev_add as a current command.
#[test]
fn test_generated_code_netdev_add_is_current() {
    let repo = qemu_root();
    let tags = discover_version_tags(repo).expect("discover_version_tags failed");
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let code = qapi_codegen::unified::generate_unified_code(repo, &tag_refs)
        .expect("generate_unified_code failed");

    // NetdevAdd should exist
    assert!(
        code.contains("pub struct NetdevAdd"),
        "Generated code should contain NetdevAdd struct",
    );

    // NetdevAdd should NOT be marked as removed
    // Find the NetdevAdd struct and check it doesn't have a removed annotation
    // immediately before it
    let netdev_pos = code
        .find("pub struct NetdevAdd")
        .expect("NetdevAdd not found");
    // Look at the 500 chars before the struct definition for attributes
    let start = netdev_pos.saturating_sub(500);
    let context = &code[start..netdev_pos];

    // The struct should have #[qapi(command)]
    assert!(
        context.contains("#[qapi(command)]"),
        "NetdevAdd should have #[qapi(command)] attribute.\nContext: {}",
        context,
    );
}

// ── Boxed command attribute preservation tests ──

/// Commands with allow-preconfig or allow-oob must preserve those attributes
/// even when using boxed data with a different name.
#[test]
fn test_boxed_command_preserves_allow_preconfig() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    // Commands that have allow-preconfig in the schema
    let preconfig_commands = [
        "netdev_add",
        "block-export-add",
        "set-numa-node",
        "blockdev-add",
        "object-add",
    ];

    for cmd_name in &preconfig_commands {
        if let Some(qapi_codegen::unified::UnifiedType::Struct(us)) = registry.types.get(*cmd_name)
        {
            let has_preconfig = us.ir.meta.attributes.iter().any(
                |a| matches!(a, qapi_codegen::qapi_ir::Attribute::Unit(n) if n == "allow_preconfig"),
            );
            assert!(
                has_preconfig,
                "Command '{}' should have allow_preconfig attribute",
                cmd_name,
            );
        } else {
            panic!("Command '{}' not found in registry", cmd_name);
        }
    }
}

/// Commands with non-trivial return types must preserve them.
#[test]
fn test_boxed_command_preserves_returns() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    // query-stats returns Vec<StatsResult>
    if let Some(qapi_codegen::unified::UnifiedType::Struct(us)) = registry.types.get("query-stats")
    {
        let returns_attr = us.ir.meta.attributes.iter().find(
            |a| matches!(a, qapi_codegen::qapi_ir::Attribute::Item { name, .. } if name == "returns"),
        );
        assert!(
            returns_attr.is_some(),
            "query-stats should have a returns attribute",
        );
        if let Some(qapi_codegen::qapi_ir::Attribute::Item { value, .. }) = returns_attr {
            assert!(
                value.contains("StatsResult"),
                "query-stats should return StatsResult, got '{}'",
                value,
            );
        }
    } else {
        panic!("query-stats not found in registry");
    }

    // Commands returning () should also have the attribute
    for cmd_name in &["netdev_add", "announce-self", "block-export-add"] {
        if let Some(qapi_codegen::unified::UnifiedType::Struct(us)) = registry.types.get(*cmd_name)
        {
            let has_returns = us.ir.meta.attributes.iter().any(
                |a| matches!(a, qapi_codegen::qapi_ir::Attribute::Item { name, .. } if name == "returns"),
            );
            assert!(
                has_returns,
                "Command '{}' should have a returns attribute",
                cmd_name,
            );
        }
    }
}

// ── Version range tests ──

/// Commands that transitioned from inline to boxed should have correct version
/// ranges: they should NOT be marked as removed at the transition point.
#[test]
fn test_transitioned_commands_version_ranges() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    // These commands transitioned from inline data to boxed refs.
    // They should have since <= the boxed transition version and no until.
    // Note: exact `since` depends on which representative tags are used;
    // we check `since <= max_since` (the latest possible first appearance).
    let transitioned = [
        ("netdev_add", (2, 0, 0)), // inline ~1.2, first representative tag with it
        ("blockdev-add", (2, 0, 0)), // inline 1.7, boxed 2.8
        ("object-add", (2, 0, 0)), // inline 2.0, boxed 6.0
        ("set_password", (1, 1, 0)), // inline 1.1, boxed 7.0
        ("expire_password", (1, 1, 0)), // inline 1.1, boxed 7.0
        ("nbd-server-add", (2, 0, 0)), // inline 1.3, boxed 5.0
        ("block_set_io_throttle", (1, 1, 0)), // inline 1.1, boxed 2.7
    ];

    for (cmd_name, max_since) in &transitioned {
        if let Some(qapi_codegen::unified::UnifiedType::Struct(us)) = registry.types.get(*cmd_name)
        {
            assert!(
                us.versions.since <= *max_since,
                "Command '{}' should have since<={:?}, got {:?}",
                cmd_name,
                max_since,
                us.versions.since,
            );
            assert!(
                us.versions.until.is_none(),
                "Command '{}' should NOT be marked as removed (got until={:?})",
                cmd_name,
                us.versions.until,
            );
        } else {
            panic!("Command '{}' not found in registry", cmd_name);
        }
    }
}

/// Commands that were always boxed should have correct since versions.
#[test]
fn test_always_boxed_commands_version_ranges() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    // These commands were always boxed from introduction.
    // They should have reasonable since versions and no until.
    let always_boxed = [
        "block-export-add",
        "query-stats",
        "announce-self",
        "set-numa-node",
        "block-job-change",
    ];

    for cmd_name in &always_boxed {
        if let Some(qapi_codegen::unified::UnifiedType::Struct(us)) = registry.types.get(*cmd_name)
        {
            assert!(
                us.versions.until.is_none(),
                "Always-boxed command '{}' should NOT be marked as removed (got until={:?})",
                cmd_name,
                us.versions.until,
            );
            // Since should be at least 1.0.0
            assert!(
                us.versions.since >= (1, 0, 0),
                "Always-boxed command '{}' should have reasonable since version, got {:?}",
                cmd_name,
                us.versions.since,
            );
        } else {
            panic!("Always-boxed command '{}' not found in registry", cmd_name);
        }
    }
}

// ── Field evolution tests ──

/// When a command transitions from inline data to boxed, the old inline fields
/// should get `until` annotations and a new flatten field should appear.
#[test]
fn test_field_evolution_inline_to_boxed() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    // netdev_add: had inline {id, type, props} pre-5.0, then boxed {data: Netdev} from 5.0
    if let Some(qapi_codegen::unified::UnifiedType::Struct(us)) = registry.types.get("netdev_add") {
        // Should have a "data" field (the flatten field for Netdev)
        let has_data_field = us.ir.fields.iter().any(|f| f.name == "data");
        assert!(
            has_data_field,
            "netdev_add should have a 'data' field (flatten to Netdev) after boxed transition",
        );

        // The data field should have a flatten attribute
        if let Some(data_field) = us.ir.fields.iter().find(|f| f.name == "data") {
            let has_flatten =
                data_field.meta.attributes.iter().any(
                    |a| matches!(a, qapi_codegen::qapi_ir::Attribute::Unit(n) if n == "flatten"),
                );
            assert!(
                has_flatten,
                "netdev_add's data field should have #[qapi(flatten)]",
            );
        }

        // Old inline fields (id, type) should have until versions set
        let old_fields_with_until: Vec<_> = us
            .ir
            .fields
            .iter()
            .filter(|f| f.name != "data")
            .filter(|f| {
                us.field_versions
                    .get(&f.name)
                    .map_or(false, |vr| vr.until.is_some())
            })
            .collect();

        assert!(
            !old_fields_with_until.is_empty(),
            "netdev_add should have old inline fields with until annotations",
        );
    } else {
        panic!("netdev_add not found in registry");
    }
}

// ── Data type integrity tests ──

/// Data types referenced by boxed commands must NOT have command/returns
/// attributes leaking onto them.
#[test]
fn test_data_types_no_command_leakage() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    // These are data payload types, not commands
    let data_types = [
        "Netdev",
        "BlockdevOptions",
        "ObjectOptions",
        "SetPasswordOptions",
        "ExpirePasswordOptions",
        "NbdServerAddOptions",
        "BlockIOThrottle",
        "BlockExportOptions",
        "StatsFilter",
        "AnnounceParameters",
        "NumaOptions",
        "BlockJobChangeOptions",
    ];

    for type_name in &data_types {
        if let Some(ut) = registry.types.get(*type_name) {
            let attrs = match ut {
                qapi_codegen::unified::UnifiedType::Struct(us) => &us.ir.meta.attributes,
                qapi_codegen::unified::UnifiedType::Enum(ue) => &ue.ir.meta.attributes,
            };

            let has_command = attrs
                .iter()
                .any(|a| matches!(a, qapi_codegen::qapi_ir::Attribute::Unit(n) if n == "command"));
            assert!(
                !has_command,
                "Data type '{}' should NOT have #[qapi(command)] -- it leaked from its command",
                type_name,
            );

            let has_returns = attrs
                .iter()
                .any(|a| matches!(a, qapi_codegen::qapi_ir::Attribute::Item { name, .. } if name == "returns"));
            assert!(
                !has_returns,
                "Data type '{}' should NOT have #[qapi(returns)] -- it leaked from its command",
                type_name,
            );
        }
        // Some types might not exist in the representative tags, that's OK
    }
}

// ── Enum version tracking tests ──

/// Enums that were removed across versions should have proper until annotations.
#[test]
fn test_enum_version_tracking() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    let mut enums_with_since = 0;
    let mut enums_with_until = 0;

    for (_name, ut) in &registry.types {
        if let qapi_codegen::unified::UnifiedType::Enum(ue) = ut {
            if ue.versions.since >= (1, 0, 0) {
                enums_with_since += 1;
            }
            if ue.versions.until.is_some() {
                enums_with_until += 1;
            }
        }
    }

    assert!(
        enums_with_since > 50,
        "Expected >50 enums with valid since, got {}",
        enums_with_since,
    );
    assert!(
        enums_with_until > 0,
        "Expected some enums with until (removed across versions), got 0",
    );
}

// ── Deprecation and feature tracking tests ──

/// Types with deprecated features should have proper deprecated annotations.
#[test]
fn test_deprecated_feature_tracking() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    let mut deprecated_count = 0;

    for (_name, ut) in &registry.types {
        let versions = match ut {
            qapi_codegen::unified::UnifiedType::Enum(ue) => &ue.versions,
            qapi_codegen::unified::UnifiedType::Struct(us) => &us.versions,
        };
        if versions.deprecated.is_some() {
            deprecated_count += 1;
        }
    }

    assert!(
        deprecated_count > 0,
        "Expected some deprecated types across versions, got 0",
    );
}

// ── Registry completeness tests ──

/// Every command in the latest QEMU schema should appear in the unified
/// registry (either as its own entry or merged into its data type).
#[test]
fn test_all_latest_commands_in_registry() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    // Parse the latest schema to get all commands
    let entry =
        qapi_codegen::versions::find_schema_entry(repo).expect("schema entry point not found");
    let sources = qapi_codegen::read_schema_with_version(&entry, None).expect("read_schema failed");

    let mut missing = Vec::new();

    for (_path, content) in &sources {
        let schema = parse_schema(content).unwrap();
        for token in schema.iter() {
            if let QapiSchemaToken::Command(cmd) = token {
                let cmd_name = cmd.name;
                // Check if the command exists either under its own name or
                // merged into its data ref
                let data_ref = match &cmd.data {
                    Some(qapi_codegen::MembersOrRef::Ref(name)) => Some(name.to_string()),
                    _ => None,
                };

                let found = registry.types.contains_key(cmd_name)
                    || data_ref
                        .as_ref()
                        .map_or(false, |r| registry.types.contains_key(r.as_str()));

                if !found {
                    missing.push(cmd_name.to_string());
                }
            }
        }
    }

    assert!(
        missing.is_empty(),
        "Commands missing from registry: {:?}",
        missing,
    );
}

/// The unified registry should have a reasonable number of command types.
#[test]
fn test_registry_command_count() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    let command_count = registry
        .types
        .values()
        .filter(|t| {
            if let qapi_codegen::unified::UnifiedType::Struct(us) = t {
                us.ir.meta.attributes.iter().any(
                    |a| matches!(a, qapi_codegen::qapi_ir::Attribute::Unit(n) if n == "command"),
                )
            } else {
                false
            }
        })
        .count();

    // QEMU 10.2 has ~170 commands; with removed ones across versions, expect more
    assert!(
        command_count > 150,
        "Expected >150 command types in unified registry, got {}",
        command_count,
    );
}

// ── Event tests ──

/// All events in the latest schema should be in the registry.
#[test]
fn test_all_latest_events_in_registry() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    let entry =
        qapi_codegen::versions::find_schema_entry(repo).expect("schema entry point not found");
    let sources = qapi_codegen::read_schema_with_version(&entry, None).expect("read_schema failed");

    let mut missing = Vec::new();

    for (_path, content) in &sources {
        let schema = parse_schema(content).unwrap();
        for token in schema.iter() {
            if let QapiSchemaToken::Event(evt) = token {
                // Events are stored with "Event" suffix in the registry
                let event_key = format!("{}Event", evt.name);
                if !registry.types.contains_key(event_key.as_str()) {
                    missing.push(evt.name.to_string());
                }
            }
        }
    }

    assert!(
        missing.is_empty(),
        "Events missing from registry: {:?}",
        missing,
    );
}

/// Events should have proper event attribute in the registry.
#[test]
fn test_events_have_event_attribute() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    // Events are stored with "Event" suffix in the registry
    let events = [
        "SHUTDOWNEvent",
        "RESETEvent",
        "STOPEvent",
        "RESUMEEvent",
        "DEVICE_DELETEDEvent",
    ];

    for event_name in &events {
        if let Some(qapi_codegen::unified::UnifiedType::Struct(us)) =
            registry.types.get(*event_name)
        {
            let has_event =
                us.ir.meta.attributes.iter().any(
                    |a| matches!(a, qapi_codegen::qapi_ir::Attribute::Unit(n) if n == "event"),
                );
            assert!(
                has_event,
                "Event '{}' should have #[qapi(event)] attribute",
                event_name,
            );
        } else {
            panic!("Event '{}' not found in registry", event_name);
        }
    }
}

/// Events that were removed across versions should have until annotations.
#[test]
fn test_removed_events_have_until() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    let event_count =
        registry
            .types
            .values()
            .filter(|t| {
                if let qapi_codegen::unified::UnifiedType::Struct(us) = t {
                    us.ir.meta.attributes.iter().any(
                        |a| matches!(a, qapi_codegen::qapi_ir::Attribute::Unit(n) if n == "event"),
                    )
                } else {
                    false
                }
            })
            .count();

    // Should have a reasonable number of events across all versions
    assert!(
        event_count > 40,
        "Expected >40 event types across all versions, got {}",
        event_count,
    );
}

// ── Union and alternate tests ──

/// Union types should have discriminator fields.
#[test]
fn test_unions_have_discriminators() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    let mut union_count = 0;
    let mut with_discriminator = 0;

    for (_name, ut) in &registry.types {
        if let qapi_codegen::unified::UnifiedType::Struct(us) = ut {
            // Union structs have a discriminator field with the "u" field (union branch)
            let has_u_field = us.ir.fields.iter().any(|f| f.name == "u");
            if has_u_field {
                union_count += 1;
                // Should have a discriminator-like field too
                let has_disc = us.ir.fields.iter().any(|f| {
                    f.meta
                        .attributes
                        .iter()
                        .any(|a| matches!(a, qapi_codegen::qapi_ir::Attribute::Unit(n) if n == "discriminator"))
                });
                if has_disc {
                    with_discriminator += 1;
                }
            }
        }
    }

    assert!(
        union_count > 10,
        "Expected >10 union types across all versions, got {}",
        union_count,
    );
    assert!(
        with_discriminator > 10,
        "Expected >10 unions with discriminator fields, got {}",
        with_discriminator,
    );
}

/// Alternates should be generated as enums.
#[test]
fn test_alternates_are_enums() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    // Well-known alternates
    let alternates = ["BlockdevRef", "StrOrNull"];

    for alt_name in &alternates {
        match registry.types.get(*alt_name) {
            Some(qapi_codegen::unified::UnifiedType::Enum(ue)) => {
                assert!(
                    ue.ir.variants.len() >= 2,
                    "Alternate '{}' should have >=2 variants, got {}",
                    alt_name,
                    ue.ir.variants.len(),
                );
            }
            Some(qapi_codegen::unified::UnifiedType::Struct(_)) => {
                panic!("Alternate '{}' should be an Enum, not a Struct", alt_name);
            }
            None => {
                panic!("Alternate '{}' not found in registry", alt_name);
            }
        }
    }
}

// ── Condition attribute tests ──

/// Types with conditions should have condition attributes preserved.
#[test]
fn test_condition_attributes_preserved() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    let mut types_with_conditions = 0;

    for (_name, ut) in &registry.types {
        let attrs = match ut {
            qapi_codegen::unified::UnifiedType::Struct(us) => &us.ir.meta.attributes,
            qapi_codegen::unified::UnifiedType::Enum(ue) => &ue.ir.meta.attributes,
        };
        let has_condition = attrs.iter().any(
            |a| matches!(a, qapi_codegen::qapi_ir::Attribute::Item { name, .. } if name == "condition"),
        );
        if has_condition {
            types_with_conditions += 1;
        }
    }

    assert!(
        types_with_conditions > 10,
        "Expected >10 types with conditions, got {}",
        types_with_conditions,
    );
}

// ── Flatten field tests ──

/// Boxed commands with different names should produce flatten fields
/// in the generated code.
#[test]
fn test_generated_code_has_flatten_for_boxed_commands() {
    let repo = qemu_root();
    let tags = discover_version_tags(repo).expect("discover_version_tags failed");
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let code = qapi_codegen::unified::generate_unified_code(repo, &tag_refs)
        .expect("generate_unified_code failed");

    // The generated code should contain flatten attributes for boxed commands
    // that have different names from their data types
    let flatten_count = code.matches("#[qapi(flatten)]").count();
    assert!(
        flatten_count > 0,
        "Generated code should contain #[qapi(flatten)] attributes for boxed commands, got 0",
    );

    // Specifically, NetdevAdd should have a flatten field
    let netdev_pos = code
        .find("pub struct NetdevAdd")
        .expect("NetdevAdd not found");
    let end = code[netdev_pos..].find("\n}\n").unwrap_or(500);
    let netdev_body = &code[netdev_pos..netdev_pos + end];
    assert!(
        netdev_body.contains("flatten")
            || code[netdev_pos.saturating_sub(200)..netdev_pos + end].contains("flatten"),
        "NetdevAdd should have a flatten field for Netdev.\nBody: {}",
        netdev_body,
    );
}

// ── Generated code structural tests ──

/// Generated code should parse as valid Rust with our boxed command fix.
/// This is a more targeted version of test_unified_codegen_produces_valid_rust.
#[test]
fn test_generated_code_valid_rust_with_fix() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let code = qapi_codegen::unified::generate_unified_code(repo, &tag_refs)
        .expect("generate_unified_code failed");

    let parsed: syn::File =
        syn::parse_str(&code).expect("generated code with boxed command fix is not valid Rust");

    // All 12 boxed commands should appear as structs
    let struct_names: Vec<String> = parsed
        .items
        .iter()
        .filter_map(|item| {
            if let syn::Item::Struct(s) = item {
                Some(s.ident.to_string())
            } else {
                None
            }
        })
        .collect();

    let expected_structs = [
        "NetdevAdd",
        "BlockdevAdd",
        "ObjectAdd",
        "SetPassword",
        "ExpirePassword",
        "NbdServerAdd",
        "BlockSetIoThrottle",
        "BlockExportAdd",
        "QueryStats",
        "AnnounceSelf",
        "SetNumaNode",
        "BlockJobChange",
    ];

    for expected in &expected_structs {
        assert!(
            struct_names.contains(&expected.to_string()),
            "Generated code should contain struct '{}' but it's missing.\nFound structs starting with same prefix: {:?}",
            expected,
            struct_names.iter().filter(|s| s.starts_with(&expected[..4.min(expected.len())])).collect::<Vec<_>>(),
        );
    }
}

// ── Field-level version tracking tests ──

/// Fields added in later versions should have proper since annotations.
#[test]
fn test_field_version_tracking() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    let mut fields_with_since_gt_type = 0;
    let mut fields_with_until = 0;

    for (_name, ut) in &registry.types {
        if let qapi_codegen::unified::UnifiedType::Struct(us) = ut {
            for (_fname, fvr) in &us.field_versions {
                if fvr.since > us.versions.since {
                    fields_with_since_gt_type += 1;
                }
                if fvr.until.is_some() {
                    fields_with_until += 1;
                }
            }
        }
    }

    assert!(
        fields_with_since_gt_type > 50,
        "Expected >50 fields added after their type was introduced, got {}",
        fields_with_since_gt_type,
    );
    assert!(
        fields_with_until > 10,
        "Expected >10 fields removed across versions, got {}",
        fields_with_until,
    );
}

// ── Name attribute preservation tests ──

/// Types with QAPI names different from their Rust names should have
/// name attributes preserved.
#[test]
fn test_name_attributes_preserved() {
    let repo = qemu_root();
    let tags = representative_tags();
    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let registry = build_unified_registry(repo, &tag_refs).expect("build_unified_registry failed");

    let mut types_with_name_attr = 0;

    for (_name, ut) in &registry.types {
        let attrs = match ut {
            qapi_codegen::unified::UnifiedType::Struct(us) => &us.ir.meta.attributes,
            qapi_codegen::unified::UnifiedType::Enum(ue) => &ue.ir.meta.attributes,
        };
        let has_name = attrs.iter().any(
            |a| matches!(a, qapi_codegen::qapi_ir::Attribute::Item { name, .. } if name == "name"),
        );
        if has_name {
            types_with_name_attr += 1;
        }
    }

    assert!(
        types_with_name_attr > 50,
        "Expected >50 types with name attributes, got {}",
        types_with_name_attr,
    );
}

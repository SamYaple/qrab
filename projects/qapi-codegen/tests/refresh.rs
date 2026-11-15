//! Current-schema coverage and regressions in the generation pipeline.
use qapi_codegen::unified::{
    UnifiedRegistry, UnifiedType, Version, build_qga_registry, build_unified_registry,
};
use qapi_codegen::versions::{
    PINNED_VERSIONS, PinnedVersion, parse_pinned_versions, verify_pinned_versions,
};
use qapi_codegen::{QapiSchemaToken, parse_schema, read_schema_with_version};
use std::{collections::BTreeSet, path::Path, process::Command};

fn qemu() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/qemu")
        .canonicalize()
        .unwrap()
}

fn assert_definition_coverage(repo: &Path, entry: &str, version: (u32, u32, u32)) {
    let output = Command::new("python3")
        .arg("-c")
        .arg(
            r#"
import sys
sys.path.insert(0, sys.argv[1] + '/scripts')
from qapi.parser import QAPISchemaParser
parser = QAPISchemaParser(sys.argv[1] + '/' + sys.argv[2])
for expr in parser.exprs:
    for kind in ['enum', 'struct', 'union', 'alternate', 'command', 'event']:
        if kind in expr:
            print(kind + ' ' + expr[kind])
"#,
        )
        .arg(repo)
        .arg(entry)
        .output()
        .expect("python3 is required for the upstream parser oracle");
    assert!(
        output.status.success(),
        "Upstream parser failed for {version:?}/{entry}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected: BTreeSet<_> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    let mut actual = BTreeSet::new();
    for (_, content) in read_schema_with_version(&repo.join(entry), Some(version)).unwrap() {
        for token in parse_schema(&content).unwrap() {
            let (kind, name) = match token {
                QapiSchemaToken::Enum(v) => ("enum", v.name),
                QapiSchemaToken::Struct(v) => ("struct", v.name),
                QapiSchemaToken::Union(v) => ("union", v.name),
                QapiSchemaToken::Alternate(v) => ("alternate", v.name),
                QapiSchemaToken::Command(v) => ("command", v.name),
                QapiSchemaToken::Event(v) => ("event", v.name),
                _ => continue,
            };
            assert!(
                actual.insert(format!("{kind} {name}")),
                "Duplicate {kind} {name}"
            );
        }
    }
    assert_eq!(
        actual, expected,
        "Schema coverage differs for {version:?}/{entry}"
    );
}

#[test]
fn current_definitions_match_upstream_parser() {
    for entry in ["qapi/qapi-schema.json", "qga/qapi-schema.json"] {
        assert_definition_coverage(&qemu(), entry, (11, 1, 3));
    }
}

#[test]
fn recent_release_matrix_matches_each_releases_own_parser() {
    use qapi_codegen::versions::{extract_schema_from_tag, parse_version_tag};
    let pins = parse_pinned_versions(PINNED_VERSIONS).unwrap();
    let repo = qemu();
    for series in [(9, 2), (10, 0), (10, 1), (10, 2), (11, 0), (11, 1)] {
        let pin = pins
            .iter()
            .rev()
            .find(|pin| {
                let (major, minor, _) = parse_version_tag(&pin.tag).unwrap();
                (major, minor) == series
            })
            .unwrap();
        let schemas = extract_schema_from_tag(&repo, &pin.tag).unwrap();
        // Use that release's parser, so newer documentation rules cannot mask
        // a compatibility regression in the Rust grammar.
        let archive = Command::new("git")
            .current_dir(&repo)
            .args(["archive", &pin.tag, "--", "scripts/qapi"])
            .output()
            .unwrap();
        assert!(
            archive.status.success(),
            "{}",
            String::from_utf8_lossy(&archive.stderr)
        );
        tar::Archive::new(archive.stdout.as_slice())
            .unpack(schemas.path())
            .unwrap();
        for entry in ["qapi/qapi-schema.json", "qga/qapi-schema.json"] {
            assert_definition_coverage(schemas.path(), entry, parse_version_tag(&pin.tag).unwrap());
        }
    }
}

#[test]
fn qga_commands_do_not_overwrite_their_response_types() {
    let registry = build_qga_registry(&qemu(), &["v11.1.3"]).unwrap();
    for (name, field) in [
        ("GuestFileRead", "buf-b64"),
        ("GuestFileWrite", "count"),
        ("GuestFileSeek", "position"),
        ("GuestExec", "pid"),
        ("GuestExecStatus", "exited"),
    ] {
        let UnifiedType::Struct(response) = &registry.types[name] else {
            panic!("Missing response {name}")
        };
        assert!(
            response.ir.fields.iter().any(|f| f.name == field),
            "Lost {name}.{field}"
        );
        let UnifiedType::Struct(command) = &registry.types[&format!("{name}Command")] else {
            panic!("Missing command {name}")
        };
        assert!(command.ir.meta.attributes.iter().any(|a| matches!(a, qapi_codegen::qapi_ir::Attribute::Item { name: key, value } if key == "returns" && value == name)));
    }
}

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "tag.gpgsign=false",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn generation_rejects_broken_qga_and_moved_release_tags() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::create_dir(repo.path().join("qapi")).unwrap();
    std::fs::create_dir(repo.path().join("qga")).unwrap();
    std::fs::write(repo.path().join("qapi/qapi-schema.json"), "").unwrap();
    std::fs::write(
        repo.path().join("qga/qapi-schema.json"),
        "{ 'command': 'broken', 'unknown': true }",
    )
    .unwrap();
    git(repo.path(), &["init", "-q"]);
    git(repo.path(), &["add", "."]);
    git(
        repo.path(),
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "fixture",
        ],
    );
    git(repo.path(), &["tag", "v11.1.3"]);
    let pin = PinnedVersion {
        tag: "v11.1.3".into(),
        commit: git(repo.path(), &["rev-parse", "HEAD"]),
    };
    verify_pinned_versions(repo.path(), &[pin]).unwrap();
    assert!(
        verify_pinned_versions(
            repo.path(),
            &[PinnedVersion {
                tag: "v11.1.3".into(),
                commit: "0".repeat(40)
            }]
        )
        .is_err()
    );
    let error = build_qga_registry(repo.path(), &["v11.1.3"]).unwrap_err();
    assert!(format!("{error:#}").contains("v11.1.3"));
}

#[test]
fn include_aliases_are_deduplicated_but_cycles_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root.json");
    let child = dir.path().join("child.json");
    std::fs::write(
        &root,
        "{ 'include': 'child.json' }\n{ 'include': './child.json' }\n",
    )
    .unwrap();
    std::fs::write(&child, "{ 'struct': 'S', 'data': {} }\n").unwrap();
    assert_eq!(read_schema_with_version(&root, None).unwrap().len(), 2);
    std::fs::write(&child, "{ 'include': './root.json' }\n").unwrap();
    assert!(
        read_schema_with_version(&root, None)
            .unwrap_err()
            .to_string()
            .contains("inclusion loop")
    );
}

#[test]
fn invalid_utf8_boundary_in_schema_excerpt_does_not_panic() {
    assert!(parse_schema(&"☃".repeat(100)).is_err());
}

type ReturnChange<S = &'static str> = (S, Vec<(Version, S)>);

/// Every command whose return type changed between pinned releases, with
/// the first version of each type. Generated code must use the newest one.
fn changed_returns(registry: &UnifiedRegistry) -> Vec<ReturnChange<String>> {
    registry
        .types
        .iter()
        .filter_map(|(name, ty)| match ty {
            UnifiedType::Struct(s) if s.returns_history.len() > 1 => {
                assert_eq!(
                    s.returns(),
                    s.returns_history.last().map(|(_, r)| r.as_str()),
                    "{name} must return its newest type"
                );
                Some((name.clone(), s.returns_history.clone()))
            }
            _ => None,
        })
        .collect()
}

fn owned(expected: &[ReturnChange]) -> Vec<ReturnChange<String>> {
    expected
        .iter()
        .map(|(name, history)| {
            (
                name.to_string(),
                history.iter().map(|(v, r)| (*v, r.to_string())).collect(),
            )
        })
        .collect()
}

fn pinned_tags() -> Vec<String> {
    parse_pinned_versions(PINNED_VERSIONS)
        .unwrap()
        .into_iter()
        .map(|p| p.tag)
        .collect()
}

#[test]
fn command_returns_follow_the_newest_release() {
    let tags = pinned_tags();
    let tags: Vec<_> = tags.iter().map(String::as_str).collect();
    let qmp = build_unified_registry(&qemu(), &tags).unwrap();
    let qmp_expected: &[ReturnChange] = &[
        (
            "DeviceListProperties",
            vec![
                ((1, 2, 0), "Vec<DevicePropertyInfo>"),
                ((2, 12, 0), "Vec<ObjectPropertyInfo>"),
            ],
        ),
        (
            "QueryTarget",
            vec![((1, 2, 0), "TargetInfo"), ((10, 1, 0), "QemuTargetInfo")],
        ),
    ];
    assert_eq!(changed_returns(&qmp), owned(qmp_expected));

    let qga = build_qga_registry(&qemu(), &tags).unwrap();
    let qga_expected: &[ReturnChange] = &[(
        "GuestFstrim",
        vec![
            ((1, 2, 0), "()"),
            ((2, 4, 0), "GuestFilesystemTrimResponse"),
        ],
    )];
    assert_eq!(changed_returns(&qga), owned(qga_expected));
}

#[test]
fn guest_fstrim_returns_the_trim_response() {
    let registry = build_qga_registry(&qemu(), &["v1.4.0", "v2.4.0", "v11.1.3"]).unwrap();
    let UnifiedType::Struct(command) = &registry.types["GuestFstrim"] else {
        panic!("Missing guest-fstrim")
    };
    assert_eq!(command.returns(), Some("GuestFilesystemTrimResponse"));
}

#[test]
fn schema_grammar_matches_qemu() {
    let repo = qemu();
    let dir = tempfile::tempdir().unwrap();
    let cases = [
        (
            "{ 'enum': 'UnionKind', 'data': ['first', 'second'] }\n{ 'union': 'EmptyPayload', 'base': {'kind': 'UnionKind'}, 'discriminator': 'kind', 'data': {} }",
            true,
        ),
        ("{ 'enum': 'Empty', 'data': [] }", true),
        ("{ 'struct': 'Empty', 'data': {} }", true),
        ("{ 'struct': 'MissingData' }", false),
        ("{ 'enum': 'MissingData' }", false),
        ("{ 'pragma': {} }", true),
        (
            "{ 'pragma': { 'doc-required': false, 'doc-required': true } }",
            false,
        ),
        ("{ 'enum ': 'ExampleEnum', 'data': ['a'] }", false),
        ("{ 'enum': 'ExampleEnum', 'data': ['line\nbreak'] }", false),
        ("{ 'enum': 'ExampleEnum', 'data': ['☃'] }", false),
        (
            "{ 'enum': 'ExampleEnum', 'data': ['a'], 'if': { 'all': [] } }",
            false,
        ),
        (
            "{ 'enum': 'ExampleEnum', 'data': ['a'], 'if': 'config_a' }",
            false,
        ),
        (
            "{ 'enum': 'ExampleEnum', 'data': ['a'], 'if': {'not': {'any': ['CONFIG_A', 'CONFIG_B']}} }",
            true,
        ),
    ];
    qapi_codegen::set_parser_version((11, 1, 3));
    for (index, (input, valid)) in cases.iter().enumerate() {
        let file = dir.path().join(format!("case-{index}.json"));
        std::fs::write(&file, input).unwrap();
        let output = Command::new("python3")
            .arg("-c")
            .arg(
                r#"
import sys
sys.path.insert(0, sys.argv[1] + '/scripts')
from qapi.schema import QAPISchema
from qapi.error import QAPIError
try:
    QAPISchema(sys.argv[2])
except QAPIError as error:
    print(error, file=sys.stderr)
    sys.exit(1)
"#,
            )
            .arg(&repo)
            .arg(&file)
            .output()
            .unwrap();
        assert!(
            output.status.code().is_some_and(|code| code <= 1),
            "Python oracle failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.status.success(),
            *valid,
            "QEMU oracle disagrees for {input}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            parse_schema(input).is_ok(),
            *valid,
            "Rust parser disagrees for {input}"
        );
    }
}

#[cfg(unix)]
#[test]
fn escaped_backslash_in_include_is_decoded() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root.json");
    let child = dir.path().join(r"child\name.json");
    std::fs::write(&root, r"{ 'include': 'child\\name.json' }").unwrap();
    std::fs::write(&child, "{ 'struct': 'Empty', 'data': {} }").unwrap();
    let sources = read_schema_with_version(&root, None).unwrap();
    assert_eq!(sources.len(), 2);
    assert!(sources.iter().any(|(path, _)| path == &child));
}

#[test]
fn simple_union_layout_lifetimes_follow_each_types_actual_transition() {
    let registry = build_unified_registry(
        &qemu(),
        &["v2.9.0", "v2.10.0", "v6.1.0", "v6.2.0", "v11.1.3"],
    )
    .unwrap();
    for (name, until) in [
        ("SocketAddress", (2, 10, 0)),
        ("SocketAddressLegacy", (6, 2, 0)),
    ] {
        let UnifiedType::Struct(value) = &registry.types[name] else {
            panic!("missing {name}")
        };
        assert_eq!(value.attr_versions["simple_union"].until, Some(until));
    }
}

#[test]
fn legacy_untagged_unions_keep_alternate_identity_and_original_lifetime() {
    let registry =
        build_unified_registry(&qemu(), &["v1.7.0", "v2.0.0", "v2.1.0", "v11.1.3"]).unwrap();
    let UnifiedType::Enum(value) = &registry.types["BlockdevRef"] else {
        panic!("legacy alternate became a tagged struct")
    };
    assert_eq!(value.versions.since, (1, 7, 0));
    assert!(
        value
            .ir
            .variants
            .iter()
            .any(|variant| variant.name == "reference")
    );
    assert!(!registry.types.contains_key("BlockdevRefBranch"));
    assert!(!registry.types.contains_key("BlockdevRefType"));
}

#[test]
fn historical_guest_agent_schema_paths_are_not_silently_skipped() {
    let registry = build_qga_registry(&qemu(), &["v0.15.0", "v1.0", "v1.4.0", "v11.1.3"]).unwrap();
    assert_eq!(registry.versions.first(), Some(&(0, 15, 0)));
    let UnifiedType::Struct(command) = &registry.types["GuestPing"] else {
        panic!("missing early guest-ping")
    };
    assert_eq!(command.versions.since, (0, 15, 0));
    let UnifiedType::Struct(info) = &registry.types["GuestAgentInfo"] else {
        panic!("missing early guest-info result")
    };
    assert_eq!(info.versions.since, (0, 15, 0));
}

fn schema_fixture_repo(path: &str, content: &str) -> tempfile::TempDir {
    let repo = tempfile::tempdir().unwrap();
    let file = repo.path().join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, content).unwrap();
    git(repo.path(), &["init", "-q"]);
    git(repo.path(), &["add", "."]);
    git(
        repo.path(),
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ],
    );
    git(repo.path(), &["tag", "v11.1.3"]);
    repo
}

#[test]
fn qga_only_schema_repositories_extract_without_a_qmp_tree() {
    let repo = schema_fixture_repo(
        "qga/qapi-schema.json",
        "{ 'command':'guest-test', 'returns':'bool' }",
    );
    let registry = build_qga_registry(repo.path(), &["v11.1.3"]).unwrap();
    assert!(registry.types.contains_key("GuestTest"));
    let error = build_unified_registry(repo.path(), &["v11.1.3"]).unwrap_err();
    assert!(format!("{error:#}").contains("No QMP schema entry point"));
    git(repo.path(), &["tag", "v0.15.0"]);
    assert!(
        build_unified_registry(repo.path(), &["v0.15.0"])
            .unwrap()
            .types
            .is_empty()
    );
}

#[test]
fn missing_modern_guest_agent_schemas_are_errors_not_skipped_releases() {
    let repo = schema_fixture_repo("qapi/qapi-schema.json", "");
    let error = build_qga_registry(repo.path(), &["v11.1.3"]).unwrap_err();
    assert!(format!("{error:#}").contains("No QGA schema entry point"));
    git(repo.path(), &["tag", "v0.14.0"]);
    assert!(
        build_qga_registry(repo.path(), &["v0.14.0"])
            .unwrap()
            .types
            .is_empty()
    );
}

#[test]
fn guest_agent_discovery_reaches_its_first_schema_release() {
    let tags = qapi_codegen::versions::discover_qga_version_tags(&qemu()).unwrap();
    assert_eq!(tags.first().map(String::as_str), Some("v0.15.0"));
    assert!(tags.iter().any(|tag| tag == "v0.15.1"));
}

#[test]
fn four_component_release_schemas_generate_the_same_rust_as_their_base_release() {
    use qapi_codegen::unified::{generate_qga_code, generate_unified_code};
    let repo = qemu();
    let discovered = qapi_codegen::versions::discover_version_tags(&repo).unwrap();
    for (base, extra) in [
        ("v2.4.0", "v2.4.0.1"),
        ("v2.5.1", "v2.5.1.1"),
        ("v2.8.1", "v2.8.1.1"),
        ("v3.1.1", "v3.1.1.1"),
    ] {
        assert!(
            discovered.iter().position(|tag| tag == base).unwrap()
                < discovered.iter().position(|tag| tag == extra).unwrap()
        );
        let before = generate_unified_code(&repo, &[base]).unwrap();
        let after = generate_unified_code(&repo, &[extra]).unwrap();
        assert_eq!(before.combined(), after.combined(), "QMP {extra}");
        assert_eq!(before.metadata, after.metadata, "QMP metadata {extra}");
        let before = generate_qga_code(&repo, &[base]).unwrap();
        let after = generate_qga_code(&repo, &[extra]).unwrap();
        assert_eq!(before.combined(), after.combined(), "QGA {extra}");
        assert_eq!(before.metadata, after.metadata, "QGA metadata {extra}");
    }
}

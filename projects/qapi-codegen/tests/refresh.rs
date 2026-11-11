//! Current-schema coverage and regressions in the generation pipeline.
use qapi_codegen::unified::{UnifiedType, build_qga_registry};
use qapi_codegen::versions::{PinnedVersion, verify_pinned_versions};
use qapi_codegen::{QapiSchemaToken, parse_schema, read_schema_with_version};
use std::{collections::BTreeSet, path::Path, process::Command};

fn qemu() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/qemu")
        .canonicalize()
        .unwrap()
}

#[test]
fn current_definitions_match_upstream_parser() {
    let repo = qemu();
    for entry in ["qapi/qapi-schema.json", "qga/qapi-schema.json"] {
        // QEMU's own parser provides an independent completeness oracle.
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
            .arg(&repo)
            .arg(entry)
            .output()
            .expect("python3 is required for the upstream parser oracle");
        assert!(
            output.status.success(),
            "Upstream parser failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let expected: BTreeSet<_> = String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();
        let mut actual = BTreeSet::new();
        for (_, content) in read_schema_with_version(&repo.join(entry), Some((11, 1, 3))).unwrap() {
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
        assert_eq!(actual, expected, "Schema coverage differs for {entry}");
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

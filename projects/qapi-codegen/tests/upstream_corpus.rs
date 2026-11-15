//! Positive fixtures maintained by QEMU exercise syntax beyond production schemas.
use qapi_codegen::read_schema_with_version;
use std::path::Path;

#[test]
fn freeform_documentation_cannot_replace_definition_documentation() {
    qapi_codegen::set_parser_version((11, 1, 3));
    let section = "##\n# = Section\n#\n# Body.\n##\n";
    for ending in ["\n", "\r\n", "\r"] {
        for definition in [
            "{'struct':'Example','data':{}}",
            "{'enum':'Example','data':['one']}",
            "{'alternate':'Example','data':{'one':'str'}}",
            "{'union':'Example','data':{'one':'str'}}",
            "{'command':'example'}",
            "{'event':'EXAMPLE'}",
        ] {
            let input = format!("{section}{definition}").replace('\n', ending);
            assert!(qapi_codegen::parse_schema(&input).is_err(), "{input:?}");
        }
        let input =
            format!("{section}##\n# @Example:\n# Summary.\n##\n{{'struct':'Example','data':{{}}}}")
                .replace('\n', ending);
        qapi_codegen::parse_schema(&input).unwrap();
        let input = format!("{section}{{'pragma':{{}}}}\n{{'struct':'Example','data':{{}}}}")
            .replace('\n', ending);
        qapi_codegen::parse_schema(&input).unwrap();
    }
    for version in [(2, 7, 0), (2, 8, 0)] {
        qapi_codegen::set_parser_version(version);
        qapi_codegen::parse_schema(&format!("{section}{{'struct':'Example','data':{{}}}}"))
            .unwrap();
    }
    qapi_codegen::set_parser_version((11, 1, 3));
}

#[test]
fn pre_formal_docs_remain_comments_and_keep_recognized_sections() {
    qapi_codegen::set_parser_version((2, 8, 0));
    let input = "# ordinary\n##\n# = Section\n##\n## @Example\n# Description.\n##\n{'struct':'Example','data':{### inline comment\n'value':'str'}}\n### trailing comment";
    let schema = qapi_codegen::parse_schema(input).unwrap();
    assert!(
        schema
            .iter()
            .any(|token| matches!(token, qapi_codegen::QapiSchemaToken::Doc(_)))
    );
    assert!(schema.iter().any(
        |token| matches!(token, qapi_codegen::QapiSchemaToken::Struct(value) if value.doc.is_some())
    ));
    qapi_codegen::set_parser_version((2, 9, 0));
    assert!(qapi_codegen::parse_schema(input).is_err());
    qapi_codegen::set_parser_version((11, 1, 3));
}

#[test]
fn malformed_schema_diagnostics_locate_later_definitions() {
    qapi_codegen::set_parser_version((11, 1, 3));
    let prefix = "# Unicode comment: ☃\n{'struct':'Good','data':{}}\n\n    ";
    for ending in ["\n", "\r\n", "\r"] {
        let prefix = prefix.replace('\n', ending);
        for invalid in [
            "{'struct':'Bad','data':{'value':}}",
            "{'struct':'Bad','data':{'value':'str',}}",
            "{'unknown':'Bad'}",
            "unexpected",
        ] {
            let error = qapi_codegen::parse_schema(&format!("{prefix}{invalid}"))
                .unwrap_err()
                .to_string();
            assert!(error.contains("line 4, column 5"), "{error}");
            assert!(error.contains(invalid), "{error}");
        }
    }
    let error = qapi_codegen::parse_schema(&format!(
        "{prefix}{{'struct':'Bad','data':{{'value':'str','value':'int'}}}}"
    ))
    .unwrap_err()
    .to_string();
    assert!(error.contains("line 4, column 43"), "{error}");
    assert!(error.contains("Verify"), "{error}");
}

#[test]
fn upstream_positive_schema_fixtures_parse() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/qemu/tests/qapi-schema");
    let mut fixtures = Vec::new();
    for entry in std::fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|extension| extension == "err")
            && std::fs::read_to_string(&path).unwrap().trim().is_empty()
            && path.with_extension("json").exists()
        {
            fixtures.push(path.with_extension("json"));
        }
    }
    fixtures.sort();
    assert!(
        !fixtures.is_empty(),
        "QEMU positive fixture corpus is missing"
    );
    let mut failures = Vec::new();
    for path in fixtures {
        if let Err(error) = read_schema_with_version(&path, Some((11, 1, 3))) {
            failures.push(format!("{}: {error:#}", path.display()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn assert_documentation_coverage(entry: &str) {
    use qapi_codegen::{QapiSchemaToken, parse_schema};
    use std::{collections::BTreeSet, process::Command};
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/qemu");
    let schema = repo.join(entry);
    let output = Command::new("python3")
        .arg("-c")
        .arg(
            r#"
import sys
sys.path.insert(0, sys.argv[1] + '/scripts')
from qapi.parser import QAPISchemaParser
for expr in QAPISchemaParser(sys.argv[2]).exprs:
    if expr.doc is not None:
        print(expr.doc.symbol)
        for section in expr.doc.all_sections:
            if section.kind.name in ('MEMBER', 'FEATURE'):
                print(expr.doc.symbol + '\t' + section.kind.name + '\t' + section.name + '\t' + ' '.join(section.text.split()))
"#,
        )
        .arg(&repo)
        .arg(&schema)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected: BTreeSet<_> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    let mut actual = BTreeSet::new();
    for (_, content) in read_schema_with_version(&schema, Some((11, 1, 3))).unwrap() {
        for token in parse_schema(&content).unwrap() {
            let doc = match token {
                QapiSchemaToken::Enum(value) => value.doc,
                QapiSchemaToken::Struct(value) => value.doc,
                QapiSchemaToken::Union(value) => value.doc,
                QapiSchemaToken::Alternate(value) => value.doc,
                QapiSchemaToken::Command(value) => value.doc,
                QapiSchemaToken::Event(value) => value.doc,
                _ => continue,
            };
            if let Some(doc) = doc {
                actual.insert(doc.name.to_string());
                for (kind, fields) in [("MEMBER", &doc.fields), ("FEATURE", &doc.features)] {
                    for (name, lines) in fields {
                        let text = lines
                            .join(" ")
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ");
                        actual.insert(format!("{}\t{kind}\t{name}\t{text}", doc.name));
                    }
                }
                if doc.name == "cmd" {
                    assert_eq!(doc.returns, ["@Object"]);
                    assert_eq!(doc.errors, ["some"]);
                    assert!(doc.body.iter().any(|line| line.starts_with("Returns:")));
                    assert!(
                        doc.body
                            .iter()
                            .any(|line| line.contains("admonition:: Notes"))
                    );
                }
            }
        }
    }
    assert_eq!(actual, expected);
}

#[test]
fn upstream_documentation_stays_attached_to_definitions() {
    assert_documentation_coverage("tests/qapi-schema/doc-good.json");
}

#[test]
fn production_documentation_matches_qemu() {
    for entry in ["qapi/qapi-schema.json", "qga/qapi-schema.json"] {
        assert_documentation_coverage(entry);
    }
}

#[test]
fn invalid_definition_documentation_cannot_fall_back_to_plain_comments() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/qemu/tests/qapi-schema");
    for fixture in [
        "doc-before-include.json",
        "doc-before-pragma.json",
        "doc-missing-expr.json",
        "doc-missing-colon.json",
        "doc-invalid-end.json",
        "doc-empty-symbol.json",
    ] {
        assert!(
            read_schema_with_version(&root.join(fixture), Some((11, 1, 3))).is_err(),
            "accepted {fixture}"
        );
    }
}

#[test]
fn committed_parse_errors_report_a_location_and_bounded_excerpt() {
    let input = format!(
        "# ☃\n{{ 'struct': 'Example', 'data': {{\n  'name': 'str',\n  'name': 'int' }} }}\n{}",
        "☃".repeat(1000)
    );
    let message = qapi_codegen::parse_schema(&input).unwrap_err().to_string();
    assert!(message.contains("line 4, column 3"), "{message}");
    assert!(
        message.chars().count() < 240,
        "error excerpt is not bounded"
    );
}

//! Historical literal decoding, include paths and canonical reference identity.
mod common;
use common::{git, run_fixture, write_module};
use qapi_codegen::{parsers::common::qstring, set_parser_version};
use std::{path::Path, process::Command};

#[test]
fn literal_values_match_mirrored_qemu_lexers() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/qemu");
    let literals = [
        "''",
        "'ordinary'",
        r"'a\\b'",
        r"'a\nb'",
        r"'\b\f\n\r\t'",
        r#"'\'\"\/\\'"#,
        r"'\u0041'",
        r"'\u0000'",
        r"'\u007f'",
        r"'\u0080'",
        r"'\u+041'",
        r"'\z'",
        "'snowman ☃'",
        "'tab\there'",
        "'DEL\u{7f}'",
        "'NUL\0'",
        "\"double\"",
        "'unterminated",
        "'raw\nline'",
        "'raw\rline'",
    ];
    for (tag, version, parser, class, mode) in [
        ("v2.3.0", (2, 3, 0), "scripts/qapi.py", "QAPISchema", "fp"),
        ("v2.4.0", (2, 4, 0), "scripts/qapi.py", "QAPISchema", "fp"),
        (
            "v4.1.0",
            (4, 1, 0),
            "scripts/qapi/common.py",
            "QAPISchemaParser",
            "fp",
        ),
        (
            "v4.2.0",
            (4, 2, 0),
            "scripts/qapi/parser.py",
            "QAPISchemaParser",
            "path",
        ),
        (
            "v11.1.3",
            (11, 1, 3),
            "scripts/qapi/parser.py",
            "QAPISchemaParser",
            "path",
        ),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let archive = Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["archive", tag, "scripts"])
            .output()
            .unwrap();
        assert!(archive.status.success());
        tar::Archive::new(archive.stdout.as_slice())
            .unpack(temp.path())
            .unwrap();
        for (index, literal) in literals.iter().enumerate() {
            std::fs::write(
                temp.path().join(format!("{index}.json")),
                format!("{{'literal':{literal}}}"),
            )
            .unwrap();
        }
        let output = Command::new("python3")
            .arg("-c")
            .arg(
                r#"
import collections, importlib, importlib.util, os, string, sys, types
root, parser, cls, mode, count = sys.argv[1:]
sys.path.insert(0, os.path.join(root, 'scripts'))
# Adapt obsolete Python 2 helpers; the QEMU lexer itself remains unchanged.
shim = types.ModuleType('ordereddict')
shim.OrderedDict = collections.OrderedDict
sys.modules['ordereddict'] = shim
if not hasattr(string, 'maketrans'):
    string.maketrans = str.maketrans
if parser.startswith('scripts/qapi/'):
    module = importlib.import_module(parser[len('scripts/'):-3].replace('/', '.'))
else:
    spec = importlib.util.spec_from_file_location('oracle', os.path.join(root, parser))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
for index in range(int(count)):
    filename = os.path.join(root, str(index) + '.json')
    try:
        if mode == 'fp':
            with open(filename, encoding='utf-8') as fp:
                parsed = getattr(module, cls)(fp)
        else:
            parsed = getattr(module, cls)(filename)
        expr = parsed.exprs[0]
        value = expr.get('expr', expr)['literal']
        print('ok:' + value.encode('utf-8').hex())
    except Exception:
        print('err')
"#,
            )
            .arg(temp.path())
            .arg(parser)
            .arg(class)
            .arg(mode)
            .arg(literals.len().to_string())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{tag}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let expected = String::from_utf8(output.stdout).unwrap();
        let expected: Vec<_> = expected.lines().collect();
        assert_eq!(expected.len(), literals.len());
        set_parser_version(version);
        for (literal, expected) in literals.iter().zip(expected) {
            let actual = match qstring(literal) {
                Ok(("", value)) => format!(
                    "ok:{}",
                    value
                        .as_bytes()
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>()
                ),
                _ => "err".into(),
            };
            assert_eq!(actual, expected, "{tag} {literal:?}");
        }
    }
}

#[test]
#[cfg(unix)] // POSIX filenames can contain literal backslashes and newlines.
fn include_paths_are_decoded_exactly_once() {
    use qapi_codegen::read_schema_with_version;
    for (filename, literal, version) in [
        ("one\\file.json", r"one\\file.json", (11, 1, 3)),
        ("two\\\\file.json", r"two\\\\file.json", (11, 1, 3)),
        ("leaf-e.json", r"leaf-\u0065.json", (4, 1, 0)),
        ("leaf-☃.json", "leaf-☃.json", (4, 1, 0)),
        ("leaf-\n.json", r"leaf-\n.json", (4, 1, 0)),
    ] {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join(filename), "{'struct':'Leaf','data':{}}").unwrap();
        let root = temp.path().join("root.json");
        std::fs::write(&root, format!("{{'include':'{literal}'}}")).unwrap();
        assert_eq!(
            read_schema_with_version(&root, Some(version))
                .unwrap()
                .len(),
            2
        );
    }
}

#[test]
fn escaped_keywords_names_and_cache_epochs_compile_and_round_trip() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("schema");
    std::fs::create_dir_all(repo.join("qapi")).unwrap();
    git(&repo, &["init", "-q"]);
    let entry = repo.join("qapi/qapi-schema.json");
    let epoch = r"{'enum':'EscapeEpoch','data':['\u0061','plain']}";
    std::fs::write(&entry, epoch).unwrap();
    git(&repo, &["add", "."]);
    git(
        &repo,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-qm",
            "legacy",
        ],
    );
    git(&repo, &["tag", "v2.3.0"]);
    git(&repo, &["tag", "v2.4.0"]);
    let escaped = concat!(
        r"{'struct':'\u0046oo','data':{'va\u006cue':'str','\u002aextra':'str'}}",
        "\n",
        r"{'\u0073truct':'Holder','data':{'item':'Foo','a\u0072ray':['\u0046oo'],'namespaced':'__org.qemu_Thing','implicit':'FooList'}}",
        "\n",
        r"{'struct':'\u005f\u005forg.qemu_Thing','data':{'marker':'bool'}}",
        "\n",
        r"{'command':'read-foo','returns':'\u0046oo'}",
        "\n",
    );
    std::fs::write(&entry, format!("{epoch}\n{escaped}")).unwrap();
    git(&repo, &["add", "."]);
    git(
        &repo,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-qm",
            "escaped",
        ],
    );
    git(&repo, &["tag", "v4.1.0"]);
    let modern = "{'enum':'EscapeEpoch','data':['a','plain']}\n{'struct':'Foo','data':{'value':'str','*extra':'str'}}\n{'struct':'Holder','data':{'item':'Foo','array':['Foo'],'namespaced':'__org.qemu_Thing','implicit':'FooList'}}\n{'struct':'__org.qemu_Thing','data':{'marker':'bool'}}\n{'command':'read-foo','returns':'Foo'}";
    std::fs::write(&entry, modern).unwrap();
    git(&repo, &["add", "."]);
    git(
        &repo,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-qm",
            "modern",
        ],
    );
    git(&repo, &["tag", "v11.1.3"]);
    let output = qapi_codegen::unified::generate_unified_code(
        &repo,
        &["v2.3.0", "v2.4.0", "v4.1.0", "v11.1.3"],
    )
    .unwrap();
    let fixture = temp.path().join("fixture");
    write_module(&fixture.join("src/generated"), output);
    run_fixture(
        &workspace,
        &fixture,
        r#"
pub use qapi_runtime::{PreconfigSafe,QapiCommand,QapiEvent,QapiFlatUnion,qemu_version,scoped_qemu_version,unavailable_type_message,with_version};
mod generated;
pub use generated::*;
fn main() {
    for (version, tag) in [((2,3,0),"u0061"),((2,4,0),"a"),((4,1,0),"a"),((11,1,3),"a")] {
        with_version(version, || {
            let value: EscapeEpoch = serde_json::from_value(serde_json::json!(tag)).unwrap();
            assert_eq!(serde_json::to_value(value).unwrap(), serde_json::json!(tag));
        });
    }
    with_version((4,1,0), || {
        let wire=serde_json::json!({"item":{"value":"item"},"array":[{"value":"array","extra":"yes"}],"namespaced":{"marker":true},"implicit":[{"value":"implicit"}]});
        let value: Holder=serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(value).unwrap(),wire);
        let _: <commands::ReadFoo as QapiCommand>::Returns = Foo { value:"return".into(), extra:None };
    });
}
"#,
    );
}

#[test]
fn decoded_duplicate_keys_are_detected_before_overwriting() {
    set_parser_version((4, 1, 0));
    let error =
        qapi_codegen::parse_schema(r"{'\u0073truct':'Foo','struct':'Bar','data':{}}").unwrap_err();
    assert!(error.to_string().contains("Verify"), "{error}");
}

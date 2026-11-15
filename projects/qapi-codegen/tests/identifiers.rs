//! Compile generated Rust for names accepted by QEMU but special in Rust.
use qapi_codegen::unified::generate_unified_code;
use std::{path::Path, process::Command};

fn git(repo: &Path, args: &[&str]) {
    let result = Command::new("git")
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn valid_schema_keywords_and_standard_type_names_compile_and_round_trip() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("schema");
    std::fs::create_dir_all(repo.join("qapi")).unwrap();
    let names = [
        "Self", "String", "Option", "Vec", "Box", "Result", "HashMap", "Send", "Sync", "Sized",
        "Drop", "FnOnce", "Some", "None", "Ok", "Err", "Visitor", "Dict",
    ];
    let mut schema = String::new();
    for name in names {
        schema.push_str(&format!(
            "{{ 'struct':'{name}', 'data':{{'value':'str'}} }}\n"
        ));
    }
    schema.push_str("{ 'enum':'Kind', 'data':['self','string','some','none','ok','err','option','vec','box','result','hash-map'] }\n");
    schema.push_str("{ 'struct':'Holder', 'data':{");
    for name in names {
        schema.push_str(&format!("'{}':'{name}',", name.to_ascii_lowercase()));
    }
    schema.push_str("'crate':'str','super':'str','type':'str','async':'str','kind':'Kind'} }\n");
    schema.push_str("{ 'command':'self', 'data':{'value':'str'}, 'returns':'Self' }\n");
    let entry = repo.join("qapi/qapi-schema.json");
    std::fs::write(&entry, &schema).unwrap();
    let oracle = Command::new("python3").arg("-c").arg("import sys; sys.path.insert(0, sys.argv[1]+'/scripts'); from qapi.schema import QAPISchema; QAPISchema(sys.argv[2])")
        .arg(workspace.join("vendor/qemu")).arg(&entry).output().unwrap();
    assert!(
        oracle.status.success(),
        "QEMU rejected fixture: {}",
        String::from_utf8_lossy(&oracle.stderr)
    );
    git(&repo, &["init", "-q"]);
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
            "fixture",
        ],
    );
    git(&repo, &["tag", "v11.1.3"]);
    let output = generate_unified_code(&repo, &["v11.1.3"]).unwrap();
    let fixture = temp.path().join("fixture");
    let generated = fixture.join("src/generated");
    std::fs::create_dir_all(&generated).unwrap();
    for (name, text) in [
        ("mod.rs", output.types),
        ("commands.rs", output.commands),
        ("events.rs", output.events),
        ("metadata.rs", output.metadata),
    ] {
        std::fs::write(generated.join(name), text).unwrap();
    }
    let manifest = format!(
        r#"[package]
name = "qapi-generated-identifiers"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
qapi_runtime = {{ package = "qapi", path = {:?} }}
qapi-macros = {{ path = {:?} }}
serde = "1"
serde_json = "1"
phf = {{ version = "0.11", features = ["macros"] }}
"#,
        workspace.join("projects/qapi"),
        workspace.join("projects/qapi-macros")
    );
    std::fs::write(fixture.join("Cargo.toml"), manifest).unwrap();
    let main = r#"
pub use qapi_runtime::{QapiCommand, QapiEvent, QapiFlatUnion, PreconfigSafe, qemu_version, scoped_qemu_version, with_version, unavailable_type_message};
mod generated;
pub use generated::*;
fn main() {
    with_version((11,1,3), || {
        let command = commands::QapiSelfCommand { value: "command".into() };
        assert_eq!(<commands::QapiSelfCommand as QapiCommand>::NAME, "self");
        assert_eq!(serde_json::to_value(command).unwrap(), serde_json::json!({"value":"command"}));
        let _: Option<QapiSelf> = None::<<commands::QapiSelfCommand as QapiCommand>::Returns>;
        let mut wire = serde_json::Map::new();
        for name in ["self","string","option","vec","box","result","hashmap","send","sync","sized","drop","fnonce","some","none","ok","err","visitor","dict"] {
            wire.insert(name.into(), serde_json::json!({"value":name}));
        }
        for name in ["crate","super","type","async"] { wire.insert(name.into(), serde_json::json!(name)); }
        wire.insert("kind".into(), serde_json::json!("string"));
        let wire = serde_json::Value::Object(wire);
        let value: Holder = serde_json::from_value(wire.clone()).unwrap();
        assert!(matches!(value.kind, Kind::String));
        assert_eq!(value.self_.value, "self");
        assert_eq!(value.crate_, "crate");
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
        let field = metadata::STRUCTS.get("Holder").unwrap().fields.iter().find(|field| field.name == "self_").unwrap();
        assert_eq!(field.wire_name, "self");
    });
}
"#;
    std::fs::write(fixture.join("src/main.rs"), main).unwrap();
    let result = Command::new(env!("CARGO"))
        .args(["run", "--offline", "--quiet", "--manifest-path"])
        .arg(fixture.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(workspace.join("target/qapi-identifier-fixtures"))
        .env_remove("QEMU_MIN_VERSION")
        .env_remove("QEMU_MAX_VERSION")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "Generated fixture failed:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

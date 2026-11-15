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
    schema.push_str("{ 'struct':'PeerWindows', 'if':'CONFIG_WIN32', 'data':{'value':'str'} }\n");
    schema.push_str("{ 'struct':'PeerFields', 'data':{'windows':{'type':'str','if':'CONFIG_WIN32'},'posix':{'type':'str','if':'CONFIG_POSIX'}} }\n");
    schema.push_str("{ 'enum':'PeerKind', 'data':[{'name':'windows','if':'CONFIG_WIN32'},{'name':'posix','if':'CONFIG_POSIX'}] }\n");
    schema.push_str("{ 'command':'peer-windows', 'if':'CONFIG_WIN32', 'returns':'PeerWindows' }\n");
    schema.push_str("{ 'pragma':{'command-returns-exceptions':['read-string','read-strings']} }\n");
    schema.push_str("{ 'command':'read-string', 'returns':'str' }\n");
    schema.push_str("{ 'command':'read-strings', 'returns':['str'] }\n");
    schema.push_str(
        "{ 'alternate':'PeerPayload', 'data':{'text':'str','object':'String','nothing':'null'} }\n",
    );
    schema.push_str(
        "{ 'alternate':'PeerNumber', 'data':{'number':'int','kind':'Kind','nothing':'null'} }\n",
    );
    schema.push_str("{ 'enum':'EmptyKind', 'data':['none'] }\n");
    schema.push_str("{ 'struct':'EmptyBase', 'data':{'kind':'EmptyKind'} }\n");
    schema.push_str(
        "{ 'union':'EmptyUnion', 'base':'EmptyBase', 'discriminator':'kind', 'data':{} }\n",
    );
    let entry = repo.join("qapi/qapi-schema.json");
    std::fs::write(&entry, &schema).unwrap();
    let oracle = Command::new("python3").arg("-c").arg("import sys; sys.path.insert(0, sys.argv[1]+'/scripts'); from qapi.schema import QAPISchema; QAPISchema(sys.argv[2])")
        .arg(workspace.join("vendor/qemu")).arg(&entry).output().unwrap();
    assert!(
        oracle.status.success(),
        "QEMU rejected fixture: {}",
        String::from_utf8_lossy(&oracle.stderr)
    );
    let modern_schema = schema.clone();
    // Modern QEMU no longer accepts simple unions. Keep a historical array
    // payload in the compiled fixture after validating the modern definitions.
    schema.push_str("{ 'union':'ArrayPayload', 'data':{'strings':['str'],'objects':['String'],'kinds':['Kind']} }\n");
    schema.push_str("{ 'struct':'OpaqueReferences', 'data':{'visitor':'visitor','dict':'dict','wildcard':'**'} }\n");
    std::fs::write(&entry, &schema).unwrap();
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
    git(&repo, &["tag", "v6.1.0"]);
    std::fs::write(&entry, modern_schema).unwrap();
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
            "remove historical union",
        ],
    );
    git(&repo, &["tag", "v11.1.3"]);
    let output = generate_unified_code(&repo, &["v6.1.0", "v11.1.3"]).unwrap();
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
        let empty: EmptyUnion = serde_json::from_value(serde_json::json!({"kind":"none"})).unwrap();
        assert_eq!(serde_json::to_value(empty).unwrap(), serde_json::json!({"kind":"none"}));
        assert_eq!(metadata::ENUMS.get("EmptyUnionBranch").unwrap().kind, metadata::EnumKind::FlatUnionBranch);
        let _: Option<<commands::PeerWindowsCommand as QapiCommand>::Returns> = Some(PeerWindows { value: "remote".into() });
        for wire in [serde_json::json!({}), serde_json::json!({"windows":"remote"}), serde_json::json!({"posix":"remote"})] {
            let value: PeerFields = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
        }
        for wire in ["windows", "posix"] {
            let value: PeerKind = serde_json::from_value(serde_json::json!(wire)).unwrap();
            assert_eq!(serde_json::to_value(value).unwrap(), serde_json::json!(wire));
        }
        // Schema optionality still describes the member when its peer condition holds.
        assert!(metadata::STRUCTS.get("PeerFields").unwrap().fields.iter().all(|field| !field.optional));
        with_version((6,1,0), || {
        let wire = serde_json::json!({"visitor":[1,2],"dict":false,"wildcard":null});
        let opaque: OpaqueReferences = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(opaque).unwrap(), wire);
        for field in metadata::STRUCTS.get("OpaqueReferences").unwrap().fields {
            let ty = &metadata::TYPE_REFS[field.ty as usize];
            assert_eq!(ty.kind, metadata::TypeKind::Primitive);
            assert_eq!(ty.prim, metadata::Prim::Any);
        }
        for wire in [serde_json::json!({"type":"strings","data":["a"]}), serde_json::json!({"type":"objects","data":[{"value":"a"}]}), serde_json::json!({"type":"kinds","data":["string"]})] {
            let value: ArrayPayload = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
        }
        });
        for variant in metadata::ENUMS.get("ArrayPayloadBranch").unwrap().variants {
            assert_eq!(variant.payload_struct, None);
            let list = &metadata::TYPE_REFS[variant.payload_type.unwrap() as usize];
            assert_eq!(list.kind, metadata::TypeKind::List);
            let element = &metadata::TYPE_REFS[list.inner as usize];
            match variant.wire_name {
                "strings" => assert_eq!(element.prim, metadata::Prim::Str),
                "objects" => {assert_eq!(element.kind, metadata::TypeKind::Struct); assert_eq!(element.name, "QapiString");},
                "kinds" => {assert_eq!(element.kind, metadata::TypeKind::Enum); assert_eq!(element.name, "Kind");},
                _ => unreachable!(),
            }
        }
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
        let field = metadata::STRUCTS.get("Holder").unwrap().fields.iter().find(|field| field.name == "string").unwrap();
        let ty = &metadata::TYPE_REFS[field.ty as usize];
        assert_eq!(ty.kind, metadata::TypeKind::Struct);
        assert_eq!(ty.name, "QapiString");
        let command = metadata::COMMANDS.iter().find(|command| command.wire_name == "read-string").unwrap();
        let ty = &metadata::TYPE_REFS[command.returns.unwrap() as usize];
        assert_eq!(ty.kind, metadata::TypeKind::Primitive);
        assert_eq!(ty.prim, metadata::Prim::Str);
        let command = metadata::COMMANDS.iter().find(|command| command.wire_name == "read-strings").unwrap();
        let ty = &metadata::TYPE_REFS[command.returns.unwrap() as usize];
        assert_eq!(ty.kind, metadata::TypeKind::List);
        assert_eq!(metadata::TYPE_REFS[ty.inner as usize].prim, metadata::Prim::Str);
        for variant in metadata::ENUMS.get("PeerPayload").unwrap().variants.iter().chain(metadata::ENUMS.get("PeerNumber").unwrap().variants) {
            let ty = &metadata::TYPE_REFS[variant.payload_type.unwrap() as usize];
            match variant.wire_name {
                "object" => {assert_eq!(ty.kind, metadata::TypeKind::Struct); assert_eq!(variant.payload_struct, Some("QapiString"));},
                "text" => assert_eq!(ty.prim, metadata::Prim::Str),
                "kind" => {assert_eq!(ty.kind, metadata::TypeKind::Enum); assert_eq!(ty.name, "Kind"); assert_eq!(variant.payload_struct, None);},
                "number" => assert_eq!(ty.prim, metadata::Prim::I64),
                "nothing" => assert_eq!(ty.prim, metadata::Prim::Null),
                _ => unreachable!(),
            }
        }
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

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
    let nested_schema = "{ 'struct':'NestedInline', 'data':{'outer':{'inner':{'value':'str','*count':'int'},'*empty':{}},'*choice':{'flag':'bool'}} }\n{ 'struct':'NestedInlineOuterInner', 'data':{'declared':'bool'} }\n";
    std::fs::write(&entry, nested_schema).unwrap();
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
    git(&repo, &["tag", "v2.3.0"]);
    std::fs::write(&entry, &schema).unwrap();
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
            "historical union",
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
    let output = generate_unified_code(&repo, &["v2.3.0", "v6.1.0", "v11.1.3"]).unwrap();
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
    let main = include_str!("fixtures/compiler/main.rs");
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

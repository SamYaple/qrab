//! Compile generated Rust for names accepted by QEMU but special in Rust.
mod common;
use common::{git, run_fixture, write_module};
use qapi_codegen::unified::generate_unified_code;
use std::{path::Path, process::Command};

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
    schema.push_str("{ 'pragma':{'command-returns-exceptions':['read-string','read-strings','query-changing-direct','query-changing-array']} }\n");
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
    schema.push_str("{ 'struct':'CollisionBranch', 'data':{'marker':'str'} }\n");
    schema.push_str("{ 'struct':'CollisionBranchGenerated', 'data':{'marker':'int'} }\n");
    schema.push_str("{ 'struct':'CollisionBase', 'data':{'kind':'EmptyKind'} }\n");
    schema.push_str("{ 'union':'Collision', 'base':'CollisionBase', 'discriminator':'kind', 'data':{'none':'PeerWindows'} }\n");
    schema.push_str("{ 'struct':'SignalEvent', 'data':{'marker':'str'} }\n");
    schema.push_str("{ 'struct':'SignalEventGenerated', 'data':{'marker':'int'} }\n");
    schema.push_str("{ 'event':'SIGNAL', 'data':{'value':'str'} }\n");
    schema.push_str("{ 'struct':'Foo', 'data':{'marker':'str'} }\n");
    schema.push_str("{ 'struct':'FooCommand', 'data':{'marker':'bool'} }\n");
    schema.push_str("{ 'command':'foo', 'data':{'value':'str'}, 'returns':'Foo' }\n");
    schema.push_str("{ 'struct':'ArrayPayloadType', 'data':{'marker':'bool'} }\n");
    schema.push_str("{ 'union':'VersionedUnion', 'base':'CollisionBase', 'discriminator':'kind', 'data':{'none':'PeerWindows'} }\n");
    let modern_only = "{ 'struct':'VersionedUnionBranch', 'data':{'marker':'bool'} }\n{ 'struct':'NestedInlineOuterEmpty', 'data':{'marker':'bool'} }\n";
    schema.push_str(modern_only);
    let changing_type = "{ 'struct':'ChangingType', 'data':{'value':'str'} }\n";
    schema.push_str(changing_type);
    schema.push_str("{ 'struct':'ChangingTypeLegacy', 'data':{'marker':'bool'} }\n");
    schema.push_str("{ 'command':'query-changing-direct', 'returns':'ChangingType' }\n");
    schema.push_str("{ 'command':'query-changing-array', 'returns':['ChangingType'] }\n");
    let cycle_type = "{ 'enum':'CycleShape', 'data':['new'] }\n";
    schema.push_str(cycle_type);
    schema.push_str("{ 'struct':'CycleHolder', 'data':{'item':'CycleShape'} }\n");
    schema.push_str("{ 'struct':'ChangingHolder', 'data':{'item':'ChangingType'} }\n");
    schema.push_str("{ 'command':'query-changing', 'returns':'ChangingHolder' }\n");
    let changing_alt = "{ 'alternate':'ChangingAlt', 'data':{'text':'str','object':'String'} }\n";
    schema.push_str(changing_alt);
    schema.push_str("{ 'struct':'AltHolder', 'data':{'item':'ChangingAlt'} }\n");
    schema.push_str("{ 'pragma':{'member-name-exceptions':['CollisionFields','VirtualBase']} }\n");
    let collision_fields = "{ 'struct':'CollisionFields', 'data':{'FooBAR':'str','FooBar':'str','foo_bar_alt':'str','FooBAR_alt':'str'} }\n";
    schema.push_str(collision_fields);
    schema.push_str("{ 'command':'manual-arguments', 'gen':false, 'data':{'arguments':'str'} }\n");
    schema.push_str("{ 'struct':'VirtualBase', 'data':{'kind':'EmptyKind','U':'str'} }\n");
    schema.push_str("{ 'union':'VirtualOwner', 'base':'VirtualBase', 'discriminator':'kind', 'data':{'none':'PeerWindows'} }\n");
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
    schema = schema.replace(modern_only, "");
    schema = schema.replace(collision_fields, "{ 'struct':'CollisionFields', 'data':{'FooBAR':'str','FooBar':'str','FooBAR_alt':'str'} }\n");
    schema = schema.replace(changing_alt, "{ 'enum':'ChangingAlt', 'data':['old'] }\n");
    schema = schema.replace(changing_type, "{ 'enum':'ChangingType', 'data':['old'] }\n");
    schema = schema.replace(
        cycle_type,
        "{ 'struct':'CycleShape', 'data':{'value':'str'} }\n",
    );
    // Modern QEMU no longer accepts simple unions. Keep a historical array
    // payload in the compiled fixture after validating the modern definitions.
    schema.push_str("{ 'union':'ArrayPayload', 'data':{'strings':['str'],'objects':['String'],'kinds':['Kind']} }\n");
    schema.push_str("{ 'struct':'OpaqueReferences', 'data':{'visitor':'visitor','dict':'dict','wildcard':'**'} }\n");
    schema.push_str("{ 'union':'HistoricalChangingPayload', 'data':{'item':'ChangingType'} }\n");
    let nested_schema = "{ 'struct':'NestedInline', 'data':{'outer':{'inner':{'value':'str','*count':'int'},'*empty':{}},'*choice':{'flag':'bool'}} }\n{ 'struct':'NestedInlineOuterInner', 'data':{'declared':'bool'} }\n{ 'enum':'CycleShape', 'data':['old'] }\n{ 'struct':'CycleHolder', 'data':{'item':'CycleShape'} }\n";
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
    qapi_codegen::set_parser_version((1, 0, 0));
    let output = generate_unified_code(&repo, &["v2.3.0", "v6.1.0", "v11.1.3"]).unwrap();
    assert_eq!(qapi_codegen::parser_version(), (1, 0, 0));
    let fixture = temp.path().join("fixture");
    write_module(&fixture.join("src/generated"), output);
    run_fixture(
        &workspace,
        &fixture,
        include_str!("fixtures/compiler/main.rs"),
    );
    common::run_fixture_with_bounds(
        &workspace,
        &fixture,
        include_str!("fixtures/compiler/old.rs"),
        Some(("6.1.0", "6.1.0")),
    );
}

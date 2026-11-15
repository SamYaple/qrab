//! Compile a command whose execution flags have two separate lifetimes.
mod common;
use common::{git, run_fixture, write_module};
use std::path::Path;

#[test]
fn generated_command_flags_preserve_a_removed_interval() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("schema");
    std::fs::create_dir_all(repo.join("qapi")).unwrap();
    git(&repo, &["init", "-q"]);
    for (tag, schema) in [
        (
            "v6.1.0",
            "{'command':'flip','allow-oob':true,'allow-preconfig':true,'success-response':false}",
        ),
        ("v8.0.0", "{'command':'flip'}"),
        (
            "v11.1.3",
            "{'command':'flip','allow-oob':true,'allow-preconfig':true,'success-response':false}",
        ),
    ] {
        std::fs::write(repo.join("qapi/qapi-schema.json"), schema).unwrap();
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
                tag,
            ],
        );
        git(&repo, &["tag", tag]);
    }
    let output =
        qapi_codegen::unified::generate_unified_code(&repo, &["v6.1.0", "v8.0.0", "v11.1.3"])
            .unwrap();
    let fixture = temp.path().join("fixture");
    write_module(&fixture.join("src/generated"), output);
    run_fixture(
        &workspace,
        &fixture,
        r#"
pub use qapi_runtime::{PreconfigSafe, QapiCommand, QapiEvent, QapiFlatUnion, qemu_version, scoped_qemu_version, unavailable_type_message, with_version};
mod generated;
pub use generated::*;
fn main() {
    for (version, enabled) in [((6,1,0),true), ((8,0,0),false), ((11,1,3),true)] {
        assert_eq!(commands::Flip::allows_oob(version), enabled);
        assert_eq!(commands::Flip::allows_preconfig(version), enabled);
        assert_eq!(commands::Flip::sends_success_response(version), !enabled);
    }
    assert_eq!(commands::Flip::PRECONFIG_SINCE, Some((6,1,0)));
    assert!(commands::Flip::ALLOW_OOB && commands::Flip::ALLOW_PRECONFIG);
    assert!(!commands::Flip::SUCCESS_RESPONSE);
    let command = &metadata::COMMANDS[0];
    assert!(command.allow_oob && command.allow_preconfig && !command.success_response);
}
"#,
    );
}

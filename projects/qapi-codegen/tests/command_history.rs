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
        let feature = if tag == "v8.0.0" {
            "[]"
        } else {
            "['unstable']"
        };
        let initial = if tag == "v6.1.0" {
            "['unstable']"
        } else {
            "[]"
        };
        let surface = if tag == "v8.0.0" {
            ""
        } else {
            "{'command':'foo','data':'Foo','returns':'Foo'}\n{'command':'ping'}\n{'event':'FOO','data':'Foo'}"
        };
        let schema = format!(
            "{schema}\n{{'command':'feature-flip','features':{feature}}}\n{{'command':'stabilized','features':{initial}}}\n{{'struct':'Foo','data':{{'value':'str'}}}}\n{surface}"
        );
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
    with_version((6,1,0), || {
        let command = commands::FooLegacy { value:"old".into() };
        assert_eq!(commands::FooLegacy::NAME, "foo");
        assert_eq!(commands::FooLegacy::REMOVED_IN, Some((8,0,0)));
        assert_eq!(serde_json::to_value(command).unwrap(),serde_json::json!({"value":"old"}));
        assert_eq!(commands::PingLegacy::NAME,"ping");
    });
    with_version((11,1,3), || {
        let command = commands::FooCommand { value:"new".into() };
        assert_eq!(commands::FooCommand::NAME,"foo");
        assert_eq!(commands::FooCommand::AVAILABLE_SINCE,(11,1,3));
        assert_eq!(serde_json::to_value(command).unwrap(),serde_json::json!({"value":"new"}));
        assert_eq!(commands::Ping::NAME,"ping");
    });
    with_version((8,0,0), || {
        assert!(serde_json::to_value(Foo {value:"data remains valid".into()}).is_ok());
        assert!(serde_json::to_value(commands::FooLegacy {value:"old".into()}).is_err());
        assert!(serde_json::to_value(commands::FooCommand {value:"new".into()}).is_err());
        assert!(serde_json::to_value(events::FooEventLegacy {value:"old".into()}).is_err());
        assert!(serde_json::to_value(events::FooEvent {value:"new".into()}).is_err());
        assert!(serde_json::from_value::<events::FooEvent>(serde_json::json!({"value":"new"})).is_err());
        assert!(serde_json::from_value::<events::FooEventLegacy>(serde_json::json!({"value":"old"})).is_err());

    });
    for (version, enabled) in [((6,1,0),true), ((8,0,0),false), ((11,1,3),true)] {
        assert_eq!(commands::Flip::allows_oob(version), enabled);
        assert_eq!(commands::Flip::allows_preconfig(version), enabled);
        assert_eq!(commands::Flip::sends_success_response(version), !enabled);
        assert_eq!(commands::FeatureFlip::is_unstable(version), enabled);
        assert_eq!(commands::Stabilized::is_unstable(version), version == (6,1,0));
    }
    assert_eq!(commands::Flip::PRECONFIG_SINCE, Some((6,1,0)));
    assert!(commands::Flip::ALLOW_OOB && commands::Flip::ALLOW_PRECONFIG);
    assert!(!commands::Flip::SUCCESS_RESPONSE);
    assert!(commands::FeatureFlip::UNSTABLE);
    assert!(!commands::Stabilized::UNSTABLE);
    let command = metadata::COMMANDS.iter().find(|command| command.wire_name == "flip").unwrap();
    assert!(command.allow_oob && command.allow_preconfig && !command.success_response);
    assert!(metadata::COMMANDS.iter().find(|command| command.wire_name == "feature-flip").unwrap().unstable);
    assert!(!metadata::COMMANDS.iter().find(|command| command.wire_name == "stabilized").unwrap().unstable);
}
"#,
    );
}

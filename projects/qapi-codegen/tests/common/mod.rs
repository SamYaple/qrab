use std::{path::Path, process::Command};

pub fn git(repo: &Path, args: &[&str]) {
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

pub fn write_module(path: &Path, output: qapi_codegen::unified::GeneratedOutput) {
    std::fs::create_dir_all(path).unwrap();
    for (name, text) in [
        ("mod.rs", output.types),
        ("commands.rs", output.commands),
        ("events.rs", output.events),
        ("metadata.rs", output.metadata),
    ] {
        std::fs::write(path.join(name), text).unwrap();
    }
}

pub fn run_fixture(workspace: &Path, fixture: &Path, main: &str) {
    run_fixture_with_bounds(workspace, fixture, main, None)
}

pub fn run_fixture_with_bounds(
    workspace: &Path,
    fixture: &Path,
    main: &str,
    bounds: Option<(&str, &str)>,
) {
    let manifest = format!(
        r#"[package]
name = "qapi-generated-fixture"
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
    std::fs::write(fixture.join("src/main.rs"), main).unwrap();
    let mut command = Command::new(env!("CARGO"));
    let target = workspace.join("target/qapi-codegen-fixtures");
    let target = bounds.map_or(target.clone(), |(min, max)| {
        target.join(format!("{min}-{max}"))
    });
    command
        .args(["run", "--offline", "--quiet", "--manifest-path"])
        .arg(fixture.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(target)
        .env_remove("QEMU_MIN_VERSION")
        .env_remove("QEMU_MAX_VERSION");
    if let Some((min, max)) = bounds {
        command
            .env("QEMU_MIN_VERSION", min)
            .env("QEMU_MAX_VERSION", max);
    }
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "Generated fixture failed:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

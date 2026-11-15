//! Compile the complete generated API for QEMU's positive schema fixtures.
mod common;
use common::{git, run_fixture, write_module};
use qapi_codegen::unified::generate_unified_code;
use std::path::Path;

fn copy_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn upstream_positive_schema_fixtures_generate_compilable_rust() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let source = workspace.join("vendor/qemu/tests/qapi-schema");
    let mut fixtures = Vec::new();
    for entry in std::fs::read_dir(&source).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|extension| extension == "err")
            && std::fs::read_to_string(&path).unwrap().trim().is_empty()
            && path.with_extension("json").exists()
        {
            fixtures.push(path.with_extension("json").file_name().unwrap().to_owned());
        }
    }
    fixtures.sort();
    assert!(!fixtures.is_empty());
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("schema");
    copy_tree(&source, &repo.join("qapi"));
    git(&repo, &["init", "-q"]);
    let fixture = temp.path().join("fixture");
    let mut main = String::from(
        "pub use qapi_runtime::{QapiCommand, QapiEvent, QapiFlatUnion, PreconfigSafe, qemu_version, scoped_qemu_version, with_version, unavailable_type_message};\nfn main() {}\n",
    );
    for (index, name) in fixtures.iter().enumerate() {
        let schema = format!("{{'include':'{}'}}\n", name.to_str().unwrap());
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
                "fixture",
            ],
        );
        git(&repo, &["tag", "-f", "v11.1.3"]);
        let output = generate_unified_code(&repo, &["v11.1.3"])
            .unwrap_or_else(|error| panic!("{}: {error:#}", name.display()));
        let module = format!("corpus_{index}");
        write_module(&fixture.join("src").join(&module), output);
        main.push_str(&format!("mod {module}; // {}\n", name.display()));
    }
    run_fixture(&workspace, &fixture, &main);
}

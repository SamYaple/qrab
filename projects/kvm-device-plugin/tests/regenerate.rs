//! Drift check for the committed gRPC code: regenerates `src/v1beta1.rs` from
//! `proto/deviceplugin-v1beta1.proto` with the pinned tonic-prost-build in a
//! throwaway Cargo project and requires a byte-identical result.
use std::{path::Path, process::Command};

const GENERATOR: &str = r#"use std::path::PathBuf;
fn main() {
    let a: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let include = a[0].parent().unwrap().to_path_buf();
    tonic_prost_build::configure()
        .out_dir(&a[1])
        .compile_protos(&[a[0].clone()], &[include])
        .unwrap();
}
"#;

#[test]
#[ignore = "builds tonic-prost-build 0.14.6 (crates.io or a warm cache) and needs protoc"]
fn committed_grpc_code_matches_a_fresh_generation() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let work = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(work.path().join("src")).unwrap();
    std::fs::write(
        work.path().join("Cargo.toml"),
        "[package]\nname = \"regen\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\
         [dependencies]\ntonic-prost-build = \"=0.14.6\"\n[workspace]\n",
    )
    .unwrap();
    std::fs::write(work.path().join("src/main.rs"), GENERATOR).unwrap();
    let out = work.path().join("out");
    std::fs::create_dir(&out).unwrap();
    let status = Command::new(env!("CARGO"))
        .args(["run", "--quiet", "--manifest-path"])
        .arg(work.path().join("Cargo.toml"))
        .arg("--")
        .arg(crate_dir.join("proto/deviceplugin-v1beta1.proto"))
        .arg(&out)
        .env("CARGO_TARGET_DIR", work.path().join("target"))
        .status()
        .unwrap();
    assert!(status.success(), "generator failed");
    let fresh = std::fs::read_to_string(out.join("v1beta1.rs")).unwrap();
    let committed = std::fs::read_to_string(crate_dir.join("src/v1beta1.rs")).unwrap();
    assert!(
        fresh == committed,
        "src/v1beta1.rs drifted from the proto; copy the regenerated file from this test"
    );
}

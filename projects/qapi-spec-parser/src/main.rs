use anyhow::Result;
use qapi_spec_parser::{generate_qapi_code, versions::find_schema_entry};
use std::path::Path;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage:");
        eprintln!("  qapi-spec-parser <qemu-src-root> [schema-path]");
        eprintln!("  qapi-spec-parser unified --repo <qemu-git-repo> --versions v1.1.0,v2.0.0,...");
        std::process::exit(1);
    }

    if args[1] == "unified" {
        return run_unified(&args[2..]);
    }

    let qemu_src_root = Path::new(&args[1]);
    let schema_file = if args.len() >= 3 {
        qemu_src_root.join(&args[2])
    } else {
        find_schema_entry(qemu_src_root)
            .expect("Could not find QAPI schema entry point")
    };

    let output = generate_qapi_code(qemu_src_root, &schema_file)?;
    print!("{}", output);
    Ok(())
}

fn run_unified(args: &[String]) -> Result<()> {
    let mut repo = None;
    let mut versions = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--repo" => {
                i += 1;
                repo = Some(args[i].clone());
            }
            "--versions" => {
                i += 1;
                versions = Some(args[i].clone());
            }
            other => anyhow::bail!("Unknown argument: {}", other),
        }
        i += 1;
    }

    let repo = repo.ok_or_else(|| anyhow::anyhow!("--repo is required"))?;
    let versions_str = versions.ok_or_else(|| anyhow::anyhow!("--versions is required"))?;
    let version_tags: Vec<&str> = versions_str.split(',').collect();

    let code = qapi_spec_parser::unified::generate_unified_code(
        Path::new(&repo),
        &version_tags,
    )?;
    print!("{}", code);
    Ok(())
}

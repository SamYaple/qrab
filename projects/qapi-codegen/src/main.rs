use anyhow::{Context, Result};
use qapi_codegen::versions::discover_version_tags;
use std::path::Path;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let repo = if args.len() >= 2 && !args[1].starts_with("--") {
        args[1].clone()
    } else {
        "vendor/qemu".to_string()
    };

    let qga_mode = args.iter().any(|a| a == "--qga");

    // Find --output-dir argument
    let output_dir = args
        .windows(2)
        .find(|w| w[0] == "--output-dir")
        .map(|w| w[1].clone());

    let repo_path = Path::new(&repo);
    anyhow::ensure!(
        repo_path.join(".git").exists() || repo_path.join("HEAD").exists(),
        "Not a git repository: {}. Usage: qapi-codegen [qemu-git-repo] [--qga] --output-dir <dir>",
        repo
    );

    let tags = discover_version_tags(repo_path)?;
    anyhow::ensure!(
        !tags.is_empty(),
        "No QEMU version tags found in {}",
        repo
    );
    eprintln!(
        "Discovered {} version tags ({} to {})",
        tags.len(),
        tags.first().unwrap(),
        tags.last().unwrap(),
    );

    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

    let output = if qga_mode {
        qapi_codegen::unified::generate_qga_code(repo_path, &tag_refs)?
    } else {
        qapi_codegen::unified::generate_unified_code(repo_path, &tag_refs)?
    };

    if let Some(dir) = output_dir {
        let dir_path = Path::new(&dir);
        std::fs::create_dir_all(dir_path)
            .with_context(|| format!("Failed to create output directory: {}", dir))?;

        std::fs::write(dir_path.join("mod.rs"), &output.types)
            .with_context(|| format!("Failed to write {}/mod.rs", dir))?;
        std::fs::write(dir_path.join("commands.rs"), &output.commands)
            .with_context(|| format!("Failed to write {}/commands.rs", dir))?;
        std::fs::write(dir_path.join("events.rs"), &output.events)
            .with_context(|| format!("Failed to write {}/events.rs", dir))?;
        std::fs::write(dir_path.join("metadata.rs"), &output.metadata)
            .with_context(|| format!("Failed to write {}/metadata.rs", dir))?;

        eprintln!("Wrote {}/mod.rs, commands.rs, events.rs, metadata.rs", dir);
    } else {
        // Legacy mode: print all three parts concatenated (for backward compat with tests)
        print!("{}", output.types);
        print!("{}", output.commands);
        print!("{}", output.events);
    }

    Ok(())
}

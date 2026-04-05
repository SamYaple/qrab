use anyhow::Result;
use qapi_codegen::versions::discover_version_tags;
use std::path::Path;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let repo = if args.len() >= 2 {
        args[1].clone()
    } else {
        "vendor/qemu".to_string()
    };

    let repo_path = Path::new(&repo);
    anyhow::ensure!(
        repo_path.join(".git").exists() || repo_path.join("HEAD").exists(),
        "Not a git repository: {}. Usage: qapi-codegen [qemu-git-repo]",
        repo
    );

    let tags = discover_version_tags(repo_path)?;
    eprintln!("Discovered {} version tags ({} to {})",
        tags.len(),
        tags.first().unwrap(),
        tags.last().unwrap(),
    );

    let tag_refs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();
    let code = qapi_codegen::unified::generate_unified_code(repo_path, &tag_refs)?;
    print!("{}", code);
    Ok(())
}

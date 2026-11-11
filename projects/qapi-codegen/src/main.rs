use anyhow::{Context, Result};
use qapi_codegen::versions::{PINNED_VERSIONS, parse_pinned_versions, verify_pinned_versions};
use std::path::PathBuf;

const USAGE: &str = "Usage: qapi-codegen [qemu-git-repo] [--qga] --output-dir <dir> [--versions-file <file>]\n\nDefaults to vendor/qemu and the embedded qemu-versions.lock release pins.\nFetch release tags from the local Gitea QEMU mirror before generation. --versions-file selects an explicit\nalternate set of tag/commit pairs. No output is written until all inputs parse.";

#[derive(Debug, PartialEq, Eq)]
struct Options {
    repo: PathBuf,
    output_dir: PathBuf,
    versions_file: Option<PathBuf>,
    qga: bool,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<Options>> {
    let mut args = args.into_iter();
    let mut repo = None;
    let mut output_dir = None;
    let mut versions_file = None;
    let mut qga = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => return Ok(None),
            "--qga" => qga = true,
            "--output-dir" | "--versions-file" => {
                let value = args
                    .next()
                    .filter(|v| !v.starts_with('-'))
                    .with_context(|| format!("{arg} requires a path"))?;
                let slot = if arg == "--output-dir" {
                    &mut output_dir
                } else {
                    &mut versions_file
                };
                anyhow::ensure!(slot.is_none(), "Repeated argument: {arg}");
                *slot = Some(PathBuf::from(value));
            }
            _ => {
                anyhow::ensure!(!arg.starts_with('-'), "Unknown argument: {arg}");
                anyhow::ensure!(repo.is_none(), "Unexpected argument: {arg}");
                repo = Some(PathBuf::from(arg));
            }
        }
    }
    Ok(Some(Options {
        repo: repo.unwrap_or_else(|| "vendor/qemu".into()),
        output_dir: output_dir.context("--output-dir is required")?,
        versions_file,
        qga,
    }))
}

fn main() -> Result<()> {
    let Some(options) = parse_args(std::env::args().skip(1)).context(USAGE)? else {
        println!("{USAGE}");
        return Ok(());
    };
    let pins = match &options.versions_file {
        Some(path) => std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?,
        None => PINNED_VERSIONS.to_owned(),
    };
    let versions = parse_pinned_versions(&pins)?;
    verify_pinned_versions(&options.repo, &versions)?;
    let tags: Vec<_> = versions.iter().map(|v| v.tag.as_str()).collect();
    eprintln!(
        "Generating from {} pinned releases ({} to {})",
        tags.len(),
        tags[0],
        tags.last().unwrap()
    );
    let output = if options.qga {
        qapi_codegen::unified::generate_qga_code(&options.repo, &tags)?
    } else {
        qapi_codegen::unified::generate_unified_code(&options.repo, &tags)?
    };
    std::fs::create_dir_all(&options.output_dir)?;
    for (name, content) in [
        ("mod.rs", output.types),
        ("commands.rs", output.commands),
        ("events.rs", output.events),
        ("metadata.rs", output.metadata),
    ] {
        let path = options.output_dir.join(name);
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(input: &[&str]) -> Result<Option<Options>> {
        parse_args(input.iter().map(|s| s.to_string()))
    }

    #[test]
    fn rejects_incomplete_or_unknown_arguments() {
        for input in [
            vec![],
            vec!["--output-dir"],
            vec!["--unknown"],
            vec!["repo", "other"],
            vec!["--output-dir", "--qga"],
        ] {
            assert!(args(&input).is_err(), "{input:?}");
        }
    }

    #[test]
    fn accepts_options_before_repo() {
        let options = args(&[
            "--qga",
            "--output-dir",
            "out",
            "qemu",
            "--versions-file",
            "pins",
        ])
        .unwrap()
        .unwrap();
        assert_eq!(
            options,
            Options {
                repo: "qemu".into(),
                output_dir: "out".into(),
                versions_file: Some("pins".into()),
                qga: true,
            }
        );
        assert!(args(&["--help"]).unwrap().is_none());
    }
}

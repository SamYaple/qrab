use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Given a QEMU source root, find the schema entry point.
/// Tries `qapi/qapi-schema.json` first (v4.0+), then falls back to root `qapi-schema.json`.
pub fn find_schema_entry(qemu_root: &Path) -> Option<PathBuf> {
    let qapi_dir = qemu_root.join("qapi").join("qapi-schema.json");
    if qapi_dir.exists() {
        return Some(qapi_dir);
    }
    let root_schema = qemu_root.join("qapi-schema.json");
    if root_schema.exists() {
        return Some(root_schema);
    }
    None
}

/// Parse a QEMU version tag like "v1.1.0" into (major, minor, patch).
pub fn parse_version_tag(tag: &str) -> Option<(u32, u32, u32)> {
    let tag = tag.strip_prefix('v').unwrap_or(tag);
    let parts: Vec<&str> = tag.split('.').collect();
    match parts.len() {
        2 => Some((parts[0].parse().ok()?, parts[1].parse().ok()?, 0)),
        3 => Some((
            parts[0].parse().ok()?,
            parts[1].parse().ok()?,
            parts[2].parse().ok()?,
        )),
        _ => None,
    }
}

/// Extract schema files from a git tag into a temporary directory and return the path.
/// The caller is responsible for cleaning up the temp directory.
pub fn extract_schema_from_tag(qemu_repo: &Path, tag: &str) -> Result<tempfile::TempDir> {
    let tmpdir = tempfile::tempdir().context("Failed to create temp directory")?;

    // List all .json files in the tag
    let output = Command::new("git")
        .args(["ls-tree", "-r", "--name-only", tag])
        .current_dir(qemu_repo)
        .output()
        .context("Failed to run git ls-tree")?;

    let file_list = String::from_utf8(output.stdout).context("Invalid UTF-8 in git output")?;

    for path in file_list.lines() {
        if !path.ends_with(".json") {
            continue;
        }
        // Only extract qapi schema files
        if !path.starts_with("qapi/") && !path.starts_with("qapi-schema") {
            continue;
        }

        let file_output = Command::new("git")
            .args(["show", &format!("{}:{}", tag, path)])
            .current_dir(qemu_repo)
            .output()
            .with_context(|| format!("Failed to extract {}:{}", tag, path))?;

        if !file_output.status.success() {
            continue;
        }

        let dest = tmpdir.path().join(path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, &file_output.stdout)?;
    }

    Ok(tmpdir)
}

/// Find the schema entry point in an extracted temp directory.
pub fn find_extracted_schema_entry(dir: &Path) -> Option<PathBuf> {
    // Try qapi/qapi-schema.json first (v2.12.0+)
    let qapi_dir = dir.join("qapi").join("qapi-schema.json");
    if qapi_dir.exists() {
        return Some(qapi_dir);
    }
    // Fall back to root qapi-schema.json (v1.x - v2.11.x)
    let root = dir.join("qapi-schema.json");
    if root.exists() {
        return Some(root);
    }
    None
}

/// Discover all QEMU version tags from v1.0 to v10.2.0 (including patch versions)
/// by querying the git repository.
pub fn discover_version_tags(qemu_repo: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["tag", "-l", "v[0-9]*"])
        .current_dir(qemu_repo)
        .output()
        .context("Failed to run git tag")?;

    anyhow::ensure!(output.status.success(), "git tag failed");

    let tag_list = String::from_utf8(output.stdout).context("Invalid UTF-8 in git output")?;

    let mut tags: Vec<String> = tag_list
        .lines()
        .filter(|tag| {
            // Skip release candidates
            if tag.contains("rc") {
                return false;
            }
            // Parse version and filter to v1.0+
            if let Some((major, _, _)) = parse_version_tag(tag) {
                major >= 1
            } else {
                false
            }
        })
        .map(|s| s.to_string())
        .collect();

    // Sort by version
    tags.sort_by(|a, b| {
        let va = parse_version_tag(a).unwrap_or((0, 0, 0));
        let vb = parse_version_tag(b).unwrap_or((0, 0, 0));
        va.cmp(&vb)
    });

    anyhow::ensure!(!tags.is_empty(), "No version tags found in {}", qemu_repo.display());

    Ok(tags)
}

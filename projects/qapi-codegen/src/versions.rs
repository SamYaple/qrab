use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Audited release inputs used by default, independent of the local tag set.
pub const PINNED_VERSIONS: &str = include_str!("../qemu-versions.lock");

#[derive(Debug, PartialEq, Eq)]
pub struct PinnedVersion {
    pub tag: String,
    pub commit: String,
}

/// Read a release lockfile containing `tag peeled-commit` pairs.
pub fn parse_pinned_versions(input: &str) -> Result<Vec<PinnedVersion>> {
    let mut versions = Vec::new();
    let mut previous = None;
    for (line, value) in input.lines().enumerate() {
        let value = value.split('#').next().unwrap().trim();
        if value.is_empty() {
            continue;
        }
        let fields: Vec<_> = value.split_whitespace().collect();
        anyhow::ensure!(
            fields.len() == 2,
            "Invalid release pin on line {}",
            line + 1
        );
        let version = parse_version_tag(fields[0])
            .with_context(|| format!("Invalid release tag on line {}", line + 1))?;
        anyhow::ensure!(
            fields[0].starts_with('v') && previous.is_none_or(|v| version > v),
            "Release pins must be unique and ordered: {}",
            fields[0]
        );
        anyhow::ensure!(
            fields[1].len() == 40 && fields[1].bytes().all(|b| b.is_ascii_hexdigit()),
            "Invalid commit pin for {}",
            fields[0]
        );
        previous = Some(version);
        versions.push(PinnedVersion {
            tag: fields[0].into(),
            commit: fields[1].to_ascii_lowercase(),
        });
    }
    anyhow::ensure!(!versions.is_empty(), "Release lockfile is empty");
    Ok(versions)
}

/// Check every release tag before generation so missing or moved tags cannot
/// silently change lifecycle annotations or produce an incomplete library.
pub fn verify_pinned_versions(repo: &Path, versions: &[PinnedVersion]) -> Result<()> {
    for pin in versions {
        let output = Command::new("git")
            .args([
                "rev-parse",
                "--verify",
                &format!("refs/tags/{}^{{commit}}", pin.tag),
            ])
            .current_dir(repo)
            .output()
            .context("Failed to run git rev-parse")?;
        anyhow::ensure!(
            output.status.success(),
            "Missing QEMU release {}; fetch upstream tags first",
            pin.tag
        );
        let actual = String::from_utf8(output.stdout)?;
        anyhow::ensure!(
            actual.trim() == pin.commit,
            "QEMU release {} moved: expected {}, found {}",
            pin.tag,
            pin.commit,
            actual.trim()
        );
    }
    Ok(())
}

/// Given a QEMU source root, find the schema entry point.
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

/// Extract schema files from a git tag into a temporary directory.
/// Uses `git archive` for a single subprocess per version.
pub fn extract_schema_from_tag(qemu_repo: &Path, tag: &str) -> Result<tempfile::TempDir> {
    let tmpdir = tempfile::tempdir().context("Failed to create temp directory")?;

    // Use git archive to extract all schema files in one shot.
    // Try: qapi-schema.json + qapi/ + qga/ (covers all versions and QGA).
    // git archive fails if ANY path doesn't exist, so try progressively.
    let paths_to_try: &[&[&str]] = &[
        &["qapi-schema.json", "qapi/", "qga/"], // root schema + qapi dir + qga
        &["qapi-schema.json", "qapi/"],         // root schema + qapi dir (no qga)
        &["qapi/", "qga/"],                     // qapi dir + qga (v2.12.0+)
        &["qapi-schema.json", "qga/"],          // root schema + qga
        &["qapi-schema.json"],                  // root schema only (v1.0-v1.4.0)
        &["qapi/"],                             // qapi dir only
    ];

    let mut tar_data = None;
    for paths in paths_to_try {
        let mut args = vec!["archive".to_string(), tag.to_string(), "--".to_string()];
        args.extend(paths.iter().map(|p| p.to_string()));
        let output = Command::new("git")
            .args(&args)
            .current_dir(qemu_repo)
            .output()
            .context("Failed to run git archive")?;
        if output.status.success() {
            tar_data = Some(output.stdout);
            break;
        }
    }
    let tar_data = tar_data
        .with_context(|| format!("git archive failed for all path combos on tag {}", tag))?;

    // Extract tar to tmpdir, filtering to .json files only
    let mut archive = tar::Archive::new(&tar_data[..]);
    for entry in archive.entries().context("Failed to read tar entries")? {
        let mut entry = entry.context("Failed to read tar entry")?;
        let path = entry
            .path()
            .context("Failed to get entry path")?
            .into_owned();
        let path_str = path.to_string_lossy();

        // Only extract .json files (skip .c, .h, Makefile, etc.)
        if !path_str.ends_with(".json") {
            continue;
        }
        // Only extract QAPI/QGA schema files
        if !path_str.starts_with("qapi") && !path_str.starts_with("qga") {
            continue;
        }

        let dest = tmpdir.path().join(&path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        entry
            .unpack(&dest)
            .with_context(|| format!("Failed to unpack {}", path_str))?;
    }

    Ok(tmpdir)
}

/// Find the schema entry point in an extracted temp directory.
pub fn find_extracted_schema_entry(dir: &Path) -> Option<PathBuf> {
    let qapi_dir = dir.join("qapi").join("qapi-schema.json");
    if qapi_dir.exists() {
        return Some(qapi_dir);
    }
    let root = dir.join("qapi-schema.json");
    if root.exists() {
        return Some(root);
    }
    None
}

/// Find the QGA schema entry point in an extracted temp directory.
pub fn find_extracted_qga_schema_entry(dir: &Path) -> Option<PathBuf> {
    let qga = dir.join("qga").join("qapi-schema.json");
    if qga.exists() {
        return Some(qga);
    }
    None
}

/// Discover all QEMU version tags in a git repository.
/// Returns all release tags (including patch releases), sorted by version.
pub fn discover_version_tags(qemu_repo: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["tag", "--list", "v*"])
        .current_dir(qemu_repo)
        .output()
        .context("Failed to run git tag")?;

    anyhow::ensure!(
        output.status.success(),
        "git tag failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let tags_str = String::from_utf8(output.stdout).context("Invalid UTF-8 in git tag output")?;
    let mut tags: Vec<String> = tags_str
        .lines()
        .filter(|t| {
            let t = t.trim();
            !t.is_empty()
                && !t.contains("rc")
                && parse_version_tag(t).is_some_and(|v| v >= (1, 0, 0))
        })
        .map(|s| s.trim().to_string())
        .collect();

    tags.sort_by_key(|t| parse_version_tag(t).unwrap_or((0, 0, 0)));
    Ok(tags)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_releases_are_ordered_and_end_at_current_baseline() {
        let pins = parse_pinned_versions(PINNED_VERSIONS).unwrap();
        assert_eq!(pins.first().unwrap().tag, "v1.0");
        assert_eq!(pins.last().unwrap().tag, "v11.1.3");
    }

    #[test]
    fn invalid_release_pins_are_rejected() {
        for input in ["", "v11.1.3", "v11.2.0-rc0 deadbeef", "v11.1.3 deadbeef"] {
            assert!(parse_pinned_versions(input).is_err(), "{input}");
        }
        let pin = "v11.1.3 005c61c0ef6f6ae615283440b4bb7be51b2c6ddd\n";
        assert!(parse_pinned_versions(&pin.repeat(2)).is_err());
    }
}

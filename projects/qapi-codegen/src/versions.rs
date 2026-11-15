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
        let version = parse_version_tag_full(fields[0])
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
            "Missing QEMU release {}; fetch tags from the local Gitea QEMU mirror first",
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

/// Parse a stable release tag, retaining its historical fourth component for ordering.
pub fn parse_version_tag_full(tag: &str) -> Option<(u32, u32, u32, u32)> {
    let tag = tag.strip_prefix('v').unwrap_or(tag);
    let parts: Vec<_> = tag.split('.').collect();
    if !(2..=4).contains(&parts.len()) {
        return None;
    }
    let mut values = [0; 4];
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        values[index] = part.parse().ok()?;
    }
    Some((values[0], values[1], values[2], values[3]))
}

/// Parse the three-part version carried by QMP greetings and lifecycle metadata.
pub fn parse_version_tag(tag: &str) -> Option<(u32, u32, u32)> {
    let (major, minor, patch, _) = parse_version_tag_full(tag)?;
    Some((major, minor, patch))
}

/// Extract schema files from a git tag into a temporary directory.
/// Discover existing schema roots, then archive them in one operation.
pub fn extract_schema_from_tag(qemu_repo: &Path, tag: &str) -> Result<tempfile::TempDir> {
    let tmpdir = tempfile::tempdir().context("Failed to create temp directory")?;
    let tree = Command::new("git")
        .args([
            "ls-tree",
            "--name-only",
            "-z",
            tag,
            "--",
            "qapi-schema.json",
            "qapi-schema-guest.json",
            "qapi",
            "qga",
        ])
        .current_dir(qemu_repo)
        .output()
        .context("Failed to inspect schema roots")?;
    anyhow::ensure!(
        tree.status.success(),
        "Failed to inspect QAPI schema roots for {tag}: {}",
        String::from_utf8_lossy(&tree.stderr)
    );
    let roots = String::from_utf8(tree.stdout).context("Invalid UTF-8 in schema paths")?;
    let paths: Vec<_> = roots.split('\0').filter(|path| !path.is_empty()).collect();
    if paths.is_empty() {
        return Ok(tmpdir);
    }
    let output = Command::new("git")
        .args(["archive", tag, "--"])
        .args(paths)
        .current_dir(qemu_repo)
        .output()
        .context("Failed to run git archive")?;
    anyhow::ensure!(
        output.status.success(),
        "Failed to archive QAPI schemas for {tag}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tar_data = output.stdout;

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
    let legacy = dir.join("qapi-schema-guest.json");
    legacy.exists().then_some(legacy)
}

/// Discover release tags from the QMP schema era (1.0 onward).
pub fn discover_version_tags(qemu_repo: &Path) -> Result<Vec<String>> {
    discover_schema_era_tags(qemu_repo, (1, 0, 0))
}

/// Discover release tags from the guest-agent schema era (0.15 onward).
pub fn discover_qga_version_tags(qemu_repo: &Path) -> Result<Vec<String>> {
    discover_schema_era_tags(qemu_repo, (0, 15, 0))
}

fn discover_schema_era_tags(qemu_repo: &Path, earliest: (u32, u32, u32)) -> Result<Vec<String>> {
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
                && parse_version_tag(t).is_some_and(|v| v >= earliest)
        })
        .map(|s| s.trim().to_string())
        .collect();

    tags.sort_by_key(|tag| parse_version_tag_full(tag).unwrap());
    Ok(tags)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_release_components_are_strict_and_preserve_subpatch_order() {
        assert_eq!(parse_version_tag("v2.8.1.1"), Some((2, 8, 1)));
        assert_eq!(parse_version_tag_full("v2.8.1.1"), Some((2, 8, 1, 1)));
        for invalid in [
            "v+2.8.1",
            "v2..1",
            "v2.8.1.x",
            "v2.8.1.1.1",
            "v2.8.1.4294967296",
            "v2.8.1-rc0",
        ] {
            assert!(parse_version_tag(invalid).is_none(), "{invalid}");
        }
        let commit = "0".repeat(40);
        let pins = format!("v2.8.1 {commit}\nv2.8.1.1 {commit}\nv2.8.2 {commit}\n");
        assert_eq!(parse_pinned_versions(&pins).unwrap().len(), 3);
        assert!(parse_pinned_versions(&format!("v2.8.1.1 {commit}\nv2.8.1 {commit}\n")).is_err());
        assert!(parse_pinned_versions(&format!("v2.8.1 {commit}\nv2.8.1.0 {commit}\n")).is_err());
    }

    #[test]
    fn pinned_releases_are_ordered_and_end_at_current_baseline() {
        let pins = parse_pinned_versions(PINNED_VERSIONS).unwrap();
        assert_eq!(pins.first().unwrap().tag, "v0.15.0");
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

//! Explicit broad scan; normal generation remains independent of extra local tags.
use qapi_codegen::{
    unified::{build_qga_registry, build_unified_registry},
    versions::{
        PINNED_VERSIONS, discover_qga_version_tags, discover_version_tags, parse_pinned_versions,
        parse_version_tag_full, verify_pinned_versions,
    },
};
use std::path::Path;

#[test]
#[ignore = "scans every locally mirrored schema-era release through the pinned baseline"]
fn all_mirrored_schema_era_releases_lower_without_skipping_present_inputs() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/qemu");
    let pins = parse_pinned_versions(PINNED_VERSIONS).unwrap();
    verify_pinned_versions(&repo, &pins).unwrap();
    let ceiling = parse_version_tag_full(&pins.last().unwrap().tag).unwrap();
    let qmp: Vec<_> = discover_version_tags(&repo)
        .unwrap()
        .into_iter()
        .filter(|tag| parse_version_tag_full(tag).unwrap() <= ceiling)
        .collect();
    let qga: Vec<_> = discover_qga_version_tags(&repo)
        .unwrap()
        .into_iter()
        .filter(|tag| parse_version_tag_full(tag).unwrap() <= ceiling)
        .collect();
    let qmp_tags: Vec<_> = qmp.iter().map(String::as_str).collect();
    let qga_tags: Vec<_> = qga.iter().map(String::as_str).collect();
    eprintln!(
        "Scanning {} QMP releases and {} QGA releases",
        qmp_tags.len(),
        qga_tags.len()
    );
    assert_eq!(
        build_unified_registry(&repo, &qmp_tags)
            .unwrap()
            .versions
            .len(),
        qmp_tags.len()
    );
    assert_eq!(
        build_qga_registry(&repo, &qga_tags).unwrap().versions.len(),
        qga_tags.len()
    );
}

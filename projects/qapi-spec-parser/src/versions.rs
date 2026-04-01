use std::path::{Path, PathBuf};

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

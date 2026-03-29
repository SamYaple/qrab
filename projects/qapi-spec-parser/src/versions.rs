use std::path::{Path, PathBuf};

/// A QEMU version we test against, with its schema entry point location.
#[derive(Debug, Clone)]
pub struct QemuVersion {
    pub tag: &'static str,
    /// Path relative to the QEMU source root where the schema entry point lives.
    pub schema_entry: &'static str,
}

/// All major QEMU releases that have a QAPI schema.
/// v1.1.0 is the earliest version with qapi-schema.json.
/// v4.0.0 is where the schema moved from root to qapi/.
pub const QEMU_VERSIONS: &[QemuVersion] = &[
    // Root-level qapi-schema.json era (v1.1.0 – v3.1.0)
    QemuVersion { tag: "v1.1.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v1.2.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v1.3.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v1.4.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v1.5.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v1.6.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v1.7.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v2.0.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v2.1.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v2.2.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v2.3.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v2.4.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v2.5.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v2.6.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v2.7.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v2.8.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v2.9.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v2.10.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v2.11.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v2.12.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v3.0.0", schema_entry: "qapi-schema.json" },
    QemuVersion { tag: "v3.1.0", schema_entry: "qapi-schema.json" },
    // qapi/ subdirectory era (v4.0.0+)
    QemuVersion { tag: "v4.0.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v4.1.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v4.2.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v5.0.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v5.1.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v5.2.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v6.0.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v6.1.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v6.2.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v7.0.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v7.1.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v7.2.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v8.0.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v8.1.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v8.2.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v9.0.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v9.1.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v9.2.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v10.0.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v10.1.0", schema_entry: "qapi/qapi-schema.json" },
    QemuVersion { tag: "v10.2.0", schema_entry: "qapi/qapi-schema.json" },
];

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

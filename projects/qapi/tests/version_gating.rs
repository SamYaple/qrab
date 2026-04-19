//! Dual-layer version gating invariants.
//!
//! QEMU fields carry `#[qapi(added = "X.Y.Z")]`. At runtime, when
//! `set_qemu_version()` sets a version below `X.Y.Z`, those fields must
//! be omitted from serialized JSON. On the deserialize side, older QEMU
//! versions simply do not emit these fields, so deserialization must
//! tolerate their absence and yield `None` without error.
//!
//! `QEMU_VERSION` is a `OnceLock`, so it can only be set once per test
//! binary. This file pins it to `(10, 1, 0)` — below `10.2.0`, the
//! `since` value of `ImageInfo::limits`. Other files in `tests/` run as
//! separate binaries and may pick a different version.

use qapi::*;

/// Lock the process-wide QEMU version below the `limits` field's
/// `since = "10.2.0"` bound. Idempotent so individual tests can call
/// it unconditionally.
fn init_version_below_limits() {
    // Ignore the error path: another test in this binary already set
    // the version, which is fine as long as it matched.
    let _ = set_qemu_version(10, 1, 0);
    assert_eq!(qemu_version(), (10, 1, 0));
}

/// Build a representative `ImageInfo` with the gated `limits` field
/// populated. Required fields kept minimal.
fn image_info_with_limits() -> ImageInfo {
    ImageInfo {
        actual_size: None,
        backing_filename: None,
        backing_filename_format: None,
        backing_image: None,
        cluster_size: None,
        compressed: None,
        dirty_flag: None,
        encrypted: None,
        filename: "test.qcow2".to_string(),
        format: "qcow2".to_string(),
        format_specific: None,
        full_backing_filename: None,
        limits: Some(BlockLimitsInfo {
            discard_alignment: None,
            max_discard: None,
            max_hw_iov: None,
            max_hw_transfer: None,
            max_iov: 1024,
            max_transfer: None,
            max_write_zeroes: None,
            min_mem_alignment: 512,
            opt_mem_alignment: 4096,
            opt_transfer: None,
            request_alignment: 512,
            write_zeroes_alignment: None,
        }),
        snapshots: None,
        virtual_size: 1 << 30,
    }
}

/// When `qemu_version()` is below a field's `since`, serializing that
/// field must omit it from the wire JSON even if the value is `Some(_)`.
#[test]
fn field_above_runtime_version_is_skipped_on_serialize() {
    init_version_below_limits();
    let info = image_info_with_limits();
    let json = serde_json::to_string(&info).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert!(
        parsed.get("limits").is_none(),
        "limits (added 10.2.0) must be omitted at runtime 10.1.0, got: {}",
        json
    );
    // Sanity: required fields are still emitted.
    assert_eq!(parsed["filename"], "test.qcow2");
    assert_eq!(parsed["format"], "qcow2");
    assert_eq!(parsed["virtual-size"], 1 << 30);
}

/// When the wire JSON from an older QEMU omits a gated field, the
/// deserializer must tolerate its absence and produce `None` rather
/// than error out with "missing field".
#[test]
fn field_above_runtime_version_deserializes_as_default() {
    init_version_below_limits();
    // JSON as an older QEMU (pre-10.2.0) would emit — no `limits` key.
    let json = r#"{"filename":"old.qcow2","format":"qcow2","virtual-size":536870912}"#;
    let info: ImageInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.filename, "old.qcow2");
    assert_eq!(info.format, "qcow2");
    assert_eq!(info.virtual_size, 536870912);
    assert!(
        info.limits.is_none(),
        "limits must default to None when absent in JSON"
    );
}

/// Round-trip companion: serialize at 10.1.0, immediately deserialize
/// the result, and confirm the gated field survives as `None` (it was
/// dropped on the wire in the first place).
#[test]
fn gated_field_round_trip_drops_value() {
    init_version_below_limits();
    let info = image_info_with_limits();
    let json = serde_json::to_string(&info).unwrap();
    let back: ImageInfo = serde_json::from_str(&json).unwrap();
    assert!(
        back.limits.is_none(),
        "round-trip at low version should strip limits, got {:?}",
        back.limits
    );
}

/// `set_qemu_version` is a `OnceLock`. A second call with a different
/// version must fail and report the still-effective value.
#[test]
fn set_qemu_version_is_once_lock() {
    init_version_below_limits();
    let result = set_qemu_version(7, 0, 0);
    assert_eq!(result, Err((10, 1, 0)));
    // The effective version is still the first one set.
    assert_eq!(qemu_version(), (10, 1, 0));
}

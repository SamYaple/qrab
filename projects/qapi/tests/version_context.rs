use qapi::{qemu_version, with_version};

#[test]
fn serialization_context_is_nested_and_restored_on_panic() {
    let original = qemu_version();
    with_version((11, 1, 3), || {
        assert_eq!(qemu_version(), (11, 1, 3));
        with_version((10, 2, 3), || assert_eq!(qemu_version(), (10, 2, 3)));
        assert_eq!(qemu_version(), (11, 1, 3));
        let result = std::panic::catch_unwind(|| with_version((1, 0, 0), || panic!("test unwind")));
        assert!(result.is_err());
        assert_eq!(qemu_version(), (11, 1, 3));
        std::thread::spawn(move || assert_eq!(qemu_version(), original))
            .join()
            .unwrap();
    });
    assert_eq!(qemu_version(), original);
}

#[test]
fn out_of_context_serialization_names_the_fix_and_the_helper_works() {
    let props: qapi::CpuInstanceProperties =
        serde_json::from_value(serde_json::json!({"core-id": 2, "thread-id": 0})).unwrap();
    let error = serde_json::to_value(&props).unwrap_err().to_string();
    assert!(error.contains("CpuInstanceProperties"), "{error}");
    assert!(error.contains("qapi::with_version"), "{error}");
    let value = qapi::to_value((11, 1, 3), &props).unwrap();
    assert_eq!(value["core-id"], 2);
    assert_eq!(qemu_version(), (0, 0, 0), "the helper restores the context");

    // Inside a context, a type outside its lifetime names the peer version.
    let removed: qapi::TargetInfo =
        serde_json::from_value(serde_json::json!({"arch": "x86_64"})).unwrap();
    let error = qapi::to_value((11, 1, 3), &removed)
        .unwrap_err()
        .to_string();
    assert!(error.contains("not available in QEMU 11.1.3"), "{error}");
    assert!(qapi::to_value((10, 0, 0), &removed).is_ok());
}

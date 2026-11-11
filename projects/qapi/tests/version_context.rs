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

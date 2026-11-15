//! The peer's QEMU/guest build does not have to match the Rust client's host OS.
use serde_json::json;

#[qapi::qapi(condition = "CONFIG_WIN32")]
struct RemoteWindows {
    value: String,
}
#[qapi::qapi(condition = "CONFIG_POSIX")]
struct RemotePosix {
    value: String,
}
#[qapi::qapi]
enum RemoteKind {
    #[qapi(name = "windows", condition = "CONFIG_WIN32")]
    Windows,
    #[qapi(name = "posix", condition = "CONFIG_POSIX")]
    Posix,
}
#[qapi::qapi]
struct RemoteFields {
    #[qapi(condition = "CONFIG_WIN32")]
    windows: Option<String>,
    #[qapi(condition = "CONFIG_POSIX")]
    posix: Option<String>,
}

#[test]
fn remote_platform_types_variants_and_fields_exist_on_every_client_host() {
    let windows: RemoteWindows = serde_json::from_value(json!({"value":"windows"})).unwrap();
    let posix: RemotePosix = serde_json::from_value(json!({"value":"posix"})).unwrap();
    assert_eq!(
        serde_json::to_value(windows).unwrap(),
        json!({"value":"windows"})
    );
    assert_eq!(
        serde_json::to_value(posix).unwrap(),
        json!({"value":"posix"})
    );
    for kind in ["windows", "posix"] {
        let parsed: RemoteKind = serde_json::from_value(json!(kind)).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), json!(kind));
    }
    for wire in [json!({"windows":"w"}), json!({"posix":"p"}), json!({})] {
        let value: RemoteFields = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
    }
}

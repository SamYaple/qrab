use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use openidconnect::{
    core::{
        CoreIdToken, CoreIdTokenClaims, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
        CoreRsaPrivateSigningKey,
    },
    ClientId, IssuerUrl, JsonWebKeyId, PrivateSigningKey,
};
use qrab_redfish::{auth, AppState};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;
use tower::ServiceExt;

const ISSUER: &str = "https://idm.example/oauth2/openid/qrab-redfish";
const AUDIENCE: &str = "qrab-redfish";
const ADMIN: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const VIEWER: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const UNLISTED: &str = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
const STATE_ID: &str = "0f0e0d0c-0b0a-4908-8706-050403020100";
const PORTAL: &str = "192.0.2.10:3260";

fn key(name: &str) -> CoreRsaPrivateSigningKey {
    let pem = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .unwrap();
    CoreRsaPrivateSigningKey::from_pem(&pem, Some(JsonWebKeyId::new("test".into()))).unwrap()
}

fn token(signer: &str, issuer: &str, audience: &str, subject: &str, expires_in: i64) -> String {
    let now = auth::now() as i64;
    let claims: CoreIdTokenClaims = serde_json::from_value(json!({
        "iss": issuer, "aud": [audience], "sub": subject, "iat": now - 5, "exp": now + expires_in
    }))
    .unwrap();
    CoreIdToken::new(
        claims,
        &key(signer),
        CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
        None,
        None,
    )
    .unwrap()
    .to_string()
}

fn good(subject: &str) -> String {
    token("test-only-signing-key.pem", ISSUER, AUDIENCE, subject, 300)
}

struct Fixture {
    app: AppState,
    receiver: Option<manager::pod::ControlReceiver>,
    config: manager::pod::Config,
    _dir: tempfile_dir::Dir,
}

/// A tiny tempdir so the test needs no extra dependency.
mod tempfile_dir {
    pub struct Dir(pub std::path::PathBuf);
    impl Dir {
        pub fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "qrab-redfish-{}-{}",
                std::process::id(),
                uuid_like()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    fn uuid_like() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        NEXT.fetch_add(1, Ordering::Relaxed)
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

fn fixture() -> Fixture {
    let dir = tempfile_dir::Dir::new();
    let config: manager::pod::Config = serde_json::from_value(json!({
        "name":"redfish-vm","kernel":"/unused","cpus":2,"memory_mib":1536,"runtime_dir":dir.0,
        "state_id":STATE_ID,
        "iscsi_disks":[{"id":"root","portal":PORTAL,"target":"iqn.2026-11.com.yapplesauce:qrab.test.disk",
            "initiator_name":"iqn.2026-11.com.yapplesauce:qrab.vm.x","username":"chap-user",
            "password_file":"/run/qrab-auth/root/password"}]
    }))
    .unwrap();
    let (control, receiver) =
        manager::pod::control_channel_for(&config, manager::pod::Deployment::default());
    let keys = CoreJsonWebKeySet::new(vec![key("test-only-signing-key.pem").as_verification_key()]);
    let authenticator = auth::Authenticator::new(
        IssuerUrl::new(ISSUER.into()).unwrap(),
        ClientId::new(AUDIENCE.into()),
        keys,
        [ADMIN.to_string()].into_iter().collect(),
        [VIEWER.to_string()].into_iter().collect(),
    );
    Fixture {
        app: AppState {
            auth: Arc::new(authenticator),
            control: manager::pod::Backend::Local(control),
            streams: Arc::new(Semaphore::new(2)),
        },
        receiver: Some(receiver),
        config,
        _dir: dir,
    }
}

fn write_status(config: &manager::pod::Config, phase: &str, run_state: &str) {
    std::fs::write(
        config.runtime_dir.join("status.json"),
        serde_json::to_vec(&json!({
            "phase":phase,"ready":run_state=="running","manager_pid":1,"manager_start_ticks":1,
            "qemu_pid":2,"qemu_start_ticks":2,"qemu_version":[11,1,3],"run_state":run_state,
            "observed_unix_ms":0,"exit_code":null,"detail":null
        }))
        .unwrap(),
    )
    .unwrap();
}

fn record(fixture: &Fixture, name: &str, data: Value) {
    fixture.receiver.as_ref().unwrap().record_event(
        serde_json::from_value(json!({
            "event":name,"data":data,"timestamp":{"seconds":1_700_000_000,"microseconds":250_000}
        }))
        .unwrap(),
    );
}

async fn call(
    app: &AppState,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: &str,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let response = qrab_redfish::router(app.clone())
        .oneshot(request.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, headers, value)
}

fn assert_error(body: &Value, message_id: &str) {
    let info = &body["error"]["@Message.ExtendedInfo"][0];
    assert_eq!(
        info["MessageId"],
        format!("Base.1.18.{message_id}"),
        "{body}"
    );
    assert_eq!(info["@odata.type"], "#Message.v1_2_0.Message");
    assert!(body["error"]["code"]
        .as_str()
        .unwrap()
        .starts_with("Base.1.18."));
    assert!(body["error"]["message"].is_string());
}

#[tokio::test]
async fn bearer_tokens_are_required_and_fully_verified() {
    let fixture = fixture();
    let app = &fixture.app;
    write_status(&fixture.config, "running", "running");
    let rejected = [
        None,
        Some("not-a-jwt".to_string()),
        Some(token(
            "test-only-signing-key.pem",
            ISSUER,
            AUDIENCE,
            ADMIN,
            -60,
        )),
        Some(token(
            "test-only-signing-key.pem",
            ISSUER,
            "another-client",
            ADMIN,
            300,
        )),
        Some(token(
            "test-only-signing-key.pem",
            "https://evil.example/oauth2/openid/qrab-redfish",
            AUDIENCE,
            ADMIN,
            300,
        )),
        Some(token(
            "test-only-foreign-key.pem",
            ISSUER,
            AUDIENCE,
            ADMIN,
            300,
        )),
    ];
    for path in [
        "/redfish",
        "/redfish/v1/",
        SYSTEM_PATH,
        "/redfish/v1/EventService/SSE",
    ] {
        for token in &rejected {
            let (status, headers, body) = call(app, "GET", path, token.as_deref(), "").await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{path} {token:?}");
            assert!(headers["www-authenticate"]
                .to_str()
                .unwrap()
                .starts_with("Bearer"));
            assert_eq!(headers["odata-version"], "4.0");
            assert_error(&body, "NoValidSession");
        }
    }
    // No Basic authentication with stored passwords.
    let response = qrab_redfish::router(app.clone())
        .oneshot(
            Request::builder()
                .uri("/redfish/v1/")
                .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    // A valid token for an unlisted subject is authenticated but not authorized.
    let (status, _, body) = call(app, "GET", "/redfish/v1/", Some(&good(UNLISTED)), "").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_error(&body, "InsufficientPrivilege");
    // Viewers read but cannot reset; admins can.
    let viewer = good(VIEWER);
    assert_eq!(
        call(app, "GET", SYSTEM_PATH, Some(&viewer), "").await.0,
        StatusCode::OK
    );
    let (status, _, body) = call(app, "POST", RESET, Some(&viewer), r#"{"ResetType":"On"}"#).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_error(&body, "InsufficientPrivilege");
    let (status, _, _) = call(
        app,
        "POST",
        RESET,
        Some(&good(ADMIN)),
        r#"{"ResetType":"On"}"#,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "On is a no-op for a running VM"
    );
}

const SYSTEM_PATH: &str = "/redfish/v1/Systems/1";
const RESET: &str = "/redfish/v1/Systems/1/Actions/ComputerSystem.Reset";

#[tokio::test]
async fn resources_have_redfish_shapes_and_links() {
    let fixture = fixture();
    let app = &fixture.app;
    write_status(&fixture.config, "running", "running");
    let admin = good(ADMIN);
    let get = |path: &'static str, token: String| async move {
        let (status, _, body) = call(app, "GET", path, Some(&token), "").await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
        body
    };
    let versions = get("/redfish", admin.clone()).await;
    assert_eq!(versions, json!({"v1":"/redfish/v1/"}));
    let root = get("/redfish/v1/", admin.clone()).await;
    assert_eq!(root["@odata.id"], "/redfish/v1/");
    assert!(root["@odata.type"]
        .as_str()
        .unwrap()
        .starts_with("#ServiceRoot.v1_"));
    assert_eq!(root["Systems"]["@odata.id"], "/redfish/v1/Systems");
    assert_eq!(root["Managers"]["@odata.id"], "/redfish/v1/Managers");
    assert_eq!(
        root["EventService"]["@odata.id"],
        "/redfish/v1/EventService"
    );
    assert_eq!(root["UUID"], STATE_ID);
    assert_eq!(get("/redfish/v1", admin.clone()).await, root);

    let systems = get("/redfish/v1/Systems", admin.clone()).await;
    assert_eq!(
        systems["@odata.type"],
        "#ComputerSystemCollection.ComputerSystemCollection"
    );
    assert_eq!(systems["Members@odata.count"], 1);
    assert_eq!(systems["Members"][0]["@odata.id"], SYSTEM_PATH);

    let system = get(SYSTEM_PATH, admin.clone()).await;
    assert!(system["@odata.type"]
        .as_str()
        .unwrap()
        .starts_with("#ComputerSystem.v1_"));
    assert_eq!(
        (system["Id"].as_str(), system["Name"].as_str()),
        (Some("1"), Some("redfish-vm"))
    );
    assert_eq!(system["SystemType"], "Virtual");
    assert_eq!(system["PowerState"], "On");
    assert_eq!(system["Status"], json!({"State":"Enabled","Health":"OK"}));
    assert_eq!(system["ProcessorSummary"]["Count"], 2);
    assert_eq!(system["MemorySummary"]["TotalSystemMemoryGiB"], 1.5);
    let reset = &system["Actions"]["#ComputerSystem.Reset"];
    assert_eq!(reset["target"], RESET);
    assert_eq!(
        reset["ResetType@Redfish.AllowableValues"],
        json!(["On", "GracefulShutdown", "ForceRestart", "Pause", "Resume"])
    );
    assert_eq!(
        system["Links"]["ManagedBy"][0]["@odata.id"],
        "/redfish/v1/Managers/1"
    );
    assert_eq!(system["UUID"], STATE_ID);
    let oem = &system["Actions"]["Oem"];
    assert_eq!(
        oem["#QrabComputerSystem.Freeze"]["target"],
        qrab_redfish::FREEZE_TARGET
    );
    assert_eq!(
        oem["#QrabComputerSystem.Thaw"]["target"],
        qrab_redfish::THAW_TARGET
    );
    assert_eq!(
        oem["#QrabComputerSystem.Update"]["target"],
        qrab_redfish::UPDATE_TARGET
    );
    // Cached guest-agent state, available before QEMU runs.
    assert_eq!(system["Oem"]["Qrab"]["GuestAgent"]["enabled"], true);
    assert_eq!(system["Oem"]["Qrab"]["GuestAgent"]["connected"], false);
    let admin_config = system["Oem"]["Qrab"]["Configuration"].to_string();
    assert!(admin_config.contains(PORTAL) && admin_config.contains("chap-user"));
    // Viewers get the BMC's redacted configuration and no identity.
    let viewer_system = get(SYSTEM_PATH, good(VIEWER)).await;
    let viewer_config = viewer_system["Oem"]["Qrab"]["Configuration"].to_string();
    for sensitive in [PORTAL, "chap-user", "/run/qrab-auth", STATE_ID] {
        assert!(
            !viewer_system.to_string().contains(sensitive),
            "{sensitive} leaked"
        );
    }
    assert!(viewer_config.contains("\"redacted\":true"));

    let managers = get("/redfish/v1/Managers", admin.clone()).await;
    assert_eq!(
        managers["Members"][0]["@odata.id"],
        "/redfish/v1/Managers/1"
    );
    let manager = get("/redfish/v1/Managers/1", admin.clone()).await;
    assert!(manager["@odata.type"]
        .as_str()
        .unwrap()
        .starts_with("#Manager.v1_"));
    assert_eq!(manager["ManagerType"], "BMC");
    assert_eq!(
        manager["Links"]["ManagerForServers"][0]["@odata.id"],
        SYSTEM_PATH
    );
    let services = get("/redfish/v1/Managers/1/LogServices", admin.clone()).await;
    assert_eq!(
        services["Members"][0]["@odata.id"],
        "/redfish/v1/Managers/1/LogServices/Events"
    );
    let log = get("/redfish/v1/Managers/1/LogServices/Events", admin.clone()).await;
    assert!(log["@odata.type"]
        .as_str()
        .unwrap()
        .starts_with("#LogService.v1_"));
    assert_eq!(log["MaxNumberOfRecords"], 256);
    assert_eq!(log["OverWritePolicy"], "WrapsWhenFull");
    assert_eq!(log["Oem"]["Qrab"]["QmpDropped"], 0);
    assert_eq!(log["Status"]["Health"], "OK");

    record(&fixture, "STOP", json!({}));
    record(
        &fixture,
        "BLOCK_IO_ERROR",
        json!({"device":"","node-name":"iscsi-root","operation":"write","action":"report"}),
    );
    let entries = get(
        "/redfish/v1/Managers/1/LogServices/Events/Entries",
        admin.clone(),
    )
    .await;
    assert_eq!(entries["Members@odata.count"], 2);
    let entry = &entries["Members"][1];
    assert_eq!(
        entry["@odata.id"],
        "/redfish/v1/Managers/1/LogServices/Events/Entries/2"
    );
    assert!(entry["@odata.type"]
        .as_str()
        .unwrap()
        .starts_with("#LogEntry.v1_"));
    assert_eq!(entry["EntryType"], "Oem");
    assert_eq!(entry["Severity"], "Critical");
    assert_eq!(entry["Message"], "BLOCK_IO_ERROR");
    assert_eq!(entry["Created"], "2023-11-14T22:13:20.250Z");
    assert_eq!(entry["Oem"]["Qrab"]["Data"]["node-name"], "iscsi-root");
    assert_eq!(
        get(
            "/redfish/v1/Managers/1/LogServices/Events/Entries/2",
            admin.clone()
        )
        .await,
        *entry
    );
    let (status, _, body) = call(
        app,
        "GET",
        "/redfish/v1/Managers/1/LogServices/Events/Entries/99",
        Some(&admin),
        "",
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_error(&body, "ResourceNotFound");

    let events = get("/redfish/v1/EventService", admin.clone()).await;
    assert!(events["@odata.type"]
        .as_str()
        .unwrap()
        .starts_with("#EventService.v1_"));
    assert_eq!(events["ServerSentEventUri"], "/redfish/v1/EventService/SSE");
    let subscriptions = get("/redfish/v1/EventService/Subscriptions", admin.clone()).await;
    assert_eq!(subscriptions["Members@odata.count"], 0);
    let (status, _, body) = call(
        app,
        "POST",
        "/redfish/v1/EventService/Subscriptions",
        Some(&admin),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_error(&body, "OperationNotAllowed");
}

#[tokio::test]
async fn reset_types_map_honestly_and_bad_requests_get_redfish_errors() {
    let mut fixture = fixture();
    write_status(&fixture.config, "running", "paused");
    let admin = good(ADMIN);
    for (body, message_id) in [
        (r#"{"ResetType":"ForceOff"}"#, "ActionParameterNotSupported"),
        (
            r#"{"ResetType":"GracefulRestart"}"#,
            "ActionParameterNotSupported",
        ),
        (
            r#"{"ResetType":"Explode"}"#,
            "ActionParameterValueNotInList",
        ),
        (r#"{"Reset":"On"}"#, "ActionParameterMissing"),
        ("not json", "MalformedJSON"),
    ] {
        let (status, _, reply) = call(&fixture.app, "POST", RESET, Some(&admin), body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_error(&reply, message_id);
    }
    let paused: manager::pod::Status = serde_json::from_slice(
        &std::fs::read(fixture.config.runtime_dir.join("status.json")).unwrap(),
    )
    .unwrap();
    use manager::pod::Action;
    for (reset_type, action) in [
        ("On", Action::Resume),
        ("Resume", Action::Resume),
        ("Pause", Action::Pause),
        ("ForceRestart", Action::Reset),
        ("GracefulShutdown", Action::Powerdown),
    ] {
        assert_eq!(
            qrab_redfish::reset_action(reset_type, Some(&paused)).unwrap(),
            Some(action)
        );
    }
    write_status(&fixture.config, "exited", "shutdown");
    let (status, _, reply) = call(
        &fixture.app,
        "POST",
        RESET,
        Some(&admin),
        r#"{"ResetType":"On"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_error(&reply, "OperationNotAllowed");
    // An admin action reaches the manager; with no supervisor it is unavailable.
    fixture.receiver = None;
    let (status, _, reply) = call(
        &fixture.app,
        "POST",
        RESET,
        Some(&admin),
        r#"{"ResetType":"ForceRestart"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_error(&reply, "ServiceTemporarilyUnavailable");
    assert_eq!(
        reply["error"]["message"],
        "The VM did not complete the reset."
    );
}

#[tokio::test]
async fn power_state_follows_the_manager_status() {
    let fixture = fixture();
    let viewer = good(VIEWER);
    for (phase, run_state, power, state, health) in [
        (
            "starting",
            "prelaunch",
            json!("PoweringOn"),
            "Starting",
            "OK",
        ),
        ("running", "paused", json!("Paused"), "Quiesced", "OK"),
        (
            "running",
            "guest-panicked",
            json!("On"),
            "Enabled",
            "Critical",
        ),
        (
            "stopping",
            "shutdown",
            json!("PoweringOff"),
            "Disabled",
            "OK",
        ),
        ("failed", "shutdown", json!("Off"), "Disabled", "Critical"),
    ] {
        write_status(&fixture.config, phase, run_state);
        let (_, _, system) = call(&fixture.app, "GET", SYSTEM_PATH, Some(&viewer), "").await;
        assert_eq!(system["PowerState"], power, "{phase} {run_state}");
        assert_eq!(system["Status"], json!({"State":state,"Health":health}));
    }
    std::fs::remove_file(fixture.config.runtime_dir.join("status.json")).unwrap();
    let (_, _, system) = call(&fixture.app, "GET", SYSTEM_PATH, Some(&viewer), "").await;
    assert!(system["PowerState"].is_null());
}

#[tokio::test]
async fn event_stream_delivers_redfish_events_and_ends_at_token_expiry() {
    use futures::StreamExt;
    let fixture = fixture();
    record(
        &fixture,
        "RESET",
        json!({"guest":false,"reason":"host-qmp-system-reset"}),
    );
    let short = token("test-only-signing-key.pem", ISSUER, AUDIENCE, VIEWER, 2);
    let response = qrab_redfish::router(fixture.app.clone())
        .oneshot(
            Request::builder()
                .uri("/redfish/v1/EventService/SSE")
                .header("authorization", format!("Bearer {short}"))
                .header("last-event-id", "0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("text/event-stream"));
    let mut body = response.into_body().into_data_stream();
    let first = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
    assert!(first.contains("id: 1"), "{first}");
    let data: Value = serde_json::from_str(
        first
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap(),
    )
    .unwrap();
    assert!(data["@odata.type"]
        .as_str()
        .unwrap()
        .starts_with("#Event.v1_"));
    assert_eq!(data["Events"][0]["Message"], "RESET");
    assert_eq!(
        data["Events"][0]["OriginOfCondition"]["@odata.id"],
        SYSTEM_PATH
    );
    let drain = tokio::spawn(async move { while body.next().await.is_some() {} });
    tokio::time::timeout(Duration::from_secs(5), drain)
        .await
        .expect("stream outlived its bearer token")
        .unwrap();
    assert_eq!(fixture.app.streams.available_permits(), 2);
}

#[tokio::test]
async fn unknown_resources_are_redfish_not_found() {
    let fixture = fixture();
    let (status, headers, body) = call(
        &fixture.app,
        "GET",
        "/redfish/v1/Chassis",
        Some(&good(ADMIN)),
        "",
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(headers["odata-version"], "4.0");
    assert_error(&body, "ResourceNotFound");
}

#[test]
fn timestamps_are_rfc3339_utc() {
    assert_eq!(qrab_redfish::rfc3339(0), "1970-01-01T00:00:00.000Z");
    assert_eq!(
        qrab_redfish::rfc3339(951_782_400_123),
        "2000-02-29T00:00:00.123Z"
    );
    assert_eq!(
        qrab_redfish::rfc3339(1_794_441_599_999),
        "2026-11-11T23:59:59.999Z"
    );
}

#[tokio::test]
async fn guest_agent_actions_are_admin_only_and_validated() {
    let fixture = fixture();
    let app = &fixture.app;
    let viewer = good(VIEWER);
    let admin = good(ADMIN);
    for (target, body) in [
        (qrab_redfish::FREEZE_TARGET, ""),
        (qrab_redfish::THAW_TARGET, "{}"),
        (qrab_redfish::UPDATE_TARGET, r#"{"Args":["release=2"]}"#),
    ] {
        let (status, _, reply) = call(app, "POST", target, Some(&viewer), body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{target}");
        assert_error(&reply, "InsufficientPrivilege");
        let (status, _, _) = call(app, "POST", target, None, body).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        // Admins reach the manager; the agent is not connected yet.
        let (status, _, reply) = call(app, "POST", target, Some(&admin), body).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{target}");
        assert_eq!(
            reply["error"]["message"],
            "The guest agent did not complete the request."
        );
    }
    for (body, message_id) in [
        (
            r#"{"Args":["$(reboot)"]}"#,
            "ActionParameterValueFormatError",
        ),
        (r#"{"Args":"release=2"}"#, "ActionParameterValueTypeError"),
        ("[1]", "MalformedJSON"),
    ] {
        let (status, _, reply) =
            call(app, "POST", qrab_redfish::UPDATE_TARGET, Some(&admin), body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_error(&reply, message_id);
    }
}

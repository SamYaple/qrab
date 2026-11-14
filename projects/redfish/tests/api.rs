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
const ADMIN_GROUP: &str = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
const VIEWER_GROUP: &str = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Groups {
    groups: Vec<String>,
}
impl openidconnect::AdditionalClaims for Groups {}

/// A valid token for `subject` whose `groups` claim lists `groups`.
fn grouped(subject: &str, groups: &[&str]) -> String {
    let now = auth::now() as i64;
    signed(
        "test-only-signing-key.pem",
        json!({
            "iss": ISSUER, "aud": [AUDIENCE], "sub": subject, "iat": now - 5, "exp": now + 300,
            "groups": groups
        }),
    )
}

/// A token with arbitrary claims (a `groups` claim among them), signed by the
/// named fixture key.
fn signed(signer: &str, claims: Value) -> String {
    let claims: openidconnect::IdTokenClaims<Groups, openidconnect::core::CoreGenderClaim> =
        serde_json::from_value(claims).unwrap();
    openidconnect::IdToken::<
        Groups,
        openidconnect::core::CoreGenderClaim,
        openidconnect::core::CoreJweContentEncryptionAlgorithm,
        CoreJwsSigningAlgorithm,
    >::new(
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
    )
    .with_groups(
        [ADMIN_GROUP.to_string()].into_iter().collect(),
        [VIEWER_GROUP.to_string()].into_iter().collect(),
    );
    Fixture {
        app: AppState {
            auth: Arc::new(authenticator),
            control: manager::pod::Backend::Local(control),
            streams: Arc::new(Semaphore::new(2)),
            requests: Arc::new(Semaphore::new(32)),
            tasks: Default::default(),
            telemetry: Default::default(),
            subscriptions: Default::default(),
            triggers: Default::default(),
            exporter: Default::default(),
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
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
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
    assert_eq!(system["ProcessorSummary"]["LogicalProcessorCount"], 2);
    assert!(system["ProcessorSummary"].get("Count").is_none());
    assert_eq!(system["MemorySummary"]["TotalSystemMemoryGiB"], 1.5);
    let reset = &system["Actions"]["#ComputerSystem.Reset"];
    assert_eq!(reset["target"], RESET);
    assert_eq!(
        reset["ResetType@Redfish.AllowableValues"],
        json!([
            "On",
            "GracefulShutdown",
            "ForceRestart",
            "Pause",
            "Resume",
            "Nmi",
            "GracefulRestart"
        ])
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
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_error(&body, "ServiceTemporarilyUnavailable");
}

#[tokio::test]
async fn reset_types_map_honestly_and_bad_requests_get_redfish_errors() {
    let mut fixture = fixture();
    write_status(&fixture.config, "running", "paused");
    let admin = good(ADMIN);
    for (body, message_id) in [
        (r#"{"ResetType":"ForceOff"}"#, "ActionParameterNotSupported"),
        (r#"{"ResetType":"ForceOn"}"#, "ActionParameterNotSupported"),
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
        ("Nmi", Action::Nmi),
        ("GracefulRestart", Action::Reboot),
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

/// Open the SSE stream as a viewer, optionally resuming after `last`.
async fn open_stream(app: &AppState, last: Option<&str>) -> axum::body::BodyDataStream {
    let mut request = Request::builder()
        .uri("/redfish/v1/EventService/SSE")
        .header("authorization", format!("Bearer {}", good(VIEWER)));
    if let Some(last) = last {
        request = request.header("last-event-id", last);
    }
    let response = qrab_redfish::router(app.clone())
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response.into_body().into_data_stream()
}

/// SSE frames as (`id`, decoded `data`), read until one carries `until` as its id.
async fn frames_until(
    body: &mut axum::body::BodyDataStream,
    until: &str,
) -> Vec<(Option<String>, Value)> {
    use futures::StreamExt;
    let mut text = String::new();
    let complete = tokio::time::timeout(Duration::from_secs(5), async {
        while !text.contains(&format!("id: {until}\n")) {
            let chunk = body.next().await.unwrap().unwrap();
            text.push_str(std::str::from_utf8(&chunk).unwrap());
        }
    })
    .await;
    assert!(complete.is_ok(), "no SSE frame with id {until}: {text}");
    text.split("\n\n")
        .filter_map(|frame| {
            let data = frame.lines().find_map(|line| line.strip_prefix("data: "))?;
            let id = frame
                .lines()
                .find_map(|line| line.strip_prefix("id: "))
                .map(str::to_string);
            Some((id, serde_json::from_str(data).unwrap()))
        })
        .collect()
}

fn message_id(data: &Value) -> &Value {
    &data["Events"][0]["MessageId"]
}

#[tokio::test]
async fn an_evicted_resume_cursor_gets_one_gap_event_before_the_replay() {
    let fixture = fixture();
    let total = manager::pod::EVENT_CAPACITY as u64 + 10;
    for _ in 0..total {
        record(&fixture, "STOP", json!({}));
    }
    // Events 1..=10 are evicted; a client that last saw #3 missed #4..=#10.
    let mut body = open_stream(&fixture.app, Some("3")).await;
    let frames = frames_until(&mut body, "12").await;
    assert_eq!(frames.len(), 3, "{frames:?}");
    let (id, gap) = &frames[0];
    // No SSE id: the client's Last-Event-ID stays the last QMP sequence it saw.
    assert!(id.is_none());
    assert_eq!(gap["@odata.type"], "#Event.v1_10_0.Event");
    let record_ = &gap["Events"][0];
    assert_eq!(record_["MessageId"], qrab_redfish::MISSED_MESSAGE);
    assert_eq!(record_["EventType"], "Other");
    assert_eq!(record_["Severity"], "Warning");
    assert_eq!(record_["MessageArgs"], json!(["7", "3"]));
    assert_eq!(
        record_["OriginOfCondition"]["@odata.id"],
        "/redfish/v1/Managers/1/LogServices/Events"
    );
    assert_eq!(
        record_["Oem"]["Qrab"],
        json!({"@odata.type":"#QrabEventGap.v1_0_0.QrabEventGap",
            "MissedEvents":7,"After":3,"OldestAvailableSequence":11})
    );
    // The gap precedes the replay of what the history still holds.
    let replay: Vec<_> = frames[1..].iter().map(|(id, _)| id.as_deref()).collect();
    assert_eq!(replay, [Some("11"), Some("12")]);
    assert!(frames[1..]
        .iter()
        .all(|(_, data)| message_id(data) == "Qrab.1.0.QmpEvent"));

    // A cursor still inside the history replays without a gap.
    let mut body = open_stream(&fixture.app, Some(&(total - 2).to_string())).await;
    let frames = frames_until(&mut body, &total.to_string()).await;
    assert_eq!(frames.len(), 2, "{frames:?}");
    assert!(frames.iter().all(|(id, _)| id.is_some()));
}

#[tokio::test]
async fn a_changed_qmp_dropped_counter_is_one_stream_event() {
    let fixture = fixture();
    let receiver = fixture.receiver.as_ref().unwrap();
    record(&fixture, "STOP", json!({}));
    let mut live = open_stream(&fixture.app, None).await;
    // Let the stream reach its idle wait before the counter changes.
    tokio::time::sleep(Duration::from_millis(50)).await;
    receiver.set_qmp_dropped(2);
    tokio::time::sleep(Duration::from_millis(50)).await;
    record(&fixture, "RESUME", json!({}));
    let frames = frames_until(&mut live, "2").await;
    assert_eq!(frames.len(), 2, "{frames:?}");
    let (id, notice) = &frames[0];
    assert!(id.is_none());
    assert_eq!(notice["@odata.type"], "#Event.v1_10_0.Event");
    let record_ = &notice["Events"][0];
    assert_eq!(record_["MessageId"], qrab_redfish::DROPPED_MESSAGE);
    assert_eq!(record_["Severity"], "Warning");
    assert_eq!(record_["MessageArgs"], json!(["2"]));
    assert_eq!(
        record_["Oem"]["Qrab"],
        json!({"@odata.type":"#QrabQmpDropped.v1_0_0.QrabQmpDropped","QmpDropped":2})
    );
    assert_eq!(frames[1].1["Events"][0]["Message"], "RESUME");

    // A new stream starts from the current nonzero total, sent once.
    drop(live);
    let mut fresh = open_stream(&fixture.app, Some("0")).await;
    let frames = frames_until(&mut fresh, "2").await;
    let notices: Vec<_> = frames
        .iter()
        .filter(|(_, data)| message_id(data) == qrab_redfish::DROPPED_MESSAGE)
        .collect();
    assert_eq!(notices.len(), 1, "{frames:?}");
    assert_eq!(notices[0].1["Events"][0]["Oem"]["Qrab"]["QmpDropped"], 2);
    assert_eq!(frames.len(), 3, "{frames:?}");
}

#[tokio::test]
async fn unknown_resources_are_redfish_not_found() {
    let fixture = fixture();
    let (status, headers, body) = call(
        &fixture.app,
        "GET",
        "/redfish/v1/Chassis/unknown",
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

#[tokio::test]
async fn group_claims_grant_configured_roles_only() {
    let fixture = fixture();
    let app = &fixture.app;
    write_status(&fixture.config, "running", "running");
    let reset = r#"{"ResetType":"On"}"#;
    // A configured viewer group reads but cannot reset.
    let viewer = grouped(UNLISTED, &["admins@idm.example", VIEWER_GROUP]);
    assert_eq!(
        call(app, "GET", SYSTEM_PATH, Some(&viewer), "").await.0,
        StatusCode::OK
    );
    let (status, _, body) = call(app, "POST", RESET, Some(&viewer), reset).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_error(&body, "InsufficientPrivilege");
    // An admin group wins over a viewer group, in either order.
    for groups in [[ADMIN_GROUP, VIEWER_GROUP], [VIEWER_GROUP, ADMIN_GROUP]] {
        let admin = grouped(UNLISTED, &groups);
        assert_eq!(
            call(app, "POST", RESET, Some(&admin), reset).await.0,
            StatusCode::NO_CONTENT
        );
    }
    // Unconfigured groups, names standing in for UUIDs, and an empty claim grant nothing.
    for groups in [
        vec!["ffffffff-ffff-4fff-8fff-ffffffffffff"],
        vec!["qrab_vm_qrab_redfish-vm_admins"],
        vec![],
    ] {
        let token = grouped(UNLISTED, &groups);
        assert_eq!(
            call(app, "GET", SYSTEM_PATH, Some(&token), "").await.0,
            StatusCode::FORBIDDEN,
            "{groups:?}"
        );
    }
}

#[tokio::test]
async fn native_inventory_is_discoverable_authenticated_and_redacted() {
    let mut fixture = fixture();
    fixture.receiver = None;
    write_status(&fixture.config, "running", "running");
    let token = good(VIEWER);
    for path in [
        "/redfish/v1/Chassis",
        "/redfish/v1/Chassis/1",
        "/redfish/v1/Systems/1/ResetActionInfo",
        "/redfish/v1/Systems/1/Memory",
        "/redfish/v1/Systems/1/Memory/1",
        "/redfish/v1/Systems/1/EthernetInterfaces",
        "/redfish/v1/Systems/1/SimpleStorage",
        "/redfish/v1/Systems/1/SimpleStorage/1",
    ] {
        assert_eq!(
            call(&fixture.app, "GET", path, None, "").await.0,
            StatusCode::UNAUTHORIZED
        );
        let (status, _, body) = call(&fixture.app, "GET", path, Some(&token), "").await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
        assert_eq!(body["@odata.id"], path);
        for secret in [PORTAL, "chap-user", "/run/qrab-auth", STATE_ID] {
            assert!(
                !body.to_string().contains(secret),
                "{path}: leaked {secret}"
            );
        }
        if let Some(members) = body["Members"].as_array() {
            for member in members {
                assert_eq!(
                    call(
                        &fixture.app,
                        "GET",
                        member["@odata.id"].as_str().unwrap(),
                        Some(&token),
                        ""
                    )
                    .await
                    .0,
                    StatusCode::OK
                );
            }
        }
    }
    for path in [
        "/redfish/v1/Systems/1/Processors",
        "/redfish/v1/Systems/1/Processors/1",
    ] {
        assert_eq!(
            call(&fixture.app, "GET", path, None, "").await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            call(&fixture.app, "GET", path, Some(&token), "").await.0,
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
    let (_, _, memory) = call(
        &fixture.app,
        "GET",
        "/redfish/v1/Systems/1/Memory/1",
        Some(&token),
        "",
    )
    .await;
    assert_eq!(memory["CapacityMiB"], 1536);
    let (_, _, nics) = call(
        &fixture.app,
        "GET",
        "/redfish/v1/Systems/1/EthernetInterfaces",
        Some(&token),
        "",
    )
    .await;
    assert_eq!(nics["Members@odata.count"], 0);
    for path in [
        "/redfish/v1/Systems/1/Memory/2",
        "/redfish/v1/Systems/1/Unknown",
        "/redfish/v1/Systems/1/Memory/01",
    ] {
        let (status, _, body) = call(&fixture.app, "GET", path, Some(&token), "").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_error(&body, "ResourceNotFound");
    }
}

#[tokio::test]
async fn telemetry_discovery_and_live_failure_are_authenticated() {
    let mut fixture = fixture();
    fixture.receiver = None; // No VM supervisor: live queries must fail, never invent zeroes.
    let viewer = good(VIEWER);
    for (path, expected) in [
        ("/redfish/v1/TelemetryService", StatusCode::OK),
        ("/redfish/v1/TelemetryService/MetricReports", StatusCode::OK),
        (
            "/redfish/v1/TelemetryService/MetricReports/VM",
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        (
            "/redfish/v1/Systems/1/Oem/Qrab/Metrics",
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    ] {
        assert_eq!(
            call(&fixture.app, "GET", path, None, "").await.0,
            StatusCode::UNAUTHORIZED
        );
        let (status, _, body) = call(&fixture.app, "GET", path, Some(&viewer), "").await;
        assert_eq!(status, expected, "{path}: {body}");
        if expected == StatusCode::SERVICE_UNAVAILABLE {
            assert_error(&body, "ServiceTemporarilyUnavailable");
        }
    }
}

#[tokio::test]
async fn live_report_round_trips_through_the_sidecar_control_socket() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let mut fixture = fixture();
    let path = fixture.config.runtime_dir.join("metrics.sock");
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    fixture.app.control = manager::pod::Backend::Socket(manager::pod::Remote::new(path));
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut stream = BufReader::new(stream);
        stream.get_mut().write_all(b"{\"protocol\":\"qrab-control\",\"version\":1,\"vm\":\"test\",\"operations\":[\"metrics\"]}\n").await.unwrap();
        // Reuse one connection for both report and MetricProperty resource.
        for _ in 0..2 {
            let mut line = String::new();
            stream.read_line(&mut line).await.unwrap();
            let request: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(request["op"], "metrics");
            let sample = manager::pod::VmMetrics {
                observed_unix_ms: 1_700_000_000_123,
                logical_processors: 4,
                memory_bytes: 3_221_225_472,
                block_read_bytes: 4096,
                ..Default::default()
            };
            let reply = json!({"v":1,"id":request["id"],"result":sample}).to_string() + "\n";
            stream.get_mut().write_all(reply.as_bytes()).await.unwrap();
        }
    });
    let viewer = good(VIEWER);
    let (status, _, report) = call(
        &fixture.app,
        "GET",
        "/redfish/v1/TelemetryService/MetricReports/VM",
        Some(&viewer),
        "",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    let value = report["MetricValues"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["MetricId"] == "BlockReadBytes")
        .unwrap();
    assert_eq!(value["MetricValue"], "4096");
    let property = value["MetricProperty"].as_str().unwrap();
    let (path, pointer) = property.split_once('#').unwrap();
    let (status, _, metrics) = call(&fixture.app, "GET", path, Some(&viewer), "").await;
    assert_eq!(status, StatusCode::OK, "{metrics}");
    assert_eq!(metrics.pointer(pointer).unwrap(), 4096);
    assert_eq!(metrics["Timestamp"], report["Timestamp"]);
    server.await.unwrap();
}

#[tokio::test]
async fn task_and_diagnostic_admission_and_download_keep_role_boundaries() {
    let mut fixture = fixture();
    write_status(&fixture.config, "running", "running");
    fixture.receiver = None; // Unavailable metrics/agent are explicit best-effort/failure paths.
    let admin = good(ADMIN);
    let viewer = good(VIEWER);
    let task_service = "/redfish/v1/TaskService";
    let task_collection = "/redfish/v1/TaskService/Tasks";
    let update = "/redfish/v1/TaskService/Actions/Oem/QrabTaskService.StartGuestUpdate";
    let diagnostic_service = "/redfish/v1/Managers/1/LogServices/Diagnostics";
    let collect =
        "/redfish/v1/Managers/1/LogServices/Diagnostics/Actions/LogService.CollectDiagnosticData";
    for path in [task_service, task_collection, diagnostic_service] {
        assert_eq!(
            call(&fixture.app, "GET", path, None, "").await.0,
            StatusCode::UNAUTHORIZED
        );
        let (status, _, body) = call(&fixture.app, "GET", path, Some(&viewer), "").await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
        assert_eq!(body["@odata.id"], path);
    }
    for (path, body) in [
        (update, r#"{"Args":[]}"#),
        (collect, r#"{"DiagnosticDataType":"Manager"}"#),
    ] {
        assert_eq!(
            call(&fixture.app, "POST", path, None, body).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            call(&fixture.app, "POST", path, Some(&viewer), body)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
    for (path, body) in [
        (update, r#"{"Args":["--unsafe"]}"#),
        (collect, r#"{}"#),
        (collect, r#"{"DiagnosticDataType":"OS"}"#),
    ] {
        assert_eq!(
            call(&fixture.app, "POST", path, Some(&admin), body).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    let (status, headers, task) = call(
        &fixture.app,
        "POST",
        collect,
        Some(&admin),
        r#"{"DiagnosticDataType":"Manager"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{task}");
    assert_eq!(task["TaskState"], "Running");
    let monitor = headers["location"].to_str().unwrap();
    assert_eq!(task["TaskMonitor"], monitor);
    assert_eq!(
        call(&fixture.app, "GET", monitor, None, "").await.0,
        StatusCode::UNAUTHORIZED
    );
    let (entry_headers, entry) = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let (status, headers, body) =
                call(&fixture.app, "GET", monitor, Some(&viewer), "").await;
            if status == StatusCode::CREATED {
                break (headers, body);
            }
            assert_eq!(status, StatusCode::ACCEPTED, "{body}");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        entry_headers["location"].to_str().unwrap(),
        entry["@odata.id"].as_str().unwrap()
    );
    let attachment = entry["AdditionalDataURI"].as_str().unwrap();
    assert_eq!(
        call(&fixture.app, "GET", attachment, None, "").await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&fixture.app, "GET", attachment, Some(&viewer), "")
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, headers, bundle) = call(&fixture.app, "GET", attachment, Some(&admin), "").await;
    assert_eq!(status, StatusCode::OK, "{bundle}");
    assert_eq!(headers["content-type"], "application/json");
    assert!(headers["content-disposition"]
        .to_str()
        .unwrap()
        .contains("attachment"));
    assert_eq!(bundle["redacted"], true);
    assert_eq!(bundle["availability"]["metrics"], false);
    for secret in [PORTAL, "chap-user", "/run/qrab-auth", STATE_ID] {
        assert!(
            !bundle.to_string().contains(secret),
            "diagnostic leaked {secret}"
        );
    }
    let task_uri = task["@odata.id"].as_str().unwrap();
    let (_, _, done) = call(&fixture.app, "GET", task_uri, Some(&viewer), "").await;
    assert_eq!(done["TaskState"], "Completed");
    assert!(done.get("EndTime").is_some());
    assert_eq!(
        done["Links"]["CreatedResources"][0]["@odata.id"],
        entry["@odata.id"]
    );
}

#[tokio::test]
async fn asynchronous_guest_update_reports_unknown_outcome_without_false_success() {
    let mut fixture = fixture();
    fixture.receiver = None;
    let admin = good(ADMIN);
    let update = "/redfish/v1/TaskService/Actions/Oem/QrabTaskService.StartGuestUpdate";
    let (status, headers, task) = call(
        &fixture.app,
        "POST",
        update,
        Some(&admin),
        r#"{"Args":["channel=stable"]}"#,
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{task}");
    let monitor = headers["location"].to_str().unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let (status, _, body) = call(&fixture.app, "GET", monitor, Some(&admin), "").await;
            if status == StatusCode::INTERNAL_SERVER_ERROR {
                assert_error(&body, "GeneralError");
                break;
            }
            assert_eq!(status, StatusCode::ACCEPTED);
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let (_, _, done) = call(
        &fixture.app,
        "GET",
        task["@odata.id"].as_str().unwrap(),
        Some(&good(VIEWER)),
        "",
    )
    .await;
    assert_eq!(done["TaskState"], "Exception");
    assert_eq!(done["TaskStatus"], "Critical");
    assert!(!done.to_string().contains("channel=stable"));
}

#[tokio::test]
async fn subscriptions_require_allowlisted_https_and_keep_receiver_details_admin_only() {
    let mut fixture = fixture();
    let admin = good(ADMIN);
    let viewer = good(VIEWER);
    let collection = "/redfish/v1/EventService/Subscriptions";
    let body = r#"{"Destination":"https://events.example/receiver","Protocol":"Redfish","Context":"operator-context","RegistryPrefixes":["Qrab"],"ResourceTypes":["ComputerSystem"],"Severities":["Critical"]}"#;
    assert_eq!(
        call(&fixture.app, "POST", collection, None, body).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&fixture.app, "POST", collection, Some(&viewer), body)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&fixture.app, "POST", collection, Some(&admin), body)
            .await
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    fixture.app.subscriptions =
        qrab_redfish::SubscriptionStore::new(vec!["https://events.example/receiver".into()], None)
            .unwrap();
    for destination in [
        "http://events.example/receiver",
        "https://user:pass@events.example/receiver",
        "https://events.example/receiver?token=secret",
    ] {
        let body = json!({"Destination":destination,"Protocol":"Redfish"}).to_string();
        assert_eq!(
            call(&fixture.app, "POST", collection, Some(&admin), &body)
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    let unlisted = r#"{"Destination":"https://events.example/different","Protocol":"Redfish"}"#;
    assert_eq!(
        call(&fixture.app, "POST", collection, Some(&admin), unlisted)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let headers = r#"{"Destination":"https://events.example/receiver","Protocol":"Redfish","HttpHeaders":[{"Authorization":"secret"}]}"#;
    assert_eq!(
        call(&fixture.app, "POST", collection, Some(&admin), headers)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let (status, headers, created) =
        call(&fixture.app, "POST", collection, Some(&admin), body).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let uri = headers["location"].to_str().unwrap();
    assert_eq!(created["@odata.id"], uri);
    assert_eq!(created["Destination"], "https://events.example/receiver");
    assert_eq!(created["DeliveryRetryPolicy"], "TerminateAfterRetries");
    let (_, _, visible) = call(&fixture.app, "GET", uri, Some(&viewer), "").await;
    assert!(
        !visible.to_string().contains("events.example")
            && !visible.to_string().contains("operator-context")
    );
    assert_eq!(
        call(&fixture.app, "DELETE", uri, Some(&viewer), "").await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&fixture.app, "DELETE", uri, Some(&admin), "").await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&fixture.app, "GET", uri, Some(&admin), "").await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn event_subscription_capacity_is_bounded_without_evicting_existing_receivers() {
    let mut fixture = fixture();
    fixture.app.subscriptions =
        qrab_redfish::SubscriptionStore::new(vec!["https://events.example/receiver".into()], None)
            .unwrap();
    let admin = good(ADMIN);
    let collection = "/redfish/v1/EventService/Subscriptions";
    let body = r#"{"Destination":"https://events.example/receiver","Protocol":"Redfish"}"#;
    let mut first = String::new();
    for index in 0..32 {
        let (status, headers, reply) =
            call(&fixture.app, "POST", collection, Some(&admin), body).await;
        assert_eq!(status, StatusCode::CREATED, "{reply}");
        if index == 0 {
            first = headers["location"].to_str().unwrap().into();
        }
    }
    let (status, _, error) = call(&fixture.app, "POST", collection, Some(&admin), body).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_error(&error, "ServiceTemporarilyUnavailable");
    assert_eq!(
        call(&fixture.app, "GET", &first, Some(&admin), "").await.0,
        StatusCode::OK
    );
    let (_, _, listing) = call(&fixture.app, "GET", collection, Some(&admin), "").await;
    assert_eq!(listing["Members@odata.count"], 32);
}

#[tokio::test]
async fn numeric_triggers_have_admin_mutations_and_authenticated_crossing_logs() {
    let fixture = fixture();
    let admin = good(ADMIN);
    let viewer = good(VIEWER);
    let collection = "/redfish/v1/TelemetryService/Triggers";
    let body = r#"{"Name":"RAM threshold","MetricProperties":["/redfish/v1/Systems/1/Oem/Qrab/Metrics#/MemoryBytes"],"NumericThresholds":{"UpperCritical":{"Reading":150,"Activation":"Increasing","DwellTime":"PT0S"}},"TriggerActions":["LogToLogService"]}"#;
    assert_eq!(
        call(&fixture.app, "GET", collection, None, "").await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&fixture.app, "POST", collection, Some(&viewer), body)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, headers, created) =
        call(&fixture.app, "POST", collection, Some(&admin), body).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let uri = headers["location"].to_str().unwrap();
    assert_eq!(created["@odata.id"], uri);
    assert_eq!(created["TriggerActions"], json!(["LogToLogService"]));
    assert_eq!(
        call(&fixture.app, "GET", uri, Some(&viewer), "").await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &fixture.app,
            "PATCH",
            uri,
            Some(&viewer),
            r#"{"TriggerEnabled":false}"#
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    fixture.app.triggers.evaluate(&manager::pod::VmMetrics {
        observed_unix_ms: 1,
        memory_bytes: 100,
        ..Default::default()
    });
    fixture.app.triggers.evaluate(&manager::pod::VmMetrics {
        observed_unix_ms: 2,
        memory_bytes: 200,
        ..Default::default()
    });
    let alerts = "/redfish/v1/Managers/1/LogServices/Telemetry/Entries";
    assert_eq!(
        call(&fixture.app, "GET", alerts, None, "").await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, _, entries) = call(&fixture.app, "GET", alerts, Some(&viewer), "").await;
    assert_eq!(status, StatusCode::OK, "{entries}");
    assert_eq!(entries["Members@odata.count"], 1);
    assert_eq!(entries["Members"][0]["Severity"], "Critical");
    assert_eq!(entries["Members"][0]["Oem"]["Qrab"]["Reading"], 200);
    assert_eq!(
        call(
            &fixture.app,
            "PATCH",
            uri,
            Some(&admin),
            r#"{"TriggerActions":["RedfishEvent"]}"#
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &fixture.app,
            "PATCH",
            uri,
            Some(&admin),
            r#"{"TriggerEnabled":false}"#
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&fixture.app, "DELETE", uri, Some(&viewer), "").await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&fixture.app, "DELETE", uri, Some(&admin), "").await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&fixture.app, "GET", uri, Some(&admin), "").await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn native_hardware_and_boot_routes_round_trip_over_the_control_socket() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let mut fixture = fixture();
    let path = fixture.config.runtime_dir.join("hardware.sock");
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    fixture.app.control = manager::pod::Backend::Socket(manager::pod::Remote::new(path));
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut stream = BufReader::new(stream);
        stream.get_mut().write_all(b"{\"protocol\":\"qrab-control\",\"version\":1,\"vm\":\"test\",\"operations\":[\"inventory\",\"agent_boot\",\"agent_boot_next\"]}\n").await.unwrap();
        let hardware = manager::pod::HardwareInventory {
            observed_unix_ms: 1_700_000_000_123,
            processors: vec![manager::pod::ProcessorInventory {
                id: "socket-0".into(),
                socket: Some(0),
                total_threads: 4,
                total_cores: Some(2),
                model: Some("test-x86-cpu".into()),
            }],
            drives: vec![manager::pod::DriveInventory {
                id: "disk-root".into(),
                capacity_bytes: Some(1048576),
                read_only: false,
                removable: false,
                media_present: true,
                encrypted: false,
                format: Some("raw".into()),
                io_failed: false,
                counters: Some(manager::pod::DriveCounters {
                    read_bytes: 4096,
                    write_bytes: 8192,
                    ..Default::default()
                }),
            }],
        };
        let mut boot = manager::pod::BootSnapshot {
            observed_unix_ms: 1_700_000_000_123,
            state: manager::pod::BootState {
                efi_runtime_available: true,
                boot_order: vec!["Boot0001".into()],
                boot_next: None,
                options: vec![manager::pod::BootOption {
                    reference: "Boot0001".into(),
                    description: "Linux".into(),
                    active: true,
                }],
            },
        };
        loop {
            let mut line = String::new();
            if stream.read_line(&mut line).await.unwrap() == 0 {
                break;
            }
            let request: Value = serde_json::from_str(&line).unwrap();
            let result = match request["op"].as_str().unwrap() {
                "inventory" => serde_json::to_value(&hardware).unwrap(),
                "agent_boot" => serde_json::to_value(&boot).unwrap(),
                "agent_boot_next" => {
                    boot.state.boot_next = request["reference"].as_str().map(str::to_owned);
                    serde_json::to_value(&boot).unwrap()
                }
                other => panic!("unexpected control operation {other}"),
            };
            let reply = json!({"v":1,"id":request["id"],"result":result}).to_string() + "\n";
            stream.get_mut().write_all(reply.as_bytes()).await.unwrap();
        }
    });
    let viewer = good(VIEWER);
    let admin = good(ADMIN);
    for path in [
        "/redfish/v1/Systems/1/Processors",
        "/redfish/v1/Systems/1/Processors/socket-0",
        "/redfish/v1/Systems/1/Storage",
        "/redfish/v1/Systems/1/Storage/1",
        "/redfish/v1/Systems/1/Storage/1/Volumes",
        "/redfish/v1/Systems/1/Storage/1/Volumes/disk-root",
        "/redfish/v1/Chassis/1/Drives/disk-root",
        "/redfish/v1/Chassis/1/Drives/disk-root/Metrics",
        "/redfish/v1/Systems/1/BootOptions",
        "/redfish/v1/Systems/1/BootOptions/Boot0001",
    ] {
        let (status, _, body) = call(&fixture.app, "GET", path, Some(&viewer), "").await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
        assert_eq!(body["@odata.id"], path);
    }
    let body = r#"{"Boot":{"BootSourceOverrideEnabled":"Once","BootSourceOverrideTarget":"UefiBootNext","BootNext":"Boot0001"}}"#;
    assert_eq!(
        call(&fixture.app, "PATCH", SYSTEM_PATH, Some(&viewer), body)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&fixture.app, "PATCH", SYSTEM_PATH, Some(&admin), body)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            &fixture.app,
            "PATCH",
            SYSTEM_PATH,
            Some(&admin),
            r#"{"Boot":{"BootSourceOverrideEnabled":"Disabled"}}"#
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    server.abort();
}

#[tokio::test]
async fn serial_history_and_firmware_staging_require_admin() {
    let fixture = fixture();
    let viewer = good(VIEWER);
    let admin = good(ADMIN);
    for path in [
        "/redfish/v1/Managers/1/LogServices/Serial",
        "/redfish/v1/Managers/1/LogServices/Serial/Entries",
        "/redfish/v1/Managers/1/LogServices/Serial/Entries/Tail",
    ] {
        assert_eq!(
            call(&fixture.app, "GET", path, None, "").await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            call(&fixture.app, "GET", path, Some(&viewer), "").await.0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            call(&fixture.app, "GET", path, Some(&admin), "").await.0,
            StatusCode::OK
        );
    }
    let path = "/redfish/v1/UpdateService/Actions/Oem/QrabUpdateService.StageFirmware";
    assert_eq!(
        call(
            &fixture.app,
            "POST",
            path,
            Some(&viewer),
            r#"{"ArtifactId":"approved"}"#
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &fixture.app,
            "POST",
            path,
            Some(&admin),
            r#"{"ArtifactId":"../escape"}"#
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    // The native task is accepted, but no live persistent lease/catalog exists.
    let (status, _, task) = call(
        &fixture.app,
        "POST",
        path,
        Some(&admin),
        r#"{"ArtifactId":"approved"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let path = task["@odata.id"].as_str().unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let (_, _, task) = call(&fixture.app, "GET", path, Some(&admin), "").await;
            if task["TaskState"] == "Exception" {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn admission_is_shared_and_held_through_slow_response_bodies() {
    let mut fixture = fixture();
    fixture.app.requests = Arc::new(Semaphore::new(1));
    let token = good(VIEWER);
    let first = qrab_redfish::router(fixture.app.clone())
        .oneshot(
            Request::builder()
                .uri("/redfish/v1/")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(fixture.app.requests.available_permits(), 0);
    let (status, _, body) =
        call(&fixture.app, "GET", "/redfish/v1/Systems", Some(&token), "").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_error(&body, "ServiceTemporarilyUnavailable");
    let health = qrab_redfish::router(fixture.app.clone())
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);
    drop(first);
    assert_eq!(fixture.app.requests.available_permits(), 1);
    assert_eq!(
        call(&fixture.app, "GET", "/redfish/v1/Systems", Some(&token), "")
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn secure_boot_pending_settings_and_databases_round_trip_without_changing_current_boot() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let mut fixture = fixture();
    write_status(&fixture.config, "running", "running");
    let mut config = fixture.config.clone();
    config.restart_on_exit = true;
    let config_view = manager::pod::ConfigView::new(&config, &Default::default());
    let status: Value =
        serde_json::from_slice(&std::fs::read(config.runtime_dir.join("status.json")).unwrap())
            .unwrap();
    let socket = config.runtime_dir.join("secure.sock");
    let listener = tokio::net::UnixListener::bind(&socket).unwrap();
    fixture.app.control = manager::pod::Backend::Socket(manager::pod::Remote::new(socket));
    let worker = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut stream = BufReader::new(stream);
        stream.get_mut().write_all(b"{\"protocol\":\"qrab-control\",\"version\":1,\"vm\":\"test\",\"operations\":[]}\n").await.unwrap();
        let mut settings = manager::pod::SecureBootSettings {
            supported: true,
            enabled: Some(true),
            current_setting: Some(true),
            ..Default::default()
        };
        loop {
            let mut line = String::new();
            if stream.read_line(&mut line).await.unwrap() == 0 {
                break;
            }
            let request: Value = serde_json::from_str(&line).unwrap();
            let result = match request["op"].as_str().unwrap() {
                "agent_firmware" => serde_json::to_value(manager::pod::FirmwareSnapshot {
                    observed_unix_ms: 1_700_000_000_123,
                    state: manager::pod::FirmwareState {
                        efi_runtime_available: true,
                        secure_boot: Some(true),
                        setup_mode: Some(false),
                        audit_mode: Some(false),
                        deployed_mode: Some(false),
                    },
                })
                .unwrap(),
                "secure_boot_settings" => serde_json::to_value(&settings).unwrap(),
                "stage_secure_boot" => {
                    settings.enabled = request["enabled"].as_bool();
                    settings.pending_cold_boot = true;
                    settings.generation += 1;
                    serde_json::to_value(&settings).unwrap()
                }
                "agent_secure_boot_databases" => {
                    json!({"observed_unix_ms":1_700_000_000_123u64,"efi_runtime_available":true,"databases":[{"id":"db","present":true,"attributes":39,"signatures":[],"unsupported_types":[],"error":null}]})
                }
                "config" => serde_json::to_value(&config_view).unwrap(),
                "status" => status.clone(),
                "action" => {
                    assert_eq!(request["action"], "coldrestart");
                    Value::Null
                }
                other => panic!("unexpected secure operation {other}"),
            };
            let reply = json!({"v":1,"id":request["id"],"result":result}).to_string() + "\n";
            stream.get_mut().write_all(reply.as_bytes()).await.unwrap();
        }
    });
    let admin = good(ADMIN);
    let viewer = good(VIEWER);
    let resource = "/redfish/v1/Systems/1/SecureBoot";
    assert_eq!(
        call(&fixture.app, "GET", resource, None, "").await.0,
        StatusCode::UNAUTHORIZED
    );
    let (_, _, before) = call(&fixture.app, "GET", resource, Some(&viewer), "").await;
    assert_eq!(before["SecureBootCurrentBoot"], "Enabled");
    assert_eq!(before["SecureBootEnable"], true);
    assert_eq!(
        call(
            &fixture.app,
            "PATCH",
            resource,
            Some(&viewer),
            r#"{"SecureBootEnable":false}"#
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &fixture.app,
            "PATCH",
            resource,
            Some(&admin),
            r#"{"SecureBootEnable":"false"}"#
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (code, _, task) = call(
        &fixture.app,
        "PATCH",
        resource,
        Some(&admin),
        r#"{"SecureBootEnable":false}"#,
    )
    .await;
    assert_eq!(code, StatusCode::ACCEPTED);
    let task_uri = task["@odata.id"].as_str().unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let (_, _, task) = call(&fixture.app, "GET", task_uri, Some(&admin), "").await;
            if task["TaskState"] == "Completed" {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let (_, _, after) = call(&fixture.app, "GET", resource, Some(&viewer), "").await;
    assert_eq!(after["SecureBootCurrentBoot"], "Enabled");
    assert_eq!(after["SecureBootEnable"], true);
    assert_eq!(after["Oem"]["Qrab"]["PendingColdBoot"], true);
    let (_, _, pending) = call(
        &fixture.app,
        "GET",
        "/redfish/v1/Systems/1/SecureBoot/Settings",
        Some(&viewer),
        "",
    )
    .await;
    assert_eq!(pending["SecureBootEnable"], false);
    for path in [
        "/redfish/v1/Systems/1/SecureBoot/SecureBootDatabases",
        "/redfish/v1/Systems/1/SecureBoot/SecureBootDatabases/db",
        "/redfish/v1/Systems/1/SecureBoot/SecureBootDatabases/db/Certificates",
        "/redfish/v1/Systems/1/SecureBoot/SecureBootDatabases/db/Signatures",
    ] {
        let (status, _, body) = call(&fixture.app, "GET", path, Some(&viewer), "").await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
    }
    let (_, _, info) = call(
        &fixture.app,
        "GET",
        "/redfish/v1/Systems/1/ResetActionInfo",
        Some(&viewer),
        "",
    )
    .await;
    assert!(info["Parameters"][0]["AllowableValues"]
        .as_array()
        .unwrap()
        .contains(&json!("PowerCycle")));
    assert_eq!(
        call(
            &fixture.app,
            "POST",
            RESET,
            Some(&admin),
            r#"{"ResetType":"PowerCycle"}"#
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    worker.abort();
}

const BMC_ISSUER: &str = "https://idm.example/oauth2/openid/qrab-bmc";
const BMC_AUDIENCE: &str = "qrab-bmc";
const BMC_CLI_AUDIENCE: &str = "qrab-bmc-cli";
const DASHBOARD_ISSUER: &str = "https://idm.example/oauth2/openid/qrab-dashboard";
const DASHBOARD_AUDIENCE: &str = "qrab-dashboard";
const SIGNING_KEY: &str = "test-only-signing-key.pem";
const FOREIGN_KEY: &str = "test-only-foreign-key.pem";

/// The default fixture plus the BMC client (with a second audience on its
/// issuer) and a dashboard client that signs with its own key.
fn trusted_fixture() -> Fixture {
    let mut fixture = fixture();
    let keys = |name: &str| CoreJsonWebKeySet::new(vec![key(name).as_verification_key()]);
    let authenticator = auth::Authenticator::new(
        IssuerUrl::new(ISSUER.into()).unwrap(),
        ClientId::new(AUDIENCE.into()),
        keys(SIGNING_KEY),
        [ADMIN.to_string()].into_iter().collect(),
        [VIEWER.to_string()].into_iter().collect(),
    )
    .with_groups(
        [ADMIN_GROUP.to_string()].into_iter().collect(),
        [VIEWER_GROUP.to_string()].into_iter().collect(),
    )
    .trusting(
        oidc_bearer::Verifier::new(
            IssuerUrl::new(BMC_ISSUER.into()).unwrap(),
            ClientId::new(BMC_AUDIENCE.into()),
            keys(SIGNING_KEY),
        )
        .with_audience(ClientId::new(BMC_CLI_AUDIENCE.into())),
    )
    .trusting(oidc_bearer::Verifier::new(
        IssuerUrl::new(DASHBOARD_ISSUER.into()).unwrap(),
        ClientId::new(DASHBOARD_AUDIENCE.into()),
        keys(FOREIGN_KEY),
    ));
    fixture.app.auth = Arc::new(authenticator);
    fixture
}

/// (signing key, issuer, audience) of every trusted pair in `trusted_fixture`.
const TRUSTED: [(&str, &str, &str); 4] = [
    (SIGNING_KEY, ISSUER, AUDIENCE),
    (SIGNING_KEY, BMC_ISSUER, BMC_AUDIENCE),
    (SIGNING_KEY, BMC_ISSUER, BMC_CLI_AUDIENCE),
    (FOREIGN_KEY, DASHBOARD_ISSUER, DASHBOARD_AUDIENCE),
];

fn claims(issuer: &str, audience: Value, subject: &str, groups: &[&str]) -> Value {
    let now = auth::now() as i64;
    json!({
        "iss": issuer, "aud": audience, "sub": subject, "iat": now - 5, "exp": now + 300,
        "groups": groups
    })
}

async fn assert_unauthenticated(app: &AppState, token: &str) {
    let (status, headers, body) = call(app, "GET", SYSTEM_PATH, Some(token), "").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{token}");
    assert!(headers["www-authenticate"]
        .to_str()
        .unwrap()
        .starts_with("Bearer"));
    assert_error(&body, "NoValidSession");
}

#[tokio::test]
async fn trusted_token_sources_are_selected_by_issuer_and_grant_only_listed_roles() {
    let fixture = trusted_fixture();
    let app = &fixture.app;
    write_status(&fixture.config, "running", "running");
    let reset = r#"{"ResetType":"On"}"#;
    // Every pair is accepted, checked only by its own issuer's verifier, and
    // maps to the role its subject or groups grant.
    for (signer, issuer, audience) in TRUSTED {
        let token = |subject: &str, groups: &[&str]| {
            signed(signer, claims(issuer, json!([audience]), subject, groups))
        };
        let viewer = token(VIEWER, &[]);
        assert_eq!(
            app.auth.verifier_for(&viewer).map(|v| v.issuer()),
            Some(issuer)
        );
        assert_eq!(
            call(app, "GET", SYSTEM_PATH, Some(&viewer), "").await.0,
            StatusCode::OK,
            "{issuer} {audience}"
        );
        assert_eq!(
            call(app, "POST", RESET, Some(&viewer), reset).await.0,
            StatusCode::FORBIDDEN
        );
        for admin in [token(ADMIN, &[]), token(UNLISTED, &[ADMIN_GROUP])] {
            assert_eq!(
                call(app, "POST", RESET, Some(&admin), reset).await.0,
                StatusCode::NO_CONTENT,
                "{issuer} {audience}"
            );
        }
        let principal = app
            .auth
            .authenticate(Some(&format!(
                "Bearer {}",
                token(UNLISTED, &[VIEWER_GROUP])
            )))
            .await
            .unwrap();
        assert_eq!(principal.role, auth::Role::Viewer);
        // A trusted source grants nothing by itself.
        for unlisted in [
            token(UNLISTED, &[]),
            token(UNLISTED, &["ffffffff-ffff-4fff-8fff-ffffffffffff"]),
        ] {
            let (status, _, body) = call(app, "GET", SYSTEM_PATH, Some(&unlisted), "").await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{issuer} {audience}");
            assert_error(&body, "InsufficientPrivilege");
        }
    }
    // A trusted issuer with another source's audience is not a trusted pair,
    // alone or next to its own audience, with or without `azp`.
    for (signer, issuer, audience) in [
        (SIGNING_KEY, ISSUER, json!([BMC_AUDIENCE])),
        (SIGNING_KEY, ISSUER, json!(BMC_CLI_AUDIENCE)),
        (SIGNING_KEY, BMC_ISSUER, json!([AUDIENCE])),
        (SIGNING_KEY, BMC_ISSUER, json!([DASHBOARD_AUDIENCE])),
        (FOREIGN_KEY, DASHBOARD_ISSUER, json!([BMC_AUDIENCE])),
        (SIGNING_KEY, BMC_ISSUER, json!([BMC_AUDIENCE, AUDIENCE])),
        (
            SIGNING_KEY,
            BMC_ISSUER,
            json!([BMC_AUDIENCE, BMC_CLI_AUDIENCE]),
        ),
    ] {
        assert_unauthenticated(app, &signed(signer, claims(issuer, audience, ADMIN, &[]))).await;
    }
    for (audience, azp) in [
        (json!([BMC_AUDIENCE, AUDIENCE]), BMC_AUDIENCE),
        (json!([BMC_AUDIENCE]), AUDIENCE),
        (json!([BMC_AUDIENCE]), BMC_CLI_AUDIENCE),
    ] {
        let mut claims = claims(BMC_ISSUER, audience, ADMIN, &[]);
        claims["azp"] = json!(azp);
        assert_unauthenticated(app, &signed(SIGNING_KEY, claims)).await;
    }
    // An unconfigured issuer has no verifier, even when a trusted source's
    // key signed it for a trusted audience.
    for audience in [AUDIENCE, BMC_AUDIENCE, DASHBOARD_AUDIENCE] {
        let token = signed(
            SIGNING_KEY,
            claims(
                "https://idm.example/oauth2/openid/qrab-other",
                json!([audience]),
                ADMIN,
                &[],
            ),
        );
        assert!(app.auth.verifier_for(&token).is_none());
        assert_unauthenticated(app, &token).await;
    }
    // Only the issuer's own keys are consulted: another source's key never
    // signs for it.
    for (signer, issuer, audience) in [
        (SIGNING_KEY, DASHBOARD_ISSUER, DASHBOARD_AUDIENCE),
        (FOREIGN_KEY, ISSUER, AUDIENCE),
        (FOREIGN_KEY, BMC_ISSUER, BMC_AUDIENCE),
    ] {
        let token = signed(signer, claims(issuer, json!([audience]), ADMIN, &[]));
        assert_eq!(
            app.auth.verifier_for(&token).map(|v| v.issuer()),
            Some(issuer)
        );
        assert_unauthenticated(app, &token).await;
    }
}

const GUEST_MAC: &str = "02:00:00:00:00:01";
const GUEST_IP: &str = "198.51.100.7";
const RELAY: &str = "10.203.0.40:7700";

async fn scrape(
    app: &AppState,
    method: &str,
    path: &str,
) -> (StatusCode, axum::http::HeaderMap, String) {
    let response = qrab_redfish::metrics_router(app.clone())
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, headers, String::from_utf8(bytes.to_vec()).unwrap())
}

type Sample = (String, Vec<(String, String)>, f64);

/// A strict reader for the Prometheus text format subset the exporter emits:
/// every line is HELP, TYPE or a sample of a family already typed.
fn parse_exposition(text: &str) -> Vec<Sample> {
    fn metric_name(name: &str) -> bool {
        let mut bytes = name.bytes();
        bytes
            .next()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_' || b == b':')
            && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b':')
    }
    assert!(text.ends_with('\n'), "exposition ends with a newline");
    let mut types = std::collections::BTreeMap::new();
    let mut helped = std::collections::BTreeSet::new();
    let mut samples = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# HELP ") {
            let (name, help) = rest.split_once(' ').expect(line);
            assert!(metric_name(name) && !help.is_empty(), "{line}");
            assert!(helped.insert(name.to_owned()), "duplicate HELP: {line}");
            continue;
        }
        if let Some(rest) = line.strip_prefix("# TYPE ") {
            let (name, kind) = rest.split_once(' ').expect(line);
            assert!(metric_name(name) && helped.contains(name), "{line}");
            assert!(matches!(kind, "gauge" | "counter"), "{line}");
            assert!(
                types.insert(name.to_owned(), kind.to_owned()).is_none(),
                "{line}"
            );
            continue;
        }
        assert!(
            !line.starts_with('#') && !line.is_empty(),
            "unexpected line {line:?}"
        );
        let (series, value) = line.rsplit_once(' ').expect(line);
        let value: f64 = value.parse().expect(line);
        let (name, labels) = match series.split_once('{') {
            None => (series, Vec::new()),
            Some((name, rest)) => {
                let body = rest.strip_suffix('}').expect(line);
                let mut labels = Vec::new();
                let mut chars = body.chars().peekable();
                while chars.peek().is_some() {
                    let key: String = chars.by_ref().take_while(|c| *c != '=').collect();
                    assert!(metric_name(&key) && !key.contains(':'), "{line}");
                    assert_eq!(chars.next(), Some('"'), "{line}");
                    let mut value = String::new();
                    loop {
                        match chars.next().expect(line) {
                            '"' => break,
                            '\\' => match chars.next().expect(line) {
                                'n' => value.push('\n'),
                                c @ ('\\' | '"') => value.push(c),
                                other => panic!("bad escape {other} in {line}"),
                            },
                            c => value.push(c),
                        }
                    }
                    labels.push((key, value));
                    match chars.next() {
                        None => break,
                        Some(',') => {}
                        Some(other) => panic!("unexpected {other} in {line}"),
                    }
                }
                (name, labels)
            }
        };
        let kind = types
            .get(name)
            .unwrap_or_else(|| panic!("untyped sample {line}"));
        if kind == "counter" {
            assert!(name.ends_with("_total"), "{line}");
            assert!(value >= 0.0, "{line}");
        }
        samples.push((name.to_owned(), labels, value));
    }
    samples
}

fn sensitive(value: &str) -> bool {
    let mac = |sep: char| {
        let parts: Vec<_> = value.split(sep).collect();
        parts.len() == 6
            && parts
                .iter()
                .all(|p| p.len() == 2 && p.bytes().all(|b| b.is_ascii_hexdigit()))
    };
    value.contains('/')
        || value.contains(':')
        || ["iqn.", "eui.", "naa."]
            .iter()
            .any(|p| value.to_ascii_lowercase().starts_with(p))
        || value.parse::<std::net::IpAddr>().is_ok()
        || mac(':')
        || mac('-')
}

/// The Grafana dashboard's queries, read from the shipped ConfigMap.
fn dashboard_exprs() -> Vec<String> {
    let yaml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contrib/kubernetes/monitoring/grafana-dashboard.yaml"),
    )
    .unwrap();
    let mut lines = yaml
        .lines()
        .skip_while(|l| !l.trim_end().ends_with("qrab-vm.json: |"));
    let key = lines.next().expect("qrab-vm.json block");
    let indent = key.len() - key.trim_start().len();
    let json: String = lines
        .take_while(|l| l.trim().is_empty() || l.len() - l.trim_start().len() > indent)
        .map(|l| format!("{}\n", l.trim_start()))
        .collect();
    let dashboard: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(dashboard["uid"], "qrab-vm");
    fn walk(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (key, value) in map {
                    match (key.as_str(), value) {
                        ("expr", Value::String(expr)) => out.push(expr.clone()),
                        ("definition", Value::String(expr)) => out.push(expr.clone()),
                        _ => walk(value, out),
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|v| walk(v, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(&dashboard, &mut out);
    out
}

fn label_set<'a>(samples: &'a [Sample], key: &str) -> std::collections::BTreeSet<&'a str> {
    samples
        .iter()
        .flat_map(|(_, labels, _)| labels.iter())
        .filter(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .collect()
}

#[tokio::test]
async fn prometheus_scrape_is_valid_sanitized_viewer_data_and_coalesced() {
    use std::collections::{BTreeMap, BTreeSet};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let mut fixture = fixture();
    // The full admin view still carries the iSCSI portal/IQN and NIC relay.
    let mut view = fixture.app.control.config_view().await.unwrap();
    view.nics.push(
        serde_json::from_value(json!({"id":"lan","mac":GUEST_MAC,"endpoint":RELAY})).unwrap(),
    );
    view.nics.push(
        serde_json::from_value(json!({"id":"02-00-00-00-00-09","mac":"02:00:00:00:00:09"}))
            .unwrap(),
    );
    assert!(serde_json::to_string(&view).unwrap().contains(PORTAL));
    let path = fixture.config.runtime_dir.join("prometheus.sock");
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    fixture.app.control = manager::pod::Backend::Socket(manager::pod::Remote::new(path));
    let drive = |id: &str, read_bytes| manager::pod::DriveInventory {
        id: id.into(),
        capacity_bytes: Some(1 << 30),
        read_only: false,
        removable: false,
        media_present: true,
        encrypted: false,
        format: Some("raw".into()),
        io_failed: false,
        counters: Some(manager::pod::DriveCounters {
            read_bytes,
            write_bytes: 8192,
            read_operations: 3,
            write_operations: 4,
            flush_operations: 1,
        }),
    };
    let interface = |name: &str, mac: Option<&str>| manager::pod::NetworkInterface {
        name: name.into(),
        hardware_address: mac.map(str::to_owned),
        ip_addresses: vec![manager::pod::NetworkAddress {
            address: GUEST_IP.into(),
            prefix: 24,
        }],
        statistics: Some(manager::pod::NetworkStatistics {
            rx_bytes: 1000,
            rx_packets: 10,
            tx_bytes: 2000,
            tx_packets: 20,
            ..Default::default()
        }),
        interface_enabled: Some(true),
        link_up: Some(true),
    };
    let replies: BTreeMap<&str, Value> = [
        (
            "status",
            json!({"phase":"running","ready":true,"manager_pid":1,"manager_start_ticks":1,
            "qemu_pid":2,"qemu_start_ticks":2,"qemu_version":[11,1,3],"run_state":"running",
            "observed_unix_ms":0,"exit_code":null,"detail":"/run/qrab/private"}),
        ),
        (
            "agent_status",
            serde_json::to_value(manager::pod::AgentStatus {
                enabled: true,
                connected: true,
                version: Some("1.0".into()),
                ..Default::default()
            })
            .unwrap(),
        ),
        (
            "metrics",
            serde_json::to_value(manager::pod::VmMetrics {
                observed_unix_ms: 1_700_000_000_123,
                logical_processors: 4,
                memory_bytes: 3_221_225_472,
                ..Default::default()
            })
            .unwrap(),
        ),
        (
            "inventory",
            serde_json::to_value(manager::pod::HardwareInventory {
                observed_unix_ms: 1_700_000_000_123,
                processors: vec![],
                drives: vec![
                    drive("disk-root", 4096),
                    drive("iqn.2026-11.com.example:leak", 1),
                ],
            })
            .unwrap(),
        ),
        ("config", serde_json::to_value(&view).unwrap()),
        (
            "agent_network",
            serde_json::to_value(manager::pod::GuestNetworkSnapshot {
                observed_unix_ms: 1_700_000_000_123,
                interfaces: vec![
                    interface("enp1s0", Some(&GUEST_MAC.to_ascii_uppercase())),
                    interface("lo", None),
                ],
            })
            .unwrap(),
        ),
    ]
    .into_iter()
    .collect();
    let replies = Arc::new(replies);
    let counts = Arc::new(std::sync::Mutex::new(BTreeMap::<String, u64>::new()));
    let server = {
        let counts = counts.clone();
        tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let (counts, replies) = (counts.clone(), replies.clone());
                tokio::spawn(async move {
                    let mut stream = BufReader::new(stream);
                    let hello = json!({"protocol":"qrab-control","version":1,"vm":"test",
                        "operations":replies.keys().collect::<Vec<_>>()})
                    .to_string()
                        + "\n";
                    stream.get_mut().write_all(hello.as_bytes()).await.unwrap();
                    loop {
                        let mut line = String::new();
                        if stream.read_line(&mut line).await.unwrap() == 0 {
                            return;
                        }
                        let request: Value = serde_json::from_str(&line).unwrap();
                        let op = request["op"].as_str().unwrap().to_owned();
                        let result = replies
                            .get(op.as_str())
                            .unwrap_or_else(|| panic!("unexpected {op}"))
                            .clone();
                        *counts.lock().unwrap().entry(op).or_default() += 1;
                        let reply =
                            json!({"v":1,"id":request["id"],"result":result}).to_string() + "\n";
                        stream.get_mut().write_all(reply.as_bytes()).await.unwrap();
                    }
                });
            }
        })
    };

    let (status, headers, text) = scrape(&fixture.app, "GET", "/metrics").await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert_eq!(
        headers["content-type"],
        qrab_redfish::prometheus::CONTENT_TYPE
    );
    let (_, _, again) = scrape(&fixture.app, "GET", "/metrics").await;
    assert_eq!(
        again, text,
        "a scrape inside the window reuses the collection"
    );
    {
        let counts = counts.lock().unwrap();
        assert_eq!(counts.get("metrics"), Some(&1), "{counts:?}");
        assert_eq!(counts.len(), 6, "{counts:?}");
        assert!(counts.values().all(|n| *n == 1), "{counts:?}");
    }
    // The scrape's sample feeds the shared on-request telemetry history.
    let (status, _, reports) = call(
        &fixture.app,
        "GET",
        "/redfish/v1/TelemetryService/MetricReports",
        Some(&good(VIEWER)),
        "",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reports["Members@odata.count"], 2, "{reports}");

    let samples = parse_exposition(&text);
    let value = |name: &str, label: Option<(&str, &str)>| {
        samples
            .iter()
            .find(|(n, labels, _)| {
                n == name
                    && label.is_none_or(|(k, v)| labels.iter().any(|(lk, lv)| lk == k && lv == v))
            })
            .map(|(_, _, v)| *v)
    };
    let families: BTreeSet<_> = samples.iter().map(|(n, _, _)| n.as_str()).collect();
    for family in [
        "qrab_redfish_build_info",
        "qrab_vm_power_state",
        "qrab_vm_health",
        "qrab_vm_vcpus",
        "qrab_vm_memory_configured_bytes",
        "qrab_vm_block_read_bytes_total",
        "qrab_vm_block_write_bytes_total",
        "qrab_vm_block_read_ops_total",
        "qrab_vm_block_write_ops_total",
        "qrab_vm_nic_rx_bytes_total",
        "qrab_vm_nic_tx_bytes_total",
        "qrab_vm_nic_rx_packets_total",
        "qrab_vm_nic_tx_packets_total",
        "qrab_vm_guest_agent_connected",
        "qrab_vm_qmp_events_dropped_total",
    ] {
        assert!(families.contains(family), "missing {family}:\n{text}");
    }
    assert_eq!(
        value("qrab_vm_power_state", Some(("state", "On"))),
        Some(1.0)
    );
    assert_eq!(
        value("qrab_vm_power_state", Some(("state", "Off"))),
        Some(0.0)
    );
    assert_eq!(value("qrab_vm_health", None), Some(0.0));
    assert_eq!(value("qrab_vm_vcpus", None), Some(4.0));
    assert_eq!(
        value("qrab_vm_memory_configured_bytes", None),
        Some(3_221_225_472.0)
    );
    assert_eq!(
        value(
            "qrab_vm_block_read_bytes_total",
            Some(("drive", "disk-root"))
        ),
        Some(4096.0)
    );
    assert_eq!(
        value("qrab_vm_nic_tx_bytes_total", Some(("nic", "lan"))),
        Some(2000.0)
    );
    assert_eq!(value("qrab_vm_guest_agent_connected", None), Some(1.0));
    // Only configured, safe drive and NIC ids become label values; the
    // PodMonitor owns the vm/pod/instance target labels.
    for (name, labels, _) in &samples {
        for (key, value) in labels {
            assert!(
                !matches!(key.as_str(), "vm" | "pod" | "instance" | "namespace"),
                "{name}"
            );
            assert!(!sensitive(value), "{name}{{{key}={value:?}}}");
        }
    }
    assert_eq!(label_set(&samples, "drive"), BTreeSet::from(["disk-root"]));
    assert_eq!(label_set(&samples, "nic"), BTreeSet::from(["lan"]));
    let lower = text.to_ascii_lowercase();
    for secret in [
        PORTAL,
        "iqn.",
        GUEST_MAC,
        "02:00:00:00:00:09",
        GUEST_IP,
        RELAY,
        "enp1s0",
        "/run/",
    ] {
        assert!(
            !lower.contains(&secret.to_ascii_lowercase()),
            "{secret} leaked:\n{text}"
        );
    }

    // The shipped Grafana dashboard queries only exported families or cAdvisor.
    let exprs = dashboard_exprs();
    let mut referenced = BTreeSet::new();
    for expr in &exprs {
        let mut rest = expr.as_str();
        while let Some(at) = rest.find("qrab_") {
            let name: String = rest[at..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            rest = &rest[at + name.len()..];
            referenced.insert(name);
        }
    }
    assert!(referenced.contains("qrab_vm_power_state"), "{referenced:?}");
    for name in &referenced {
        assert!(
            families.contains(name.as_str()),
            "dashboard queries unknown {name}"
        );
    }
    for cadvisor in [
        "container_cpu_usage_seconds_total",
        "container_memory_working_set_bytes",
    ] {
        assert!(exprs.iter().any(|e| e.contains(cadvisor)), "{cadvisor}");
    }
    server.abort();
}

#[tokio::test]
async fn metrics_listener_serves_only_metrics_and_the_api_never_does() {
    let mut fixture = fixture();
    fixture.receiver = None; // A down manager still yields a valid, partial exposition.
    let (status, _, text) = scrape(&fixture.app, "GET", "/metrics").await;
    assert_eq!(status, StatusCode::OK);
    let samples = parse_exposition(&text);
    assert!(samples.iter().any(|(n, _, _)| n == "qrab_vm_power_state"));
    assert!(
        !samples.iter().any(|(n, _, _)| n == "qrab_vm_vcpus"),
        "no invented zeroes"
    );
    for path in [
        "/",
        "/redfish",
        "/redfish/v1/",
        "/healthz",
        "/readyz",
        "/metrics/",
        "/redfish/v1/Systems/1/Oem/Qrab/Metrics",
    ] {
        assert_eq!(
            scrape(&fixture.app, "GET", path).await.0,
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
    for method in ["POST", "PUT", "DELETE", "PATCH"] {
        assert_eq!(
            scrape(&fixture.app, method, "/metrics").await.0,
            StatusCode::METHOD_NOT_ALLOWED,
            "{method}"
        );
    }
    for token in [None, Some(good(ADMIN))] {
        let (status, _, body) = call(&fixture.app, "GET", "/metrics", token.as_deref(), "").await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_error(&body, "ResourceNotFound");
    }
}

#[tokio::test]
async fn optional_public_discovery_never_downgrades_bad_credentials() {
    let mut fixture = fixture();
    let auth = Arc::try_unwrap(fixture.app.auth).ok().unwrap();
    fixture.app.auth = Arc::new(auth.with_public_discovery(true));
    for path in ["/redfish", "/redfish/v1", "/redfish/v1/"] {
        let (status, _, value) = call(&fixture.app, "GET", path, None, "").await;
        assert_eq!(status, StatusCode::OK, "{value}");
        assert!(value.get("UUID").is_none());
        assert!(!value.to_string().contains(STATE_ID));
        assert_eq!(
            call(&fixture.app, "GET", path, Some("bad"), "").await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        call(&fixture.app, "GET", "/redfish/v1/Systems/1", None, "")
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let (_, _, root) = call(&fixture.app, "GET", "/redfish/v1/", Some(&good(ADMIN)), "").await;
    assert_eq!(root["UUID"], STATE_ID);
    assert_eq!(
        root["Links"]["Sessions"]["@odata.id"],
        "/redfish/v1/SessionService/Sessions"
    );
}

/// Manual independent-client target. It is the real router/auth implementation
/// with a deterministic manager fixture, not a booted VM or conformance claim.
/// Bind only loopback, use ephemeral test credentials, and keep it time bounded.
#[tokio::test]
#[ignore = "explicit loopback target for independent Redfish clients"]
async fn independent_client_fixture_server() {
    let mut fixture = fixture();
    write_status(&fixture.config, "running", "running");
    drop(fixture.receiver.take());
    let accounts =
        std::env::var("REDFISH_LOCAL_ACCOUNTS_FILE").expect("ephemeral test account file required");
    let auth = Arc::try_unwrap(fixture.app.auth)
        .ok()
        .unwrap()
        .with_local_accounts(std::path::Path::new(&accounts))
        .unwrap()
        .with_public_discovery(true);
    fixture.app.auth = Arc::new(auth);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:18881")
        .await
        .unwrap();
    println!("Independent Redfish fixture listening on http://127.0.0.1:18881 (600 seconds)");
    axum::serve(listener, qrab_redfish::router(fixture.app.clone()))
        .with_graceful_shutdown(async { tokio::time::sleep(Duration::from_secs(600)).await })
        .await
        .unwrap();
}

async fn session_call(
    app: &AppState,
    method: &str,
    path: &str,
    session: &str,
) -> (StatusCode, Value) {
    let response = qrab_redfish::router(app.clone())
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("x-auth-token", session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value)
}

#[tokio::test]
async fn oidc_sessions_preserve_roles_are_revocable_and_never_escape_in_json() {
    let fixture = fixture();
    let mut sessions = Vec::new();
    for subject in [ADMIN, VIEWER] {
        let (status, headers, body) = call(
            &fixture.app,
            "POST",
            qrab_redfish::sessions::OIDC_CREATE,
            Some(&good(subject)),
            "{}",
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let token = headers["x-auth-token"].to_str().unwrap().to_owned();
        let location = headers["location"].to_str().unwrap().to_owned();
        assert!(!body.to_string().contains(&token));
        assert!(!location.contains(&token));
        assert_eq!(
            session_call(&fixture.app, "GET", "/redfish/v1/Systems/1", &token)
                .await
                .0,
            StatusCode::OK
        );
        sessions.push((token, location));
    }
    let (viewer, location) = &sessions[1];
    assert_eq!(
        session_call(
            &fixture.app,
            "POST",
            "/redfish/v1/Systems/1/Actions/ComputerSystem.Reset",
            viewer
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        session_call(&fixture.app, "DELETE", &sessions[0].1, viewer)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        session_call(&fixture.app, "DELETE", location, viewer)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        session_call(&fixture.app, "GET", "/redfish/v1/Systems/1", viewer)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        session_call(&fixture.app, "GET", "/redfish/v1/Systems/1", &sessions[0].0)
            .await
            .0,
        StatusCode::OK
    );
    let mixed = qrab_redfish::router(fixture.app.clone())
        .oneshot(
            Request::builder()
                .uri("/redfish/v1/Systems/1")
                .header("x-auth-token", &sessions[0].0)
                .header("authorization", format!("Bearer {}", good(ADMIN)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mixed.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn revoked_session_closes_an_already_admitted_idle_event_stream() {
    let fixture = fixture();
    let (status, headers, _) = call(
        &fixture.app,
        "POST",
        qrab_redfish::sessions::OIDC_CREATE,
        Some(&good(VIEWER)),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let token = headers["x-auth-token"].to_str().unwrap();
    let location = headers["location"].to_str().unwrap();
    let response = qrab_redfish::router(fixture.app.clone())
        .oneshot(
            Request::builder()
                .uri("/redfish/v1/EventService/SSE")
                .header("x-auth-token", token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    // Begin polling before revocation, so this checks ongoing authorization.
    let reader = tokio::spawn(async move {
        axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap()
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        session_call(&fixture.app, "DELETE", location, token)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    tokio::time::timeout(Duration::from_secs(3), reader)
        .await
        .expect("revoked idle SSE must close promptly")
        .unwrap();
    assert_eq!(fixture.app.streams.available_permits(), 2);
}

#[tokio::test]
async fn protocol_discovery_and_negotiation_are_explicit_and_auth_stays_required() {
    let mut fixture = fixture();
    let auth = Arc::try_unwrap(fixture.app.auth).ok().unwrap();
    fixture.app.auth = Arc::new(auth.with_public_discovery(true));
    for path in ["/redfish/v1/AccountService", "/redfish/v1/SessionService"] {
        assert_eq!(
            call(&fixture.app, "GET", path, None, "").await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    let (_, headers, root) = call(&fixture.app, "GET", "/redfish/v1/", None, "").await;
    assert_eq!(headers["allow"], "GET, HEAD");
    assert_eq!(
        headers["link"],
        "<https://redfish.dmtf.org/schemas/v1/ServiceRoot.v1_17_0.json>; rel=describedby"
    );
    assert_eq!(root["Tasks"]["@odata.id"], "/redfish/v1/TaskService");
    let (_, headers, _) = call(
        &fixture.app,
        "GET",
        "/redfish/v1/Systems/1",
        Some(&good(ADMIN)),
        "",
    )
    .await;
    assert_eq!(headers["allow"], "GET, HEAD, PATCH");
    let response = qrab_redfish::router(fixture.app.clone())
        .oneshot(
            Request::builder()
                .uri("/redfish/v1/")
                .header("odata-version", "4.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);
    assert_eq!(response.headers()["odata-version"], "4.0");
    let response = qrab_redfish::router(fixture.app.clone())
        .oneshot(
            Request::builder()
                .uri("/redfish/v1/$metadata")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("application/xml"));
    let xml = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    assert!(std::str::from_utf8(&xml)
        .unwrap()
        .contains("ServiceRoot.v1_17_0.ServiceRoot"));
    let (status, _, odata) = call(&fixture.app, "GET", "/redfish/v1/odata", None, "").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!odata.to_string().contains(STATE_ID));
    assert_eq!(
        call(
            &fixture.app,
            "GET",
            "/redfish/v1/$metadata",
            Some("invalid"),
            ""
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn malformed_session_creation_is_redfish_json_and_alias_uses_same_validation() {
    let fixture = fixture();
    for path in [
        qrab_redfish::sessions::COLLECTION,
        "/redfish/v1/SessionService/Sessions/Members",
    ] {
        let (status, _, error) = call(&fixture.app, "POST", path, None, "{}").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(error["error"]["code"]
            .as_str()
            .unwrap()
            .starts_with("Base."));
        let (status, _, error) = call(
            &fixture.app,
            "POST",
            path,
            None,
            r#"{"UserName":"absent","Password":"wrong"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(!error.to_string().contains("wrong"));
    }
}

#[tokio::test]
async fn oidc_session_cannot_extend_the_originating_id_tokens_lifetime() {
    let fixture = fixture();
    let short = token("test-only-signing-key.pem", ISSUER, AUDIENCE, ADMIN, 2);
    let (status, headers, body) = call(
        &fixture.app,
        "POST",
        qrab_redfish::sessions::OIDC_CREATE,
        Some(&short),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let expiry = chrono::DateTime::parse_from_rfc3339(body["ExpirationTime"].as_str().unwrap())
        .unwrap()
        .timestamp();
    assert!(expiry <= auth::now() as i64 + 2);
    let session = headers["x-auth-token"].to_str().unwrap();
    assert_eq!(
        session_call(&fixture.app, "GET", "/redfish/v1/Systems/1", session)
            .await
            .0,
        StatusCode::OK
    );
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert_eq!(
        session_call(&fixture.app, "GET", "/redfish/v1/Systems/1", session)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn event_discovery_advertises_push_only_when_push_delivery_is_enabled() {
    let mut fixture = fixture();
    let (_, _, disabled) = call(
        &fixture.app,
        "GET",
        "/redfish/v1/EventService",
        Some(&good(VIEWER)),
        "",
    )
    .await;
    assert!(disabled.get("Subscriptions").is_none());
    assert_eq!(
        disabled["ServerSentEventUri"],
        "/redfish/v1/EventService/SSE"
    );
    fixture.app.subscriptions =
        qrab_redfish::SubscriptionStore::new(vec!["https://events.example/receiver".into()], None)
            .unwrap();
    let (_, _, enabled) = call(
        &fixture.app,
        "GET",
        "/redfish/v1/EventService",
        Some(&good(VIEWER)),
        "",
    )
    .await;
    assert_eq!(
        enabled["Subscriptions"]["@odata.id"],
        "/redfish/v1/EventService/Subscriptions"
    );
}

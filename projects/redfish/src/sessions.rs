//! In-memory, bounded, revocable Redfish sessions. Session bearer material is
//! returned exactly once in X-Auth-Token; only its SHA-256 digest is retained.
use crate::{
    auth::{self, Principal, Rejection, Role},
    collection, link, principal, AppState, Failure, Json,
};
use axum::{
    extract::{rejection::JsonRejection, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json as InputJson, Router,
};
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Mutex, time::Instant};

pub const SERVICE: &str = "/redfish/v1/SessionService";
pub const COLLECTION: &str = "/redfish/v1/SessionService/Sessions";
pub const OIDC_CREATE: &str =
    "/redfish/v1/SessionService/Actions/Oem/QrabSessionService.CreateFromOIDC";
const TIMEOUT: u64 = 600;
const CAPACITY: usize = 64;

#[derive(Default)]
pub(crate) struct Store(Mutex<HashMap<[u8; 32], Session>>);
struct Session {
    id: String,
    principal: Principal,
    last_used: Instant,
    absolute: u64,
}
impl Session {
    fn valid(&self) -> bool {
        auth::now() < self.absolute && self.last_used.elapsed().as_secs() < TIMEOUT
    }
    fn view(&self) -> Value {
        json!({"@odata.id":format!("{COLLECTION}/{}",self.id),"@odata.type":"#Session.v1_8_0.Session",
            "Id":self.id,"Name":"User Session","UserName":if self.principal.issuer.is_none() { self.principal.subject.strip_prefix("local:").unwrap_or(&self.principal.subject) } else {&self.principal.subject},
            "SessionType":"Redfish", "ExpirationTime":i64::try_from(self.absolute).ok().and_then(|t| chrono::DateTime::from_timestamp(t, 0)).map(|d|d.to_rfc3339())})
    }
}
impl Store {
    fn create(
        &self,
        mut principal: Principal,
        absolute: u64,
    ) -> Result<(String, String, Value), Failure> {
        let mut map = self
            .0
            .lock()
            .map_err(|_| Failure::unavailable("Session store unavailable."))?;
        map.retain(|_, s| s.valid());
        if map.len() >= CAPACITY {
            return Err(Failure::unavailable("Session capacity reached."));
        }
        let mut secret = [0u8; 32];
        getrandom::fill(&mut secret)
            .map_err(|_| Failure::unavailable("Session entropy unavailable."))?;
        let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(secret);
        let mut id_bytes = [0u8; 16];
        getrandom::fill(&mut id_bytes)
            .map_err(|_| Failure::unavailable("Session entropy unavailable."))?;
        let id = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(id_bytes);
        principal.session_id = Some(id.clone());
        let session = Session {
            id: id.clone(),
            principal,
            last_used: Instant::now(),
            absolute,
        };
        let view = session.view();
        let digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        map.insert(digest, session);
        Ok((token, format!("{COLLECTION}/{id}"), view))
    }
    pub(crate) fn authenticate(&self, token: &str) -> Result<Principal, Rejection> {
        if token.len() != 43
            || !token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
            return Err(Rejection::Unauthenticated);
        }
        let key: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        let mut map = self.0.lock().map_err(|_| Rejection::Unauthenticated)?;
        map.retain(|_, s| s.valid());
        let session = map.get_mut(&key).ok_or(Rejection::Unauthenticated)?;
        session.last_used = Instant::now();
        let mut principal = session.principal.clone();
        principal.expires = session.absolute.min(auth::now().saturating_add(TIMEOUT));
        Ok(principal)
    }
    pub(crate) fn active(&self, id: &str) -> bool {
        self.0
            .lock()
            .ok()
            .is_some_and(|map| map.values().any(|s| s.id == id && s.valid()))
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(SERVICE, get(service))
        .route(COLLECTION, get(list).post(create))
        .route("/redfish/v1/SessionService/Sessions/Members", post(create))
        .route(
            "/redfish/v1/SessionService/Sessions/{id}",
            get(get_session).delete(delete),
        )
        .route(OIDC_CREATE, post(create_from_oidc))
        .route("/redfish/v1/AccountService", get(accounts))
        .route("/redfish/v1/AccountService/Roles", get(roles))
        .route("/redfish/v1/AccountService/Roles/{id}", get(role))
}
async fn service(State(app): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    Ok(Json(
        json!({"@odata.id":SERVICE,"@odata.type":"#SessionService.v1_1_9.SessionService",
        "Id":"SessionService","Name":"Session Service","ServiceEnabled":true,"SessionTimeout":TIMEOUT,
        "Status":{"State":"Enabled","Health":"OK"},"Sessions":link(COLLECTION),
        "Actions":{"Oem":{"#QrabSessionService.CreateFromOIDC":{"target":OIDC_CREATE}}}}),
    ))
}
async fn accounts(State(app): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    Ok(Json(
        json!({"@odata.id":"/redfish/v1/AccountService","@odata.type":"#AccountService.v1_18_0.AccountService",
        "Id":"AccountService","Name":"Account Service","ServiceEnabled":true,
        "Roles":link("/redfish/v1/AccountService/Roles"),
        "HTTPBasicAuth":if app.auth.basic_enabled(){"Enabled"}else{"Disabled"},
        "Status":{"State":"Enabled","Health":"OK"}}),
    ))
}
async fn roles(State(app): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    Ok(Json(collection(
        "/redfish/v1/AccountService/Roles",
        "RoleCollection",
        "Roles",
        ["Administrator", "Operator", "ReadOnly"]
            .iter()
            .map(|id| link(&format!("/redfish/v1/AccountService/Roles/{id}")))
            .collect(),
    )))
}
async fn role(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let privileges = match id.as_str() {
        "Administrator" => vec![
            "Login",
            "ConfigureManager",
            "ConfigureUsers",
            "ConfigureSelf",
            "ConfigureComponents",
        ],
        "Operator" => vec!["Login", "ConfigureSelf", "ConfigureComponents"],
        "ReadOnly" => vec!["Login", "ConfigureSelf"],
        _ => return Err(Failure::not_found()),
    };
    Ok(Json(
        json!({"@odata.id":format!("/redfish/v1/AccountService/Roles/{id}"),
        "@odata.type":"#Role.v1_3_0.Role", "Id":id,"RoleId":id,"Name":format!("{id} Role"),
        "IsPredefined":true,"AssignedPrivileges":privileges,"OemPrivileges":[]}),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Credentials {
    #[serde(rename = "UserName")]
    username: String,
    #[serde(rename = "Password")]
    password: String,
}
async fn create(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Result<InputJson<Credentials>, JsonRejection>,
) -> Result<Response, Failure> {
    if headers.contains_key("x-auth-token") {
        return Err(Failure::unauthenticated());
    }
    let InputJson(credentials) = body.map_err(json_rejection)?;
    if let Some(value) =
        auth::unique_header(&headers, "authorization").map_err(|_| Failure::unauthenticated())?
    {
        let (username, password) =
            auth::basic_credentials(value).map_err(|_| Failure::unauthenticated())?;
        if username != credentials.username || password != credentials.password {
            return Err(Failure::unauthenticated());
        }
    }
    let who = app
        .auth
        .authenticate_password(&credentials.username, &credentials.password)
        .await
        .map_err(|_| Failure::unauthenticated())?;
    // Local sessions have a sliding idle timeout and a one-day hard ceiling.
    created(&app.auth.sessions, who, auth::now().saturating_add(86400))
}
async fn create_from_oidc(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Result<InputJson<Value>, JsonRejection>,
) -> Result<Response, Failure> {
    let InputJson(body) = body.map_err(json_rejection)?;
    if body.as_object().is_none_or(|m| !m.is_empty()) || headers.contains_key("x-auth-token") {
        return Err(Failure::unauthenticated());
    }
    let header =
        auth::unique_header(&headers, "authorization").map_err(|_| Failure::unauthenticated())?;
    let who = app.auth.authenticate(header).await.map_err(|e| match e {
        Rejection::Forbidden => Failure::forbidden(),
        _ => Failure::unauthenticated(),
    })?;
    let expires = who.expires;
    created(&app.auth.sessions, who, expires)
}
fn created(store: &Store, who: Principal, absolute: u64) -> Result<Response, Failure> {
    let (token, location, body) = store.create(who, absolute)?;
    let mut response = (StatusCode::CREATED, Json(body)).into_response();
    let mut value = HeaderValue::from_str(&token)
        .map_err(|_| Failure::unavailable("Session header unavailable."))?;
    value.set_sensitive(true);
    response.headers_mut().insert("x-auth-token", value);
    response.headers_mut().insert(
        "location",
        HeaderValue::from_str(&location)
            .map_err(|_| Failure::unavailable("Session location unavailable."))?,
    );
    Ok(response)
}
async fn list(State(app): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, Failure> {
    let who = principal(&app, &headers).await?;
    let mut map = app
        .auth
        .sessions
        .0
        .lock()
        .map_err(|_| Failure::unavailable("Session store unavailable."))?;
    map.retain(|_, s| s.valid());
    let mut members: Vec<_> = map
        .values()
        .filter(|s| who.role == Role::Admin || same_owner(&s.principal, &who))
        .map(|s| link(&format!("{COLLECTION}/{}", s.id)))
        .collect();
    members.sort_by_key(|v| v["@odata.id"].as_str().unwrap_or_default().to_owned());
    Ok(Json(collection(
        COLLECTION,
        "SessionCollection",
        "Sessions",
        members,
    )))
}
async fn get_session(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Failure> {
    let who = principal(&app, &headers).await?;
    let map = app
        .auth
        .sessions
        .0
        .lock()
        .map_err(|_| Failure::unavailable("Session store unavailable."))?;
    let session = map
        .values()
        .find(|s| s.id == id && s.valid())
        .ok_or_else(Failure::not_found)?;
    if who.role != Role::Admin && !same_owner(&who, &session.principal) {
        return Err(Failure::forbidden());
    }
    Ok(Json(session.view()))
}
async fn delete(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, Failure> {
    let who = principal(&app, &headers).await?;
    let mut map = app
        .auth
        .sessions
        .0
        .lock()
        .map_err(|_| Failure::unavailable("Session store unavailable."))?;
    let (key, session) = map
        .iter()
        .find(|(_, s)| s.id == id && s.valid())
        .ok_or_else(Failure::not_found)?;
    if who.role != Role::Admin && !same_owner(&who, &session.principal) {
        return Err(Failure::forbidden());
    }
    let key = *key;
    map.remove(&key);
    Ok(StatusCode::NO_CONTENT)
}

fn same_owner(a: &Principal, b: &Principal) -> bool {
    a.subject == b.subject && a.issuer == b.issuer
}
fn json_rejection(error: JsonRejection) -> Failure {
    let status = match error.status() {
        StatusCode::UNPROCESSABLE_ENTITY => StatusCode::BAD_REQUEST,
        status => status,
    };
    Failure::new(
        status,
        "MalformedJSON",
        "A valid JSON request body with the required properties is required.",
        "Use application/json and the documented request properties.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn user() -> Principal {
        Principal {
            subject: "local:test".into(),
            issuer: None,
            role: Role::Viewer,
            expires: auth::now() + 300,
            session_id: None,
        }
    }
    #[test]
    fn secrets_are_distinct_and_expiration_and_revocation_are_enforced() {
        let store = Store::default();
        let (token, location, view) = store.create(user(), auth::now() + 300).unwrap();
        assert!(!location.contains(&token));
        assert!(!view.to_string().contains(&token));
        let who = store.authenticate(&token).unwrap();
        assert!(store.active(who.session_id.as_ref().unwrap()));
        assert_eq!(
            store.authenticate("guess").unwrap_err(),
            Rejection::Unauthenticated
        );
        store.0.lock().unwrap().clear();
        assert!(!store.active(who.session_id.as_ref().unwrap()));
        assert_eq!(
            store.authenticate(&token).unwrap_err(),
            Rejection::Unauthenticated
        );
        let (token, _, _) = store.create(user(), auth::now()).unwrap();
        assert_eq!(
            store.authenticate(&token).unwrap_err(),
            Rejection::Unauthenticated
        );
    }
    #[test]
    fn same_subjects_from_different_credential_namespaces_are_not_same_owner() {
        let local = user();
        let mut oidc = local.clone();
        oidc.issuer = Some("https://issuer.example".into());
        assert!(!same_owner(&local, &oidc));
        let mut other_issuer = oidc.clone();
        other_issuer.issuer = Some("https://other.example".into());
        assert!(!same_owner(&oidc, &other_issuer));
        assert!(same_owner(&local, &local));
    }
    #[test]
    fn idle_and_capacity_are_bounded() {
        let store = Store::default();
        let (token, _, _) = store.create(user(), auth::now() + 3600).unwrap();
        for s in store.0.lock().unwrap().values_mut() {
            s.last_used = Instant::now() - std::time::Duration::from_secs(TIMEOUT);
        }
        assert!(store.authenticate(&token).is_err());
        for _ in 0..CAPACITY {
            store.create(user(), auth::now() + 3600).unwrap();
        }
        assert!(store.create(user(), auth::now() + 3600).is_err());
    }
}

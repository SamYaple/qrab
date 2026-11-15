//! An in-process fake of the Proton API routes qrab uses, with real SRP on
//! the server side (proton-srp) and a modulus signed by Proton's key, so the
//! client's modulus verification runs exactly as against Proton.
#![allow(dead_code)]
use axum::{
    extract::State,
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json, Router,
};
use proton_srp::{
    RPGPVerifier, SRPAuth, SRPVerifierB64, ServerClientProof, ServerClientVerifier,
    ServerInteraction,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

/// Signed by Proton's SRP modulus key (from proton-srp's own example).
pub const MODULUS: &str = "-----BEGIN PGP SIGNED MESSAGE-----\nHash: SHA256\n\nW2z5HBi8RvsfYzZTS7qBaUxxPhsfHJFZpu3Kd6s1JafNrCCH9rfvPLrfuqocxWPgWDH2R8neK7PkNvjxto9TStuY5z7jAzWRvFWN9cQhAKkdWgy0JY6ywVn22+HFpF4cYesHrqFIKUPDMSSIlWjBVmEJZ/MusD44ZT29xcPrOqeZvwtCffKtGAIjLYPZIEbZKnDM1Dm3q2K/xS5h+xdhjnndhsrkwm9U9oyA2wxzSXFL+pdfj2fOdRwuR5nW0J2NFrq3kJjkRmpO/Genq1UW+TEknIWAb6VzJJJA244K/H8cnSx2+nSNZO3bbo6Ys228ruV9A8m6DhxmS+bihN3ttQ==\n-----BEGIN PGP SIGNATURE-----\nVersion: ProtonMail\nComment: https://protonmail.com\n\nwl4EARYIABAFAlwB1j0JEDUFhcTpUY8mAAD8CgEAnsFnF4cF0uSHKkXa1GIa\nGO86yMV4zDZEZcDSJo0fgr8A/AlupGN9EdHlsrZLmTA1vhIx+rOgxdEff28N\nkvNM7qIK\n=q6vu\n-----END PGP SIGNATURE-----";

pub const USER: &str = "qrab@proton.test";
pub const PASSWORD: &str = "correct horse battery staple";
pub const TOTP: &str = "123456";
pub const HV_TOKEN: &str = "hv-token-1";
pub const SERVER_KEY: &str = "xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=";

#[derive(Clone)]
pub struct Live {
    pub access: String,
    pub refresh: String,
    pub second_factor_done: bool,
}

#[derive(Default)]
pub struct Fake {
    pub app_version: String,
    pub totp: bool,
    pub human_verification: bool,
    pub max_tier: u8,
    pub verifier: Option<SRPVerifierB64>,
    pub srp: HashMap<String, ServerInteraction>,
    pub sessions: HashMap<String, Live>,
    pub catalog: Value,
    pub loads: Value,
    pub certificates: Vec<Value>,
    pub refreshes: u32,
    pub counter: u32,
    /// Every request's headers and body, for leak checks.
    pub seen: Vec<String>,
}

pub type Shared = Arc<Mutex<Fake>>;

pub fn logical(name: &str, tier: u8, features: u32, status: u8, score: f64, ip: &str) -> Value {
    let id = format!("id-{}", name.replace('#', "-"));
    json!({
        "ID": id, "Name": name, "EntryCountry": &name[..2], "ExitCountry": &name[..2],
        "City": "Zurich", "Tier": tier, "Features": features, "Status": status,
        "Load": 30, "Score": score,
        "Servers": [{"ID": format!("{id}-1"), "EntryIP": ip, "ExitIP": ip,
                     "Domain": "node.protonvpn.net", "X25519PublicKey": SERVER_KEY, "Status": 1}]
    })
}

impl Fake {
    pub fn new() -> Self {
        let verifier: SRPVerifierB64 =
            SRPAuth::generate_verifier(&RPGPVerifier::default(), PASSWORD, None, MODULUS)
                .expect("verifier")
                .into();
        Self {
            app_version: qrab_proton::api::APP_VERSION.to_owned(),
            max_tier: 2,
            verifier: Some(verifier),
            catalog: json!({"Code": 1000, "LogicalServers": [
                logical("CH#1", 2, 4, 1, 2.0, "203.0.113.1"),
                logical("CH#2", 2, 0, 1, 1.0, "203.0.113.2"),
                logical("SE#7", 2, 4, 1, 1.5, "203.0.113.7"),
            ]}),
            loads: json!({"Code": 1000, "LogicalServers": [
                {"ID": "id-CH-1", "Load": 12, "Score": 0.5, "Status": 1},
            ]}),
            ..Default::default()
        }
    }

    fn token(&mut self, what: &str) -> String {
        self.counter += 1;
        format!("{what}{:028}", self.counter)
    }
}

fn reply(status: u16, body: Value) -> Response {
    (StatusCode::from_u16(status).unwrap(), Json(body)).into_response()
}

fn error(status: u16, code: i64, message: &str) -> Response {
    reply(status, json!({"Code": code, "Error": message}))
}

async fn handle(
    State(fake): State<Shared>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: String,
) -> Response {
    let mut fake = fake.lock().unwrap();
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_owned()
    };
    fake.seen.push(format!("{method} {uri} {headers:?} {body}"));
    if header("x-pm-appversion") != fake.app_version {
        return error(400, 5003, "This version of the app is no longer supported");
    }
    let body: Value = serde_json::from_str(&body).unwrap_or_default();
    let path = uri.path();
    let uid = header("x-pm-uid");
    let bearer = header("authorization")
        .strip_prefix("Bearer ")
        .map(str::to_owned);
    let authed = fake
        .sessions
        .get(&uid)
        .is_some_and(|s| Some(&s.access) == bearer.as_ref());
    let full = authed && fake.sessions[&uid].second_factor_done;
    match (method.as_str(), path) {
        ("POST", "/auth/v4/info") => {
            if fake.human_verification && header("x-pm-human-verification-token") != HV_TOKEN {
                return reply(
                    422,
                    json!({"Code": 9001, "Error": "Human verification required",
                           "Details": {"HumanVerificationToken": HV_TOKEN,
                                       "HumanVerificationMethods": ["captcha", "email"]}}),
                );
            }
            let verifier = fake.verifier.clone().unwrap();
            let mut server = ServerInteraction::new_with_modulus_extractor(
                &RPGPVerifier::default(),
                MODULUS,
                &ServerClientVerifier::try_from(&verifier).unwrap(),
            )
            .unwrap();
            let challenge = server.generate_challenge();
            let session = fake.token("srp");
            fake.srp.insert(session.clone(), server);
            reply(
                200,
                json!({"Code": 1000, "Modulus": MODULUS, "ServerEphemeral": challenge.encode_b64(),
                       "Version": 4, "Salt": verifier.salt, "SRPSession": session}),
            )
        }
        ("POST", "/auth/v4") => {
            let session = body["SRPSession"].as_str().unwrap_or_default().to_owned();
            let Some(mut server) = fake.srp.remove(&session) else {
                return error(422, 2001, "unknown SRP session");
            };
            let Ok(proof) = ServerClientProof::new(
                body["ClientEphemeral"].as_str().unwrap_or_default(),
                body["ClientProof"].as_str().unwrap_or_default(),
            ) else {
                return error(422, 2001, "bad proof encoding");
            };
            let Ok(server_proof) = server.verify_proof(&proof) else {
                return error(422, 8002, "Incorrect login credentials");
            };
            let uid = fake.token("uid");
            let access = fake.token("acc");
            let refresh = fake.token("ref");
            let totp = fake.totp;
            fake.sessions.insert(
                uid.clone(),
                Live {
                    access: access.clone(),
                    refresh: refresh.clone(),
                    second_factor_done: !totp,
                },
            );
            let scopes = if totp {
                json!(["twofactor"])
            } else {
                json!(["self", "vpn"])
            };
            reply(
                200,
                json!({"Code": 1000, "UID": uid, "AccessToken": access, "RefreshToken": refresh,
                       "TokenType": "Bearer", "ServerProof": server_proof.encode_b64(),
                       "Scopes": scopes, "2FA": {"Enabled": if totp { 1 } else { 0 }, "TOTP": 1}}),
            )
        }
        ("POST", "/auth/v4/2fa") if authed => {
            if body["TwoFactorCode"] != TOTP {
                return error(422, 8002, "Incorrect code");
            }
            fake.sessions.get_mut(&uid).unwrap().second_factor_done = true;
            reply(200, json!({"Code": 1000, "Scopes": ["self", "vpn"]}))
        }
        ("POST", "/auth/v4/refresh") => {
            let token = body["RefreshToken"].as_str().unwrap_or_default().to_owned();
            if bearer.is_some() || fake.sessions.get(&uid).is_none_or(|s| s.refresh != token) {
                return error(422, 10013, "Invalid refresh token");
            }
            fake.refreshes += 1;
            let access = fake.token("acc");
            let refresh = fake.token("ref");
            let live = fake.sessions.get_mut(&uid).unwrap();
            live.access = access.clone();
            live.refresh = refresh.clone();
            reply(
                200,
                json!({"Code": 1000, "AccessToken": access, "RefreshToken": refresh,
                       "Scopes": ["self", "vpn"], "UID": uid}),
            )
        }
        ("DELETE", "/auth/v4") if authed => {
            fake.sessions.remove(&uid);
            reply(200, json!({"Code": 1000}))
        }
        ("GET", "/vpn/v2") if full => {
            let tier = fake.max_tier;
            reply(
                200,
                json!({"Code": 1000, "VPN": {"PlanName": "visionary2022", "MaxTier": tier,
                       "MaxConnect": 10, "Name": "openvpn-user", "Password": "openvpn-secret"}}),
            )
        }
        ("GET", "/vpn/v1/logicals") if full => reply(200, fake.catalog.clone()),
        ("GET", "/vpn/v1/loads") if full => reply(200, fake.loads.clone()),
        ("POST", "/vpn/v1/certificate") if full => {
            fake.certificates.push(body.clone());
            let serial = fake.token("serial");
            reply(
                200,
                json!({"Code": 1000, "SerialNumber": serial, "ExpirationTime": 2_000_000_000i64,
                       "RefreshTime": 1_999_000_000i64, "Certificate": "-----BEGIN CERTIFICATE-----\n...",
                       "ClientKey": body["ClientPublicKey"]}),
            )
        }
        _ if !authed && path != "/auth/v4/info" => error(401, 401, "Invalid access token"),
        _ => error(422, 2011, "Insufficient scope"),
    }
}

/// Serve a fake on a loopback port; returns its base URL.
pub async fn serve(fake: Fake) -> (String, Shared) {
    let shared: Shared = Arc::new(Mutex::new(fake));
    let app = Router::new().fallback(handle).with_state(shared.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, shared)
}

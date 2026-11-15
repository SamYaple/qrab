//! Proton's HTTP API, the way ProtonVPN's own clients use it: SRP login
//! (`/auth/v4/info`, `/auth/v4`), TOTP (`/auth/v4/2fa`), bearer tokens with
//! `x-pm-uid`, rotating refresh tokens (`/auth/v4/refresh`), and the VPN
//! routes (`/vpn/v2`, `/vpn/v1/logicals`, `/vpn/v1/loads`,
//! `/vpn/v1/certificate`).
//!
//! The API is not public; the app-version header is the one constant that
//! breaks when Proton retires a client version (code 5003,
//! [`Error::AppVersionRejected`]).
use crate::catalog::{Catalog, Load};
use crate::wg::Features;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use zeroize::Zeroizing;

/// ProtonVPN's API host for native clients.
pub const API_BASE: &str = "https://vpn-api.proton.me";
/// The client version sent as `x-pm-appversion`; Proton refuses retired ones.
pub const APP_VERSION: &str = "linux-vpn@4.19.0";
pub const USER_AGENT: &str = "ProtonVPN/4.19.0 (Linux; qrab)";
const TIMEOUT: Duration = Duration::from_secs(30);

/// A Proton human-verification challenge (HTTP 422, code 9001): a person
/// completes it in a browser, then the same request is retried with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Challenge {
    pub token: String,
    pub methods: Vec<String>,
}

impl Challenge {
    /// Where a person completes the challenge.
    pub fn url(&self) -> String {
        format!(
            "https://verify.proton.me/?methods={}&token={}",
            self.methods.join(","),
            self.token
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Proton no longer accepts client version {0}; update qrab_proton::api::APP_VERSION")]
    AppVersionRejected(String),
    #[error("the access token expired; refresh the session")]
    Unauthorized,
    #[error("Proton revoked or expired the session; run `qrab-proton login` again")]
    SessionRevoked,
    #[error("Proton asks for human verification: open {} , complete it, then retry", .0.url())]
    HumanVerification(Challenge),
    #[error("Proton refused the email or password")]
    BadCredentials,
    #[error("Proton refused the two-factor code")]
    BadTwoFactor,
    #[error("the account's two-factor authentication is FIDO2 only; add TOTP to log in here")]
    UnsupportedTwoFactor,
    #[error("rate limited by Proton; retry after {0:?}")]
    RateLimited(Option<Duration>),
    #[error("Proton API error {code} (HTTP {status}): {message}")]
    Api {
        status: u16,
        code: i64,
        message: String,
    },
    #[error("SRP: {0}")]
    Srp(String),
    #[error("the server's SRP proof did not verify")]
    ServerProof,
    #[error("transport: {0}")]
    Transport(String),
    #[error("unexpected response from {0}: {1}")]
    Decode(String, String),
    #[error("refusing API base {0}: only https, or http to a loopback address")]
    InsecureBase(String),
}

/// A Proton session. `uid` and `refresh_token` are what persists; the access
/// token is short-lived. Debug never shows a token.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Session {
    pub uid: String,
    access_token: String,
    refresh_token: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("uid", &self.uid)
            .field("scopes", &self.scopes)
            .finish_non_exhaustive()
    }
}

impl Session {
    /// A stored session, to be refreshed before use.
    pub fn stored(uid: &str, refresh_token: &str) -> Self {
        Self {
            uid: uid.to_owned(),
            access_token: String::new(),
            refresh_token: refresh_token.to_owned(),
            scopes: Vec::new(),
        }
    }

    pub fn refresh_token(&self) -> &str {
        &self.refresh_token
    }

    pub fn has_vpn_scope(&self) -> bool {
        self.scopes.iter().any(|s| s == "vpn")
    }
}

/// What the account's second factor needs after the password.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondFactor {
    None,
    Totp,
}

/// The account's VPN plan.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct VpnPlan {
    #[serde(default)]
    pub plan_name: Option<String>,
    #[serde(default)]
    pub max_tier: u8,
    #[serde(default)]
    pub max_connect: u32,
}

/// A certificate for a profile key.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Certificate {
    #[serde(default)]
    pub serial_number: String,
    /// Unix seconds.
    pub expiration_time: i64,
    /// Unix seconds: when Proton suggests renewing.
    #[serde(default)]
    pub refresh_time: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AuthInfo {
    modulus: String,
    server_ephemeral: String,
    version: u8,
    salt: String,
    #[serde(rename = "SRPSession")]
    srp_session: String,
}

#[derive(Deserialize, Default)]
struct TwoFactorInfo {
    #[serde(rename = "Enabled", default)]
    enabled: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AuthReply {
    #[serde(rename = "UID")]
    uid: String,
    access_token: String,
    refresh_token: String,
    server_proof: String,
    #[serde(default)]
    scopes: Vec<String>,
    #[serde(rename = "2FA", default)]
    two_factor: TwoFactorInfo,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RefreshReply {
    access_token: String,
    refresh_token: String,
    #[serde(default)]
    scopes: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ScopesReply {
    #[serde(default)]
    scopes: Vec<String>,
}

#[derive(Deserialize)]
struct PlanReply {
    #[serde(rename = "VPN")]
    vpn: VpnPlan,
}

#[derive(Deserialize)]
struct LoadsReply {
    #[serde(rename = "LogicalServers", default)]
    servers: Vec<Load>,
}

/// A Proton API client. Cheap to clone.
#[derive(Clone)]
pub struct Client {
    base: String,
    http: reqwest::Client,
    app_version: String,
}

fn transport(error: reqwest::Error) -> Error {
    // reqwest's message names the URL, never a header or body.
    Error::Transport(error.without_url().to_string())
}

impl Client {
    /// The production API.
    pub fn new() -> Result<Self, Error> {
        Self::with_base(API_BASE)
    }

    /// Another API base: https, or http to loopback (the tests' fake API).
    pub fn with_base(base: &str) -> Result<Self, Error> {
        let url = reqwest::Url::parse(base).map_err(|_| Error::InsecureBase(base.into()))?;
        let loopback = match url.host() {
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            Some(url::Host::Domain(d)) => d == "localhost",
            None => false,
        };
        if !(url.scheme() == "https" || (url.scheme() == "http" && loopback)) {
            return Err(Error::InsecureBase(base.into()));
        }
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(USER_AGENT)
            .https_only(url.scheme() == "https")
            .build()
            .map_err(transport)?;
        Ok(Self {
            base: base.trim_end_matches('/').to_owned(),
            http,
            app_version: APP_VERSION.to_owned(),
        })
    }

    /// Override the app version (when Proton retires [`APP_VERSION`]).
    pub fn app_version(mut self, version: &str) -> Self {
        self.app_version = version.to_owned();
        self
    }

    async fn call<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        session: Option<&Session>,
        body: Option<Value>,
        verified: Option<&Challenge>,
    ) -> Result<T, Error> {
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.base))
            .header("x-pm-appversion", &self.app_version)
            .header("Accept", "application/vnd.protonmail.v1+json");
        if let Some(session) = session {
            request = request.header("x-pm-uid", &session.uid);
            if !session.access_token.is_empty() {
                request = request.bearer_auth(&session.access_token);
            }
        }
        if let Some(challenge) = verified {
            request = request
                .header("x-pm-human-verification-token", &challenge.token)
                .header(
                    "x-pm-human-verification-token-type",
                    challenge.methods.first().map_or("captcha", String::as_str),
                );
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.map_err(transport)?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .map(Duration::from_secs);
        let text = response.text().await.map_err(transport)?;
        #[derive(Deserialize)]
        struct Envelope {
            #[serde(rename = "Code", default)]
            code: i64,
        }
        // The catalog is tens of megabytes: decode a success straight into
        // its type, and build a JSON tree only for (small) errors.
        let envelope: Envelope = serde_json::from_str(&text)
            .map_err(|_| Error::Decode(path.into(), format!("HTTP {status}, not JSON")))?;
        if status == 200 && (envelope.code == 1000 || envelope.code == 1001) {
            return serde_json::from_str(&text)
                .map_err(|e| Error::Decode(path.into(), e.to_string()));
        }
        let value: Value = serde_json::from_str(&text).unwrap_or_default();
        let code = envelope.code;
        let message = value
            .get("Error")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        Err(match (status, code) {
            (_, 5003 | 5005) => Error::AppVersionRejected(self.app_version.clone()),
            (422, 9001) => {
                let details = value.get("Details").cloned().unwrap_or_default();
                let token = details
                    .get("HumanVerificationToken")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let methods = details
                    .get("HumanVerificationMethods")
                    .and_then(Value::as_array)
                    .map(|m| {
                        m.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_else(|| vec!["captcha".to_owned()]);
                Error::HumanVerification(Challenge { token, methods })
            }
            (429, _) => Error::RateLimited(retry_after),
            (401, _) => Error::Unauthorized,
            (_, 8002) => Error::BadCredentials,
            _ => Error::Api {
                status,
                code,
                message,
            },
        })
    }

    /// Log in with a password. `verified` is a completed human-verification
    /// challenge from an earlier attempt. When the result asks for
    /// [`SecondFactor::Totp`], finish with [`Client::submit_totp`].
    pub async fn login(
        &self,
        username: &str,
        password: &str,
        verified: Option<&Challenge>,
    ) -> Result<(Session, SecondFactor), Error> {
        let info: AuthInfo = self
            .call(
                reqwest::Method::POST,
                "/auth/v4/info",
                None,
                Some(json!({"Username": username})),
                verified,
            )
            .await?;
        let version = proton_srp::SrpHashVersion::try_from(info.version)
            .map_err(|e| Error::Srp(e.to_string()))?;
        // The modulus is PGP-signed by Proton; rPGP verifies it against
        // Proton's key before it is used.
        let auth = proton_srp::SRPAuth::with_pgp(
            Some(username),
            password,
            version,
            &info.salt,
            &info.modulus,
            &info.server_ephemeral,
        )
        .map_err(|e| Error::Srp(e.to_string()))?;
        let proof: proton_srp::SRPProofB64 = auth
            .generate_proofs()
            .map_err(|e| Error::Srp(e.to_string()))?
            .into();
        let reply: AuthReply = self
            .call(
                reqwest::Method::POST,
                "/auth/v4",
                None,
                Some(json!({
                    "Username": username,
                    "ClientEphemeral": proof.client_ephemeral,
                    "ClientProof": proof.client_proof,
                    "SRPSession": info.srp_session,
                })),
                verified,
            )
            .await?;
        if !proof.compare_server_proof(&reply.server_proof) {
            return Err(Error::ServerProof);
        }
        let second = match reply.two_factor.enabled {
            0 => SecondFactor::None,
            bits if bits & 1 != 0 => SecondFactor::Totp,
            _ => return Err(Error::UnsupportedTwoFactor),
        };
        Ok((
            Session {
                uid: reply.uid,
                access_token: reply.access_token,
                refresh_token: reply.refresh_token,
                scopes: reply.scopes,
            },
            second,
        ))
    }

    /// Complete a login that asked for a TOTP code.
    pub async fn submit_totp(&self, session: &mut Session, code: &str) -> Result<(), Error> {
        let code = Zeroizing::new(code.trim().to_owned());
        let reply: ScopesReply = self
            .call(
                reqwest::Method::POST,
                "/auth/v4/2fa",
                Some(session),
                Some(json!({"TwoFactorCode": code.as_str()})),
                None,
            )
            .await
            .map_err(|e| match e {
                Error::BadCredentials | Error::Unauthorized => Error::BadTwoFactor,
                Error::Api { status: 422, .. } => Error::BadTwoFactor,
                other => other,
            })?;
        session.scopes = reply.scopes;
        Ok(())
    }

    /// A fresh access token. Proton rotates the refresh token on every
    /// refresh: persist the returned session's [`Session::refresh_token`]
    /// before using it, or the session is lost.
    pub async fn refresh(&self, session: &Session) -> Result<Session, Error> {
        let stale = Session {
            access_token: String::new(),
            ..session.clone()
        };
        let mut attempts = 0;
        loop {
            attempts += 1;
            let result: Result<RefreshReply, Error> = self
                .call(
                    reqwest::Method::POST,
                    "/auth/v4/refresh",
                    Some(&stale),
                    Some(json!({
                        "ResponseType": "token",
                        "GrantType": "refresh_token",
                        "RefreshToken": session.refresh_token,
                        "RedirectURI": "http://protonmail.ch",
                    })),
                    None,
                )
                .await;
            match result {
                Ok(reply) => {
                    return Ok(Session {
                        uid: session.uid.clone(),
                        access_token: reply.access_token,
                        refresh_token: reply.refresh_token,
                        scopes: reply.scopes,
                    })
                }
                // A race on Proton's side: the same request again.
                Err(Error::Api { status: 409, .. }) if attempts < 3 => continue,
                Err(Error::Api {
                    status: 400 | 422, ..
                })
                | Err(Error::Unauthorized)
                | Err(Error::BadCredentials) => return Err(Error::SessionRevoked),
                Err(other) => return Err(other),
            }
        }
    }

    /// End the session at Proton.
    pub async fn logout(&self, session: &Session) -> Result<(), Error> {
        let _: Value = self
            .call(
                reqwest::Method::DELETE,
                "/auth/v4",
                Some(session),
                None,
                None,
            )
            .await?;
        Ok(())
    }

    pub async fn plan(&self, session: &Session) -> Result<VpnPlan, Error> {
        let reply: PlanReply = self
            .call(reqwest::Method::GET, "/vpn/v2", Some(session), None, None)
            .await?;
        Ok(reply.vpn)
    }

    pub async fn logicals(&self, session: &Session) -> Result<Catalog, Error> {
        self.call(
            reqwest::Method::GET,
            "/vpn/v1/logicals?SecureCoreFilter=all&WithState=true",
            Some(session),
            None,
            None,
        )
        .await
    }

    pub async fn loads(&self, session: &Session) -> Result<Vec<Load>, Error> {
        let reply: LoadsReply = self
            .call(
                reqwest::Method::GET,
                "/vpn/v1/loads",
                Some(session),
                None,
                None,
            )
            .await?;
        Ok(reply.servers)
    }

    /// Certify `key` for `minutes` (Proton's minimum is one day) with
    /// `features` bound to it.
    pub async fn certificate(
        &self,
        session: &Session,
        key: &crate::keys::ProfileKey,
        minutes: u32,
        features: &Features,
    ) -> Result<Certificate, Error> {
        self.call(
            reqwest::Method::POST,
            "/vpn/v1/certificate",
            Some(session),
            Some(json!({
                "ClientPublicKey": key.public_pem(),
                "Duration": format!("{} min", minutes.max(24 * 60)),
                "Features": features.certificate_features(),
            })),
            None,
        )
        .await
    }
}

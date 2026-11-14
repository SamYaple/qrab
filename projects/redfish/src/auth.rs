//! OIDC bearer, optional local Basic, and revocable Redfish session authentication.
//! Per-request bearer authentication: an OIDC ID token (JWT) verified against
//! the issuer's JWKS, like Kubernetes API-server OIDC authentication. There
//! are no static tokens or plaintext stored passwords. Several issuers may be trusted; the token's unverified `iss`
//! selects the one verifier that checks it, and only the VM's subject and
//! group lists grant a role.
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::http::{header, HeaderMap};
use base64::Engine;
use openidconnect::{core::CoreJsonWebKeySet, ClientId, IssuerUrl};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::sync::Semaphore;

pub use oidc_bearer::{groups_claim, now, MAX_TOKEN_BYTES};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Viewer,
    Admin,
}

#[derive(Debug, Clone)]
pub struct Principal {
    pub subject: String,
    /// OIDC issuer namespace; None identifies a local account.
    pub issuer: Option<String>,
    pub role: Role,
    /// Unix seconds; live streams end here.
    pub expires: u64,
    /// Resource identifier only; never a session token.
    pub session_id: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Rejection {
    /// Missing, malformed, unverifiable or expired credentials: 401.
    Unauthenticated,
    /// A valid token whose subject is not listed: 403.
    Forbidden,
}

pub struct Authenticator {
    /// One per issuer.
    verifiers: Vec<oidc_bearer::Verifier>,
    admins: HashSet<String>,
    viewers: HashSet<String>,
    admin_groups: HashSet<String>,
    viewer_groups: HashSet<String>,
    pub(crate) sessions: crate::sessions::Store,
    local_accounts: Vec<LocalAccount>,
    password_permits: Arc<Semaphore>,
    attempts: Mutex<Attempts>,
    public_discovery: bool,
}

impl Authenticator {
    /// Fixed keys, for tests and offline use; never refreshed.
    pub fn new(
        issuer: IssuerUrl,
        audience: ClientId,
        keys: CoreJsonWebKeySet,
        admins: HashSet<String>,
        viewers: HashSet<String>,
    ) -> Self {
        Self {
            verifiers: vec![oidc_bearer::Verifier::new(issuer, audience, keys)],
            admins,
            viewers,
            admin_groups: HashSet::new(),
            viewer_groups: HashSet::new(),
            sessions: Default::default(),
            local_accounts: Vec::new(),
            password_permits: Arc::new(Semaphore::new(2)),
            attempts: Mutex::new(Attempts::default()),
            public_discovery: false,
        }
    }

    /// Also accept tokens verified by `verifier`, whose issuer must not
    /// already be trusted (add audiences with
    /// [`oidc_bearer::Verifier::with_audience`] instead).
    pub fn trusting(mut self, verifier: oidc_bearer::Verifier) -> Self {
        assert!(
            self.verifier_named(verifier.issuer()).is_none(),
            "one verifier per issuer"
        );
        self.verifiers.push(verifier);
        self
    }

    fn verifier_named(&self, issuer: &str) -> Option<&oidc_bearer::Verifier> {
        self.verifiers.iter().find(|v| v.issuer() == issuer)
    }

    /// The only verifier that may check `token`: the one for its unverified
    /// `iss`. Other sources are never tried.
    pub fn verifier_for(&self, token: &str) -> Option<&oidc_bearer::Verifier> {
        self.verifier_named(&oidc_bearer::unverified_issuer(token)?)
    }

    /// Also grant roles to members of these Kanidm group UUIDs, read from the
    /// token's `groups` claim.
    pub fn with_groups(mut self, admins: HashSet<String>, viewers: HashSet<String>) -> Self {
        self.admin_groups = admins;
        self.viewer_groups = viewers;
        self
    }

    /// OIDC discovery over HTTPS with redirects disabled.
    pub async fn discover(config: &crate::config::Config) -> anyhow::Result<Self> {
        let mut verifiers = Vec::with_capacity(config.sources.len());
        for source in &config.sources {
            let (first, rest) = source
                .audiences
                .split_first()
                .ok_or_else(|| anyhow::anyhow!("{} has no audience", source.issuer))?;
            let verifier =
                oidc_bearer::Verifier::discover(&source.issuer, first, config.ca_file.as_deref())
                    .await
                    .map_err(|e| e.context(source.issuer.clone()))?;
            verifiers.push(rest.iter().fold(verifier, |v, audience| {
                v.with_audience(ClientId::new(audience.clone()))
            }));
        }
        let auth = Self {
            verifiers,
            admins: config.admin_subjects.clone(),
            viewers: config.viewer_subjects.clone(),
            admin_groups: config.admin_groups.clone(),
            viewer_groups: config.viewer_groups.clone(),
            sessions: Default::default(),
            local_accounts: Vec::new(),
            password_permits: Arc::new(Semaphore::new(2)),
            attempts: Mutex::new(Attempts::default()),
            public_discovery: config.public_discovery,
        };
        match &config.local_accounts_file {
            Some(path) => auth.with_local_accounts(path),
            None => Ok(auth),
        }
    }

    fn role(&self, subject: &str, groups: &[String]) -> Option<Role> {
        let member = |set: &HashSet<String>| groups.iter().any(|g| set.contains(g));
        if self.admins.contains(subject) || member(&self.admin_groups) {
            Some(Role::Admin)
        } else if self.viewers.contains(subject) || member(&self.viewer_groups) {
            Some(Role::Viewer)
        } else {
            None
        }
    }

    /// Authenticate an `Authorization` header value.
    pub async fn authenticate(&self, header: Option<&str>) -> Result<Principal, Rejection> {
        let token = header
            .and_then(oidc_bearer::bearer_token)
            .ok_or(Rejection::Unauthenticated)?;
        let verifier = self.verifier_for(token).ok_or(Rejection::Unauthenticated)?;
        let verified = verifier
            .verify(token)
            .await
            .ok_or(Rejection::Unauthenticated)?;
        let role = self
            .role(&verified.subject, &verified.groups)
            .ok_or(Rejection::Forbidden)?;
        Ok(Principal {
            subject: verified.subject,
            issuer: Some(verifier.issuer().to_owned()),
            role,
            expires: verified.expires,
            session_id: None,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountFile {
    accounts: Vec<LocalAccount>,
}

// Intentionally not Debug or Serialize: PHC values never leave this module.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalAccount {
    username: String,
    password_hash: String,
    role: Role,
}

struct Attempts {
    available: f64,
    updated: Instant,
}
impl Default for Attempts {
    fn default() -> Self {
        Self {
            available: 16.0,
            updated: Instant::now(),
        }
    }
}
impl Attempts {
    fn admit(&mut self) -> bool {
        let now = Instant::now();
        self.available =
            (self.available + now.duration_since(self.updated).as_secs_f64() * 4.0).min(16.0);
        self.updated = now;
        if self.available < 1.0 {
            return false;
        }
        self.available -= 1.0;
        true
    }
}

impl Authenticator {
    pub fn with_public_discovery(mut self, enabled: bool) -> Self {
        self.public_discovery = enabled;
        self
    }
    pub fn public_discovery(&self) -> bool {
        self.public_discovery
    }
    pub fn basic_enabled(&self) -> bool {
        !self.local_accounts.is_empty()
    }

    /// Load a read-only Secret projection. New credentials require a sidecar
    /// restart, which also invalidates every existing in-memory session.
    pub fn with_local_accounts(mut self, path: &Path) -> anyhow::Result<Self> {
        use anyhow::{ensure, Context};
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .context("open Redfish local accounts")?
            .take(65537)
            .read_to_end(&mut bytes)
            .context("read Redfish local accounts")?;
        ensure!(bytes.len() <= 65536, "Redfish account file exceeds 64 KiB");
        let parsed: AccountFile = serde_json::from_slice(&bytes)
            .map_err(|_| anyhow::anyhow!("invalid Redfish local account file"))?;
        ensure!(
            !parsed.accounts.is_empty() && parsed.accounts.len() <= 16,
            "Redfish account file requires 1..16 accounts"
        );
        let mut names = HashSet::new();
        for account in &parsed.accounts {
            ensure!(
                valid_username(&account.username) && names.insert(account.username.clone()),
                "invalid or duplicate Redfish local username"
            );
            ensure!(
                valid_hash(&account.password_hash),
                "invalid or unsupported Redfish Argon2id hash policy"
            );
        }
        self.local_accounts = parsed.accounts;
        Ok(self)
    }

    /// All credential paths share unambiguous header parsing. Never fall back
    /// to another credential when a supplied credential fails verification.
    pub async fn authenticate_headers(&self, headers: &HeaderMap) -> Result<Principal, Rejection> {
        let authorization = unique_header(headers, header::AUTHORIZATION.as_str())?;
        let session = unique_header(headers, "x-auth-token")?;
        match (authorization, session) {
            (Some(_), Some(_)) | (None, None) => Err(Rejection::Unauthenticated),
            (None, Some(token)) => self.sessions.authenticate(token),
            (Some(value), None) => {
                let Some((scheme, _)) = value.split_once(' ') else {
                    return Err(Rejection::Unauthenticated);
                };
                if scheme.eq_ignore_ascii_case("basic") {
                    let (username, password) = basic_credentials(value)?;
                    self.authenticate_password(&username, &password).await
                } else {
                    self.authenticate(Some(value)).await
                }
            }
        }
    }

    pub(crate) async fn authenticate_password(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Principal, Rejection> {
        if !valid_username(username)
            || password.is_empty()
            || password.len() > 1024
            || self.local_accounts.is_empty()
        {
            return Err(Rejection::Unauthenticated);
        }
        if !self
            .attempts
            .lock()
            .map_err(|_| Rejection::Unauthenticated)?
            .admit()
        {
            return Err(Rejection::Unauthenticated);
        }
        let permit = self
            .password_permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| Rejection::Unauthenticated)?;
        let selected = self.local_accounts.iter().find(|a| a.username == username);
        // Unknown users perform the same bounded password work.
        let account = selected.unwrap_or(&self.local_accounts[0]).clone();
        let known = selected.is_some();
        let password = password.as_bytes().to_vec();
        let verified = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let hash = PasswordHash::new(&account.password_hash).ok()?;
            let valid = Argon2::default().verify_password(&password, &hash).is_ok();
            (valid && known).then_some((account.username, account.role))
        })
        .await
        .map_err(|_| Rejection::Unauthenticated)?
        .ok_or(Rejection::Unauthenticated)?;
        Ok(Principal {
            subject: format!("local:{}", verified.0),
            issuer: None,
            role: verified.1,
            expires: now().saturating_add(600),
            session_id: None,
        })
    }

    /// Streaming checks do not refresh idle expiry. A revoked session is no
    /// longer valid even when a stream was admitted before DELETE.
    pub fn principal_active(&self, who: &Principal) -> bool {
        now() < who.expires
            && who
                .session_id
                .as_deref()
                .is_none_or(|id| self.sessions.active(id))
    }
}

pub(crate) fn unique_header<'a>(
    headers: &'a HeaderMap,
    name: &str,
) -> Result<Option<&'a str>, Rejection> {
    let mut values = headers.get_all(name).iter();
    let value = values.next();
    if values.next().is_some() {
        return Err(Rejection::Unauthenticated);
    }
    value
        .map(|v| v.to_str().map_err(|_| Rejection::Unauthenticated))
        .transpose()
}
fn valid_username(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}
fn valid_hash(raw: &str) -> bool {
    let Ok(hash) = PasswordHash::new(raw) else {
        return false;
    };
    let decimal = |name| hash.params.get_decimal(name).unwrap_or(0);
    hash.params.iter().count() == 3
        && raw.len() <= 256
        && hash.algorithm.as_str() == "argon2id"
        && hash.version == Some(19)
        && (19456..=32768).contains(&decimal("m"))
        && (2..=4).contains(&decimal("t"))
        && (1..=2).contains(&decimal("p"))
        && hash.hash.is_some_and(|h| h.len() == 32)
        && hash.salt.is_some_and(|s| s.as_str().len() >= 22)
}

pub(crate) fn basic_credentials(value: &str) -> Result<(String, String), Rejection> {
    let (scheme, encoded) = value.split_once(' ').ok_or(Rejection::Unauthenticated)?;
    if !scheme.eq_ignore_ascii_case("basic") || encoded.len() > 2048 {
        return Err(Rejection::Unauthenticated);
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| Rejection::Unauthenticated)?;
    let text = std::str::from_utf8(&decoded).map_err(|_| Rejection::Unauthenticated)?;
    let (username, password) = text.split_once(':').ok_or(Rejection::Unauthenticated)?;
    Ok((username.to_owned(), password.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use argon2::{password_hash::SaltString, PasswordHasher};
    fn auth() -> Authenticator {
        Authenticator::new(
            IssuerUrl::new("https://issuer.example".into()).unwrap(),
            ClientId::new("redfish".into()),
            serde_json::from_str("{\"keys\":[]}").unwrap(),
            HashSet::new(),
            HashSet::new(),
        )
    }
    fn hashed() -> String {
        let salt = SaltString::encode_b64(b"test-only-16byte!").unwrap();
        Argon2::default()
            .hash_password(b"test-password", &salt)
            .unwrap()
            .to_string()
    }
    #[tokio::test]
    async fn local_basic_is_optional_bounded_and_unambiguous() {
        let mut auth = auth();
        assert!(!auth.basic_enabled());
        assert!(auth
            .authenticate_password("validator", "test-password")
            .await
            .is_err());
        let hash = hashed();
        assert!(valid_hash(&hash));
        auth.local_accounts.push(LocalAccount {
            username: "validator".into(),
            password_hash: hash,
            role: Role::Viewer,
        });
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD.encode(b"validator:test-password")
            )
            .parse()
            .unwrap(),
        );
        let who = auth.authenticate_headers(&headers).await.unwrap();
        assert_eq!(who.subject, "local:validator");
        assert_eq!(who.role, Role::Viewer);
        assert!(who.session_id.is_none());
        assert!(auth
            .authenticate_password("validator", "wrong")
            .await
            .is_err());
        assert!(auth
            .authenticate_password("unknown", "test-password")
            .await
            .is_err());
        headers.insert("x-auth-token", "some-session".parse().unwrap());
        assert!(auth.authenticate_headers(&headers).await.is_err());
        headers.remove("x-auth-token");
        headers.append("authorization", "Bearer unrelated".parse().unwrap());
        assert!(auth.authenticate_headers(&headers).await.is_err());
        let mut headers = HeaderMap::new();
        headers.insert("x-auth-token", "x".repeat(43).parse().unwrap());
        headers.append("x-auth-token", "y".repeat(43).parse().unwrap());
        assert!(auth.authenticate_headers(&headers).await.is_err());
    }
    #[test]
    fn unsafe_account_hashes_and_names_are_rejected_without_secret_errors() {
        assert!(!valid_username("local:oidc"));
        assert!(!valid_username(""));
        assert!(!valid_hash("plaintext"));
        let hash = hashed();
        assert!(!valid_hash(&hash.replace("m=19456", "m=999999999")));
        let path = std::env::temp_dir().join(format!("qrab-auth-accounts-{}", std::process::id()));
        std::fs::write(&path,serde_json::to_vec(&serde_json::json!({"accounts":[{"username":"validator","password_hash":hash,"role":"viewer"}]})).unwrap()).unwrap();
        let loaded = auth().with_local_accounts(&path).unwrap();
        assert!(loaded.basic_enabled());
        std::fs::write(&path,b"{\"accounts\":[{\"username\":\"oops\",\"password_hash\":\"secret-plaintext\",\"role\":\"viewer\"}]}").unwrap();
        let error = auth().with_local_accounts(&path).err().unwrap().to_string();
        assert!(!error.contains("secret-plaintext"));
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn password_attempt_work_is_bounded() {
        let mut attempts = Attempts::default();
        for _ in 0..16 {
            assert!(attempts.admit());
        }
        assert!(!attempts.admit());
        attempts.updated = Instant::now() - std::time::Duration::from_secs(1);
        assert!(attempts.admit());
    }
}

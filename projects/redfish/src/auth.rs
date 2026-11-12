//! Per-request bearer authentication: an OIDC ID token (JWT) verified against
//! the issuer's JWKS, like Kubernetes API-server OIDC authentication. There
//! are no Redfish sessions, static tokens, Basic credentials or anonymous
//! access.
use openidconnect::{core::CoreJsonWebKeySet, ClientId, IssuerUrl};
use serde::Serialize;
use std::collections::HashSet;

pub use oidc_bearer::{groups_claim, now, MAX_TOKEN_BYTES};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Viewer,
    Admin,
}

#[derive(Debug, Clone)]
pub struct Principal {
    pub subject: String,
    pub role: Role,
    /// Unix seconds; live streams end here.
    pub expires: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Rejection {
    /// Missing, malformed, unverifiable or expired credentials: 401.
    Unauthenticated,
    /// A valid token whose subject is not listed: 403.
    Forbidden,
}

pub struct Authenticator {
    verifier: oidc_bearer::Verifier,
    admins: HashSet<String>,
    viewers: HashSet<String>,
    admin_groups: HashSet<String>,
    viewer_groups: HashSet<String>,
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
            verifier: oidc_bearer::Verifier::new(issuer, audience, keys),
            admins,
            viewers,
            admin_groups: HashSet::new(),
            viewer_groups: HashSet::new(),
        }
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
        Ok(Self {
            verifier: oidc_bearer::Verifier::discover(
                &config.issuer,
                &config.audience,
                config.ca_file.as_deref(),
            )
            .await?,
            admins: config.admin_subjects.clone(),
            viewers: config.viewer_subjects.clone(),
            admin_groups: config.admin_groups.clone(),
            viewer_groups: config.viewer_groups.clone(),
        })
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
        let verified = self
            .verifier
            .verify(token)
            .await
            .ok_or(Rejection::Unauthenticated)?;
        let role = self
            .role(&verified.subject, &verified.groups)
            .ok_or(Rejection::Forbidden)?;
        Ok(Principal {
            subject: verified.subject,
            role,
            expires: verified.expires,
        })
    }
}

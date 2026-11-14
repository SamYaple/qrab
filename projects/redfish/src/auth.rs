//! Per-request bearer authentication: an OIDC ID token (JWT) verified against
//! the issuer's JWKS, like Kubernetes API-server OIDC authentication. There
//! are no Redfish sessions, static tokens, Basic credentials or anonymous
//! access. Several issuers may be trusted; the token's unverified `iss`
//! selects the one verifier that checks it, and only the VM's subject and
//! group lists grant a role.
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
    /// One per issuer.
    verifiers: Vec<oidc_bearer::Verifier>,
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
            verifiers: vec![oidc_bearer::Verifier::new(issuer, audience, keys)],
            admins,
            viewers,
            admin_groups: HashSet::new(),
            viewer_groups: HashSet::new(),
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
        Ok(Self {
            verifiers,
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
            .verifier_for(token)
            .ok_or(Rejection::Unauthenticated)?
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

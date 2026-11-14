//! The admin bucket: console features with data-exfiltration or
//! device-injection risk (docs/bmc.md#admin-bucket). A feature is usable on a
//! console connection only while it is enabled for this VM
//! (`BMC_ADMIN_BUCKET`), the caller is an admin, and the connection asked for
//! it at admission (`/api/console?features=audio,usb-redirect`).
use crate::{auth, AppState, Failure};
use serde::Serialize;
use std::{collections::BTreeSet, sync::Arc, time::Duration};
use tokio::sync::watch;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum Feature {
    /// Guest audio playback (no microphone).
    #[serde(rename = "audio")]
    Audio,
    /// Browser USB device redirection into the guest.
    #[serde(rename = "usb-redirect")]
    UsbRedirect,
}

impl Feature {
    pub const ALL: [Self; 2] = [Self::Audio, Self::UsbRedirect];

    pub fn name(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::UsbRedirect => "usb-redirect",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.name() == name)
    }
}

impl std::fmt::Display for Feature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// A comma-separated feature list, as in `BMC_ADMIN_BUCKET` and `?features=`.
/// Blank items are ignored; the first unknown name is the error.
///
/// ```
/// use bmc::admin_bucket::{parse_list, Feature};
/// assert_eq!(
///     parse_list(" usb-redirect,audio,,audio").unwrap().into_iter().collect::<Vec<_>>(),
///     [Feature::Audio, Feature::UsbRedirect]
/// );
/// assert!(parse_list("").unwrap().is_empty());
/// assert_eq!(parse_list("audio,microphone").unwrap_err(), "microphone");
/// ```
pub fn parse_list(raw: &str) -> Result<BTreeSet<Feature>, String> {
    raw.split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| Feature::parse(name).ok_or_else(|| name.to_owned()))
        .collect()
}

/// The features a console admission asks for: every `features` query
/// parameter, each a comma-separated list. Unknown names are 400.
pub fn requested(query: &[(String, String)]) -> Result<BTreeSet<Feature>, Failure> {
    let mut features = BTreeSet::new();
    for (_, value) in query.iter().filter(|(key, _)| key == "features") {
        features.extend(
            parse_list(value)
                .map_err(|_| Failure::bad_request("Unknown console feature requested."))?,
        );
    }
    Ok(features)
}

/// The gate for one feature on one connection: enabled for this VM, held by
/// an admin, and in `requested`, the connection's explicit request. Every
/// outcome is logged at info with the subject, feature and outcome.
pub fn admit(
    app: &AppState,
    feature: Feature,
    principal: &auth::Principal,
    requested: &BTreeSet<Feature>,
) -> Result<(), Failure> {
    let refusal = if !requested.contains(&feature) {
        Some("not requested on this connection")
    } else if !app.config.admin_bucket.contains(&feature) {
        Some("not enabled for this VM")
    } else if principal.role != auth::Role::Admin {
        Some("not an admin")
    } else {
        None
    };
    let subject = &principal.user.subject;
    let bearer = principal.is_bearer();
    match refusal {
        None => {
            tracing::info!(%subject, %feature, bearer, outcome = "admitted", "admin bucket feature admission");
            Ok(())
        }
        Some(reason) => {
            tracing::info!(%subject, %feature, bearer, outcome = "denied", reason, "admin bucket feature admission");
            Err(Failure::forbidden())
        }
    }
}

/// Admits every feature in `requested`, or refuses the whole admission with
/// 403 at the first refusal. Nothing requested admits nothing.
pub fn admission(
    app: &AppState,
    principal: &auth::Principal,
    requested: &BTreeSet<Feature>,
) -> Result<Grants, Failure> {
    for &feature in requested {
        admit(app, feature, principal, requested)?;
    }
    Ok(Grants::new(requested.clone()))
}

/// One console connection's admitted features. A feature channel checks
/// [`Grants::is_active`] before it starts and ends when
/// [`Grants::revoked`] resolves. Revocation is final for the connection.
#[derive(Clone, Debug)]
pub struct Grants(Arc<watch::Sender<BTreeSet<Feature>>>);

impl Grants {
    pub(crate) fn new(features: BTreeSet<Feature>) -> Self {
        Self(Arc::new(watch::Sender::new(features)))
    }

    /// No features: a console admitted without `?features=`.
    pub fn none() -> Self {
        Self::new(BTreeSet::new())
    }

    pub fn active(&self) -> BTreeSet<Feature> {
        self.0.borrow().clone()
    }

    pub fn is_active(&self, feature: Feature) -> bool {
        self.0.borrow().contains(&feature)
    }

    /// Resolves once `feature` is not, or no longer, admitted.
    pub async fn revoked(&self, feature: Feature) {
        let mut current = self.0.subscribe();
        let _ = current.wait_for(|set| !set.contains(&feature)).await;
    }

    /// Withdraws every active feature, logging each.
    pub fn revoke_all(&self, subject: &str, reason: &'static str) {
        let revoked = self.0.send_replace(BTreeSet::new());
        for feature in revoked {
            tracing::info!(%subject, %feature, outcome = "revoked", reason, "admin bucket feature revoked");
        }
    }

    /// Rechecks the admission at least once a second, as the console does,
    /// and revokes the features at the first failure: logout, expiry, or loss
    /// of the admin role. Never resolves, so the console itself keeps its own
    /// rules; run it beside the connection's other futures.
    pub async fn supervise(&self, app: &AppState, principal: &auth::Principal) {
        if !self.active().is_empty() {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                if !principal.still_current(app, auth::Role::Admin).await {
                    self.revoke_all(&principal.user.subject, "admin session or role ended");
                    break;
                }
            }
        }
        std::future::pending().await
    }
}

//! The broker's Kubernetes side: the session Secret, the key ring, profile
//! Secrets, the catalog ConfigMap and profile status.
use super::*;
use crate::{
    api::{Client, Session, VpnPlan},
    catalog::Load,
    login::SESSION_SECRET,
    session::{Keeper, SaveError, SessionStore},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use k8s_openapi::{
    api::core::v1::{ConfigMap, Secret},
    ByteString,
};
use kube::{
    api::{Patch, PatchParams, PostParams},
    Api, Resource, ResourceExt,
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

const PASS: Duration = Duration::from_secs(15);
const LOADS_EVERY: Duration = Duration::from_secs(15 * 60);
const LOGICALS_EVERY: Duration = Duration::from_secs(6 * 3600);

pub fn log(event: &str, detail: serde_json::Value) {
    println!("{}", json!({"event": event, "detail": detail}));
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

/// Secret `proton-session`, read and replaced by name only.
pub struct SecretStore {
    pub api: Api<Secret>,
}

fn field(secret: &Secret, key: &str) -> Option<String> {
    let bytes = secret.data.as_ref()?.get(key)?;
    String::from_utf8(bytes.0.clone()).ok()
}

impl SessionStore for SecretStore {
    async fn load(&self) -> Result<Option<(Session, String)>, String> {
        let Some(secret) = self
            .api
            .get_opt(SESSION_SECRET)
            .await
            .map_err(|e| e.to_string())?
        else {
            return Ok(None);
        };
        let (Some(uid), Some(refresh)) = (field(&secret, "uid"), field(&secret, "refresh_token"))
        else {
            return Ok(None);
        };
        Ok(Some((
            Session::stored(uid.trim(), refresh.trim()),
            secret.resource_version().unwrap_or_default(),
        )))
    }

    async fn save(&self, session: &Session, version: &str) -> Result<String, SaveError> {
        let mut secret = self
            .api
            .get(SESSION_SECRET)
            .await
            .map_err(|e| SaveError::Other(e.to_string()))?;
        if secret.resource_version().as_deref() != Some(version) {
            return Err(SaveError::Conflict);
        }
        let mut data = BTreeMap::new();
        data.insert(
            "uid".to_owned(),
            ByteString(session.uid.clone().into_bytes()),
        );
        data.insert(
            "refresh_token".to_owned(),
            ByteString(session.refresh_token().as_bytes().to_vec()),
        );
        secret.data = Some(data);
        secret.string_data = None;
        match self
            .api
            .replace(SESSION_SECRET, &PostParams::default(), &secret)
            .await
        {
            Ok(saved) => Ok(saved.resource_version().unwrap_or_default()),
            Err(kube::Error::Api(e)) if e.code == 409 => Err(SaveError::Conflict),
            Err(e) => Err(SaveError::Other(e.to_string())),
        }
    }
}

/// Profile keys, by profile UID, in Secret `proton-keys`.
struct KeyRing {
    api: Api<Secret>,
    keys: BTreeMap<String, ProfileKey>,
    version: Option<String>,
    loaded: bool,
}

impl KeyRing {
    async fn load(&mut self) -> Result<(), String> {
        if self.loaded {
            return Ok(());
        }
        if let Some(secret) = self
            .api
            .get_opt(KEYS_SECRET)
            .await
            .map_err(|e| e.to_string())?
        {
            for (uid, seed) in secret.data.clone().unwrap_or_default() {
                let seed = Zeroizing::new(String::from_utf8_lossy(&seed.0).into_owned());
                if let Ok(key) = ProfileKey::from_stored(&seed) {
                    self.keys.insert(uid, key);
                }
            }
            self.version = secret.resource_version();
        }
        self.loaded = true;
        Ok(())
    }

    /// The keys of `uids`, generating missing ones and dropping those of
    /// deleted profiles; persisted before any key is used.
    async fn sync(&mut self, uids: &[String]) -> Result<(), String> {
        self.load().await?;
        let mut changed = false;
        for uid in uids {
            if !self.keys.contains_key(uid) {
                let key = ProfileKey::generate().map_err(|e| e.to_string())?;
                self.keys.insert(uid.clone(), key);
                changed = true;
            }
        }
        let before = self.keys.len();
        self.keys.retain(|uid, _| uids.contains(uid));
        changed |= self.keys.len() != before;
        if !changed {
            return Ok(());
        }
        let data: BTreeMap<String, ByteString> = self
            .keys
            .iter()
            .map(|(uid, key)| (uid.clone(), ByteString(key.to_stored().as_bytes().to_vec())))
            .collect();
        let mut secret = Secret {
            data: Some(data),
            type_: Some("Opaque".into()),
            ..Default::default()
        };
        secret.metadata.name = Some(KEYS_SECRET.into());
        secret.metadata.labels = Some(BTreeMap::from([(
            "app.kubernetes.io/component".to_owned(),
            COMPONENT.to_owned(),
        )]));
        let saved = match &self.version {
            None => self.api.create(&PostParams::default(), &secret).await,
            Some(version) => {
                secret.metadata.resource_version = Some(version.clone());
                self.api
                    .replace(KEYS_SECRET, &PostParams::default(), &secret)
                    .await
            }
        };
        match saved {
            Ok(saved) => {
                self.version = saved.resource_version();
                Ok(())
            }
            Err(error) => {
                // Re-read next pass rather than risk losing a stored key.
                self.loaded = false;
                self.keys.clear();
                self.version = None;
                Err(format!("cannot store profile keys: {error}"))
            }
        }
    }
}

struct Account {
    plan: VpnPlan,
    catalog: Catalog,
    plan_at: Instant,
    logicals_at: Instant,
    loads_at: Instant,
}

pub struct Broker {
    namespace: String,
    keeper: Keeper<SecretStore>,
    profiles: Api<VpnProfile>,
    secrets: Api<Secret>,
    configmaps: Api<ConfigMap>,
    keys: KeyRing,
    account: Option<Account>,
}

fn blocking_reason(error: &Error) -> &'static str {
    match error {
        Error::SessionRevoked => "SessionRevoked",
        Error::AppVersionRejected(_) => "AppVersionRejected",
        Error::RateLimited(_) => "RateLimited",
        _ => "ProtonUnavailable",
    }
}

impl Broker {
    pub fn new(client: kube::Client, namespace: &str, proton: Client) -> Self {
        let secrets: Api<Secret> = Api::namespaced(client.clone(), namespace);
        Self {
            namespace: namespace.to_owned(),
            keeper: Keeper::new(
                SecretStore {
                    api: secrets.clone(),
                },
                proton,
            ),
            profiles: Api::namespaced(client.clone(), namespace),
            secrets: secrets.clone(),
            configmaps: Api::namespaced(client, namespace),
            keys: KeyRing {
                api: secrets,
                keys: BTreeMap::new(),
                version: None,
                loaded: false,
            },
            account: None,
        }
    }

    async fn refresh_account(&mut self) -> Result<(), Error> {
        let stale =
            |at: Option<Instant>, every: Duration| at.is_none_or(|at| at.elapsed() >= every);
        let account = self.account.as_ref();
        if stale(account.map(|a| a.plan_at), LOGICALS_EVERY)
            || stale(account.map(|a| a.logicals_at), LOGICALS_EVERY)
        {
            let plan = self
                .keeper
                .with(|c, s| async move { c.plan(&s).await })
                .await?;
            let mut catalog = self
                .keeper
                .with(|c, s| async move { c.logicals(&s).await })
                .await?;
            let loads: Vec<Load> = self
                .keeper
                .with(|c, s| async move { c.loads(&s).await })
                .await?;
            catalog.apply_loads(&loads);
            log(
                "catalog",
                json!({"servers": catalog.servers.len(), "plan": plan.plan_name, "maxTier": plan.max_tier}),
            );
            let at = Instant::now();
            self.account = Some(Account {
                plan,
                catalog,
                plan_at: at,
                logicals_at: at,
                loads_at: at,
            });
            self.publish_catalog().await;
        } else if stale(account.map(|a| a.loads_at), LOADS_EVERY) {
            let loads: Vec<Load> = self
                .keeper
                .with(|c, s| async move { c.loads(&s).await })
                .await?;
            if let Some(account) = &mut self.account {
                account.catalog.apply_loads(&loads);
                account.loads_at = Instant::now();
            }
            self.publish_catalog().await;
        }
        Ok(())
    }

    async fn publish_catalog(&self) {
        let Some(account) = &self.account else { return };
        let compact = account.catalog.compact(now(), account.plan.max_tier);
        let bytes = match crate::catalog::encode(&compact) {
            Ok(bytes) => bytes,
            Err(error) => return log("catalog_encode_failed", json!(error)),
        };
        let desired = json!({
            "apiVersion": "v1", "kind": "ConfigMap",
            "metadata": {"name": CATALOG_CONFIGMAP, "namespace": self.namespace,
                         "labels": {"app.kubernetes.io/component": COMPONENT}},
            "binaryData": {CATALOG_KEY: STANDARD.encode(&bytes)},
        });
        if let Err(error) = self
            .configmaps
            .patch(
                CATALOG_CONFIGMAP,
                &PatchParams::apply(FIELD_MANAGER).force(),
                &Patch::Apply(&desired),
            )
            .await
        {
            log("catalog_publish_failed", json!(error.to_string()));
        }
    }

    async fn apply_secret(&self, profile: &VpnProfile, conf: &str) -> Result<(), kube::Error> {
        let name = secret_name(&profile.name_any());
        let desired = json!({
            "apiVersion": "v1", "kind": "Secret", "type": "Opaque",
            "metadata": {
                "name": name, "namespace": self.namespace,
                "labels": {"app.kubernetes.io/component": COMPONENT, PROFILE_LABEL: profile.name_any()},
                "ownerReferences": [{
                    "apiVersion": VpnProfile::api_version(&()), "kind": VpnProfile::kind(&()),
                    "name": profile.name_any(), "uid": profile.uid().unwrap_or_default(),
                    "controller": true,
                }],
            },
            "data": {VPN_FILE: STANDARD.encode(conf.as_bytes())},
        });
        self.secrets
            .patch(
                &name,
                &PatchParams::apply(FIELD_MANAGER).force(),
                &Patch::Apply(&desired),
            )
            .await
            .map(|_| ())
    }

    async fn set_status(&self, profile: &VpnProfile, status: &VpnProfileStatus) {
        if profile.status.as_ref() == Some(status) {
            return;
        }
        if let Err(error) = self
            .profiles
            .patch_status(
                &profile.name_any(),
                &PatchParams::default(),
                &Patch::Merge(json!({"status": status})),
            )
            .await
        {
            log(
                "status_failed",
                json!({"profile": profile.name_any(), "error": error.to_string()}),
            );
        }
    }

    /// One pass over every profile.
    pub async fn pass(&mut self) {
        let mut profiles = match self.profiles.list(&Default::default()).await {
            Ok(list) => list.items,
            Err(error) => return log("list_failed", json!(error.to_string())),
        };
        profiles.sort_by_key(|p| (p.creation_timestamp(), p.name_any()));
        let uids: Vec<String> = profiles.iter().filter_map(|p| p.uid()).collect();
        if let Err(error) = self.keys.sync(&uids).await {
            log("keys_failed", json!(error));
            return;
        }
        if profiles.is_empty() {
            return;
        }
        if let Err(error) = self.refresh_account().await {
            log(
                "proton_failed",
                json!({"reason": blocking_reason(&error), "error": error.to_string()}),
            );
            let message = error.to_string();
            for profile in &profiles {
                self.set_status(
                    profile,
                    &blocked(profile, blocking_reason(&error), &message),
                )
                .await;
            }
            return;
        }
        for (rank, profile) in profiles.iter().enumerate() {
            let Some(key) = profile
                .uid()
                .and_then(|uid| self.keys.keys.get(&uid).cloned())
            else {
                continue;
            };
            let account = self.account.as_ref().expect("refreshed above");
            let world = World {
                catalog: &account.catalog,
                max_tier: account.plan.max_tier,
                max_connect: account.plan.max_connect,
                now: now(),
            };
            let keeper = &mut self.keeper;
            let certify_key = key.clone();
            let outcome = reconcile(profile, &key, &world, rank, |features| async move {
                keeper
                    .with(|c, s| {
                        let (key, features) = (certify_key.clone(), features.clone());
                        async move {
                            c.certificate(&s, &key, CERTIFICATE_MINUTES, &features)
                                .await
                        }
                    })
                    .await
            })
            .await;
            let mut status = outcome.status;
            if let Some(conf) = &outcome.conf {
                if let Err(error) = self.apply_secret(profile, conf).await {
                    status.conditions = vec![ready(false, "SecretFailed", error.to_string())];
                }
            }
            if status.server != profile.status.as_ref().and_then(|s| s.server.clone()) {
                log(
                    "selected",
                    json!({"profile": profile.name_any(), "server": status.server, "entry": status.entry_ip}),
                );
            }
            self.set_status(profile, &status).await;
        }
    }
}

/// The Secret key a VPN NIC's anchor mounts (qrab_netd::vpn::VPN_FILE).
pub const VPN_FILE: &str = "wg.conf";

/// Run forever: a pass every 15 s.
pub async fn run(namespace: &str, proton: Client) -> Result<(), String> {
    let client = kube::Client::try_default()
        .await
        .map_err(|e| format!("no Kubernetes client: {e}"))?;
    let mut broker = Broker::new(client, namespace, proton);
    log("started", json!({"namespace": namespace}));
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|e| e.to_string())?;
    loop {
        broker.pass().await;
        tokio::select! {
            _ = tokio::time::sleep(PASS) => {}
            _ = term.recv() => return Ok(()),
        }
    }
}

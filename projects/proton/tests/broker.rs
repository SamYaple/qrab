//! The broker's session keeping and profile reconciliation against the fake
//! Proton API.
mod support;

use qrab_proton::{
    api::{Client, Error, Session},
    broker::{reconcile, World},
    catalog::{Catalog, Feature},
    crd::{ProtonSpec, VpnProfile, VpnProfileSpec},
    keys::ProfileKey,
    session::{Keeper, SaveError, SessionStore},
};
use std::sync::{Arc, Mutex};
use support::{serve, Fake, Shared, PASSWORD, USER};

#[derive(Default)]
struct Stored {
    session: Option<Session>,
    version: u64,
    conflict_next: bool,
    fail_next: bool,
    saves: u32,
}

#[derive(Clone, Default)]
struct Memory(Arc<Mutex<Stored>>);

impl SessionStore for Memory {
    async fn load(&self) -> Result<Option<(Session, String)>, String> {
        let stored = self.0.lock().unwrap();
        Ok(stored.session.clone().map(|s| {
            (
                Session::stored(&s.uid, s.refresh_token()),
                stored.version.to_string(),
            )
        }))
    }

    async fn save(&self, session: &Session, version: &str) -> Result<String, SaveError> {
        let mut stored = self.0.lock().unwrap();
        if stored.fail_next {
            stored.fail_next = false;
            return Err(SaveError::Other("apiserver unavailable".into()));
        }
        if stored.conflict_next {
            // Someone touched the Secret's metadata: the version moves.
            stored.conflict_next = false;
            stored.version += 1;
            return Err(SaveError::Conflict);
        }
        if version != stored.version.to_string() {
            return Err(SaveError::Conflict);
        }
        stored.version += 1;
        stored.saves += 1;
        stored.session = Some(session.clone());
        Ok(stored.version.to_string())
    }
}

async fn login(base: &str) -> Session {
    Client::with_base(base)
        .unwrap()
        .login(USER, PASSWORD, None)
        .await
        .unwrap()
        .0
}

fn refreshes(fake: &Shared) -> u32 {
    fake.lock().unwrap().refreshes
}

#[tokio::test]
async fn a_rotated_refresh_token_is_stored_before_it_is_used() {
    let (base, fake) = serve(Fake::new()).await;
    let store = Memory::default();
    let handed_over = login(&base).await;
    store.0.lock().unwrap().session = Some(handed_over.clone());
    let mut keeper = Keeper::new(store.clone(), Client::with_base(&base).unwrap());
    let session = keeper.session().await.unwrap();
    assert_eq!(refreshes(&fake), 1);
    // What is stored is what is in use, and it is not the handed-over token.
    let stored = store.0.lock().unwrap().session.clone().unwrap();
    assert_eq!(stored.refresh_token(), session.refresh_token());
    assert_ne!(stored.refresh_token(), handed_over.refresh_token());
    // The next call reuses it: no refresh.
    keeper.session().await.unwrap();
    assert_eq!(refreshes(&fake), 1);
    let plan = keeper
        .with(|c, s| async move { c.plan(&s).await })
        .await
        .unwrap();
    assert_eq!(plan.max_tier, 2);

    // An access token Proton stops accepting is refreshed once, then the call works.
    {
        let mut fake = fake.lock().unwrap();
        let live = fake.sessions.get_mut(&session.uid).unwrap();
        live.access = "revoked-access".into();
    }
    keeper
        .with(|c, s| async move { c.plan(&s).await })
        .await
        .unwrap();
    assert_eq!(refreshes(&fake), 2);
    assert_eq!(store.0.lock().unwrap().saves, 2);
}

#[tokio::test]
async fn a_failed_or_conflicting_save_is_retried_without_refreshing_again() {
    let (base, fake) = serve(Fake::new()).await;
    let store = Memory::default();
    store.0.lock().unwrap().session = Some(login(&base).await);
    let mut keeper = Keeper::new(store.clone(), Client::with_base(&base).unwrap());

    store.0.lock().unwrap().fail_next = true;
    assert!(keeper.session().await.is_err());
    assert_eq!(refreshes(&fake), 1);
    // Retried first: stored, and no second refresh (that would kill the
    // only live token).
    let session = keeper.session().await.unwrap();
    assert_eq!(refreshes(&fake), 1);
    assert_eq!(
        store
            .0
            .lock()
            .unwrap()
            .session
            .clone()
            .unwrap()
            .refresh_token(),
        session.refresh_token()
    );

    // A conflict where the stored token is still ours: re-read and save again.
    keeper.expire();
    store.0.lock().unwrap().conflict_next = true;
    assert!(keeper.session().await.is_err());
    assert_eq!(refreshes(&fake), 2);
    let session = keeper.session().await.unwrap();
    assert_eq!(refreshes(&fake), 2);
    assert_eq!(
        store
            .0
            .lock()
            .unwrap()
            .session
            .clone()
            .unwrap()
            .refresh_token(),
        session.refresh_token()
    );
}

#[tokio::test]
async fn a_new_login_takes_over_and_a_revoked_session_is_reported() {
    let (base, fake) = serve(Fake::new()).await;
    let store = Memory::default();
    store.0.lock().unwrap().session = Some(login(&base).await);
    let mut keeper = Keeper::new(store.clone(), Client::with_base(&base).unwrap());
    let first = keeper.session().await.unwrap();

    // The person logs in again (qrab-proton login replaces the Secret).
    let second = login(&base).await;
    {
        let mut stored = store.0.lock().unwrap();
        stored.session = Some(second.clone());
        stored.version += 1;
    }
    let adopted = keeper.session().await.unwrap();
    assert_eq!(adopted.uid, second.uid);
    assert_ne!(adopted.uid, first.uid);

    // The stored session is revoked at Proton: typed, and nothing refreshes it.
    fake.lock().unwrap().sessions.clear();
    keeper.expire();
    assert!(matches!(keeper.session().await, Err(Error::SessionRevoked)));
    // No stored session at all reads the same.
    store.0.lock().unwrap().session = None;
    assert!(matches!(keeper.session().await, Err(Error::SessionRevoked)));
}

fn profile(spec: ProtonSpec) -> VpnProfile {
    let mut profile = VpnProfile::new("ch-p2p", VpnProfileSpec { proton: spec });
    profile.metadata.uid = Some("uid-1".into());
    profile.metadata.generation = Some(1);
    profile
}

async fn catalog(base: &str) -> (Client, Session, Catalog) {
    let client = Client::with_base(base).unwrap();
    let session = login(base).await;
    let mut catalog = client.logicals(&session).await.unwrap();
    catalog.apply_loads(&client.loads(&session).await.unwrap());
    (client, session, catalog)
}

#[tokio::test]
async fn a_profile_gets_a_config_and_keeps_its_key_across_server_changes() {
    let (base, fake) = serve(Fake::new()).await;
    let (client, session, mut catalog) = catalog(&base).await;
    let key = ProfileKey::generate().unwrap();
    let mut p = profile(ProtonSpec {
        port_forwarding: true,
        net_shield: 2,
        vpn_accelerator: true,
        ..Default::default()
    });
    let now = 1_900_000_000;
    fn world(catalog: &Catalog, now: i64) -> World<'_> {
        World {
            catalog,
            max_tier: 2,
            max_connect: 10,
            now,
        }
    }
    let certify = |features| {
        let (client, session, key) = (client.clone(), session.clone(), key.clone());
        async move { client.certificate(&session, &key, 60, &features).await }
    };

    let outcome = reconcile(&p, &key, &world(&catalog, now), 0, certify).await;
    let conf = outcome.conf.expect("a config");
    let tunnel = qrab_netd::vpn::Tunnel::parse(&conf).unwrap();
    // The best P2P server after loads: CH#1.
    assert_eq!(tunnel.endpoint.to_string(), "203.0.113.1:51820");
    assert!(conf.contains("# qrab: port-forwarding"));
    let status = outcome.status;
    assert_eq!(status.server.as_deref(), Some("CH#1"));
    assert_eq!(status.secret_name.as_deref(), Some("vpn-ch-p2p"));
    assert_eq!(status.ready().unwrap().status, "True");
    assert_eq!(status.server_features, [Feature::P2p]);
    assert_eq!(
        status.public_key.as_deref(),
        Some(key.wireguard_public().as_str())
    );
    assert_eq!(fake.lock().unwrap().certificates.len(), 1);
    assert_eq!(
        fake.lock().unwrap().certificates[0]["Features"]["PortForwarding"],
        true
    );
    let private = format!("PrivateKey = {}", key.wireguard_private().as_str());
    assert!(conf.contains(&private));
    // No status field carries key material.
    let json = serde_json::to_string(&status).unwrap();
    assert!(!json.contains(key.wireguard_private().as_str()));

    // Steady state: same config, no new certificate.
    p.status = Some(status);
    let again = reconcile(&p, &key, &world(&catalog, now), 0, certify).await;
    assert_eq!(again.conf.as_ref().map(|c| c.as_str()), Some(conf.as_str()));
    assert_eq!(fake.lock().unwrap().certificates.len(), 1);

    // A manual choice: new peer, same private key, no new certificate.
    p.status = Some(again.status);
    p.spec.proton.server = Some("SE#7".into());
    let manual = reconcile(&p, &key, &world(&catalog, now), 0, certify).await;
    let conf = manual.conf.unwrap();
    assert!(conf.contains("Endpoint = 203.0.113.7:51820") && conf.contains(&private));
    assert_eq!(fake.lock().unwrap().certificates.len(), 1);

    // Back to auto, and the selected server goes into maintenance: reselected.
    p.status = Some(manual.status);
    p.spec.proton.server = None;
    catalog
        .servers
        .iter_mut()
        .find(|l| l.name == "SE#7")
        .unwrap()
        .status = 0;
    let moved = reconcile(&p, &key, &world(&catalog, now), 0, certify).await;
    assert_eq!(moved.status.server.as_deref(), Some("CH#1"));
    assert!(moved.conf.unwrap().contains(&private));

    // New features: a new certificate.
    p.status = Some(moved.status);
    p.spec.proton.moderate_nat = true;
    let refeatured = reconcile(&p, &key, &world(&catalog, now), 0, certify).await;
    assert!(refeatured.conf.is_some());
    assert_eq!(fake.lock().unwrap().certificates.len(), 2);
    assert!(
        refeatured
            .status
            .certified_features
            .as_ref()
            .unwrap()
            .moderate_nat
    );
}

#[tokio::test]
async fn unservable_profiles_report_why_and_keep_the_existing_tunnel() {
    let (base, _) = serve(Fake::new()).await;
    let (client, session, catalog) = catalog(&base).await;
    let key = ProfileKey::generate().unwrap();
    let world = World {
        catalog: &catalog,
        max_tier: 2,
        max_connect: 1,
        now: 1_900_000_000,
    };
    let certify = |features| {
        let (client, session, key) = (client.clone(), session.clone(), key.clone());
        async move { client.certificate(&session, &key, 60, &features).await }
    };
    // A manual server that cannot do port forwarding: no config, the reason.
    let p = profile(ProtonSpec {
        server: Some("CH#2".into()),
        port_forwarding: true,
        ..Default::default()
    });
    let outcome = reconcile(&p, &key, &world, 0, certify).await;
    assert!(outcome.conf.is_none());
    let ready = outcome.status.ready().unwrap().clone();
    assert_eq!(
        (ready.status.as_str(), ready.reason.as_str()),
        ("False", "NoServer")
    );
    assert!(
        ready.message.contains("port forwarding needs it"),
        "{}",
        ready.message
    );

    // Beyond the plan's connection limit: configured, but flagged.
    let p = profile(ProtonSpec::default());
    let outcome = reconcile(&p, &key, &world, 1, certify).await;
    assert!(outcome.conf.is_some());
    assert!(outcome
        .status
        .ready()
        .unwrap()
        .message
        .contains("beyond the plan's 1"));

    // A renewal that fails while the old certificate still holds: the config
    // stays, the failure is reported.
    let mut renewing = p.clone();
    let mut status = outcome.status;
    status.certificate_renews = Some(world.now - 1);
    renewing.status = Some(status);
    let failing = |_| async { Err(Error::RateLimited(None)) };
    let outcome = reconcile(&renewing, &key, &world, 0, failing).await;
    assert!(outcome.conf.is_some());
    assert!(outcome
        .status
        .ready()
        .unwrap()
        .message
        .contains("renewal failed"));

    // No certificate at all: no config.
    let outcome = reconcile(&p, &key, &world, 0, failing).await;
    assert!(outcome.conf.is_none());
    assert_eq!(outcome.status.ready().unwrap().reason, "CertificateFailed");
}

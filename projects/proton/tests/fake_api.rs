//! The Proton client against an in-process fake API.
mod support;

use qrab_proton::{
    api::{Client, Error, SecondFactor, Session},
    catalog::Criteria,
    keys::ProfileKey,
    wg::{wg_conf, Features},
};
use support::{serve, Fake, HV_TOKEN, PASSWORD, TOTP, USER};

async fn logged_in(base: &str) -> (Client, Session) {
    let client = Client::with_base(base).unwrap();
    let (session, second) = client.login(USER, PASSWORD, None).await.unwrap();
    assert_eq!(second, SecondFactor::None);
    (client, session)
}

#[tokio::test]
async fn a_login_reaches_the_plan_catalog_and_a_certificate() {
    let (base, fake) = serve(Fake::new()).await;
    let (client, session) = logged_in(&base).await;
    assert!(session.has_vpn_scope());
    let plan = client.plan(&session).await.unwrap();
    assert_eq!((plan.max_tier, plan.max_connect), (2, 10));
    // The plan reply carries the account's OpenVPN credentials; nothing kept
    // shows them.
    assert!(!format!("{plan:?}").contains("openvpn-secret"));

    let mut catalog = client.logicals(&session).await.unwrap();
    catalog.apply_loads(&client.loads(&session).await.unwrap());
    let criteria = Criteria {
        port_forwarding: true,
        ..Default::default()
    };
    // CH#1 scored 2.0 in the catalog; the fresh loads make it the best P2P.
    let choice = catalog.select(&criteria, plan.max_tier, None).unwrap();
    assert_eq!((choice.name.as_str(), choice.load), ("CH#1", 12));

    let key = ProfileKey::generate().unwrap();
    let features = Features {
        port_forwarding: true,
        net_shield: 2,
        ..Features::default()
    };
    let certificate = client
        .certificate(&session, &key, 60, &features)
        .await
        .unwrap();
    assert_eq!(certificate.expiration_time, 2_000_000_000);
    let sent = fake.lock().unwrap().certificates[0].clone();
    assert_eq!(sent["ClientPublicKey"], key.public_pem());
    assert_eq!(sent["Duration"], "1440 min", "Proton's minimum is a day");
    assert_eq!(sent["Features"]["PortForwarding"], true);
    assert_eq!(sent["Features"]["NetShieldLevel"], 2);

    // The config the NIC gets parses with qrab-netd's own parser.
    let conf = wg_conf(&key, &choice, &features);
    let tunnel = qrab_netd::vpn::Tunnel::parse(&conf).unwrap();
    assert_eq!(tunnel.endpoint.to_string(), "203.0.113.1:51820");
    assert_eq!(tunnel.address.to_string(), "10.2.0.2");
    assert!(tunnel.setconf().contains(&format!(
        "PrivateKey = {}\n",
        key.wireguard_private().as_str()
    )));
    assert!(!format!("{session:?}").contains("acc") && !format!("{session:?}").contains("ref"));
}

#[tokio::test]
async fn totp_completes_a_login_and_a_wrong_code_is_refused() {
    let mut fake = Fake::new();
    fake.totp = true;
    let (base, _) = serve(fake).await;
    let client = Client::with_base(&base).unwrap();
    let (mut session, second) = client.login(USER, PASSWORD, None).await.unwrap();
    assert_eq!(second, SecondFactor::Totp);
    assert!(!session.has_vpn_scope());
    // Before the second factor nothing VPN-side works.
    assert!(client.plan(&session).await.is_err());
    assert!(matches!(
        client.submit_totp(&mut session, "000000").await,
        Err(Error::BadTwoFactor)
    ));
    client.submit_totp(&mut session, TOTP).await.unwrap();
    assert!(session.has_vpn_scope());
    assert_eq!(client.plan(&session).await.unwrap().max_tier, 2);
}

#[tokio::test]
async fn human_verification_is_surfaced_then_retried_with_its_token() {
    let mut fake = Fake::new();
    fake.human_verification = true;
    let (base, _) = serve(fake).await;
    let client = Client::with_base(&base).unwrap();
    let Err(Error::HumanVerification(challenge)) = client.login(USER, PASSWORD, None).await else {
        panic!("expected a human-verification challenge");
    };
    assert_eq!(challenge.token, HV_TOKEN);
    assert_eq!(
        challenge.url(),
        format!("https://verify.proton.me/?methods=captcha,email&token={HV_TOKEN}")
    );
    let (session, _) = client
        .login(USER, PASSWORD, Some(&challenge))
        .await
        .unwrap();
    assert!(session.has_vpn_scope());
}

#[tokio::test]
async fn a_wrong_password_is_refused_without_echoing_it() {
    let (base, fake) = serve(Fake::new()).await;
    let client = Client::with_base(&base).unwrap();
    let error = client
        .login(USER, "not the password", None)
        .await
        .unwrap_err();
    assert!(matches!(error, Error::BadCredentials), "{error}");
    assert!(!error.to_string().contains("not the password"));
    // SRP: the password never crosses the wire, in any request.
    let (_, _) = client.login(USER, PASSWORD, None).await.unwrap();
    for request in &fake.lock().unwrap().seen {
        assert!(!request.contains(PASSWORD) && !request.contains("not the password"));
    }
}

#[tokio::test]
async fn refresh_rotates_the_token_and_the_old_one_is_dead() {
    let (base, fake) = serve(Fake::new()).await;
    let (client, session) = logged_in(&base).await;
    // What the cluster stores: uid and refresh token only.
    let stored = Session::stored(&session.uid, session.refresh_token());
    let fresh = client.refresh(&stored).await.unwrap();
    assert_ne!(fresh.refresh_token(), session.refresh_token());
    assert!(client.plan(&fresh).await.is_ok());
    // The old access token no longer works, and neither does the old refresh token.
    assert!(matches!(
        client.plan(&session).await,
        Err(Error::Unauthorized)
    ));
    assert!(matches!(
        client.refresh(&stored).await,
        Err(Error::SessionRevoked)
    ));
    let again = client.refresh(&fresh).await.unwrap();
    assert!(client.plan(&again).await.is_ok());
    assert_eq!(fake.lock().unwrap().refreshes, 2);
    // A logged-out session cannot be refreshed.
    client.logout(&again).await.unwrap();
    assert!(matches!(
        client.refresh(&again).await,
        Err(Error::SessionRevoked)
    ));
}

#[tokio::test]
async fn a_retired_app_version_is_a_typed_error() {
    let mut fake = Fake::new();
    fake.app_version = "linux-vpn@9.9.9".into();
    let (base, _) = serve(fake).await;
    let client = Client::with_base(&base).unwrap();
    let error = client.login(USER, PASSWORD, None).await.unwrap_err();
    assert!(matches!(error, Error::AppVersionRejected(_)), "{error}");
    let client = client.app_version("linux-vpn@9.9.9");
    assert!(client.login(USER, PASSWORD, None).await.is_ok());
}

#[test]
fn only_https_or_loopback_bases_are_accepted() {
    for base in ["http://vpn-api.proton.me", "ftp://127.0.0.1", "not a url"] {
        assert!(
            matches!(Client::with_base(base), Err(Error::InsecureBase(_))),
            "{base}"
        );
    }
    for base in [
        "https://vpn-api.proton.me",
        "http://127.0.0.1:9",
        "http://[::1]:9",
        "http://localhost:9",
    ] {
        assert!(Client::with_base(base).is_ok(), "{base}");
    }
}

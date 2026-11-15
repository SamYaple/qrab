//! The broker's full pass against a real cluster with the fake Proton API.
//! Opt-in: `QRAB_LIVE_BROKER_KUBECONFIG` names a kubeconfig that
//! impersonates `system:serviceaccount:qrab:qrab-proton-broker` (so RBAC and
//! the admission policy apply), and `kubectl` must work as an admin for the
//! setup (it stands in for `qrab-proton login`). It creates VpnProfile
//! `broker-live`, Secret `proton-session` (only if absent) and removes what
//! it created.
mod support;

use base64::{engine::general_purpose::STANDARD, Engine};
use qrab_proton::{api::Client, broker::runner::Broker, login::secret_manifest};
use std::process::Command;
use support::{serve, Fake, PASSWORD, USER};

fn kubectl(args: &[&str], input: Option<&str>) -> String {
    use std::io::Write;
    let mut child = Command::new("kubectl")
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    } else {
        drop(child.stdin.take());
    }
    let out = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn cleanup() {
    kubectl(
        &[
            "-n",
            "qrab",
            "delete",
            "vpnprofile",
            "broker-live",
            "--ignore-not-found",
            "--wait=true",
        ],
        None,
    );
    kubectl(
        &[
            "-n",
            "qrab",
            "delete",
            "secret",
            "-l",
            "qrab.yapplesauce.com/live-test=true",
            "--ignore-not-found",
        ],
        None,
    );
}

#[tokio::test]
async fn a_full_pass_writes_the_profile_secret_and_the_catalog() {
    let Ok(kubeconfig) = std::env::var("QRAB_LIVE_BROKER_KUBECONFIG") else {
        eprintln!("skipped: QRAB_LIVE_BROKER_KUBECONFIG is not set");
        return;
    };
    assert!(
        kubectl(
            &[
                "-n",
                "qrab",
                "get",
                "secret",
                "proton-session",
                "-o",
                "name",
                "--ignore-not-found"
            ],
            None
        )
        .trim()
        .is_empty(),
        "a real proton-session exists; this test would replace it"
    );
    let exists = |kind: &str, name: &str| {
        !kubectl(
            &[
                "-n",
                "qrab",
                "get",
                kind,
                name,
                "-o",
                "name",
                "--ignore-not-found",
            ],
            None,
        )
        .trim()
        .is_empty()
    };
    // The fake's keys and catalog must not outlive the test.
    let had_keys = exists("secret", "proton-keys");
    let had_catalog = exists("configmap", "proton-catalog");
    let leftovers = move || {
        if !had_keys {
            kubectl(
                &[
                    "-n",
                    "qrab",
                    "delete",
                    "secret",
                    "proton-keys",
                    "--ignore-not-found",
                ],
                None,
            );
        }
        if !had_catalog {
            kubectl(
                &[
                    "-n",
                    "qrab",
                    "delete",
                    "configmap",
                    "proton-catalog",
                    "--ignore-not-found",
                ],
                None,
            );
        }
    };
    cleanup();
    let (base, fake) = serve(Fake::new()).await;
    let proton = Client::with_base(&base).unwrap();
    // What `qrab-proton login` would store, labelled so cleanup finds it.
    let (session, _) = proton.login(USER, PASSWORD, None).await.unwrap();
    let mut manifest: serde_json::Value =
        serde_json::from_str(&secret_manifest("qrab", &session)).unwrap();
    manifest["metadata"]["labels"]["qrab.yapplesauce.com/live-test"] = "true".into();
    kubectl(&["apply", "-f", "-"], Some(&manifest.to_string()));
    kubectl(
        &["apply", "-f", "-"],
        Some(
            r#"{"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VpnProfile",
                "metadata":{"name":"broker-live","namespace":"qrab"},
                "spec":{"proton":{"portForwarding":true}}}"#,
        ),
    );

    let config = kube::Config::from_custom_kubeconfig(
        kube::config::Kubeconfig::read_from(&kubeconfig).unwrap(),
        &Default::default(),
    )
    .await
    .unwrap();
    let client = kube::Client::try_from(config).unwrap();
    let mut broker = Broker::new(client, "qrab", proton.clone());
    broker.pass().await;

    let conf = kubectl(
        &[
            "-n",
            "qrab",
            "get",
            "secret",
            "vpn-broker-live",
            "-o",
            "jsonpath={.data.wg\\.conf}",
        ],
        None,
    );
    let conf = String::from_utf8(STANDARD.decode(conf.trim()).unwrap()).unwrap();
    let tunnel = qrab_netd::vpn::Tunnel::parse(&conf).unwrap();
    assert_eq!(tunnel.endpoint.to_string(), "203.0.113.1:51820");
    let status = kubectl(
        &[
            "-n",
            "qrab",
            "get",
            "vpnprofile",
            "broker-live",
            "-o",
            "jsonpath={.status}",
        ],
        None,
    );
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert_eq!(status["server"], "CH#1");
    assert_eq!(status["conditions"][0]["status"], "True", "{status}");
    // The stored session rotated, and it is the one the fake knows.
    let stored = kubectl(
        &[
            "-n",
            "qrab",
            "get",
            "secret",
            "proton-session",
            "-o",
            "jsonpath={.data.refresh_token}",
        ],
        None,
    );
    let stored = String::from_utf8(STANDARD.decode(stored.trim()).unwrap()).unwrap();
    assert_ne!(stored, session.refresh_token());
    assert!(fake
        .lock()
        .unwrap()
        .sessions
        .values()
        .any(|s| s.refresh == stored));
    // The catalog is published.
    let catalog = kubectl(
        &[
            "-n",
            "qrab",
            "get",
            "configmap",
            "proton-catalog",
            "-o",
            "jsonpath={.binaryData.catalog\\.json\\.gz}",
        ],
        None,
    );
    let catalog = qrab_proton::catalog::decode(&STANDARD.decode(catalog.trim()).unwrap()).unwrap();
    assert_eq!(catalog.servers.len(), 3);

    // A manual switch is applied on the next pass, same key.
    kubectl(
        &[
            "-n",
            "qrab",
            "patch",
            "vpnprofile",
            "broker-live",
            "--type",
            "merge",
            "-p",
            r#"{"spec":{"proton":{"server":"SE#7"}}}"#,
        ],
        None,
    );
    broker.pass().await;
    let switched = kubectl(
        &[
            "-n",
            "qrab",
            "get",
            "secret",
            "vpn-broker-live",
            "-o",
            "jsonpath={.data.wg\\.conf}",
        ],
        None,
    );
    let switched = String::from_utf8(STANDARD.decode(switched.trim()).unwrap()).unwrap();
    assert!(
        switched.contains("Endpoint = 203.0.113.7:51820"),
        "{switched}"
    );
    let key_line = |c: &str| {
        c.lines()
            .find(|l| l.starts_with("PrivateKey"))
            .unwrap()
            .to_owned()
    };
    assert_eq!(key_line(&switched), key_line(&conf));
    assert_eq!(fake.lock().unwrap().certificates.len(), 1);

    cleanup();
    // The profile Secret goes with its profile.
    for _ in 0..30 {
        if kubectl(
            &[
                "-n",
                "qrab",
                "get",
                "secret",
                "vpn-broker-live",
                "-o",
                "name",
                "--ignore-not-found",
            ],
            None,
        )
        .trim()
        .is_empty()
        {
            leftovers();
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    panic!("vpn-broker-live outlived its profile");
}

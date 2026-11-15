//! The interactive login against the fake Proton API, with scripted input.
mod support;

use qrab_proton::{
    api::{Client, Error, Session},
    login::{self, secret_manifest, Prompt},
};
use std::collections::VecDeque;
use support::{serve, Fake, PASSWORD, TOTP, USER};
use zeroize::Zeroizing;

#[derive(Default)]
struct Script {
    answers: VecDeque<String>,
    asked: Vec<String>,
    said: Vec<String>,
}

impl Script {
    fn new(answers: &[&str]) -> Self {
        Self {
            answers: answers.iter().map(|a| a.to_string()).collect(),
            ..Default::default()
        }
    }
}

impl Prompt for Script {
    fn line(&mut self, prompt: &str) -> std::io::Result<String> {
        self.asked.push(prompt.to_owned());
        self.answers
            .pop_front()
            .ok_or_else(|| std::io::Error::other(format!("no scripted answer for {prompt:?}")))
    }
    fn secret(&mut self, prompt: &str) -> std::io::Result<Zeroizing<String>> {
        self.line(&format!("(secret) {prompt}")).map(Zeroizing::new)
    }
    fn say(&mut self, message: &str) {
        self.said.push(message.to_owned());
    }
}

#[tokio::test]
async fn a_login_with_verification_and_totp_yields_only_uid_and_refresh_token() {
    let mut fake = Fake::new();
    fake.totp = true;
    fake.human_verification = true;
    let (base, shared) = serve(fake).await;
    let client = Client::with_base(&base).unwrap();
    // Email, password, Enter after the browser step, a wrong code, the right one.
    let mut script = Script::new(&[USER, PASSWORD, "", "000000", TOTP]);
    let logged_in = login::login(&client, &mut script).await.unwrap();
    assert_eq!(
        script.asked,
        [
            "Proton email: ",
            "(secret) Password: ",
            "",
            "(secret) Two-factor code: ",
            "(secret) Two-factor code: "
        ]
    );
    assert!(script.said[0].contains("https://verify.proton.me/?methods=captcha,email&token="));
    assert!(script.said[1].contains("refused"));
    assert_eq!(logged_in.plan.max_connect, 10);

    let manifest: serde_json::Value =
        serde_json::from_str(&secret_manifest("qrab", &logged_in.session)).unwrap();
    let data = manifest["stringData"].as_object().unwrap();
    let mut keys: Vec<&str> = data.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, ["refresh_token", "uid"]);
    let text = manifest.to_string();
    assert!(!text.contains(PASSWORD) && !text.contains(TOTP));
    // The manifest's session is live: the broker can take it over.
    let stored = Session::stored(
        data["uid"].as_str().unwrap(),
        data["refresh_token"].as_str().unwrap(),
    );
    let live = client.refresh(&stored).await.unwrap();
    assert!(client.plan(&live).await.is_ok());

    // Logging out revokes it at Proton.
    login::logout(&client, &live).await.unwrap();
    assert!(shared.lock().unwrap().sessions.is_empty());
}

#[tokio::test]
async fn repeated_verification_challenges_end_the_login() {
    let mut fake = Fake::new();
    fake.always_challenge = true;
    let (base, _) = serve(fake).await;
    let client = Client::with_base(&base).unwrap();
    let mut script = Script::new(&[USER, PASSWORD, "", "", "", ""]);
    let error = login::login(&client, &mut script).await.err().unwrap();
    assert!(matches!(error, Error::HumanVerification(_)), "{error}");
    // Three browser rounds, then the challenge is reported, not looped on.
    assert_eq!(script.said.len(), 3);
    assert_eq!(script.answers.len(), 1);
}

#[tokio::test]
async fn an_account_without_a_vpn_plan_is_refused() {
    let mut fake = Fake::new();
    fake.no_vpn_scope = true;
    let (base, _) = serve(fake).await;
    let client = Client::with_base(&base).unwrap();
    let mut script = Script::new(&[USER, PASSWORD]);
    let error = login::login(&client, &mut script).await.err().unwrap();
    assert!(error.to_string().contains("no VPN scope"), "{error}");
}

#[tokio::test]
async fn a_wrong_password_ends_the_login_without_a_session() {
    let (base, shared) = serve(Fake::new()).await;
    let client = Client::with_base(&base).unwrap();
    let mut script = Script::new(&[USER, "wrong"]);
    let error = login::login(&client, &mut script).await.err().unwrap();
    assert!(matches!(error, Error::BadCredentials), "{error}");
    assert!(shared.lock().unwrap().sessions.is_empty());
}

#[tokio::test]
async fn the_command_prints_a_session_secret_from_piped_answers() {
    use std::io::Write;
    let (base, _) = serve(Fake::new()).await;
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_qrab-proton"))
        .args(["login", "--print", "--namespace", "vms", "--api", &base])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(format!("{USER}\n{PASSWORD}\n").as_bytes())
        .unwrap();
    let output = tokio::task::spawn_blocking(move || child.wait_with_output().unwrap())
        .await
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        stderr.contains("plan visionary2022, tier 2, up to 10 connections"),
        "{stderr}"
    );
    let manifest: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(manifest["metadata"]["namespace"], "vms");
    assert!(manifest["stringData"]["refresh_token"].as_str().is_some());
    let everything = format!("{stderr}{}", String::from_utf8_lossy(&output.stdout));
    assert!(!everything.contains(PASSWORD));
}

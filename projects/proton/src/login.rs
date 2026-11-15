//! The workstation side: log in once, hand the session to the cluster.
//!
//! The password and TOTP code live only in this process. What leaves it is
//! the Secret `proton-session` with the session's UID and refresh token,
//! which from then on belongs to the broker alone: Proton rotates the refresh
//! token on every refresh, and two holders refreshing copies of one session
//! would revoke it, so this command never refreshes a session it handed over.
//!
//! ```
//! use qrab_proton::{api::Session, login::secret_manifest};
//! let manifest = secret_manifest("qrab", &Session::stored("uid-1", "refresh-1"));
//! let value: serde_json::Value = serde_json::from_str(&manifest).unwrap();
//! assert_eq!(value["metadata"]["name"], "proton-session");
//! assert_eq!(value["stringData"], serde_json::json!({"uid": "uid-1", "refresh_token": "refresh-1"}));
//! ```
use crate::api::{Client, Error, SecondFactor, Session, VpnPlan};
use serde_json::json;
use zeroize::Zeroizing;

/// The Secret holding the session, in the VPN NICs' namespace.
pub const SESSION_SECRET: &str = "proton-session";
pub const SESSION_LABEL: &str = "qrab.yapplesauce.com/proton-session";
/// Human-verification rounds before giving up.
const VERIFY_ATTEMPTS: usize = 3;

/// How the login talks to the person running it.
pub trait Prompt {
    fn line(&mut self, prompt: &str) -> std::io::Result<String>;
    /// Read without echo.
    fn secret(&mut self, prompt: &str) -> std::io::Result<Zeroizing<String>>;
    fn say(&mut self, message: &str);
}

/// A logged-in session and the plan it carries.
pub struct LoggedIn {
    pub session: Session,
    pub plan: VpnPlan,
}

/// Log in interactively: email, password, a TOTP code when the account asks
/// for one, and human verification in a browser when Proton asks for it.
pub async fn login(client: &Client, prompt: &mut impl Prompt) -> Result<LoggedIn, Error> {
    let io = |e: std::io::Error| Error::Transport(format!("reading the terminal: {e}"));
    let email = prompt.line("Proton email: ").map_err(io)?;
    let email = email.trim();
    let password = prompt.secret("Password: ").map_err(io)?;
    let mut verified = None;
    let mut challenges = 0;
    let (mut session, second) = loop {
        match client.login(email, &password, verified.as_ref()).await {
            Err(Error::HumanVerification(challenge)) if challenges < VERIFY_ATTEMPTS => {
                challenges += 1;
                prompt.say(&format!(
                    "Proton wants to verify you are human. Open this link, complete it, then press Enter:\n  {}",
                    challenge.url()
                ));
                prompt.line("").map_err(io)?;
                verified = Some(challenge);
            }
            other => break other?,
        }
    };
    drop(password);
    if second == SecondFactor::Totp {
        let mut tries = 0;
        loop {
            let code = prompt.secret("Two-factor code: ").map_err(io)?;
            match client.submit_totp(&mut session, &code).await {
                Ok(()) => break,
                Err(Error::BadTwoFactor) if tries < 2 => {
                    tries += 1;
                    prompt.say("That code was refused; try the next one.");
                }
                Err(other) => return Err(other),
            }
        }
    }
    if !session.has_vpn_scope() {
        return Err(Error::Api {
            status: 403,
            code: 0,
            message: "the session has no VPN scope; does the account have a VPN plan?".into(),
        });
    }
    let plan = client.plan(&session).await?;
    Ok(LoggedIn { session, plan })
}

/// The Secret manifest (JSON, which kubectl accepts) for `session`: UID and
/// refresh token, nothing else.
pub fn secret_manifest(namespace: &str, session: &Session) -> String {
    json!({
        "apiVersion": "v1",
        "kind": "Secret",
        "metadata": {
            "name": SESSION_SECRET,
            "namespace": namespace,
            "labels": {SESSION_LABEL: "true"},
        },
        "type": "Opaque",
        "stringData": {"uid": session.uid, "refresh_token": session.refresh_token()},
    })
    .to_string()
}

/// End a stored session at Proton. Stop the broker first: this refreshes
/// the session to get an access token, which the broker's next refresh
/// would then race.
pub async fn logout(client: &Client, stored: &Session) -> Result<(), Error> {
    let live = client.refresh(stored).await?;
    client.logout(&live).await
}

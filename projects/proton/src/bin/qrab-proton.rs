//! `qrab-proton`: connect the cluster to a Proton account.
use qrab_proton::{
    api::{Client, Session},
    login::{self, Prompt, SESSION_SECRET},
};
use std::io::{BufRead, IsTerminal, Write};
use std::process::{Command, ExitCode, Stdio};
use zeroize::Zeroizing;

const HELP: &str = "\
qrab-proton: ProtonVPN for qrab VPN NICs

USAGE:
  qrab-proton login  [--namespace NS] [--print] [--api URL]
  qrab-proton logout [--namespace NS] [--api URL]

login   Log in to Proton (email, password, a TOTP code if the account has
        two-factor, human verification in a browser if Proton asks) and store
        ONLY the session (UID and refresh token) in Secret NS/proton-session
        with `kubectl apply`. The password and TOTP code never leave this
        process. --print writes the Secret manifest to stdout instead.
logout  Revoke the stored session at Proton and delete the Secret. Scale the
        broker to zero first: logging out refreshes the session, which a
        running broker would race.

OPTIONS:
  --namespace NS  The VPN NICs' namespace (default: qrab)
  --api URL       The Proton API base (default: https://vpn-api.proton.me)
";

struct Terminal;

impl Prompt for Terminal {
    fn line(&mut self, prompt: &str) -> std::io::Result<String> {
        eprint!("{prompt}");
        std::io::stderr().flush()?;
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line)?;
        Ok(line.trim_end_matches(['\r', '\n']).to_owned())
    }

    fn secret(&mut self, prompt: &str) -> std::io::Result<Zeroizing<String>> {
        use nix::sys::termios::{tcgetattr, tcsetattr, LocalFlags, SetArg};
        let stdin = std::io::stdin();
        let saved = stdin.is_terminal().then(|| tcgetattr(&stdin)).transpose()?;
        if let Some(saved) = &saved {
            let mut quiet = saved.clone();
            quiet.local_flags.remove(LocalFlags::ECHO);
            tcsetattr(&stdin, SetArg::TCSANOW, &quiet)?;
        }
        let read = self.line(prompt).map(Zeroizing::new);
        if let Some(saved) = &saved {
            tcsetattr(&stdin, SetArg::TCSANOW, saved)?;
            eprintln!();
        }
        read
    }

    fn say(&mut self, message: &str) {
        eprintln!("{message}");
    }
}

struct Options {
    namespace: String,
    print: bool,
    api: Option<String>,
}

fn options(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        namespace: "qrab".into(),
        print: false,
        api: None,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--namespace" | "-n" => {
                options.namespace = args.next().ok_or("--namespace needs a value")?.clone()
            }
            "--print" => options.print = true,
            "--api" => options.api = Some(args.next().ok_or("--api needs a value")?.clone()),
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(options)
}

fn kubectl(args: &[&str], input: Option<&str>) -> Result<String, String> {
    let mut child = Command::new("kubectl")
        .args(args)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run kubectl: {e}"))?;
    if let Some(input) = input {
        // Through stdin, never argv: the token must not show in `ps`.
        child
            .stdin
            .take()
            .expect("piped")
            .write_all(input.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!(
            "kubectl {}: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn stored_session(namespace: &str) -> Result<Session, String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let json = kubectl(
        &[
            "-n",
            namespace,
            "get",
            "secret",
            SESSION_SECRET,
            "-o",
            "json",
        ],
        None,
    )?;
    let value: serde_json::Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let field = |name: &str| -> Result<String, String> {
        let encoded = value["data"][name]
            .as_str()
            .ok_or(format!("the Secret has no {name}"))?;
        String::from_utf8(STANDARD.decode(encoded).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    };
    Ok(Session::stored(&field("uid")?, &field("refresh_token")?))
}

fn client(options: &Options) -> Result<Client, String> {
    match &options.api {
        Some(base) => Client::with_base(base),
        None => Client::new(),
    }
    .map_err(|e| e.to_string())
}

async fn run(args: Vec<String>) -> Result<(), String> {
    let (command, rest) = args.split_first().ok_or(HELP)?;
    if rest.iter().any(|a| a == "--help" || a == "-h") || command == "help" {
        print!("{HELP}");
        return Ok(());
    }
    let options = options(rest)?;
    match command.as_str() {
        "login" => {
            let client = client(&options)?;
            let logged_in = login::login(&client, &mut Terminal)
                .await
                .map_err(|e| e.to_string())?;
            let plan = &logged_in.plan;
            eprintln!(
                "Logged in: plan {}, tier {}, up to {} connections.",
                plan.plan_name.as_deref().unwrap_or("?"),
                plan.max_tier,
                plan.max_connect
            );
            let manifest = login::secret_manifest(&options.namespace, &logged_in.session);
            if options.print {
                println!("{manifest}");
                return Ok(());
            }
            if kubectl(
                &[
                    "-n",
                    &options.namespace,
                    "get",
                    "secret",
                    SESSION_SECRET,
                    "-o",
                    "name",
                ],
                None,
            )
            .is_ok()
            {
                eprintln!(
                    "Replacing the stored session. The old one stays in Proton's session list \
                     until it expires; remove it there if you like."
                );
            }
            kubectl(&["apply", "-f", "-"], Some(&manifest))?;
            eprintln!(
                "Stored Secret {}/{SESSION_SECRET} (UID and refresh token only).",
                options.namespace
            );
            Ok(())
        }
        "logout" => {
            let client = client(&options)?;
            let stored = stored_session(&options.namespace)?;
            login::logout(&client, &stored)
                .await
                .map_err(|e| e.to_string())?;
            kubectl(
                &["-n", &options.namespace, "delete", "secret", SESSION_SECRET],
                None,
            )?;
            eprintln!("Revoked the session and deleted the Secret.");
            Ok(())
        }
        other => Err(format!("unknown command {other:?}\n\n{HELP}")),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("qrab-proton: {error}");
            ExitCode::FAILURE
        }
    }
}

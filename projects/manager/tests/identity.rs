//! Guest identity provisioning against a real swtpm and a real qrab-tpm-issuer
//! behind SPIFFE mTLS (Workload API fixtures). Every TPM result is checked
//! independently with tpm2-tools and openssl, using SPIRE's own SRK template
//! (`tpm2_createprimary -C o -G rsa2048:null:aes128cfb -a ...|noda`, the
//! native provisioning runbook) and its raw blob formats.
use manager::pod::{
    identity::{self, Bundle, Marker, Provisioned, Vtpm},
    Config, GuestIdentity,
};
use qrab_spiffe::fixture::{Authority, Workload};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Arc,
    time::Duration,
};

const UID: &str = "0b6f2a3e-5c1d-4e8f-9a7b-1c2d3e4f5a6b";
const ISSUER_ID: &str = "spiffe://yapplesauce.com/kubernetes/qrab-tpm-issuer/qrab-tpm-issuer";
const SWTPM: &str = "/usr/bin/swtpm";

fn swtpm_present() -> bool {
    if Path::new(SWTPM).is_file() {
        return true;
    }
    eprintln!("\n\n*** SKIPPED: {SWTPM} is missing; guest identity provisioning is UNTESTED ***\n");
    false
}

fn run(command: &mut Command) -> Vec<u8> {
    let output = command
        .output()
        .unwrap_or_else(|e| panic!("{command:?}: {e} (tpm2-tools and openssl are required)"));
    assert!(
        output.status.success(),
        "{command:?}: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn write_ca(dir: &Path, name: &str, cn: &str) {
    run(Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
        ])
        .args(["-nodes", "-days", "3650", "-subj"])
        .arg(format!("/CN={cn}"))
        .args(["-addext", "basicConstraints=critical,CA:TRUE,pathlen:0"])
        .args(["-addext", "keyUsage=critical,keyCertSign,cRLSign"])
        .arg("-keyout")
        .arg(dir.join(format!("{name}.key")))
        .arg("-out")
        .arg(dir.join(format!("{name}.pem"))));
}

/// qrab-tpm-issuer as deployed: its own SVID, admitting only vm/<uuid>.
struct Issuer {
    ca: tempfile::TempDir,
    state: tempfile::TempDir,
    authority: Authority,
    address: std::net::SocketAddr,
    _api: Workload,
    task: tokio::task::JoinHandle<()>,
}

impl Issuer {
    async fn start() -> Self {
        qrab_spiffe::http::install_tls13_provider().unwrap();
        let ca = tempfile::tempdir().unwrap();
        write_ca(ca.path(), "ek-ca", "qrab-endorsement-ca-1");
        write_ca(ca.path(), "devid-ca", "qrab-devid-ca-1");
        let state = tempfile::tempdir().unwrap();
        let issuer = Arc::new(
            qrab_tpm_issuer::Issuer::load(ca.path(), &state.path().join("records"), false).unwrap(),
        );
        let authority = Authority::new();
        let api = Workload::serve(vec![authority.svid(ISSUER_ID)]);
        let identity = qrab_spiffe::Identity::connect(&api.endpoint).await.unwrap();
        let listener = qrab_tpm_issuer::bind("127.0.0.1:0".parse().unwrap(), &identity)
            .await
            .unwrap();
        let address = axum::serve::Listener::local_addr(&listener).unwrap();
        let task = tokio::spawn(async move {
            axum::serve(
                listener,
                qrab_tpm_issuer::router(issuer)
                    .into_make_service_with_connect_info::<qrab_spiffe::http::Peer>(),
            )
            .await
            .unwrap();
        });
        Self {
            ca,
            state,
            authority,
            address,
            _api: api,
            task,
        }
    }

    /// The manager's side: its vm/<uid> Workload API and settings.
    async fn vm(&self, dir: &Path) -> (Workload, qrab_spiffe::Identity, GuestIdentity) {
        let api = Workload::serve(vec![self.authority.svid(&format!(
            "spiffe://yapplesauce.com/kubernetes/qrab/vm/{UID}"
        ))]);
        let identity = qrab_spiffe::Identity::connect(&api.endpoint).await.unwrap();
        let settings = GuestIdentity {
            enabled: true,
            issuer_url: format!("https://{}", self.address),
            issuer_id: ISSUER_ID.into(),
            workload_api: Some(api.endpoint.clone()),
            guest_tools_dir: dir.join("tools"),
        };
        (api, identity, settings)
    }
}

/// A VM's retained state and runtime directories.
struct Vm {
    dir: tempfile::TempDir,
}

impl Vm {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("state/identity/tpm")).unwrap();
        std::fs::create_dir_all(dir.path().join("run")).unwrap();
        Self { dir }
    }
    fn tpm(&self) -> PathBuf {
        self.dir.path().join("state/identity/tpm")
    }
    fn guest(&self) -> PathBuf {
        identity::guest_dir(&self.dir.path().join("state"))
    }
    fn run(&self) -> PathBuf {
        self.dir.path().join("run")
    }
    async fn provision(
        &self,
        settings: &GuestIdentity,
        identity: &qrab_spiffe::Identity,
    ) -> manager::Result<Provisioned> {
        let swtpm = PathBuf::from(SWTPM);
        let (tpm, guest, run) = (self.tpm(), self.guest(), self.run());
        let vtpm = Vtpm {
            swtpm_binary: &swtpm,
            tpm_state: &tpm,
            guest: &guest,
            socket_dir: &run,
        };
        identity::provision(&vtpm, settings, identity).await
    }
    fn file(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.guest().join(name)).unwrap()
    }
}

/// An independent swtpm on the same state, driven by tpm2-tools.
struct Tools {
    child: Child,
    tcti: String,
    work: tempfile::TempDir,
}

impl Tools {
    fn start(state: &Path) -> Self {
        let work = tempfile::tempdir().unwrap();
        let socket = work.path().join("t.sock");
        let child = Command::new(SWTPM)
            .args(["socket", "--tpm2", "--tpmstate"])
            .arg(format!("dir={}", state.display()))
            .arg("--server")
            .arg(format!("type=unixio,path={}", socket.display()))
            .arg("--ctrl")
            .arg(format!("type=unixio,path={}.ctrl", socket.display()))
            .args(["--flags", "not-need-init,startup-clear"])
            .stdin(Stdio::null())
            .spawn()
            .unwrap();
        for _ in 0..400 {
            if socket.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        Self {
            child,
            tcti: format!("swtpm:path={}", socket.display()),
            work,
        }
    }
    fn path(&self, name: &str) -> PathBuf {
        self.work.path().join(name)
    }
    fn tpm2(&self, tool: &str, args: &[&str]) -> Vec<u8> {
        run(Command::new(tool)
            .args(args)
            .env("TPM2TOOLS_TCTI", &self.tcti)
            .current_dir(self.work.path()))
    }
    fn try_tpm2(&self, tool: &str, args: &[&str]) -> bool {
        Command::new(tool)
            .args(args)
            .env("TPM2TOOLS_TCTI", &self.tcti)
            .current_dir(self.work.path())
            .output()
            .unwrap()
            .status
            .success()
    }
    /// The EK `tpm2_createek -G rsa` derives, as a bare TPMT_PUBLIC.
    fn ek_public(&self) -> Vec<u8> {
        self.tpm2(
            "tpm2_createek",
            &["-c", "ek.ctx", "-G", "rsa", "-u", "ek.pub"],
        );
        self.tpm2("tpm2_flushcontext", &["-t"]);
        let blob = std::fs::read(self.path("ek.pub")).unwrap();
        assert_eq!(
            usize::from(u16::from_be_bytes([blob[0], blob[1]])),
            blob.len() - 2
        );
        blob[2..].to_vec()
    }
    fn stop(self) {
        drop(self)
    }
}

impl Drop for Tools {
    // Also on a failed assertion: a leaked swtpm would hold the test's
    // stdout open. SIGKILL, because swtpm 0.10 can miss a SIGTERM right
    // after a client disconnects; libtpms has already stored every NV change.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn digest_tree(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_owned()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let digest = Sha256::digest(std::fs::read(&path).unwrap()).to_vec();
                out.insert(path, digest);
            }
        }
    }
    out
}

fn tpm2b(bytes: &[u8]) -> Vec<u8> {
    let mut out = (bytes.len() as u16).to_be_bytes().to_vec();
    out.extend_from_slice(bytes);
    out
}

fn openssl_verify(ca: &Path, leaf_der: &[u8], work: &Path) {
    let der = work.join("leaf.der");
    let pem = work.join("leaf.pem");
    std::fs::write(&der, leaf_der).unwrap();
    run(Command::new("openssl")
        .args(["x509", "-inform", "der", "-in"])
        .arg(&der)
        .arg("-out")
        .arg(&pem));
    let out = run(Command::new("openssl")
        .args(["verify", "-x509_strict", "-CAfile"])
        .arg(ca)
        .arg(&pem));
    assert!(String::from_utf8_lossy(&out).contains(": OK"));
}

#[tokio::test]
async fn a_fresh_vtpm_is_provisioned_once_and_verifies_with_tpm2_tools() {
    if !swtpm_present() {
        return;
    }
    let issuer = Issuer::start().await;
    let vm = Vm::new();
    let (_api, identity, settings) = issuer.vm(vm.dir.path()).await;

    let Provisioned::New(marker) = vm.provision(&settings, &identity).await.unwrap() else {
        panic!("a fresh TPM must be provisioned");
    };
    assert!(!marker.replaced_ek_certificate);
    let (ek_der, devid_pem) = (vm.file("ek.der"), vm.file("devid.pem"));

    let tools = Tools::start(&vm.tpm());
    // The EK is the TCG L-1 EK that tpm2-tools (and SPIRE) regenerate.
    let ek_public = tools.ek_public();
    assert_eq!(ek_public, vm.file("ek.pub"));
    assert_eq!(marker.ek_public_sha256, hex(&Sha256::digest(&ek_public)));
    // The EK certificate at 0x01c00002 is the issued one and chains to the
    // endorsement CA.
    tools.tpm2("tpm2_nvread", &["0x01c00002", "-o", "nv-ek.der"]);
    assert_eq!(std::fs::read(tools.path("nv-ek.der")).unwrap(), ek_der);
    openssl_verify(
        &issuer.ca.path().join("ek-ca.pem"),
        &ek_der,
        tools.work.path(),
    );
    let readpublic = String::from_utf8(tools.tpm2("tpm2_nvreadpublic", &["0x01c00002"])).unwrap();
    assert!(
        readpublic.contains("platformcreate") && readpublic.contains("writelocked"),
        "{readpublic}"
    );
    // The raw blobs load under SPIRE's SRKTemplateHighRSA (as the runbook
    // creates it) once their TPM2B prefixes are restored, and sign; the
    // signature verifies with the DevID certificate's public key.
    std::fs::write(tools.path("devid.pub"), tpm2b(&vm.file("devid.pub"))).unwrap();
    std::fs::write(tools.path("devid.priv"), tpm2b(&vm.file("devid.priv"))).unwrap();
    std::fs::write(tools.path("devid.pem"), &devid_pem).unwrap();
    std::fs::write(tools.path("msg"), b"qrab guest identity").unwrap();
    tools.tpm2(
        "tpm2_createprimary",
        &[
            "-C",
            "o",
            "-G",
            "rsa2048:null:aes128cfb",
            "-a",
            "fixedtpm|fixedparent|sensitivedataorigin|userwithauth|restricted|decrypt|noda",
            "-c",
            "srk.ctx",
        ],
    );
    // No resource manager: free the transient slots between tools.
    tools.tpm2("tpm2_flushcontext", &["-t"]);
    tools.tpm2(
        "tpm2_load",
        &[
            "-C",
            "srk.ctx",
            "-u",
            "devid.pub",
            "-r",
            "devid.priv",
            "-c",
            "devid.ctx",
        ],
    );
    tools.tpm2("tpm2_flushcontext", &["-t"]);
    tools.tpm2(
        "tpm2_sign",
        &[
            "-c",
            "devid.ctx",
            "-g",
            "sha256",
            "-s",
            "rsassa",
            "-f",
            "plain",
            "-o",
            "sig",
            "msg",
        ],
    );
    let key = run(Command::new("openssl")
        .args(["x509", "-pubkey", "-noout", "-in"])
        .arg(tools.path("devid.pem")));
    std::fs::write(tools.path("devid-key.pem"), key).unwrap();
    let verified = run(Command::new("openssl")
        .args(["dgst", "-sha256", "-verify"])
        .arg(tools.path("devid-key.pem"))
        .arg("-signature")
        .arg(tools.path("sig"))
        .arg(tools.path("msg")));
    assert!(String::from_utf8_lossy(&verified).contains("Verified OK"));
    // The DevID certificate chains to the DevID CA with the expected CN.
    let devid_der = vm.file("devid.der");
    openssl_verify(
        &issuer.ca.path().join("devid-ca.pem"),
        &devid_der,
        tools.work.path(),
    );
    let subject = run(Command::new("openssl")
        .args(["x509", "-noout", "-subject", "-in"])
        .arg(tools.path("devid.pem")));
    assert!(String::from_utf8_lossy(&subject).contains(&format!("qrab-vm-{UID}")));
    // Under an SRK of a different template (no noDA, as tpm2-openssl's
    // default) the blob must not load: the parent template is load-bearing.
    tools.tpm2("tpm2_flushcontext", &["-t"]);
    tools.tpm2(
        "tpm2_createprimary",
        &[
            "-C",
            "o",
            "-G",
            "rsa2048:null:aes128cfb",
            "-a",
            "fixedtpm|fixedparent|sensitivedataorigin|userwithauth|restricted|decrypt",
            "-c",
            "other.ctx",
        ],
    );
    tools.tpm2("tpm2_flushcontext", &["-t"]);
    assert!(!tools.try_tpm2(
        "tpm2_load",
        &[
            "-C",
            "other.ctx",
            "-u",
            "devid.pub",
            "-r",
            "devid.priv",
            "-c",
            "x.ctx"
        ],
    ));
    tools.stop();

    // A second run is a no-op: no TPM or issuer change, even with the
    // issuer gone.
    let tpm_before = digest_tree(&vm.tpm());
    let issuer_before = digest_tree(issuer.state.path());
    issuer.task.abort();
    let mut gone = settings.clone();
    gone.issuer_url = "https://127.0.0.1:1".into();
    assert_eq!(
        vm.provision(&gone, &identity).await.unwrap(),
        Provisioned::Retained(marker.clone())
    );
    assert_eq!(digest_tree(&vm.tpm()), tpm_before);
    assert_eq!(digest_tree(issuer.state.path()), issuer_before);
    assert_eq!(Marker::read(&vm.guest()).unwrap(), Some(marker));

    // The disk carries what agent.conf names, and the trust bundle.
    let bundle = Bundle::of(&identity).unwrap();
    assert_eq!(bundle.trust_domain, "yapplesauce.com");
    assert!(bundle.pem.starts_with("-----BEGIN CERTIFICATE-----\n"));
    let image = vm.guest().join(identity::IMAGE);
    identity::write_image(&vm.guest(), &bundle, &settings.guest_tools_dir, &image).unwrap();
    let volume = fatfs::FileSystem::new(
        std::fs::File::open(&image).unwrap(),
        fatfs::FsOptions::new(),
    )
    .unwrap();
    let read = |name: &str| {
        let mut bytes = Vec::new();
        volume
            .root_dir()
            .open_file(name)
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        bytes
    };
    assert_eq!(read("devid.pub"), vm.file("devid.pub"));
    assert_eq!(read("devid.priv"), vm.file("devid.priv"));
    assert_eq!(read("devid.pem"), devid_pem);
    assert_eq!(read("bundle.pem"), bundle.pem.as_bytes());
    let conf = String::from_utf8(read("agent.conf")).unwrap();
    assert!(conf.contains(r#"trust_domain = "yapplesauce.com""#));
    assert!(conf.contains(r#"devid_priv_path = "/run/qrab-identity/devid.priv""#));
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[tokio::test]
async fn existing_state_keeps_its_ek_and_only_a_platform_certificate_is_replaced() {
    if !swtpm_present() {
        return;
    }
    let issuer = Issuer::start().await;
    let vm = Vm::new();
    let (_api, identity, settings) = issuer.vm(vm.dir.path()).await;
    // A VM that ran before asking for guest identity, with an older
    // platform-provisioned (swtpm_setup-style) EK certificate.
    let tools = Tools::start(&vm.tpm());
    let ek_before = tools.ek_public();
    std::fs::write(tools.path("old.der"), b"old certificate").unwrap();
    tools.tpm2(
        "tpm2_nvdefine",
        &[
            "0x01c00002",
            "-C",
            "p",
            "-s",
            "15",
            "-a",
            "platformcreate|authread|ownerread|ppread|ppwrite|no_da|writedefine",
        ],
    );
    tools.tpm2("tpm2_nvwrite", &["0x01c00002", "-C", "p", "-i", "old.der"]);
    tools.stop();

    let Provisioned::New(marker) = vm.provision(&settings, &identity).await.unwrap() else {
        panic!("provisioning expected");
    };
    assert!(marker.replaced_ek_certificate);
    assert_eq!(vm.file("previous-ek-cert.der"), b"old certificate");
    assert_eq!(vm.file("ek.pub"), ek_before);
    let tools = Tools::start(&vm.tpm());
    assert_eq!(tools.ek_public(), ek_before, "the EK never changes");
    tools.tpm2("tpm2_nvread", &["0x01c00002", "-o", "nv-ek.der"]);
    assert_eq!(
        std::fs::read(tools.path("nv-ek.der")).unwrap(),
        vm.file("ek.der")
    );
    tools.stop();
}

#[tokio::test]
async fn an_owner_defined_ek_certificate_index_refuses_without_a_marker() {
    if !swtpm_present() {
        return;
    }
    let issuer = Issuer::start().await;
    let vm = Vm::new();
    let (_api, identity, settings) = issuer.vm(vm.dir.path()).await;
    let tools = Tools::start(&vm.tpm());
    let ek_before = tools.ek_public();
    tools.tpm2(
        "tpm2_nvdefine",
        &[
            "0x01c00002",
            "-C",
            "o",
            "-s",
            "8",
            "-a",
            "ownerwrite|ownerread|authread|authwrite",
        ],
    );
    tools.stop();
    let error = vm.provision(&settings, &identity).await.unwrap_err();
    assert!(error.to_string().contains("owner-defined"), "{error}");
    assert_eq!(Marker::read(&vm.guest()).unwrap(), None);
    let tools = Tools::start(&vm.tpm());
    assert_eq!(tools.ek_public(), ek_before);
    tools.stop();
}

#[test]
fn qemu_gains_the_read_only_identity_disk_only_when_enabled() {
    let base = serde_json::json!({"kernel":"/unused","tpm":{},"state_dir":"/var/lib/qrab"});
    let plain: Config = serde_json::from_value(base.clone()).unwrap();
    let mut value = base.clone();
    value["guest_identity"] = serde_json::json!({"enabled":true});
    let enabled: Config = serde_json::from_value(value).unwrap();
    let mut value = base;
    value["guest_identity"] = serde_json::json!({"enabled":false,"issuer_url":"https://elsewhere"});
    let disabled: Config = serde_json::from_value(value).unwrap();

    let socket = Path::new("/run/qrab/session/qmp.sock");
    let plain = plain.arguments(socket);
    assert_eq!(disabled.arguments(socket), plain);
    assert!(!plain.iter().any(|a| a.contains("identity")));
    let with = enabled.arguments(socket);
    let added: Vec<&String> = with.iter().filter(|a| !plain.contains(a)).collect();
    // "-device" is already in the plain argv (the guest agent's bus).
    assert_eq!(added.len(), 5, "{added:?}");
    assert_eq!(
        (added[0], added[2]),
        (&"-blockdev".to_string(), &"-blockdev".to_string())
    );
    assert!(added[1].contains(r#""filename":"/var/lib/qrab/identity/guest/identity.img""#));
    assert!(added[1].contains(r#""read-only":true"#) && added[3].contains(r#""read-only":true"#));
    assert_eq!(
        added[4],
        "virtio-blk-pci,drive=identity,id=qrab-identity,serial=qrab-identity"
    );
    assert_eq!(with.len(), plain.len() + 6);
    // Guest identity needs the vTPM it provisions.
    let no_tpm: Config = serde_json::from_value(
        serde_json::json!({"kernel":"/bin/true","qemu_binary":"/bin/true","guest_identity":{"enabled":true}}),
    )
    .unwrap();
    assert!(no_tpm
        .validate()
        .unwrap_err()
        .to_string()
        .contains("needs a tpm"));
}

//! Explicit kernel route fixture. Run only in a private user/network namespace.
use super::*;

fn isolated_user_namespace() {
    let mapping = std::fs::read_to_string("/proc/self/uid_map").unwrap();
    let fields: Vec<_> = mapping.split_whitespace().collect();
    assert!(
        fields.len() == 3 && fields[0] == "0" && fields[2] == "1",
        "run this test under unshare --user --map-root-user --net"
    );
    // Capabilities in this one-user namespace cannot modify the initial host
    // network namespace. Missing --net therefore fails before route changes.
}

fn fixture_agent(root: &Path) -> Agent {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client =
        Client::try_from(kube::Config::new("http://127.0.0.1:9".parse().unwrap())).unwrap();
    let (store, _writer) = reflector::store();
    Agent {
        proc_root: root.to_owned(),
        store,
        api: Api::namespaced(client, "qrab"),
        namespace: "qrab".into(),
        plumbed: Mutex::default(),
        reported: Mutex::default(),
        granting: Mutex::default(),
        cilium: PathBuf::new(),
        dhcp: Mutex::default(),
        orphaned: Mutex::default(),
        runtime: tokio::runtime::Handle::current(),
        peers_busy: Default::default(),
        peer_tls: Mutex::new(None),
        peer_strangers: Default::default(),
        forwarded: Mutex::default(),
    }
}

#[tokio::test]
async fn forwarding_change_resets_age_and_holds_the_sweep_lock_through_the_effect() {
    let agent = Arc::new(fixture_agent(Path::new("/unused")));
    let ip = "10.251.7.9".parse().unwrap();
    agent.orphaned.lock().unwrap().insert(ip, Instant::now());
    let (entered, started) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel();
    let worker = agent.clone();
    let task = std::thread::spawn(move || {
        worker
            .forwarding_change(ip, || {
                entered.send(()).unwrap();
                released.recv().unwrap();
                Ok(())
            })
            .unwrap();
    });
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    let serialized = agent.orphaned.try_lock().is_err();
    release.send(()).unwrap();
    task.join().unwrap();
    assert!(serialized, "sweep could run during the forwarding effect");
    assert!(!agent.orphaned.lock().unwrap().contains_key(&ip));
}

#[tokio::test]
#[ignore = "requires private user/network namespace and iproute2; use unshare --user --map-root-user --net"]
async fn fresh_forward_does_not_inherit_an_expired_orphan_grace() {
    isolated_user_namespace();
    for args in [
        vec!["link", "add", "qfwd0", "type", "dummy"],
        vec!["addr", "add", "10.252.0.1/24", "dev", "qfwd0"],
        vec!["link", "set", "qfwd0", "up"],
    ] {
        sys::ip(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).unwrap();
    }
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("1/ns")).unwrap();
    std::os::unix::fs::symlink("/proc/self/ns/net", root.path().join("1/ns/net")).unwrap();
    let agent = fixture_agent(root.path());
    let guest: std::net::Ipv4Addr = "10.251.7.9".parse().unwrap();
    let via = "10.252.0.2".parse().unwrap();
    let anchor: Pod = serde_json::from_value(serde_json::json!({
        "apiVersion":"v1","kind":"Pod",
        "metadata":{"name":"fixture-anchor","namespace":"qrab","uid":"anchor-snapshot"},
        "status":{"podIP":guest.to_string()}
    }))
    .unwrap();
    // A route left by an earlier handoff was already aged as orphaned.
    sys::forward(guest, via).unwrap();
    agent.orphaned.lock().unwrap().insert(
        guest,
        Instant::now() - FORWARD_GRACE - Duration::from_secs(1),
    );
    let request = wire::Request {
        nic: "lan".into(),
        queues: 0,
        op: wire::Op::Forward,
        via: Some(via),
    };
    // This is the snapshot already admitted by grant(). The anchor can leave
    // before a sweep observes it, such as a fast return and another handoff.
    agent.control(&anchor, &request).unwrap();
    assert!(sys::forwards().unwrap().contains(&guest));
    agent.sweep_forwards();
    assert!(
        sys::forwards().unwrap().contains(&guest),
        "a fresh handoff route was removed using an earlier route's age"
    );
    // It still expires after its own grace when no local anchor holds it.
    agent.orphaned.lock().unwrap().insert(
        guest,
        Instant::now() - FORWARD_GRACE - Duration::from_secs(1),
    );
    agent.sweep_forwards();
    assert!(!sys::forwards().unwrap().contains(&guest));
}

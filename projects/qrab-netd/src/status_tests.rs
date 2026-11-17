//! A private HTTP Kubernetes stand-in, including conditional merge semantics.
//! Never contacts a cluster, changes a namespace or starts a real DHCP worker.
use super::*;
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::atomic::{AtomicBool, Ordering},
};

const OLD: &str = "6b9f0c44-0000-4000-8000-000000000001";
const NEW: &str = "6b9f0c44-0000-4000-8000-000000000002";

fn pod(uid: &str, version: &str) -> Pod {
    serde_json::from_value(json!({
        "apiVersion":"v1", "kind":"Pod",
        "metadata":{"name":"reused", "namespace":"qrab", "uid":uid,
            "resourceVersion":version,
            "labels":{authz::NAME_LABEL:"qrab", authz::COMPONENT_LABEL:authz::NIC_COMPONENT,
                authz::OWNER_UID:"vm-uid", authz::NIC_LABEL:"lan"},
            "annotations":{qrab_netd::DATAPATH_ANNOTATION:qrab_netd::DATAPATH_TAP}},
        "spec":{"containers":[{"name":qrab_netd::ANCHOR_CONTAINER,"image":"fixture"}]},
        "status":{"podIP":"10.99.0.7", "conditions":[{"type":"ContainersReady", "status":"True"}],
            "containerStatuses":[{"name":qrab_netd::ANCHOR_CONTAINER,"image":"fixture","imageID":"fixture","ready":false,"restartCount":0,
                "containerID":format!("containerd://{}", "1".repeat(64)),"state":{"running":{}}}]}
    }))
    .unwrap()
}

struct State {
    object: Value,
    requests: Vec<Value>,
}

struct Server {
    api: Api<Pod>,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Server {
    fn new(current: Pod) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let config = kube::Config::new(format!("http://{address}").parse().unwrap());
        let api = Api::namespaced(Client::try_from(config).unwrap(), "qrab");
        let state = Arc::new(Mutex::new(State {
            object: serde_json::to_value(current).unwrap(),
            requests: Vec::new(),
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let (held, running) = (state.clone(), stop.clone());
        let worker = std::thread::spawn(move || {
            while !running.load(Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("fake API accept: {e}"),
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = Vec::new();
                let (offset, length) = loop {
                    let mut chunk = [0u8; 1024];
                    let n = stream.read(&mut chunk).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&chunk[..n]);
                    assert!(bytes.len() < 32 * 1024);
                    if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                        let header = std::str::from_utf8(&bytes[..end]).unwrap();
                        assert!(
                            header.starts_with("PATCH /api/v1/namespaces/qrab/pods/reused/status")
                        );
                        assert!(header.contains("application/strategic-merge-patch+json"));
                        let length = header
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap();
                        break (end + 4, length);
                    }
                };
                while bytes.len() < offset + length {
                    let mut chunk = [0u8; 1024];
                    let n = stream.read(&mut chunk).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&chunk[..n]);
                }
                let patch: Value = serde_json::from_slice(&bytes[offset..offset + length]).unwrap();
                let mut state = held.lock().unwrap();
                state.requests.push(patch.clone());
                // An unconditional patch is allowed, as by Kubernetes. A
                // provided stale UID or version rejects the whole write.
                let conflict = ["uid", "resourceVersion"].iter().any(|key| {
                    patch["metadata"]
                        .get(*key)
                        .is_some_and(|v| v != &state.object["metadata"][key])
                });
                let (code, response) = if conflict {
                    (
                        "409 Conflict",
                        json!({"apiVersion":"v1", "kind":"Status", "status":"Failure",
                        "reason":"Conflict", "code":409, "message":"observed Pod no longer matches"}),
                    )
                } else {
                    for condition in patch["status"]["conditions"].as_array().unwrap() {
                        let conditions =
                            state.object["status"]["conditions"].as_array_mut().unwrap();
                        if let Some(old) = conditions
                            .iter_mut()
                            .find(|c| c["type"] == condition["type"])
                        {
                            *old = condition.clone();
                        } else {
                            conditions.push(condition.clone());
                        }
                    }
                    let version = state.object["metadata"]["resourceVersion"]
                        .as_str()
                        .unwrap()
                        .parse::<u64>()
                        .unwrap()
                        + 1;
                    state.object["metadata"]["resourceVersion"] = version.to_string().into();
                    ("200 OK", state.object.clone())
                };
                drop(state);
                let body = serde_json::to_vec(&response).unwrap();
                write!(stream, "HTTP/1.1 {code}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        Self {
            api,
            state,
            stop,
            worker: Some(worker),
        }
    }

    fn condition(&self, kind: &str) -> Option<Value> {
        self.state.lock().unwrap().object["status"]["conditions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["type"] == kind)
            .cloned()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.worker.take().unwrap().join().unwrap();
    }
}

fn agent(api: Api<Pod>, store: reflector::Store<Pod>) -> Arc<Agent> {
    Arc::new(Agent {
        proc_root: PathBuf::new(),
        store,
        api,
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
    })
}

fn fake_plumbed(agent: &Agent, summary: &str) -> (tempfile::TempDir, Arc<AtomicBool>) {
    let root = tempfile::tempdir().unwrap();
    let process = root.path().join("7");
    std::fs::create_dir_all(process.join("ns")).unwrap();
    std::fs::write(
        process.join("cgroup"),
        format!("0::/kubepods/pod{OLD}/{}\n", "1".repeat(64)),
    )
    .unwrap();
    std::fs::write(process.join("ns/net"), "held namespace fixture").unwrap();
    let anchor = Arc::new(Anchor::find(root.path(), &pod(OLD, "11")).unwrap());
    let responder = DhcpWorker::new(
        &anchor,
        qrab_netd::dhcp::Lease {
            mac: [2, 0, 0, 0, 0, 1],
            ip: "10.99.0.7".parse().unwrap(),
            seconds: 600,
            dns: Some(qrab_netd::GUEST_GATEWAY),
            mtu: None,
        },
    );
    let stop = responder.stop_flag();
    agent.dhcp.lock().unwrap().insert(OLD.into(), responder);
    agent.plumbed.lock().unwrap().insert(
        OLD.into(),
        Plumbed {
            anchor,
            verified_at: Instant::now(),
            summary: summary.into(),
            vpn: None,
        },
    );
    (root, stop)
}

#[tokio::test]
async fn delayed_readiness_cannot_mark_a_same_name_replacement_ready() {
    let server = Server::new(pod(NEW, "20"));
    let (store, _writer) = reflector::store();
    let agent = agent(server.api.clone(), store);
    report(
        &server.api,
        &agent,
        &pod(OLD, "10"),
        &Ok("old namespace plumbed".into()),
    )
    .await;
    assert!(server.condition(PLUMBED_CONDITION).is_none());
    assert!(agent.reported.lock().unwrap().is_empty());
    assert_eq!(
        server.condition("ContainersReady").unwrap()["status"],
        "True"
    );
}

#[tokio::test]
async fn delayed_vpn_port_cannot_be_published_on_a_replacement_pod() {
    let server = Server::new(pod(NEW, "20"));
    let (store, _writer) = reflector::store();
    let agent = agent(server.api.clone(), store);
    let (_root, _stop) = fake_plumbed(&agent, "applied plan");
    let ready = agent.plumbed.lock().unwrap().get(OLD).unwrap().clone();
    let worker = agent.clone();
    tokio::task::spawn_blocking(move || {
        worker.finish_port_round(&pod(OLD, "10"), &ready, &Ok(45000))
    })
    .await
    .unwrap();
    assert!(server.condition(PORT_FORWARD_CONDITION).is_none());
    assert!(!agent.forwarded.lock().unwrap().contains_key(OLD));
}

#[tokio::test]
async fn a_status_conflict_is_retried_without_replumbing_the_namespace() {
    let server = Server::new(pod(OLD, "11"));
    let (store, mut writer) = reflector::store();
    let agent = agent(server.api.clone(), store);
    let summary = "tap prepared";
    report(&server.api, &agent, &pod(OLD, "10"), &Ok(summary.into())).await;
    assert!(server.condition(PLUMBED_CONDITION).is_none());
    assert!(agent.reported.lock().unwrap().is_empty());

    let (_root, _stop) = fake_plumbed(&agent, summary);
    writer.apply_watcher_event(&watcher::Event::Apply(pod(OLD, "11")));
    // The successful namespace is fresh. Reconciliation must retry only its
    // rejected status write, using the updated watch snapshot. Entering the
    // fake namespace or reapplying a network plan would fail this test.
    reconcile(&agent, &server.api).await;
    assert_eq!(
        server.condition(PLUMBED_CONDITION).unwrap()["status"],
        "True"
    );
    assert_eq!(
        server.condition("ContainersReady").unwrap()["status"],
        "True"
    );
    assert_eq!(server.state.lock().unwrap().requests.len(), 2);
    assert_eq!(
        agent.reported.lock().unwrap().get(OLD),
        Some(&(true, summary.into()))
    );
    reconcile(&agent, &server.api).await;
    assert_eq!(
        server.state.lock().unwrap().requests.len(),
        2,
        "successful outcomes remain cached"
    );
}

#[tokio::test]
async fn a_status_write_requires_both_uid_and_resource_version() {
    let server = Server::new(pod(NEW, "20"));
    for field in ["uid", "resourceVersion"] {
        let mut missing = serde_json::to_value(pod(NEW, "20")).unwrap();
        missing["metadata"].as_object_mut().unwrap().remove(field);
        let missing: Pod = serde_json::from_value(missing).unwrap();
        assert!(write_condition(
            &server.api,
            &missing,
            json!({"type":PLUMBED_CONDITION,"status":"True"})
        )
        .await
        .is_err());
    }
    assert!(server.state.lock().unwrap().requests.is_empty());
}

#[tokio::test]
async fn losing_a_process_reference_invalidates_grants_without_stopping_a_reusable_socket() {
    let server = Server::new(pod(OLD, "11"));
    let (store, _writer) = reflector::store();
    let agent = agent(server.api.clone(), store);
    let (_root, stop) = fake_plumbed(&agent, "tap prepared");
    assert!(agent.ready_anchor(&pod(OLD, "11")).is_ok());
    agent.invalidate_ready(OLD);
    assert!(agent.ready_anchor(&pod(OLD, "11")).is_err());
    assert!(!stop.load(Ordering::Relaxed));
    assert!(agent.dhcp.lock().unwrap().contains_key(OLD));
}

#[tokio::test]
async fn an_exited_responder_cannot_keep_a_recent_context_ready() {
    let server = Server::new(pod(OLD, "11"));
    let (store, mut writer) = reflector::store();
    let agent = agent(server.api.clone(), store);
    let (_root, stop) = fake_plumbed(&agent, "tap prepared");
    stop.store(true, Ordering::Relaxed);
    assert!(agent.ready_anchor(&pod(OLD, "11")).is_err());
    writer.apply_watcher_event(&watcher::Event::Apply(pod(OLD, "11")));
    // The old timestamp is fresh, but the stopped worker is not. A retry
    // must run; the deliberately absent real proc root prevents plumbing.
    reconcile(&agent, &server.api).await;
    assert_eq!(
        server.condition(PLUMBED_CONDITION).unwrap()["status"],
        "False"
    );
}

#[tokio::test]
async fn namespace_discovery_uses_the_current_container_instead_of_an_older_live_pid() {
    use std::os::unix::fs::MetadataExt;
    let old_id = "1".repeat(64);
    let current_id = "2".repeat(64);
    let root = tempfile::tempdir().unwrap();
    for (pid, id) in [(7, &old_id), (9, &current_id)] {
        let dir = root.path().join(pid.to_string());
        std::fs::create_dir_all(dir.join("ns")).unwrap();
        std::fs::write(
            dir.join("cgroup"),
            format!(
                "0::/kubepods.slice/kubepods-pod{}.slice/cri-containerd-{id}.scope\n",
                OLD.replace('-', "_")
            ),
        )
        .unwrap();
        std::fs::write(dir.join("ns/net"), id).unwrap();
    }
    let mut current = serde_json::to_value(pod(OLD, "11")).unwrap();
    current["spec"] = json!({"containers":[{"name":"anchor","image":"fixture"}]});
    current["status"]["containerStatuses"] = json!([{
        "name":"anchor", "image":"fixture", "imageID":"fixture", "restartCount":1,
        "ready":false, "containerID":format!("containerd://{current_id}"), "state":{"running":{}}
    }]);
    let current: Pod = serde_json::from_value(current).unwrap();
    let server = Server::new(current.clone());
    let (store, _writer) = reflector::store();
    let mut agent = agent(server.api.clone(), store);
    Arc::get_mut(&mut agent).unwrap().proc_root = root.path().to_owned();
    let selected = agent.anchor_for(&current).unwrap();
    assert_eq!(selected.process().pid(), 9);
    assert_eq!(
        selected.namespace().unwrap().metadata().unwrap().ino(),
        std::fs::metadata(root.path().join("9/ns/net"))
            .unwrap()
            .ino()
    );
}

#[tokio::test]
async fn a_late_port_round_cannot_publish_for_a_replaced_namespace() {
    let server = Server::new(pod(OLD, "11"));
    let (store, _writer) = reflector::store();
    let agent = agent(server.api.clone(), store);
    let (root, _stop) = fake_plumbed(&agent, "old plan");
    let old = agent.plumbed.lock().unwrap().get(OLD).unwrap().clone();
    let link = root.path().join("7/ns/net");
    std::fs::rename(&link, root.path().join("7/ns/old-net")).unwrap();
    std::fs::write(&link, "new namespace").unwrap();
    let mut replacement = old.clone();
    replacement.anchor = Arc::new(Anchor::find(root.path(), &pod(OLD, "11")).unwrap());
    agent
        .plumbed
        .lock()
        .unwrap()
        .insert(OLD.into(), replacement);
    let worker = agent.clone();
    let accepted = tokio::task::spawn_blocking(move || {
        worker.finish_port_round(&pod(OLD, "11"), &old, &Ok(45000))
    })
    .await
    .unwrap();
    assert!(!accepted);
    assert!(agent.forwarded.lock().unwrap().is_empty());
    assert!(server.condition(PORT_FORWARD_CONDITION).is_none());
}

#[tokio::test]
async fn applied_config_generations_preserve_healthy_rounds_and_discard_old_completions() {
    let server = Server::new(pod(OLD, "11"));
    let (store, _writer) = reflector::store();
    let agent = agent(server.api.clone(), store);
    let (_root, _stop) = fake_plumbed(&agent, "plan A");
    let mut a = agent.plumbed.lock().unwrap().get(OLD).unwrap().clone();
    a.vpn = Some(AppliedVpn {
        digest: [1; 32],
        gateway: Some(qrab_netd::GUEST_GATEWAY),
    });
    agent.remember_applied(OLD.into(), a.clone());
    let (worker, original) = (agent.clone(), a.clone());
    assert!(
        tokio::task::spawn_blocking(move || worker.finish_port_round(
            &pod(OLD, "11"),
            &original,
            &Ok(45000)
        ))
        .await
        .unwrap()
    );
    let before = agent.forwarded.lock().unwrap().get(OLD).unwrap().at;
    let mut verified_again = a.clone();
    verified_again.verified_at = Instant::now();
    agent.remember_applied(OLD.into(), verified_again);
    assert_eq!(agent.forwarded.lock().unwrap().get(OLD).unwrap().at, before);
    let mut b = a.clone();
    b.vpn.as_mut().unwrap().digest = [2; 32];
    agent.remember_applied(OLD.into(), b);
    assert!(agent.forwarded.lock().unwrap().is_empty());
    let worker = agent.clone();
    assert!(
        !tokio::task::spawn_blocking(move || worker.finish_port_round(
            &pod(OLD, "12"),
            &a,
            &Ok(45001)
        ))
        .await
        .unwrap()
    );
    assert!(agent.forwarded.lock().unwrap().is_empty());
    assert_eq!(
        server.state.lock().unwrap().requests.len(),
        1,
        "late results must not publish status"
    );
}

#[tokio::test]
async fn disabling_applied_forwarding_reports_unmapped_even_after_cache_reset() {
    let mut target = pod(OLD, "11");
    target.metadata.annotations.as_mut().unwrap().insert(
        qrab_netd::vpn::VPN_ANNOTATION.into(),
        qrab_netd::vpn::VPN_WIREGUARD.into(),
    );
    let server = Server::new(target.clone());
    server.state.lock().unwrap().object["status"]["conditions"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type":PORT_FORWARD_CONDITION,"status":"True","message":"45000"}));
    let (store, mut writer) = reflector::store();
    let agent = agent(server.api.clone(), store);
    let (_root, _stop) = fake_plumbed(&agent, "forwarding disabled");
    let mut ready = agent.plumbed.lock().unwrap().get(OLD).unwrap().clone();
    ready.vpn = Some(AppliedVpn {
        digest: [2; 32],
        gateway: None,
    });
    agent.remember_applied(OLD.into(), ready);
    writer.apply_watcher_event(&watcher::Event::Apply(target));
    let worker = agent.clone();
    tokio::task::spawn_blocking(move || worker.renew_port_forwards())
        .await
        .unwrap();
    assert_eq!(
        server.condition(PORT_FORWARD_CONDITION).unwrap()["status"],
        "False"
    );
    assert!(agent
        .forwarded
        .lock()
        .unwrap()
        .get(OLD)
        .unwrap()
        .result
        .is_err());
}

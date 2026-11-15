use anyhow::{Context, Result};
use futures::StreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{Api, Patch, PatchParams},
    runtime::{reflector, watcher, WatchStreamExt},
    Client, ResourceExt,
};
use qrab_netd::{authz, plan::NicPlan, sys, wire, PLUMBED_CONDITION, SOCKET_DIR, SOCKET_NAME};
use std::{
    collections::HashMap,
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// A plumbed anchor is re-verified this often; an unplumbed one every pass.
const REVERIFY: Duration = Duration::from_secs(60);

fn log(event: &str, detail: serde_json::Value) {
    eprintln!("{}", serde_json::json!({"event": event, "detail": detail}));
}

struct Agent {
    proc_root: PathBuf,
    store: reflector::Store<Pod>,
    /// Anchor UID → when its plan last applied cleanly.
    plumbed: Mutex<HashMap<String, Instant>>,
    /// Anchor UID → the condition last written (status, message).
    reported: Mutex<HashMap<String, (bool, String)>>,
    /// One grant at a time, so the busy check and the attach cannot race.
    granting: Mutex<()>,
}

impl Agent {
    fn netns(&self, pod: &Pod) -> Result<(i32, PathBuf), String> {
        let uid = pod.uid().ok_or("Pod has no UID")?;
        let pid =
            sys::pid_of_pod(&self.proc_root, &uid).ok_or("no running process of the anchor yet")?;
        Ok((pid, sys::netns_of(&self.proc_root, pid)))
    }

    fn plumb(&self, pod: &Pod) -> Result<String, String> {
        let plan = NicPlan::for_anchor(pod)?;
        let (pid, ns) = self.netns(pod)?;
        let resolver = sys::pod_resolver(&self.proc_root, pid);
        let summary = format!("tap {} routes {}", plan.tap, plan.guest_ip);
        sys::in_netns(&ns, move || sys::apply(&plan, resolver)).map_err(|e| e.to_string())?;
        Ok(summary)
    }

    fn grant(&self, stream: &UnixStream) -> Result<String, String> {
        let pid = nix::sys::socket::getsockopt(stream, nix::sys::socket::sockopt::PeerCredentials)
            .map_err(|e| e.to_string())?
            .pid();
        let request = wire::read_request(stream).map_err(|e| e.to_string())?;
        let pods = self.store.state();
        let requester = sys::pod_uid_of(&self.proc_root, pid).and_then(|uid| {
            pods.iter()
                .find(|p| p.uid().as_deref() == Some(uid.as_str()))
        });
        let anchor = authz::select_anchor(
            requester.map(|p| p.as_ref()),
            &request.nic,
            pods.iter().map(|p| p.as_ref()),
        )
        .map_err(|r| r.to_string())?;
        let anchor_uid = anchor.uid().unwrap_or_default();
        if !self.plumbed.lock().unwrap().contains_key(&anchor_uid) {
            return Err(format!("NIC {:?} is not plumbed yet", request.nic));
        }
        let plan = NicPlan::for_anchor(anchor)?;
        let _one = self.granting.lock().unwrap();
        let holders = sys::tap_holders(&self.proc_root, &plan.tap);
        if !holders.is_empty() {
            return Err(format!(
                "tap {} is already held by pid {holders:?}",
                plan.tap
            ));
        }
        let (_, ns) = self.netns(anchor)?;
        let (tap, queues) = (plan.tap.clone(), request.queues);
        let taps = sys::in_netns(&ns, move || sys::open_tap_queues(&tap, queues))
            .map_err(|e| e.to_string())?;
        let vhosts = sys::open_vhost(queues).map_err(|e| e.to_string())?;
        wire::send_grant(stream, &plan.tap, &taps, &vhosts).map_err(|e| e.to_string())?;
        Ok(format!(
            "pid {pid} received {queues} queue(s) of {} for NIC {:?}",
            plan.tap, request.nic
        ))
    }
}

async fn report(pods: &Api<Pod>, agent: &Agent, pod: &Pod, outcome: &Result<String, String>) {
    let (Some(uid), name) = (pod.uid(), pod.name_any()) else {
        return;
    };
    let (ok, message) = match outcome {
        Ok(summary) => (true, summary.clone()),
        Err(error) => (false, error.clone()),
    };
    if agent.reported.lock().unwrap().get(&uid) == Some(&(ok, message.clone())) {
        return;
    }
    let condition = serde_json::json!({
        "type": PLUMBED_CONDITION,
        "status": if ok { "True" } else { "False" },
        "reason": if ok { "Plumbed" } else { "NotPlumbed" },
        "message": message,
        "lastTransitionTime": k8s_openapi::apimachinery::pkg::apis::meta::v1::Time(k8s_openapi::jiff::Timestamp::now()),
    });
    let patch = Patch::Strategic(serde_json::json!({"status": {"conditions": [condition]}}));
    match pods
        .patch_status(&name, &PatchParams::default(), &patch)
        .await
    {
        Ok(_) => {
            log(
                "reported",
                serde_json::json!({"pod": name, "plumbed": ok, "message": message}),
            );
            agent.reported.lock().unwrap().insert(uid, (ok, message));
        }
        Err(error) => log(
            "report_failed",
            serde_json::json!({"pod": name, "error": error.to_string()}),
        ),
    }
}

async fn reconcile(agent: &Arc<Agent>, pods: &Api<Pod>) {
    let anchors: Vec<_> = agent
        .store
        .state()
        .into_iter()
        .filter(|p| authz::is_tap_anchor(p))
        .collect();
    for pod in &anchors {
        let uid = pod.uid().unwrap_or_default();
        let fresh = agent
            .plumbed
            .lock()
            .unwrap()
            .get(&uid)
            .is_some_and(|at| at.elapsed() < REVERIFY);
        if fresh {
            continue;
        }
        let worker = agent.clone();
        let target = pod.clone();
        let outcome = tokio::task::spawn_blocking(move || worker.plumb(&target))
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
        match &outcome {
            Ok(_) => {
                agent.plumbed.lock().unwrap().insert(uid, Instant::now());
            }
            Err(error) => {
                agent.plumbed.lock().unwrap().remove(&uid);
                log(
                    "plumb_failed",
                    serde_json::json!({"pod": pod.name_any(), "error": error}),
                );
            }
        }
        report(pods, agent, pod, &outcome).await;
    }
    let live: Vec<String> = anchors.iter().filter_map(|p| p.uid()).collect();
    agent
        .plumbed
        .lock()
        .unwrap()
        .retain(|uid, _| live.contains(uid));
    agent
        .reported
        .lock()
        .unwrap()
        .retain(|uid, _| live.contains(uid));
}

async fn serve(agent: Arc<Agent>, socket: PathBuf) -> Result<()> {
    std::fs::create_dir_all(socket.parent().context("socket path has no directory")?)?;
    let _ = std::fs::remove_file(&socket);
    let listener = tokio::net::UnixListener::bind(&socket)?;
    // VM managers run unprivileged; access control is the peer check.
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o666))?;
    log("listening", serde_json::json!(socket));
    loop {
        let (stream, _) = listener.accept().await?;
        let agent = agent.clone();
        tokio::task::spawn_blocking(move || {
            let stream = match stream
                .into_std()
                .and_then(|s| s.set_nonblocking(false).map(|()| s))
            {
                Ok(stream) => stream,
                Err(error) => return log("accept_failed", serde_json::json!(error.to_string())),
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            match agent.grant(&stream) {
                Ok(summary) => log("granted", serde_json::json!(summary)),
                Err(error) => {
                    log("refused", serde_json::json!(error));
                    let _ = wire::send_refusal(&stream, &error);
                }
            }
        });
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        [] => {}
        ["request", nic] | ["request", nic, _] => {
            // Diagnostic client: what a VM's manager would receive.
            let queues = args.get(2).map_or(Ok(1), |q| q.parse()).context("queues must be 1..=8")?;
            let grant = wire::request(&Path::new(SOCKET_DIR).join(SOCKET_NAME), nic, queues)?;
            println!(
                "{}",
                serde_json::json!({"tap": grant.tap, "taps": grant.taps.len(), "vhosts": grant.vhosts.len()})
            );
            return Ok(());
        }
        _ => anyhow::bail!(
            "usage: qrab-netd (configure with NODE_NAME, QRAB_NAMESPACE, QRAB_NETD_PROC)\n       qrab-netd request NIC [QUEUES]"
        ),
    }
    let node =
        std::env::var("NODE_NAME").context("NODE_NAME (the downward-API node name) is required")?;
    let namespace = std::env::var("QRAB_NAMESPACE").unwrap_or_else(|_| "qrab".into());
    let proc_root =
        PathBuf::from(std::env::var("QRAB_NETD_PROC").unwrap_or_else(|_| "/proc".into()));
    if let Err(error) = sys::check_devices() {
        log("devices_unavailable", serde_json::json!(error));
        anyhow::bail!(error);
    }
    let pods: Api<Pod> = Api::namespaced(Client::try_default().await?, &namespace);
    let (store, writer) = reflector::store();
    let config = watcher::Config::default()
        .fields(&format!("spec.nodeName={node}"))
        .labels("app.kubernetes.io/name=qrab");
    let changed = Arc::new(tokio::sync::Notify::new());
    let events = reflector(writer, watcher(pods.clone(), config))
        .default_backoff()
        .touched_objects();
    let notify = changed.clone();
    tokio::spawn(events.for_each(move |event| {
        if let Err(error) = event {
            log("watch_error", serde_json::json!(error.to_string()));
        }
        notify.notify_one();
        std::future::ready(())
    }));
    store
        .wait_until_ready()
        .await
        .context("pod watch never synced")?;
    log(
        "started",
        serde_json::json!({"node": node, "namespace": namespace}),
    );
    let agent = Arc::new(Agent {
        proc_root,
        store,
        plumbed: Mutex::default(),
        reported: Mutex::default(),
        granting: Mutex::default(),
    });
    let mut server = tokio::spawn(serve(
        agent.clone(),
        Path::new(SOCKET_DIR).join(SOCKET_NAME),
    ));
    let mut tick = tokio::time::interval(Duration::from_secs(10));
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    loop {
        tokio::select! {
            _ = changed.notified() => {}
            _ = tick.tick() => {}
            _ = term.recv() => return Ok(()),
            result = &mut server => {
                return result?.context("socket server stopped");
            }
        }
        reconcile(&agent, &pods).await;
    }
}

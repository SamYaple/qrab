//! A Pod's API identity and its currently plumbed network namespace.
//!
//! A sandbox can be replaced without replacing its Pod. Keep the namespace
//! descriptor open, so its identity cannot be recycled while queues or a
//! readiness entry still refer to it. The wire token is deliberately opaque.
use crate::{process::Process, sys};
use k8s_openapi::api::core::v1::Pod;
use std::{
    fs::File,
    io,
    os::unix::fs::MetadataExt,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
};

#[derive(Clone, Copy, PartialEq, Eq)]
struct NamespaceId {
    device: u64,
    inode: u64,
}

impl NamespaceId {
    fn of(namespace: &File) -> io::Result<Self> {
        let metadata = namespace.metadata()?;
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
}

/// A verified process and an already-open namespace for one anchor Pod.
pub struct Anchor {
    uid: String,
    container: String,
    process: Process,
    namespace: File,
    identity: NamespaceId,
}

impl Anchor {
    pub fn find(proc_root: &Path, pod: &Pod) -> io::Result<Self> {
        let (uid, container) = expected_container(pod)?;
        for process in sys::processes_of_pod(proc_root, uid) {
            if let Ok(anchor) = Self::from_process(uid, container, process) {
                return Ok(anchor);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no process of the current running anchor container yet",
        ))
    }

    fn from_process(uid: &str, container: &str, process: Process) -> io::Result<Self> {
        let namespace = process.network_namespace()?;
        let identity = NamespaceId::of(&namespace)?;
        let anchor = Self {
            uid: uid.into(),
            container: container.into(),
            process,
            namespace,
            identity,
        };
        anchor.ensure_current()?;
        Ok(anchor)
    }

    pub fn process(&self) -> &Process {
        &self.process
    }

    /// A duplicate of the exact descriptor used when the plan was applied.
    pub fn namespace(&self) -> io::Result<File> {
        self.namespace.try_clone()
    }

    /// Stable across agent restarts, different for a replacement sandbox.
    pub fn token(&self) -> String {
        format!(
            "{}@{:x}:{:x}",
            self.uid, self.identity.device, self.identity.inode
        )
    }

    /// Check the held process, never a later lookup of its numeric PID.
    pub fn ensure_current(&self) -> io::Result<()> {
        let (uid, container) = self.process.pod_container()?;
        if uid != self.uid
            || container != self.container
            || NamespaceId::of(&self.process.network_namespace()?)? != self.identity
        {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "anchor network namespace changed; plumbing is pending",
            ));
        }
        Ok(())
    }

    /// A still-live old process is insufficient if Kubernetes now identifies
    /// another running container. Readiness itself is deliberately not required.
    pub fn ensure_for(&self, pod: &Pod) -> io::Result<()> {
        let (uid, container) = expected_container(pod)?;
        if uid != self.uid || container != self.container {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "anchor container incarnation changed",
            ));
        }
        self.ensure_current()
    }
}

fn expected_container(pod: &Pod) -> io::Result<(&str, &str)> {
    let missing = || {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Pod has no current running anchor container",
        )
    };
    let uid = pod
        .metadata
        .uid
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(missing)?;
    if !pod.spec.as_ref().is_some_and(|spec| {
        spec.containers
            .iter()
            .any(|c| c.name == crate::ANCHOR_CONTAINER)
    }) {
        return Err(missing());
    }
    let mut statuses = pod
        .status
        .as_ref()
        .and_then(|s| s.container_statuses.as_ref())
        .into_iter()
        .flatten()
        .filter(|s| s.name == crate::ANCHOR_CONTAINER);
    let status = statuses.next().ok_or_else(missing)?;
    if statuses.next().is_some()
        || !status
            .state
            .as_ref()
            .is_some_and(|s| s.running.is_some() && s.waiting.is_none() && s.terminated.is_none())
    {
        return Err(missing());
    }
    let (runtime, id) = status
        .container_id
        .as_deref()
        .and_then(|s| s.split_once("://"))
        .ok_or_else(missing)?;
    if !matches!(runtime, "containerd" | "cri-o" | "docker")
        || !crate::cgroup::valid_container_id(id)
    {
        return Err(missing());
    }
    Ok((uid, id))
}

/// A namespace-bound DHCP responder. Removing its cache entry stops it,
/// including when a Pod UID remains present but its sandbox has changed.
pub struct DhcpWorker {
    token: String,
    stop: Arc<AtomicBool>,
    lease: Arc<RwLock<crate::dhcp::Lease>>,
}

impl DhcpWorker {
    pub fn new(anchor: &Anchor, lease: crate::dhcp::Lease) -> Self {
        Self {
            token: anchor.token(),
            stop: Arc::new(AtomicBool::new(false)),
            lease: Arc::new(RwLock::new(lease)),
        }
    }

    pub fn serves(&self, anchor: &Anchor) -> bool {
        self.token == anchor.token() && !self.stop.load(Ordering::Relaxed)
    }

    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        self.stop.clone()
    }

    pub fn lease(&self) -> Arc<RwLock<crate::dhcp::Lease>> {
        self.lease.clone()
    }

    /// Publish one whole lease snapshot without rebinding the same socket.
    pub fn update(&self, lease: crate::dhcp::Lease) {
        *self.lease.write().unwrap_or_else(|e| e.into_inner()) = lease;
    }
}

impl Drop for DhcpWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UID: &str = "6b9f0c44-0000-4000-8000-000000000001";
    const LEASE: crate::dhcp::Lease = crate::dhcp::Lease {
        mac: [2, 0, 0, 0, 0, 1],
        ip: std::net::Ipv4Addr::new(10, 99, 0, 7),
        seconds: 600,
        dns: Some(crate::GUEST_GATEWAY),
        mtu: None,
    };

    fn pod() -> Pod {
        serde_json::from_value(serde_json::json!({
            "metadata":{"uid":UID},
            "spec":{"containers":[{"name":crate::ANCHOR_CONTAINER,"image":"fixture"}]},
            "status":{"containerStatuses":[{"name":crate::ANCHOR_CONTAINER,"image":"fixture","imageID":"fixture",
                "ready":false,"restartCount":0,"containerID":format!("containerd://{}", "1".repeat(64)),"state":{"running":{}}}]}
        })).unwrap()
    }

    fn process(root: &Path) -> std::path::PathBuf {
        let path = root.join("7");
        std::fs::create_dir_all(path.join("ns")).unwrap();
        std::fs::write(
            path.join("cgroup"),
            format!("0::/kubepods/pod{UID}/{}\n", "1".repeat(64)),
        )
        .unwrap();
        std::fs::write(path.join("ns/net"), "first namespace").unwrap();
        path
    }

    #[test]
    fn a_same_uid_namespace_replacement_invalidates_the_old_ready_context() {
        let root = tempfile::tempdir().unwrap();
        let path = process(root.path());
        let original = Anchor::find(root.path(), &pod()).unwrap();
        let token = original.token();
        let descriptor = original.namespace().unwrap();
        let worker = DhcpWorker::new(&original, LEASE);
        let stopped = worker.stop_flag();
        assert!(worker.serves(&original));
        // A fresh observation of an unchanged namespace has the same token:
        // restarting netd must not force a running QEMU to replug every NIC.
        assert_eq!(Anchor::find(root.path(), &pod()).unwrap().token(), token);

        std::fs::rename(path.join("ns/net"), path.join("ns/old-net")).unwrap();
        std::fs::write(path.join("ns/net"), "replacement namespace").unwrap();
        assert_eq!(original.process().pod_uid().unwrap(), UID);
        assert!(original.ensure_current().is_err());
        let replacement = Anchor::find(root.path(), &pod()).unwrap();
        assert_ne!(replacement.token(), token);
        assert!(!worker.serves(&replacement));
        // Both the ready context and callers' copies retain the old resource.
        assert_eq!(
            NamespaceId::of(&descriptor).unwrap().inode,
            original.identity.inode
        );
        assert_eq!(original.token(), token);
        drop(worker);
        assert!(stopped.load(Ordering::Relaxed));
        let next = DhcpWorker::new(&replacement, LEASE);
        assert!(next.serves(&replacement));
        next.stop_flag().store(true, Ordering::Relaxed);
        assert!(
            !next.serves(&replacement),
            "failed workers must be restarted"
        );
    }

    #[test]
    fn an_exited_or_reassigned_process_cannot_keep_an_anchor_ready() {
        let root = tempfile::tempdir().unwrap();
        let path = process(root.path());
        let anchor = Anchor::find(root.path(), &pod()).unwrap();
        std::fs::write(path.join("cgroup"), "0::/system.slice/unrelated.service\n").unwrap();
        assert!(anchor.ensure_current().is_err());
        assert!(Anchor::find(root.path(), &pod()).is_err());
        std::fs::remove_file(path.join("cgroup")).unwrap();
        assert!(anchor.ensure_current().is_err());
    }
    #[test]
    fn an_old_live_container_is_not_current_after_the_api_incarnation_changes() {
        let root = tempfile::tempdir().unwrap();
        let first = process(root.path());
        let old = Anchor::find(root.path(), &pod()).unwrap();
        let next = root.path().join("9");
        std::fs::create_dir_all(next.join("ns")).unwrap();
        std::fs::write(
            next.join("cgroup"),
            format!("0::/kubepods/pod{UID}/{}\n", "2".repeat(64)),
        )
        .unwrap();
        std::fs::write(next.join("ns/net"), "new namespace").unwrap();
        let mut current = pod();
        current
            .status
            .as_mut()
            .unwrap()
            .container_statuses
            .as_mut()
            .unwrap()[0]
            .container_id = Some(format!("containerd://{}", "2".repeat(64)));
        assert!(
            old.ensure_current().is_ok(),
            "the older process is still alive"
        );
        assert!(old.ensure_for(&current).is_err());
        let replacement = Anchor::find(root.path(), &current).unwrap();
        assert_eq!(replacement.process().pid(), 9);
        assert_ne!(replacement.token(), old.token());
        // A container restart in the same namespace changes process identity
        // while preserving the attachment token and reusable DHCP socket.
        std::fs::remove_file(next.join("ns/net")).unwrap();
        std::fs::hard_link(first.join("ns/net"), next.join("ns/net")).unwrap();
        assert_eq!(
            Anchor::find(root.path(), &current).unwrap().token(),
            old.token()
        );
    }

    #[test]
    fn unresolved_or_terminated_anchor_status_never_falls_back_to_an_older_pid() {
        let root = tempfile::tempdir().unwrap();
        process(root.path());
        for change in [
            "missing",
            "waiting",
            "terminated",
            "short",
            "unknown-runtime",
            "wrong-name",
        ] {
            let mut object = serde_json::to_value(pod()).unwrap();
            match change {
                "missing" => {
                    object["status"]
                        .as_object_mut()
                        .unwrap()
                        .remove("containerStatuses");
                }
                "waiting" => {
                    object["status"]["containerStatuses"][0]["state"] =
                        serde_json::json!({"waiting":{"reason":"ImagePullBackOff"}})
                }
                "terminated" => {
                    object["status"]["containerStatuses"][0]["state"] =
                        serde_json::json!({"terminated":{"exitCode":0}})
                }
                "short" => {
                    object["status"]["containerStatuses"][0]["containerID"] =
                        "containerd://1111".into()
                }
                "unknown-runtime" => {
                    object["status"]["containerStatuses"][0]["containerID"] =
                        format!("unknown://{}", "1".repeat(64)).into()
                }
                "wrong-name" => object["status"]["containerStatuses"][0]["name"] = "sidecar".into(),
                _ => unreachable!(),
            }
            let object: Pod = serde_json::from_value(object).unwrap();
            assert!(Anchor::find(root.path(), &object).is_err(), "{change}");
        }
        assert!(
            Anchor::find(root.path(), &pod()).is_ok(),
            "Running need not be Ready"
        );
    }
}

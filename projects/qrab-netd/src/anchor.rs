//! A Pod's API identity and its currently plumbed network namespace.
//!
//! A sandbox can be replaced without replacing its Pod. Keep the namespace
//! descriptor open, so its identity cannot be recycled while queues or a
//! readiness entry still refer to it. The wire token is deliberately opaque.
use crate::{process::Process, sys};
use std::{
    fs::File,
    io,
    os::unix::fs::MetadataExt,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
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
    process: Process,
    namespace: File,
    identity: NamespaceId,
}

impl Anchor {
    pub fn find(proc_root: &Path, uid: &str) -> io::Result<Self> {
        for process in sys::processes_of_pod(proc_root, uid) {
            if let Ok(anchor) = Self::from_process(uid, process) {
                return Ok(anchor);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no running process of the anchor yet",
        ))
    }

    fn from_process(uid: &str, process: Process) -> io::Result<Self> {
        let namespace = process.network_namespace()?;
        let identity = NamespaceId::of(&namespace)?;
        let anchor = Self {
            uid: uid.into(),
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
        if self.process.pod_uid()? != self.uid
            || NamespaceId::of(&self.process.network_namespace()?)? != self.identity
        {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "anchor network namespace changed; plumbing is pending",
            ));
        }
        Ok(())
    }
}

/// A namespace-bound DHCP responder. Removing its cache entry stops it,
/// including when a Pod UID remains present but its sandbox has changed.
pub struct DhcpWorker {
    token: String,
    stop: Arc<AtomicBool>,
}

impl DhcpWorker {
    pub fn new(anchor: &Anchor) -> Self {
        Self {
            token: anchor.token(),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn serves(&self, anchor: &Anchor) -> bool {
        self.token == anchor.token() && !self.stop.load(Ordering::Relaxed)
    }

    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        self.stop.clone()
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

    fn process(root: &Path) -> std::path::PathBuf {
        let path = root.join("7");
        std::fs::create_dir_all(path.join("ns")).unwrap();
        std::fs::write(
            path.join("cgroup"),
            format!("0::/kubepods/pod{UID}/container\n"),
        )
        .unwrap();
        std::fs::write(path.join("ns/net"), "first namespace").unwrap();
        path
    }

    #[test]
    fn a_same_uid_namespace_replacement_invalidates_the_old_ready_context() {
        let root = tempfile::tempdir().unwrap();
        let path = process(root.path());
        let original = Anchor::find(root.path(), UID).unwrap();
        let token = original.token();
        let descriptor = original.namespace().unwrap();
        let worker = DhcpWorker::new(&original);
        let stopped = worker.stop_flag();
        assert!(worker.serves(&original));
        // A fresh observation of an unchanged namespace has the same token:
        // restarting netd must not force a running QEMU to replug every NIC.
        assert_eq!(Anchor::find(root.path(), UID).unwrap().token(), token);

        std::fs::rename(path.join("ns/net"), path.join("ns/old-net")).unwrap();
        std::fs::write(path.join("ns/net"), "replacement namespace").unwrap();
        assert_eq!(original.process().pod_uid().unwrap(), UID);
        assert!(original.ensure_current().is_err());
        let replacement = Anchor::find(root.path(), UID).unwrap();
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
        let next = DhcpWorker::new(&replacement);
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
        let anchor = Anchor::find(root.path(), UID).unwrap();
        std::fs::write(path.join("cgroup"), "0::/system.slice/unrelated.service\n").unwrap();
        assert!(anchor.ensure_current().is_err());
        assert!(Anchor::find(root.path(), UID).is_err());
        std::fs::remove_file(path.join("cgroup")).unwrap();
        assert!(anchor.ensure_current().is_err());
    }
}

use super::persistent::StateLease;
use std::{
    ops::{Deref, DerefMut},
    time::Duration,
};
use tokio::process::Child;

/// Keep the persistent-state lease until the actual child has been reaped,
/// including when the supervising future is canceled or its runtime is dropped.
pub(super) struct ManagedChild {
    child: Option<Child>,
    lease: Option<StateLease>,
    spawned_pid: u32,
}

impl ManagedChild {
    pub(super) fn new(child: Child, lease: Option<StateLease>) -> Self {
        let spawned_pid = child.id().expect("newly spawned child has a PID");
        Self {
            child: Some(child),
            lease,
            spawned_pid,
        }
    }

    pub(super) fn spawned_pid(&self) -> u32 {
        self.spawned_pid
    }

    fn cleanup(&mut self) -> Option<(Child, Option<StateLease>)> {
        self.child.take().map(|child| (child, self.lease.take()))
    }
}

impl Deref for ManagedChild {
    type Target = Child;
    fn deref(&self) -> &Child {
        self.child.as_ref().expect("managed child")
    }
}
impl DerefMut for ManagedChild {
    fn deref_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("managed child")
    }
}

fn reap(mut child: Child, lease: Option<StateLease>) {
    let _ = child.start_kill();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            // Unknown wait outcomes retain the lease rather than permit a
            // potentially concurrent writer. Process restart releases it.
            Ok(None) | Err(_) => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    drop(lease);
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        let Some((mut child, lease)) = self.cleanup() else {
            return;
        };
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        let _ = child.start_kill();
        // Only exceptional Drop needs a thread. A Tokio cleanup task could be
        // canceled by runtime shutdown before wait/reap and release the flock.
        std::thread::spawn(move || reap(child, lease));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pod::{persistent::PersistentState, Config};
    use std::{path::PathBuf, process::Stdio};

    #[tokio::test]
    async fn cleanup_owns_state_until_child_reaping_and_survives_owner_drop() {
        let directory = tempfile::tempdir().unwrap();
        let config:Config=serde_json::from_value(serde_json::json!({"name":"lease-test","kernel":"/unused","tpm":{},"state_dir":directory.path()})).unwrap();
        let state = PersistentState::prepare(&config).unwrap().unwrap();
        let child = tokio::process::Command::new("/bin/sleep")
            .arg("1000")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let pid = child.id().unwrap();
        let mut managed = ManagedChild::new(child, Some(state.lease()));
        let (child, lease) = managed.cleanup().unwrap();
        drop(managed);
        drop(state);
        assert!(
            PersistentState::prepare(&config).is_err(),
            "cleanup lost the state-owner lease before reaping"
        );
        let thread = std::thread::spawn(move || reap(child, lease));
        thread.join().unwrap();
        assert!(!PathBuf::from(format!("/proc/{pid}")).exists());
        PersistentState::prepare(&config).unwrap();
    }
}

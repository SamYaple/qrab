//! Pod UIDs from `/proc/<pid>/cgroup`, which is how the agent maps a socket
//! peer or an anchor's sandbox process to its Pod.

fn is_uid(candidate: &[u8]) -> bool {
    candidate.len() == 36
        && candidate.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-' || *c == b'_',
            _ => c.is_ascii_hexdigit(),
        })
}

/// The Pod UID from a host-view unified-cgroup-v2 record, for both the
/// systemd and cgroupfs drivers. Only exact Pod components under the standard
/// kubelet-owned `/kubepods.slice` or `/kubepods` root establish identity.
/// A substring in a user scope, delegated subtree or arbitrary ancestor does
/// not. Custom roots, namespace-relative views and multi-record/v1 files fail
/// closed; the node agent must read the host's unified hierarchy.
///
/// ```
/// use qrab_netd::cgroup::pod_uid;
/// let systemd = "0::/kubepods.slice/kubepods-besteffort.slice/kubepods-besteffort-pod73a925e4_0392_4cb8_8409_1abb7b5cc1b6.slice/cri-containerd-1f57.scope\n";
/// assert_eq!(pod_uid(systemd).as_deref(), Some("73a925e4-0392-4cb8-8409-1abb7b5cc1b6"));
/// let cgroupfs = "0::/kubepods/burstable/pod73a925e4-0392-4cb8-8409-1abb7b5cc1b6/1f57\n";
/// assert_eq!(pod_uid(cgroupfs).as_deref(), Some("73a925e4-0392-4cb8-8409-1abb7b5cc1b6"));
/// assert_eq!(pod_uid("0::/system.slice/containerd.service\n"), None);
/// ```
pub fn pod_uid(cgroup: &str) -> Option<String> {
    let record = cgroup.strip_suffix('\n').unwrap_or(cgroup);
    // Cgroup names can contain text delimiters. Never scan later lines for
    // a more convenient identity or treat them as independent v2 records.
    if record.contains(['\n', '\r']) {
        return None;
    }
    let path = record.strip_prefix("0::")?;
    let (rest, prefix, suffix) = if let Some(rest) = path.strip_prefix("/kubepods/") {
        (
            rest.strip_prefix("burstable/")
                .or_else(|| rest.strip_prefix("besteffort/"))
                .unwrap_or(rest),
            "pod",
            "",
        )
    } else {
        let rest = path.strip_prefix("/kubepods.slice/")?;
        if let Some(rest) = rest.strip_prefix("kubepods-burstable.slice/") {
            (rest, "kubepods-burstable-pod", ".slice")
        } else if let Some(rest) = rest.strip_prefix("kubepods-besteffort.slice/") {
            (rest, "kubepods-besteffort-pod", ".slice")
        } else {
            (rest, "kubepods-pod", ".slice")
        }
    };
    let component = rest.split('/').next()?;
    let uid = component.strip_prefix(prefix)?.strip_suffix(suffix)?;
    is_uid(uid.as_bytes()).then(|| uid.replace('_', "-").to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    const UID: &str = "6b9f0c44-0000-4000-8000-000000000001";

    #[test]
    fn a_host_user_scope_cannot_impersonate_a_pod() {
        // This path was produced by an unprivileged systemd-run --user
        // transient scope. A kernel-reported path is not enough: its Pod
        // component must belong to the kubelet-owned hierarchy.
        let host = format!("0::/user.slice/user-1002.slice/user@1002.service/app.slice/qrab-audit-pod{UID}.scope\n");
        assert_eq!(pod_uid(&host), None);
    }

    #[test]
    fn exact_kubelet_pod_components_support_each_qos_and_driver() {
        for qos in ["", "burstable", "besteffort"] {
            let fs_qos = if qos.is_empty() {
                String::new()
            } else {
                format!("{qos}/")
            };
            let cgroupfs = format!("0::/kubepods/{fs_qos}pod{UID}/container-id\n");
            assert_eq!(pod_uid(&cgroupfs).as_deref(), Some(UID));
            let (parent, prefix) = if qos.is_empty() {
                (String::new(), "kubepods-pod".to_owned())
            } else {
                (
                    format!("kubepods-{qos}.slice/"),
                    format!("kubepods-{qos}-pod"),
                )
            };
            let systemd = format!(
                "0::/kubepods.slice/{parent}{prefix}{}.slice/cri-containerd-id.scope\n",
                UID.replace('-', "_").to_uppercase()
            );
            assert_eq!(pod_uid(&systemd).as_deref(), Some(UID));
        }
        // A nested name cannot override the actual parent Pod's identity.
        let nested = format!("0::/kubepods/pod{UID}/child/podffffffff-ffff-4fff-8fff-ffffffffffff");
        assert_eq!(pod_uid(&nested).as_deref(), Some(UID));
    }

    #[test]
    fn lookalike_roots_components_and_extra_records_are_not_pod_identity() {
        let valid = format!("0::/kubepods/pod{UID}\n");
        for path in [
            format!("/user.slice/kubepods/pod{UID}"),
            format!("/delegated/kubepods.slice/kubepods-pod{UID}.slice"),
            format!("/kubepods/not-a-qos/pod{UID}"),
            format!("/kubepods/pod{UID}-extra"),
            format!("/kubepods/prefix-pod{UID}"),
            format!("/kubepods.slice/kubepods-pod{UID}.slice.extra"),
            format!("/kubepods.slice/kubepods-burstable.slice/kubepods-pod{UID}.slice"),
            format!("/../../kubepods.slice/kubepods-pod{UID}.slice"),
            "/kubepods/podnot-a-uuid".into(),
        ] {
            assert_eq!(pod_uid(&format!("0::{path}\n")), None, "{path}");
        }
        for text in [
            format!("0::/user.slice/forged\n{valid}"),
            format!("{valid}{valid}"),
            format!("0::/kubepods/pod{UID}\r\n"),
            format!("4:cpu:/kubepods/pod{UID}\n"),
            format!("4:cpu:/kubepods/pod{UID}\n{valid}"),
        ] {
            assert_eq!(pod_uid(&text), None, "{text:?}");
        }
    }
}

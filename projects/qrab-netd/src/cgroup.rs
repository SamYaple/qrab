//! Pod UIDs from `/proc/<pid>/cgroup`, which is how the agent maps a socket
//! peer or an anchor's sandbox process to its Pod.

fn is_uid(candidate: &[u8]) -> bool {
    candidate.len() == 36
        && candidate.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-' || *c == b'_',
            _ => c.is_ascii_hexdigit(),
        })
}

/// The Pod UID in a cgroup file, for both the systemd driver
/// (`kubepods-besteffort-pod<uid with _>.slice`) and cgroupfs (`pod<uid>`).
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
    cgroup
        .lines()
        .filter_map(|line| line.rsplit_once(':').map(|(_, path)| path))
        .flat_map(|path| path.split('/'))
        .find_map(|segment| {
            let bytes = segment.as_bytes();
            (0..bytes.len().saturating_sub(38)).find_map(|i| {
                (&bytes[i..i + 3] == b"pod" && is_uid(&bytes[i + 3..i + 39])).then(|| {
                    segment[i + 3..i + 39]
                        .replace('_', "-")
                        .to_ascii_lowercase()
                })
            })
        })
}

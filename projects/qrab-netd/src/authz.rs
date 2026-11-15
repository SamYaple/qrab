//! Which anchor's descriptors a socket peer may receive. The peer is resolved
//! to its Pod (SO_PEERCRED pid → cgroup → Pod UID) before this decision; a
//! VM Pod only ever receives its own NICs.
use crate::{DATAPATH_ANNOTATION, DATAPATH_TAP};
use k8s_openapi::api::core::v1::Pod;
use std::fmt;

pub const NAME_LABEL: &str = "app.kubernetes.io/name";
pub const COMPONENT_LABEL: &str = "app.kubernetes.io/component";
pub const OWNER_UID: &str = "qrab.yapplesauce.com/owner-uid";
pub const NIC_LABEL: &str = "qrab.yapplesauce.com/nic";
pub const VM_COMPONENT: &str = "vm";
pub const NIC_COMPONENT: &str = "vm-nic";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The peer is no Pod on this node (host process, other node's Pod).
    UnknownPeer,
    /// The peer's Pod is not a qrab VM Pod.
    NotAVmPod,
    /// No tap-mode anchor of this VM serves the NIC on this node.
    NoSuchNic(String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPeer => f.write_str("the peer is not a Pod on this node"),
            Self::NotAVmPod => f.write_str("only a qrab VM Pod may request NIC descriptors"),
            Self::NoSuchNic(nic) => write!(f, "this VM has no tap-mode NIC {nic:?} on this node"),
        }
    }
}

fn label<'a>(pod: &'a Pod, key: &str) -> Option<&'a str> {
    pod.metadata.labels.as_ref()?.get(key).map(String::as_str)
}

fn is_qrab(pod: &Pod, component: &str) -> bool {
    label(pod, NAME_LABEL) == Some("qrab") && label(pod, COMPONENT_LABEL) == Some(component)
}

/// The VM UID a Pod belongs to, from its `owner-uid` label.
pub fn owner(pod: &Pod) -> Option<&str> {
    label(pod, OWNER_UID).filter(|uid| !uid.is_empty())
}

/// The NIC name of an anchor.
pub fn nic(pod: &Pod) -> Option<&str> {
    label(pod, NIC_LABEL).filter(|nic| !nic.is_empty())
}

/// A live qrab NIC anchor in tap mode: the Pods this agent plumbs.
pub fn is_tap_anchor(pod: &Pod) -> bool {
    is_qrab(pod, NIC_COMPONENT)
        && pod.metadata.deletion_timestamp.is_none()
        && owner(pod).is_some()
        && nic(pod).is_some()
        && pod
            .metadata
            .annotations
            .as_ref()
            .and_then(|a| a.get(DATAPATH_ANNOTATION))
            .map(String::as_str)
            == Some(DATAPATH_TAP)
}

/// The anchor whose descriptors `requester` may receive for `nic`.
pub fn select_anchor<'a>(
    requester: Option<&Pod>,
    nic_name: &str,
    candidates: impl IntoIterator<Item = &'a Pod>,
) -> Result<&'a Pod, Refusal> {
    let requester = requester.ok_or(Refusal::UnknownPeer)?;
    let vm = owner(requester)
        .filter(|_| {
            is_qrab(requester, VM_COMPONENT) && requester.metadata.deletion_timestamp.is_none()
        })
        .ok_or(Refusal::NotAVmPod)?;
    candidates
        .into_iter()
        .find(|pod| is_tap_anchor(pod) && owner(pod) == Some(vm) && nic(pod) == Some(nic_name))
        .ok_or_else(|| Refusal::NoSuchNic(nic_name.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pod(component: &str, owner: &str, nic: Option<&str>, tap: bool) -> Pod {
        let mut labels =
            serde_json::json!({NAME_LABEL: "qrab", COMPONENT_LABEL: component, OWNER_UID: owner});
        if let Some(nic) = nic {
            labels[NIC_LABEL] = nic.into();
        }
        let annotations = if tap {
            serde_json::json!({DATAPATH_ANNOTATION: DATAPATH_TAP})
        } else {
            serde_json::json!({})
        };
        serde_json::from_value(
            serde_json::json!({"metadata": {"labels": labels, "annotations": annotations}}),
        )
        .unwrap()
    }

    #[test]
    fn a_vm_receives_only_its_own_tap_nics() {
        let anchors = [
            pod(NIC_COMPONENT, "vm-a", Some("lan"), true),
            pod(NIC_COMPONENT, "vm-b", Some("lan"), true),
            pod(NIC_COMPONENT, "vm-a", Some("wan"), false),
        ];
        let a = pod(VM_COMPONENT, "vm-a", None, false);
        let b = pod(VM_COMPONENT, "vm-b", None, false);
        assert_eq!(
            owner(select_anchor(Some(&a), "lan", &anchors).unwrap()),
            Some("vm-a")
        );
        assert_eq!(
            owner(select_anchor(Some(&b), "lan", &anchors).unwrap()),
            Some("vm-b")
        );
        // An unannotated NIC Pod is not handed out, and another VM's NIC is never found.
        assert_eq!(
            select_anchor(Some(&a), "wan", &anchors),
            Err(Refusal::NoSuchNic("wan".into()))
        );
        assert_eq!(
            select_anchor(Some(&b), "wan", &anchors),
            Err(Refusal::NoSuchNic("wan".into()))
        );
    }

    #[test]
    fn crafted_nic_names_and_owners_never_cross_vms() {
        let weird = [
            "",
            "lan ",
            "LAN",
            "../lan",
            "lan\0",
            "*",
            ".*",
            &"l".repeat(5000),
        ];
        let mut anchors = vec![pod(NIC_COMPONENT, "vm-b", Some("lan"), true)];
        for name in weird {
            anchors.push(pod(NIC_COMPONENT, "vm-b", Some(name), true));
        }
        // An anchor with an empty owner is never anyone's.
        anchors.push(pod(NIC_COMPONENT, "", Some("lan"), true));
        let a = pod(VM_COMPONENT, "vm-a", None, false);
        for name in weird.iter().copied().chain(["lan"]) {
            assert!(
                select_anchor(Some(&a), name, &anchors).is_err(),
                "vm-a got {name:?}"
            );
        }
        // A VM Pod whose own owner label is empty is not a VM.
        let ownerless = pod(VM_COMPONENT, "", None, false);
        assert_eq!(
            select_anchor(Some(&ownerless), "lan", &anchors),
            Err(Refusal::NotAVmPod)
        );
    }

    #[test]
    fn unknown_peers_and_non_vm_pods_are_refused() {
        let anchors = [pod(NIC_COMPONENT, "vm-a", Some("lan"), true)];
        assert_eq!(
            select_anchor(None, "lan", &anchors),
            Err(Refusal::UnknownPeer)
        );
        // The anchor itself, or any other qrab Pod with the same owner label, is not a VM.
        let anchor = pod(NIC_COMPONENT, "vm-a", Some("lan"), true);
        assert_eq!(
            select_anchor(Some(&anchor), "lan", &anchors),
            Err(Refusal::NotAVmPod)
        );
        let redfish = pod("redfish-events", "vm-a", None, false);
        assert_eq!(
            select_anchor(Some(&redfish), "lan", &anchors),
            Err(Refusal::NotAVmPod)
        );
        let mut stranger = pod(VM_COMPONENT, "vm-a", None, false);
        stranger
            .metadata
            .labels
            .as_mut()
            .unwrap()
            .insert(NAME_LABEL.into(), "other".into());
        assert_eq!(
            select_anchor(Some(&stranger), "lan", &anchors),
            Err(Refusal::NotAVmPod)
        );
    }

    #[test]
    fn deleting_anchors_are_not_plumbed_or_served() {
        let mut anchor = pod(NIC_COMPONENT, "vm-a", Some("lan"), true);
        assert!(is_tap_anchor(&anchor));
        anchor.metadata.deletion_timestamp =
            Some(k8s_openapi::apimachinery::pkg::apis::meta::v1::Time(
                k8s_openapi::jiff::Timestamp::UNIX_EPOCH,
            ));
        assert!(!is_tap_anchor(&anchor));
        assert!(!is_tap_anchor(&pod(
            NIC_COMPONENT,
            "vm-a",
            Some("lan"),
            false
        )));
        assert!(!is_tap_anchor(&pod(NIC_COMPONENT, "vm-a", None, true)));
    }
}

//! Names and MAC addresses derived from a VM UID and NIC name, identical on
//! every node so a migrating guest finds the same gateway on its new node.
use sha2::{Digest, Sha256};

fn digest(domain: &str, uid: &str, nic: &str) -> [u8; 32] {
    Sha256::digest(format!("{domain}\0{uid}\0{nic}")).into()
}

fn mac_from(bytes: &[u8]) -> String {
    std::iter::once(0x02)
        .chain(bytes[..5].iter().copied())
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// The guest's MAC. Must equal `qrab_operator::nic::mac`, which the operator
/// gives QEMU and the NoCloud seed.
///
/// ```
/// assert_eq!(
///     qrab_netd::identity::guest_mac("6b9f0c44-0000-4000-8000-000000000001", "lan"),
///     "02:21:c2:51:b0:38"
/// );
/// ```
pub fn guest_mac(uid: &str, nic: &str) -> String {
    mac_from(&digest("qrab-nic", uid, nic))
}

/// The tap's own MAC: the guest's gateway, distinct from the guest's MAC.
///
/// ```
/// use qrab_netd::identity::{guest_mac, tap_mac};
/// let uid = "6b9f0c44-0000-4000-8000-000000000001";
/// assert_eq!(tap_mac(uid, "lan"), tap_mac(uid, "lan"));
/// assert_ne!(tap_mac(uid, "lan"), guest_mac(uid, "lan"));
/// assert_ne!(tap_mac(uid, "lan"), tap_mac(uid, "wan"));
/// ```
pub fn tap_mac(uid: &str, nic: &str) -> String {
    let mut bytes = digest("qrab-tap", uid, nic);
    if mac_from(&bytes) == guest_mac(uid, nic) {
        bytes[4] ^= 1;
    }
    mac_from(&bytes)
}

/// The tap's interface name, unique on a node: `qt` and 12 hex digits (14
/// bytes, within the kernel's 15).
///
/// ```
/// let name = qrab_netd::identity::tap_name("6b9f0c44-0000-4000-8000-000000000001", "lan");
/// assert!(name.starts_with("qt") && name.len() == 14);
/// ```
pub fn tap_name(uid: &str, nic: &str) -> String {
    let bytes = digest("qrab-tap-name", uid, nic);
    let hex: String = bytes[..6].iter().map(|b| format!("{b:02x}")).collect();
    format!("qt{hex}")
}

//! qrab-netd: the per-node agent of the tap datapath (docs/networking.md,
//! "Tap datapath"). For every NIC anchor Pod on its node whose
//! `qrab.yapplesauce.com/nic-datapath` annotation is `tap`, it enters the
//! anchor's network namespace, creates a persistent multiqueue tap there,
//! moves the guest's /32 from `eth0` to the tap, and answers the guest's
//! gateway and DNS address. It holds no tap file descriptors itself: the
//! owning VM's manager asks over [`wire`] for a NIC's queues and receives fresh
//! tap and vhost-net descriptors, so restarting the agent never disturbs a
//! running guest.
#[cfg(feature = "agent")]
pub mod authz;
pub mod cgroup;
pub mod ct;
pub mod dhcp;
pub mod identity;
pub mod natpmp;
pub mod peer;
#[cfg(feature = "agent")]
pub mod peer_tls;
#[cfg(feature = "agent")]
pub mod plan;
#[cfg(feature = "agent")]
pub mod sys;
pub mod vpn;
pub mod wire;

/// Anchor annotation selecting the tap datapath; any other value (or none)
/// leaves the Pod alone.
pub const DATAPATH_ANNOTATION: &str = "qrab.yapplesauce.com/nic-datapath";
pub const DATAPATH_TAP: &str = "tap";
/// Pod condition the agent sets on each anchor it serves; anchors list it as
/// a readiness gate so they become Ready only once plumbed.
pub const PLUMBED_CONDITION: &str = "qrab.yapplesauce.com/nic-plumbed";
/// Anchor condition of a port-forwarding VPN NIC: True with the forwarded
/// public port as its message, False with why not.
pub const PORT_FORWARD_CONDITION: &str = "qrab.yapplesauce.com/vpn-port-forward";
/// Host directory holding the agent's socket. The KVM device plugin's CDI
/// spec mounts it into VM Pods at the same path.
pub const SOCKET_DIR: &str = "/run/qrab-netd";
pub const SOCKET_NAME: &str = "netd.sock";
/// The guest's on-link gateway and DNS server.
pub const GUEST_GATEWAY: std::net::Ipv4Addr = std::net::Ipv4Addr::new(169, 254, 1, 1);

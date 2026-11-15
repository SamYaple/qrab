//! What one NIC's anchor network namespace must look like, as `ip` commands,
//! sysctls and an nftables ruleset. Every step is idempotent so the agent can
//! re-apply the whole plan after a restart without disturbing a running guest.
use crate::{authz, identity, GUEST_GATEWAY};
use k8s_openapi::api::core::v1::Pod;
use std::net::Ipv4Addr;

/// The Pod's own interface, where Cilium delivers the guest's /32.
pub const UPLINK: &str = "eth0";
/// See [`NicPlan::uplink_placeholder`].
pub const UPLINK_PLACEHOLDER: &str = "169.254.1.2/32";
/// nftables table holding the guest's DNS redirect.
pub const NFT_TABLE: &str = crate::vpn::NFT_TABLE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NicPlan {
    pub tap: String,
    pub tap_mac: String,
    pub guest_mac: String,
    /// The guest's address: the anchor's pool /32 for a plain NIC, the
    /// tunnel's address for a VPN NIC.
    pub guest_ip: Ipv4Addr,
    /// The WireGuard tunnel of a VPN NIC ([`crate::vpn`]); the agent loads it
    /// from the anchor's mounted Secret ([`NicPlan::with_tunnel`]).
    pub vpn: Option<crate::vpn::Tunnel>,
}

impl NicPlan {
    /// The plan for a tap-mode anchor, from its labels and Pod IP.
    pub fn for_anchor(pod: &Pod) -> Result<Self, String> {
        if !authz::is_tap_anchor(pod) {
            return Err("not a tap-mode qrab NIC anchor".into());
        }
        let (Some(uid), Some(nic)) = (authz::owner(pod), authz::nic(pod)) else {
            return Err("anchor lacks its owner-uid or nic label".into());
        };
        let guest_ip = pod
            .status
            .as_ref()
            .and_then(|s| s.pod_ip.as_deref())
            .ok_or("anchor has no Pod IP yet")?
            .parse::<Ipv4Addr>()
            .map_err(|_| "the anchor's Pod IP is not IPv4".to_owned())?;
        Ok(Self {
            tap: identity::tap_name(uid, nic),
            tap_mac: identity::tap_mac(uid, nic),
            guest_mac: identity::guest_mac(uid, nic),
            guest_ip,
            vpn: None,
        })
    }

    /// Whether the anchor is a VPN NIC's (its tunnel config is mounted).
    pub fn is_vpn(pod: &Pod) -> bool {
        pod.metadata
            .annotations
            .as_ref()
            .and_then(|a| a.get(crate::vpn::VPN_ANNOTATION))
            .is_some_and(|v| v == crate::vpn::VPN_WIREGUARD)
    }

    /// A VPN NIC's plan: the guest gets the tunnel's address; the anchor's
    /// pool /32 stays on `eth0` as the tunnel's outer address.
    pub fn with_tunnel(mut self, tunnel: crate::vpn::Tunnel) -> Self {
        self.guest_ip = tunnel.address;
        self.vpn = Some(tunnel);
        self
    }

    /// `ip` commands for an existing tap, run inside the anchor's namespace
    /// before the guest's address leaves `eth0`.
    pub fn link_commands(&self, mtu: u32) -> Vec<Vec<String>> {
        let tap = self.tap.as_str();
        let host = format!("{}/32", self.guest_ip);
        let gateway = format!("{GUEST_GATEWAY}/32");
        [
            vec![
                "link",
                "set",
                "dev",
                tap,
                "address",
                &self.tap_mac,
                "mtu",
                &mtu.to_string(),
                "up",
            ],
            vec!["addr", "replace", &gateway, "dev", tap],
            vec!["route", "replace", &host, "dev", tap],
            vec![
                "neigh",
                "replace",
                &self.guest_ip.to_string(),
                "lladdr",
                &self.guest_mac,
                "dev",
                tap,
                "nud",
                "permanent",
            ],
        ]
        .into_iter()
        .map(|args| args.into_iter().map(str::to_owned).collect())
        .collect()
    }

    /// A link-scope address `eth0` keeps once the guest's /32 has left it.
    /// The kernel flushes every IPv4 route of a device whose last IPv4
    /// address goes, including the anchor's default route, so this is added
    /// before the release. Link scope keeps it out of source selection for
    /// forwarded traffic; it only sources the anchor's own ARP for its
    /// gateway.
    pub fn uplink_placeholder(&self) -> Vec<String> {
        [
            "addr",
            "replace",
            UPLINK_PLACEHOLDER,
            "dev",
            UPLINK,
            "scope",
            "link",
        ]
        .map(str::to_owned)
        .to_vec()
    }

    /// The command that hands the address over: once it leaves `eth0`, the
    /// anchor forwards the guest's traffic instead of delivering it locally.
    pub fn uplink_release(&self) -> Vec<String> {
        [
            "addr",
            "del",
            &format!("{}/32", self.guest_ip),
            "dev",
            UPLINK,
        ]
        .map(str::to_owned)
        .to_vec()
    }

    /// `/proc/sys` keys and values. Forwarding is the point; reverse-path
    /// filtering would drop guest packets arriving on a tap without a route.
    pub fn sysctls(&self) -> Vec<(String, &'static str)> {
        vec![
            ("net/ipv4/ip_forward".into(), "1"),
            ("net/ipv4/conf/all/rp_filter".into(), "0"),
            (format!("net/ipv4/conf/{UPLINK}/rp_filter"), "0"),
            (format!("net/ipv4/conf/{}/rp_filter", self.tap), "0"),
        ]
    }

    /// The guest's DNS server is its gateway; redirect it to the anchor's
    /// own resolver (cluster DNS, reached through Cilium like any Pod
    /// traffic). Replacing the table is one atomic transaction.
    pub fn dns_ruleset(&self, resolver: Ipv4Addr) -> String {
        let tap = &self.tap;
        format!(
            "table ip {NFT_TABLE}\ndelete table ip {NFT_TABLE}\n\
             table ip {NFT_TABLE} {{\n  chain dns {{\n    type nat hook prerouting priority dstnat; policy accept;\n    \
             iifname \"{tap}\" ip daddr {GUEST_GATEWAY} udp dport 53 dnat to {resolver}\n    \
             iifname \"{tap}\" ip daddr {GUEST_GATEWAY} tcp dport 53 dnat to {resolver}\n  }}\n}}\n"
        )
    }
}

/// The first IPv4 nameserver in a resolv.conf.
///
/// ```
/// let conf = "search qrab.svc.cluster.local\nnameserver fd00::a\nnameserver 10.96.0.10\n";
/// assert_eq!(qrab_netd::plan::resolver(conf), Some("10.96.0.10".parse().unwrap()));
/// ```
pub fn resolver(resolv_conf: &str) -> Option<Ipv4Addr> {
    resolv_conf.lines().find_map(|line| {
        let mut words = line.split_whitespace();
        (words.next() == Some("nameserver"))
            .then(|| words.next()?.parse().ok())
            .flatten()
    })
}

/// The MTU in one line of `ip -o link show`.
///
/// ```
/// let line = "623: eth0@if624: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc noqueue state UP";
/// assert_eq!(qrab_netd::plan::mtu(line), Some(1500));
/// ```
pub fn mtu(ip_link: &str) -> Option<u32> {
    let mut words = ip_link.split_whitespace();
    words.find(|w| *w == "mtu")?;
    words.next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DATAPATH_ANNOTATION, DATAPATH_TAP};

    fn anchor() -> Pod {
        serde_json::from_value(serde_json::json!({
            "metadata": {
                "labels": {
                    authz::NAME_LABEL: "qrab", authz::COMPONENT_LABEL: authz::NIC_COMPONENT,
                    authz::OWNER_UID: "6b9f0c44-0000-4000-8000-000000000001", authz::NIC_LABEL: "lan",
                },
                "annotations": {DATAPATH_ANNOTATION: DATAPATH_TAP},
            },
            "status": {"podIP": "10.244.128.7"},
        }))
        .unwrap()
    }

    #[test]
    fn one_nic_renders_its_full_plan() {
        let plan = NicPlan::for_anchor(&anchor()).unwrap();
        let uid = "6b9f0c44-0000-4000-8000-000000000001";
        assert_eq!(plan.tap, identity::tap_name(uid, "lan"));
        assert_eq!(plan.guest_mac, "02:21:c2:51:b0:38");
        let tap = plan.tap.clone();
        let mac = plan.tap_mac.clone();
        let words = |s: &str| s.split(' ').map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            plan.link_commands(1500),
            vec![
                words(&format!("link set dev {tap} address {mac} mtu 1500 up")),
                words(&format!("addr replace 169.254.1.1/32 dev {tap}")),
                words(&format!("route replace 10.244.128.7/32 dev {tap}")),
                words(&format!(
                    "neigh replace 10.244.128.7 lladdr 02:21:c2:51:b0:38 dev {tap} nud permanent"
                )),
            ]
        );
        assert_eq!(
            plan.uplink_release(),
            words("addr del 10.244.128.7/32 dev eth0")
        );
        assert!(plan
            .sysctls()
            .contains(&("net/ipv4/ip_forward".into(), "1")));
        assert!(plan
            .sysctls()
            .contains(&(format!("net/ipv4/conf/{tap}/rp_filter"), "0")));
        let rules = plan.dns_ruleset("10.96.0.10".parse().unwrap());
        assert!(rules.starts_with("table ip qrab_netd\ndelete table ip qrab_netd\n"));
        assert!(rules.contains(&format!(
            "iifname \"{tap}\" ip daddr 169.254.1.1 udp dport 53 dnat to 10.96.0.10"
        )));
        assert!(rules.contains(&format!(
            "iifname \"{tap}\" ip daddr 169.254.1.1 tcp dport 53 dnat to 10.96.0.10"
        )));
    }

    #[test]
    fn anchors_without_an_ipv4_address_or_tap_mode_have_no_plan() {
        let mut pending = anchor();
        pending.status = None;
        assert!(NicPlan::for_anchor(&pending).is_err());
        let mut v6 = anchor();
        v6.status.as_mut().unwrap().pod_ip = Some("fd00::7".into());
        assert!(NicPlan::for_anchor(&v6).is_err());
        let mut unannotated = anchor();
        unannotated.metadata.annotations = None;
        assert!(NicPlan::for_anchor(&unannotated).is_err());
    }
}

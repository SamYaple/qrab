//! The `wg.conf` a VPN NIC's Secret carries, and the connection features
//! Proton binds to a profile's certificate.
//!
//! ```
//! use qrab_proton::{catalog::Choice, keys::ProfileKey, wg::{wg_conf, Features}};
//! let key = ProfileKey::from_seed([7; 32]);
//! let choice = Choice {
//!     name: "CH#12".into(), logical_id: "l".into(), physical_id: "p".into(),
//!     entry_ip: "185.159.157.1".parse().unwrap(),
//!     public_key: "xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=".into(),
//!     entry_country: "CH".into(), exit_country: "CH".into(), city: None,
//!     load: 20, score: 1.0, features: 4, tier: 2,
//! };
//! let features = Features { port_forwarding: true, ..Features::default() };
//! let conf = wg_conf(&key, &choice, &features);
//! assert!(conf.contains("Endpoint = 185.159.157.1:51820\n"));
//! assert!(conf.contains("# qrab: port-forwarding\n"));
//! assert!(conf.contains(&format!("PrivateKey = {}\n", key.wireguard_private().as_str())));
//! ```
use crate::{catalog::Choice, keys::ProfileKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::net::Ipv4Addr;
use zeroize::Zeroizing;

/// Every Proton WireGuard client gets this address and resolver in the tunnel.
pub const TUNNEL_ADDRESS: Ipv4Addr = Ipv4Addr::new(10, 2, 0, 2);
pub const TUNNEL_DNS: Ipv4Addr = Ipv4Addr::new(10, 2, 0, 1);
pub const WIREGUARD_PORT: u16 = 51820;
/// The marker qrab-netd looks for to run NAT-PMP renewals for the NIC.
pub const PORT_FORWARDING_MARKER: &str = "# qrab: port-forwarding";

/// Connection features bound to a certificate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Features {
    /// NetShield DNS filtering: 0 off, 1 malware, 2 malware, ads and trackers.
    #[serde(default = "netshield_default")]
    pub net_shield: u8,
    #[serde(default)]
    pub port_forwarding: bool,
    #[serde(default)]
    pub moderate_nat: bool,
    #[serde(default = "yes")]
    pub vpn_accelerator: bool,
}

fn netshield_default() -> u8 {
    1
}
fn yes() -> bool {
    true
}

impl Default for Features {
    fn default() -> Self {
        Self {
            net_shield: netshield_default(),
            port_forwarding: false,
            moderate_nat: false,
            vpn_accelerator: true,
        }
    }
}

impl Features {
    /// The certificate request's `Features`, mirroring ProtonVPN's client:
    /// only non-default values are sent (`RandomNAT` and `SplitTCP` default
    /// on at Proton, so only `false` is ever sent for them).
    pub fn certificate_features(&self) -> Value {
        let mut map = Map::new();
        if !self.moderate_nat {
            map.insert("RandomNAT".into(), json!(false));
        }
        if !self.vpn_accelerator {
            map.insert("SplitTCP".into(), json!(false));
        }
        if self.port_forwarding {
            map.insert("PortForwarding".into(), json!(true));
        }
        if self.net_shield != 0 {
            map.insert("NetShieldLevel".into(), json!(self.net_shield.min(2)));
        }
        Value::Object(map)
    }
}

/// The wg-quick config for `choice` with `key`. It carries the private key:
/// it belongs only in the profile's `vpn-*` Secret.
pub fn wg_conf(key: &ProfileKey, choice: &Choice, features: &Features) -> Zeroizing<String> {
    let mut text = Zeroizing::new(String::new());
    text.push_str("[Interface]\n");
    text.push_str(&format!(
        "# qrab: proton {} ({})\n",
        choice.name, choice.physical_id
    ));
    if features.port_forwarding {
        text.push_str(PORT_FORWARDING_MARKER);
        text.push('\n');
    }
    text.push_str(&format!(
        "PrivateKey = {}\n",
        key.wireguard_private().as_str()
    ));
    text.push_str(&format!(
        "Address = {TUNNEL_ADDRESS}/32\nDNS = {TUNNEL_DNS}\n\n"
    ));
    text.push_str(&format!(
        "[Peer]\nPublicKey = {}\nAllowedIPs = 0.0.0.0/0\nEndpoint = {}:{WIREGUARD_PORT}\n",
        choice.public_key, choice.entry_ip
    ));
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certificate_features_mirror_protons_client() {
        assert_eq!(
            Features::default().certificate_features(),
            json!({"RandomNAT": false, "NetShieldLevel": 1})
        );
        let all = Features {
            net_shield: 9,
            port_forwarding: true,
            moderate_nat: true,
            vpn_accelerator: false,
        };
        assert_eq!(
            all.certificate_features(),
            json!({"SplitTCP": false, "PortForwarding": true, "NetShieldLevel": 2})
        );
        let off = Features {
            net_shield: 0,
            ..Features::default()
        };
        assert_eq!(off.certificate_features(), json!({"RandomNAT": false}));
    }
}

//! WireGuard VPN NICs. The anchor terminates the tunnel: `wg0` lives in the
//! anchor's network namespace, its encrypted UDP leaves through the anchor's
//! own `eth0` (under the anchor's Cilium identity and security groups), and
//! the guest's tap forwards only to and from `wg0`. The private key comes
//! from a Secret mounted read-only into the anchor; the guest and the VM Pod
//! never see it.
//!
//! The configuration is a wg-quick file, as ProtonVPN and most providers
//! export it:
//!
//! ```
//! use qrab_netd::vpn::Tunnel;
//! let tunnel = Tunnel::parse("
//! [Interface]
//! ## Key for qrab-test
//! PrivateKey = yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=
//! Address = 10.2.0.2/32
//! DNS = 10.2.0.1
//!
//! [Peer]
//! PublicKey = xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=
//! AllowedIPs = 0.0.0.0/0
//! Endpoint = 185.159.157.1:51820
//! ").unwrap();
//! assert_eq!(tunnel.address.to_string(), "10.2.0.2");
//! assert_eq!(tunnel.dns, ["10.2.0.1".parse::<std::net::Ipv4Addr>().unwrap()]);
//! // The key never shows in logs.
//! assert!(!format!("{tunnel:?}").contains("yAnz5"));
//! // What `wg setconf` gets: no wg-quick keys, a fixed port, a keepalive.
//! let setconf = tunnel.setconf();
//! assert!(setconf.contains("ListenPort = 51820\n") && setconf.contains("PersistentKeepalive = 25\n"));
//! assert!(!setconf.contains("Address") && !setconf.contains("DNS"));
//! ```
use std::net::{Ipv4Addr, SocketAddrV4};

/// Anchor annotation marking a VPN NIC (value [`VPN_WIREGUARD`]).
pub const VPN_ANNOTATION: &str = "qrab.yapplesauce.com/nic-vpn";
pub const VPN_WIREGUARD: &str = "wireguard";
/// Where the anchor mounts the Secret, and the key holding the config.
pub const VPN_MOUNT: &str = "/run/qrab-vpn";
pub const VPN_FILE: &str = "wg.conf";
/// The tunnel interface in the anchor's namespace.
pub const WG_LINK: &str = "wg0";
/// Fixed, so a migrated anchor keeps the same UDP 5-tuple towards the server.
pub const LISTEN_PORT: u16 = 51820;
/// Routing table the tap's traffic is steered into.
pub const TABLE: &str = "100";
/// WireGuard's worst-case IPv4/IPv6 encapsulation overhead.
pub const OVERHEAD: u32 = 80;
/// Keeps NAT mappings open and makes a migrated anchor handshake at once.
pub const KEEPALIVE: u16 = 25;
/// nftables table of the anchor's rules (DNS redirect or kill switch).
pub const NFT_TABLE: &str = "qrab_netd";
/// Largest config accepted.
pub const MAX_CONFIG: usize = 16 * 1024;

/// One WireGuard tunnel: what the guest sees and what `wg` is told.
#[derive(Clone, PartialEq, Eq)]
pub struct Tunnel {
    /// The guest's address inside the tunnel (the first IPv4 `Address`).
    pub address: Ipv4Addr,
    pub dns: Vec<Ipv4Addr>,
    pub mtu: Option<u32>,
    pub endpoint: SocketAddrV4,
    private_key: String,
    public_key: String,
    preshared_key: Option<String>,
    allowed_ips: String,
    keepalive: u16,
}

impl std::fmt::Debug for Tunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tunnel")
            .field("address", &self.address)
            .field("dns", &self.dns)
            .field("mtu", &self.mtu)
            .field("endpoint", &self.endpoint)
            .field("public_key", &self.public_key)
            .field("private_key", &"<redacted>")
            .finish_non_exhaustive()
    }
}

/// A WireGuard key: 32 bytes in standard base64 (44 characters, one `=`).
fn key(value: &str) -> Option<String> {
    let ok = value.len() == 44
        && value.ends_with('=')
        && value[..43]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/');
    ok.then(|| value.to_owned())
}

impl Tunnel {
    /// Parse a wg-quick config with one `[Peer]`. Hooks (`PreUp`, `PostUp`,
    /// …) are refused rather than ignored: nothing here runs them, and a
    /// config that relies on them would not behave as its author expects.
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > MAX_CONFIG {
            return Err("the WireGuard config is too large".into());
        }
        #[derive(PartialEq)]
        enum Section {
            None,
            Interface,
            Peer,
        }
        let mut section = Section::None;
        let mut peers = 0;
        let (mut private_key, mut address, mut dns, mut mtu) = (None, None, Vec::new(), None);
        let (mut public_key, mut preshared_key, mut allowed_ips, mut endpoint, mut keepalive) =
            (None, None, None, None, None);
        for (number, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or_default().trim();
            if line.is_empty() {
                continue;
            }
            let at = |what: &str| format!("line {}: {what}", number + 1);
            if line.eq_ignore_ascii_case("[interface]") {
                section = Section::Interface;
                continue;
            }
            if line.eq_ignore_ascii_case("[peer]") {
                section = Section::Peer;
                peers += 1;
                continue;
            }
            let (name, value) = line
                .split_once('=')
                .map(|(n, v)| (n.trim().to_ascii_lowercase(), v.trim()))
                .ok_or_else(|| at("expected `Key = value`"))?;
            match (&section, name.as_str()) {
                (Section::Interface, "privatekey") => {
                    private_key = Some(key(value).ok_or_else(|| at("PrivateKey is not a WireGuard key"))?)
                }
                (Section::Interface, "address") => {
                    for part in value.split(',').map(str::trim) {
                        let ip = part.split('/').next().unwrap_or_default();
                        if let (None, Ok(v4)) = (&address, ip.parse::<Ipv4Addr>()) {
                            address = Some(v4);
                        }
                    }
                }
                (Section::Interface, "dns") => dns.extend(
                    value
                        .split(',')
                        .filter_map(|d| d.trim().parse::<Ipv4Addr>().ok()),
                ),
                (Section::Interface, "mtu") => {
                    mtu = Some(
                        value
                            .parse::<u32>()
                            .ok()
                            .filter(|m| (576..=9000).contains(m))
                            .ok_or_else(|| at("MTU must be 576..9000"))?,
                    )
                }
                (Section::Interface, "listenport" | "table" | "saveconfig" | "fwmark") => {}
                (Section::Interface, "preup" | "postup" | "predown" | "postdown") => {
                    return Err(at("wg-quick hooks are not supported"))
                }
                (Section::Peer, "publickey") => {
                    public_key = Some(key(value).ok_or_else(|| at("PublicKey is not a WireGuard key"))?)
                }
                (Section::Peer, "presharedkey") => {
                    preshared_key =
                        Some(key(value).ok_or_else(|| at("PresharedKey is not a WireGuard key"))?)
                }
                (Section::Peer, "allowedips") => {
                    let ips: Vec<&str> = value.split(',').map(str::trim).collect();
                    if ips.iter().any(|p| {
                        p.is_empty()
                            || !p
                                .bytes()
                                .all(|b| b.is_ascii_hexdigit() || b".:/".contains(&b))
                    }) {
                        return Err(at("AllowedIPs must be a comma-separated list of prefixes"));
                    }
                    allowed_ips = Some(ips.join(", "));
                }
                (Section::Peer, "endpoint") => {
                    endpoint = Some(value.parse::<SocketAddrV4>().map_err(|_| {
                        at("Endpoint must be an IPv4 address and port (host names are not resolved)")
                    })?)
                }
                (Section::Peer, "persistentkeepalive") => {
                    keepalive = Some(
                        value
                            .parse::<u16>()
                            .map_err(|_| at("PersistentKeepalive must be seconds"))?,
                    )
                }
                (Section::None, _) => return Err(at("a key before [Interface]")),
                (_, other) => return Err(at(&format!("unknown key {other:?}"))),
            }
        }
        if peers != 1 {
            return Err(format!(
                "the config must have exactly one [Peer], it has {peers}"
            ));
        }
        Ok(Self {
            address: address.ok_or("the [Interface] has no IPv4 Address")?,
            dns,
            mtu,
            endpoint: endpoint.ok_or("the [Peer] has no Endpoint")?,
            private_key: private_key.ok_or("the [Interface] has no PrivateKey")?,
            public_key: public_key.ok_or("the [Peer] has no PublicKey")?,
            preshared_key,
            allowed_ips: allowed_ips.unwrap_or_else(|| "0.0.0.0/0".into()),
            keepalive: keepalive.filter(|k| *k > 0).unwrap_or(KEEPALIVE),
        })
    }

    /// The `wg setconf` input (carries the private key: pass it on stdin).
    pub fn setconf(&self) -> String {
        let mut text = format!(
            "[Interface]\nPrivateKey = {}\nListenPort = {LISTEN_PORT}\n\n[Peer]\nPublicKey = {}\n",
            self.private_key, self.public_key
        );
        if let Some(psk) = &self.preshared_key {
            text += &format!("PresharedKey = {psk}\n");
        }
        text += &format!(
            "AllowedIPs = {}\nEndpoint = {}\nPersistentKeepalive = {}\n",
            self.allowed_ips, self.endpoint, self.keepalive
        );
        text
    }

    /// The tunnel's MTU on an uplink of `uplink` bytes.
    pub fn mtu_on(&self, uplink: u32) -> u32 {
        self.mtu.unwrap_or(uplink.saturating_sub(OVERHEAD))
    }
}

/// The anchor's filter for a VPN NIC: the tap forwards only to and from the
/// tunnel (the kill switch: with `wg0` down nothing of the guest's leaves), and
/// the guest reaches the anchor itself only for DHCP and pings of its
/// gateway. Replacing the table is one atomic transaction.
pub fn kill_switch(tap: &str, guest: Ipv4Addr) -> String {
    let table = NFT_TABLE;
    format!(
        "table inet {table}\ndelete table inet {table}\n\
         table inet {table} {{\n  chain forward {{\n    type filter hook forward priority filter; policy drop;\n    \
         iifname \"{tap}\" oifname \"{WG_LINK}\" ip saddr {guest} accept\n    \
         iifname \"{WG_LINK}\" oifname \"{tap}\" ip daddr {guest} accept\n  }}\n  \
         chain input {{\n    type filter hook input priority filter; policy accept;\n    \
         iifname \"{tap}\" udp dport 67 accept\n    \
         iifname \"{tap}\" ip daddr {gw} icmp type echo-request accept\n    \
         iifname \"{tap}\" drop\n  }}\n}}\n",
        gw = crate::GUEST_GATEWAY
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_A: &str = "yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=";
    const KEY_B: &str = "xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=";

    fn config(interface: &str, peers: &str) -> String {
        format!("[Interface]\nPrivateKey = {KEY_A}\n{interface}\n{peers}")
    }
    fn peer(extra: &str) -> String {
        format!("[Peer]\nPublicKey = {KEY_B}\nEndpoint = 203.0.113.9:51820\n{extra}\n")
    }

    #[test]
    fn protonvpn_style_configs_parse() {
        // ProtonVPN: comments, IPv4 + IPv6 Address and DNS, no keepalive.
        let tunnel = Tunnel::parse(&config(
            "Address = 10.2.0.2/32, 2a07:b944::2:2/128\nDNS = 10.2.0.1, 2a07:b944::2:1",
            &peer("AllowedIPs = 0.0.0.0/0, ::/0"),
        ))
        .unwrap();
        assert_eq!(tunnel.address, Ipv4Addr::new(10, 2, 0, 2));
        assert_eq!(tunnel.dns, [Ipv4Addr::new(10, 2, 0, 1)]);
        assert_eq!(tunnel.endpoint.to_string(), "203.0.113.9:51820");
        assert_eq!(tunnel.mtu_on(1500), 1420);
        let setconf = tunnel.setconf();
        assert_eq!(
            setconf,
            format!("[Interface]\nPrivateKey = {KEY_A}\nListenPort = 51820\n\n[Peer]\nPublicKey = {KEY_B}\nAllowedIPs = 0.0.0.0/0, ::/0\nEndpoint = 203.0.113.9:51820\nPersistentKeepalive = 25\n")
        );
        // A preshared key, an MTU and keepalive are carried over.
        let tunnel = Tunnel::parse(&config(
            "Address = 10.66.0.2/24\nMTU = 1380",
            &peer(&format!("PresharedKey = {KEY_A}\nPersistentKeepalive = 15")),
        ))
        .unwrap();
        assert_eq!(tunnel.mtu_on(1500), 1380);
        assert!(tunnel
            .setconf()
            .contains(&format!("PresharedKey = {KEY_A}\n")));
        assert!(tunnel.setconf().contains("PersistentKeepalive = 15\n"));
        assert!(
            tunnel.setconf().contains("AllowedIPs = 0.0.0.0/0\n"),
            "full tunnel by default"
        );
    }

    #[test]
    fn unusable_or_unsafe_configs_are_refused() {
        for (text, problem) in [
            (config("Address = 10.2.0.2/32", ""), "exactly one [Peer]"),
            (
                config("Address = 10.2.0.2/32", &(peer("") + &peer(""))),
                "exactly one [Peer]",
            ),
            (config("", &peer("")), "no IPv4 Address"),
            (
                config("Address = fd00::2/128", &peer("")),
                "no IPv4 Address",
            ),
            (
                config("Address = 10.2.0.2/32\nPostUp = iptables -F", &peer("")),
                "hooks",
            ),
            (
                config(
                    "Address = 10.2.0.2/32",
                    &peer("").replace("203.0.113.9", "vpn.example.com"),
                ),
                "host names",
            ),
            (
                config("Address = 10.2.0.2/32", &peer("").replace(KEY_B, "short")),
                "PublicKey",
            ),
            (
                config("Address = 10.2.0.2/32\nMTU = 70000", &peer("")),
                "MTU",
            ),
            (
                config(
                    "Address = 10.2.0.2/32",
                    &peer("AllowedIPs = 0.0.0.0/0; rm -rf /"),
                ),
                "AllowedIPs",
            ),
            (
                config("Address = 10.2.0.2/32", &peer("Bogus = 1")),
                "unknown key",
            ),
            ("PrivateKey = x\n".into(), "before [Interface]"),
            ("x".repeat(MAX_CONFIG + 1), "too large"),
        ] {
            let error = Tunnel::parse(&text).unwrap_err();
            assert!(error.contains(problem), "{problem}: {error}");
            assert!(!error.contains(KEY_A), "errors never echo the private key");
        }
    }

    #[test]
    fn the_kill_switch_forwards_only_between_tap_and_tunnel() {
        let rules = kill_switch("qt0123456789ab", Ipv4Addr::new(10, 2, 0, 2));
        assert!(rules.contains("type filter hook forward priority filter; policy drop;"));
        assert!(
            rules.contains("iifname \"qt0123456789ab\" oifname \"wg0\" ip saddr 10.2.0.2 accept")
        );
        assert!(
            rules.contains("iifname \"wg0\" oifname \"qt0123456789ab\" ip daddr 10.2.0.2 accept")
        );
        assert!(!rules.contains("eth0"), "no path to the uplink");
        assert!(rules.contains("iifname \"qt0123456789ab\" drop"));
    }
}

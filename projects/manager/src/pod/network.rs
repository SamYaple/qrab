//! Sanitized read-only guest network snapshot; never inspect the Pod's NICs.
use qapi::qga::{commands::GuestNetworkGetInterfaces, GuestNetworkInterface as QgaInterface};
use qapi_protocol::{qga::Qga, Error, Result};
pub use qrab_guest_agent::proto::{NetworkAddress, NetworkInterface, NetworkStatistics};
use qrab_guest_agent::proto::{
    QrabNetworkGetInterfaces, MAX_NETWORK_ADDRESSES, MAX_NETWORK_INTERFACES,
};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GuestNetworkSnapshot {
    pub observed_unix_ms: u64,
    pub interfaces: Vec<NetworkInterface>,
}

impl GuestNetworkSnapshot {
    /// Queried on the existing guest-agent worker, with its command deadline.
    /// Standard QGA is supported; qrab's extension adds actual interface flags.
    pub(super) async fn query(qga: &mut Qga) -> Result<Self> {
        let interfaces = match qga.execute(&QrabNetworkGetInterfaces {}).await {
            Ok(items) => items,
            Err(Error::Qmp(error))
                if error.class == "CommandNotFound" || error.class == "CommandDisabled" =>
            {
                qga.execute(&GuestNetworkGetInterfaces {})
                    .await?
                    .into_iter()
                    .map(from_qga)
                    .collect()
            }
            Err(error) => return Err(error),
        };
        Ok(Self {
            observed_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            interfaces: sanitize(interfaces),
        })
    }
}

fn from_qga(item: QgaInterface) -> NetworkInterface {
    NetworkInterface {
        name: item.name,
        hardware_address: item.hardware_address,
        ip_addresses: item
            .ip_addresses
            .unwrap_or_default()
            .into_iter()
            .filter_map(|a| {
                Some(NetworkAddress {
                    address: a.ip_address,
                    prefix: u8::try_from(a.prefix).ok()?,
                })
            })
            .collect(),
        statistics: item.statistics.map(|s| NetworkStatistics {
            rx_bytes: s.rx_bytes,
            rx_packets: s.rx_packets,
            rx_errs: s.rx_errs,
            rx_dropped: s.rx_dropped,
            tx_bytes: s.tx_bytes,
            tx_packets: s.tx_packets,
            tx_errs: s.tx_errs,
            tx_dropped: s.tx_dropped,
        }),
        interface_enabled: None,
        link_up: None,
    }
}

/// Guest replies are untrusted input. Bound cardinality/strings and retain
/// only valid IP/prefix pairs, MACs and numeric counters, with no paths.
fn sanitize(items: Vec<NetworkInterface>) -> Vec<NetworkInterface> {
    let mut out = Vec::new();
    for mut item in items.into_iter().take(MAX_NETWORK_INTERFACES) {
        if item.name.is_empty()
            || item.name.len() > 63
            || !item.name.bytes().all(|c| c.is_ascii_graphic() && c != b'/')
        {
            continue;
        }
        if out.iter().any(|i: &NetworkInterface| i.name == item.name) {
            continue;
        }
        item.hardware_address = item.hardware_address.and_then(|mac| {
            let octets = mac.split(':').collect::<Vec<_>>();
            (octets.len() == 6
                && octets
                    .iter()
                    .all(|p| p.len() == 2 && p.bytes().all(|c| c.is_ascii_hexdigit())))
            .then(|| mac.to_ascii_lowercase())
        });
        item.ip_addresses = item
            .ip_addresses
            .into_iter()
            .take(MAX_NETWORK_ADDRESSES)
            .filter_map(|mut a| {
                let ip = a.address.parse::<IpAddr>().ok()?;
                if a.prefix > if ip.is_ipv4() { 32 } else { 128 } {
                    return None;
                }
                a.address = ip.to_string();
                Some(a)
            })
            .collect();
        item.ip_addresses
            .sort_by(|a, b| (&a.address, a.prefix).cmp(&(&b.address, b.prefix)));
        item.ip_addresses.dedup();
        out.push(item);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_guest_inventory_cannot_leak_paths_or_unbounded_strings() {
        let valid = NetworkInterface {
            name: "eth0".into(),
            hardware_address: Some("02:AA:BB:CC:DD:EE".into()),
            ip_addresses: vec![
                NetworkAddress {
                    address: "192.0.2.8".into(),
                    prefix: 24,
                },
                NetworkAddress {
                    address: "192.0.2.9".into(),
                    prefix: 128,
                },
                NetworkAddress {
                    address: "../../secret".into(),
                    prefix: 0,
                },
            ],
            interface_enabled: Some(true),
            link_up: Some(false),
            ..Default::default()
        };
        let items = sanitize(vec![
            valid.clone(),
            valid,
            NetworkInterface {
                name: "/etc/passwd".into(),
                ..Default::default()
            },
        ]);
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].hardware_address.as_deref(),
            Some("02:aa:bb:cc:dd:ee")
        );
        assert_eq!(items[0].ip_addresses.len(), 1);
        assert_eq!(items[0].link_up, Some(false));
        assert!(!serde_json::to_string(&items).unwrap().contains("secret"));
    }
    #[test]
    fn standard_qga_leaves_link_state_unknown() {
        let item = QgaInterface {
            name: "ens3".into(),
            hardware_address: None,
            ip_addresses: None,
            statistics: None,
        };
        let item = from_qga(item);
        assert_eq!(item.interface_enabled, None);
        assert_eq!(item.link_up, None);
        assert!(item.ip_addresses.is_empty());
    }
    #[tokio::test]
    async fn standard_qga_fallback_returns_guest_addresses_and_counters() {
        use qapi_protocol::server::QgaSession;
        use serde_json::json;
        let (host, guest) = tokio::net::UnixStream::pair().unwrap();
        let server = tokio::spawn(async move {
            let mut session = QgaSession::from_stream(guest);
            session.handle_sync().await.unwrap();
            assert_eq!(session.recv_command().await.unwrap().0, "guest-info");
            session
                .send_return(json!({"version":"11.1.3","supported_commands":[]}))
                .await
                .unwrap();
            assert_eq!(
                session.recv_command().await.unwrap().0,
                "qrab-network-get-interfaces"
            );
            session
                .send_error("CommandNotFound", "standard QGA")
                .await
                .unwrap();
            assert_eq!(
                session.recv_command().await.unwrap().0,
                "guest-network-get-interfaces"
            );
            session
                .send_return(
                    json!([{"name":"ens3","hardware-address":"02:AA:BB:CC:DD:EE",
                "ip-addresses":[{"ip-address-type":"ipv4","ip-address":"192.0.2.8","prefix":24}],
                "statistics":{"rx-bytes":123,"rx-packets":1,"rx-errs":0,"rx-dropped":0,
                    "tx-bytes":456,"tx-packets":2,"tx-errs":0,"tx-dropped":0}}]),
                )
                .await
                .unwrap();
        });
        let mut qga = Qga::from_stream(host).await.unwrap();
        let snapshot = GuestNetworkSnapshot::query(&mut qga).await.unwrap();
        assert!(snapshot.observed_unix_ms > 0);
        assert_eq!(
            snapshot.interfaces[0].hardware_address.as_deref(),
            Some("02:aa:bb:cc:dd:ee")
        );
        assert_eq!(snapshot.interfaces[0].ip_addresses[0].address, "192.0.2.8");
        assert_eq!(
            snapshot.interfaces[0].statistics.as_ref().unwrap().rx_bytes,
            123
        );
        assert_eq!(snapshot.interfaces[0].link_up, None);
        server.await.unwrap();
    }
}

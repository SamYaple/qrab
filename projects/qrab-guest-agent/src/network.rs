//! Read-only network inventory in the agent's own network namespace.
use crate::proto::{
    NetworkAddress, NetworkInterface, NetworkStatistics, MAX_NETWORK_ADDRESSES,
    MAX_NETWORK_INTERFACES,
};
use std::{
    collections::BTreeMap,
    ffi::CStr,
    io::{self, Read},
    net::{Ipv4Addr, Ipv6Addr},
};

fn bounded_file(path: impl AsRef<std::path::Path>, limit: u64) -> io::Result<String> {
    let mut text = String::new();
    std::fs::File::open(path)?
        .take(limit + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > limit {
        return Err(io::Error::other("network inventory file exceeds bound"));
    }
    Ok(text)
}

fn counters(text: &str) -> BTreeMap<String, NetworkStatistics> {
    text.lines()
        .filter_map(|line| {
            let (name, values) = line.split_once(':')?;
            let numbers = values
                .split_whitespace()
                .map(str::parse::<u64>)
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            if numbers.len() != 16 {
                return None;
            }
            Some((
                name.trim().to_owned(),
                NetworkStatistics {
                    rx_bytes: numbers[0],
                    rx_packets: numbers[1],
                    rx_errs: numbers[2],
                    rx_dropped: numbers[3],
                    tx_bytes: numbers[8],
                    tx_packets: numbers[9],
                    tx_errs: numbers[10],
                    tx_dropped: numbers[11],
                },
            ))
        })
        .take(MAX_NETWORK_INTERFACES)
        .collect()
}

fn prefix(mask: &[u8]) -> Option<u8> {
    let mut bits = 0;
    let mut zero = false;
    for byte in mask {
        for shift in (0..8).rev() {
            if byte & (1 << shift) != 0 {
                if zero {
                    return None;
                }
                bits += 1;
            } else {
                zero = true;
            }
        }
    }
    Some(bits)
}

struct IfAddrs(*mut libc::ifaddrs);
impl Drop for IfAddrs {
    fn drop(&mut self) {
        // SAFETY: this pointer was obtained from a successful getifaddrs call.
        unsafe { libc::freeifaddrs(self.0) };
    }
}

pub(crate) fn interfaces() -> io::Result<Vec<NetworkInterface>> {
    let mut raw = std::ptr::null_mut();
    // SAFETY: raw points to a writable pointer; the successful list is owned
    // by IfAddrs until all pointed-to addresses have been copied.
    if unsafe { libc::getifaddrs(&mut raw) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let list = IfAddrs(raw);
    let stats = bounded_file("/proc/net/dev", 1024 * 1024)
        .map(|s| counters(&s))
        .unwrap_or_default();
    let mut found = BTreeMap::<String, NetworkInterface>::new();
    let mut cursor = list.0;
    let mut entries = 0;
    while !cursor.is_null() {
        entries += 1;
        if entries > MAX_NETWORK_INTERFACES * (MAX_NETWORK_ADDRESSES + 2) {
            break;
        }
        // SAFETY: cursor is a node of the live getifaddrs list.
        let entry = unsafe { &*cursor };
        cursor = entry.ifa_next;
        if entry.ifa_name.is_null() {
            continue;
        }
        // SAFETY: getifaddrs supplies a NUL-terminated interface name.
        let Ok(name) = unsafe { CStr::from_ptr(entry.ifa_name) }.to_str() else {
            continue;
        };
        if name.is_empty()
            || name.len() >= libc::IF_NAMESIZE
            || !name.bytes().all(|c| c.is_ascii_graphic() && c != b'/')
        {
            continue;
        }
        if !found.contains_key(name) && found.len() == MAX_NETWORK_INTERFACES {
            continue;
        }
        let item = found.entry(name.to_owned()).or_insert_with(|| {
            let state = bounded_file(format!("/sys/class/net/{name}/operstate"), 32).ok();
            NetworkInterface {
                name: name.to_owned(),
                interface_enabled: Some(entry.ifa_flags & libc::IFF_UP as u32 != 0),
                link_up: state.as_deref().and_then(|s| match s.trim() {
                    "up" => Some(true),
                    "down" | "lowerlayerdown" => Some(false),
                    _ => None,
                }),
                statistics: stats.get(name).cloned(),
                ..Default::default()
            }
        });
        if entry.ifa_addr.is_null() {
            continue;
        }
        // SAFETY: the family is present in every sockaddr supplied by getifaddrs.
        let family = unsafe { (*entry.ifa_addr).sa_family } as i32;
        match family {
            libc::AF_PACKET => {
                // SAFETY: AF_PACKET identifies a sockaddr_ll.
                let link = unsafe { &*entry.ifa_addr.cast::<libc::sockaddr_ll>() };
                if link.sll_halen == 6 {
                    item.hardware_address = Some(
                        link.sll_addr[..6]
                            .iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<Vec<_>>()
                            .join(":"),
                    );
                }
            }
            libc::AF_INET | libc::AF_INET6
                if !entry.ifa_netmask.is_null()
                    && item.ip_addresses.len() < MAX_NETWORK_ADDRESSES =>
            {
                let address = if family == libc::AF_INET {
                    // SAFETY: AF_INET identifies sockaddr_in for address/netmask.
                    let addr = unsafe { &*entry.ifa_addr.cast::<libc::sockaddr_in>() };
                    let mask = unsafe { &*entry.ifa_netmask.cast::<libc::sockaddr_in>() };
                    prefix(&mask.sin_addr.s_addr.to_ne_bytes()).map(|prefix| NetworkAddress {
                        address: Ipv4Addr::from(addr.sin_addr.s_addr.to_ne_bytes()).to_string(),
                        prefix,
                    })
                } else {
                    // SAFETY: AF_INET6 identifies sockaddr_in6 for address/netmask.
                    let addr = unsafe { &*entry.ifa_addr.cast::<libc::sockaddr_in6>() };
                    let mask = unsafe { &*entry.ifa_netmask.cast::<libc::sockaddr_in6>() };
                    prefix(&mask.sin6_addr.s6_addr).map(|prefix| NetworkAddress {
                        address: Ipv6Addr::from(addr.sin6_addr.s6_addr).to_string(),
                        prefix,
                    })
                };
                if let Some(address) = address {
                    if !item.ip_addresses.contains(&address) {
                        item.ip_addresses.push(address);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(found.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn masks_and_linux_counters_are_parsed_without_guessing() {
        assert_eq!(prefix(&[255, 255, 255, 0]), Some(24));
        assert_eq!(prefix(&[255, 254, 255, 0]), None);
        assert_eq!(prefix(&[255; 16]), Some(128));
        let stats = counters(
            "Inter-| Receive\n eth0: 10 11 12 13 0 0 0 0 20 21 22 23 0 0 0 0\n bad: 1 nope\n",
        );
        assert_eq!(stats.len(), 1);
        assert_eq!(stats["eth0"].rx_bytes, 10);
        assert_eq!(stats["eth0"].tx_dropped, 23);
    }
    #[test]
    fn linux_inventory_reads_the_callers_network_namespace() {
        let interfaces = interfaces().unwrap();
        let lo = interfaces.iter().find(|n| n.name == "lo").unwrap();
        assert!(lo.ip_addresses.iter().any(|a| a.address == "127.0.0.1"));
        assert!(interfaces.len() <= MAX_NETWORK_INTERFACES);
    }
}

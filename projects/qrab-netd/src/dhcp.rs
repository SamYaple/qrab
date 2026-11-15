//! A static-lease DHCPv4 responder for one tap: it answers only the guest's
//! MAC, always with the guest's /32, the on-link gateway and DNS
//! `169.254.1.1` (the same contract as the NoCloud seed). Guests without
//! cloud-init network config (CirrOS uses DHCP) configure themselves from it.
//!
//! ```
//! use qrab_netd::dhcp::{reply, Lease};
//! let lease = Lease { mac: [2, 0x21, 0xc2, 0x51, 0xb0, 0x38], ip: "10.244.128.7".parse().unwrap(), seconds: 86400 };
//! let discover = qrab_netd::dhcp::request(1, 0x1234_5678, lease.mac);
//! let offer = reply(&discover, &lease).unwrap();
//! assert_eq!(&offer[16..20], &[10, 244, 128, 7]); // yiaddr
//! let stranger = qrab_netd::dhcp::request(1, 1, [2, 0, 0, 0, 0, 9]);
//! assert!(reply(&stranger, &lease).is_none());
//! ```
use crate::GUEST_GATEWAY;
use std::net::Ipv4Addr;

const MAGIC: [u8; 4] = [99, 130, 83, 99];
const HEADER: usize = 236;

/// The one lease a tap's responder hands out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lease {
    pub mac: [u8; 6],
    pub ip: Ipv4Addr,
    pub seconds: u32,
}

/// `xx:xx:xx:xx:xx:xx` as bytes.
pub fn mac(text: &str) -> Option<[u8; 6]> {
    let mut mac = [0u8; 6];
    let mut parts = text.split(':');
    for byte in &mut mac {
        *byte = u8::from_str_radix(parts.next()?, 16).ok()?;
    }
    parts.next().is_none().then_some(mac)
}

fn options(packet: &[u8]) -> impl Iterator<Item = (u8, &[u8])> {
    let mut rest = packet.get(HEADER + 4..).unwrap_or_default();
    std::iter::from_fn(move || loop {
        let (&code, tail) = rest.split_first()?;
        match code {
            0 => rest = tail,
            255 => return None,
            _ => {
                let (&len, tail) = tail.split_first()?;
                let value = tail.get(..usize::from(len))?;
                rest = &tail[usize::from(len)..];
                return Some((code, value));
            }
        }
    })
}

/// The reply to a client message from the leased MAC: OFFER for DISCOVER,
/// ACK for REQUEST. Anything else, or any other MAC, gets nothing.
pub fn reply(request: &[u8], lease: &Lease) -> Option<Vec<u8>> {
    if request.len() < HEADER + 4
        || request[0] != 1
        || request[1] != 1
        || request[2] != 6
        || request[28..34] != lease.mac
        || request[HEADER..HEADER + 4] != MAGIC
    {
        return None;
    }
    let kind = options(request)
        .find(|(code, _)| *code == 53)
        .and_then(|(_, v)| v.first().copied())?;
    let answer = match kind {
        1 => 2, // DISCOVER -> OFFER
        3 => 5, // REQUEST -> ACK
        _ => return None,
    };
    let gateway = GUEST_GATEWAY.octets();
    let mut packet = vec![0u8; HEADER];
    packet[0] = 2; // BOOTREPLY
    packet[1] = 1;
    packet[2] = 6;
    packet[4..8].copy_from_slice(&request[4..8]); // xid
    packet[10..12].copy_from_slice(&request[10..12]); // flags
    packet[16..20].copy_from_slice(&lease.ip.octets()); // yiaddr
    packet[28..44].copy_from_slice(&request[28..44]); // chaddr
    packet.extend(MAGIC);
    let mut option = |code: u8, value: &[u8]| {
        packet.push(code);
        packet.push(u8::try_from(value.len()).expect("short option"));
        packet.extend_from_slice(value);
    };
    option(53, &[answer]);
    option(54, &gateway);
    option(51, &lease.seconds.to_be_bytes());
    option(1, &[255, 255, 255, 255]);
    option(3, &gateway);
    option(6, &gateway);
    // RFC 3442: the gateway itself on-link, then the default route via it.
    let mut routes = vec![32];
    routes.extend(gateway);
    routes.extend([0, 0, 0, 0, 0]);
    routes.extend(gateway);
    option(121, &routes);
    packet.push(255);
    packet.resize(packet.len().max(300), 0);
    Some(packet)
}

/// A minimal client message of `kind` (1 DISCOVER, 3 REQUEST), for tests.
#[doc(hidden)]
pub fn request(kind: u8, xid: u32, mac: [u8; 6]) -> Vec<u8> {
    let mut packet = vec![0u8; HEADER];
    packet[0] = 1;
    packet[1] = 1;
    packet[2] = 6;
    packet[4..8].copy_from_slice(&xid.to_be_bytes());
    packet[10] = 0x80; // broadcast flag
    packet[28..34].copy_from_slice(&mac);
    packet.extend(MAGIC);
    packet.extend([53, 1, kind, 255]);
    packet
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lease() -> Lease {
        Lease {
            mac: mac("02:21:c2:51:b0:38").unwrap(),
            ip: "10.244.128.7".parse().unwrap(),
            seconds: 86400,
        }
    }

    fn option(packet: &[u8], code: u8) -> Option<Vec<u8>> {
        options(packet)
            .find(|(c, _)| *c == code)
            .map(|(_, v)| v.to_vec())
    }

    #[test]
    fn discover_and_request_get_the_static_lease() {
        let lease = lease();
        for (kind, answer) in [(1, 2), (3, 5)] {
            let reply = reply(&request(kind, 0xdead_beef, lease.mac), &lease).unwrap();
            assert_eq!(reply[0], 2);
            assert_eq!(&reply[4..8], &0xdead_beefu32.to_be_bytes(), "xid is echoed");
            assert_eq!(&reply[10..12], &[0x80, 0], "flags are echoed");
            assert_eq!(&reply[16..20], &[10, 244, 128, 7], "yiaddr");
            assert_eq!(&reply[28..34], &lease.mac, "chaddr");
            assert_eq!(option(&reply, 53), Some(vec![answer]));
            assert_eq!(option(&reply, 54), Some(vec![169, 254, 1, 1]));
            assert_eq!(option(&reply, 1), Some(vec![255, 255, 255, 255]));
            assert_eq!(option(&reply, 3), Some(vec![169, 254, 1, 1]));
            assert_eq!(option(&reply, 6), Some(vec![169, 254, 1, 1]));
            assert_eq!(option(&reply, 51), Some(86400u32.to_be_bytes().to_vec()));
            assert_eq!(
                option(&reply, 121),
                Some(vec![32, 169, 254, 1, 1, 0, 0, 0, 0, 0, 169, 254, 1, 1])
            );
            assert!(reply.len() >= 300, "padded to the BOOTP minimum");
        }
    }

    #[test]
    fn other_macs_and_other_messages_get_nothing() {
        let lease = lease();
        assert!(reply(&request(1, 1, [2, 0, 0, 0, 0, 1]), &lease).is_none());
        assert!(
            reply(&request(7, 1, lease.mac), &lease).is_none(),
            "RELEASE"
        );
        let mut server_reply = request(1, 1, lease.mac);
        server_reply[0] = 2;
        assert!(reply(&server_reply, &lease).is_none());
        assert!(reply(&[1; 100], &lease).is_none(), "truncated");
        assert_eq!(mac("02:21:c2:51:b0"), None);
    }

    /// A seeded xorshift64*, so failures reproduce.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n.max(1) as u64) as usize
        }
    }

    /// Every reply is a BOOTREPLY to the lease's MAC carrying the lease's
    /// address, whatever the client sent.
    fn check(packet: &[u8], lease: &Lease) {
        if let Some(answer) = reply(packet, lease) {
            assert_eq!(&packet[28..34], &lease.mac, "answered a foreign MAC");
            assert_eq!(answer[0], 2, "BOOTREPLY");
            assert_eq!(&answer[16..20], &lease.ip.octets(), "yiaddr");
            assert_eq!(&answer[28..34], &lease.mac, "chaddr");
            assert!(matches!(
                option(&answer, 53).as_deref(),
                Some([2]) | Some([5])
            ));
        }
    }

    #[test]
    fn random_and_mutated_packets_never_panic_or_answer_strangers() {
        let lease = lease();
        let mut rng = Rng(0x5eed_d4c9);
        let mut answered = 0;
        for round in 0..50_000 {
            let mut packet = match round % 4 {
                0 => (0..rng.below(700)).map(|_| rng.next() as u8).collect(),
                1 => request(1, rng.next() as u32, lease.mac),
                2 => request(3, rng.next() as u32, lease.mac),
                _ => request(1 + rng.below(8) as u8, 7, [2, 0, 0, 0, 0, rng.next() as u8]),
            };
            if round % 4 != 0 {
                // Corrupt or append option bytes (lengths past the end, no
                // END, pads), sometimes truncate.
                for _ in 0..rng.below(6) {
                    if rng.below(2) == 0 && packet.len() > HEADER + 4 {
                        let at = HEADER + 4 + rng.below(packet.len() - HEADER - 4);
                        packet[at] = rng.next() as u8;
                    } else {
                        packet.push(rng.next() as u8);
                    }
                }
                if rng.below(5) == 0 {
                    packet.truncate(rng.below(packet.len() + 1));
                }
            }
            check(&packet, &lease);
            answered += usize::from(reply(&packet, &lease).is_some());
        }
        assert!(
            answered > 1000,
            "the mutations still exercise answering ({answered})"
        );
    }

    #[test]
    fn hostile_option_lengths_and_padding_are_bounded() {
        let lease = lease();
        let mut base = request(1, 1, lease.mac);
        base.truncate(HEADER + 4);
        // An option whose length runs past the end; only padding; no END;
        // a message-type option of length 0.
        for options in [
            vec![53, 200, 1],
            vec![0; 2000],
            vec![12, 3, b'a', b'b', b'c', 53, 1, 1],
            vec![53, 0, 255],
            vec![255, 53, 1, 1],
        ] {
            let mut packet = base.clone();
            packet.extend(&options);
            check(&packet, &lease);
        }
        let mut no_end = base.clone();
        no_end.extend([12, 3, b'a', b'b', b'c', 53, 1, 1]);
        assert!(reply(&no_end, &lease).is_some(), "END is optional");
        let mut after_end = base;
        after_end.extend([255, 53, 1, 1]);
        assert!(
            reply(&after_end, &lease).is_none(),
            "nothing after END counts"
        );
    }
}

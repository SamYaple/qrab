//! NAT-PMP (RFC 6886) port mapping, as ProtonVPN's P2P servers offer it to
//! a WireGuard client: the client asks the tunnel gateway for a UDP and a TCP
//! mapping with a 60 s lifetime and renews it before it lapses; the server
//! then forwards the mapped public port to the client's tunnel address.
//!
//! ```
//! use qrab_netd::natpmp::{parse, request, Proto};
//! // What Proton documents: `natpmpc -a 1 0 udp 60`.
//! assert_eq!(request(Proto::Udp, 0, 1, 60), [0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 60]);
//! let reply = [0, 129, 0, 0, 0, 0, 0x30, 0x39, 0, 0, 0xa0, 0xf2, 0, 0, 0, 60];
//! let mapping = parse(Proto::Udp, &reply).unwrap();
//! assert_eq!((mapping.external, mapping.lifetime, mapping.epoch), (41202, 60, 12345));
//! ```
use std::{io, net::UdpSocket, time::Duration};

/// The gateway's NAT-PMP port.
pub const PORT: u16 = 5351;
/// Lifetime asked for, as Proton documents.
pub const LIFETIME: u32 = 60;
/// Renew well inside the lifetime, leaving room for a slow round.
pub const RENEW: Duration = Duration::from_secs(40);
/// Proton's documented request: private port 0, suggested public port 1.
pub const INTERNAL: u16 = 0;
pub const SUGGESTED: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Proto {
    Udp = 1,
    Tcp = 2,
}

/// A granted mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mapping {
    pub proto: Proto,
    pub internal: u16,
    pub external: u16,
    pub lifetime: u32,
    /// Seconds since the gateway's mapping table was (re)started.
    pub epoch: u32,
}

/// A map request (RFC 6886 section 3.3).
pub fn request(proto: Proto, internal: u16, external: u16, lifetime: u32) -> [u8; 12] {
    let mut out = [0u8; 12];
    out[1] = proto as u8;
    out[4..6].copy_from_slice(&internal.to_be_bytes());
    out[6..8].copy_from_slice(&external.to_be_bytes());
    out[8..12].copy_from_slice(&lifetime.to_be_bytes());
    out
}

/// Parse the gateway's answer to a `proto` map request.
pub fn parse(proto: Proto, reply: &[u8]) -> Result<Mapping, String> {
    if reply.len() < 16 {
        return Err(format!("short NAT-PMP reply ({} bytes)", reply.len()));
    }
    if reply[0] != 0 {
        return Err(format!("NAT-PMP version {} is not 0", reply[0]));
    }
    if reply[1] != 128 + proto as u8 {
        return Err(format!(
            "NAT-PMP reply to opcode {}, not {}",
            reply[1],
            128 + proto as u8
        ));
    }
    let word = |at: usize| u16::from_be_bytes([reply[at], reply[at + 1]]);
    let long =
        |at: usize| u32::from_be_bytes([reply[at], reply[at + 1], reply[at + 2], reply[at + 3]]);
    let result = word(2);
    if result != 0 {
        let why = match result {
            1 => "unsupported version",
            2 => "not authorized or refused (is this a P2P server, and is port forwarding certified?)",
            3 => "network failure",
            4 => "out of resources",
            5 => "unsupported opcode",
            _ => "unknown result",
        };
        return Err(format!("NAT-PMP refused: {why} ({result})"));
    }
    let mapping = Mapping {
        proto,
        epoch: long(4),
        internal: word(8),
        external: word(10),
        lifetime: long(12),
    };
    if mapping.external == 0 || mapping.lifetime == 0 {
        return Err("NAT-PMP granted no mapping".into());
    }
    Ok(mapping)
}

/// Ask over `socket` (connected to the gateway's [`PORT`]) for a `proto`
/// mapping, retransmitting as RFC 6886 says (250 ms, doubling), four tries.
pub fn map(socket: &UdpSocket, proto: Proto) -> io::Result<Mapping> {
    let ask = request(proto, INTERNAL, SUGGESTED, LIFETIME);
    let mut wait = Duration::from_millis(250);
    let mut buf = [0u8; 64];
    let mut last = None;
    for _ in 0..4 {
        socket.send(&ask)?;
        socket.set_read_timeout(Some(wait))?;
        loop {
            match socket.recv(&mut buf) {
                Ok(n) => match parse(proto, &buf[..n]) {
                    Ok(mapping) => return Ok(mapping),
                    // A stray or stale reply: keep listening this round.
                    Err(error) if error.contains("opcode") => continue,
                    Err(error) => return Err(io::Error::other(error)),
                },
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    last = Some(e);
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        wait *= 2;
    }
    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        format!(
            "no NAT-PMP reply from the tunnel gateway ({})",
            last.map(|e| e.to_string()).unwrap_or_default()
        ),
    ))
}

/// Map UDP and TCP; the forwarded port is the UDP one (Proton grants the
/// same number for both).
pub fn map_both(socket: &UdpSocket) -> Result<u16, String> {
    let udp = map(socket, Proto::Udp).map_err(|e| e.to_string())?;
    let tcp = map(socket, Proto::Tcp).map_err(|e| e.to_string())?;
    if udp.external != tcp.external {
        return Err(format!(
            "the gateway mapped UDP {} but TCP {}",
            udp.external, tcp.external
        ));
    }
    Ok(udp.external)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn reply(op: u8, result: u16, external: u16, lifetime: u32) -> Vec<u8> {
        let mut out = vec![0, op];
        out.extend(result.to_be_bytes());
        out.extend(7u32.to_be_bytes());
        out.extend(0u16.to_be_bytes());
        out.extend(external.to_be_bytes());
        out.extend(lifetime.to_be_bytes());
        out
    }

    #[test]
    fn replies_are_checked_field_by_field() {
        assert!(parse(Proto::Tcp, &reply(130, 0, 5000, 60)).is_ok());
        for (bytes, problem) in [
            (reply(130, 0, 5000, 60)[..15].to_vec(), "short"),
            (reply(129, 0, 5000, 60), "opcode"),
            (reply(130, 2, 5000, 60), "not authorized"),
            (reply(130, 3, 5000, 60), "network failure"),
            (reply(130, 0, 0, 60), "no mapping"),
            (reply(130, 0, 5000, 0), "no mapping"),
            (
                {
                    let mut r = reply(130, 0, 5000, 60);
                    r[0] = 2;
                    r
                },
                "version",
            ),
        ] {
            let error = parse(Proto::Tcp, &bytes).unwrap_err();
            assert!(error.contains(problem), "{problem}: {error}");
        }
    }

    /// A gateway on loopback: answers each request after `drop_first`
    /// unanswered ones, records what it got.
    fn gateway(
        drop_first: usize,
        external: u16,
    ) -> (UdpSocket, std::thread::JoinHandle<Vec<Vec<u8>>>) {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let client = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        client.connect(server.local_addr().unwrap()).unwrap();
        let handle = std::thread::spawn(move || {
            server
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut seen = Vec::new();
            let mut buf = [0u8; 64];
            while let Ok((n, from)) = server.recv_from(&mut buf) {
                seen.push(buf[..n].to_vec());
                if seen.len() <= drop_first {
                    continue;
                }
                let op = 128 + buf[1];
                // A stray reply to the other opcode first: it must be ignored.
                server.send_to(&reply(op ^ 3, 0, 1, 60), from).unwrap();
                server.send_to(&reply(op, 0, external, 60), from).unwrap();
                if buf[1] == Proto::Tcp as u8 {
                    break;
                }
            }
            seen
        });
        (client, handle)
    }

    #[test]
    fn both_protocols_are_mapped_through_lost_requests() {
        let (client, handle) = gateway(2, 41202);
        assert_eq!(map_both(&client), Ok(41202));
        let seen = handle.join().unwrap();
        // Two lost UDP requests, the answered one, then TCP.
        assert_eq!(seen.len(), 4);
        assert_eq!(seen[0], request(Proto::Udp, 0, 1, 60));
        assert_eq!(seen[3], request(Proto::Tcp, 0, 1, 60));
    }

    #[test]
    fn a_silent_gateway_times_out_in_bounded_time() {
        let silent = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let client = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        client.connect(silent.local_addr().unwrap()).unwrap();
        let started = std::time::Instant::now();
        let error = map(&client, Proto::Udp).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        // 250 + 500 + 1000 + 2000 ms.
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
    }
}

//! The VM-facing socket protocol. The manager sends one JSON line naming a
//! NIC and a queue count; the agent answers with one message whose JSON body
//! says what it sends and whose SCM_RIGHTS carry `queues` tap descriptors
//! followed by `queues` vhost-net descriptors, or a refusal with none.
//!
//! ```
//! use qrab_netd::wire::{receive_grant, send_grant, send_refusal, Request};
//! use std::{io::Write, os::fd::OwnedFd, os::unix::net::UnixStream};
//! let (agent, manager) = UnixStream::pair().unwrap();
//! let file = |_| OwnedFd::from(tempfile::tempfile().unwrap());
//! let taps: Vec<OwnedFd> = (0..2).map(file).collect();
//! let vhosts: Vec<OwnedFd> = (0..2).map(file).collect();
//! send_grant(&agent, "qt0123456789ab", "anchor-uid", &taps, &vhosts).unwrap();
//! let grant = receive_grant(&manager).unwrap();
//! assert_eq!((grant.tap.as_str(), grant.taps.len(), grant.vhosts.len()), ("qt0123456789ab", 2, 2));
//! assert_eq!(grant.anchor.as_deref(), Some("anchor-uid"));
//! send_refusal(&agent, "no").unwrap();
//! assert_eq!(receive_grant(&manager).unwrap_err().to_string(), "qrab-netd refused: no");
//! ```
//!
//! Control ops carry no descriptors; `fds` stays the default, so a request
//! without `op` is the original grant request:
//!
//! ```
//! use qrab_netd::wire::{Op, Request};
//! let old: Request = serde_json::from_str(r#"{"nic":"lan","queues":2}"#).unwrap();
//! assert_eq!(old.op, Op::Fds);
//! let forward = Request { nic: "lan".into(), queues: 0, op: Op::Forward, via: Some("10.203.0.12".parse().unwrap()) };
//! assert_eq!(serde_json::to_string(&forward).unwrap(), r#"{"nic":"lan","queues":0,"op":"forward","via":"10.203.0.12"}"#);
//! ```
use serde::{Deserialize, Serialize};
use std::{
    io::{self, IoSlice, IoSliceMut, Read, Write},
    os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
    os::unix::net::UnixStream,
    path::Path,
    time::{Duration, Instant},
};

/// Queues per NIC the agent hands out at most (QEMU queue pairs).
pub const MAX_QUEUES: u8 = 8;
const MAX_MESSAGE: usize = 4096;
/// How long a client has to send its whole request line. A per-read timeout
/// alone would let a peer that dribbles one byte at a time hold an agent
/// thread for hours.
pub const REQUEST_DEADLINE: Duration = Duration::from_secs(5);
/// Longest NIC name accepted (a DNS label; the CRD allows 15).
pub const MAX_NIC_NAME: usize = 63;

/// What a request asks for. `fds` hands out the NIC's queues; the others
/// drive a live-migration handoff of the NIC's anchor on this node
/// (docs/networking.md, "Tap live migration").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Op {
    #[default]
    Fds,
    /// Fence the anchor's endpoint so it can later stand by (Cilium handoff
    /// `prepare`; regenerates, up to a minute).
    Prepare,
    /// Make the anchor's endpoint the IP's active owner on this node.
    Activate,
    /// Make it a standby: not delivered locally, its own traffic dropped.
    Standby,
    /// Route the NIC's address from this node's host to `via` (the
    /// destination node), for traffic still arriving here after the switch.
    Forward,
    /// Remove that route.
    Unforward,
    /// Name the plumbed anchor/namespace incarnation serving this NIC.
    /// The token is opaque: a manager compares it with the queues QEMU
    /// holds and re-attaches when the Pod or its sandbox was replaced.
    Probe,
    /// Pull the NIC's conntrack entries ([`crate::ct`]) from the other
    /// node holding its address and import them here; the reply counts them.
    /// The agents exchange them directly ([`crate::peer`]): a manager never
    /// supplies entries.
    #[serde(rename = "ct-pull")]
    CtPull,
}

/// Largest conntrack reply.
pub const PAYLOAD_LIMIT: usize = 256 << 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub nic: String,
    #[serde(default)]
    pub queues: u8,
    #[serde(default, skip_serializing_if = "is_fds")]
    pub op: Op,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<std::net::Ipv4Addr>,
}

fn is_fds(op: &Op) -> bool {
    *op == Op::Fds
}

#[derive(Debug, Serialize, Deserialize)]
struct Reply {
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tap: Option<String>,
    #[serde(default)]
    queues: u8,
    /// Opaque anchor/namespace token whose tap a grant or probe refers to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    anchor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    imported: Option<usize>,
}

/// A NIC's descriptors, ready for `-netdev tap,fds=…,vhostfds=…`.
#[derive(Debug)]
pub struct Grant {
    pub tap: String,
    /// UID of the anchor Pod the queues belong to (absent from an older agent).
    pub anchor: Option<String>,
    pub taps: Vec<OwnedFd>,
    pub vhosts: Vec<OwnedFd>,
}

/// Read one request line, bounded in size and in total time
/// ([`REQUEST_DEADLINE`]).
pub fn read_request(stream: &UnixStream) -> io::Result<Request> {
    read_request_within(stream, REQUEST_DEADLINE)
}

fn read_request_within(stream: &UnixStream, deadline: Duration) -> io::Result<Request> {
    let start = Instant::now();
    let mut line = Vec::with_capacity(256);
    let mut chunk = [0u8; 512];
    loop {
        let remaining = deadline
            .checked_sub(start.elapsed())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "request deadline elapsed"))?;
        stream.set_read_timeout(Some(remaining))?;
        let n = (&*stream).read(&mut chunk)?;
        if n == 0 {
            break;
        }
        line.extend_from_slice(&chunk[..n]);
        if let Some(end) = line.iter().position(|b| *b == b'\n') {
            line.truncate(end);
            break;
        }
        if line.len() > MAX_MESSAGE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request line too long",
            ));
        }
    }
    parse_request(&line)
}

fn is_nic_name(name: &str) -> bool {
    (1..=MAX_NIC_NAME).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !name.starts_with('-')
        && !name.ends_with('-')
}

/// Decode and validate one request line (without its newline).
///
/// ```
/// use qrab_netd::wire::{parse_request, Op};
/// assert_eq!(parse_request(br#"{"nic":"lan","queues":2}"#).unwrap().op, Op::Fds);
/// assert!(parse_request(br#"{"nic":"lan","queues":9}"#).is_err(), "more than MAX_QUEUES");
/// assert!(parse_request(br#"{"nic":"../x","queues":1}"#).is_err(), "not a DNS label");
/// ```
pub fn parse_request(line: &[u8]) -> io::Result<Request> {
    if line.len() > MAX_MESSAGE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "request line too long",
        ));
    }
    let request: Request = serde_json::from_slice(line.trim_ascii_end())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("bad request: {e}")))?;
    let valid = is_nic_name(&request.nic)
        && match request.op {
            Op::Fds => (1..=MAX_QUEUES).contains(&request.queues) && request.via.is_none(),
            Op::Forward => request.queues == 0 && request.via.is_some(),
            _ => request.queues == 0 && request.via.is_none(),
        };
    if !valid {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("a request names a NIC (a DNS label of at most {MAX_NIC_NAME}); fds takes 1..={MAX_QUEUES} queues, forward takes via, the rest take neither"),
        ));
    }
    Ok(request)
}

fn send(stream: &UnixStream, reply: &Reply, fds: &[RawFd]) -> io::Result<()> {
    let body = serde_json::to_vec(reply)?;
    let iov = [IoSlice::new(&body)];
    let rights = [nix::sys::socket::ControlMessage::ScmRights(fds)];
    let cmsgs: &[_] = if fds.is_empty() { &[] } else { &rights };
    nix::sys::socket::sendmsg::<()>(
        stream.as_raw_fd(),
        &iov,
        cmsgs,
        nix::sys::socket::MsgFlags::empty(),
        None,
    )
    .map_err(io::Error::from)?;
    Ok(())
}

pub fn send_grant(
    stream: &UnixStream,
    tap: &str,
    anchor: &str,
    taps: &[OwnedFd],
    vhosts: &[OwnedFd],
) -> io::Result<()> {
    let queues = u8::try_from(taps.len()).map_err(|_| io::Error::other("too many queues"))?;
    if taps.len() != vhosts.len() || queues == 0 || queues > MAX_QUEUES {
        return Err(io::Error::other(
            "a grant carries 1..=MAX_QUEUES tap and vhost descriptors each",
        ));
    }
    let fds: Vec<RawFd> = taps.iter().chain(vhosts).map(AsRawFd::as_raw_fd).collect();
    send(
        stream,
        &Reply {
            ok: true,
            error: None,
            tap: Some(tap.to_owned()),
            queues,
            anchor: Some(anchor.to_owned()),
            imported: None,
        },
        &fds,
    )
}

/// Answer a `probe` with the serving anchor's UID.
pub fn send_probe(stream: &UnixStream, anchor: &str) -> io::Result<()> {
    send(
        stream,
        &Reply {
            ok: true,
            error: None,
            tap: None,
            queues: 0,
            anchor: Some(anchor.to_owned()),
            imported: None,
        },
        &[],
    )
}

/// Acknowledge a control op (anything but `fds`).
pub fn send_ok(stream: &UnixStream) -> io::Result<()> {
    send(
        stream,
        &Reply {
            ok: true,
            error: None,
            tap: None,
            queues: 0,
            anchor: None,
            imported: None,
        },
        &[],
    )
}

pub fn send_refusal(stream: &UnixStream, error: &str) -> io::Result<()> {
    send(
        stream,
        &Reply {
            ok: false,
            error: Some(error.to_owned()),
            tap: None,
            queues: 0,
            anchor: None,
            imported: None,
        },
        &[],
    )
}

/// Receive the agent's answer to a request.
pub fn receive_grant(stream: &UnixStream) -> io::Result<Grant> {
    let mut body = vec![0u8; MAX_MESSAGE];
    let mut space = nix::cmsg_space!([RawFd; 2 * MAX_QUEUES as usize]);
    let (bytes, mut fds) = {
        let mut iov = [IoSliceMut::new(&mut body)];
        let message = nix::sys::socket::recvmsg::<()>(
            stream.as_raw_fd(),
            &mut iov,
            Some(&mut space),
            nix::sys::socket::MsgFlags::MSG_CMSG_CLOEXEC,
        )
        .map_err(io::Error::from)?;
        let mut fds = Vec::new();
        for cmsg in message.cmsgs().map_err(io::Error::from)? {
            if let nix::sys::socket::ControlMessageOwned::ScmRights(received) = cmsg {
                // SAFETY: the kernel just installed these descriptors for us.
                fds.extend(
                    received
                        .into_iter()
                        .map(|fd| unsafe { OwnedFd::from_raw_fd(fd) }),
                );
            }
        }
        (message.bytes, fds)
    };
    let reply: Reply = serde_json::from_slice(&body[..bytes])
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("bad reply: {e}")))?;
    if !reply.ok {
        return Err(io::Error::other(format!(
            "qrab-netd refused: {}",
            reply.error.unwrap_or_default()
        )));
    }
    let queues = usize::from(reply.queues);
    if queues == 0 || fds.len() != 2 * queues {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "reply and descriptors disagree",
        ));
    }
    let vhosts = fds.split_off(queues);
    Ok(Grant {
        tap: reply.tap.unwrap_or_default(),
        anchor: reply.anchor,
        taps: fds,
        vhosts,
    })
}

fn connect(socket: &Path, request: &Request, timeout: Duration) -> io::Result<UnixStream> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(timeout))?;
    let mut line = serde_json::to_vec(request)?;
    line.push(b'\n');
    stream.write_all(&line)?;
    Ok(stream)
}

/// Ask the agent at `socket` for `nic`'s descriptors (the manager's side).
pub fn request(socket: &Path, nic: &str, queues: u8) -> io::Result<Grant> {
    let request = Request {
        nic: nic.to_owned(),
        queues,
        op: Op::Fds,
        via: None,
    };
    receive_grant(&connect(socket, &request, Duration::from_secs(30))?)
}

/// Run a handoff control op for `nic` (the manager's side). `prepare` may
/// take up to a minute while Cilium regenerates the endpoint.
pub fn control(
    socket: &Path,
    nic: &str,
    op: Op,
    via: Option<std::net::Ipv4Addr>,
) -> io::Result<()> {
    let request = Request {
        nic: nic.to_owned(),
        queues: 0,
        op,
        via,
    };
    let stream = connect(socket, &request, Duration::from_secs(90))?;
    let mut body = vec![0u8; MAX_MESSAGE];
    let n = io::Read::read(&mut &stream, &mut body)?;
    let reply: Reply = serde_json::from_slice(&body[..n])
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("bad reply: {e}")))?;
    if reply.ok {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "qrab-netd refused: {}",
            reply.error.unwrap_or_default()
        )))
    }
}

/// Answer a `ct-pull` with how many entries were written.
pub fn send_imported(stream: &UnixStream, imported: usize) -> io::Result<()> {
    send(
        stream,
        &Reply {
            ok: true,
            error: None,
            tap: None,
            queues: 0,
            anchor: None,
            imported: Some(imported),
        },
        &[],
    )
}

/// Read a whole reply (up to [`PAYLOAD_LIMIT`]) until the agent closes.
fn read_reply(stream: &UnixStream) -> io::Result<Reply> {
    let mut body = Vec::new();
    stream
        .take(PAYLOAD_LIMIT as u64 + 1)
        .read_to_end(&mut body)?;
    if body.len() > PAYLOAD_LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "reply too large",
        ));
    }
    let end = body.iter().position(|b| *b == b'\n').unwrap_or(body.len());
    let reply: Reply = serde_json::from_slice(&body[..end])
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("bad reply: {e}")))?;
    if !reply.ok {
        return Err(io::Error::other(format!(
            "qrab-netd refused: {}",
            reply.error.clone().unwrap_or_default()
        )));
    }
    Ok(reply)
}

/// How long a manager waits for a `ct-pull`: below the managers' 10 s
/// switchover timeout, so the target always answers the source (if only
/// with a failure) before the source stops waiting. A late answer would be
/// read as the reply to the next switchover request.
pub const PULL_DEADLINE: Duration = Duration::from_secs(8);

/// Have this node's agent pull the NIC's conntrack entries from the other
/// node holding its address and import them (the target manager's side),
/// within [`PULL_DEADLINE`]. Returns how many were written.
pub fn conntrack_pull(socket: &Path, nic: &str) -> io::Result<usize> {
    let request = Request {
        nic: nic.to_owned(),
        queues: 0,
        op: Op::CtPull,
        via: None,
    };
    read_reply(&connect(socket, &request, PULL_DEADLINE)?)?
        .imported
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "the reply names no count"))
}

/// Ask which opaque anchor/namespace token serves `nic` on this node now
/// (the manager's side). Refused while that namespace is not plumbed.
pub fn probe(socket: &Path, nic: &str) -> io::Result<String> {
    let request = Request {
        nic: nic.to_owned(),
        queues: 0,
        op: Op::Probe,
        via: None,
    };
    let stream = connect(socket, &request, Duration::from_secs(10))?;
    let mut body = vec![0u8; MAX_MESSAGE];
    let n = io::Read::read(&mut &stream, &mut body)?;
    let reply: Reply = serde_json::from_slice(&body[..n])
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("bad reply: {e}")))?;
    match reply {
        Reply {
            ok: true,
            anchor: Some(anchor),
            ..
        } => Ok(anchor),
        Reply { ok: true, .. } => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "probe reply names no anchor",
        )),
        Reply { error, .. } => Err(io::Error::other(format!(
            "qrab-netd refused: {}",
            error.unwrap_or_default()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_conntrack_pull_names_only_the_nic_and_returns_a_count() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("netd.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let agent = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let request = read_request(&stream).unwrap();
            assert_eq!(request.op, Op::CtPull);
            send_imported(&stream, 7).unwrap();
        });
        assert_eq!(conntrack_pull(&socket, "lan").unwrap(), 7);
        agent.join().unwrap();
        let request = Request {
            nic: "lan".into(),
            queues: 0,
            op: Op::CtPull,
            via: None,
        };
        assert_eq!(
            serde_json::to_string(&request).unwrap(),
            r#"{"nic":"lan","queues":0,"op":"ct-pull"}"#
        );
        // An older manager's ops are unknown now.
        for op in ["ct-export", "ct-import"] {
            let line = format!(r#"{{"nic":"lan","op":"{op}"}}"#);
            assert!(serde_json::from_str::<Request>(&line).is_err(), "{op}");
        }
    }

    /// A seeded xorshift64*, so failures reproduce.
    struct Rng(u64);
    impl Rng {
        fn new(seed: u64) -> Self {
            Self(seed.max(1))
        }
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n.max(1) as u64) as usize
        }
        fn bytes(&mut self, max: usize) -> Vec<u8> {
            (0..self.below(max + 1))
                .map(|_| self.next() as u8)
                .collect()
        }
    }

    #[test]
    fn random_bytes_and_mutated_requests_never_panic_and_stay_bounded() {
        let mut rng = Rng::new(0x5eed_0001);
        let valid = br#"{"nic":"lan","queues":2,"op":"forward","via":"10.203.0.12"}"#;
        for _ in 0..20_000 {
            let line = if rng.below(2) == 0 {
                rng.bytes(300)
            } else {
                let mut line = valid.to_vec();
                for _ in 0..=rng.below(4) {
                    let at = rng.below(line.len());
                    line[at] = rng.next() as u8;
                }
                line
            };
            if let Ok(request) = parse_request(&line) {
                assert!(is_nic_name(&request.nic));
                match request.op {
                    Op::Fds => assert!((1..=MAX_QUEUES).contains(&request.queues)),
                    _ => assert_eq!(request.queues, 0),
                }
            }
        }
    }

    #[test]
    fn out_of_range_and_malformed_fields_are_refused() {
        for line in [
            r#"{"nic":"lan","queues":0}"#,
            r#"{"nic":"lan","queues":9}"#,
            r#"{"nic":"lan","queues":65535}"#,
            r#"{"nic":"lan","queues":-1}"#,
            r#"{"nic":"lan","queues":2.5}"#,
            r#"{"nic":"","queues":1}"#,
            r#"{"queues":1}"#,
            r#"{"nic":"LAN","queues":1}"#,
            r#"{"nic":"-lan","queues":1}"#,
            r#"{"nic":"lan/../x","queues":1}"#,
            r#"{"nic":"l\u0000n","queues":1}"#,
            r#"{"nic":"lan","queues":1,"op":"fds","via":"10.0.0.1"}"#,
            r#"{"nic":"lan","queues":1,"op":"activate"}"#,
            r#"{"nic":"lan","op":"forward"}"#,
            r#"{"nic":"lan","op":"standby","via":"10.0.0.1"}"#,
            r#"{"nic":"lan","op":"reboot"}"#,
            r#"{"nic":"lan","queues":1,"extra":true}"#,
            r#"{"nic":"lan","queues":1}{"nic":"wan","queues":1}"#,
            r#"[]"#,
            "",
        ] {
            assert!(parse_request(line.as_bytes()).is_err(), "{line}");
        }
        let long = format!(r#"{{"nic":"{}","queues":1}}"#, "a".repeat(MAX_NIC_NAME + 1));
        assert!(parse_request(long.as_bytes()).is_err());
        let ok = format!(r#"{{"nic":"{}","queues":8}}"#, "a".repeat(MAX_NIC_NAME));
        assert!(parse_request(ok.as_bytes()).is_ok());
    }

    #[test]
    fn a_dribbling_client_cannot_hold_the_reader_past_the_deadline() {
        let (agent, mut client) = UnixStream::pair().unwrap();
        let writer = std::thread::spawn(move || {
            for byte in br#"{"nic":"lan","queues":1}"# {
                if client.write_all(&[*byte]).is_err() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(40));
            }
        });
        let start = Instant::now();
        let error = read_request_within(&agent, Duration::from_millis(200)).unwrap_err();
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "{:?}",
            start.elapsed()
        );
        assert!(
            matches!(
                error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            ),
            "{error}"
        );
        drop(agent);
        writer.join().unwrap();
    }

    #[test]
    fn an_endless_line_is_refused_at_the_size_bound() {
        let (agent, mut client) = UnixStream::pair().unwrap();
        let writer = std::thread::spawn(move || {
            let _ = client.write_all(&[b'x'; 3 * MAX_MESSAGE]);
        });
        let error = read_request_within(&agent, Duration::from_secs(5)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{error}");
        drop(agent);
        writer.join().unwrap();
    }

    #[test]
    fn a_complete_request_is_read_and_anything_after_its_newline_ignored() {
        let (agent, mut client) = UnixStream::pair().unwrap();
        client
            .write_all(b"{\"nic\":\"lan\",\"queues\":2}\ntrailing")
            .unwrap();
        let request = read_request(&agent).unwrap();
        assert_eq!((request.nic.as_str(), request.queues), ("lan", 2));
    }
}

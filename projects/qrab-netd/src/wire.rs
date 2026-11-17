//! The VM-facing socket protocol. The manager sends one JSON line naming a
//! NIC and a queue count; the agent answers with one JSON line whose body
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
    io::{self, IoSlice, Read, Write},
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
    /// Opaque anchor/namespace token (absent from an older agent).
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
    let mut body = serde_json::to_vec(reply)?;
    if body.len() > MAX_MESSAGE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "reply too large",
        ));
    }
    body.push(b'\n');
    let iov = [IoSlice::new(&body)];
    let rights = [nix::sys::socket::ControlMessage::ScmRights(fds)];
    let cmsgs: &[_] = if fds.is_empty() { &[] } else { &rights };
    let written = nix::sys::socket::sendmsg::<()>(
        stream.as_raw_fd(),
        &iov,
        cmsgs,
        nix::sys::socket::MsgFlags::MSG_NOSIGNAL,
        None,
    )
    .map_err(io::Error::from)?;
    if written == 0 {
        return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "empty reply write",
        ));
    }
    // SCM_RIGHTS accompanies the first successful write only. A short
    // stream write must finish the body without duplicating its descriptors.
    (&*stream).write_all(&body[written..])
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

/// Answer a `probe` with the serving anchor/namespace token.
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

/// Receive one chunk, owning every installed SCM_RIGHTS descriptor before
/// checking truncation. Nix 0.29's cmsgs() refuses MSG_CTRUNC before exposing
/// the installed subset, which would leak that subset on the error path.
fn receive_chunk(stream: &UnixStream, body: &mut [u8]) -> io::Result<(usize, Vec<OwnedFd>)> {
    use nix::libc;
    const CONTROL_BYTES: usize = nix::sys::socket::cmsg_space::<[RawFd; 2 * MAX_QUEUES as usize]>();
    const WORDS: usize = CONTROL_BYTES.div_ceil(std::mem::size_of::<usize>());
    const { assert!(std::mem::align_of::<usize>() >= std::mem::align_of::<libc::cmsghdr>()) };
    let mut control = [0usize; WORDS];
    let mut iov = libc::iovec {
        iov_base: body.as_mut_ptr().cast(),
        iov_len: body.len(),
    };
    // SAFETY: zero is a valid empty msghdr; the live buffers below replace
    // its iovec/control pointers for the duration of recvmsg.
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    message.msg_control = control.as_mut_ptr().cast();
    message.msg_controllen = CONTROL_BYTES;
    // SAFETY: stream and every writable buffer remain valid and correctly
    // sized. CLOEXEC applies atomically to all installed descriptors.
    let bytes = unsafe { libc::recvmsg(stream.as_raw_fd(), &mut message, libc::MSG_CMSG_CLOEXEC) };
    if bytes < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut fds = Vec::new();
    let mut unexpected = false;
    // SAFETY: the control buffer is aligned and contains kernel-validated
    // ancillary headers. On truncation Linux still returns complete FDs in
    // the retained SCM_RIGHTS header; the discarded ones are kernel-closed.
    unsafe {
        let mut header = libc::CMSG_FIRSTHDR(&message);
        while !header.is_null() {
            if (*header).cmsg_level == libc::SOL_SOCKET && (*header).cmsg_type == libc::SCM_RIGHTS {
                let count = ((*header).cmsg_len - libc::CMSG_LEN(0) as usize)
                    / std::mem::size_of::<RawFd>();
                let data = libc::CMSG_DATA(header).cast::<RawFd>();
                for index in 0..count {
                    fds.push(OwnedFd::from_raw_fd(std::ptr::read_unaligned(
                        data.add(index),
                    )));
                }
            } else {
                unexpected = true;
            }
            header = libc::CMSG_NXTHDR(&message, header);
        }
    }
    if unexpected || message.msg_flags & (libc::MSG_CTRUNC | libc::MSG_TRUNC) != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unexpected or truncated reply descriptors",
        ));
    }
    Ok((bytes as usize, fds))
}

fn receive_reply_within(
    stream: &UnixStream,
    deadline: Duration,
) -> io::Result<(Reply, Vec<OwnedFd>)> {
    let start = Instant::now();
    let mut body = Vec::with_capacity(256);
    let mut fds = Vec::new();
    let mut chunk = [0u8; 512];
    loop {
        let remaining = deadline
            .checked_sub(start.elapsed())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "reply deadline elapsed"))?;
        stream.set_read_timeout(Some(remaining))?;
        let (bytes, received) = match receive_chunk(stream, &mut chunk) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        fds.extend(received);
        if fds.len() > 2 * MAX_QUEUES as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "too many reply descriptors",
            ));
        }
        if bytes == 0 {
            // Earlier agents emitted a complete JSON value then closed.
            break;
        }
        body.extend_from_slice(&chunk[..bytes]);
        if let Some(end) = body.iter().position(|b| *b == b'\n') {
            body.truncate(end);
            break;
        }
        if body.len() > MAX_MESSAGE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "reply too large",
            ));
        }
    }
    if body.len() > MAX_MESSAGE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "reply too large",
        ));
    }
    let reply: Reply = serde_json::from_slice(&body)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("bad reply: {e}")))?;
    if !reply.ok {
        return Err(io::Error::other(format!(
            "qrab-netd refused: {}",
            reply.error.unwrap_or_default()
        )));
    }
    Ok((reply, fds))
}

fn reply_deadline(stream: &UnixStream) -> io::Result<Duration> {
    Ok(stream.read_timeout()?.unwrap_or(Duration::from_secs(30)))
}

/// Receive the complete agent reply, with bounded time/bytes/descriptors.
pub fn receive_grant(stream: &UnixStream) -> io::Result<Grant> {
    let (reply, mut fds) = receive_reply_within(stream, reply_deadline(stream)?)?;
    let queues = usize::from(reply.queues);
    if queues == 0 || queues > usize::from(MAX_QUEUES) || fds.len() != 2 * queues {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "reply and descriptors disagree",
        ));
    }
    let vhosts = fds.split_off(queues);
    Ok(Grant {
        tap: reply
            .tap
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "reply names no tap"))?,
        anchor: reply.anchor,
        taps: fds,
        vhosts,
    })
}

fn connect(socket: &Path, request: &Request, timeout: Duration) -> io::Result<UnixStream> {
    use nix::sys::socket::{AddressFamily, SockFlag, SockType, UnixAddr};
    // A blocking Unix connect can wait indefinitely on a full listen queue,
    // before socket read/write timeouts can help. Refuse a busy agent at once;
    // callers already retry unavailable attachment/control requests.
    let fd = nix::sys::socket::socket(
        AddressFamily::Unix,
        SockType::Stream,
        SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
        None,
    )
    .map_err(io::Error::from)?;
    nix::sys::socket::connect(
        fd.as_raw_fd(),
        &UnixAddr::new(socket).map_err(io::Error::from)?,
    )
    .map_err(io::Error::from)?;
    let mut stream = UnixStream::from(fd);
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
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
    read_reply(&connect(socket, &request, Duration::from_secs(90))?)?;
    Ok(())
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

/// Read a complete descriptor-free reply, honoring the total read deadline.
fn read_reply(stream: &UnixStream) -> io::Result<Reply> {
    let (reply, fds) = receive_reply_within(stream, reply_deadline(stream)?)?;
    if !fds.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "control reply carries descriptors",
        ));
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
    let reply = read_reply(&stream)?;
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

    fn send_bytes_with_fds(stream: &UnixStream, bytes: &[u8], fds: &[RawFd]) {
        let rights = [nix::sys::socket::ControlMessage::ScmRights(fds)];
        let sent = nix::sys::socket::sendmsg::<()>(
            stream.as_raw_fd(),
            &[IoSlice::new(bytes)],
            &rights,
            nix::sys::socket::MsgFlags::empty(),
            None,
        )
        .unwrap();
        assert_eq!(sent, bytes.len());
    }

    #[test]
    fn grant_fragmentation_keeps_descriptors_until_the_complete_reply() {
        let (agent, manager) = UnixStream::pair().unwrap();
        let worker = std::thread::spawn(move || {
            let files: Vec<OwnedFd> = (0..2)
                .map(|_| tempfile::tempfile().unwrap().into())
                .collect();
            let raw: Vec<_> = files.iter().map(AsRawFd::as_raw_fd).collect();
            send_bytes_with_fds(&agent, b"{\"ok\":", &raw);
            std::thread::sleep(Duration::from_millis(30));
            (&agent)
                .write_all(
                    b"true,\"tap\":\"qt0123456789ab\",\"queues\":1,\"anchor\":\"pod@1:7\"}\n",
                )
                .unwrap();
        });
        let result = receive_grant(&manager);
        worker.join().unwrap();
        let grant = result.unwrap();
        assert_eq!(grant.anchor.as_deref(), Some("pod@1:7"));
        assert_eq!((grant.taps.len(), grant.vhosts.len()), (1, 1));
        for fd in grant.taps.iter().chain(&grant.vhosts) {
            let flags = nix::fcntl::fcntl(fd.as_raw_fd(), nix::fcntl::FcntlArg::F_GETFD).unwrap();
            assert_ne!(flags & nix::libc::FD_CLOEXEC, 0);
        }
    }

    #[test]
    fn truncated_ancillary_descriptors_are_refused_and_closed() {
        let (agent, manager) = UnixStream::pair().unwrap();
        let (mut witness, endpoint) = UnixStream::pair().unwrap();
        witness
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let files: Vec<OwnedFd> = (0..2 * MAX_QUEUES + 1)
            .map(|_| endpoint.try_clone().unwrap().into())
            .collect();
        let raw: Vec<_> = files.iter().map(AsRawFd::as_raw_fd).collect();
        send_bytes_with_fds(
            &agent,
            b"{\"ok\":true,\"tap\":\"qt0\",\"queues\":8}\n",
            &raw,
        );
        drop((files, endpoint, agent));
        assert!(receive_grant(&manager).is_err());
        // EOF proves that every installed duplicate was closed. Counting
        // process-wide FDs would race the other concurrently running tests.
        assert_eq!(witness.read(&mut [0u8; 1]).unwrap(), 0);
    }

    #[test]
    fn probes_controls_and_conntrack_counts_accept_fragmented_replies() {
        use std::os::unix::net::UnixListener;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("netd.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let worker = std::thread::spawn(move || {
            for op in [Op::Probe, Op::Prepare, Op::CtPull] {
                let (mut stream, _) = listener.accept().unwrap();
                assert_eq!(read_request(&stream).unwrap().op, op);
                stream.write_all(b"{\"ok\":").unwrap();
                std::thread::sleep(Duration::from_millis(20));
                stream
                    .write_all(b"true,\"anchor\":\"pod@1:7\",\"imported\":12}\n")
                    .unwrap();
            }
        });
        assert_eq!(probe(&path, "lan").unwrap(), "pod@1:7");
        control(&path, "lan", Op::Prepare, None).unwrap();
        assert_eq!(conntrack_pull(&path, "lan").unwrap(), 12);
        worker.join().unwrap();
    }

    #[test]
    fn grant_rights_can_arrive_with_a_later_stream_fragment() {
        let (mut agent, manager) = UnixStream::pair().unwrap();
        let worker = std::thread::spawn(move || {
            agent.write_all(b"{\"ok\":").unwrap();
            std::thread::sleep(Duration::from_millis(20));
            let files: Vec<OwnedFd> = (0..2)
                .map(|_| tempfile::tempfile().unwrap().into())
                .collect();
            let raw: Vec<_> = files.iter().map(AsRawFd::as_raw_fd).collect();
            send_bytes_with_fds(&agent, b"true,\"tap\":\"qt0\",\"queues\":1}\n", &raw);
        });
        let grant = receive_grant(&manager).unwrap();
        worker.join().unwrap();
        assert_eq!((grant.taps.len(), grant.vhosts.len()), (1, 1));
    }

    #[test]
    fn an_old_unterminated_reply_is_accepted_only_when_complete_at_eof() {
        for (body, good) in [
            (br#"{"ok":true,"imported":7}"#.as_slice(), true),
            (br#"{"ok":true,"imported":"#.as_slice(), false),
        ] {
            let (mut agent, manager) = UnixStream::pair().unwrap();
            agent.write_all(body).unwrap();
            drop(agent);
            let result = read_reply(&manager);
            if good {
                assert_eq!(result.unwrap().imported, Some(7));
            } else {
                assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
            }
        }
    }

    #[test]
    fn malformed_and_oversized_grants_drop_received_descriptors() {
        let oversized = vec![b'x'; MAX_MESSAGE + 1];
        for body in [
            b"not-json\n".as_slice(),
            b"{\"ok\":false,\"error\":\"no\"}\n".as_slice(),
            b"{\"ok\":true,\"tap\":\"qt0\",\"queues\":8}\n".as_slice(),
            b"{\"ok\":true,\"queues\":1}\n".as_slice(),
            oversized.as_slice(),
        ] {
            let (agent, manager) = UnixStream::pair().unwrap();
            let (mut witness, endpoint) = UnixStream::pair().unwrap();
            witness
                .set_read_timeout(Some(Duration::from_millis(200)))
                .unwrap();
            send_bytes_with_fds(&agent, body, &[endpoint.as_raw_fd(), endpoint.as_raw_fd()]);
            drop((agent, endpoint));
            assert!(receive_grant(&manager).is_err());
            assert_eq!(witness.read(&mut [0u8; 1]).unwrap(), 0);
        }
    }

    #[test]
    fn descriptor_free_replies_refuse_and_close_unexpected_rights() {
        let (agent, manager) = UnixStream::pair().unwrap();
        let (mut witness, endpoint) = UnixStream::pair().unwrap();
        witness
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        send_bytes_with_fds(&agent, b"{\"ok\":true}\n", &[endpoint.as_raw_fd()]);
        drop((agent, endpoint));
        assert_eq!(
            read_reply(&manager).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(witness.read(&mut [0u8; 1]).unwrap(), 0);
    }

    #[test]
    fn a_dribbling_reply_cannot_extend_the_total_deadline() {
        let (mut agent, manager) = UnixStream::pair().unwrap();
        let (mut witness, endpoint) = UnixStream::pair().unwrap();
        witness
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let worker = std::thread::spawn(move || {
            send_bytes_with_fds(&agent, b"{", &[endpoint.as_raw_fd()]);
            drop(endpoint);
            for byte in b"\"ok\":true,\"tap\":\"qt0\",\"queues\":1}\n" {
                std::thread::sleep(Duration::from_millis(40));
                if agent.write_all(&[*byte]).is_err() {
                    return;
                }
            }
        });
        let start = Instant::now();
        let error = receive_reply_within(&manager, Duration::from_millis(200)).unwrap_err();
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(matches!(
            error.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
        ));
        assert_eq!(witness.read(&mut [0u8; 1]).unwrap(), 0);
        drop(manager);
        worker.join().unwrap();
    }

    #[test]
    fn a_full_agent_listen_queue_is_a_bounded_retryable_error() {
        use nix::sys::socket::{AddressFamily, Backlog, SockFlag, SockType, UnixAddr};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("busy.sock");
        let listener = nix::sys::socket::socket(
            AddressFamily::Unix,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC,
            None,
        )
        .unwrap();
        let address = UnixAddr::new(&path).unwrap();
        nix::sys::socket::bind(listener.as_raw_fd(), &address).unwrap();
        nix::sys::socket::listen(&listener, Backlog::new(0).unwrap()).unwrap();
        let occupied = UnixStream::connect(&path).unwrap();
        let (sent, result) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            sent.send(
                connect(
                    &path,
                    &Request {
                        nic: "lan".into(),
                        queues: 0,
                        op: Op::Probe,
                        via: None,
                    },
                    Duration::from_millis(100),
                )
                .map(|_| ())
                .map_err(|e| e.kind()),
            )
            .unwrap();
        });
        let answer = result.recv_timeout(Duration::from_secs(1));
        // Release even a broken blocking implementation before failing.
        drop((listener, occupied));
        worker.join().unwrap();
        assert_eq!(answer.unwrap(), Err(io::ErrorKind::WouldBlock));
    }

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

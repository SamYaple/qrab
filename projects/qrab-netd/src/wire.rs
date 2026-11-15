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
//! send_grant(&agent, "qt0123456789ab", &taps, &vhosts).unwrap();
//! let grant = receive_grant(&manager).unwrap();
//! assert_eq!((grant.tap.as_str(), grant.taps.len(), grant.vhosts.len()), ("qt0123456789ab", 2, 2));
//! send_refusal(&agent, "no").unwrap();
//! assert_eq!(receive_grant(&manager).unwrap_err().to_string(), "qrab-netd refused: no");
//! ```
use serde::{Deserialize, Serialize};
use std::{
    io::{self, BufRead, BufReader, IoSlice, IoSliceMut, Write},
    os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
    os::unix::net::UnixStream,
    path::Path,
    time::Duration,
};

/// Queues per NIC the agent hands out at most (QEMU queue pairs).
pub const MAX_QUEUES: u8 = 8;
const MAX_MESSAGE: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub nic: String,
    pub queues: u8,
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
}

/// A NIC's descriptors, ready for `-netdev tap,fds=…,vhostfds=…`.
#[derive(Debug)]
pub struct Grant {
    pub tap: String,
    pub taps: Vec<OwnedFd>,
    pub vhosts: Vec<OwnedFd>,
}

/// Read one request line, bounded in size.
pub fn read_request(stream: &UnixStream) -> io::Result<Request> {
    let mut line = String::new();
    BufReader::new(io::Read::take(stream, MAX_MESSAGE as u64)).read_line(&mut line)?;
    let request: Request = serde_json::from_str(line.trim_end())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("bad request: {e}")))?;
    if request.nic.is_empty() || !(1..=MAX_QUEUES).contains(&request.queues) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("a request names a NIC and 1..={MAX_QUEUES} queues"),
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
        },
        &fds,
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
        taps: fds,
        vhosts,
    })
}

/// Ask the agent at `socket` for `nic`'s descriptors (the manager's side).
pub fn request(socket: &Path, nic: &str, queues: u8) -> io::Result<Grant> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    let mut line = serde_json::to_vec(&Request {
        nic: nic.to_owned(),
        queues,
    })?;
    line.push(b'\n');
    stream.write_all(&line)?;
    receive_grant(&stream)
}

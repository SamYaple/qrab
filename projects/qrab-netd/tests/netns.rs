//! Real kernel check in throwaway network namespaces: the plan applied to a
//! stand-in anchor, and a tap queue passed over the wire protocol to a
//! process in another namespace still carries frames. Needs root
//! (CAP_SYS_ADMIN, CAP_NET_ADMIN), `ip`, /dev/net/tun and /dev/vhost-net;
//! skipped with a printed reason otherwise.
use qrab_netd::{identity, plan::NicPlan, sys, wire};
use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    os::unix::net::UnixStream,
    path::PathBuf,
    sync::mpsc,
    time::Duration,
};

/// A network namespace kept alive by a parked thread.
struct Netns {
    path: PathBuf,
    done: mpsc::Sender<()>,
}

impl Netns {
    fn new() -> Self {
        let (ready, path) = mpsc::channel();
        let (done, wait) = mpsc::channel::<()>();
        std::thread::spawn(move || {
            nix::sched::unshare(nix::sched::CloneFlags::CLONE_NEWNET).expect("unshare");
            // SAFETY: gettid has no preconditions.
            let tid = unsafe { libc::syscall(libc::SYS_gettid) };
            ready
                .send(PathBuf::from(format!("/proc/self/task/{tid}/ns/net")))
                .unwrap();
            let _ = wait.recv();
        });
        Self {
            path: path.recv().unwrap(),
            done,
        }
    }
}

impl Drop for Netns {
    fn drop(&mut self) {
        let _ = self.done.send(());
    }
}

fn ip(words: &str) -> io::Result<String> {
    sys::ip(&words.split(' ').map(str::to_owned).collect::<Vec<_>>())
}

fn packet_socket(ifname: &str) -> io::Result<OwnedFd> {
    let all = (libc::ETH_P_ALL as u16).to_be();
    // SAFETY: plain socket(2); the result is checked and owned immediately.
    let fd = unsafe { libc::socket(libc::AF_PACKET, libc::SOCK_RAW, i32::from(all)) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fd is a fresh descriptor we own.
    let socket = unsafe { OwnedFd::from_raw_fd(fd) };
    let name = std::ffi::CString::new(ifname).unwrap();
    // SAFETY: valid NUL-terminated name.
    let index = unsafe { libc::if_nametoindex(name.as_ptr()) };
    // SAFETY: sockaddr_ll is plain data; zero is a valid starting value.
    let mut address: libc::sockaddr_ll = unsafe { std::mem::zeroed() };
    address.sll_family = libc::AF_PACKET as u16;
    address.sll_protocol = all;
    address.sll_ifindex = index as i32;
    let timeout = libc::timeval {
        tv_sec: 2,
        tv_usec: 0,
    };
    // SAFETY: pointers and lengths describe live, correctly sized values.
    unsafe {
        if libc::bind(
            fd,
            (&address as *const libc::sockaddr_ll).cast(),
            std::mem::size_of_val(&address) as u32,
        ) < 0
            || libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                (&timeout as *const libc::timeval).cast(),
                std::mem::size_of_val(&timeout) as u32,
            ) < 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(socket)
}

/// Wait for a frame carrying `marker` on a packet socket.
fn saw(socket: &OwnedFd, marker: &[u8]) -> bool {
    let mut buffer = [0u8; 2048];
    for _ in 0..50 {
        // SAFETY: buffer is valid for its length.
        let n = unsafe {
            libc::recv(
                socket.as_raw_fd(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                0,
            )
        };
        if n <= 0 {
            return false;
        }
        if buffer[..n as usize]
            .windows(marker.len())
            .any(|w| w == marker)
        {
            return true;
        }
    }
    false
}

/// Write one Ethernet frame (after the 10-byte virtio-net header the tap
/// expects with IFF_VNET_HDR) into a tap queue.
fn send_frame(queue: &OwnedFd, source_mac: &str, marker: &[u8]) {
    let mut frame = vec![0u8; 10];
    frame.extend([0xff; 6]);
    frame.extend(
        source_mac
            .split(':')
            .map(|b| u8::from_str_radix(b, 16).unwrap()),
    );
    frame.extend([0x88, 0xb5]);
    frame.extend(marker);
    frame.resize(frame.len().max(10 + 60), 0);
    // SAFETY: frame is valid for its length.
    let n = unsafe { libc::write(queue.as_raw_fd(), frame.as_ptr().cast(), frame.len()) };
    assert_eq!(
        n,
        frame.len() as isize,
        "write to tap queue: {}",
        io::Error::last_os_error()
    );
}

#[test]
fn a_tap_built_in_one_namespace_works_from_another() {
    // SAFETY: geteuid has no preconditions.
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("skipped: needs root to create network namespaces and taps");
        return;
    }
    if let Err(reason) = sys::check_devices() {
        eprintln!("skipped: {reason}");
        return;
    }
    let anchor = Netns::new();
    let uid = "6b9f0c44-0000-4000-8000-0000000000aa";
    let plan = NicPlan {
        tap: identity::tap_name(uid, "lan"),
        tap_mac: identity::tap_mac(uid, "lan"),
        guest_mac: identity::guest_mac(uid, "lan"),
        guest_ip: "10.99.0.7".parse().unwrap(),
    };
    let check = plan.clone();
    let packets = sys::in_netns(&anchor.path, move || {
        // A stand-in for the anchor's Cilium veth: eth0 holding the /32.
        ip("link set lo up")?;
        ip("link add eth0 mtu 1400 type dummy")?;
        ip("addr add 10.99.0.7/32 dev eth0")?;
        ip("link set eth0 up")?;
        // Cilium's Pod routes: the gateway on-link, then the default via it.
        ip("route add 10.99.9.1 dev eth0 scope link")?;
        ip("route add default via 10.99.9.1 dev eth0")?;
        sys::apply(&check, None)?;
        // Releasing the /32 must not flush the anchor's own routes.
        assert!(
            ip("-4 route show default")?.contains("via 10.99.9.1 dev eth0"),
            "the default route survives"
        );
        assert!(
            !ip("-o -4 addr show dev eth0")?.contains("10.99.0.7"),
            "the /32 left eth0"
        );
        assert!(
            ip("route get 10.99.0.7")?.contains(&format!("dev {}", check.tap)),
            "the /32 routes to the tap"
        );
        assert!(
            ip(&format!("-o link show dev {}", check.tap))?.contains("mtu 1400"),
            "the tap takes eth0's MTU"
        );
        let neighbour = ip(&format!("neigh show dev {}", check.tap))?;
        assert!(
            neighbour.contains(&check.guest_mac) && neighbour.contains("PERMANENT"),
            "{neighbour}"
        );
        assert!(ip(&format!("-o -4 addr show dev {}", check.tap))?.contains("169.254.1.1/32"));
        assert_eq!(
            std::fs::read_to_string("/proc/sys/net/ipv4/ip_forward")?.trim(),
            "1"
        );
        // Applying a finished plan again is a no-op.
        sys::apply(&check, None)?;
        packet_socket(&check.tap)
    })
    .unwrap();

    let (tap, queues) = (plan.tap.clone(), 2);
    let taps = sys::in_netns(&anchor.path, move || sys::open_tap_queues(&tap, queues)).unwrap();
    let vhosts = sys::open_vhost(queues).unwrap();
    let (agent, manager) = UnixStream::pair().unwrap();
    wire::send_grant(&agent, &plan.tap, "anchor-uid", &taps, &vhosts).unwrap();
    drop((taps, vhosts));

    let (guest_mac, marker) = (plan.guest_mac.clone(), b"qrab-netd-test-frame".to_vec());
    let elsewhere = std::thread::spawn(move || {
        nix::sched::unshare(nix::sched::CloneFlags::CLONE_NEWNET).unwrap();
        let grant = wire::receive_grant(&manager).unwrap();
        assert_eq!((grant.taps.len(), grant.vhosts.len()), (2, 2));
        send_frame(&grant.taps[0], &guest_mac, &marker);
        grant
    });
    let grant = elsewhere.join().unwrap();
    assert!(
        saw(&packets, b"qrab-netd-test-frame"),
        "a frame written in another namespace reached the tap"
    );

    // The queues are held, and re-applying (an agent restart) leaves them working.
    assert!(sys::tap_holders(std::path::Path::new("/proc"), &plan.tap)
        .contains(&(std::process::id() as i32)));
    let again = plan.clone();
    sys::in_netns(&anchor.path, move || sys::apply(&again, None)).unwrap();
    send_frame(&grant.taps[1], &plan.guest_mac, b"after-reapply");
    assert!(saw(&packets, b"after-reapply"), "queues survive a re-apply");
    std::thread::sleep(Duration::from_millis(10));
}

#[test]
fn forward_routes_are_marked_listed_and_removed() {
    // SAFETY: geteuid has no preconditions.
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("skipped: needs root to create network namespaces");
        return;
    }
    let host = Netns::new();
    sys::in_netns(&host.path, || {
        ip("link add peer0 type dummy")?;
        ip("addr add 10.99.1.1/24 dev peer0")?;
        ip("link set peer0 up")?;
        let (guest, via) = ("10.99.0.50".parse().unwrap(), "10.99.1.2".parse().unwrap());
        sys::forward(guest, via)?;
        sys::forward(guest, via)?; // idempotent
        assert_eq!(sys::forwards()?, vec![guest]);
        assert!(ip("route get 10.99.0.50")?.contains("via 10.99.1.2"));
        // Other routes are never ours.
        ip("route add 10.99.0.60/32 via 10.99.1.2")?;
        assert_eq!(sys::forwards()?, vec![guest]);
        sys::unforward(guest)?;
        sys::unforward(guest)?; // already gone is fine
        assert!(sys::forwards()?.is_empty());
        assert!(ip("route get 10.99.0.60")?.contains("via 10.99.1.2"));
        Ok(())
    })
    .unwrap();
}

#[test]
fn the_dhcp_responder_offers_the_lease_over_a_real_link() {
    // SAFETY: geteuid has no preconditions.
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("skipped: needs root to create network namespaces");
        return;
    }
    let anchor = Netns::new();
    let guest = Netns::new();
    // `ip ... netns <pid>` resolves /proc/<tid>/ns/net for the parked thread.
    let peer_ns = guest
        .path
        .to_str()
        .unwrap()
        .split('/')
        .nth(4)
        .unwrap()
        .to_owned();
    sys::in_netns(&anchor.path, move || {
        ip(&format!(
            "link add qtdhcp0 type veth peer name gst0 netns {peer_ns}"
        ))?;
        ip("link set qtdhcp0 up")?;
        // As on a plumbed anchor (the stand-in client sends from 10.99.0.99).
        for key in ["all", "qtdhcp0"] {
            std::fs::write(format!("/proc/sys/net/ipv4/conf/{key}/rp_filter"), "0")?;
        }
        ip("addr add 169.254.1.1/32 dev qtdhcp0")
    })
    .unwrap();
    let lease = qrab_netd::dhcp::Lease {
        mac: [2, 0, 0, 0, 0, 0x42],
        ip: "10.99.0.42".parse().unwrap(),
        seconds: 600,
    };
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    sys::serve_dhcp(anchor.path.clone(), "qtdhcp0".into(), lease, stop.clone()).unwrap();
    let reply = sys::in_netns(&guest.path, move || {
        ip("link set gst0 address 02:00:00:00:00:42 up")?;
        // A real client sends from 0.0.0.0 on a raw socket; this UDP stand-in
        // needs an address and a broadcast route to send at all.
        ip("addr add 10.99.0.99/32 dev gst0")?;
        ip("route add 255.255.255.255/32 dev gst0")?;
        for key in ["all", "gst0"] {
            std::fs::write(format!("/proc/sys/net/ipv4/conf/{key}/rp_filter"), "0")?;
        }
        let socket = std::net::UdpSocket::bind(("0.0.0.0", 68))?;
        nix::sys::socket::setsockopt(
            &socket,
            nix::sys::socket::sockopt::BindToDevice,
            &std::ffi::OsString::from("gst0"),
        )
        .map_err(io::Error::from)?;
        socket.set_broadcast(true)?;
        socket.set_read_timeout(Some(Duration::from_secs(2)))?;
        let mut buffer = [0u8; 1500];
        for _ in 0..5 {
            socket.send_to(
                &qrab_netd::dhcp::request(1, 7, lease.mac),
                ("255.255.255.255", 67),
            )?;
            if let Ok((n, from)) = socket.recv_from(&mut buffer) {
                return Ok((buffer[..n].to_vec(), from));
            }
        }
        Err(io::Error::other("no DHCP reply"))
    })
    .unwrap();
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let (offer, from) = reply;
    assert_eq!(from.ip().to_string(), "169.254.1.1");
    assert_eq!(&offer[16..20], &[10, 99, 0, 42], "yiaddr is the lease");
}

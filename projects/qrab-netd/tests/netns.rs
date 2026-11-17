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
        vpn: None,
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
        assert!(sys::forwards()?.is_empty(), "no table yet");
        sys::forward(guest, via)?;
        sys::forward(guest, via)?; // idempotent
        assert_eq!(sys::forwards()?, vec![guest]);
        assert!(ip("route get 10.99.0.50")?.contains("via 10.99.1.2"));
        assert_eq!(
            ip("-4 rule show priority 200")?
                .matches("lookup 252")
                .count(),
            1
        );
        // A main-table route for the same /32 (Cilium's pool route, written
        // after ours) does not take the address back.
        ip("route replace 10.99.0.50/32 dev peer0")?;
        assert!(ip("route get 10.99.0.50")?.contains("via 10.99.1.2"));
        ip("route del 10.99.0.50/32 dev peer0")?;
        // An older agent's main-table forward is listed, and replaced.
        ip("route add 10.99.0.70/32 via 10.99.1.2 proto 252")?;
        assert_eq!(sys::forwards()?.len(), 2);
        sys::forward("10.99.0.70".parse().unwrap(), via)?;
        assert!(!ip("route show proto 252")?.contains("10.99.0.70"));
        sys::unforward("10.99.0.70".parse().unwrap())?;
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
        dns: Some(qrab_netd::GUEST_GATEWAY),
        mtu: None,
    };
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    sys::serve_dhcp(
        std::fs::File::open(&anchor.path).unwrap(),
        "qtdhcp0".into(),
        lease,
        stop.clone(),
    )
    .unwrap();
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

/// One's-complement sum over 16-bit words (IPv4 and ICMP checksums).
fn checksum(bytes: &[u8]) -> u16 {
    let mut sum: u32 = bytes
        .chunks(2)
        .map(|c| u32::from(u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)])))
        .sum();
    while sum > 0xffff {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

/// An ICMP echo request `src -> dst` as the guest would send it to its
/// gateway's MAC, behind the 10-byte virtio-net header.
fn icmp_frame(dst_mac: &str, src_mac: &str, src: [u8; 4], dst: [u8; 4], marker: &[u8]) -> Vec<u8> {
    let mac = |m: &str| {
        m.split(':')
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect::<Vec<_>>()
    };
    let mut icmp = vec![8, 0, 0, 0, 0x51, 0x52, 0, 1];
    icmp.extend(marker);
    let sum = checksum(&icmp).to_be_bytes();
    icmp[2..4].copy_from_slice(&sum);
    let mut ipv4 = vec![0x45, 0, 0, 0, 0, 0, 0x40, 0, 64, 1, 0, 0];
    ipv4[2..4].copy_from_slice(&((20 + icmp.len()) as u16).to_be_bytes());
    ipv4.extend(src);
    ipv4.extend(dst);
    let sum = checksum(&ipv4).to_be_bytes();
    ipv4[10..12].copy_from_slice(&sum);
    let mut frame = vec![0u8; 10];
    frame.extend(mac(dst_mac));
    frame.extend(mac(src_mac));
    frame.extend([0x08, 0x00]);
    frame.extend(ipv4);
    frame.extend(icmp);
    frame
}

/// Read frames from a tap queue until one carries `marker` (or ~2 s pass).
fn queue_saw(queue: &OwnedFd, marker: &[u8]) -> bool {
    let mut buffer = [0u8; 2048];
    for _ in 0..40 {
        let mut poll = libc::pollfd {
            fd: queue.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd.
        if unsafe { libc::poll(&mut poll, 1, 50) } <= 0 {
            continue;
        }
        // SAFETY: buffer is valid for its length.
        let n = unsafe { libc::read(queue.as_raw_fd(), buffer.as_mut_ptr().cast(), buffer.len()) };
        if n > 0
            && buffer[..n as usize]
                .windows(marker.len())
                .any(|w| w == marker)
        {
            return true;
        }
    }
    false
}

fn wg(words: &str, stdin: Option<&str>) -> io::Result<String> {
    use std::io::Write;
    let mut child = std::process::Command::new("wg")
        .args(words.split(' '))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.unwrap_or("").as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(io::Error::other(format!("wg {words} failed")));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

#[test]
fn a_vpn_tap_reaches_only_its_tunnel() {
    // SAFETY: geteuid has no preconditions.
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("skipped: needs root to create network namespaces and taps");
        return;
    }
    if let Err(reason) = sys::check_devices() {
        eprintln!("skipped: {reason}");
        return;
    }
    if wg("--version", None).is_err() {
        eprintln!("skipped: needs wireguard-tools");
        return;
    }
    let (anchor_key, provider_key) = (wg("genkey", None).unwrap(), wg("genkey", None).unwrap());
    let (anchor_pub, provider_pub) = (
        wg("pubkey", Some(&anchor_key)).unwrap(),
        wg("pubkey", Some(&provider_key)).unwrap(),
    );
    let anchor = Netns::new();
    let provider = Netns::new();
    // The provider: a WireGuard server at 10.50.0.1, tunnel address 10.66.0.1.
    let (key, peer) = (provider_key.clone(), anchor_pub.clone());
    let provider_packets = sys::in_netns(&provider.path, move || {
        ip("link set lo up")?;
        ip("link add wg0 type wireguard")?;
        let conf = format!("[Interface]\nPrivateKey = {key}\nListenPort = 51820\n\n[Peer]\nPublicKey = {peer}\nAllowedIPs = 10.66.0.2/32\n");
        wg("setconf wg0 /dev/stdin", Some(&conf))?;
        ip("addr add 10.66.0.1/24 dev wg0")?;
        ip("link set wg0 up")?;
        Ok::<_, io::Error>(())
    });
    provider_packets.unwrap();
    // The anchor's eth0 is a veth to the provider (the Pod's uplink).
    let provider_tid = provider
        .path
        .to_string_lossy()
        .split('/')
        .nth(4)
        .unwrap()
        .to_owned();
    let uid = "6b9f0c44-0000-4000-8000-0000000000bb";
    let tunnel = qrab_netd::vpn::Tunnel::parse(&format!(
        "[Interface]\nPrivateKey = {anchor_key}\nAddress = 10.66.0.2/32\nDNS = 10.66.0.1\n\n[Peer]\nPublicKey = {provider_pub}\nAllowedIPs = 0.0.0.0/0\nEndpoint = 10.50.0.1:51820\n"
    ))
    .unwrap();
    let plan = NicPlan {
        tap: identity::tap_name(uid, "vpn"),
        tap_mac: identity::tap_mac(uid, "vpn"),
        guest_mac: identity::guest_mac(uid, "vpn"),
        guest_ip: "10.244.128.99".parse().unwrap(),
        vpn: None,
    }
    .with_tunnel(tunnel);
    let check = plan.clone();
    let uplink = sys::in_netns(&anchor.path, move || {
        ip("link set lo up")?;
        ip(&format!(
            "link add eth0 type veth peer name vp0 netns {provider_tid}"
        ))?;
        ip("addr add 10.50.0.2/24 dev eth0")?;
        ip("link set eth0 up")?;
        let mtu = sys::apply(&check, None)?;
        assert_eq!(mtu, 1420, "the tap takes the tunnel's MTU");
        // The pool address would stay on eth0; here the guest has the tunnel's.
        assert!(ip("route get 10.66.0.2")?.contains(&format!("dev {}", check.tap)));
        assert!(ip("-4 rule show")?.contains(&format!("iif {} lookup 100", check.tap)));
        assert!(ip("-4 route show table 100")?.contains("default dev wg0"));
        assert!(ip("-4 route show table 100")?.contains("blackhole default"));
        sys::apply(&check, None)?; // re-applying keeps the session
        Ok::<_, io::Error>(())
    });
    uplink.unwrap();
    let vp0 = sys::in_netns(&provider.path, || {
        ip("addr add 10.50.0.1/24 dev vp0")?;
        ip("link set vp0 up")?;
        packet_socket("vp0")
    })
    .unwrap();
    let tap = plan.tap.clone();
    let queue = sys::in_netns(&anchor.path, move || sys::open_tap_queues(&tap, 1))
        .unwrap()
        .remove(0);
    let ping = |dst: [u8; 4], marker: &[u8]| {
        let frame = icmp_frame(&plan.tap_mac, &plan.guest_mac, [10, 66, 0, 2], dst, marker);
        // SAFETY: frame is valid for its length.
        let n = unsafe { libc::write(queue.as_raw_fd(), frame.as_ptr().cast(), frame.len()) };
        assert_eq!(n, frame.len() as isize, "{}", io::Error::last_os_error());
    };
    // Through the tunnel: the provider answers, and the reply comes back.
    let mut answered = false;
    for attempt in 0..20 {
        ping([10, 66, 0, 1], format!("qrab-vpn-{attempt:02}").as_bytes());
        if queue_saw(&queue, format!("qrab-vpn-{attempt:02}").as_bytes()) {
            answered = true;
            break;
        }
    }
    assert!(answered, "the provider answered through the tunnel");
    // Plaintext never reaches the uplink: aimed past the tunnel at the
    // provider's own uplink address, the ping is still encrypted.
    ping([10, 50, 0, 1], b"qrab-vpn-leak-test");
    assert!(
        !saw(&vp0, b"qrab-vpn-leak-test"),
        "no plaintext on the uplink"
    );
    // Kill switch: with the tunnel gone, nothing of the guest's leaves.
    sys::in_netns(&anchor.path, || ip("link del wg0").map(drop)).unwrap();
    ping([10, 50, 0, 1], b"qrab-vpn-killswitch");
    ping([1, 1, 1, 1], b"qrab-vpn-killswitch");
    assert!(
        !saw(&vp0, b"qrab-vpn-killswitch"),
        "nothing leaves without the tunnel"
    );
    drop(provider);
}

#[test]
fn a_port_forwarding_vpn_nic_maps_through_the_tunnel_only() {
    // SAFETY: geteuid has no preconditions.
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("skipped: needs root to create network namespaces and taps");
        return;
    }
    if let Err(reason) = sys::check_devices() {
        eprintln!("skipped: {reason}");
        return;
    }
    if wg("--version", None).is_err() {
        eprintln!("skipped: needs wireguard-tools");
        return;
    }
    let (anchor_key, provider_key) = (wg("genkey", None).unwrap(), wg("genkey", None).unwrap());
    let (anchor_pub, provider_pub) = (
        wg("pubkey", Some(&anchor_key)).unwrap(),
        wg("pubkey", Some(&provider_key)).unwrap(),
    );
    let anchor = Netns::new();
    let provider = Netns::new();
    // The provider accepts only the client's tunnel address, as Proton does,
    // and runs a NAT-PMP gateway on its tunnel address.
    let (key, peer) = (provider_key.clone(), anchor_pub.clone());
    let gateway = sys::in_netns(&provider.path, move || {
        ip("link set lo up")?;
        ip("link add wg0 type wireguard")?;
        let conf = format!("[Interface]\nPrivateKey = {key}\nListenPort = 51820\n\n[Peer]\nPublicKey = {peer}\nAllowedIPs = 10.66.0.2/32\n");
        wg("setconf wg0 /dev/stdin", Some(&conf))?;
        ip("addr add 10.66.0.1/24 dev wg0")?;
        ip("link set wg0 up")?;
        std::net::UdpSocket::bind("10.66.0.1:5351")
    })
    .unwrap();
    let (seen_tx, seen) = mpsc::channel();
    std::thread::spawn(move || {
        gateway
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        let mut buf = [0u8; 64];
        while let Ok((n, from)) = gateway.recv_from(&mut buf) {
            let _ = seen_tx.send((from, buf[..n].to_vec()));
            let mut reply = vec![0, 128 + buf[1], 0, 0, 0, 0, 0, 9, 0, 0];
            reply.extend(41234u16.to_be_bytes());
            reply.extend(60u32.to_be_bytes());
            gateway.send_to(&reply, from).unwrap();
        }
    });
    let provider_tid = provider
        .path
        .to_string_lossy()
        .split('/')
        .nth(4)
        .unwrap()
        .to_owned();
    let uid = "6b9f0c44-0000-4000-8000-0000000000cc";
    let tunnel = qrab_netd::vpn::Tunnel::parse(&format!(
        "[Interface]\n# qrab: port-forwarding\nPrivateKey = {anchor_key}\nAddress = 10.66.0.2/32\nDNS = 10.66.0.1\n\n[Peer]\nPublicKey = {provider_pub}\nAllowedIPs = 0.0.0.0/0\nEndpoint = 10.50.0.1:51820\n"
    ))
    .unwrap();
    assert!(tunnel.port_forwarding);
    let plan = NicPlan {
        tap: identity::tap_name(uid, "vpn"),
        tap_mac: identity::tap_mac(uid, "vpn"),
        guest_mac: identity::guest_mac(uid, "vpn"),
        guest_ip: "10.244.128.98".parse().unwrap(),
        vpn: None,
    }
    .with_tunnel(tunnel);
    let check = plan.clone();
    sys::in_netns(&anchor.path, move || {
        ip("link set lo up")?;
        ip(&format!(
            "link add eth0 type veth peer name vp0 netns {provider_tid}"
        ))?;
        ip("addr add 10.50.0.2/24 dev eth0")?;
        ip("link set eth0 up")?;
        sys::apply(&check, None)?;
        sys::apply(&check, None)?; // idempotent with the NAT chain too
        Ok::<_, io::Error>(())
    })
    .unwrap();
    sys::in_netns(&provider.path, || {
        ip("addr add 10.50.0.1/24 dev vp0")?;
        ip("link set vp0 up")
    })
    .unwrap();
    let tap = plan.tap.clone();
    let queue = sys::in_netns(&anchor.path, move || sys::open_tap_queues(&tap, 1))
        .unwrap()
        .remove(0);
    // The mapping, from the anchor namespace, as qrab-netd does it.
    let mapped = sys::in_netns(&anchor.path, || {
        Ok(sys::map_forwarded_port("10.66.0.1".parse().unwrap()))
    })
    .unwrap();
    assert_eq!(mapped, Ok(41234));
    // Every request arrived from the tunnel address (the only source the
    // provider admits): UDP (retransmitted while WireGuard's first handshake
    // completes), then TCP, as Proton documents them.
    let requests: Vec<_> = seen.try_iter().collect();
    assert!(
        requests
            .iter()
            .all(|(from, _)| from.ip().to_string() == "10.66.0.2"),
        "{requests:?}"
    );
    let mut kinds: Vec<Vec<u8>> = requests.iter().map(|(_, body)| body.clone()).collect();
    kinds.dedup();
    use qrab_netd::natpmp::{request, Proto};
    assert_eq!(
        kinds,
        [
            request(Proto::Udp, 0, 1, 60).to_vec(),
            request(Proto::Tcp, 0, 1, 60).to_vec()
        ]
    );
    // A renewal over the established tunnel needs no retransmission.
    let before = requests.len();
    let renewed = sys::in_netns(&anchor.path, || {
        Ok(sys::map_forwarded_port("10.66.0.1".parse().unwrap()))
    })
    .unwrap();
    assert_eq!(renewed, Ok(41234));
    assert_eq!(
        seen.try_iter().count(),
        2,
        "one UDP and one TCP request after {before}"
    );
    // The replies went to qrab-netd, not to the guest.
    assert!(
        !queue_saw(&queue, &[0xa1, 0x12, 0, 0, 0, 60]),
        "no NAT-PMP reply reaches the tap"
    );
    drop(provider);
}

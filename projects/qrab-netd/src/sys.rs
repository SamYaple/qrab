//! Kernel side: entering a Pod's network namespace, the tap and vhost-net
//! devices, and applying a [`NicPlan`]. Everything here needs the agent's
//! privileges (CAP_SYS_ADMIN for setns, CAP_NET_ADMIN in the target
//! namespace) and host PIDs.
use crate::{cgroup, plan::NicPlan, plan::UPLINK};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    net::Ipv4Addr,
    os::fd::{AsRawFd, OwnedFd},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const TUN: &str = "/dev/net/tun";
const VHOST_NET: &str = "/dev/vhost-net";
const IFF_TAP: libc::c_short = 0x0002;
const IFF_NO_PI: libc::c_short = 0x1000;
const IFF_MULTI_QUEUE: libc::c_short = 0x0100;
const IFF_VNET_HDR: libc::c_short = 0x4000;
const TAP_FLAGS: libc::c_short = IFF_TAP | IFF_NO_PI | IFF_MULTI_QUEUE | IFF_VNET_HDR;
const TUNSETIFF: libc::c_ulong = 0x4004_54ca;
const TUNSETPERSIST: libc::c_ulong = 0x4004_54cb;

/// The node devices the datapath needs. Opening vhost-net also loads the
/// `vhost_net` module on demand (its devname alias).
pub fn check_devices() -> Result<(), String> {
    for device in [TUN, VHOST_NET] {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(device)
            .map_err(|e| {
                format!(
                    "{device} is unusable on this node ({e}); load the tun and vhost_net modules"
                )
            })?;
    }
    Ok(())
}

/// Run `f` on a thread that has joined the network namespace at `ns`.
/// Processes it spawns inherit that namespace.
pub fn in_netns<T: Send + 'static>(
    ns: &Path,
    f: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> io::Result<T> {
    let ns = File::open(ns)?;
    std::thread::spawn(move || {
        nix::sched::setns(&ns, nix::sched::CloneFlags::CLONE_NEWNET).map_err(io::Error::from)?;
        f()
    })
    .join()
    .map_err(|_| io::Error::other("network namespace worker panicked"))?
}

/// `ip <args>` in the calling thread's network namespace.
pub fn ip(args: &[String]) -> io::Result<String> {
    let output = Command::new("ip")
        .args(args)
        .stdin(Stdio::null())
        .output()?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    Err(io::Error::other(format!(
        "ip {}: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

fn ip_words(args: &[&str]) -> io::Result<String> {
    ip(&args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>())
}

#[repr(C)]
struct IfReq {
    name: [libc::c_char; libc::IFNAMSIZ],
    flags: libc::c_short,
    _pad: [u8; 22],
}

fn tun_attach(name: &str) -> io::Result<OwnedFd> {
    if name.is_empty() || name.len() >= libc::IFNAMSIZ {
        return Err(io::Error::other("invalid tap name"));
    }
    let file = OpenOptions::new().read(true).write(true).open(TUN)?;
    let mut request = IfReq {
        name: [0; libc::IFNAMSIZ],
        flags: TAP_FLAGS,
        _pad: [0; 22],
    };
    for (slot, byte) in request.name.iter_mut().zip(name.bytes()) {
        *slot = byte as libc::c_char;
    }
    // SAFETY: TUNSETIFF reads an ifreq from a valid pointer to a properly
    // sized, initialized struct for the duration of the call.
    if unsafe { libc::ioctl(file.as_raw_fd(), TUNSETIFF, &mut request) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(file.into())
}

/// Create the persistent multiqueue tap in the calling thread's namespace.
/// The device outlives this descriptor; queues attach later.
pub fn create_persistent_tap(name: &str) -> io::Result<()> {
    let queue = tun_attach(name)?;
    // SAFETY: TUNSETPERSIST takes an integer argument on a tun descriptor.
    if unsafe { libc::ioctl(queue.as_raw_fd(), TUNSETPERSIST, 1 as libc::c_ulong) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Attach `count` new queues to an existing tap in the calling thread's
/// namespace.
pub fn open_tap_queues(name: &str, count: u8) -> io::Result<Vec<OwnedFd>> {
    (0..count).map(|_| tun_attach(name)).collect()
}

pub fn open_vhost(count: u8) -> io::Result<Vec<OwnedFd>> {
    (0..count)
        .map(|_| {
            Ok(OpenOptions::new()
                .read(true)
                .write(true)
                .open(VHOST_NET)?
                .into())
        })
        .collect()
}

/// Whether the tap exists in the calling thread's namespace.
pub fn tap_exists(name: &str) -> bool {
    ip_words(&["-o", "link", "show", "dev", name]).is_ok()
}

/// PIDs holding a queue of the named tap (from tun's `iff:` fdinfo line).
/// Tap names are unique per node, so a holder anywhere means the NIC is in
/// use and handing out more queues would split its traffic.
pub fn tap_holders(proc_root: &Path, name: &str) -> Vec<i32> {
    let wanted = format!("iff:\t{name}");
    let Ok(entries) = fs::read_dir(proc_root) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str()?.parse::<i32>().ok())
        .filter(|pid| {
            fs::read_dir(proc_root.join(pid.to_string()).join("fdinfo"))
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .any(|fd| {
                    fs::read_to_string(fd.path()).is_ok_and(|t| t.lines().any(|l| l == wanted))
                })
        })
        .collect()
}

/// The Pod UID of a process.
pub fn pod_uid_of(proc_root: &Path, pid: i32) -> Option<String> {
    cgroup::pod_uid(&fs::read_to_string(proc_root.join(pid.to_string()).join("cgroup")).ok()?)
}

/// A process of the Pod (any of its containers shares the namespace).
pub fn pid_of_pod(proc_root: &Path, uid: &str) -> Option<i32> {
    pids_of_pod(proc_root, uid).into_iter().next()
}

/// Every process of the Pod: its sandbox (pause) and its containers'. They
/// share the network namespace but not the mount namespace.
pub fn pids_of_pod(proc_root: &Path, uid: &str) -> Vec<i32> {
    let Ok(entries) = fs::read_dir(proc_root) else {
        return Vec::new();
    };
    let mut pids: Vec<i32> = entries
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str()?.parse::<i32>().ok())
        .filter(|pid| pod_uid_of(proc_root, *pid).as_deref() == Some(uid))
        .collect();
    pids.sort_unstable();
    pids
}

pub fn netns_of(proc_root: &Path, pid: i32) -> PathBuf {
    proc_root.join(pid.to_string()).join("ns/net")
}

/// The first IPv4 nameserver the Pod's own resolver uses.
pub fn pod_resolver(proc_root: &Path, pid: i32) -> Option<Ipv4Addr> {
    crate::plan::resolver(
        &fs::read_to_string(proc_root.join(pid.to_string()).join("root/etc/resolv.conf")).ok()?,
    )
}

fn write_sysctl(key: &str, value: &str) -> io::Result<()> {
    let path = Path::new("/proc/sys").join(key);
    OpenOptions::new()
        .write(true)
        .open(&path)
        .and_then(|mut f| f.write_all(value.as_bytes()))
        .map_err(|e| io::Error::other(format!("{}: {e}", path.display())))
}

fn nft(ruleset: &str) -> io::Result<()> {
    let mut child = Command::new("nft")
        .args(["-f", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(ruleset.as_bytes())?;
    let output = child.wait_with_output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "nft: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

/// Apply `plan` in the calling thread's namespace: create the tap if
/// needed, route the guest's /32 and gateway to it, redirect DNS when a
/// resolver is given, and finally take the address off `eth0`. Re-applying
/// a finished plan changes nothing.
/// Build `plan` in the calling thread's (anchor) namespace. Returns the tap's
/// MTU, which the DHCP responder offers to a VPN NIC's guest.
pub fn apply(plan: &NicPlan, resolver: Option<Ipv4Addr>) -> io::Result<u32> {
    if !tap_exists(&plan.tap) {
        create_persistent_tap(&plan.tap)?;
    }
    let uplink = ip_words(&["-o", "link", "show", "dev", UPLINK])?;
    let mtu =
        crate::plan::mtu(&uplink).ok_or_else(|| io::Error::other("cannot read the eth0 MTU"))?;
    if let Some(tunnel) = &plan.vpn {
        return apply_vpn(plan, tunnel, tunnel.mtu_on(mtu));
    }
    for command in plan.link_commands(mtu) {
        ip(&command)?;
    }
    for (key, value) in plan.sysctls() {
        write_sysctl(&key, value)?;
    }
    if let Some(resolver) = resolver {
        nft(&plan.dns_ruleset(resolver))?;
    }
    ip(&plan.uplink_placeholder())?;
    let held = ip_words(&["-o", "-4", "addr", "show", "dev", UPLINK])?;
    if held.contains(&format!("inet {}/", plan.guest_ip)) {
        ip(&plan.uplink_release())?;
    }
    if ip_words(&["-4", "route", "show", "default"])?
        .trim()
        .is_empty()
    {
        return Err(io::Error::other(
            "the anchor has no default route (it was flushed with eth0's last address); recreate the anchor Pod",
        ));
    }
    Ok(mtu)
}

/// A VPN NIC: the tap reaches only `wg0`, whose UDP leaves through `eth0`
/// from the anchor's own (pool) address, which therefore stays on `eth0`.
fn apply_vpn(plan: &NicPlan, tunnel: &crate::vpn::Tunnel, mtu: u32) -> io::Result<u32> {
    use crate::vpn::{kill_switch, TABLE, WG_LINK};
    // The kill switch goes in first, so no step below opens a path to eth0.
    nft(&kill_switch(&plan.tap, plan.guest_ip))?;
    for command in plan.link_commands(mtu) {
        ip(&command)?;
    }
    for (key, value) in plan.sysctls() {
        write_sysctl(&key, value)?;
    }
    if !link_exists(WG_LINK) {
        ip_words(&["link", "add", WG_LINK, "type", "wireguard"])?;
    }
    wg_syncconf(&tunnel.setconf())?;
    write_sysctl(&format!("net/ipv4/conf/{WG_LINK}/rp_filter"), "0")?;
    ip_words(&["link", "set", WG_LINK, "mtu", &mtu.to_string(), "up"])?;
    // Everything from the tap goes through the tunnel's table: wg0, else
    // nothing (the blackhole keeps a down tunnel from falling back to main).
    if !ip_words(&["-4", "rule", "show"])?.contains(&format!("iif {} lookup {TABLE}", plan.tap)) {
        ip_words(&[
            "-4", "rule", "add", "iif", &plan.tap, "lookup", TABLE, "priority", "100",
        ])?;
    }
    ip_words(&[
        "-4", "route", "replace", "default", "dev", WG_LINK, "table", TABLE,
    ])?;
    ip_words(&[
        "-4",
        "route",
        "replace",
        "blackhole",
        "default",
        "table",
        TABLE,
        "metric",
        "4096",
    ])?;
    Ok(mtu)
}

fn link_exists(name: &str) -> bool {
    ip_words(&["link", "show", "dev", name]).is_ok()
}

/// `wg syncconf wg0` with the config on stdin: the private key is never on a
/// command line, and an unchanged peer keeps its session.
fn wg_syncconf(config: &str) -> io::Result<()> {
    let mut child = Command::new("wg")
        .args(["syncconf", crate::vpn::WG_LINK, "/dev/stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(config.as_bytes())?;
    let output = child.wait_with_output()?;
    if output.status.success() {
        return Ok(());
    }
    // wg's own messages do not echo keys.
    Err(io::Error::other(format!(
        "wg syncconf: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

/// The node's Cilium agent process, whose mount namespace has the pinned
/// BPF maps and bpftool.
pub fn cilium_agent_pid(proc_root: &Path) -> Option<i32> {
    fs::read_dir(proc_root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str()?.parse::<i32>().ok())
        .find(|pid| {
            fs::read_to_string(proc_root.join(pid.to_string()).join("comm"))
                .is_ok_and(|c| c.trim() == "cilium-agent")
        })
}

/// bpftool in the agent's mount and PID namespaces (its /proc must match
/// for bpftool's fdinfo reads).
fn bpftool(proc_root: &Path, args: &[&str], stdin: Option<&str>) -> io::Result<String> {
    let pid = cilium_agent_pid(proc_root)
        .ok_or_else(|| io::Error::other("no cilium-agent process on this node"))?;
    let mut child = Command::new("nsenter")
        .args(["-t", &pid.to_string(), "-m", "-p", "--", "bpftool"])
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(stdin.unwrap_or_default().as_bytes())?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "bpftool {}: {}",
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// CLOCK_MONOTONIC in seconds: Cilium's conntrack clock (`ktime`).
pub fn monotonic_secs() -> u64 {
    nix::time::clock_gettime(nix::time::ClockId::CLOCK_MONOTONIC)
        .map(|t| u64::try_from(t.tv_sec()).unwrap_or(0))
        .unwrap_or(0)
}

/// This node's conntrack entries for flows of `ip`.
pub fn export_conntrack(proc_root: &Path, ip: Ipv4Addr) -> io::Result<crate::ct::Export> {
    let mut entries = Vec::new();
    for (map, name) in crate::ct::MAPS {
        let path = format!("{}/{name}", crate::ct::PIN_DIR);
        let dump = bpftool(proc_root, &["-j", "map", "dump", "pinned", &path], None)?;
        entries.extend(crate::ct::select(
            crate::ct::parse_dump(map, &dump).map_err(io::Error::other)?,
            ip,
        ));
    }
    Ok(crate::ct::Export::new(monotonic_secs(), entries))
}

/// Whether `ip` is this node's own or one of its local endpoints', judged
/// in the calling thread's (host) namespace.
fn is_local_here(ip: Ipv4Addr) -> bool {
    match ip_words(&["-4", "route", "get", &ip.to_string()]) {
        Ok(route) => route.starts_with("local ") || route.contains(" dev lxc"),
        // Unroutable: treat as local, so nothing is imported for it.
        Err(_) => true,
    }
}

/// Import an export into this node's maps for the NIC at `ip`. Runs in the
/// host namespace; returns how many entries were written.
pub fn import_conntrack(
    proc_root: &Path,
    ip: Ipv4Addr,
    export: &crate::ct::Export,
) -> io::Result<usize> {
    let now = monotonic_secs();
    let mut accepted = Vec::new();
    for entry in export.decode().map_err(io::Error::other)? {
        crate::ct::importable(&entry, ip, is_local_here).map_err(io::Error::other)?;
        if let Some(moved) = crate::ct::rebase(&entry, export.now, now) {
            accepted.push(moved);
        }
    }
    if !accepted.is_empty() {
        bpftool(
            proc_root,
            &["batch", "file", "/dev/stdin"],
            Some(&crate::ct::batch(&accepted)),
        )?;
    }
    Ok(accepted.len())
}

/// The wg-quick config a VPN anchor mounts, read through the root of the
/// Pod's process that has the mount: the anchor container's, not the
/// sandbox's (whose mount namespace has no volumes).
pub fn read_tunnel(proc_root: &Path, pod_uid: &str) -> Result<crate::vpn::Tunnel, String> {
    let mut last = None;
    for pid in pids_of_pod(proc_root, pod_uid) {
        let path = proc_root
            .join(pid.to_string())
            .join("root")
            .join(crate::vpn::VPN_MOUNT.trim_start_matches('/'))
            .join(crate::vpn::VPN_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => return crate::vpn::Tunnel::parse(&text),
            Err(error) => last = Some(error),
        }
    }
    Err(match last {
        Some(error) => format!("cannot read the anchor's VPN config: {error}"),
        None => "no running process of the anchor yet".into(),
    })
}

/// Route protocol marking the transient forward routes this agent owns in
/// the host's main table, so it finds and removes only its own.
pub const FORWARD_PROTO: &str = "252";

/// The host's network namespace (the agent runs with host PIDs).
pub fn host_netns(proc_root: &Path) -> PathBuf {
    proc_root.join("1/ns/net")
}

/// `<ip>/32 via <via>` in the calling thread's (host) namespace.
pub fn forward(ip: Ipv4Addr, via: Ipv4Addr) -> io::Result<()> {
    ip_words(&[
        "route",
        "replace",
        &format!("{ip}/32"),
        "via",
        &via.to_string(),
        "proto",
        FORWARD_PROTO,
    ])
    .map(drop)
}

/// Remove the forward route for `ip`, if any.
pub fn unforward(ip: Ipv4Addr) -> io::Result<()> {
    match ip_words(&["route", "del", &format!("{ip}/32"), "proto", FORWARD_PROTO]) {
        Err(e) if e.to_string().contains("No such process") => Ok(()),
        other => other.map(drop),
    }
}

/// Addresses with a forward route of ours in the calling thread's namespace.
pub fn forwards() -> io::Result<Vec<Ipv4Addr>> {
    Ok(ip_words(&["-4", "route", "show", "proto", FORWARD_PROTO])?
        .lines()
        .filter_map(|line| {
            line.split_whitespace()
                .next()?
                .trim_end_matches("/32")
                .parse()
                .ok()
        })
        .collect())
}

/// Ask the node's Cilium agent to move an endpoint through the handoff
/// states (`prepare`, `active`, `standby`; forks/cilium P3) over its local
/// API socket. `endpoint` is `cep-name:<namespace>/<pod>`.
pub fn cilium_handoff(socket: &Path, endpoint: &str, state: &str) -> io::Result<()> {
    use std::io::Read;
    let mut stream = std::os::unix::net::UnixStream::connect(socket)?;
    // prepare regenerates the endpoint and may take up to a minute.
    stream.set_read_timeout(Some(std::time::Duration::from_secs(90)))?;
    let body = format!("{{\"state\":\"{state}\"}}");
    let id = endpoint.replace('/', "%2F");
    write!(
        stream,
        "PATCH /v1/endpoint/{id}/handoff HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    let mut reply = String::new();
    stream.read_to_string(&mut reply)?;
    let status = reply.split_whitespace().nth(1).unwrap_or("");
    if status.starts_with('2') {
        return Ok(());
    }
    let detail = reply.split("\r\n\r\n").nth(1).unwrap_or("").trim();
    Err(io::Error::other(format!(
        "cilium handoff {state} for {endpoint}: HTTP {status} {detail}"
    )))
}

/// Offset of the reply's DHCP message type value (option 53 comes first).
const HEADER_KIND_OFFSET: usize = 242;

/// Serve DHCP for one tap until `stop` is set: a UDP socket on port 67 in
/// the anchor's namespace, bound to the tap, answering only `lease.mac`.
/// The socket keeps the namespace alive, so the caller stops it when the
/// anchor goes.
pub fn serve_dhcp(
    ns: PathBuf,
    tap: String,
    lease: crate::dhcp::Lease,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> io::Result<()> {
    let netns = File::open(&ns)?;
    std::thread::Builder::new().name(format!("dhcp-{tap}")).spawn(move || {
        let run = || -> io::Result<()> {
            nix::sched::setns(&netns, nix::sched::CloneFlags::CLONE_NEWNET).map_err(io::Error::from)?;
            let socket = socket2_udp(&tap)?;
            let mut buffer = [0u8; 1500];
            eprintln!("{}", serde_json::json!({"event":"dhcp_serving","detail":{"tap":tap,"address":lease.ip}}));
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                let (n, from) = match socket.recv_from(&mut buffer) {
                    Ok(received) => received,
                    Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => continue,
                    Err(e) => return Err(e),
                };
                if let Some(reply) = crate::dhcp::reply(&buffer[..n], &lease) {
                    socket.send_to(&reply, (Ipv4Addr::BROADCAST, 68))?;
                    eprintln!("{}", serde_json::json!({"event":"dhcp_reply","detail":{"tap":tap,"from":from.to_string(),"kind":reply[HEADER_KIND_OFFSET]}}));
                }
            }
            Ok(())
        };
        if let Err(error) = run() {
            eprintln!("{}", serde_json::json!({"event":"dhcp_failed","detail":{"tap":tap,"error":error.to_string()}}));
        }
    })?;
    Ok(())
}

fn socket2_udp(tap: &str) -> io::Result<std::net::UdpSocket> {
    use nix::sys::socket::{setsockopt, sockopt};
    let socket = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 67))?;
    setsockopt(
        &socket,
        sockopt::BindToDevice,
        &std::ffi::OsString::from(tap),
    )
    .map_err(io::Error::from)?;
    socket.set_broadcast(true)?;
    socket.set_read_timeout(Some(std::time::Duration::from_secs(1)))?;
    Ok(socket)
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_vpn_config_is_read_through_the_process_that_mounts_it() {
        let uid = "6b9f0c44-0000-4000-8000-0000000000cc";
        let proc_root = tempfile::tempdir().unwrap();
        let cgroup = format!(
            "0::/kubepods.slice/kubepods-pod{}.slice/cri-containerd-x.scope\n",
            uid.replace('-', "_")
        );
        // pid 7: the sandbox, no volumes; pid 9: the anchor container.
        for pid in ["7", "9", "11"] {
            let dir = proc_root.path().join(pid);
            std::fs::create_dir_all(dir.join("root")).unwrap();
            let owner = if pid == "11" {
                "0::/system.slice/other.scope\n".to_owned()
            } else {
                cgroup.clone()
            };
            std::fs::write(dir.join("cgroup"), owner).unwrap();
        }
        assert_eq!(pids_of_pod(proc_root.path(), uid), [7, 9]);
        assert!(read_tunnel(proc_root.path(), uid)
            .unwrap_err()
            .contains("No such file"));
        let mount = proc_root.path().join("9/root/run/qrab-vpn");
        std::fs::create_dir_all(&mount).unwrap();
        std::fs::write(mount.join("wg.conf"), "[Interface]\nPrivateKey = yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=\nAddress = 10.2.0.2/32\n[Peer]\nPublicKey = xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=\nEndpoint = 203.0.113.9:51820\n").unwrap();
        // The sandbox comes first but lacks the mount; the container has it.
        assert_eq!(
            read_tunnel(proc_root.path(), uid)
                .unwrap()
                .address
                .to_string(),
            "10.2.0.2"
        );
        assert_eq!(
            read_tunnel(proc_root.path(), "00000000-0000-4000-8000-000000000000").unwrap_err(),
            "no running process of the anchor yet"
        );
    }
    use super::*;
    use std::io::{BufRead, BufReader, Read};

    /// A stand-in for the Cilium agent socket: records one request and
    /// answers with `status`.
    fn agent(status: &'static str) -> (tempfile::TempDir, std::thread::JoinHandle<String>) {
        let dir = tempfile::tempdir().unwrap();
        let listener =
            std::os::unix::net::UnixListener::bind(dir.path().join("cilium.sock")).unwrap();
        let served = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
                if line == "\r\n" {
                    break;
                }
                head.push_str(&line);
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: 5\r\n\r\nnope!"
            )
            .unwrap();
            format!("{head}{}", String::from_utf8(body).unwrap())
        });
        (dir, served)
    }

    #[test]
    fn handoff_patches_the_endpoint_by_cep_name() {
        let (dir, served) = agent("200 OK");
        cilium_handoff(
            &dir.path().join("cilium.sock"),
            "cep-name:qrab/web-nic-lan-kube02",
            "active",
        )
        .unwrap();
        let request = served.join().unwrap();
        assert!(
            request.starts_with(
                "PATCH /v1/endpoint/cep-name:qrab%2Fweb-nic-lan-kube02/handoff HTTP/1.1\r\n"
            ),
            "{request}"
        );
        assert!(request.ends_with("{\"state\":\"active\"}"), "{request}");
    }

    #[test]
    fn a_refused_handoff_reports_the_agents_answer() {
        let (dir, served) = agent("409 Conflict");
        let error = cilium_handoff(
            &dir.path().join("cilium.sock"),
            "cep-name:qrab/x",
            "standby",
        )
        .unwrap_err();
        served.join().unwrap();
        assert_eq!(
            error.to_string(),
            "cilium handoff standby for cep-name:qrab/x: HTTP 409 nope!"
        );
    }
}

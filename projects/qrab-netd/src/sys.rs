//! Kernel side: entering a Pod's network namespace, the tap and vhost-net
//! devices, and applying a [`NicPlan`]. Everything here needs the agent's
//! privileges (CAP_SYS_ADMIN for setns, CAP_NET_ADMIN in the target
//! namespace) and host PIDs.
use crate::process::Process;
use crate::{plan::NicPlan, plan::UPLINK};
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

/// Run `f` in a configured host/test namespace. Authorized Pod resources use
/// an already-open descriptor through [`in_namespace`], not a later PID path.
pub fn in_netns<T: Send + 'static>(
    ns: &Path,
    f: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> io::Result<T> {
    in_namespace(File::open(ns)?, f)
}

/// Enter an already-open namespace on a dedicated worker. Authorized Pod
/// resources use this form, so a later numeric PID lookup cannot replace it.
pub fn in_namespace<T: Send + 'static>(
    ns: File,
    f: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> io::Result<T> {
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

/// Held processes of the Pod, including sandbox and containers. Membership
/// and subsequent resource reads use the same proc directory descriptor.
pub fn processes_of_pod(proc_root: &Path, uid: &str) -> Vec<crate::process::Process> {
    let Ok(entries) = fs::read_dir(proc_root) else {
        return Vec::new();
    };
    let mut processes: Vec<_> = entries
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str()?.parse::<i32>().ok())
        .filter_map(|pid| crate::process::Process::open(proc_root, pid).ok())
        .filter(|process| process.pod_uid().ok().as_deref() == Some(uid))
        .collect();
    processes.sort_unstable_by_key(crate::process::Process::pid);
    processes
}

/// The first IPv4 nameserver the Pod's own resolver uses.
pub fn pod_resolver(process: &crate::process::Process) -> Option<Ipv4Addr> {
    crate::plan::resolver(
        &process
            .read_text(Path::new("root/etc/resolv.conf"), 16 * 1024)
            .ok()?,
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
    let natpmp = tunnel.port_forwarding.then(|| tunnel.gateway()).flatten();
    nft(&kill_switch(&plan.tap, plan.guest_ip, natpmp))?;
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
    // qrab-netd's own NAT-PMP requests reach the gateway through the tunnel
    // (the main table: the guest's traffic follows table 100 instead).
    if let Some(gateway) = natpmp {
        ip_words(&[
            "-4",
            "route",
            "replace",
            &format!("{gateway}/32"),
            "dev",
            WG_LINK,
        ])?;
    }
    Ok(mtu)
}

/// Map a forwarded port with the tunnel gateway, from the calling thread's
/// (anchor) namespace: requests leave from the tap's gateway address and
/// the kill switch's `natpmp` chain gives them the tunnel address.
pub fn map_forwarded_port(gateway: Ipv4Addr) -> Result<u16, String> {
    let socket = std::net::UdpSocket::bind((crate::GUEST_GATEWAY, 0))
        .map_err(|e| format!("cannot bind the NAT-PMP socket: {e}"))?;
    socket
        .connect((gateway, crate::natpmp::PORT))
        .map_err(|e| format!("cannot reach the tunnel gateway {gateway}: {e}"))?;
    crate::natpmp::map_both(&socket)
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

/// bpftool in the agent's mount and PID namespaces (its /proc must match
/// for bpftool's fdinfo reads).
fn bpftool(
    provider: &crate::cilium::Provider,
    args: &[&str],
    stdin: Option<&str>,
) -> io::Result<String> {
    let mut child = provider.spawn_bpftool(args, Stdio::piped())?;
    let written = child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(stdin.unwrap_or_default().as_bytes());
    // Reap the tool even if it closed stdin before the complete batch.
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "bpftool {}: {}",
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    written?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// CLOCK_MONOTONIC in seconds: Cilium's conntrack clock (`ktime`).
pub fn monotonic_secs() -> u64 {
    nix::time::clock_gettime(nix::time::ClockId::CLOCK_MONOTONIC)
        .map(|t| u64::try_from(t.tv_sec()).unwrap_or(0))
        .unwrap_or(0)
}

/// This node's conntrack entries for flows of `ip`.
pub fn export_conntrack(
    proc_root: &Path,
    socket: &Path,
    ip: Ipv4Addr,
) -> io::Result<crate::ct::Export> {
    let provider = crate::cilium::Provider::connect(socket, proc_root)?;
    let mut entries = Vec::new();
    for (map, name) in crate::ct::MAPS {
        let path = format!("{}/{name}", crate::ct::PIN_DIR);
        entries.extend(bpftool_dump(&provider, &path, |dump| {
            crate::ct::select_dump(map, dump, ip)
        })?);
    }
    Ok(crate::ct::Export::new(monotonic_secs(), entries))
}

/// `bpftool -j map dump pinned <path>`, its output handed to `read` as it
/// streams (a whole dump is tens of MB once parsed; netd's memory limit is
/// 128 MiB).
fn bpftool_dump<T>(
    provider: &crate::cilium::Provider,
    path: &str,
    read: impl FnOnce(std::process::ChildStdout) -> Result<T, String>,
) -> io::Result<T> {
    let mut child =
        provider.spawn_bpftool(&["-j", "map", "dump", "pinned", path], Stdio::null())?;
    let read = read(child.stdout.take().expect("piped stdout"));
    if read.is_err() {
        let _ = child.kill();
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "bpftool dump: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    read.map_err(io::Error::other)
}

/// Whether `route` (`ip route get` in a node's host namespace) reaches its
/// address directly on a host link: another node or the VM host on the node
/// network, not a Pod (local ones go through Cilium's devices, remote ones
/// via a node) or anything behind a router.
///
/// ```
/// use qrab_netd::sys::on_link;
/// assert!(on_link("10.203.0.12 dev enp0s2 src 10.203.0.11 uid 0"));
/// assert!(!on_link("10.244.3.250 via 10.203.0.12 dev enp0s2 src 10.203.0.11 uid 0"));
/// assert!(!on_link("10.244.0.49 dev cilium_host src 10.244.0.66 uid 0"));
/// assert!(!on_link("10.244.0.9 dev lxc1a2b src 10.244.0.66 uid 0"));
/// assert!(!on_link("local 10.203.0.11 dev lo src 10.203.0.11 uid 0"));
/// ```
pub fn on_link(route: &str) -> bool {
    !route.starts_with("local ")
        && !route.contains(" via ")
        && !route.contains(" dev cilium_")
        && !route.contains(" dev lxc")
}

/// Whether `ip` is a peer on this node's host links (see [`on_link`]),
/// judged in the calling thread's (host) namespace.
pub fn is_link_peer(ip: Ipv4Addr) -> bool {
    ip_words(&["-4", "route", "get", &ip.to_string()]).is_ok_and(|route| on_link(&route))
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
    socket: &Path,
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
        let provider = crate::cilium::Provider::connect(socket, proc_root)?;
        bpftool(
            &provider,
            &["batch", "file", "/dev/stdin"],
            Some(&crate::ct::batch(&accepted)),
        )?;
    }
    Ok(accepted.len())
}

/// The config mounted by the already-verified current anchor container.
/// Never rescan arbitrary same-UID processes for a convenient older mount.
pub fn read_tunnel(process: &Process) -> Result<crate::vpn::Tunnel, String> {
    tunnel_snapshot(process).map(|snapshot| snapshot.tunnel)
}

/// The parsed plan and fingerprint of the exact same bounded byte snapshot.
#[derive(Debug)]
pub struct TunnelSnapshot {
    pub tunnel: crate::vpn::Tunnel,
    pub digest: [u8; 32],
}

pub fn tunnel_snapshot(process: &Process) -> Result<TunnelSnapshot, String> {
    use sha2::{Digest, Sha256};
    let text = read_tunnel_text(process)?;
    Ok(TunnelSnapshot {
        tunnel: crate::vpn::Tunnel::parse(&text)?,
        digest: Sha256::digest(text.as_bytes()).into(),
    })
}

/// A digest read through that same held container reference.
pub fn tunnel_digest(process: &Process) -> Option<[u8; 32]> {
    use sha2::{Digest, Sha256};
    read_tunnel_text(process)
        .ok()
        .map(|text| Sha256::digest(text.as_bytes()).into())
}

fn read_tunnel_text(process: &Process) -> Result<String, String> {
    let path = Path::new("root")
        .join(crate::vpn::VPN_MOUNT.trim_start_matches('/'))
        .join(crate::vpn::VPN_FILE);
    process
        .read_text(&path, crate::vpn::MAX_CONFIG)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::FileTooLarge {
                "the WireGuard config is too large".into()
            } else {
                format!("cannot read the anchor's VPN config: {error}")
            }
        })
}

/// Route protocol marking the transient forward routes this agent owns, so
/// it finds and removes only its own.
pub const FORWARD_PROTO: &str = "252";
/// The table forward routes live in. They used to share the main table
/// with Cilium's multi-pool route for the same /32, and whoever wrote last
/// won: live, the source's agent re-installed its pool route (to the
/// now-standby endpoint) 16 s after a switch, black-holing what still
/// reached the old node.
pub const FORWARD_TABLE: &str = "252";
/// The rule sending every lookup to [`FORWARD_TABLE`] first: after Cilium's
/// `local` rule (priority 100), before `main`. Cilium's BPF FIB lookups
/// honour rules (no `BPF_FIB_LOOKUP_DIRECT` without a table id).
pub const FORWARD_RULE_PRIORITY: &str = "200";

/// The host's network namespace (the agent runs with host PIDs).
pub fn host_netns(proc_root: &Path) -> PathBuf {
    proc_root.join("1/ns/net")
}

/// Ensure the [`FORWARD_RULE_PRIORITY`] rule exists in the calling thread's
/// namespace.
fn ensure_forward_rule() -> io::Result<()> {
    let rules = ip_words(&["-4", "rule", "show", "priority", FORWARD_RULE_PRIORITY])?;
    if rules.contains(&format!("lookup {FORWARD_TABLE}")) {
        return Ok(());
    }
    ip_words(&[
        "-4",
        "rule",
        "add",
        "priority",
        FORWARD_RULE_PRIORITY,
        "lookup",
        FORWARD_TABLE,
    ])
    .map(drop)
}

/// `<ip>/32 via <via>` in [`FORWARD_TABLE`] of the calling thread's (host)
/// namespace, ahead of any main-table route for it.
pub fn forward(ip: Ipv4Addr, via: Ipv4Addr) -> io::Result<()> {
    ensure_forward_rule()?;
    ip_words(&[
        "route",
        "replace",
        &format!("{ip}/32"),
        "via",
        &via.to_string(),
        "proto",
        FORWARD_PROTO,
        "table",
        FORWARD_TABLE,
    ])
    .map(drop)?;
    // An older agent's main-table forward for the address goes.
    delete_forward(ip, "main")
}

fn delete_forward(ip: Ipv4Addr, table: &str) -> io::Result<()> {
    match ip_words(&[
        "route",
        "del",
        &format!("{ip}/32"),
        "proto",
        FORWARD_PROTO,
        "table",
        table,
    ]) {
        Err(e) if e.to_string().contains("No such process") => Ok(()),
        Err(e) if e.to_string().contains("does not exist") => Ok(()),
        other => other.map(drop),
    }
}

/// Remove the forward route for `ip`, if any (also an older agent's, in the
/// main table).
pub fn unforward(ip: Ipv4Addr) -> io::Result<()> {
    delete_forward(ip, FORWARD_TABLE)?;
    delete_forward(ip, "main")
}

/// Addresses with a forward route of ours in the calling thread's namespace.
pub fn forwards() -> io::Result<Vec<Ipv4Addr>> {
    let mut found = Vec::new();
    for table in [FORWARD_TABLE, "main"] {
        let routes = match ip_words(&[
            "-4",
            "route",
            "show",
            "table",
            table,
            "proto",
            FORWARD_PROTO,
        ]) {
            Ok(routes) => routes,
            // A table nothing was ever added to does not exist yet.
            Err(e) if e.to_string().contains("does not exist") => continue,
            Err(e) => return Err(e),
        };
        found.extend(routes.lines().filter_map(|line| {
            line.split_whitespace()
                .next()?
                .trim_end_matches("/32")
                .parse::<Ipv4Addr>()
                .ok()
        }));
    }
    found.sort();
    found.dedup();
    Ok(found)
}

/// Offset of the reply's DHCP message type value (option 53 comes first).
const HEADER_KIND_OFFSET: usize = 242;

/// Serve DHCP for one tap until `stop` is set: a UDP socket on port 67 in
/// the anchor's namespace, bound to the tap, answering only the current MAC.
/// The socket keeps the namespace alive, so the caller stops it when the
/// anchor goes. Bind/setup complete before success is returned. Each request
/// reads one whole lease snapshot, so applied DNS/address/MTU changes do not
/// need a socket restart.
pub fn serve_dhcp(
    netns: File,
    tap: String,
    lease: std::sync::Arc<std::sync::RwLock<crate::dhcp::Lease>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> io::Result<()> {
    let interface = tap.clone();
    // The UDP socket retains this namespace for subsequent I/O, regardless
    // of the serving thread's namespace. Setup errors must reach plumbing
    // before it can publish readiness, rather than only a background log.
    let socket = in_namespace(netns, move || socket2_udp(&interface))?;
    std::thread::Builder::new().name(format!("dhcp-{tap}")).spawn(move || {
        let run = || -> io::Result<()> {
            let mut buffer = [0u8; 1500];
            let address = lease.read().unwrap_or_else(|e| e.into_inner()).ip;
            eprintln!("{}", serde_json::json!({"event":"dhcp_serving","detail":{"tap":tap,"address":address}}));
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                let (n, from) = match socket.recv_from(&mut buffer) {
                    Ok(received) => received,
                    Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => continue,
                    Err(e) => return Err(e),
                };
                let current = *lease.read().unwrap_or_else(|e| e.into_inner());
                if let Some(reply) = crate::dhcp::reply(&buffer[..n], &current) {
                    socket.send_to(&reply, (Ipv4Addr::BROADCAST, 68))?;
                    eprintln!("{}", serde_json::json!({"event":"dhcp_reply","detail":{"tap":tap,"from":from.to_string(),"kind":reply[HEADER_KIND_OFFSET]}}));
                }
            }
            Ok(())
        };
        let outcome = run();
        // Release port 67 before publishing exit. A replacement must not
        // race a failed worker that still owns the old socket.
        drop(socket);
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Err(error) = outcome {
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
    fn a_writable_process_name_cannot_authorize_a_privileged_provider() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("5")).unwrap();
        std::fs::write(root.path().join("5/comm"), "cilium-agent\n").unwrap();
        assert!(crate::cilium::Provider::connect(
            &root.path().join("missing-cilium.sock"),
            root.path()
        )
        .is_err());
    }

    #[test]
    fn a_tunnel_snapshot_keeps_the_digest_of_the_bytes_actually_parsed() {
        use sha2::{Digest, Sha256};
        let root = tempfile::tempdir().unwrap();
        let mount = root.path().join("7/root/run/qrab-vpn");
        std::fs::create_dir_all(&mount).unwrap();
        let path = mount.join("wg.conf");
        let config = |address: &str| {
            format!("[Interface]\nPrivateKey = yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=\nAddress = 10.2.0.2/32\nDNS = 10.2.0.1\n[Peer]\nPublicKey = xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=\nEndpoint = {address}:51820\n")
        };
        let a = config("203.0.113.9");
        let b = config("203.0.113.10");
        let process = Process::open(root.path(), 7).unwrap();
        std::fs::write(&path, &a).unwrap();
        let earlier = tunnel_digest(&process).unwrap();
        std::fs::write(&path, &b).unwrap();
        let applied = tunnel_snapshot(&process).unwrap();
        assert_eq!(applied.tunnel.endpoint.to_string(), "203.0.113.10:51820");
        assert_eq!(
            applied.digest,
            <[u8; 32]>::from(Sha256::digest(b.as_bytes()))
        );
        assert_ne!(applied.digest, earlier);
        std::fs::write(&path, &a).unwrap();
        let desired = tunnel_digest(&process).unwrap();
        // Recording the pre-plumb digest would falsely consider restored A
        // applied, even though the parsed/applied plan was B.
        assert_eq!(earlier, desired);
        assert_ne!(applied.digest, desired);
        assert_eq!(applied.tunnel.endpoint.to_string(), "203.0.113.10:51820");
    }

    #[test]
    fn dhcp_namespace_setup_must_succeed_before_the_responder_is_reported_started() {
        let namespace = tempfile::tempfile().unwrap();
        let lease = crate::dhcp::Lease {
            mac: [2, 0, 0, 0, 0, 1],
            ip: "10.99.0.7".parse().unwrap(),
            seconds: 600,
            dns: Some(crate::GUEST_GATEWAY),
            mtu: None,
        };
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        // A regular file cannot be entered as a network namespace, even
        // when this fixture runs with namespace capabilities.
        assert!(super::serve_dhcp(
            namespace,
            "missing0".into(),
            std::sync::Arc::new(std::sync::RwLock::new(lease)),
            stop,
        )
        .is_err());
    }

    #[test]
    fn a_vpn_config_is_read_through_the_process_that_mounts_it() {
        let uid = "6b9f0c44-0000-4000-8000-0000000000cc";
        let proc_root = tempfile::tempdir().unwrap();
        let cgroup = format!(
            "0::/kubepods.slice/kubepods-pod{}.slice/cri-containerd-x.scope\n",
            uid.replace('-', "_")
        );
        // pid 5: a host-user lookalike; pid 7: an older same-UID process;
        // pid 9: the selected anchor container; pid 11: unrelated host service.
        for pid in ["5", "7", "9", "11"] {
            let dir = proc_root.path().join(pid);
            std::fs::create_dir_all(dir.join("root")).unwrap();
            let owner = match pid {
                "5" => format!("0::/user.slice/user-1002.slice/user@1002.service/app.slice/qrab-audit-pod{uid}.scope\n"),
                "11" => "0::/system.slice/other.scope\n".to_owned(),
                _ => cgroup.clone(),
            };
            std::fs::write(dir.join("cgroup"), owner).unwrap();
        }
        assert!(crate::process::Process::open(proc_root.path(), 5)
            .unwrap()
            .pod_uid()
            .is_err());
        assert_eq!(
            processes_of_pod(proc_root.path(), uid)
                .iter()
                .map(crate::process::Process::pid)
                .collect::<Vec<_>>(),
            [7, 9]
        );
        let old_mount = proc_root.path().join("7/root/run/qrab-vpn");
        std::fs::create_dir_all(&old_mount).unwrap();
        std::fs::write(
            old_mount.join("wg.conf"),
            "an older config must not substitute for the selected process",
        )
        .unwrap();
        assert!(
            read_tunnel(&crate::process::Process::open(proc_root.path(), 9).unwrap())
                .unwrap_err()
                .contains("No such file")
        );
        let mount = proc_root.path().join("9/root/run/qrab-vpn");
        std::fs::create_dir_all(&mount).unwrap();
        std::fs::write(mount.join("wg.conf"), "[Interface]\nPrivateKey = yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=\nAddress = 10.2.0.2/32\n[Peer]\nPublicKey = xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=\nEndpoint = 203.0.113.9:51820\n").unwrap();
        // The earlier same-UID mount never overrides the selected container.
        assert_eq!(
            read_tunnel(&crate::process::Process::open(proc_root.path(), 9).unwrap())
                .unwrap()
                .address
                .to_string(),
            "10.2.0.2"
        );
        assert!(
            processes_of_pod(proc_root.path(), "00000000-0000-4000-8000-000000000000").is_empty()
        );
    }
    use super::*;
}

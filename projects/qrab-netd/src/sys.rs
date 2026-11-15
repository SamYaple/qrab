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
    fs::read_dir(proc_root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str()?.parse::<i32>().ok())
        .find(|pid| pod_uid_of(proc_root, *pid).as_deref() == Some(uid))
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
pub fn apply(plan: &NicPlan, resolver: Option<Ipv4Addr>) -> io::Result<()> {
    if !tap_exists(&plan.tap) {
        create_persistent_tap(&plan.tap)?;
    }
    let uplink = ip_words(&["-o", "link", "show", "dev", UPLINK])?;
    let mtu =
        crate::plan::mtu(&uplink).ok_or_else(|| io::Error::other("cannot read the eth0 MTU"))?;
    for command in plan.link_commands(mtu) {
        ip(&command)?;
    }
    for (key, value) in plan.sysctls() {
        write_sysctl(&key, value)?;
    }
    if let Some(resolver) = resolver {
        nft(&plan.dns_ruleset(resolver))?;
    }
    let held = ip_words(&["-o", "-4", "addr", "show", "dev", UPLINK])?;
    if held.contains(&format!("inet {}/", plan.guest_ip)) {
        ip(&plan.uplink_release())?;
    }
    Ok(())
}

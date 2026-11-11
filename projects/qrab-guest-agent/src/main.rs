use qrab_guest_agent::{sys::Linux, Agent, Config};
use std::{
    path::{Path, PathBuf},
    process::ExitCode,
    sync::Arc,
    time::Duration,
};

const PORT_NAME: &str = "org.qemu.guest_agent.0";
const USAGE: &str =
    "qrab-guest-agent [--device PATH] [--freeze-timeout SECS] [--update-timeout SECS]

Serves the QEMU guest agent protocol on the virtio-serial port named
org.qemu.guest_agent.0. Without --device the port is found through
/dev/virtio-ports or, without udev, /sys/class/virtio-ports.
The update hook path is fixed at /etc/qrab/update-hook.";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("qrab-guest-agent: {error}\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn seconds(
    value: Option<String>,
    range: std::ops::RangeInclusive<u64>,
) -> Result<Duration, String> {
    value
        .and_then(|v| v.parse().ok())
        .filter(|v| range.contains(v))
        .map(Duration::from_secs)
        .ok_or_else(|| format!("expected seconds in {range:?}"))
}

fn run() -> Result<(), String> {
    let mut config = Config::default();
    let mut device = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--device" => device = Some(PathBuf::from(args.next().ok_or("--device needs a path")?)),
            "--freeze-timeout" => config.freeze_timeout = seconds(args.next(), 1..=3600)?,
            "--update-timeout" => config.update.timeout = seconds(args.next(), 1..=86400)?,
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    let agent = Agent::new(config, Arc::new(Linux::default()));
    eprintln!(
        "qrab-guest-agent: started (version {})",
        env!("CARGO_PKG_VERSION")
    );
    let mut waiting_logged = false;
    loop {
        let Some(path) = device.clone().or_else(find_port) else {
            if !std::mem::replace(&mut waiting_logged, true) {
                eprintln!("qrab-guest-agent: waiting for virtio-serial port {PORT_NAME}");
            }
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };
        let file = match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
        {
            Ok(file) => file,
            Err(e) => {
                if !std::mem::replace(&mut waiting_logged, true) {
                    eprintln!("qrab-guest-agent: cannot open {}: {e}", path.display());
                }
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
        };
        waiting_logged = false;
        eprintln!("qrab-guest-agent: serving {}", path.display());
        let writer = file.try_clone().map_err(|e| e.to_string())?;
        // A virtio-serial port reads EOF while no host client is connected.
        // Sleep like qemu-ga instead of spinning, then keep the open port.
        loop {
            match agent.serve(&file, &writer) {
                Ok(()) => std::thread::sleep(Duration::from_millis(100)),
                Err(e) => {
                    eprintln!("qrab-guest-agent: {}: {e}; reopening", path.display());
                    std::thread::sleep(Duration::from_secs(1));
                    break;
                }
            }
        }
    }
}

fn find_port() -> Option<PathBuf> {
    let link = Path::new("/dev/virtio-ports").join(PORT_NAME);
    if link.exists() {
        return Some(link);
    }
    std::fs::read_dir("/sys/class/virtio-ports")
        .ok()?
        .flatten()
        .find(|entry| {
            std::fs::read_to_string(entry.path().join("name")).is_ok_and(|n| n.trim() == PORT_NAME)
        })
        .map(|entry| Path::new("/dev").join(entry.file_name()))
}

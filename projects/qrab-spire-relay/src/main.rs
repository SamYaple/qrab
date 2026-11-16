use qrab_spire_relay::{guest, parse_args, wire::PORT_NAME, USAGE};
use std::{
    net::TcpListener,
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

fn main() -> ExitCode {
    let options = match parse_args(std::env::args().skip(1)) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("qrab-spire-relay: {error}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!(
        "qrab-spire-relay: started (version {})",
        env!("CARGO_PKG_VERSION")
    );
    let relay = match guest::Relay::new(bind(), options.max_connections) {
        Ok(relay) => relay,
        Err(error) => {
            eprintln!("qrab-spire-relay: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut waiting_logged = false;
    loop {
        let Some(path) = options.device.clone().or_else(find_port) else {
            if !std::mem::replace(&mut waiting_logged, true) {
                eprintln!("qrab-spire-relay: waiting for virtio-serial port {PORT_NAME}");
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
                    eprintln!("qrab-spire-relay: cannot open {}: {e}", path.display());
                }
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
        };
        waiting_logged = false;
        eprintln!("qrab-spire-relay: serving {}", path.display());
        // The port reads EOF while no host is connected; each EOF ends a
        // host session and closes its connections.
        loop {
            let writer = match file.try_clone() {
                Ok(writer) => writer,
                Err(e) => {
                    eprintln!("qrab-spire-relay: {e}");
                    return ExitCode::FAILURE;
                }
            };
            match relay.serve(&file, writer) {
                Ok(()) => std::thread::sleep(Duration::from_millis(100)),
                Err(e) => {
                    eprintln!("qrab-spire-relay: {}: {e}; reopening", path.display());
                    std::thread::sleep(Duration::from_secs(1));
                    break;
                }
            }
        }
    }
}

/// Binds the fixed loopback address, waiting until loopback is up.
fn bind() -> TcpListener {
    let mut logged = false;
    loop {
        match TcpListener::bind(guest::LISTEN) {
            Ok(listener) => return listener,
            Err(e) => {
                if !std::mem::replace(&mut logged, true) {
                    eprintln!("qrab-spire-relay: cannot listen on {}: {e}", guest::LISTEN);
                }
                std::thread::sleep(Duration::from_secs(1));
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

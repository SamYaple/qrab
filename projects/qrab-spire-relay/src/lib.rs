//! Guest relay for the SPIRE agent: `127.0.0.1:8081` multiplexed over the
//! virtio-serial port `org.yapplesauce.spire.0`, which the qrab manager
//! splices to the SPIRE server. See docs/spire-vm.md.
//!
//! [`wire`] is the frame codec shared with the manager.
pub mod guest;
pub mod wire;

use std::path::PathBuf;

pub const USAGE: &str = "qrab-spire-relay [--device PATH] [--max-connections N]

Listens on 127.0.0.1:8081 only and relays each connection to the SPIRE server
over the virtio-serial port named org.yapplesauce.spire.0. Without --device
the port is found through /dev/virtio-ports or, without udev,
/sys/class/virtio-ports. The listen address is fixed.";

#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    pub device: Option<PathBuf>,
    pub max_connections: usize,
}

/// `None` asks for the usage text.
pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<Options>, String> {
    let mut options = Options {
        device: None,
        max_connections: guest::MAX_CONNECTIONS,
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--device" => {
                options.device = Some(args.next().ok_or("--device needs a path")?.into());
            }
            "--max-connections" => {
                options.max_connections = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .filter(|v| (1..=64).contains(v))
                    .ok_or("--max-connections needs a count in 1..=64")?;
            }
            "-h" | "--help" => return Ok(None),
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(Some(options))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<Options>, String> {
        parse_args(args.iter().map(|a| a.to_string()))
    }

    #[test]
    fn only_the_port_and_bound_are_configurable() {
        assert_eq!(
            parse(&["--device", "/dev/vport1p2", "--max-connections", "3"]),
            Ok(Some(Options {
                device: Some("/dev/vport1p2".into()),
                max_connections: 3
            }))
        );
        assert_eq!(parse(&[]).unwrap().unwrap().max_connections, 8);
        for args in [
            &["--listen", "0.0.0.0:8081"][..],
            &["--bind", "10.0.0.1:8081"],
            &["--port", "9000"],
            &["--max-connections", "0"],
            &["--max-connections", "65"],
            &["--device"],
        ] {
            assert!(parse(args).is_err(), "{args:?}");
        }
        assert_eq!(parse(&["--help"]), Ok(None));
    }
}

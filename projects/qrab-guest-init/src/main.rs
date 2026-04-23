//! Thin binary wrapper around [`qrab_guest_init::run_gate`].
//!
//! Two modes:
//!
//! - **Live** (`QRAB_GUEST_INIT_LIVE=1`): emits the final token to
//!   `/dev/console` and, on failure, calls [`qrab_guest_init::halt`]
//!   to hard-halt the VM with no console shell. This is the mode the
//!   UKI image installs as `/sbin/init`.
//!
//! - **Dry run** (default): emits to stderr and exits with code 0 on
//!   `QRAB_INIT_OK`, non-zero otherwise. Safe to run on a dev host so
//!   a stray `cargo run` doesn't brick the workstation.
//!
//! The gate itself is identical in both modes — only the sink and the
//! failure tail differ.

use std::io;
use std::process::ExitCode;

use qrab_guest_init::log::{ConsoleSink, TokenSink};
use qrab_guest_init::{halt, real_probe::RealProbe, run_gate};

fn main() -> ExitCode {
    let live = std::env::var_os("QRAB_GUEST_INIT_LIVE").is_some();
    let probe = RealProbe::live_defaults();

    let (emit_final, exit_code) = if live {
        match open_console_sink() {
            Ok(mut sink) => match run_gate(&probe, &mut sink) {
                Ok(()) => (None, ExitCode::SUCCESS),
                Err(t) => (Some(t), ExitCode::FAILURE),
            },
            Err(e) => {
                eprintln!(
                    "qrab-guest-init: cannot open /dev/console ({e}); halting"
                );
                halt();
            }
        }
    } else {
        let mut sink = ConsoleSink::new(io::stderr());
        match run_gate(&probe, &mut sink) {
            Ok(()) => (None, ExitCode::SUCCESS),
            Err(t) => (Some(t), ExitCode::FAILURE),
        }
    };

    if let Some(t) = emit_final {
        if live {
            // Re-open in case the sink we used above was already
            // moved; we don't expect a second failure here, but if
            // `/dev/console` vanishes we still fall through to halt.
            if let Ok(mut sink) = open_console_sink() {
                let _ = sink.emit(t);
            } else {
                eprintln!("qrab-guest-init: final token {}", t.as_str());
            }
            halt();
        } else {
            eprintln!("qrab-guest-init: would halt on {}", t.as_str());
        }
    }

    exit_code
}

fn open_console_sink() -> io::Result<ConsoleSink<std::fs::File>> {
    let f = qrab_guest_init::log::open_console()?;
    Ok(ConsoleSink::new(f))
}

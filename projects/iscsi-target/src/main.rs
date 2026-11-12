use anyhow::{bail, ensure, Result};
use qrab_iscsi_target::{apply, fence, restore, Backend, LinuxHost, Phase, Report, Spec, Store};

const HELP:&str="qrab-iscsi-target plan|apply|fence|status|handoff --vm-uid UUID [--size-mib 128] [--recipient-uid 1000]\nqrab-iscsi-target bootstrap|restore|install-unit\nqrab-iscsi-target vm-disk-create|vm-disk-clone|vm-disk-expand|vm-disk-export|vm-disk-fence|vm-disk-status|vm-disk-retire|vm-image-publish|vm-image-status|vm-image-retire ... (storage daemon wrapper; vm-disk-export reads CHAP from stdin)\nqrab-iscsi-target vm-image-import --image NAME --image-generation N --source /ABSOLUTE/PATH.raw --sha256 HEX64 (administrator only; never a storage daemon verb)\nFixed dedicated ZFS/LIO paths only. apply/fence/restore/handoff require root. Credentials never go to stdout.";
fn root() -> Result<()> {
    ensure!(
        nix::unistd::getuid().is_root() && nix::unistd::geteuid().is_root(),
        "root is required; use the reviewed installer/operator procedure"
    );
    Ok(())
}
fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or_default();
    let wrapper = qrab_iscsi_target::disk::COMMANDS.contains(&command);
    if wrapper || qrab_iscsi_target::disk::ADMIN_COMMANDS.contains(&command) {
        // Storage-daemon wrapper mode: JSON report on stdout, one JSON refusal
        // line on stderr. The CHAP secret is read only from stdin.
        // Administrator-only commands share the output contract but never
        // the daemon's dispatch table.
        let result = root().and_then(|()| {
            if wrapper {
                qrab_iscsi_target::disk::dispatch(
                    &args,
                    &mut std::io::stdin().lock(),
                    &Store::system(),
                    &mut LinuxHost::new(),
                )
            } else {
                qrab_iscsi_target::disk::admin_dispatch(
                    &args,
                    &Store::system(),
                    &mut LinuxHost::new(),
                )
            }
        });
        return match result {
            Ok(value) => {
                println!("{value}");
                std::process::ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!(
                    "{}",
                    serde_json::json!({
                        "error": qrab_iscsi_target::refusal(&error).unwrap_or("disk-operation-failed"),
                        "message": format!("{error:#}"),
                    })
                );
                std::process::ExitCode::FAILURE
            }
        };
    }
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{HELP}");
        return Ok(());
    }
    let mut uid = None;
    let mut size = 128u64;
    let mut recipient = None;
    let mut fields = args[1..].chunks_exact(2);
    for pair in &mut fields {
        match pair[0].as_str() {
            "--vm-uid" => {
                ensure!(uid.is_none(), "VM UID repeated");
                uid = Some(pair[1].clone());
            }
            "--size-mib" => size = pair[1].parse()?,
            "--recipient-uid" => recipient = Some(pair[1].parse::<u32>()?),
            _ => bail!("unknown option; {HELP}"),
        }
    }
    ensure!(fields.remainder().is_empty(), "option value missing");
    let store = Store::system();
    let mut host = LinuxHost::new();
    if args[0] == "install-unit" {
        root()?;
        ensure!(args.len() == 1, "install-unit accepts no arguments");
        use std::{
            fs,
            io::Write,
            os::unix::fs::{MetadataExt, OpenOptionsExt},
            path::Path,
        };
        const UNIT: &str = include_str!("../../../contrib/storage/qrab-iscsi-target.service");
        let path = Path::new("/etc/systemd/system/qrab-iscsi-target.service");
        match fs::symlink_metadata(path) {
            Ok(meta) => ensure!(
                meta.is_file()
                    && !meta.file_type().is_symlink()
                    && meta.uid() == 0
                    && meta.nlink() == 1
                    && meta.mode() & 0o022 == 0
                    && fs::read_to_string(path)? == UNIT,
                "existing service unit is foreign or changed"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o644)
                    .open(path)?;
                file.write_all(UNIT.as_bytes())?;
                file.sync_all()?;
                fs::File::open(path.parent().unwrap())?.sync_all()?;
            }
            Err(error) => return Err(error.into()),
        }
        return Ok(());
    }
    if args[0] == "bootstrap" {
        root()?;
        ensure!(args.len() == 1, "bootstrap accepts no arguments");
        host.bootstrap()?;
        return Ok(());
    }
    if args[0] == "restore" {
        root()?;
        ensure!(
            uid.is_none() && args.len() == 1,
            "restore accepts no arguments"
        );
        restore(&store, &mut host)?;
        println!("known target states restored; fenced targets remain disabled");
        return Ok(());
    }
    let uid = uid.ok_or_else(|| anyhow::anyhow!("--vm-uid is required"))?;
    let spec = Spec::new(
        uid.clone(),
        size.checked_mul(1024 * 1024)
            .ok_or_else(|| anyhow::anyhow!("size overflow"))?,
    )?;
    match args[0].as_str() {
        "plan" => {
            ensure!(recipient.is_none(), "plan does not export credentials");
            println!(
                "{}",
                serde_json::to_string_pretty(&Report::for_spec(&spec, None, 0))?
            );
        }
        "apply" => {
            root()?;
            println!(
                "{}",
                serde_json::to_string_pretty(&apply(&store, &mut host, spec)?)?
            );
        }
        "fence" => {
            root()?;
            ensure!(recipient.is_none(), "fence does not export credentials");
            println!(
                "{}",
                serde_json::to_string_pretty(&fence(&store, &mut host, &uid)?)?
            );
        }
        "status" => {
            root()?;
            let _lock = store.lock()?;
            let r = store
                .read(&uid)?
                .ok_or_else(|| anyhow::anyhow!("target is not owned"))?;
            match r.phase {
                Phase::Active => host.verify_active(&r)?,
                Phase::Fenced => host.verify_fenced(&r)?,
                _ => {}
            };
            println!("{}", serde_json::to_string_pretty(&r.report())?);
        }
        "handoff" => {
            root()?;
            let _lock = store.lock()?;
            let r = store
                .read(&uid)?
                .ok_or_else(|| anyhow::anyhow!("target is not owned"))?;
            ensure!(
                r.phase == Phase::Active,
                "only active target credentials may be exported"
            );
            host.verify_active(&r)?;
            let path = store.handoff(
                &r,
                recipient.ok_or_else(|| anyhow::anyhow!("--recipient-uid is required"))?,
            )?;
            println!("credential file: {}", path.display());
        }
        _ => bail!("unknown command; {HELP}"),
    }
    Ok(())
}

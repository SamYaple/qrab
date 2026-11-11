//! Test different QEMU display arg combinations by connecting to VNC
//! and reading the ServerInit name to determine which console is shown.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::process::Command;

/// Connect to a VNC Unix socket, do the handshake, return the display name.
async fn vnc_display_name(sock: &std::path::Path) -> Result<String, String> {
    let mut stream = UnixStream::connect(sock)
        .await
        .map_err(|e| format!("connect: {e}"))?;

    // 1. Read server protocol version (12 bytes: "RFB xxx.yyy\n")
    let mut ver = [0u8; 12];
    stream
        .read_exact(&mut ver)
        .await
        .map_err(|e| format!("read version: {e}"))?;
    let ver_str = String::from_utf8_lossy(&ver);

    // 2. Send client protocol version (use same as server)
    stream
        .write_all(&ver)
        .await
        .map_err(|e| format!("write version: {e}"))?;

    // 3. Read security types
    let mut num_types = [0u8; 1];
    stream
        .read_exact(&mut num_types)
        .await
        .map_err(|e| format!("read sec count: {e}"))?;
    let n = num_types[0] as usize;
    if n == 0 {
        // Server sent failure reason
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await.ok();
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut reason = vec![0u8; len];
        stream.read_exact(&mut reason).await.ok();
        return Err(format!(
            "VNC auth failed: {}",
            String::from_utf8_lossy(&reason)
        ));
    }
    let mut types = vec![0u8; n];
    stream
        .read_exact(&mut types)
        .await
        .map_err(|e| format!("read sec types: {e}"))?;

    // 4. Select security type None (1) if available
    if !types.contains(&1) {
        return Err(format!("No 'None' auth type. Available: {types:?}"));
    }
    stream
        .write_all(&[1])
        .await
        .map_err(|e| format!("write sec type: {e}"))?;

    // 5. Read security result (RFB 3.8+)
    if ver_str.contains("003.008") {
        let mut result = [0u8; 4];
        stream
            .read_exact(&mut result)
            .await
            .map_err(|e| format!("read sec result: {e}"))?;
        if u32::from_be_bytes(result) != 0 {
            return Err("Security handshake failed".into());
        }
    }

    // 6. Send ClientInit (shared = 1)
    stream
        .write_all(&[1])
        .await
        .map_err(|e| format!("write clientinit: {e}"))?;

    // 7. Read ServerInit: width(2) + height(2) + pixel_format(16) + name_len(4) + name
    let mut header = [0u8; 24]; // 2+2+16+4
    stream
        .read_exact(&mut header)
        .await
        .map_err(|e| format!("read serverinit: {e}"))?;

    let width = u16::from_be_bytes([header[0], header[1]]);
    let height = u16::from_be_bytes([header[2], header[3]]);
    let name_len = u32::from_be_bytes([header[20], header[21], header[22], header[23]]) as usize;

    let mut name = vec![0u8; name_len];
    stream
        .read_exact(&mut name)
        .await
        .map_err(|e| format!("read name: {e}"))?;
    let name_str = String::from_utf8_lossy(&name).to_string();

    eprintln!("  VNC: {width}x{height} name={name_str:?}");
    Ok(name_str)
}

struct TestVm {
    child: Option<tokio::process::Child>,
    qmp_sock: PathBuf,
    vnc_sock: PathBuf,
}

impl TestVm {
    async fn spawn(extra_args: &[&str]) -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let tmpdir = std::env::temp_dir();
        let qmp_sock = tmpdir.join(format!(
            "qrab-vnc-test-qmp-{}-{}.sock",
            std::process::id(),
            id
        ));
        let vnc_sock = tmpdir.join(format!(
            "qrab-vnc-test-vnc-{}-{}.sock",
            std::process::id(),
            id
        ));

        let _ = tokio::fs::remove_file(&qmp_sock).await;
        let _ = tokio::fs::remove_file(&vnc_sock).await;

        let mut args: Vec<String> = vec![
            "--preconfig".into(),
            "-machine".into(),
            "pc".into(),
            "-m".into(),
            "256M".into(),
        ];
        for a in extra_args {
            args.push(a.to_string());
        }
        args.extend([
            "-qmp".into(),
            format!("unix:{},server=on,wait=off", qmp_sock.display()),
        ]);

        eprintln!("  spawn: qemu-system-x86_64 {}", args.join(" "));
        let child = Command::new("qemu-system-x86_64")
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn qemu");

        // Wait for QMP socket
        for _ in 0..50 {
            if qmp_sock.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(qmp_sock.exists(), "QMP socket never appeared");

        Self {
            child: Some(child),
            qmp_sock,
            vnc_sock,
        }
    }

    async fn start(&self) {
        use qapi_protocol::Qmp;
        let (qmp, _events) = Qmp::new(&self.qmp_sock).await.expect("qmp connect");
        qmp.execute(&qapi::commands::XExitPreconfig {})
            .await
            .expect("exit preconfig");
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Add VGA device dynamically — VNC should pick it up
        let r: String = qmp
            .execute(&qapi::commands::HumanMonitorCommand {
                command_line: "device_add VGA,id=vga0".into(),
                cpu_index: None,
            })
            .await
            .expect("device_add VGA");
        eprintln!("  device_add VGA: {r:?}");
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Probe display state via QMP
        let vnc_info: String = qmp
            .execute(&qapi::commands::HumanMonitorCommand {
                command_line: "info vnc".into(),
                cpu_index: None,
            })
            .await
            .expect("info vnc");
        eprintln!("  info vnc:\n{vnc_info}");

        let display_info: String = qmp
            .execute(&qapi::commands::HumanMonitorCommand {
                command_line: "info qtree".into(),
                cpu_index: None,
            })
            .await
            .expect("info qtree");
        // Just print VGA-related lines
        for line in display_info.lines() {
            let lower = line.to_lowercase();
            if lower.contains("vga") || lower.contains("display") || lower.contains("graphic") {
                eprintln!("  qtree: {line}");
            }
        }

        // Try screendump
        let dump_path = std::env::temp_dir().join("qrab-vnc-screendump.ppm");
        let dump_result: String = qmp
            .execute(&qapi::commands::HumanMonitorCommand {
                command_line: format!("screendump {}", dump_path.display()),
                cpu_index: None,
            })
            .await
            .expect("screendump");
        eprintln!("  screendump: {dump_result:?}");
        if dump_path.exists() {
            let meta = std::fs::metadata(&dump_path).unwrap();
            eprintln!("  screendump file: {} bytes", meta.len());
            // Read first few bytes to check format
            let header = std::fs::read(&dump_path).unwrap();
            let header_str = String::from_utf8_lossy(&header[..header.len().min(100)]);
            eprintln!("  screendump header: {header_str:?}");
            let _ = std::fs::remove_file(&dump_path);
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    async fn cleanup(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.start_kill();
            if let Some(stderr) = child.stderr.take() {
                use tokio::io::AsyncReadExt;
                let mut buf = String::new();
                let mut reader = tokio::io::BufReader::new(stderr);
                let _ = reader.read_to_string(&mut buf).await;
                if !buf.is_empty() {
                    eprintln!("  QEMU stderr: {}", buf.trim());
                }
            }
            let _ = child.wait().await;
        }
        let _ = tokio::fs::remove_file(&self.qmp_sock).await;
        let _ = tokio::fs::remove_file(&self.vnc_sock).await;
    }
}

impl Drop for TestVm {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.start_kill();
        }
        let _ = std::fs::remove_file(&self.qmp_sock);
        let _ = std::fs::remove_file(&self.vnc_sock);
    }
}

/// Test a specific set of display args, return (preconfig_name, running_name).
async fn test_display_args(
    label: &str,
    args: &[&str],
) -> (Result<String, String>, Result<String, String>) {
    eprintln!("\n=== {label} ===");
    eprintln!("  args: {args:?}");

    let mut vm = TestVm::spawn(args).await;

    // Wait for VNC socket
    for _ in 0..30 {
        if vm.vnc_sock.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Check VNC during preconfig
    let preconfig_name = if vm.vnc_sock.exists() {
        eprintln!("  [preconfig]");
        vnc_display_name(&vm.vnc_sock).await
    } else {
        Err("VNC socket never appeared".into())
    };

    // Start VM (exit preconfig)
    vm.start().await;

    // Check VNC after start - reconnect fresh
    let running_name = if vm.vnc_sock.exists() {
        eprintln!("  [running]");
        vnc_display_name(&vm.vnc_sock).await
    } else {
        Err("VNC socket gone after start".into())
    };

    vm.cleanup().await;
    (preconfig_name, running_name)
}

/// After VNC handshake, request a framebuffer update and check we get pixel data.
async fn vnc_has_framebuffer_data(
    sock: &std::path::Path,
) -> Result<(u16, u16, String, bool), String> {
    let mut stream = UnixStream::connect(sock)
        .await
        .map_err(|e| format!("connect: {e}"))?;

    // Handshake
    let mut ver = [0u8; 12];
    stream
        .read_exact(&mut ver)
        .await
        .map_err(|e| format!("read ver: {e}"))?;
    stream
        .write_all(&ver)
        .await
        .map_err(|e| format!("write ver: {e}"))?;

    let mut num_types = [0u8; 1];
    stream
        .read_exact(&mut num_types)
        .await
        .map_err(|e| format!("read sec: {e}"))?;
    let n = num_types[0] as usize;
    let mut types = vec![0u8; n];
    stream
        .read_exact(&mut types)
        .await
        .map_err(|e| format!("read types: {e}"))?;
    stream
        .write_all(&[1])
        .await
        .map_err(|e| format!("write sec: {e}"))?;

    let ver_str = String::from_utf8_lossy(&ver);
    if ver_str.contains("003.008") {
        let mut result = [0u8; 4];
        stream
            .read_exact(&mut result)
            .await
            .map_err(|e| format!("sec result: {e}"))?;
    }

    stream
        .write_all(&[1])
        .await
        .map_err(|e| format!("clientinit: {e}"))?;

    let mut header = [0u8; 24];
    stream
        .read_exact(&mut header)
        .await
        .map_err(|e| format!("serverinit: {e}"))?;

    let width = u16::from_be_bytes([header[0], header[1]]);
    let height = u16::from_be_bytes([header[2], header[3]]);
    let name_len = u32::from_be_bytes([header[20], header[21], header[22], header[23]]) as usize;
    let mut name = vec![0u8; name_len];
    stream
        .read_exact(&mut name)
        .await
        .map_err(|e| format!("name: {e}"))?;
    let name_str = String::from_utf8_lossy(&name).to_string();

    // Request framebuffer update (message type 3)
    // incremental=0, x=0, y=0, w=width, h=height
    let mut req = vec![3u8, 0]; // type=3, incremental=0
    req.extend_from_slice(&0u16.to_be_bytes()); // x
    req.extend_from_slice(&0u16.to_be_bytes()); // y
    req.extend_from_slice(&width.to_be_bytes()); // w
    req.extend_from_slice(&height.to_be_bytes()); // h
    stream
        .write_all(&req)
        .await
        .map_err(|e| format!("fb req: {e}"))?;

    // Read framebuffer update response with timeout
    let mut got_pixels = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    let mut total_bytes = 0usize;

    loop {
        if tokio::time::Instant::now() > deadline {
            break;
        }
        let mut msg_type = [0u8; 1];
        match tokio::time::timeout(Duration::from_millis(500), stream.read_exact(&mut msg_type))
            .await
        {
            Ok(Ok(_)) => {}
            _ => break,
        }

        if msg_type[0] == 0 {
            // FramebufferUpdate
            let mut fb_header = [0u8; 3]; // padding(1) + num_rects(2)
            stream
                .read_exact(&mut fb_header)
                .await
                .map_err(|e| format!("fb header: {e}"))?;
            let num_rects = u16::from_be_bytes([fb_header[1], fb_header[2]]);

            for _ in 0..num_rects {
                let mut rect_header = [0u8; 12]; // x(2)+y(2)+w(2)+h(2)+encoding(4)
                stream
                    .read_exact(&mut rect_header)
                    .await
                    .map_err(|e| format!("rect: {e}"))?;
                let rw = u16::from_be_bytes([rect_header[4], rect_header[5]]) as usize;
                let rh = u16::from_be_bytes([rect_header[6], rect_header[7]]) as usize;
                let encoding = i32::from_be_bytes([
                    rect_header[8],
                    rect_header[9],
                    rect_header[10],
                    rect_header[11],
                ]);

                if encoding == 0 {
                    // Raw encoding: bpp/8 * w * h bytes
                    let bpp = header[4]; // bits-per-pixel from ServerInit pixel format
                    let bytes = (bpp as usize / 8) * rw * rh;
                    let mut pixels = vec![0u8; bytes];
                    stream
                        .read_exact(&mut pixels)
                        .await
                        .map_err(|e| format!("pixels: {e}"))?;
                    total_bytes += bytes;
                    // Check if any non-zero pixels (not all black)
                    if pixels.iter().any(|&b| b != 0) {
                        got_pixels = true;
                    }
                } else {
                    // Skip other encodings for now
                    break;
                }
            }
            break;
        }
    }

    eprintln!(
        "  VNC: {width}x{height} name={name_str:?} pixels={total_bytes} has_content={got_pixels}"
    );
    Ok((width, height, name_str, got_pixels))
}

#[tokio::test]
async fn test_vnc_with_bmc_args() {
    if !manager::Arch::X86_64.is_available() {
        eprintln!("SKIP: qemu-system-x86_64 not available");
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let id = std::process::id();
    let vnc_sock = std::env::temp_dir().join(format!("qrab-vnc-bmc-test-{id}.sock"));
    let serial_log = std::env::temp_dir().join(format!("qrab-serial-bmc-test-{id}.log"));
    let _ = tokio::fs::remove_file(&vnc_sock).await;
    let _ = tokio::fs::remove_file(&serial_log).await;

    // No -nodefaults (breaks VGA graphic console registration).
    // Suppress text consoles that steal VNC display.
    let vnc_display = format!("vnc=unix:{}", vnc_sock.display());
    let serial_path = format!("file:{}", serial_log.display());
    let cirros = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target/test-fixtures/cirros-0.6.2-x86_64-disk.img");
    let drive_arg = format!("file={},format=qcow2,if=virtio", cirros.display());
    // VNC in args but NO VGA. Add VGA dynamically after machine init.
    let vnc_legacy = format!("unix:{}", vnc_sock.display());
    let args = vec![
        "-vnc",
        &vnc_legacy,
        "-parallel",
        "none",
        "-serial",
        &serial_path,
        "-drive",
        &drive_arg,
    ];

    let mut vm = TestVm::spawn(&args).await;
    vm.vnc_sock = vnc_sock.clone();

    for _ in 0..30 {
        if vm.vnc_sock.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Start VM — this also adds VNC via QMP after machine init
    vm.start().await;
    eprintln!("\n=== VNC added post-init via QMP ===");

    // Wait for VNC socket to appear
    for _ in 0..30 {
        if vm.vnc_sock.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Check IMMEDIATELY after start (no delay) — race condition test
    eprintln!("\n=== BMC args (running, 0ms delay) ===");
    let immediate = vnc_has_framebuffer_data(&vm.vnc_sock).await;
    eprintln!("  result: {immediate:?}");

    // Check after 500ms
    tokio::time::sleep(Duration::from_millis(500)).await;
    eprintln!("\n=== BMC args (running, 500ms delay) ===");
    let delayed = vnc_has_framebuffer_data(&vm.vnc_sock).await;
    eprintln!("  result: {delayed:?}");

    // Check after 2s
    tokio::time::sleep(Duration::from_millis(1500)).await;
    eprintln!("\n=== BMC args (running, 2s delay) ===");
    let running = vnc_has_framebuffer_data(&vm.vnc_sock).await;
    eprintln!("  result: {running:?}");

    // Verify graphic console
    if let Ok((_, _, ref name, has_content)) = running {
        assert_eq!(name, "QEMU", "VNC should show graphic console");
        assert!(
            has_content,
            "Framebuffer should have non-zero pixel data after BIOS runs"
        );
    }

    vm.cleanup().await;
    let _ = tokio::fs::remove_file(&serial_log).await;
}

#[tokio::test]
async fn probe_vnc_display_configurations() {
    if !manager::Arch::X86_64.is_available() {
        eprintln!("SKIP: qemu-system-x86_64 not available");
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let configs: Vec<(&str, Vec<&str>)> = vec![
        (
            "A: -display vnc -vga std (no nodefaults)",
            vec!["-display", "VNCSOCK", "-vga", "std"],
        ),
        (
            "B: -display vnc -vga std -nodefaults",
            vec!["-nodefaults", "-display", "VNCSOCK", "-vga", "std"],
        ),
        (
            "C: -display vnc -vga std -parallel null -monitor null",
            vec![
                "-display",
                "VNCSOCK",
                "-vga",
                "std",
                "-parallel",
                "null",
                "-monitor",
                "null",
            ],
        ),
        (
            "D: -display vnc -vga std -parallel none -monitor none",
            vec![
                "-display",
                "VNCSOCK",
                "-vga",
                "std",
                "-parallel",
                "none",
                "-monitor",
                "none",
            ],
        ),
        (
            "E: -vnc sock -vga std (legacy syntax)",
            vec!["-vnc", "VNCSOCK_LEGACY", "-vga", "std"],
        ),
        (
            "F: -vnc sock -vga std -nodefaults",
            vec!["-nodefaults", "-vnc", "VNCSOCK_LEGACY", "-vga", "std"],
        ),
        (
            "G: -device VGA -display vnc",
            vec!["-display", "VNCSOCK", "-device", "VGA"],
        ),
        (
            "H: -device virtio-vga -display vnc",
            vec!["-display", "VNCSOCK", "-device", "virtio-vga"],
        ),
        (
            "I: -nodefaults -device VGA -display vnc",
            vec!["-nodefaults", "-display", "VNCSOCK", "-device", "VGA"],
        ),
    ];

    for (label, template_args) in &configs {
        // Spawn a fresh TestVm for the VNC sock path
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let vnc_sock = std::env::temp_dir().join(format!("qrab-vnc-probe-{}.sock", id));
        let _ = tokio::fs::remove_file(&vnc_sock).await;

        let args: Vec<&str> = template_args
            .iter()
            .map(|a| {
                if *a == "VNCSOCK" {
                    // Will be replaced below
                    *a
                } else if *a == "VNCSOCK_LEGACY" {
                    *a
                } else {
                    *a
                }
            })
            .collect();

        // Build actual args with real socket path
        let vnc_display = format!("vnc=unix:{}", vnc_sock.display());
        let vnc_legacy = format!("unix:{}", vnc_sock.display());
        let real_args: Vec<String> = template_args
            .iter()
            .map(|a| match *a {
                "VNCSOCK" => vnc_display.clone(),
                "VNCSOCK_LEGACY" => vnc_legacy.clone(),
                other => other.to_string(),
            })
            .collect();
        let real_args_ref: Vec<&str> = real_args.iter().map(|s| s.as_str()).collect();

        // Use raw spawn to inject the vnc_sock path
        let mut vm = TestVm::spawn(&real_args_ref).await;
        vm.vnc_sock = vnc_sock.clone();

        for _ in 0..30 {
            if vm.vnc_sock.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let preconfig = if vm.vnc_sock.exists() {
            eprintln!("\n=== {label} ===");
            eprintln!("  [preconfig]");
            vnc_display_name(&vm.vnc_sock).await
        } else {
            eprintln!("\n=== {label} ===");
            Err("VNC socket never appeared".into())
        };

        vm.start().await;

        let running = if vm.vnc_sock.exists() {
            eprintln!("  [running]");
            vnc_display_name(&vm.vnc_sock).await
        } else {
            Err("VNC socket gone".into())
        };

        eprintln!("  PRECONFIG: {preconfig:?}");
        eprintln!("  RUNNING:   {running:?}");

        vm.cleanup().await;
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

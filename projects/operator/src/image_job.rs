//! The image importer's Job containers (`qrab-operator image-job inspect|write`).
//!
//! They hold no Kubernetes token. Each downloads the source over HTTPS,
//! checks the upstream SHA-256 BEFORE anything parses the file, and reports
//! one JSON line through the container's termination message. `write` also
//! converts the image to raw, hashes it and writes it onto the stage volume
//! with qemu-img's userspace iSCSI initiator; the storage host hashes the
//! volume again when it seals it.
use crate::image::ImageFormat;
use anyhow::{bail, ensure, Context as _, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

/// The largest download and raw image a stage accepts (the stage limit).
pub const MAX_IMAGE_BYTES: u64 = 64 << 30;
pub const TERMINATION_LOG: &str = "/dev/termination-log";
pub const WORK_DIR: &str = "/work";
pub const CHAP_FILE: &str = "/run/qrab-stage/password";
pub const QEMU_IMG: &str = "/opt/qemu/bin/qemu-img";

/// What `inspect` reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Inspected {
    pub virtual_size: u64,
}

/// What `write` reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Written {
    pub size_bytes: u64,
    pub sha256: String,
}

/// A failed Job's report; `error` is a short code, `message` plain text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Failure {
    pub error: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub url: String,
    pub sha256: String,
    pub format: ImageFormat,
}

/// Where `write` puts the image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub size_bytes: u64,
    pub portal: String,
    pub target_iqn: String,
    pub initiator_iqn: String,
    pub chap_username: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobArgs {
    Inspect(Source),
    Write(Source, Target),
}

fn flag<'a>(args: &'a [String], name: &str) -> Result<&'a str> {
    let mut found = None;
    for pair in args.chunks(2) {
        ensure!(
            pair.len() == 2 && pair[0].starts_with("--"),
            "usage: image-job inspect|write --flag value ..."
        );
        if pair[0] == name {
            ensure!(found.is_none(), "{name} given twice");
            found = Some(pair[1].as_str());
        }
    }
    found.with_context(|| format!("{name} is required"))
}

fn sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The argv the importer gives a Job container, after `image-job`.
///
/// ```
/// use qrab_operator::image_job::{parse_args, JobArgs};
/// let args: Vec<String> = ["inspect", "--url", "https://example.org/x.img", "--sha256",
///     &"a".repeat(64), "--format", "qcow2"].iter().map(|s| s.to_string()).collect();
/// assert!(matches!(parse_args(&args).unwrap(), JobArgs::Inspect(_)));
/// ```
pub fn parse_args(args: &[String]) -> Result<JobArgs> {
    let (mode, rest) = args
        .split_first()
        .context("usage: image-job inspect|write ...")?;
    let known: &[&str] = match mode.as_str() {
        "inspect" => &["--url", "--sha256", "--format"],
        "write" => &[
            "--url",
            "--sha256",
            "--format",
            "--size-bytes",
            "--portal",
            "--target",
            "--initiator",
            "--chap-user",
        ],
        _ => bail!("usage: image-job inspect|write ..."),
    };
    for pair in rest.chunks(2) {
        ensure!(
            known.contains(&pair[0].as_str()),
            "unknown flag {}",
            pair[0]
        );
    }
    ensure!(
        rest.len() == known.len() * 2,
        "image-job {mode} takes exactly {}",
        known.join(" ")
    );
    let url = flag(rest, "--url")?;
    let parsed = url::Url::parse(url).context("--url")?;
    ensure!(
        parsed.scheme() == "https" && parsed.host_str().is_some(),
        "--url must be HTTPS"
    );
    let sha256 = flag(rest, "--sha256")?;
    ensure!(
        sha256_hex(sha256),
        "--sha256 must be 64 lowercase hex digits"
    );
    let format = match flag(rest, "--format")? {
        "qcow2" => ImageFormat::Qcow2,
        "raw" => ImageFormat::Raw,
        _ => bail!("--format must be qcow2 or raw"),
    };
    let source = Source {
        url: url.into(),
        sha256: sha256.into(),
        format,
    };
    if mode == "inspect" {
        return Ok(JobArgs::Inspect(source));
    }
    let size_bytes: u64 = flag(rest, "--size-bytes")?
        .parse()
        .context("--size-bytes")?;
    ensure!(
        size_bytes > 0 && size_bytes <= MAX_IMAGE_BYTES,
        "--size-bytes out of range"
    );
    let iqn = |name| -> Result<String> {
        let v = flag(rest, name)?;
        ensure!(
            v.starts_with("iqn.")
                && v.len() <= 223
                && v.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b".:-".contains(&b)),
            "{name} must be an IQN"
        );
        Ok(v.into())
    };
    let portal = flag(rest, "--portal")?;
    ensure!(
        portal.parse::<std::net::SocketAddr>().is_ok(),
        "--portal must be IP:port"
    );
    let chap = flag(rest, "--chap-user")?;
    ensure!(
        (1..=64).contains(&chap.len())
            && chap.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
        "--chap-user is invalid"
    );
    Ok(JobArgs::Write(
        source,
        Target {
            size_bytes,
            portal: portal.into(),
            target_iqn: iqn("--target")?,
            initiator_iqn: iqn("--initiator")?,
            chap_username: chap.into(),
        },
    ))
}

/// HTTPS client trusting the Mozilla roots; redirects stay on HTTPS.
fn download_client() -> Result<reqwest::Client> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let tls = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(reqwest::Client::builder()
        .no_proxy()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 10 || attempt.url().scheme() != "https" {
                attempt.error("redirects must stay on HTTPS (at most 10)")
            } else {
                attempt.follow()
            }
        }))
        .connect_timeout(Duration::from_secs(30))
        .read_timeout(Duration::from_secs(120))
        .tls_backend_preconfigured(tls)
        .build()?)
}

/// Downloads `source` to `path`, bounded, and checks its SHA-256.
pub async fn download(source: &Source, path: &Path) -> Result<u64> {
    let mut response = download_client()?
        .get(&source.url)
        .send()
        .await
        .context("download")?
        .error_for_status()
        .context("download")?;
    let mut file = std::fs::File::create(path)?;
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    while let Some(chunk) = response.chunk().await.context("download")? {
        total += chunk.len() as u64;
        ensure!(
            total <= MAX_IMAGE_BYTES,
            "the download exceeds {MAX_IMAGE_BYTES} bytes"
        );
        hasher.update(&chunk);
        file.write_all(&chunk)?;
    }
    file.sync_all()?;
    let digest = hex(&hasher.finalize());
    ensure!(
        digest == source.sha256,
        "sha256-mismatch: the download's SHA-256 is {digest}, not the expected {}",
        source.sha256
    );
    Ok(total)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 and length of a file.
pub fn hash_file(path: &Path) -> Result<(String, u64)> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut total = 0u64;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        total += n as u64;
    }
    Ok((hex(&hasher.finalize()), total))
}

fn qemu_img(args: &[&str]) -> Result<String> {
    let output = Command::new(QEMU_IMG)
        .args(args)
        .output()
        .context("run qemu-img")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "qemu-img {} failed: {}",
            args.first().unwrap_or(&""),
            stderr.chars().take(400).collect::<String>()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Reject qcow2 inputs whose contents depend on anything but the verified
/// download. A checksum of the top-level file does not authenticate a backing
/// chain or an external data file. This is a bounded dependency preflight,
/// not a replacement qcow2 parser: QEMU still validates the image itself.
///
/// The fixed fields follow the pinned QEMU qcow2 v2/v3 header contract:
/// backing offset/length at 8/16 and v3 incompatible features at 72. The
/// external-data-file feature is bit 2. No filename is read or opened here.
fn require_self_contained(mut input: impl Read, format: ImageFormat) -> Result<()> {
    if format == ImageFormat::Raw {
        return Ok(());
    }
    let mut header = [0u8; 104];
    input
        .read_exact(&mut header[..72])
        .context("truncated qcow2 fixed header")?;
    ensure!(&header[..4] == b"QFI\xfb", "invalid qcow2 magic");
    let version = u32::from_be_bytes(header[4..8].try_into().unwrap());
    ensure!(
        matches!(version, 2 | 3),
        "unsupported qcow2 version {version}"
    );
    let backing_offset = u64::from_be_bytes(header[8..16].try_into().unwrap());
    let backing_length = u32::from_be_bytes(header[16..20].try_into().unwrap());
    ensure!(
        backing_offset == 0 && backing_length == 0,
        "qcow2 backing files are not supported; import a standalone flattened image"
    );
    if version == 3 {
        input
            .read_exact(&mut header[72..])
            .context("truncated qcow2 v3 fixed header")?;
        let features = u64::from_be_bytes(header[72..80].try_into().unwrap());
        ensure!(
            features & (1 << 2) == 0,
            "qcow2 external data files are not supported; import a standalone image"
        );
        let length = u32::from_be_bytes(header[100..104].try_into().unwrap());
        ensure!(length >= 104, "invalid qcow2 v3 fixed header length");
    }
    Ok(())
}

/// The virtual size from `qemu-img info`, for a verified self-contained file.
pub fn virtual_size(path: &Path, format: ImageFormat) -> Result<u64> {
    require_self_contained(std::fs::File::open(path)?, format)?;
    let info = qemu_img(&[
        "info",
        "-f",
        format.as_str(),
        "--output=json",
        &path.to_string_lossy(),
    ])?;
    let value: serde_json::Value = serde_json::from_str(&info)?;
    let size = value["virtual-size"]
        .as_u64()
        .context("qemu-img info has no virtual-size")?;
    ensure!(
        size > 0 && size <= MAX_IMAGE_BYTES && size % 512 == 0,
        "unsupported virtual size {size}"
    );
    Ok(size)
}

/// `--target-image-opts` for the stage LUN; the CHAP password is read from
/// a file by a qemu `secret` object, never put in argv.
///
/// ```
/// use qrab_operator::image_job::{target_opts, Target};
/// let t = Target { size_bytes: 512, portal: "10.203.0.1:3260".into(),
///     target_iqn: "iqn.2026-11.com.yapplesauce:qrab.stage.cirros.g2".into(),
///     initiator_iqn: "iqn.2026-11.com.yapplesauce:qrab.importer.cirros.g2".into(),
///     chap_username: "qrab-stage-cirros-g2".into() };
/// let opts = target_opts(&t);
/// assert!(opts.contains("password-secret=chap") && !opts.contains("password="));
/// ```
pub fn target_opts(target: &Target) -> String {
    let (host, port) = target
        .portal
        .rsplit_once(':')
        .unwrap_or((&target.portal, "3260"));
    format!(
        "driver=iscsi,transport=tcp,portal={host}:{port},target={},lun=0,user={},password-secret=chap,initiator-name={}",
        target.target_iqn, target.chap_username, target.initiator_iqn
    )
}

async fn run(args: JobArgs, work: &Path) -> Result<String> {
    let (source, target) = match args {
        JobArgs::Inspect(source) => (source, None),
        JobArgs::Write(source, target) => (source, Some(target)),
    };
    let download = work.join("source");
    download_file(&source, &download).await?;
    let size = virtual_size(&download, source.format)?;
    let Some(target) = target else {
        return Ok(serde_json::to_string(&Inspected { virtual_size: size })?);
    };
    ensure!(
        size == target.size_bytes,
        "the image's virtual size {size} differs from the inspected {}",
        target.size_bytes
    );
    let raw: PathBuf = match source.format {
        ImageFormat::Raw => download.clone(),
        ImageFormat::Qcow2 => {
            let raw = work.join("image.raw");
            qemu_img(&[
                "convert",
                "-f",
                "qcow2",
                "-O",
                "raw",
                &download.to_string_lossy(),
                &raw.to_string_lossy(),
            ])?;
            std::fs::remove_file(&download)?;
            raw
        }
    };
    let (sha256, size_bytes) = hash_file(&raw)?;
    ensure!(
        size_bytes == size,
        "the raw image is {size_bytes} bytes, not {size}"
    );
    ensure!(
        Path::new(CHAP_FILE).is_file(),
        "the stage credential is not mounted"
    );
    qemu_img(&[
        "convert",
        "-n",
        "-f",
        "raw",
        "--object",
        &format!("secret,id=chap,file={CHAP_FILE}"),
        &raw.to_string_lossy(),
        "--target-image-opts",
        &target_opts(&target),
    ])?;
    Ok(serde_json::to_string(&Written { size_bytes, sha256 })?)
}

async fn download_file(source: &Source, path: &Path) -> Result<()> {
    download(source, path).await.map(|_| ())
}

/// Entry point: runs the Job and writes its report to the termination log.
pub async fn main(args: &[String]) -> Result<()> {
    let result = match parse_args(args) {
        Ok(args) => run(args, Path::new(WORK_DIR)).await,
        Err(error) => Err(error),
    };
    let (line, outcome) = match result {
        Ok(line) => (line, Ok(())),
        Err(error) => {
            let message = format!("{error:#}");
            let code = if message.starts_with("sha256-mismatch") {
                "sha256-mismatch"
            } else {
                "import-failed"
            };
            let failure = Failure {
                error: code.into(),
                message: message.chars().take(1000).collect(),
            };
            (serde_json::to_string(&failure)?, Err(error))
        }
    };
    println!("{line}");
    if let Ok(mut log) = std::fs::OpenOptions::new()
        .write(true)
        .open(TERMINATION_LOG)
    {
        let _ = log.write_all(line.as_bytes());
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).to_owned()).collect()
    }

    fn qcow_header(version: u32) -> Vec<u8> {
        let mut header = vec![0; if version == 2 { 72 } else { 104 }];
        header[..4].copy_from_slice(b"QFI\xfb");
        header[4..8].copy_from_slice(&version.to_be_bytes());
        if version != 2 {
            header[100..104].copy_from_slice(&104u32.to_be_bytes());
        }
        header
    }

    #[test]
    fn image_dependencies_are_rejected_before_qemu_can_open_them() {
        for version in [2, 3] {
            let header = qcow_header(version);
            assert!(require_self_contained(header.as_slice(), ImageFormat::Qcow2).is_ok());
            for (offset, bytes) in [
                (8, 1u64.to_be_bytes().to_vec()),
                (16, 1u32.to_be_bytes().to_vec()),
            ] {
                let mut backing = header.clone();
                backing[offset..offset + bytes.len()].copy_from_slice(&bytes);
                assert!(
                    require_self_contained(backing.as_slice(), ImageFormat::Qcow2)
                        .unwrap_err()
                        .to_string()
                        .contains("backing files")
                );
            }
        }
        let mut external = qcow_header(3);
        external[72..80].copy_from_slice(&(1u64 << 2).to_be_bytes());
        assert!(
            require_self_contained(external.as_slice(), ImageFormat::Qcow2)
                .unwrap_err()
                .to_string()
                .contains("external data files")
        );

        // Valid unrelated v3 features (compression and extended L2) do not
        // add dependencies and remain QEMU's format-validation responsibility.
        let mut standalone = qcow_header(3);
        standalone[72..80].copy_from_slice(&((1u64 << 3) | (1 << 4)).to_be_bytes());
        assert!(require_self_contained(standalone.as_slice(), ImageFormat::Qcow2).is_ok());
    }

    #[test]
    fn malformed_qcow_headers_fail_closed_with_bounded_reads() {
        for version in [2, 3] {
            let header = qcow_header(version);
            for n in 0..header.len() {
                assert!(
                    require_self_contained(&header[..n], ImageFormat::Qcow2).is_err(),
                    "v{version} length {n}"
                );
            }
        }
        let mut invalid = qcow_header(3);
        invalid[0] = 0;
        assert!(require_self_contained(invalid.as_slice(), ImageFormat::Qcow2).is_err());
        assert!(require_self_contained(qcow_header(4).as_slice(), ImageFormat::Qcow2).is_err());
        let mut short_header = qcow_header(3);
        short_header[100..104].copy_from_slice(&72u32.to_be_bytes());
        assert!(require_self_contained(short_header.as_slice(), ImageFormat::Qcow2).is_err());

        struct Bounded<'a>(&'a [u8]);
        impl Read for Bounded<'_> {
            fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
                assert!(!self.0.is_empty(), "preflight read beyond the fixed header");
                self.0.read(out)
            }
        }
        assert!(require_self_contained(Bounded(&qcow_header(3)), ImageFormat::Qcow2).is_ok());
        assert!(require_self_contained(Bounded(&[]), ImageFormat::Raw).is_ok());
    }

    #[test]
    fn virtual_size_refuses_external_references_before_starting_qemu_img() {
        let mut random = [0; 16];
        getrandom::fill(&mut random).unwrap();
        let path = std::env::temp_dir().join(format!("qrab-image-preflight-{}", hex(&random)));
        let mut header = qcow_header(3);
        header[8..16].copy_from_slice(&104u64.to_be_bytes());
        header[16..20].copy_from_slice(&16u32.to_be_bytes());
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(&header).unwrap();
        drop(file);
        // This default test does not need qemu-img, a guest or a storage host.
        let error = virtual_size(&path, ImageFormat::Qcow2).unwrap_err();
        std::fs::remove_file(&path).unwrap();
        assert!(error.to_string().contains("backing files"));
    }

    #[test]
    #[ignore = "requires QRAB_TEST_QEMU_IMG pointing to the pinned current qemu-img; local files only"]
    fn real_qcow2_fixtures_keep_the_single_file_boundary() {
        use std::os::unix::fs::DirBuilderExt;
        let binary = std::env::var_os("QRAB_TEST_QEMU_IMG").expect("set QRAB_TEST_QEMU_IMG");
        let mut random = [0; 16];
        getrandom::fill(&mut random).unwrap();
        let root = std::env::temp_dir().join(format!("qrab-qcow2-fixtures-{}", hex(&random)));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let command = |args: &[&str]| {
            let output = Command::new(&binary).args(args).output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        for (name, options) in [
            ("v2", "compat=0.10"),
            ("v3", "compat=1.1"),
            ("extl2", "extended_l2=on"),
        ] {
            let path = root.join(name);
            command(&[
                "create",
                "-q",
                "-f",
                "qcow2",
                "-o",
                options,
                path.to_str().unwrap(),
                "1M",
            ]);
            require_self_contained(std::fs::File::open(path).unwrap(), ImageFormat::Qcow2).unwrap();
        }
        let marker = root.join("outside-download.raw");
        std::fs::write(&marker, vec![0x5a; 1 << 20]).unwrap();
        let overlay = root.join("backing.qcow2");
        command(&[
            "create",
            "-q",
            "-f",
            "qcow2",
            "-b",
            marker.to_str().unwrap(),
            "-F",
            "raw",
            overlay.to_str().unwrap(),
        ]);
        assert!(
            require_self_contained(std::fs::File::open(&overlay).unwrap(), ImageFormat::Qcow2)
                .unwrap_err()
                .to_string()
                .contains("backing files")
        );
        // Demonstrate why hashing only the overlay is insufficient: the
        // normal converter reads the separate benign marker into its output.
        let flattened = root.join("baseline-converted.raw");
        command(&[
            "convert",
            "-f",
            "qcow2",
            "-O",
            "raw",
            overlay.to_str().unwrap(),
            flattened.to_str().unwrap(),
        ]);
        assert_eq!(
            std::fs::read(flattened).unwrap(),
            std::fs::read(marker).unwrap()
        );

        let external = root.join("external.qcow2");
        let data = root.join("external-data.raw");
        let options = format!("data_file={},data_file_raw=on", data.display());
        command(&[
            "create",
            "-q",
            "-f",
            "qcow2",
            "-o",
            &options,
            external.to_str().unwrap(),
            "1M",
        ]);
        assert!(
            require_self_contained(std::fs::File::open(external).unwrap(), ImageFormat::Qcow2)
                .unwrap_err()
                .to_string()
                .contains("external data files")
        );
    }

    #[test]
    fn arguments_are_exact_and_checked() {
        let sha = "a".repeat(64);
        let write = |extra: &[&str]| {
            let mut a = argv(&[
                "write",
                "--url",
                "https://example.org/x.img",
                "--sha256",
                &sha,
                "--format",
                "raw",
                "--size-bytes",
                "1048576",
                "--portal",
                "10.203.0.1:3260",
                "--target",
                "iqn.2026-11.com.yapplesauce:qrab.stage.x.g1",
                "--initiator",
                "iqn.2026-11.com.yapplesauce:qrab.importer.x.g1",
                "--chap-user",
                "qrab-stage-x-g1",
            ]);
            a.extend(extra.iter().map(|s| s.to_string()));
            a
        };
        let JobArgs::Write(source, target) = parse_args(&write(&[])).unwrap() else {
            panic!()
        };
        assert_eq!(
            (source.format, target.size_bytes),
            (ImageFormat::Raw, 1048576)
        );
        assert!(parse_args(&write(&["--extra", "x"])).is_err());
        for (from, to) in [
            ("https://example.org/x.img", "http://example.org/x.img"),
            ("10.203.0.1:3260", "host:3260"),
            ("1048576", "0"),
            ("qrab-stage-x-g1", "bad user"),
            ("iqn.2026-11.com.yapplesauce:qrab.stage.x.g1", "eui.x"),
        ] {
            let bad: Vec<String> = write(&[])
                .into_iter()
                .map(|a| if a == from { to.into() } else { a })
                .collect();
            assert!(parse_args(&bad).is_err(), "{to}");
        }
        let upper = argv(&[
            "inspect",
            "--url",
            "https://e.org/x",
            "--sha256",
            &"A".repeat(64),
            "--format",
            "raw",
        ]);
        assert!(parse_args(&upper).is_err());
        assert!(parse_args(&argv(&["inspect", "--url", "https://e.org/x"])).is_err());
        assert!(parse_args(&argv(&["shell"])).is_err());
    }

    #[test]
    fn reports_round_trip_strictly() {
        let written: Written = serde_json::from_str(r#"{"sizeBytes":512,"sha256":"ab"}"#).unwrap();
        assert_eq!(written.size_bytes, 512);
        assert!(serde_json::from_str::<Inspected>(r#"{"virtualSize":1,"x":2}"#).is_err());
    }

    #[test]
    fn files_hash_in_chunks() {
        let path = std::env::temp_dir().join(format!("qrab-image-job-hash-{}", std::process::id()));
        std::fs::write(&path, vec![7u8; (1 << 20) + 3]).unwrap();
        let (sha, size) = hash_file(&path).unwrap();
        assert_eq!(size, (1 << 20) + 3);
        assert_eq!(sha, hex(&Sha256::digest(vec![7u8; (1 << 20) + 3])));
        std::fs::remove_file(&path).unwrap();
    }
}

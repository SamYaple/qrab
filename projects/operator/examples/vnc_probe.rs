//! Explicit disposable-guest probe through its private VNC socket.
//! Copy to /tmp in the demo Pod and run with kubectl exec; never served by HTTP.
use anyhow::{bail, ensure, Context, Result};
use std::{path::Path, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
};

async fn key(stream: &mut UnixStream, code: u32) -> Result<()> {
    for down in [1, 0] {
        let mut event = vec![4, down, 0, 0];
        event.extend_from_slice(&code.to_be_bytes());
        stream.write_all(&event).await?;
    }
    tokio::time::sleep(Duration::from_millis(40)).await;
    Ok(())
}
async fn execute() -> Result<()> {
    let action = std::env::args()
        .nth(1)
        .context("usage: vnc_probe initialize|verify")?;
    ensure!(
        matches!(action.as_str(), "initialize" | "verify"),
        "only fixed disposable-disk probes are accepted"
    );
    let mut sockets = std::fs::read_dir("/run/qrab")?
        .filter_map(|e| e.ok())
        .map(|e| e.path().join("vnc.sock"))
        .filter(|p| p.exists());
    let socket = sockets.next().context("no private VM console")?;
    ensure!(sockets.next().is_none(), "ambiguous VM runtime");
    ensure!(
        socket.starts_with(Path::new("/run/qrab")),
        "invalid runtime"
    );
    let mut stream = UnixStream::connect(socket).await?;
    let mut banner = [0; 12];
    stream.read_exact(&mut banner).await?;
    ensure!(&banner == b"RFB 003.008\n", "unexpected RFB version");
    stream.write_all(&banner).await?;
    let count = stream.read_u8().await?;
    ensure!(count > 0, "VNC security negotiation failed");
    let mut types = vec![0; count as usize];
    stream.read_exact(&mut types).await?;
    ensure!(
        types.contains(&1),
        "private socket must support local RFB access"
    );
    stream.write_all(&[1]).await?;
    ensure!(stream.read_u32().await? == 0, "VNC handshake rejected");
    stream.write_all(&[1]).await?;
    let mut init = [0; 24];
    stream.read_exact(&mut init).await?;
    let length = u32::from_be_bytes(init[20..24].try_into().unwrap()) as usize;
    ensure!(length < 65536, "invalid desktop name length");
    let mut name = vec![0; length];
    stream.read_exact(&mut name).await?;
    let command = format!("qrab-disk-probe {action} --serial");
    for byte in command.bytes() {
        if !byte.is_ascii_lowercase() && !matches!(byte, b'-' | b' ') {
            bail!("invalid probe command");
        }
        key(&mut stream, byte.into()).await?;
    }
    key(&mut stream, 0xff0d).await?;
    stream.flush().await?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("sent fixed disposable-disk {action} probe through private VNC");
    Ok(())
}
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(20), execute())
        .await
        .context("console probe timed out")?
}

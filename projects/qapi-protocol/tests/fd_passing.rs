//! Descriptors ride SCM_RIGHTS with the frame of the command that takes them
//! (`getfd`), the way QEMU's QMP socket chardev expects.
use nix::sys::socket::{recvmsg, ControlMessageOwned, MsgFlags};
use qapi_protocol::Qmp;
use serde_json::{json, Value};
use std::{
    io::{IoSliceMut, Read, Write},
    os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
};

const GREETING: &str = r#"{"QMP":{"version":{"qemu":{"major":11,"minor":1,"micro":3},"package":""},"capabilities":[]}}"#;

/// Read one line, collecting any descriptors that arrive with its bytes.
async fn line_with_fds(stream: &UnixStream) -> (Value, Vec<OwnedFd>) {
    let mut bytes = Vec::new();
    let mut fds = Vec::new();
    loop {
        let mut chunk = [0u8; 1];
        let mut space = nix::cmsg_space!([RawFd; 4]);
        let n = stream
            .async_io(tokio::io::Interest::READABLE, || {
                let mut iov = [IoSliceMut::new(&mut chunk)];
                let msg = recvmsg::<()>(
                    stream.as_raw_fd(),
                    &mut iov,
                    Some(&mut space),
                    MsgFlags::empty(),
                )
                .map_err(std::io::Error::from)?;
                for cmsg in msg.cmsgs().map_err(std::io::Error::from)? {
                    if let ControlMessageOwned::ScmRights(received) = cmsg {
                        fds.extend(
                            received
                                .into_iter()
                                .map(|fd| unsafe { OwnedFd::from_raw_fd(fd) }),
                        );
                    }
                }
                Ok(msg.bytes)
            })
            .await
            .unwrap();
        assert_eq!(n, 1, "peer closed mid-line");
        if chunk[0] == b'\n' {
            return (serde_json::from_slice(&bytes).unwrap(), fds);
        }
        bytes.push(chunk[0]);
    }
}

async fn send(stream: &mut UnixStream, value: Value) {
    stream
        .write_all(format!("{value}\n").as_bytes())
        .await
        .unwrap();
}

#[tokio::test]
async fn getfd_carries_its_descriptor_and_plain_commands_carry_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("qmp.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        stream
            .write_all(format!("{GREETING}\n").as_bytes())
            .await
            .unwrap();
        let (capabilities, fds) = line_with_fds(&stream).await;
        assert_eq!(capabilities["execute"], "qmp_capabilities");
        assert!(fds.is_empty());
        send(&mut stream, json!({"return":{},"id":capabilities["id"]})).await;

        let (getfd, fds) = line_with_fds(&stream).await;
        assert_eq!(getfd["execute"], "getfd");
        assert_eq!(getfd["arguments"]["fdname"], "nic-lan-tap0");
        assert_eq!(fds.len(), 1, "exactly the getfd descriptor");
        // The received descriptor is the client's pipe: read through it,
        // then close it before answering.
        let mut read = std::fs::File::from(fds.into_iter().next().unwrap());
        let mut text = [0u8; 22];
        read.read_exact(&mut text).unwrap();
        assert_eq!(&text, b"through the descriptor");
        drop(read);
        send(&mut stream, json!({"return":{},"id":getfd["id"]})).await;

        let (plain, fds) = line_with_fds(&stream).await;
        assert_eq!(plain["execute"], "query-status");
        assert!(fds.is_empty(), "a plain command carries no descriptors");
        send(
            &mut stream,
            json!({"return":{"status":"running","running":true},"id":plain["id"]}),
        )
        .await;

        // Keep the stream open until the client is done.
        let mut rest = String::new();
        let _ = BufReader::new(stream).read_line(&mut rest).await;
    });

    let (qmp, _events) = Qmp::new(&path).await.unwrap();
    let (read, write) = nix::unistd::pipe().unwrap();
    let mut write = std::fs::File::from(write);
    write.write_all(b"through the descriptor").unwrap();
    qmp.getfd("nic-lan-tap0", read).await.unwrap();
    // The server closed its copy before answering; the client closed its own
    // once sent, so no reader is left.
    let error = write.write_all(b"x").unwrap_err();
    assert_eq!(
        error.raw_os_error(),
        Some(nix::libc::EPIPE),
        "the client kept the descriptor"
    );
    let status = qmp
        .execute_raw(json!({"execute":"query-status"}))
        .await
        .unwrap();
    assert_eq!(status["status"], "running");
    drop(qmp);
    server.await.unwrap();
}

#[tokio::test]
async fn descriptors_of_a_refused_request_are_closed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("qmp.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        stream
            .write_all(format!("{GREETING}\n").as_bytes())
            .await
            .unwrap();
        let (capabilities, _) = line_with_fds(&stream).await;
        send(&mut stream, json!({"return":{},"id":capabilities["id"]})).await;
        let mut rest = String::new();
        let _ = BufReader::new(stream).read_line(&mut rest).await;
    });
    let (qmp, _events) = Qmp::new(&path).await.unwrap();
    let (read, write) = nix::unistd::pipe().unwrap();
    // An invalid envelope is refused before any I/O, and the descriptor goes with it.
    assert!(qmp
        .execute_raw_with_fds(json!({"execute":"getfd","id":7}), vec![read])
        .await
        .is_err());
    // With the only reader closed, writing to the pipe fails with EPIPE.
    let mut write = std::fs::File::from(write);
    let error = write.write_all(b"x").unwrap_err();
    assert_eq!(error.raw_os_error(), Some(nix::libc::EPIPE));
    drop(qmp);
    server.await.unwrap();
}

//! Bounded local API handoff: verify the logical owner, then address the
//! immutable CNI attachment rather than looking the reusable name up again.
use crate::authz;
use std::{
    io::{self, Read, Write},
    path::Path,
    time::{Duration, Instant},
};

const MAX_BODY: usize = 1 << 20;
const MAX_HEADERS: usize = 16 << 10;

pub fn handoff(
    socket: &Path,
    endpoint: &str,
    owner: &str,
    nic: &str,
    state: &str,
) -> io::Result<()> {
    if !matches!(state, "prepare" | "active" | "standby") {
        return Err(invalid("invalid handoff state"));
    }
    let name = endpoint
        .strip_prefix("cep-name:")
        .ok_or_else(|| invalid("handoff lookup needs a CEP name"))?;
    let (namespace, pod) = name
        .split_once('/')
        .ok_or_else(|| invalid("handoff lookup needs namespace/name"))?;
    if namespace.is_empty()
        || pod.is_empty()
        || endpoint.len() > 512
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._/".contains(&b))
    {
        return Err(invalid("invalid handoff endpoint name"));
    }
    let body = request(
        socket,
        "GET",
        &format!("/v1/endpoint/{}", escape(endpoint)),
        &[],
        Duration::from_secs(5),
    )?;
    let endpoint: serde_json::Value =
        serde_json::from_slice(&body).map_err(|_| invalid("invalid Cilium endpoint response"))?;
    let status = endpoint
        .get("status")
        .ok_or_else(|| invalid("Cilium endpoint has no status"))?;
    let labels = status
        .get("identity")
        .and_then(|i| i.get("labels"))
        .and_then(|l| l.as_array())
        .ok_or_else(|| invalid("Cilium endpoint has no verified identity labels"))?;
    for (key, value) in [
        (authz::NAME_LABEL, "qrab"),
        (authz::COMPONENT_LABEL, authz::NIC_COMPONENT),
        (authz::OWNER_UID, owner),
        (authz::NIC_LABEL, nic),
        ("io.kubernetes.pod.namespace", namespace),
    ] {
        let prefix = format!("k8s:{key}=");
        let expected = format!("{prefix}{value}");
        let mut values = labels
            .iter()
            .filter_map(|l| l.as_str())
            .filter(|l| l.starts_with(&prefix));
        if values.next() != Some(expected.as_str()) || values.next().is_some() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Cilium endpoint does not belong to the authorized VM/NIC",
            ));
        }
    }
    let attachment = status
        .get("external-identifiers")
        .and_then(|e| e.get("cni-attachment-id"))
        .and_then(|id| id.as_str())
        .filter(|id| {
            !id.is_empty()
                && id.len() <= 256
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-._:".contains(&b))
        })
        .ok_or_else(|| invalid("Cilium endpoint has no unique CNI attachment ID"))?;
    let path = format!(
        "/v1/endpoint/cni-attachment-id:{}/handoff",
        escape(attachment)
    );
    let body = serde_json::to_vec(&serde_json::json!({"state":state}))?;
    // Preparation may regenerate for a minute; activation uses the same
    // pinned attachment. No numeric or deprecated container-ID fallback.
    request(socket, "PATCH", &path, &body, Duration::from_secs(90))?;
    Ok(())
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn escape(value: &str) -> String {
    let mut escaped = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~:".contains(&byte) {
            escaped.push(char::from(byte));
        } else {
            use std::fmt::Write;
            let _ = write!(escaped, "%{byte:02X}");
        }
    }
    escaped
}

fn request(
    socket: &Path,
    method: &str,
    path: &str,
    body: &[u8],
    deadline: Duration,
) -> io::Result<Vec<u8>> {
    let start = Instant::now();
    let mut stream = super::connect_local(socket)?;
    stream.set_write_timeout(Some(deadline))?;
    // HTTP/1.0 + close avoids implicit chunked bodies from streaming JSON
    // encoders. Explicit unexpected transfer encodings are refused below.
    write!(stream, "{method} {path} HTTP/1.0\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len())?;
    stream.write_all(body)?;
    let mut bytes = Vec::new();
    let mut head = None;
    let mut expected = None;
    let mut code = 0;
    loop {
        let remaining = deadline
            .checked_sub(start.elapsed())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::TimedOut, "Cilium API deadline elapsed")
            })?;
        stream.set_read_timeout(Some(remaining))?;
        let mut chunk = [0u8; 1024];
        let read = match stream.read(&mut chunk) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..read]);
        if head.is_none() {
            if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                if end > MAX_HEADERS {
                    return Err(invalid("Cilium API headers too large"));
                }
                let header = std::str::from_utf8(&bytes[..end])
                    .map_err(|_| invalid("invalid Cilium HTTP headers"))?;
                let mut lines = header.split("\r\n");
                let first = lines.next().unwrap_or_default();
                let mut parts = first.split_whitespace();
                if !matches!(parts.next(), Some("HTTP/1.0" | "HTTP/1.1")) {
                    return Err(invalid("invalid Cilium HTTP version"));
                }
                code = parts
                    .next()
                    .filter(|s| s.len() == 3)
                    .and_then(|s| s.parse::<u16>().ok())
                    .filter(|c| (100..600).contains(c))
                    .ok_or_else(|| invalid("invalid Cilium HTTP status"))?;
                for line in lines {
                    let (name, value) = line
                        .split_once(':')
                        .ok_or_else(|| invalid("invalid Cilium HTTP header"))?;
                    if name.eq_ignore_ascii_case("transfer-encoding") {
                        return Err(invalid("unexpected Cilium transfer encoding"));
                    }
                    if name.eq_ignore_ascii_case("content-length") {
                        if expected.is_some() {
                            return Err(invalid("duplicate Cilium content length"));
                        }
                        let length = value
                            .trim()
                            .parse::<usize>()
                            .map_err(|_| invalid("invalid Cilium content length"))?;
                        if length > MAX_BODY {
                            return Err(invalid("Cilium API body too large"));
                        }
                        expected = Some(length);
                    }
                }
                head = Some(end + 4);
            } else if bytes.len() > MAX_HEADERS {
                return Err(invalid("Cilium API headers too large"));
            }
        }
        if let Some(offset) = head {
            if bytes.len() - offset > MAX_BODY {
                return Err(invalid("Cilium API body too large"));
            }
            if let Some(length) = expected {
                if bytes.len() - offset >= length {
                    bytes.truncate(offset + length);
                    break;
                }
            }
        }
    }
    let offset = head.ok_or_else(|| {
        io::Error::new(io::ErrorKind::UnexpectedEof, "missing Cilium HTTP response")
    })?;
    if expected.is_some_and(|length| bytes.len() - offset != length) {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "truncated Cilium HTTP body",
        ));
    }
    let body = bytes.split_off(offset);
    if !(200..300).contains(&code) {
        let detail = String::from_utf8_lossy(&body[..body.len().min(512)]);
        return Err(io::Error::other(format!(
            "Cilium API {method}: HTTP {code} {detail}"
        )));
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use std::{
        io::{BufRead, BufReader},
        os::unix::net::{UnixListener, UnixStream},
    };

    fn endpoint() -> Value {
        json!({"status":{"external-identifiers":{"cni-attachment-id":"attachment-a:eth0"},
        "identity":{"labels":[
            "k8s:app.kubernetes.io/name=qrab", "k8s:app.kubernetes.io/component=vm-nic",
            "k8s:qrab.yapplesauce.com/owner-uid=vm-a", "k8s:qrab.yapplesauce.com/nic=lan",
            "k8s:io.kubernetes.pod.namespace=qrab"
        ]}}})
    }

    fn receive(stream: &UnixStream) -> (String, Vec<u8>) {
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut first = String::new();
        reader.read_line(&mut first).unwrap();
        let mut length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse::<usize>().unwrap();
            }
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        (first, body)
    }

    fn reply(stream: &mut UnixStream, code: &str, body: &[u8]) {
        write!(
            stream,
            "HTTP/1.0 {code}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(body).unwrap();
    }

    struct LookupServer {
        directory: tempfile::TempDir,
        done: Arc<AtomicBool>,
        worker: std::thread::JoinHandle<Option<(String, &'static str)>>,
    }

    fn lookup_server(description: Value) -> LookupServer {
        let dir = tempfile::tempdir().unwrap();
        let listener = UnixListener::bind(dir.path().join("api.sock")).unwrap();
        let done = Arc::new(AtomicBool::new(false));
        let finished = done.clone();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let (first, _) = receive(&stream);
            assert!(
                first.starts_with("GET /v1/endpoint/cep-name:qrab%2Fguest HTTP/1.0"),
                "{first}"
            );
            reply(
                &mut stream,
                "200 OK",
                &serde_json::to_vec(&description).unwrap(),
            );
            drop(stream);
            // Keep the API available: a missing guard must reach a successful
            // mutation, rather than merely fail on a refused connection.
            listener.set_nonblocking(true).unwrap();
            // The verified name is now rebound to attachment B.
            let name_now_points_to = "attachment-b";
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let (first, body) = receive(&stream);
                        assert_eq!(
                            serde_json::from_slice::<Value>(&body).unwrap(),
                            json!({"state":"active"})
                        );
                        reply(&mut stream, "200 OK", b"{}");
                        let selected =
                            if first.contains("/cni-attachment-id:attachment-a:eth0/handoff") {
                                "attachment-a"
                            } else {
                                name_now_points_to
                            };
                        return Some((first, selected));
                    }
                    Err(e)
                        if e.kind() == io::ErrorKind::WouldBlock
                            && !finished.load(Ordering::Relaxed) =>
                    {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => return None,
                    Err(e) => panic!("{e}"),
                }
            }
        });
        LookupServer {
            directory: dir,
            done,
            worker,
        }
    }

    #[test]
    fn a_name_replaced_after_verification_does_not_retarget_the_mutation() {
        let LookupServer {
            directory: dir,
            done,
            worker,
        } = lookup_server(endpoint());
        let result = handoff(
            &dir.path().join("api.sock"),
            "cep-name:qrab/guest",
            "vm-a",
            "lan",
            "active",
        );
        done.store(true, Ordering::Relaxed);
        let (patch, selected) = worker.join().unwrap().unwrap();
        result.unwrap();
        assert_eq!(selected, "attachment-a", "the reusable name now selects B");
        // Only immutable attachment A was verified.
        assert!(
            patch.starts_with(
                "PATCH /v1/endpoint/cni-attachment-id:attachment-a:eth0/handoff HTTP/1.0"
            ),
            "{patch}"
        );
        assert!(!patch.contains("cep-name:"));
    }

    #[test]
    fn foreign_or_incomplete_identity_never_reaches_a_patch() {
        for defect in [
            "owner",
            "nic",
            "namespace",
            "component",
            "duplicate",
            "missing-id",
            "deprecated-id",
        ] {
            let mut description = endpoint();
            match defect {
                "owner" => {
                    description["status"]["identity"]["labels"][2] =
                        "k8s:qrab.yapplesauce.com/owner-uid=vm-b".into()
                }
                "nic" => {
                    description["status"]["identity"]["labels"][3] =
                        "k8s:qrab.yapplesauce.com/nic=wan".into()
                }
                "namespace" => {
                    description["status"]["identity"]["labels"][4] =
                        "k8s:io.kubernetes.pod.namespace=other".into()
                }
                "component" => {
                    description["status"]["identity"]["labels"][1] =
                        "k8s:app.kubernetes.io/component=vm".into()
                }
                "duplicate" => description["status"]["identity"]["labels"]
                    .as_array_mut()
                    .unwrap()
                    .push("k8s:qrab.yapplesauce.com/owner-uid=vm-b".into()),
                "missing-id" => {
                    description["status"]["external-identifiers"]
                        .as_object_mut()
                        .unwrap()
                        .clear();
                }
                "deprecated-id" => {
                    description["status"]["external-identifiers"] =
                        json!({"container-id":"old-id"});
                    description["id"] = 77.into();
                }
                _ => unreachable!(),
            }
            let LookupServer {
                directory: dir,
                done,
                worker,
            } = lookup_server(description);
            let result = handoff(
                &dir.path().join("api.sock"),
                "cep-name:qrab/guest",
                "vm-a",
                "lan",
                "active",
            );
            done.store(true, Ordering::Relaxed);
            let patch = worker.join().unwrap();
            assert!(result.is_err(), "{defect}");
            assert!(patch.is_none(), "{defect} reached mutation: {patch:?}");
        }
    }

    fn raw_server(response: Vec<u8>) -> (tempfile::TempDir, std::thread::JoinHandle<()>) {
        let dir = tempfile::tempdir().unwrap();
        let listener = UnixListener::bind(dir.path().join("api.sock")).unwrap();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = receive(&stream);
            let _ = stream.write_all(&response);
        });
        (dir, worker)
    }

    #[test]
    fn local_http_framing_and_errors_are_bounded() {
        for response in [
            b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\n{}".to_vec(),
            b"HTTP/1.0 200 OK\r\n\r\n{}".to_vec(),
        ] {
            let (dir, worker) = raw_server(response);
            assert_eq!(
                request(
                    &dir.path().join("api.sock"),
                    "GET",
                    "/",
                    &[],
                    Duration::from_secs(1)
                )
                .unwrap(),
                b"{}"
            );
            worker.join().unwrap();
        }
        for response in [
            b"HTTP/1.0 200 OK\r\nContent-Length: 10\r\n\r\n{}".to_vec(),
            format!(
                "HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n",
                MAX_BODY + 1
            )
            .into_bytes(),
            b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\n{}".to_vec(),
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}\r\n0\r\n\r\n".to_vec(),
            b"HTTP/1.0 invalid\r\n\r\n".to_vec(),
            b"HTTP/1.0 409 Conflict\r\nContent-Length: 5\r\n\r\nnope!".to_vec(),
            vec![b'x'; MAX_HEADERS + 1],
        ] {
            let (dir, worker) = raw_server(response);
            assert!(request(
                &dir.path().join("api.sock"),
                "GET",
                "/",
                &[],
                Duration::from_secs(1)
            )
            .is_err());
            worker.join().unwrap();
        }
    }

    #[test]
    fn a_dribbling_api_cannot_extend_the_total_deadline() {
        let dir = tempfile::tempdir().unwrap();
        let listener = UnixListener::bind(dir.path().join("api.sock")).unwrap();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = receive(&stream);
            for byte in b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\n{}" {
                if stream.write_all(&[*byte]).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        let start = Instant::now();
        let error = request(
            &dir.path().join("api.sock"),
            "GET",
            "/",
            &[],
            Duration::from_millis(100),
        )
        .unwrap_err();
        assert!(matches!(
            error.kind(),
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
        ));
        assert!(start.elapsed() < Duration::from_secs(1));
        worker.join().unwrap();
    }
}

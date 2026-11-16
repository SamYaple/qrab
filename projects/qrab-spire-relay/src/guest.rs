//! Guest side: accepts loopback TCP connections and multiplexes them over the
//! virtio-serial port with [`crate::wire`], one reader and one writer thread
//! per connection.
use crate::wire::{Decoder, Frame, MAX_PAYLOAD, WINDOW};
use std::{
    collections::{HashMap, VecDeque},
    io::{self, Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    sync::{Arc, Condvar, Mutex, MutexGuard},
    time::Duration,
};

/// The only address the relay serves: the SPIRE agent's `server_address`.
pub const LISTEN: SocketAddr = SocketAddr::V4(std::net::SocketAddrV4::new(
    std::net::Ipv4Addr::LOCALHOST,
    8081,
));
/// Default bound on concurrent connections.
pub const MAX_CONNECTIONS: usize = 8;
/// A guest client that stops reading is dropped after this long.
const WRITE_TIMEOUT: Duration = Duration::from_secs(60);

pub struct Relay {
    shared: Arc<Shared>,
    reset_sent: Mutex<bool>,
}

struct Shared {
    state: Mutex<State>,
    credit: Condvar,
    port: Mutex<Option<Box<dyn Write + Send>>>,
    max_connections: usize,
}

#[derive(Default)]
struct State {
    /// A host session is acknowledged; connections may open.
    ready: bool,
    next_id: u32,
    conns: HashMap<u32, Conn>,
}

struct Conn {
    stream: TcpStream,
    /// Bytes the host still accepts from this connection.
    credit: u32,
    inbound: Arc<Inbound>,
}

/// Host data waiting for the guest client; at most [`WINDOW`] bytes.
#[derive(Default)]
struct Inbound {
    queue: Mutex<(VecDeque<u8>, bool)>,
    ready: Condvar,
}

impl Inbound {
    fn finish(&self, discard: bool) {
        let mut queue = self.queue.lock().unwrap();
        if discard {
            queue.0.clear();
        }
        queue.1 = true;
        self.ready.notify_all();
    }
}

impl Conn {
    /// Wakes both connection threads; `discard` drops undelivered host data.
    fn close(self, discard: bool) {
        self.inbound.finish(discard);
        let _ = self.stream.shutdown(if discard {
            Shutdown::Both
        } else {
            Shutdown::Read
        });
    }
}

impl Relay {
    /// Serves connections accepted on `listener`, which must be bound to a
    /// loopback address: the relay never listens beyond the guest itself.
    pub fn new(listener: TcpListener, max_connections: usize) -> io::Result<Self> {
        let local = listener.local_addr()?;
        if !local.ip().is_loopback() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("refusing non-loopback listener {local}"),
            ));
        }
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                next_id: 1,
                ..State::default()
            }),
            credit: Condvar::new(),
            port: Mutex::new(None),
            max_connections: max_connections.max(1),
        });
        let accepting = shared.clone();
        std::thread::spawn(move || accept(listener, accepting));
        Ok(Self {
            shared,
            reset_sent: Mutex::new(false),
        })
    }

    /// Serves one host session until the port reads EOF (the host side
    /// disconnected) or fails. Every connection is closed when it returns.
    pub fn serve(
        &self,
        mut port: impl Read,
        writer: impl Write + Send + 'static,
    ) -> io::Result<()> {
        *self.shared.port.lock().unwrap() = Some(Box::new(writer));
        // A restarted relay asks the host for a session once; a reconnected
        // host starts one by itself.
        if !std::mem::replace(&mut *self.reset_sent.lock().unwrap(), true) {
            self.shared.send(&Frame::Reset);
        }
        let mut decoder = Decoder::default();
        let mut buf = vec![0; MAX_PAYLOAD];
        let result = loop {
            let n = match port.read(&mut buf) {
                Ok(0) => break Ok(()),
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => break Err(e),
            };
            for frame in decoder.feed(&buf[..n]) {
                match frame {
                    Ok(frame) => self.shared.handle(frame),
                    Err(e) => eprintln!("qrab-spire-relay: {e}"),
                }
            }
        };
        let mut state = self.shared.state();
        state.ready = false;
        self.shared.reset(&mut state);
        result
    }

    #[cfg(test)]
    fn connections(&self) -> usize {
        self.shared.state().conns.len()
    }
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap()
    }

    fn send(&self, frame: &Frame) -> bool {
        let mut port = self.port.lock().unwrap();
        let Some(writer) = port.as_mut() else {
            return false;
        };
        let sent = writer
            .write_all(&frame.encode())
            .and_then(|()| writer.flush());
        sent.is_ok()
    }

    fn reset(&self, state: &mut State) {
        for (_, conn) in state.conns.drain() {
            conn.close(true);
        }
        self.credit.notify_all();
    }

    /// Removes `id`, waking its threads; true if it was still open.
    fn remove(&self, id: u32, discard: bool) -> bool {
        let conn = self.state().conns.remove(&id);
        self.credit.notify_all();
        conn.map(|conn| conn.close(discard)).is_some()
    }

    fn handle(&self, frame: Frame) {
        match frame {
            Frame::Hello(session) => {
                // Reset, readiness and the acknowledgement are one step, so
                // no Open can precede the Ack on the wire.
                let mut state = self.state();
                self.reset(&mut state);
                state.ready = self.send(&Frame::Ack(session));
            }
            Frame::Data(id, data) => {
                let state = self.state();
                let Some(conn) = state.conns.get(&id) else {
                    return;
                };
                let inbound = conn.inbound.clone();
                drop(state);
                let mut queue = inbound.queue.lock().unwrap();
                if queue.0.len() + data.len() > WINDOW as usize {
                    drop(queue);
                    // The host exceeded its credit; never buffer beyond it.
                    if self.remove(id, true) {
                        self.send(&Frame::Close(id));
                    }
                    return;
                }
                queue.0.extend(data);
                inbound.ready.notify_all();
            }
            Frame::Close(id) => {
                self.remove(id, false);
            }
            Frame::Credit(id, bytes) => {
                if let Some(conn) = self.state().conns.get_mut(&id) {
                    conn.credit = conn.credit.saturating_add(bytes).min(WINDOW);
                }
                self.credit.notify_all();
            }
            Frame::Ack(_) | Frame::Reset | Frame::Open(_) => {}
        }
    }
}

fn accept(listener: TcpListener, shared: Arc<Shared>) {
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("qrab-spire-relay: accept: {e}");
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
        };
        if let Err(e) = open(&shared, stream) {
            eprintln!("qrab-spire-relay: connection: {e}");
        }
    }
}

/// Registers an accepted connection. Without a host session, or at the
/// connection bound, the stream is closed at once and the client retries.
fn open(shared: &Arc<Shared>, stream: TcpStream) -> io::Result<()> {
    let mut state = shared.state();
    if !state.ready || state.conns.len() >= shared.max_connections {
        return Ok(());
    }
    stream.set_nodelay(true)?;
    stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
    let (reader, writer) = (stream.try_clone()?, stream.try_clone()?);
    let id = state.next_id;
    state.next_id = state.next_id.checked_add(1).unwrap_or(1);
    let inbound = Arc::new(Inbound::default());
    state.conns.insert(
        id,
        Conn {
            stream,
            credit: WINDOW,
            inbound: inbound.clone(),
        },
    );
    // Sent under the state lock, so a Hello handled meanwhile orders after it.
    if !shared.send(&Frame::Open(id)) {
        if let Some(conn) = state.conns.remove(&id) {
            conn.close(true);
        }
        return Ok(());
    }
    drop(state);
    let (up, down) = (shared.clone(), shared.clone());
    std::thread::spawn(move || to_host(&up, id, reader));
    std::thread::spawn(move || to_client(&down, id, writer, &inbound));
    Ok(())
}

/// Client to host, within the credit the host granted.
fn to_host(shared: &Shared, id: u32, mut stream: TcpStream) {
    let mut buf = vec![0; MAX_PAYLOAD];
    loop {
        let grant = {
            let mut state = shared.state();
            loop {
                match state.conns.get(&id) {
                    None => return,
                    Some(conn) if conn.credit > 0 => break conn.credit as usize,
                    Some(_) => state = shared.credit.wait(state).unwrap(),
                }
            }
        };
        let n = match stream.read(&mut buf[..grant.min(MAX_PAYLOAD)]) {
            Ok(0) | Err(_) => {
                if shared.remove(id, true) {
                    shared.send(&Frame::Close(id));
                }
                return;
            }
            Ok(n) => n,
        };
        match shared.state().conns.get_mut(&id) {
            Some(conn) => conn.credit -= n as u32,
            None => return,
        }
        shared.send(&Frame::Data(id, buf[..n].to_vec()));
    }
}

/// Host to client; each delivered chunk returns its credit to the host.
fn to_client(shared: &Shared, id: u32, mut stream: TcpStream, inbound: &Inbound) {
    loop {
        let chunk: Vec<u8> = {
            let mut queue = inbound.queue.lock().unwrap();
            while queue.0.is_empty() && !queue.1 {
                queue = inbound.ready.wait(queue).unwrap();
            }
            if queue.0.is_empty() {
                let _ = stream.shutdown(Shutdown::Both);
                return;
            }
            let n = queue.0.len().min(MAX_PAYLOAD);
            queue.0.drain(..n).collect()
        };
        if stream.write_all(&chunk).is_err() {
            if shared.remove(id, true) {
                shared.send(&Frame::Close(id));
            }
            return;
        }
        // A closed connection only drains; its credit is meaningless.
        if shared.state().conns.contains_key(&id) {
            shared.send(&Frame::Credit(id, chunk.len() as u32));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;

    /// A synchronous fake host speaking the wire format.
    struct Host {
        stream: UnixStream,
        decoder: Decoder,
        pending: VecDeque<Frame>,
    }

    impl Host {
        fn new(stream: UnixStream) -> Self {
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            Self {
                stream,
                decoder: Decoder::default(),
                pending: VecDeque::new(),
            }
        }

        fn send(&mut self, frame: Frame) {
            self.stream.write_all(&frame.encode()).unwrap();
        }

        fn recv(&mut self) -> Frame {
            self.try_recv().expect("a frame from the guest")
        }

        fn try_recv(&mut self) -> Option<Frame> {
            let mut buf = [0; 4096];
            while self.pending.is_empty() {
                let n = self.stream.read(&mut buf).ok()?;
                if n == 0 {
                    return None;
                }
                let frames: Vec<_> = self.decoder.feed(&buf[..n]).map(Result::unwrap).collect();
                self.pending.extend(frames);
            }
            self.pending.pop_front()
        }

        /// Starts a session and returns once the guest acknowledged it.
        fn hello(&mut self, session: u32) {
            self.send(Frame::Hello(session));
            loop {
                match self.recv() {
                    Frame::Ack(s) if s == session => return,
                    Frame::Reset | Frame::Ack(_) => {}
                    other => panic!("unexpected {other:?} before Ack"),
                }
            }
        }
    }

    fn relay() -> (Arc<Relay>, SocketAddr) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        (Arc::new(Relay::new(listener, 2).unwrap()), addr)
    }

    fn session(relay: &Arc<Relay>) -> (Host, std::thread::JoinHandle<io::Result<()>>) {
        let (host, guest) = UnixStream::pair().unwrap();
        let relay = relay.clone();
        let serving = std::thread::spawn(move || relay.serve(guest.try_clone().unwrap(), guest));
        (Host::new(host), serving)
    }

    fn connect(addr: SocketAddr) -> TcpStream {
        let stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
    }

    fn closed(stream: &mut TcpStream) -> bool {
        matches!(stream.read(&mut [0; 16]), Ok(0) | Err(_))
    }

    #[test]
    fn the_listener_must_be_loopback() {
        let any = TcpListener::bind("0.0.0.0:0").unwrap();
        let error = Relay::new(any, 1).err().expect("non-loopback is refused");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(LISTEN.ip().is_loopback());
        assert_eq!(LISTEN.to_string(), "127.0.0.1:8081");
    }

    #[test]
    fn connections_are_refused_until_a_host_session_is_acknowledged() {
        let (relay, addr) = relay();
        let (mut host, _serving) = session(&relay);
        assert_eq!(host.recv(), Frame::Reset);
        let mut early = connect(addr);
        assert!(closed(&mut early));
        host.hello(7);
        let _client = connect(addr);
        assert_eq!(host.recv(), Frame::Open(1));
    }

    #[test]
    fn data_round_trips_with_credit_and_close_is_propagated() {
        let (relay, addr) = relay();
        let (mut host, _serving) = session(&relay);
        host.hello(1);
        let mut client = connect(addr);
        assert_eq!(host.recv(), Frame::Open(1));
        client.write_all(b"ping").unwrap();
        assert_eq!(host.recv(), Frame::Data(1, b"ping".to_vec()));
        host.send(Frame::Data(1, b"pong".to_vec()));
        let mut reply = [0; 4];
        client.read_exact(&mut reply).unwrap();
        assert_eq!(&reply, b"pong");
        assert_eq!(host.recv(), Frame::Credit(1, 4));
        // Undelivered host data is flushed before the client sees the close.
        host.send(Frame::Data(1, b"bye".to_vec()));
        host.send(Frame::Close(1));
        let mut rest = Vec::new();
        client.read_to_end(&mut rest).unwrap();
        assert_eq!(rest, b"bye");
        // A client close reaches the host. "bye" may be credited if it was
        // delivered before the Close was handled.
        let second = connect(addr);
        let open = loop {
            match host.recv() {
                Frame::Credit(1, 3) => {}
                other => break other,
            }
        };
        assert_eq!(open, Frame::Open(2));
        second.shutdown(Shutdown::Both).unwrap();
        assert_eq!(host.recv(), Frame::Close(2));
    }

    #[test]
    fn the_connection_bound_closes_extra_clients() {
        let (relay, addr) = relay();
        let (mut host, _serving) = session(&relay);
        host.hello(1);
        let _a = connect(addr);
        let _b = connect(addr);
        assert_eq!(host.recv(), Frame::Open(1));
        assert_eq!(host.recv(), Frame::Open(2));
        let mut extra = connect(addr);
        assert!(closed(&mut extra));
        assert_eq!(relay.connections(), 2);
    }

    #[test]
    fn a_client_without_credit_stalls_at_the_window() {
        let (relay, addr) = relay();
        let (mut host, _serving) = session(&relay);
        host.hello(1);
        let mut client = connect(addr);
        assert_eq!(host.recv(), Frame::Open(1));
        let writer = std::thread::spawn(move || {
            let _ = client.write_all(&vec![1; 4 * WINDOW as usize]);
            client
        });
        let mut received = 0usize;
        host.stream
            .set_read_timeout(Some(Duration::from_millis(500)))
            .unwrap();
        while let Some(frame) = host.try_recv() {
            match frame {
                Frame::Data(1, data) => received += data.len(),
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(
            received, WINDOW as usize,
            "the guest never sends past its credit"
        );
        host.stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        host.send(Frame::Credit(1, 100));
        assert_eq!(host.recv(), Frame::Data(1, vec![1; 100]));
        drop(writer);
    }

    struct Sink(Arc<Mutex<Vec<u8>>>);

    impl Write for Sink {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn host_data_beyond_the_window_closes_the_connection() {
        let (relay, _) = relay();
        let sent = Arc::new(Mutex::new(Vec::new()));
        *relay.shared.port.lock().unwrap() = Some(Box::new(Sink(sent.clone())));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let _client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (stream, _) = listener.accept().unwrap();
        // No client writer drains this connection, so nothing is credited.
        let inbound = Arc::new(Inbound::default());
        relay.shared.state().conns.insert(
            5,
            Conn {
                stream,
                credit: WINDOW,
                inbound: inbound.clone(),
            },
        );
        for _ in 0..WINDOW as usize / MAX_PAYLOAD {
            relay.shared.handle(Frame::Data(5, vec![2; MAX_PAYLOAD]));
        }
        assert_eq!(relay.connections(), 1);
        assert_eq!(inbound.queue.lock().unwrap().0.len(), WINDOW as usize);
        relay.shared.handle(Frame::Data(5, vec![2]));
        assert_eq!(relay.connections(), 0);
        let queue = inbound.queue.lock().unwrap();
        assert!(queue.0.is_empty() && queue.1, "buffered data is dropped");
        let frames: Vec<_> = Decoder::default()
            .feed(&sent.lock().unwrap())
            .map(Result::unwrap)
            .collect();
        assert_eq!(frames, [Frame::Close(5)]);
    }

    #[test]
    fn a_host_reconnect_or_new_hello_closes_every_connection() {
        let (relay, addr) = relay();
        let (mut host, serving) = session(&relay);
        host.hello(1);
        let mut first = connect(addr);
        assert_eq!(host.recv(), Frame::Open(1));
        // A new Hello on the same stream (another manager took over).
        host.hello(2);
        assert!(closed(&mut first));
        let mut second = connect(addr);
        assert_eq!(host.recv(), Frame::Open(2));
        // The host side disappears: the session ends and clients are closed.
        drop(host);
        serving.join().unwrap().unwrap();
        assert!(closed(&mut second));
        assert_eq!(relay.connections(), 0);
        // A new host session works, without another Reset, and ids move on.
        let (mut host, _serving) = session(&relay);
        let mut refused = connect(addr);
        assert!(closed(&mut refused));
        host.send(Frame::Hello(3));
        assert_eq!(host.recv(), Frame::Ack(3));
        let mut third = connect(addr);
        assert_eq!(host.recv(), Frame::Open(3));
        third.write_all(b"x").unwrap();
        assert_eq!(host.recv(), Frame::Data(3, b"x".to_vec()));
    }
}

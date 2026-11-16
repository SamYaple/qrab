//! Host side of the SPIRE relay: one task owns QEMU's private chardev for
//! the `org.yapplesauce.spire.0` port and splices each guest connection to
//! its own TCP connection to the SPIRE server. It moves bytes only; the guest
//! agent's mTLS with the server stays end to end. Framing and credit rules
//! are in [`qrab_spire_relay::wire`].
use qrab_spire_relay::wire::{Decoder, Frame, MAX_PAYLOAD, WINDOW};
use std::{
    collections::HashMap,
    io,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream},
    net::{TcpStream, UnixStream},
    sync::{mpsc, Semaphore},
    task::{AbortHandle, JoinSet},
    time::Instant,
};

/// Encoded frames waiting for the chardev, each at most one data frame.
const FRAME_QUEUE: usize = 32;

#[derive(Debug, Clone)]
pub(super) struct Settings {
    pub upstream: SocketAddr,
    pub max_connections: usize,
    pub idle: Duration,
    pub connect_timeout: Duration,
    pub retry: Duration,
}

impl From<&super::SpireRelay> for Settings {
    fn from(config: &super::SpireRelay) -> Self {
        Self {
            upstream: config.upstream,
            max_connections: config.max_connections,
            idle: Duration::from_secs(config.idle_timeout_secs),
            connect_timeout: Duration::from_secs(10),
            retry: Duration::from_secs(1),
        }
    }
}

/// Owned by the supervisor for one QEMU lifetime; dropping it stops the
/// relay and every upstream connection.
pub(super) struct Relay {
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Relay {
    pub(super) fn spawn(socket: PathBuf, settings: Settings) -> Self {
        Self {
            task: tokio::spawn(worker(socket, settings)),
        }
    }
}

fn log(event: serde_json::Value) {
    eprintln!("{event}");
}

/// Session numbers differ across manager processes, so a stale `Ack` left
/// in the channel by an earlier manager never matches.
fn first_session() -> u32 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() ^ d.as_secs() as u32)
        .unwrap_or(0);
    nanos ^ std::process::id().rotate_left(16)
}

async fn worker(socket: PathBuf, settings: Settings) {
    let mut session = first_session();
    loop {
        // QEMU's chardev accepts with or without a guest relay; a restarted
        // relay asks for a new session itself.
        if let Ok(stream) = UnixStream::connect(&socket).await {
            log(serde_json::json!({"event":"spire_relay_connected"}));
            let result = serve(stream, &settings, &mut session).await;
            log(serde_json::json!({
                "event":"spire_relay_disconnected",
                "error":result.err().map(|e| e.to_string()),
            }));
        }
        tokio::time::sleep(settings.retry).await;
    }
}

struct Conn {
    generation: u64,
    /// Guest bytes for the upstream; holds at most `pending` bytes.
    pipe: DuplexStream,
    /// Guest bytes received and not yet credited back.
    pending: Arc<AtomicUsize>,
    /// Bytes the guest still accepts from the upstream.
    credit: Arc<Semaphore>,
    abort: AbortHandle,
}

struct Splice<'a> {
    settings: &'a Settings,
    session: &'a mut u32,
    acked: bool,
    frames: mpsc::Sender<Vec<u8>>,
    conns: HashMap<u32, Conn>,
    tasks: JoinSet<(u32, u64)>,
    generation: u64,
}

/// Serves one chardev connection until EOF. Dropping it ends every upstream
/// connection of the session.
async fn serve<S>(stream: S, settings: &Settings, session: &mut u32) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let (mut reader, mut writer) = tokio::io::split(stream);
    let (frames, mut queued) = mpsc::channel::<Vec<u8>>(FRAME_QUEUE);
    let mut writing = tokio::spawn(async move {
        while let Some(bytes) = queued.recv().await {
            writer.write_all(&bytes).await?;
        }
        io::Result::Ok(())
    });
    let _writing = AbortOnDrop(writing.abort_handle());
    let mut splice = Splice {
        settings,
        session,
        acked: false,
        frames,
        conns: HashMap::new(),
        tasks: JoinSet::new(),
        generation: 0,
    };
    splice.hello().await;
    let mut decoder = Decoder::default();
    let mut buf = vec![0; MAX_PAYLOAD];
    loop {
        tokio::select! {
            biased;
            Some(Ok((id, generation))) = splice.tasks.join_next() => splice.finished(id, generation),
            read = reader.read(&mut buf) => {
                let n = read?;
                if n == 0 {
                    return Ok(());
                }
                let decoded: Vec<_> = decoder.feed(&buf[..n]).collect();
                for frame in decoded {
                    match frame {
                        Ok(frame) => splice.handle(frame).await,
                        Err(error) => log(serde_json::json!({"event":"spire_relay_frame_invalid","error":error.to_string()})),
                    }
                }
            }
            written = &mut writing => return written.map_err(io::Error::other)?,
        }
    }
}

struct AbortOnDrop(AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl Splice<'_> {
    async fn send(&self, frame: Frame) {
        let _ = self.frames.send(frame.encode()).await;
    }

    /// Discards every connection and starts a session the guest must
    /// acknowledge before any of its frames count.
    async fn hello(&mut self) {
        self.conns.clear();
        // Dropping the old set aborts its tasks and frees the bound at once.
        self.tasks = JoinSet::new();
        *self.session = self.session.wrapping_add(1);
        self.acked = false;
        self.send(Frame::Hello(*self.session)).await;
    }

    async fn handle(&mut self, frame: Frame) {
        match frame {
            Frame::Reset => self.hello().await,
            Frame::Ack(session) => self.acked |= session == *self.session,
            _ if !self.acked => {}
            Frame::Open(id) => self.open(id).await,
            Frame::Data(id, data) => {
                let Some(conn) = self.conns.get_mut(&id) else {
                    return;
                };
                if conn.pending.load(Ordering::SeqCst) + data.len() > WINDOW as usize {
                    // The guest exceeded its credit; never buffer beyond it.
                    self.drop_conn(id);
                    return self.send(Frame::Close(id)).await;
                }
                conn.pending.fetch_add(data.len(), Ordering::SeqCst);
                // The pipe holds WINDOW bytes, so this completes at once; it
                // fails only once the connection task has ended.
                let _ = conn.pipe.write_all(&data).await;
            }
            // Dropping the pipe lets the task forward what the guest sent
            // before it closed, then end the upstream connection.
            Frame::Close(id) => drop(self.conns.remove(&id)),
            Frame::Credit(id, bytes) => {
                if let Some(conn) = self.conns.get(&id) {
                    let room = (WINDOW as usize).saturating_sub(conn.credit.available_permits());
                    conn.credit.add_permits((bytes as usize).min(room));
                }
            }
            Frame::Hello(_) => {}
        }
    }

    async fn open(&mut self, id: u32) {
        if self.conns.contains_key(&id) {
            self.drop_conn(id);
            return self.send(Frame::Close(id)).await;
        }
        // Closed connections still draining count against the bound.
        if self.tasks.len() >= self.settings.max_connections {
            return self.send(Frame::Close(id)).await;
        }
        let (pipe, upstream_pipe) = tokio::io::duplex(WINDOW as usize);
        let pending = Arc::new(AtomicUsize::new(0));
        let credit = Arc::new(Semaphore::new(WINDOW as usize));
        self.generation += 1;
        let abort = self.tasks.spawn(connection(
            Link {
                id,
                generation: self.generation,
                pipe: upstream_pipe,
                pending: pending.clone(),
                credit: credit.clone(),
                frames: self.frames.clone(),
            },
            self.settings.clone(),
        ));
        self.conns.insert(
            id,
            Conn {
                generation: self.generation,
                pipe,
                pending,
                credit,
                abort,
            },
        );
    }

    fn drop_conn(&mut self, id: u32) {
        if let Some(conn) = self.conns.remove(&id) {
            conn.abort.abort();
        }
    }

    fn finished(&mut self, id: u32, generation: u64) {
        if self
            .conns
            .get(&id)
            .is_some_and(|conn| conn.generation == generation)
        {
            self.conns.remove(&id);
        }
    }
}

/// A connection task's half of [`Conn`].
struct Link {
    id: u32,
    generation: u64,
    pipe: DuplexStream,
    pending: Arc<AtomicUsize>,
    credit: Arc<Semaphore>,
    frames: mpsc::Sender<Vec<u8>>,
}

/// Runs one guest connection until either side closes, it errs or it is idle
/// for [`Settings::idle`]. The guest learns of every end it did not cause.
async fn connection(mut link: Link, settings: Settings) -> (u32, u64) {
    let id = link.id;
    let done = (id, link.generation);
    let close = Frame::Close(id).encode();
    let upstream = match tokio::time::timeout(
        settings.connect_timeout,
        TcpStream::connect(settings.upstream),
    )
    .await
    {
        Ok(Ok(stream)) => stream,
        failure => {
            let error = match failure {
                Ok(Err(error)) => error.to_string(),
                _ => "connect timed out".into(),
            };
            log(
                serde_json::json!({"event":"spire_relay_upstream_failed","upstream":settings.upstream,"error":error}),
            );
            let _ = link.frames.send(close).await;
            return done;
        }
    };
    let _ = upstream.set_nodelay(true);
    let (mut up_read, mut up_write) = upstream.into_split();
    let started = Instant::now();
    let activity = AtomicU64::new(0);
    let touch = || activity.store(started.elapsed().as_millis() as u64, Ordering::Relaxed);
    let (pipe, pending, credit, frames) =
        (&mut link.pipe, &link.pending, &link.credit, &link.frames);
    // Each direction returns whether the guest still needs a Close.
    let to_upstream = async {
        let mut buf = vec![0; MAX_PAYLOAD];
        loop {
            let n = pipe.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                let _ = up_write.shutdown().await;
                return false;
            }
            touch();
            if up_write.write_all(&buf[..n]).await.is_err() {
                return true;
            }
            pending.fetch_sub(n, Ordering::SeqCst);
            let credit = Frame::Credit(id, n as u32).encode();
            if frames.send(credit).await.is_err() {
                return true;
            }
        }
    };
    let to_guest = async {
        let mut buf = vec![0; MAX_PAYLOAD];
        loop {
            // Read only as much as the guest has room for.
            let Ok(first) = credit.acquire().await else {
                return true;
            };
            first.forget();
            let extra = credit.available_permits().min(MAX_PAYLOAD - 1);
            if let Ok(more) = credit.try_acquire_many(extra as u32) {
                more.forget();
            }
            let grant = 1 + extra;
            let n = up_read.read(&mut buf[..grant]).await.unwrap_or(0);
            credit.add_permits(grant - n);
            if n == 0 {
                return true;
            }
            touch();
            if frames
                .send(Frame::Data(id, buf[..n].to_vec()).encode())
                .await
                .is_err()
            {
                return true;
            }
        }
    };
    let idle = async {
        loop {
            let seen = activity.load(Ordering::Relaxed);
            tokio::time::sleep_until(started + Duration::from_millis(seen) + settings.idle).await;
            if activity.load(Ordering::Relaxed) == seen {
                return true;
            }
        }
    };
    let notify = tokio::select! {
        notify = to_upstream => notify,
        notify = to_guest => notify,
        notify = idle => notify,
    };
    if notify {
        let _ = link.frames.send(close).await;
    }
    done
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use tokio::net::TcpListener;

    const STEP: Duration = Duration::from_secs(5);

    #[derive(Clone, Copy)]
    enum Upstream {
        Echo,
        /// Echo one read, then close.
        Once,
        /// Write without end and never read.
        Flood,
    }

    #[derive(Default)]
    struct Counters {
        accepted: AtomicUsize,
        closed: AtomicUsize,
    }

    async fn upstream(mode: Upstream) -> (SocketAddr, Arc<Counters>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let counters = Arc::new(Counters::default());
        let shared = counters.clone();
        tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                shared.accepted.fetch_add(1, Ordering::SeqCst);
                let counters = shared.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0; 65536];
                    match mode {
                        Upstream::Echo | Upstream::Once => loop {
                            let n = stream.read(&mut buf).await.unwrap_or(0);
                            if n == 0 || stream.write_all(&buf[..n]).await.is_err() {
                                break;
                            }
                            if matches!(mode, Upstream::Once) {
                                break;
                            }
                        },
                        Upstream::Flood => while stream.write_all(&buf).await.is_ok() {},
                    }
                    counters.closed.fetch_add(1, Ordering::SeqCst);
                });
            }
        });
        (addr, counters)
    }

    fn settings(upstream: SocketAddr) -> Settings {
        Settings {
            upstream,
            max_connections: 8,
            idle: Duration::from_secs(30),
            connect_timeout: STEP,
            retry: Duration::from_millis(20),
        }
    }

    async fn eventually(what: &str, check: impl Fn() -> bool) {
        let deadline = Instant::now() + STEP;
        while !check() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// A fake guest relay speaking the wire format.
    struct Guest {
        stream: DuplexStream,
        decoder: Decoder,
        pending: VecDeque<Frame>,
    }

    impl Guest {
        /// A host splice serving one end of an in-memory chardev.
        fn attach(settings: &Settings, session: u32) -> (Self, tokio::task::JoinHandle<u32>) {
            let (host, guest) = tokio::io::duplex(1 << 20);
            let settings = settings.clone();
            let serving = tokio::spawn(async move {
                let mut session = session;
                serve(host, &settings, &mut session).await.unwrap();
                session
            });
            let guest = Self {
                stream: guest,
                decoder: Decoder::default(),
                pending: VecDeque::new(),
            };
            (guest, serving)
        }

        async fn send(&mut self, frame: Frame) {
            self.stream.write_all(&frame.encode()).await.unwrap();
        }

        async fn try_recv(&mut self, wait: Duration) -> Option<Frame> {
            let mut buf = [0; 4096];
            while self.pending.is_empty() {
                let n = tokio::time::timeout(wait, self.stream.read(&mut buf))
                    .await
                    .ok()?
                    .unwrap();
                assert!(n > 0, "the host closed the chardev");
                let frames: Vec<_> = self.decoder.feed(&buf[..n]).map(Result::unwrap).collect();
                self.pending.extend(frames);
            }
            self.pending.pop_front()
        }

        /// The next frame other than a credit return.
        async fn recv(&mut self) -> Frame {
            loop {
                match self.try_recv(STEP).await.expect("a frame from the host") {
                    Frame::Credit(..) => {}
                    frame => return frame,
                }
            }
        }

        async fn accept_hello(&mut self) -> u32 {
            let Frame::Hello(session) = self.recv().await else {
                panic!("the host starts with Hello");
            };
            self.send(Frame::Ack(session)).await;
            session
        }

        /// Data the host sends on `id` until it stays quiet for a moment.
        async fn drain(&mut self, id: u32) -> usize {
            let mut total = 0;
            while let Some(frame) = self.try_recv(Duration::from_millis(300)).await {
                match frame {
                    Frame::Data(i, data) if i == id => total += data.len(),
                    other => panic!("unexpected {other:?}"),
                }
            }
            total
        }
    }

    /// The real guest relay serving a socketpair, with the host splice on
    /// the other end. Returns the guest's listen address.
    fn real_guest(
        relay: &Arc<qrab_spire_relay::guest::Relay>,
        settings: &Settings,
        session: u32,
    ) -> tokio::task::JoinHandle<u32> {
        let (host, guest) = std::os::unix::net::UnixStream::pair().unwrap();
        let relay = relay.clone();
        std::thread::spawn(move || relay.serve(guest.try_clone().unwrap(), guest));
        host.set_nonblocking(true).unwrap();
        let host = UnixStream::from_std(host).unwrap();
        let settings = settings.clone();
        tokio::spawn(async move {
            let mut session = session;
            let _ = serve(host, &settings, &mut session).await;
            session
        })
    }

    fn guest_relay() -> (Arc<qrab_spire_relay::guest::Relay>, SocketAddr) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let relay = qrab_spire_relay::guest::Relay::new(listener, 8).unwrap();
        (Arc::new(relay), addr)
    }

    /// Connects once the guest relay has a host session; before that it
    /// closes connections at once.
    async fn client(addr: SocketAddr) -> TcpStream {
        let deadline = Instant::now() + STEP;
        loop {
            let mut stream = TcpStream::connect(addr).await.unwrap();
            let mut probe = [0; 1];
            if tokio::time::timeout(Duration::from_millis(100), stream.read(&mut probe))
                .await
                .is_err()
            {
                return stream;
            }
            assert!(
                Instant::now() < deadline,
                "the guest relay never became ready"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    async fn round_trip(stream: &mut TcpStream, data: &[u8]) {
        let (mut read, mut write) = stream.split();
        let mut echoed = vec![0; data.len()];
        let (written, got) = tokio::join!(write.write_all(data), read.read_exact(&mut echoed));
        written.unwrap();
        got.unwrap();
        assert!(echoed == data, "the echo matches what was sent");
    }

    async fn closed(stream: &mut TcpStream) -> bool {
        matches!(
            tokio::time::timeout(STEP, stream.read(&mut [0; 64])).await,
            Ok(Ok(0) | Err(_))
        )
    }

    #[tokio::test]
    async fn two_guest_connections_round_trip_through_their_own_upstreams() {
        let (addr, counters) = upstream(Upstream::Echo).await;
        let (relay, guest_addr) = guest_relay();
        let _host = real_guest(&relay, &settings(addr), 0);
        let mut a = client(guest_addr).await;
        let mut b = client(guest_addr).await;
        // Several windows each, so credit flows both ways while interleaved.
        let alpha: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        let bravo: Vec<u8> = (0..300_000u32).map(|i| (i % 241) as u8 ^ 0x5a).collect();
        tokio::join!(round_trip(&mut a, &alpha), round_trip(&mut b, &bravo));
        round_trip(&mut a, b"alpha").await;
        round_trip(&mut b, b"bravo").await;
        assert_eq!(counters.accepted.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn an_upstream_close_ends_only_its_connection_and_the_next_one_reconnects() {
        let (addr, counters) = upstream(Upstream::Once).await;
        let (relay, guest_addr) = guest_relay();
        let _host = real_guest(&relay, &settings(addr), 0);
        let mut first = client(guest_addr).await;
        round_trip(&mut first, b"one").await;
        assert!(closed(&mut first).await);
        let mut second = client(guest_addr).await;
        round_trip(&mut second, b"two").await;
        assert_eq!(counters.accepted.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_chardev_reconnect_closes_guest_connections_and_serves_new_ones() {
        let (addr, counters) = upstream(Upstream::Echo).await;
        let (relay, guest_addr) = guest_relay();
        let settings = settings(addr);
        let host = real_guest(&relay, &settings, 0);
        let mut before = client(guest_addr).await;
        round_trip(&mut before, b"before").await;
        // The manager restarts or the VM migrated: the chardev drops.
        host.abort();
        assert!(closed(&mut before).await);
        eventually("the old upstream to close", || {
            counters.closed.load(Ordering::SeqCst) == 1
        })
        .await;
        let _host = real_guest(&relay, &settings, 100);
        let mut after = client(guest_addr).await;
        round_trip(&mut after, b"after").await;
        assert_eq!(counters.accepted.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_new_session_ignores_stale_guest_frames_until_its_ack() {
        let (addr, counters) = upstream(Upstream::Echo).await;
        let settings = settings(addr);
        let (mut guest, serving) = Guest::attach(&settings, 0);
        let first = guest.accept_hello().await;
        guest.send(Frame::Open(1)).await;
        guest.send(Frame::Data(1, b"a".to_vec())).await;
        assert_eq!(guest.recv().await, Frame::Data(1, b"a".to_vec()));
        drop(guest);
        let session = serving.await.unwrap();
        eventually("the first upstream to close", || {
            counters.closed.load(Ordering::SeqCst) == 1
        })
        .await;

        let (mut guest, _serving) = Guest::attach(&settings, session);
        let Frame::Hello(second) = guest.recv().await else {
            panic!("a reconnect starts with Hello");
        };
        assert_ne!(second, first);
        // Leftovers of the old session, and a stale Ack, open nothing.
        guest.send(Frame::Open(5)).await;
        guest.send(Frame::Data(5, b"stale".to_vec())).await;
        guest.send(Frame::Ack(first)).await;
        guest.send(Frame::Open(6)).await;
        guest.send(Frame::Ack(second)).await;
        // A previously used id starts fresh.
        guest.send(Frame::Open(1)).await;
        guest.send(Frame::Data(1, b"b".to_vec())).await;
        assert_eq!(guest.recv().await, Frame::Data(1, b"b".to_vec()));
        assert_eq!(counters.accepted.load(Ordering::SeqCst), 2);

        // A restarted guest relay asks for a new session; its connections end.
        guest.send(Frame::Reset).await;
        let Frame::Hello(third) = guest.recv().await else {
            panic!("Reset is answered with Hello");
        };
        assert_ne!(third, second);
        eventually("the second upstream to close", || {
            counters.closed.load(Ordering::SeqCst) == 2
        })
        .await;
    }

    #[tokio::test]
    async fn the_host_stalls_at_the_guest_window_under_a_slow_reader() {
        let (addr, _) = upstream(Upstream::Flood).await;
        let (mut guest, _serving) = Guest::attach(&settings(addr), 0);
        guest.accept_hello().await;
        guest.send(Frame::Open(1)).await;
        // The guest reader never credits: the host reads no more upstream
        // data than the window and queues nothing beyond it.
        assert_eq!(guest.drain(1).await, WINDOW as usize);
        guest.send(Frame::Credit(1, 1000)).await;
        assert_eq!(guest.drain(1).await, 1000);
    }

    #[tokio::test]
    async fn guest_data_beyond_its_credit_closes_the_connection() {
        let settings = settings("127.0.0.1:9".parse().unwrap());
        let mut session = 0;
        let (frames, mut queued) = mpsc::channel(FRAME_QUEUE);
        let mut splice = Splice {
            settings: &settings,
            session: &mut session,
            acked: true,
            frames,
            conns: HashMap::new(),
            tasks: JoinSet::new(),
            generation: 0,
        };
        // A connection whose upstream never drains its pipe.
        let (pipe, _upstream_pipe) = tokio::io::duplex(WINDOW as usize);
        let pending = Arc::new(AtomicUsize::new(0));
        let abort = splice.tasks.spawn(std::future::pending::<(u32, u64)>());
        splice.conns.insert(
            5,
            Conn {
                generation: 1,
                pipe,
                pending: pending.clone(),
                credit: Arc::new(Semaphore::new(WINDOW as usize)),
                abort,
            },
        );
        for _ in 0..WINDOW as usize / MAX_PAYLOAD {
            splice.handle(Frame::Data(5, vec![1; MAX_PAYLOAD])).await;
        }
        assert_eq!(pending.load(Ordering::SeqCst), WINDOW as usize);
        assert!(queued.try_recv().is_err());
        splice.handle(Frame::Data(5, vec![1])).await;
        assert!(splice.conns.is_empty());
        assert_eq!(queued.try_recv().unwrap(), Frame::Close(5).encode());
    }

    #[tokio::test]
    async fn connections_beyond_the_bound_are_closed() {
        let (addr, counters) = upstream(Upstream::Echo).await;
        let settings = Settings {
            max_connections: 2,
            ..settings(addr)
        };
        let (mut guest, _serving) = Guest::attach(&settings, 0);
        guest.accept_hello().await;
        for id in 1..=3 {
            guest.send(Frame::Open(id)).await;
        }
        assert_eq!(guest.recv().await, Frame::Close(3));
        eventually("two upstreams", || {
            counters.accepted.load(Ordering::SeqCst) == 2
        })
        .await;
        // Closing one frees a slot.
        guest.send(Frame::Close(1)).await;
        eventually("the closed upstream", || {
            counters.closed.load(Ordering::SeqCst) == 1
        })
        .await;
        // The slot frees once its task is joined, which may trail the close.
        let mut id = 4;
        loop {
            guest.send(Frame::Open(id)).await;
            guest.send(Frame::Data(id, b"four".to_vec())).await;
            match guest.recv().await {
                Frame::Data(i, data) if i == id => break assert_eq!(data, b"four"),
                Frame::Close(i) if i == id && id < 50 => id += 1,
                other => panic!("unexpected {other:?}"),
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    #[tokio::test]
    async fn an_idle_connection_is_closed() {
        let (addr, counters) = upstream(Upstream::Echo).await;
        let settings = Settings {
            idle: Duration::from_millis(200),
            ..settings(addr)
        };
        let (mut guest, _serving) = Guest::attach(&settings, 0);
        guest.accept_hello().await;
        guest.send(Frame::Open(1)).await;
        guest.send(Frame::Data(1, b"x".to_vec())).await;
        assert_eq!(guest.recv().await, Frame::Data(1, b"x".to_vec()));
        let started = Instant::now();
        assert_eq!(guest.recv().await, Frame::Close(1));
        assert!(started.elapsed() >= Duration::from_millis(150));
        eventually("the idle upstream to close", || {
            counters.closed.load(Ordering::SeqCst) == 1
        })
        .await;
    }

    #[tokio::test]
    async fn an_unreachable_upstream_closes_the_guest_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let (mut guest, _serving) = Guest::attach(&settings(addr), 0);
        guest.accept_hello().await;
        guest.send(Frame::Open(1)).await;
        assert_eq!(guest.recv().await, Frame::Close(1));
    }
}

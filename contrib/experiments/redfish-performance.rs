//! Explicit experiment only: compare the previous JSON-history representation
//! with the live compact implementation through the same current transport.
use super::*;
use futures::StreamExt;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Mutex,
    },
    time::Instant,
};

struct CountingAllocator;
static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
// SAFETY: every allocation is delegated unchanged to the system allocator;
// the optional counters perform no allocation and never touch allocated data.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let result = unsafe { System.alloc(layout) };
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        result
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let result = unsafe { System.alloc_zeroed(layout) };
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        result
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(pointer, layout, size) };
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(size as u64, Ordering::Relaxed);
        }
        result
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}
fn begin() {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::Relaxed);
}
fn end() -> Value {
    COUNTING.store(false, Ordering::Relaxed);
    json!({"allocations":ALLOCATIONS.load(Ordering::Relaxed),"allocated_bytes":ALLOCATED_BYTES.load(Ordering::Relaxed)})
}

#[derive(Default)]
struct LegacyHistory {
    sequence: u64,
    reports: std::collections::VecDeque<(u64, Value)>,
}
enum Recorder {
    Legacy(Mutex<LegacyHistory>),
    Optimized(TelemetryStore),
}
impl Recorder {
    fn new(legacy: bool) -> Self {
        if legacy {
            Self::Legacy(Mutex::default())
        } else {
            Self::Optimized(TelemetryStore::default())
        }
    }
    fn record(&self, sample: &pod::VmMetrics) {
        match self {
            Self::Legacy(history) => {
                // This reproduces the pre-optimization hot path, including its
                // duplicate check after building the complete JSON report.
                let mut history = history.lock().unwrap();
                let mut value = report(sample);
                if history
                    .reports
                    .back()
                    .is_some_and(|(_, previous)| previous["Timestamp"] == value["Timestamp"])
                {
                    return;
                }
                history.sequence += 1;
                let sequence = history.sequence;
                value["@odata.id"] = json!(format!("{HISTORY}-{sequence}"));
                value["Id"] = json!(format!("History-{sequence}"));
                value["ReportSequence"] = json!(sequence.to_string());
                if history.reports.len() == HISTORY_CAPACITY {
                    history.reports.pop_front();
                }
                history.reports.push_back((sequence, value));
            }
            Self::Optimized(store) => store.record(sample),
        }
    }
    fn report(&self) -> Value {
        match self {
            Self::Legacy(history) => history.lock().unwrap().reports.back().unwrap().1.clone(),
            Self::Optimized(store) => {
                let (sequence, sample) = store.0.lock().unwrap().reports.back().cloned().unwrap();
                history_report(sequence, &sample)
            }
        }
    }
    fn storage(&self) -> Value {
        match self {
            Self::Legacy(history) => {
                json!({"retained_reports":history.lock().unwrap().reports.len(),"representation":"expanded JSON trees"})
            }
            Self::Optimized(store) => store.stats(),
        }
    }
}
fn sample(timestamp: u64) -> pod::VmMetrics {
    pod::VmMetrics {
        observed_unix_ms: timestamp,
        logical_processors: 4,
        memory_bytes: 4 * 1024 * 1024 * 1024,
        block_read_bytes: 9007199254740993,
        ..Default::default()
    }
}
fn rss() -> Value {
    let data = std::fs::read_to_string("/proc/self/status").unwrap();
    let value = |key: &str| {
        data.lines()
            .find_map(|line| line.strip_prefix(key))
            .and_then(|line| line.split_whitespace().next())
            .and_then(|n| n.parse::<u64>().ok())
            .unwrap_or(0)
    };
    json!({"rss_kib":value("VmRSS:"),"peak_rss_kib":value("VmHWM:")})
}

async fn fake_control(
    path: &std::path::Path,
    delay_ms: u64,
    requests: Arc<AtomicUsize>,
) -> tokio::task::JoinHandle<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let listener = tokio::net::UnixListener::bind(path).unwrap();
    tokio::spawn(async move {
        let mut clients = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                accepted=listener.accept()=>{
                    let (stream,_)=accepted.unwrap();let requests=requests.clone();
                    clients.spawn(async move {
                        let mut stream=BufReader::new(stream);
                        stream.get_mut().write_all(b"{\"protocol\":\"qrab-control\",\"version\":1,\"vm\":\"performance\",\"operations\":[\"metrics\"]}\n").await.unwrap();
                        loop {
                            let mut line=String::new();if stream.read_line(&mut line).await.unwrap_or(0)==0{break;}
                            let request:Value=serde_json::from_str(&line).unwrap();assert_eq!(request["op"],"metrics");requests.fetch_add(1,Ordering::Relaxed);
                            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                            let reply=json!({"v":1,"id":request["id"],"result":sample(128)}).to_string()+"\n";
                            if stream.get_mut().write_all(reply.as_bytes()).await.is_err(){break;}
                        }
                    });
                }
                Some(_)=clients.join_next()=>{},
            }
        }
    })
}

#[test]
#[ignore = "explicit performance experiment; run contrib/experiments/redfish-performance.py"]
fn redfish_performance() {
    let mode = std::env::var("QRAB_PERF_MODE").unwrap_or_else(|_| "optimized".into());
    assert!(["legacy", "optimized"].contains(&mode.as_str()));
    let legacy = mode == "legacy";
    let iterations = std::env::var("QRAB_PERF_ITERATIONS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(20000)
        .min(1000000);
    let requests = std::env::var("QRAB_PERF_REQUESTS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(128)
        .clamp(1, 4096);
    let concurrency = std::env::var("QRAB_PERF_CONCURRENCY")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(4)
        .clamp(1, 32);
    let delay = std::env::var("QRAB_PERF_DELAY_MS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(2)
        .min(100);
    let recorder = Arc::new(Recorder::new(legacy));
    begin();
    for timestamp in 1..=128 {
        recorder.record(&sample(timestamp));
    }
    let seed_allocations = end();
    let started = Instant::now();
    begin();
    for _ in 0..iterations {
        recorder.record(std::hint::black_box(&sample(128)));
    }
    let duplicate_allocations = end();
    let duplicate_elapsed = started.elapsed();
    let started = Instant::now();
    begin();
    for _ in 0..256 {
        std::hint::black_box(recorder.report());
    }
    let read_allocations = end();
    let read_elapsed = started.elapsed();
    let storage = recorder.storage();
    let memory = rss();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (concurrent,persistence)=runtime.block_on(async {
        let directory=std::env::temp_dir().join(format!("qrab-redfish-performance-{}",std::process::id()));let _=std::fs::remove_dir_all(&directory);std::fs::create_dir(&directory).unwrap();let path=directory.join("control.sock");
        let query_count=Arc::new(AtomicUsize::new(0));let server=fake_control(&path,delay,query_count.clone()).await;
        let backend=pod::Backend::Socket(pod::Remote::new(path));
        let started=Instant::now();begin();
        let mut latencies=futures::stream::iter(0..requests).map(|_|{let backend=backend.clone();let recorder=recorder.clone();async move{
            let at=Instant::now();let sample=backend.metrics().await;let elapsed=at.elapsed().as_micros()as u64;
            if let Ok(sample)=sample{recorder.record(&sample);Some(elapsed)}else{None}
        }}).buffer_unordered(concurrency).collect::<Vec<_>>().await;
        let allocations=end();let elapsed=started.elapsed();let failures=latencies.iter().filter(|n|n.is_none()).count();latencies.sort();let latencies:Vec<_>=latencies.into_iter().flatten().collect();
        let percentile=|percent:usize|latencies.get(latencies.len().saturating_sub(1)*percent/100).copied().unwrap_or(0);
        let concurrent_queries=query_count.load(Ordering::Relaxed);
        let idle_store=TelemetryStore::default();let triggers=triggers::TriggerStore::default();
        for _ in 0..6{if legacy{let sample=backend.metrics().await.unwrap();recorder.record(&sample);}else{sampler_tick(&backend,&idle_store,&triggers,false).await;}}
        let idle_queries=query_count.load(Ordering::Relaxed)-concurrent_queries;
        let result=json!({"requests":requests,"concurrency":concurrency,"fake_control_delay_ms":delay,"successful":latencies.len(),"failed":failures,
            "elapsed_us":elapsed.as_micros(),"p50_us":percentile(50),"p95_us":percentile(95),"control_queries":concurrent_queries,"allocations":allocations,
            "idle_six_scheduler_ticks_control_queries":idle_queries});
        server.abort();std::fs::remove_dir_all(directory).unwrap();
        let persistence=crate::subscriptions::performance_experiment(legacy,4).await;
        (result,persistence)
    });
    println!(
        "REDFISH_PERFORMANCE {}",
        json!({"mode":mode,"profile":std::env::var("QRAB_PERF_PROFILE").unwrap_or_else(|_|"debug".into()),"history_seed_allocations":seed_allocations,
        "duplicate_sample_iterations":iterations,"duplicate_record_elapsed_us":duplicate_elapsed.as_micros(),"duplicate_record_allocations":duplicate_allocations,
        "historical_read_iterations":256,"historical_read_elapsed_us":read_elapsed.as_micros(),"historical_read_allocations":read_allocations,"history_storage":storage,
        "memory":memory,"concurrent_fake_control":concurrent,"persistent_delivery_checkpoint":persistence})
    );
}

# Redfish performance experiment

Run from the repository root:

```sh
python3 contrib/experiments/redfish-performance.py --runs 3 \
  --output /tmp/redfish-performance.json
```

This needs no QEMU, guest image, cluster, external receiver or Python packages.
The ignored Rust test is compiled with the normal library tests; ordinary
`contrib/check-runtime.sh` does not run the experiment. `--release` selects an
optimized test binary when a release build is available. `--concurrency 32` can
exercise admission under heavier load; the default of four concurrent requests
keeps the transport comparison below its admission limits.

The two variants run in separate processes. Both use the current manager socket
transport and a local fake control server. The `legacy` variant reproduces the
previous expanded JSON history, duplicate check after report construction, and
per-event durable checkpoint strategy. The `optimized` variant uses the current
production implementations. This isolates these hot paths; it is not a checkout
of the old project or an end-to-end production VM benchmark.

The allocator counter delegates to Rust's system allocator and is enabled only
during each measured region. Cumulative allocated bytes include transient
allocations that were freed; they are not retained memory or RSS. RSS and peak
RSS come independently from `/proc/self/status`. Historical report reads are
measured separately because retaining compact samples moves JSON construction
to the historical-read path.

## Recorded result

On 2026-11-14, three debug-profile runs with 20,000 repeated cached samples,
128 fake-control requests, four concurrent requests, and a 2ms fake reply delay
produced these medians:

| Measurement | Legacy model | Current implementation |
| --- | ---: | ---: |
| Duplicate history recording, 20,000 calls | 1,242,713us | 1,955us |
| Allocations during those duplicate calls | 6,640,000 | 0 |
| Cumulative allocated bytes during duplicate calls | 773,460,000 | 0 |
| Process peak RSS after history workload | 8,608KiB | 6,468KiB |
| Concurrent fake-control p95 latency | 3,586us | 3,514us |
| Failed fake-control requests | 0 | 0 |
| Idle control requests across six scheduler checks | 6 | 0 |
| Durable state writes for 128 acknowledged events | 128 | 4 |
| Time for those durable checkpoints | 35,791us | 2,896us |
| Constructing/cloning 256 historical reports | 3,654us | 16,874us |

The checkpoint test used `/tmp`, which was **tmpfs**, rather than a real PVC.
Write counts demonstrate the reduction independently of storage latency; the
absolute times do not predict PVC performance. Fake-control p95 was essentially
unchanged because its injected delay dominates this workload. The large duplicate
allocation reduction does not imply an equally large end-to-end HTTP speedup.

The current 128-sample history contains 11,264 bytes of typed sample payloads,
plus its container/lock bookkeeping. Seeding it requested 22,176 allocation bytes
in total, versus 4,993,328 cumulative allocation bytes for the legacy JSON model.
Those seeding totals also include temporary work and are not retained sizes.
Historical reads cost approximately 66us each in this debug experiment because
JSON is now constructed on demand, outside the history mutex.

## Runtime changes covered

- Default report definitions are `OnRequest`: an idle service makes no metric
  control requests. `REDFISH_TELEMETRY_INTERVAL_SECS=5..3600`, or operator
  `spec.components.redfish.telemetryIntervalSeconds`, enables an explicitly
  advertised periodic cadence. Enabled thresholds still sample every five
  seconds. Without periodic history, threshold sampling does not fabricate
  periodic historical reports.
- History retains at most 128 typed snapshots. Duplicate and older timestamps
  are rejected before constructing JSON; historical JSON is rendered after
  releasing the mutex. Failures do not create zero-valued or stale successful
  samples.
- Subscription delivery uses metadata-only event projections and constructs
  sanitized envelopes directly. Nonmatching events are skipped in pages of 64.
  Backlogs drain without an unconditional timer delay between every event;
  idle workers wait for event notifications or a bounded retry timer.
- One bounded delivery batch is checkpointed at a time, with a one-second flush
  while other requests remain pending. An empty/no-op batch performs no disk
  write. This remains **at-least-once** delivery: a crash before checkpointing
  can replay the bounded pending batch.
- Persistent subscription writes run on a blocking worker, with one writer
  admitted at a time. Readers do not hold a mutex across disk I/O. HTTP mutations
  get 503 when the writer is busy instead of accumulating waiting tasks. The
  single background delivery worker may wait with its bounded pending batch.
- Diagnostic attachments share immutable `Bytes` through the HTTP response,
  avoiding a new copy of a bundle of up to 128KiB for each download. Task
  collections construct ID links without cloning retained update output.

Authenticated `/redfish/v1/Managers/1/Oem/Qrab/Resources` includes optional
`Performance` counters for retained typed samples, background metric requests,
subscription attempts/checkpoints, persistence availability and writer admission,
and task/output/attachment retention. These are bounded diagnostic counters;
CPU and memory consumption remain separately accounted by the existing service
container/cgroup views.

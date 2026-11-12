//! Bounded in-memory history of QMP events observed by the pod supervisor.
use qapi::QapiEvent;
use qapi_protocol::QmpEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::VecDeque,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::watch;

/// Events retained per VM; older events are evicted first.
pub const EVENT_CAPACITY: usize = 256;
/// Largest page returned by one snapshot or cursor read.
pub const EVENT_PAGE_LIMIT: usize = 256;
/// Event payloads larger than this are replaced by `null` and marked truncated.
pub const EVENT_DATA_LIMIT: usize = 8 * 1024;
const EVENT_NAME_LIMIT: usize = 64;

/// QEMU's host timestamp. Absent when QEMU reported the -1/-1 "unavailable" form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QemuTime {
    pub seconds: i64,
    pub microseconds: i64,
}

/// One QMP event. `seq` increases by one per recorded event for the lifetime
/// of the control channel, so a gap in a cursor read means eviction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VmEvent {
    pub seq: u64,
    pub received_unix_ms: u64,
    #[serde(default)]
    pub qemu_time: Option<QemuTime>,
    /// The QAPI event name, for example `STOP`, `RESET` or `BLOCK_IO_ERROR`.
    pub event: String,
    #[serde(default)]
    pub data: Value,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}

impl VmEvent {
    /// Decode the payload as a generated QAPI event type when the name matches.
    ///
    /// ```
    /// # let event = manager::pod::VmEvent { seq: 1, received_unix_ms: 0, qemu_time: None,
    /// #     event: "RESET".into(), data: serde_json::json!({"guest": true, "reason": "guest-reset"}), truncated: false };
    /// let reset = event.decode::<qapi::events::ResetEvent>().unwrap().unwrap();
    /// assert_eq!(reset.guest, Some(true));
    /// assert!(event.decode::<qapi::events::StopEvent>().is_none());
    /// ```
    pub fn decode<E: QapiEvent>(&self) -> Option<serde_json::Result<E>> {
        (self.event == E::NAME && !self.truncated).then(|| {
            let data = match &self.data {
                Value::Null => Value::Object(Default::default()),
                data => data.clone(),
            };
            serde_json::from_value(data)
        })
    }
}

/// A bounded read of the event history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventPage {
    pub events: Vec<VmEvent>,
    /// Pass as `after` to continue reading without duplicates.
    pub next: u64,
    /// Sequence number of the most recent recorded event (0 when none).
    pub latest: u64,
    /// Events after the requested cursor that were evicted before this read.
    pub missed: u64,
    /// Events the supervisor discarded (invalid names) since the channel started.
    pub rejected: u64,
    /// Events the QMP client discarded before the supervisor saw them (full
    /// queue or oversized event) during the current QEMU connection.
    #[serde(default)]
    pub qmp_dropped: u64,
}

struct Ring {
    events: VecDeque<VmEvent>,
    latest: u64,
    rejected: u64,
    qmp_dropped: u64,
}

pub(super) struct EventLog {
    ring: Mutex<Ring>,
    latest: watch::Sender<u64>,
    dropped: watch::Sender<u64>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= EVENT_NAME_LIMIT
        && name
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

impl Default for EventLog {
    fn default() -> Self {
        Self {
            ring: Mutex::new(Ring {
                events: VecDeque::with_capacity(EVENT_CAPACITY),
                latest: 0,
                rejected: 0,
                qmp_dropped: 0,
            }),
            latest: watch::channel(0).0,
            dropped: watch::channel(0).0,
        }
    }
}

impl EventLog {
    fn lock(&self) -> std::sync::MutexGuard<'_, Ring> {
        self.ring
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    pub(super) fn record(&self, event: QmpEvent) {
        let mut ring = self.lock();
        if !valid_name(&event.event) {
            ring.rejected += 1;
            return;
        }
        let size = serde_json::to_vec(&event.data).map_or(usize::MAX, |v| v.len());
        let truncated = size > EVENT_DATA_LIMIT;
        ring.latest += 1;
        let seq = ring.latest;
        if ring.events.len() == EVENT_CAPACITY {
            ring.events.pop_front();
        }
        let timestamp = event.timestamp;
        ring.events.push_back(VmEvent {
            seq,
            received_unix_ms: now_ms(),
            qemu_time: (timestamp.seconds >= 0 && timestamp.microseconds >= 0).then_some(
                QemuTime {
                    seconds: timestamp.seconds,
                    microseconds: timestamp.microseconds,
                },
            ),
            event: event.event,
            data: if truncated { Value::Null } else { event.data },
            truncated,
        });
        drop(ring);
        self.latest.send_replace(seq);
    }

    /// `after: None` returns the newest `limit` events; `Some(cursor)` returns
    /// the oldest events with a sequence number greater than the cursor.
    /// `missed` counts sequence numbers in `(cursor, next]` that are not
    /// retained (evicted here, or never seen by a mirror).
    pub(super) fn page(&self, after: Option<u64>, limit: usize) -> EventPage {
        let limit = limit.clamp(1, EVENT_PAGE_LIMIT);
        let ring = self.lock();
        let (events, next, missed): (Vec<_>, u64, u64) = match after {
            None => {
                let skip = ring.events.len().saturating_sub(limit);
                let events: Vec<_> = ring.events.iter().skip(skip).cloned().collect();
                let next = events.last().map_or(ring.latest, |e| e.seq);
                (events, next, 0)
            }
            Some(cursor) => {
                let cursor = cursor.min(ring.latest);
                let start = ring.events.partition_point(|e| e.seq <= cursor);
                let events: Vec<_> = ring.events.range(start..).take(limit).cloned().collect();
                let next = events.last().map_or(ring.latest, |e| e.seq);
                let missed = (next - cursor).saturating_sub(events.len() as u64);
                (events, next, missed)
            }
        };
        EventPage {
            events,
            next,
            latest: ring.latest,
            missed,
            rejected: ring.rejected,
            qmp_dropped: ring.qmp_dropped,
        }
    }

    /// Mirror an event read from another log, keeping its sequence number.
    /// Events at or below the latest mirrored sequence are duplicates.
    pub(super) fn insert(&self, event: VmEvent, rejected: Option<u64>) {
        let mut ring = self.lock();
        if let Some(rejected) = rejected {
            ring.rejected = rejected;
        }
        if event.seq <= ring.latest {
            return;
        }
        ring.latest = event.seq;
        let seq = event.seq;
        if ring.events.len() == EVENT_CAPACITY {
            ring.events.pop_front();
        }
        ring.events.push_back(event);
        drop(ring);
        self.latest.send_replace(seq);
    }

    pub(super) fn set_qmp_dropped(&self, dropped: u64) {
        let mut ring = self.lock();
        if ring.qmp_dropped != dropped {
            ring.qmp_dropped = dropped;
            drop(ring);
            // Wake followers so the new count is forwarded.
            self.dropped.send_replace(dropped);
        }
    }

    /// Resolve once the dropped-event count differs from `known`.
    pub(super) async fn wait_dropped(&self, known: u64) {
        let mut dropped = self.dropped.subscribe();
        let _ = dropped.wait_for(|dropped| *dropped != known).await;
    }

    pub(super) fn latest(&self) -> u64 {
        self.lock().latest
    }

    /// Resolve once an event newer than `cursor` has been recorded.
    /// Also resolves when the log is [reset](Self::reset), so a waiter
    /// holding a cursor from the previous numbering re-reads.
    pub(super) async fn wait_after(&self, cursor: u64) {
        let mut latest = self.latest.subscribe();
        let start = *latest.borrow_and_update();
        let _ = latest
            .wait_for(|latest| *latest > cursor || *latest < start)
            .await;
    }

    /// Forget a mirror's history after its source restarted its numbering.
    pub(super) fn reset(&self) {
        let mut ring = self.lock();
        ring.events.clear();
        ring.latest = 0;
        ring.rejected = 0;
        ring.qmp_dropped = 0;
        drop(ring);
        self.latest.send_replace(0);
        self.dropped.send_replace(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(name: &str, data: Value) -> QmpEvent {
        serde_json::from_value(serde_json::json!({
            "event": name, "data": data, "timestamp": {"seconds": -1, "microseconds": -1}
        }))
        .unwrap()
    }

    #[test]
    fn ring_is_bounded_and_cursors_report_eviction() {
        let log = EventLog::default();
        for _ in 0..EVENT_CAPACITY + 10 {
            log.record(event("STOP", Value::Null));
        }
        let latest = (EVENT_CAPACITY + 10) as u64;
        let all = log.page(None, usize::MAX);
        assert_eq!(all.events.len(), EVENT_PAGE_LIMIT.min(EVENT_CAPACITY));
        assert_eq!(all.latest, latest);
        assert_eq!(all.events.last().unwrap().seq, latest);
        assert!(all.events[0].qemu_time.is_none());
        let from_start = log.page(Some(0), 5);
        assert_eq!(from_start.missed, 10);
        assert_eq!(from_start.events[0].seq, 11);
        assert_eq!(from_start.next, 15);
        let tail = log.page(Some(latest - 2), 100);
        assert_eq!(
            tail.events.iter().map(|e| e.seq).collect::<Vec<_>>(),
            [latest - 1, latest]
        );
        let empty = log.page(Some(latest), 100);
        assert!(empty.events.is_empty());
        assert_eq!((empty.next, empty.missed), (latest, 0));
        // A cursor from a future or reset producer cannot skip new events.
        assert_eq!(log.page(Some(u64::MAX), 1).next, latest);
    }

    #[test]
    fn mirrored_gaps_are_reported_as_missed_and_duplicates_ignored() {
        let source = EventLog::default();
        for _ in 0..5 {
            source.record(event("STOP", Value::Null));
        }
        let mirror = EventLog::default();
        for seq in [2, 4, 4, 1, 5] {
            let event = source.page(Some(seq - 1), 1).events.remove(0);
            mirror.insert(event, Some(3));
        }
        let page = mirror.page(Some(0), 10);
        assert_eq!(
            page.events.iter().map(|e| e.seq).collect::<Vec<_>>(),
            [2, 4, 5]
        );
        assert_eq!(
            (page.next, page.latest, page.missed, page.rejected),
            (5, 5, 2, 3)
        );
        assert_eq!(mirror.page(Some(2), 1).missed, 1);
    }

    #[test]
    fn invalid_names_are_rejected_and_large_payloads_truncated() {
        let log = EventLog::default();
        log.record(event("stop", Value::Null));
        log.record(event(&"A".repeat(EVENT_NAME_LIMIT + 1), Value::Null));
        log.record(event(
            "BLOCK_IO_ERROR",
            serde_json::json!({"blob": "x".repeat(EVENT_DATA_LIMIT)}),
        ));
        let page = log.page(None, 10);
        assert_eq!(page.rejected, 2);
        assert_eq!(page.events.len(), 1);
        assert!(page.events[0].truncated && page.events[0].data.is_null());
        assert!(page.events[0]
            .decode::<qapi::events::BlockIoErrorEvent>()
            .is_none());
    }

    #[tokio::test]
    async fn waiters_wake_only_for_newer_events() {
        let log = std::sync::Arc::new(EventLog::default());
        log.record(event("STOP", Value::Null));
        tokio::time::timeout(std::time::Duration::from_millis(50), log.wait_after(0))
            .await
            .unwrap();
        let waiter = tokio::spawn({
            let log = log.clone();
            async move { log.wait_after(1).await }
        });
        tokio::task::yield_now().await;
        assert!(!waiter.is_finished());
        log.record(event("RESUME", Value::Null));
        tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .unwrap()
            .unwrap();
    }
}

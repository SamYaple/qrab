//! A bounded byte tail of QEMU's guest serial output. Treat contents as secrets.
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

pub const SERIAL_CAPACITY: usize = 64 * 1024;
#[derive(Clone, Default)]
pub(super) struct SerialLog(Arc<Mutex<Tail>>);
#[derive(Default)]
struct Tail {
    bytes: VecDeque<u8>,
    dropped: u64,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SerialSnapshot {
    pub text: String,
    pub dropped_bytes: u64,
    pub retained_bytes: usize,
    pub capacity_bytes: usize,
}
impl SerialLog {
    pub(super) fn record(&self, bytes: &[u8]) {
        let mut tail = self.0.lock().unwrap_or_else(|p| p.into_inner());
        for byte in bytes {
            if tail.bytes.len() == SERIAL_CAPACITY {
                tail.bytes.pop_front();
                tail.dropped = tail.dropped.saturating_add(1);
            }
            tail.bytes.push_back(*byte);
        }
    }
    pub(super) fn snapshot(&self) -> SerialSnapshot {
        let tail = self.0.lock().unwrap_or_else(|p| p.into_inner());
        SerialSnapshot {
            text: String::from_utf8_lossy(&tail.bytes.iter().copied().collect::<Vec<_>>())
                .into_owned(),
            retained_bytes: tail.bytes.len(),
            dropped_bytes: tail.dropped,
            capacity_bytes: SERIAL_CAPACITY,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tail_survives_split_utf8_and_reports_eviction() {
        let log = SerialLog::default();
        log.record(&[b'x'; SERIAL_CAPACITY]);
        log.record(&[0xe2]);
        log.record(&[0x82, 0xac]);
        let snapshot = log.snapshot();
        assert_eq!(snapshot.retained_bytes, SERIAL_CAPACITY);
        assert_eq!(snapshot.dropped_bytes, 3);
        assert!(snapshot.text.ends_with('€'));
    }
}

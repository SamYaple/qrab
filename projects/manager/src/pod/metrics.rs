//! A fixed, read-only QMP snapshot. Never return QOM paths or backing chains.
use crate::Result;
use qapi::commands::{QueryBlockstats, QueryCpusFast, QueryMemorySizeSummary};
use qapi_protocol::Qmp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmMetrics {
    pub observed_unix_ms: u64,
    pub logical_processors: u64,
    /// Configured guest RAM, not resident host memory or guest memory use.
    pub memory_bytes: u64,
    /// Guest-facing block devices only; backing nodes are never summed twice.
    pub block_read_bytes: u64,
    pub block_write_bytes: u64,
    pub block_read_operations: u64,
    pub block_write_operations: u64,
    pub block_flush_operations: u64,
    pub block_read_time_ns: u64,
    pub block_write_time_ns: u64,
}

impl VmMetrics {
    pub(super) async fn query(qmp: &Qmp) -> Result<Self> {
        let cpus = qmp.execute(&QueryCpusFast {}).await?;
        let memory = qmp.execute(&QueryMemorySizeSummary {}).await?;
        let blocks = qmp
            .execute(&QueryBlockstats {
                query_nodes: Some(false),
            })
            .await?;
        let mut metrics = Self {
            observed_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            logical_processors: cpus.len() as u64,
            memory_bytes: memory
                .base_memory
                .saturating_add(memory.plugged_memory.unwrap_or(0)),
            ..Self::default()
        };
        for block in blocks {
            if block.device.is_none() && block.qdev.is_none() {
                continue;
            }
            let s = block.stats;
            let unsigned = |n: i64| u64::try_from(n).unwrap_or(0);
            metrics.block_read_bytes = metrics
                .block_read_bytes
                .saturating_add(unsigned(s.rd_bytes));
            metrics.block_write_bytes = metrics
                .block_write_bytes
                .saturating_add(unsigned(s.wr_bytes));
            metrics.block_read_operations = metrics
                .block_read_operations
                .saturating_add(unsigned(s.rd_operations));
            metrics.block_write_operations = metrics
                .block_write_operations
                .saturating_add(unsigned(s.wr_operations));
            metrics.block_flush_operations = metrics
                .block_flush_operations
                .saturating_add(unsigned(s.flush_operations));
            metrics.block_read_time_ns = metrics
                .block_read_time_ns
                .saturating_add(unsigned(s.rd_total_time_ns));
            metrics.block_write_time_ns = metrics
                .block_write_time_ns
                .saturating_add(unsigned(s.wr_total_time_ns));
        }
        Ok(metrics)
    }
}

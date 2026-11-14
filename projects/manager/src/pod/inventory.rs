//! Fixed QMP hardware inventory queries with a deliberately sanitized wire view.
//! Host paths, QOM paths, host TIDs, iSCSI URLs and backing chains stay private.
use crate::Result;
use qapi::commands::{QueryBlock, QueryBlockstats, QueryCpusFast};
use qapi_protocol::Qmp;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareInventory {
    pub observed_unix_ms: u64,
    pub processors: Vec<ProcessorInventory>,
    pub drives: Vec<DriveInventory>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessorInventory {
    pub id: String,
    /// QEMU socket identifier, absent when QEMU did not report socket topology.
    pub socket: Option<i64>,
    pub total_threads: u64,
    /// Omitted unless all CPUs in this socket report their core identifier.
    pub total_cores: Option<u64>,
    /// QEMU CPU model (QOM type), not the host's physical model.
    pub model: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveCounters {
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub read_operations: u64,
    pub write_operations: u64,
    pub flush_operations: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveInventory {
    pub id: String,
    pub capacity_bytes: Option<u64>,
    pub read_only: bool,
    pub removable: bool,
    pub media_present: bool,
    pub encrypted: bool,
    pub format: Option<String>,
    pub io_failed: bool,
    pub counters: Option<DriveCounters>,
}

impl HardwareInventory {
    /// The Control caller bounds execution and coalesces readers. These commands
    /// expose guest-facing devices only; node backing graphs are never returned.
    pub(super) async fn query(qmp: &Qmp) -> Result<Self> {
        let cpus = qmp.execute(&QueryCpusFast {}).await?;
        let blocks = qmp.execute(&QueryBlock { flat: Some(true) }).await?;
        let stats = qmp
            .execute(&QueryBlockstats {
                query_nodes: Some(false),
            })
            .await?;
        Ok(Self::from_qmp(cpus, blocks, stats))
    }

    fn from_qmp(
        cpus: Vec<qapi::CpuInfoFast>,
        blocks: Vec<qapi::BlockInfo>,
        stats: Vec<qapi::BlockStats>,
    ) -> Self {
        let mut sockets: BTreeMap<Option<i64>, Vec<qapi::CpuInfoFast>> = BTreeMap::new();
        for cpu in cpus {
            let socket = cpu.props.as_ref().and_then(|p| p.socket_id);
            sockets.entry(socket).or_default().push(cpu);
        }
        let processors = sockets
            .into_iter()
            .map(|(socket, cpus)| {
                let all_cores = socket.is_some()
                    && cpus
                        .iter()
                        .all(|c| c.props.as_ref().is_some_and(|p| p.core_id.is_some()));
                // Core IDs may repeat between dies/clusters/modules within a socket.
                let cores: BTreeSet<_> = cpus
                    .iter()
                    .filter_map(|c| c.props.as_ref())
                    .map(|p| {
                        (
                            p.book_id,
                            p.drawer_id,
                            p.die_id,
                            p.cluster_id,
                            p.module_id,
                            p.core_id,
                        )
                    })
                    .collect();
                let models: BTreeSet<_> = cpus.iter().filter_map(|c| c.qom_type.as_ref()).collect();
                let model = if cpus.iter().all(|c| c.qom_type.is_some()) && models.len() == 1 {
                    models.first().map(|s| (*s).clone())
                } else {
                    None
                };
                ProcessorInventory {
                    id: socket
                        .map(|s| format!("socket-{s}"))
                        .unwrap_or_else(|| "unknown".into()),
                    socket,
                    total_threads: cpus.len() as u64,
                    total_cores: all_cores.then_some(cores.len() as u64),
                    model,
                }
            })
            .collect();
        let mut used = BTreeSet::new();
        let mut drives = Vec::new();
        for (index, block) in blocks.into_iter().enumerate() {
            let node = block.inserted.as_ref().and_then(|i| i.node_name.as_deref());
            // Flash is firmware inventory, not an attached storage drive.
            if node.is_some_and(|n| n.starts_with("pflash-")) {
                continue;
            }
            let candidate = node.filter(|n| safe_id(n)).unwrap_or("");
            let mut id = if candidate.is_empty() {
                format!("device-{}", index + 1)
            } else {
                candidate.to_owned()
            };
            if !used.insert(id.clone()) {
                id = format!("device-{}", index + 1);
                used.insert(id.clone());
            }
            let counters = stats
                .iter()
                .find(|s| {
                    block
                        .qdev
                        .as_ref()
                        .is_some_and(|q| s.qdev.as_ref() == Some(q))
                        || (!block.device.is_empty()
                            && s.device.as_deref() == Some(block.device.as_str()))
                        || node.is_some_and(|n| s.node_name.as_deref() == Some(n))
                })
                .map(|s| {
                    let unsigned = |n: i64| u64::try_from(n).unwrap_or(0);
                    DriveCounters {
                        read_bytes: unsigned(s.stats.rd_bytes),
                        write_bytes: unsigned(s.stats.wr_bytes),
                        read_operations: unsigned(s.stats.rd_operations),
                        write_operations: unsigned(s.stats.wr_operations),
                        flush_operations: unsigned(s.stats.flush_operations),
                    }
                });
            let capacity_bytes = block
                .inserted
                .as_ref()
                .and_then(|i| i.image.as_ref())
                .and_then(|i| u64::try_from(i.virtual_size).ok());
            let io_failed = matches!(
                block.io_status,
                Some(qapi::BlockDeviceIoStatus::Failed | qapi::BlockDeviceIoStatus::Nospace)
            );
            drives.push(DriveInventory {
                id,
                capacity_bytes,
                read_only: block.inserted.as_ref().is_some_and(|i| i.ro),
                removable: block.removable,
                media_present: block.inserted.is_some(),
                encrypted: block.inserted.as_ref().is_some_and(|i| i.encrypted),
                format: block.inserted.as_ref().map(|i| i.drv.clone()),
                io_failed,
                counters,
            });
        }
        drives.sort_by(|a, b| a.id.cmp(&b.id));
        Self {
            observed_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            processors,
            drives,
        }
    }
}

fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn topology_counts_smt_and_repeated_core_ids_across_dies() {
        let cpus = (0..4).map(|i| serde_json::from_value(json!({
            "cpu-index":i,"qom-path":"/private/cpu","thread-id":100+i,"qom-type":"qemu64-x86_64-cpu",
            "props":{"socket-id":0,"die-id":i/2,"core-id":0,"thread-id":i%2}
        })).unwrap()).collect();
        let inventory = HardwareInventory::from_qmp(cpus, vec![], vec![]);
        assert_eq!(inventory.processors[0].total_threads, 4);
        assert_eq!(inventory.processors[0].total_cores, Some(2));
        assert!(!serde_json::to_string(&inventory)
            .unwrap()
            .contains("/private"));
    }

    #[test]
    fn storage_does_not_leak_backing_urls_paths_or_host_device_identifiers() {
        let blocks = serde_json::from_value(json!([{
            "device":"private-host-name","qdev":"/private/qdev","type":"unknown","locked":false,"removable":false,
            "io-status":"nospace","inserted":{"node-name":"iscsi-root","drv":"raw","file":"iscsi://user:secret@private/target",
                "ro":true,"encrypted":false,"image":{"filename":"/private/disk","format":"raw","virtual-size":1048576,
                "backing-filename":"/private/backing"}}
        },{
            "device":"flash","type":"unknown","locked":false,"removable":false,
            "inserted":{"node-name":"pflash-code","drv":"raw","file":"/private/firmware","ro":true,"encrypted":false}
        }])).unwrap();
        let stats = serde_json::from_value(json!([{
            "node-name":"iscsi-root","stats":{"rd_bytes":4096,"wr_bytes":8192,"rd_operations":2,"wr_operations":3,
                "flush_operations":1,"flush_total_time_ns":10,"rd_total_time_ns":20,"wr_total_time_ns":30,"wr_highest_offset":8192},
            "parent":{"stats":{"rd_bytes":99999,"wr_bytes":99999,"rd_operations":99,"wr_operations":99,
                "flush_operations":99,"flush_total_time_ns":10,"rd_total_time_ns":20,"wr_total_time_ns":30,"wr_highest_offset":99999}}
        }])).unwrap();
        let inventory = HardwareInventory::from_qmp(vec![], blocks, stats);
        assert_eq!(
            inventory.drives[0].counters.as_ref().unwrap().read_bytes,
            4096
        );
        assert_eq!(inventory.drives.len(), 1);
        assert_eq!(inventory.drives[0].id, "iscsi-root");
        assert_eq!(inventory.drives[0].capacity_bytes, Some(1048576));
        assert!(inventory.drives[0].io_failed && inventory.drives[0].read_only);
        let serialized = serde_json::to_string(&inventory).unwrap();
        for secret in ["private", "secret", "backing", "host-name"] {
            assert!(!serialized.contains(secret));
        }
    }
    #[tokio::test]
    async fn queries_are_fixed_and_flat_without_returning_the_qmp_graph() {
        use qapi_protocol::server::QmpSession;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("qmp.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let server = tokio::spawn(async move {
            let mut session = QmpSession::accept(&listener).await.unwrap();
            session.send_greeting(11, 1, 3).await.unwrap();
            session.negotiate().await.unwrap();
            for wanted in ["query-cpus-fast", "query-block", "query-blockstats"] {
                let (name, args) = session.recv_command().await.unwrap();
                assert_eq!(name, wanted);
                if name == "query-block" {
                    assert_eq!(args["flat"], true);
                }
                if name == "query-blockstats" {
                    assert_eq!(args["query-nodes"], false);
                }
                session.send_return(json!([])).await.unwrap();
            }
        });
        let (qmp, _events) = Qmp::new(&path).await.unwrap();
        let inventory = HardwareInventory::query(&qmp).await.unwrap();
        assert!(inventory.processors.is_empty() && inventory.drives.is_empty());
        assert!(inventory.observed_unix_ms > 0);
        server.await.unwrap();
    }
}

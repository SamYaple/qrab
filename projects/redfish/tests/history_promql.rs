//! Real PromQL evaluation over synthetic predecessor/current Pod series.
use qrab_redfish::history::Metric;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const UID: &str = "6d351e66-b8d1-4ef5-8ba6-1205f7d23581";
const OLD: &str = "11111111-2222-3333-4444-555566667777";

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore = "explicit real PromQL check; requires QRAB_TEST_PROMTOOL"]
fn real_promql_never_returns_a_previous_same_name_pod() {
    let tool = std::env::var_os("QRAB_TEST_PROMTOOL")
        .expect("set QRAB_TEST_PROMTOOL to a verified promtool binary");
    let root = Scratch(std::env::temp_dir().join(format!(
            "qrab-history-promql-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    std::fs::create_dir(&root.0).unwrap();
    let mut series = Vec::new();
    let mut checks = Vec::new();
    for metric in Metric::ALL {
        let (family, label, id) = match metric {
            Metric::ContainerCpu => ("container_cpu_usage_seconds_total", "container", "manager"),
            Metric::ContainerMemory => {
                ("container_memory_working_set_bytes", "container", "manager")
            }
            Metric::BlockRead => ("qrab_vm_block_read_bytes_total", "drive", "disk0"),
            Metric::BlockWrite => ("qrab_vm_block_write_bytes_total", "drive", "disk0"),
            Metric::NicRx => ("qrab_vm_nic_rx_bytes_total", "nic", "lan"),
            Metric::NicTx => ("qrab_vm_nic_tx_bytes_total", "nic", "lan"),
        };
        let container = matches!(metric, Metric::ContainerCpu | Metric::ContainerMemory);
        let memory = metric == Metric::ContainerMemory;
        let old_values = if memory {
            "128 128 128 128 128 stale _ _ _ _ _"
        } else {
            "0 6000 12000 18000 24000 stale _ _ _ _ _"
        };
        let new_values = if memory {
            "_ _ _ _ _ 7 7 7 7 7 7"
        } else {
            "_ _ _ _ _ 0 60 120 180 240 300"
        };
        let old_scope = if container {
            format!(r#"id="/kubepods/burstable/pod{OLD}/old-container""#)
        } else {
            format!(r#"pod_uid="{OLD}""#)
        };
        let new_scope = if container {
            format!(r#"id="/kubepods/burstable/pod{UID}/current-container""#)
        } else {
            format!(r#"pod_uid="{UID}""#)
        };
        for (scope, values) in [(old_scope, old_values), (new_scope, new_values)] {
            series.push(json!({"series":format!(r#"{family}{{namespace="qrab",pod="vm-a",{label}="{id}",{scope}}}"#),"values":values}));
        }
        if container {
            let systemd = UID.replace('-', "_");
            series.push(json!({"series":format!(r#"{family}{{namespace="qrab",pod="vm-a",{label}="{id}",id="/kubepods.slice/kubepods-burstable.slice/kubepods-burstable-pod{systemd}.slice/cri-containerd-current.scope"}}"#),
                "values":if memory {"_ _ _ _ _ 11 11 11 11 11 11"} else {new_values}}));
            series.push(json!({"series":format!(r#"{family}{{namespace="qrab",pod="vm-a",{label}="{id}",id="/kubepods/pod{UID}-another/foreign"}}"#),
                "values":if memory {"_ _ _ _ _ 1024 1024 1024 1024 1024 1024"} else {"_ _ _ _ _ 0 60000 120000 180000 240000 300000"}}));
        } else {
            // Legacy samples without identity must not become a fallback.
            series.push(json!({"series":format!(r#"{family}{{namespace="qrab",pod="vm-a",{label}="{id}"}}"#),"values":"0 60000 120000 180000 240000 300000 360000 420000 480000 540000 600000"}));
        }
        let expression = metric.promql("qrab", "vm-a", UID, 120);
        checks.push(json!({"expr":expression,"eval_time":"3m","exp_samples":[]}));
        checks.push(json!({"expr":expression,"eval_time":"8m","exp_samples":[{"labels":format!(r#"{{{label}="{id}"}}"#),"value":if memory {18} else if container {2} else {1}}]}));
    }
    let input: Value = json!({"rule_files":[],"evaluation_interval":"1m","tests":[{"interval":"1m","input_series":series,"promql_expr_test":checks}]});
    let path = root.0.join("history.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&input).unwrap()).unwrap();
    let output = Command::new(tool)
        .args(["test", "rules"])
        .arg(&path)
        .output()
        .expect("run promtool");
    assert!(
        output.status.success(),
        "real PromQL crossed Pod incarnations:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

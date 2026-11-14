//! Renders the dashboard's pages from saved Kubernetes objects and
//! representative BMC data, for checking layout in a browser without a
//! cluster or a sign-in. Not part of the shipped image.
//!
//!   cargo run -p qrab-dashboard --example preview -- OUT_DIR VM.json POD.json [VM.json POD.json ...]
//!
//! The first VM gets the detail page; every VM appears in the list. A
//! built-in VM with NICs and cloud-init is always added and gets
//! `detail-network.html`. A sample base-image catalog feeds the Images page
//! and the launch forms.
use k8s_openapi::api::core::v1::Pod;
use qrab_dashboard::{
    access::Role,
    bmc::{self, Panel},
    cluster::Usage,
    images,
    launch::Report,
    model,
    ui::{self, Body, ConsolePage, DetailPage, LaunchPage, Launched, Page, PanelState, UserView},
};
use qrab_operator::api::VirtualMachine;
use std::collections::HashMap;

fn load<T: serde::de::DeserializeOwned>(path: &str) -> T {
    serde_json::from_str(&std::fs::read_to_string(path).expect(path)).expect(path)
}

fn panel() -> Panel {
    let now = 1_794_517_200_000u64;
    let events = (0..14)
        .map(|i| {
            let (event, data) = match i % 7 {
                0 => ("RTC_CHANGE", serde_json::json!({"offset": 1, "qom-path": "/machine/unattached/device[8]"})),
                1 => ("ACPI_DEVICE_OST", serde_json::json!({"info": {"device": "cpu-3", "source": 1, "status": 0, "slot": "3", "slot-type": "CPU"}})),
                2 => ("DEVICE_DELETED", serde_json::json!({"path": "/machine/peripheral/iscsi-scratch-virtio-blk-pci/virtio-backend", "device": "iscsi-scratch"})),
                3 => ("BLOCK_JOB_COMPLETED", serde_json::json!({"device": "iscsi-root", "len": 134217728u64, "offset": 134217728u64, "speed": 0, "type": "commit"})),
                4 => ("RESUME", serde_json::Value::Null),
                5 => ("STOP", serde_json::Value::Null),
                _ => ("GUEST_PANICKED", serde_json::json!({"action": "pause", "info": {"type": "hyper-v", "arg1": 0, "arg2": 0, "arg3": 0, "arg4": 0, "arg5": 0}})),
            };
            bmc::Event {
                seq: i,
                received_unix_ms: now - (14 - i) * 37_000,
                event: event.into(),
                data,
                truncated: false,
            }
        })
        .collect();
    let config: bmc::Config = serde_json::from_value(serde_json::json!({
        "name": "qrab-demo", "memory_mib": 1536, "cpus": 2, "accelerator": "kvm", "boot": "uefi",
        "console": true, "tpm": true, "guest_network": false,
        "disks": [
            {"kind": "file", "id": "esp", "format": "raw", "read_only": true, "boot_index": 0},
            {"kind": "iscsi", "id": "data", "bus": "virtio", "read_only": false, "portal": "10.203.0.1:3260",
             "target": "iqn.2026-11.com.yapplesauce:qrab.disk.e309f751-5dda-4271-a0f4-4454c9527a59.data"}
        ],
        "startup_timeout_secs": 300, "shutdown_grace_secs": 30, "redacted": false,
        "state_id": "e309f751-5dda-4271-a0f4-4454c9527a59",
        "hotplug": {"max_cpus": 4, "max_memory_mib": 4096, "phase": "applied"},
        "deployment": {"node": "kube03", "pod": "qrab-demo", "namespace": "qrab",
            "image": "zot.yapplesauce.com/qrab/manager@sha256:373b51bbb3d05add0160b10277826634b9e2efe7d7f15a27e2a849cae233102c"}
    }))
    .unwrap();
    Panel {
        status: Ok(serde_json::from_value(serde_json::json!({"phase": "running", "ready": true,
            "run_state": "running", "qemu_version": [11, 1, 3], "observed_unix_ms": now,
            "detail": "QEMU is running; the guest agent answered 2 s ago and the last hot-plug applied 2 vCPUs and 1536 MiB"})).unwrap()),
        agent: Ok(serde_json::from_value(serde_json::json!({"status": {"enabled": true, "connected": true,
            "version": "qrab-guest-agent 0.1.0 (rustc 1.97.1, x86_64-unknown-linux-musl)",
            "connected_since_unix_ms": now - 3_600_000, "last_seen_unix_ms": now - 2_000,
            "last_error": "guest-fsfreeze-thaw: Command not permitted while filesystems are not frozen"},
            "freeze": "thawed",
            "update": {"state": "succeeded", "run-id": 7, "exit-code": 0, "hook-installed": true,
                "output": "Fetched 4 packages (12.3 MB)\nUnpacking linux-image-6.12.116+deb13-amd64 (6.12.116-1) over (6.12.115-1)\nSetting up qrab-guest-agent (0.1.0) ...", "error": null}})).unwrap()),
        events: Ok(bmc::Events { events, latest: 13, missed: 0, qmp_dropped: 2 }),
        config: Ok(config),
    }
}

/// A dashboard-launched CirrOS VM with two NICs (one still pending), status
/// IPs and cloud-init, plus its Pod.
fn network_vm() -> (VirtualMachine, Pod) {
    let vm = serde_json::from_value(serde_json::json!({
        "apiVersion": "qrab.yapplesauce.com/v1alpha1", "kind": "VirtualMachine",
        "metadata": {"name": "web-1", "namespace": "qrab", "uid": "4f0c2a6e-9b1d-4c55-8e2a-1d7f3b9c0a11",
            "labels": {"app.kubernetes.io/managed-by": "qrab-dashboard"}},
        "spec": {"stateVolumeClaim": "web-1-state", "cpus": 2, "memoryMiB": 1024, "accelerator": "kvm",
            "storageNode": "kube03",
            "disks": [{"id": "root", "sizeBytes": 2147483648u64, "baseImage": {"name": "cirros", "generation": 1}, "bootIndex": 0}],
            "console": {"adminGroups": ["12f802c8-4daf-4481-9f6a-a6fdb5d086c3"], "viewerGroups": ["2724fd1d-78d0-4612-a42e-448094971abb"]},
            "nics": [{"name": "eth0", "securityGroups": ["web", "ssh", "monitoring-scrape"]}, {"name": "eth1"}],
            "cloudInit": {"hostname": "web-1", "userData": "#cloud-config\nssh_authorized_keys: [ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExample sam]\npackages: [nginx]\n"}},
        "status": {"observedGeneration": 3, "phase": "Running", "ready": true, "reason": "", "message": "",
            "podName": "web-1", "nodeName": "kube02",
            "disks": [{"id": "root", "capacityBytes": 2147483648u64, "generation": 1, "phase": "Active"}],
            "nics": [
                {"name": "eth0", "mac": "02:5e:a1:3c:77:0b", "podName": "web-1-nic-eth0", "ip": "10.244.2.187", "ready": true},
                {"name": "eth1", "mac": "02:91:c4:08:2d:e6", "podName": "web-1-nic-eth1", "ip": null, "ready": false}
            ]}
    }))
    .unwrap();
    let pod = serde_json::from_value(serde_json::json!({
        "metadata": {"name": "web-1", "namespace": "qrab"},
        "spec": {"containers": [{"name": "manager", "image": "zot.yapplesauce.com/qrab/manager@sha256:373b51bbb3d05add0160b10277826634b9e2efe7d7f15a27e2a849cae233102c",
            "resources": {"requests": {"cpu": "2", "memory": "1280Mi"}, "limits": {"cpu": "2", "memory": "1280Mi"}}}]},
        "status": {"podIP": "10.244.2.141"}
    }))
    .unwrap();
    (vm, pod)
}

/// A sample `images.json`, as `contrib/images/stage-base-image.sh` writes it.
const CATALOG: &str = r#"[
 {"name":"cirros","generation":1,"title":"CirrOS 0.6.3","os":"cirros","version":"0.6.3",
  "size_bytes":117440512,"sha256":"809dc3e0e993cd8ec736e28d55b8356ccc4ee9051cad9ad1be66fad9a79470f1",
  "upstream_sha256":"7d6355852aeb6dbcd191bcda7cd74f1536cfe5cbf8a10495a7283a8396e4b75b",
  "source_url":"https://download.cirros-cloud.net/0.6.3/cirros-0.6.3-x86_64-disk.img","boot":"uefi"},
 {"name":"cirros","generation":2,"title":"CirrOS 0.6.3 (rebuilt)","os":"cirros","version":"0.6.3",
  "size_bytes":1181116007,"sha256":"aa0f2bb5b61f9bd57f3e0b3d4e0a6c2b8a7e1f9c3d2b4a6e8f0a1c3e5b7d9f1a",
  "upstream_sha256":"7d6355852aeb6dbcd191bcda7cd74f1536cfe5cbf8a10495a7283a8396e4b75b",
  "source_url":"https://download.cirros-cloud.net/0.6.3/cirros-0.6.3-x86_64-disk.img","boot":"uefi"}
]"#;

fn write(dir: &str, name: &str, page: Page, css: &str) {
    let html = ui::render(page)
        .replace("/assets/style.css", css)
        .replace("src=\"/assets/console.js\"", "src=\"about:blank\"");
    std::fs::write(format!("{dir}/{name}.html"), html).unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = &args[0];
    let css = format!("file://{}/assets/style.css", env!("CARGO_MANIFEST_DIR"));
    let user = UserView {
        name: "sam".into(),
        csrf: "csrf".into(),
        launcher: true,
    };
    let mut pairs: Vec<(VirtualMachine, Pod)> = args[1..]
        .chunks(2)
        .map(|c| (load(&c[0]), load(&c[1])))
        .collect();
    pairs.push(network_vm());
    let mut usage = HashMap::new();
    for (vm, pod) in &pairs {
        let name = pod.metadata.name.clone().unwrap_or_default();
        let _ = vm;
        usage.insert(
            name,
            Usage {
                cpu_cores: 0.125,
                memory_bytes: 433.0 * 1024.0 * 1024.0,
            },
        );
    }
    let rows: Vec<_> = pairs
        .iter()
        .map(|(vm, _)| model::row(vm, Role::Admin, &usage))
        .collect();
    let page = |body| Page {
        user: Some(user.clone()),
        notice: None,
        body,
    };
    write(dir, "list", page(Body::List(rows)), &css);
    let catalog = images::parse(CATALOG);
    write(dir, "images", page(Body::Images(catalog.clone())), &css);
    let network = pairs.len() - 1;
    for (name, index, role) in [
        ("detail", 0, Role::Admin),
        ("detail-viewer", 0, Role::Viewer),
        ("detail-network", network, Role::Admin),
    ] {
        let (vm, pod) = &pairs[index];
        let ip = pod
            .status
            .as_ref()
            .and_then(|s| s.pod_ip.as_ref())
            .and_then(|ip| ip.parse().ok());
        let detail = model::detail(vm, role, &usage, Some(pod), ip, &catalog);
        let body = Body::Detail(Box::new(DetailPage {
            detail,
            panel: PanelState::Loaded(Box::new(panel())),
            deletable: role == Role::Admin,
        }));
        write(dir, name, Page { user: Some(user.clone()), notice: Some("Launched test. Your sign-in was refreshed so it includes the VM's new admins group (console and power).".into()), body }, &css);
    }
    write(
        dir,
        "launch",
        page(Body::Launch(LaunchPage {
            namespace: "qrab".into(),
            storage_node: Some("kube03".into()),
            images: catalog.clone(),
            ..Default::default()
        })),
        &css,
    );
    write(
        dir,
        "launch-os",
        page(Body::Launch(LaunchPage {
            values: vec![
                ("name".into(), "web".into()),
                ("os".into(), "cirros:2".into()),
                ("root_size_gib".into(), "1".into()),
                ("nic0_enabled".into(), "on".into()),
                ("nic0_security_groups".into(), "web, SSH".into()),
                ("hostname".into(), "web-1".into()),
            ],
            errors: vec![
                "Root disk size (GiB) must be a whole number from 2 to 1024.".into(),
                "Security group SSH on eth0 must be a DNS label: 1-63 lowercase letters, digits or '-', starting and ending with a letter or digit.".into(),
            ],
            namespace: "qrab".into(),
            storage_node: Some("kube03".into()),
            images: catalog.clone(),
            user_data_cleared: true,
        })),
        &css,
    );
    let report = Report {
        created: vec![
            "Kanidm group qrab_vm_qrab_test_admins (12f802c8-4daf-4481-9f6a-a6fdb5d086c3)".into(),
            "Kanidm group qrab_vm_qrab_test_viewers (2724fd1d-78d0-4612-a42e-448094971abb)".into(),
            "PersistentVolumeClaim test-state".into(),
            "VirtualMachine test".into(),
        ],
        ..Default::default()
    };
    write(
        dir,
        "launched",
        page(Body::Launched(Launched {
            vm_path: Some("/vms/qrab/test".into()),
            report,
        })),
        &css,
    );
    write(
        dir,
        "console",
        page(Body::Console(ConsolePage {
            namespace: "qrab".into(),
            name: "qrab-demo".into(),
            csrf: "preview".into(),
        })),
        &css,
    );
}

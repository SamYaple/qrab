//! Renders the dashboard's pages from saved Kubernetes objects and
//! representative Redfish data, for checking layout in a browser without a
//! cluster or a sign-in. Not part of the shipped image.
//!
//!   cargo run -p qrab-dashboard --example preview -- OUT_DIR VM.json POD.json [VM.json POD.json ...]
//!
//! The first VM gets the detail page; every VM appears in the list. A
//! built-in VM with NICs and cloud-init is always added and gets
//! `detail-network.html`. A sample base-image catalog feeds the Images page
//! and the launch forms.
use k8s_openapi::{api::core::v1::Pod, apimachinery::pkg::api::resource::Quantity};
use qrab_dashboard::{
    access::Role,
    cluster::Usage,
    images,
    launch::Report,
    model,
    redfish::{Panel, System},
    ui::{self, Body, ConsolePage, DetailPage, LaunchPage, Launched, Page, PanelState, UserView},
};
use qrab_operator::api::VirtualMachine;
use std::collections::HashMap;

fn load<T: serde::de::DeserializeOwned>(path: &str) -> T {
    serde_json::from_str(&std::fs::read_to_string(path).expect(path)).expect(path)
}

fn panel() -> Panel {
    let events: Vec<serde_json::Value> = (0..14u64)
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
            serde_json::json!({"Id": i.to_string(), "Created": format!("2026-11-14T10:{:02}:{:02}.000Z", 10 + i / 2, (i * 37) % 60),
                "Severity": if event == "GUEST_PANICKED" { "Critical" } else { "OK" }, "Message": event,
                "Oem": {"Qrab": {"Sequence": i, "Event": event, "Data": data, "Truncated": false}}})
        })
        .rev()
        .collect();
    let now = 1_794_517_200_000u64;
    let system: System = serde_json::from_value(serde_json::json!({
        "PowerState": "On", "Status": {"State": "Enabled", "Health": "OK"},
        "Actions": {"#ComputerSystem.Reset": {"ResetType@Redfish.AllowableValues":
            ["On", "GracefulShutdown", "ForceRestart", "Pause", "Resume", "Nmi", "GracefulRestart", "PowerCycle"]}},
        "Oem": {"Qrab": {
            "Configuration": {
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
            },
            "GuestAgent": {"enabled": true, "connected": true,
                "version": "qrab-guest-agent 0.1.0 (rustc 1.97.1, x86_64-unknown-linux-musl)",
                "connected_since_unix_ms": now - 3_600_000, "last_seen_unix_ms": now - 2_000,
                "last_error": "guest-fsfreeze-thaw: Command not permitted while filesystems are not frozen"}
        }}
    }))
    .unwrap();
    Panel {
        system: Ok(system),
        events: Ok(qrab_dashboard::redfish::Events {
            entries: serde_json::from_value(serde_json::Value::Array(events)).unwrap(),
            qmp_dropped: Some(2),
        }),
        history: history(now / 1000),
    }
}

type Points = Vec<(f64, f64)>;

/// A representative last hour of every MetricHistory metric; NIC tx failed.
fn history(end: u64) -> Vec<qrab_dashboard::redfish::HistoryRead> {
    let end = end as i64;
    let start = end - 3600;
    let wave = |base: f64, amplitude: f64, phase: f64| -> Points {
        (0..=240)
            .map(|i| {
                let t = (start + i * 15) as f64;
                let x = i as f64 / 240.0 * std::f64::consts::TAU * 3.0 + phase;
                (
                    t,
                    (base + amplitude * x.sin() + amplitude * 0.4 * (x * 3.7).cos()).max(0.0),
                )
            })
            .collect()
    };
    let series = |items: Vec<(&str, &str, Points)>| -> Vec<qrab_dashboard::redfish::HistorySeries> {
        items
            .into_iter()
            .map(
                |(key, label, points)| qrab_dashboard::redfish::HistorySeries {
                    labels: [(key.to_owned(), label.to_owned())].into(),
                    points,
                },
            )
            .collect()
    };
    let one = |name: &str, unit: &str, series| qrab_dashboard::redfish::History {
        name: name.into(),
        unit: unit.into(),
        step: 15,
        start,
        end,
        series,
    };
    vec![
        (
            "container-cpu",
            Ok(one(
                "Container CPU",
                "cores",
                series(vec![
                    ("container", "bmc", wave(0.002, 0.001, 0.0)),
                    ("container", "manager", wave(0.12, 0.06, 1.0)),
                    ("container", "redfish", wave(0.003, 0.002, 2.0)),
                ]),
            )),
        ),
        (
            "container-memory",
            Ok(one(
                "Container memory working set",
                "bytes",
                series(vec![
                    ("container", "bmc", wave(2.4e6, 1e5, 0.0)),
                    ("container", "manager", wave(4.9e8, 1.5e7, 0.5)),
                    ("container", "redfish", wave(1.7e6, 1e5, 1.0)),
                ]),
            )),
        ),
        (
            "block-read",
            Ok(one(
                "Disk read",
                "bytes/s",
                series(vec![
                    ("drive", "boot", wave(40_000.0, 35_000.0, 0.3)),
                    ("drive", "data", wave(8_000.0, 6_000.0, 2.2)),
                ]),
            )),
        ),
        (
            "block-write",
            Ok(one(
                "Disk write",
                "bytes/s",
                series(vec![
                    ("drive", "boot", wave(12_000.0, 10_000.0, 1.3)),
                    ("drive", "data", wave(90_000.0, 60_000.0, 0.1)),
                ]),
            )),
        ),
        ("nic-rx", Ok(one("Network received", "bytes/s", Vec::new()))),
        ("nic-tx", Err("Redfish answered HTTP 503".into())),
    ]
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
            "nics": [{"name": "eth0", "securityGroups": ["web", "ssh", "monitoring-scrape"]}, {"name": "eth1", "vpn": {"secretName": "vpn-private-fixture"}}],
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
        .replace(
            "/assets/favicon.svg",
            &format!(
                "file://{}/../bmc/assets/favicon.svg",
                env!("CARGO_MANIFEST_DIR")
            ),
        )
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
    let storage_rows = [
        ("pending-storage", 1, 0, "Provisioning"),
        ("static-media", 1, 0, "Running"),
        ("without-disks", 0, 0, "Running"),
        ("reported-storage", 3, 8 * 1024u64.pow(3), "Running"),
    ]
    .into_iter()
    .map(|(name, disks, bytes, phase)| {
        let mut row = rows[0].clone();
        row.name = name.into();
        row.disks = disks;
        row.disk_bytes = bytes;
        row.phase = phase.into();
        row.ready = phase == "Running";
        row
    })
    .collect();
    write(dir, "list-storage", page(Body::List(storage_rows)), &css);
    write(dir, "list", page(Body::List(rows)), &css);
    write(
        dir,
        "list-empty-launcher",
        page(Body::List(Vec::new())),
        &css,
    );
    write(
        dir,
        "list-empty-viewer",
        Page {
            user: Some(UserView {
                launcher: false,
                ..user.clone()
            }),
            notice: None,
            body: Body::List(Vec::new()),
        },
        &css,
    );
    let catalog = images::parse(CATALOG);
    write(dir, "images", page(Body::Images(catalog.clone())), &css);
    write(dir, "images-empty", page(Body::Images(Vec::new())), &css);
    let (mut long_vm, mut long_pod) = network_vm();
    let long_name = format!("vm-{}", "a".repeat(60));
    long_vm.metadata.name = Some(long_name.clone());
    if let Some(labels) = &mut long_vm.metadata.labels {
        labels.remove("app.kubernetes.io/managed-by");
    }
    long_vm.spec.state_volume_claim = format!("{long_name}-state");
    long_vm.spec.cpus = 8;
    long_vm.spec.max_cpus = Some(256);
    long_vm.spec.memory_mib = 16384;
    long_vm.spec.max_memory_mib = Some(1048576);
    long_pod.metadata.name = Some(long_name.clone());
    if let Some(resources) = long_pod
        .spec
        .as_mut()
        .and_then(|pod| pod.containers.first_mut())
        .and_then(|container| container.resources.as_mut())
    {
        resources.requests = Some(
            [
                ("cpu".into(), Quantity("8".into())),
                ("memory".into(), Quantity("17Gi".into())),
            ]
            .into(),
        );
        resources.limits = Some(
            [
                ("cpu".into(), Quantity("256".into())),
                ("memory".into(), Quantity("1025Gi".into())),
            ]
            .into(),
        );
    }
    if let Some(status) = &mut long_vm.status {
        status.pod_name = Some(long_name.clone());
        status.node_name = Some(format!("production-worker-{}", "b".repeat(45)));
        status.ready = false;
        status.message = format!(
            "Waiting for storage volume {} to finish provisioning.",
            "c".repeat(120)
        );
    }
    let mut long_user = user.clone();
    long_user.name = "sam.with-a-long-operator-name-for-production".into();
    let long_usage = HashMap::from([(
        long_name.clone(),
        Usage {
            cpu_cores: 8.125,
            memory_bytes: 15.5 * 1024.0 * 1024.0 * 1024.0,
        },
    )]);
    write(
        dir,
        "list-long",
        Page {
            user: Some(long_user.clone()),
            body: Body::List(vec![model::row(&long_vm, Role::Admin, &long_usage)]),
            ..Default::default()
        },
        &css,
    );
    let mut long_panel = panel();
    if let Ok(system) = &mut long_panel.system
        && let Some(config) = &mut system.oem.qrab.configuration
    {
        config.name = long_name;
        config.cpus = 8;
        config.memory_mib = 16384;
        if let Some(limits) = &mut config.hotplug {
            limits.max_cpus = 256;
            limits.max_memory_mib = 1048576;
        }
    }
    write(
        dir,
        "detail-long",
        Page {
            user: Some(long_user),
            body: Body::Detail(Box::new(DetailPage {
                detail: model::detail(
                    &long_vm,
                    Role::Admin,
                    &long_usage,
                    Some(&long_pod),
                    Some("10.244.2.141".parse().unwrap()),
                    &catalog,
                ),
                panel: PanelState::Loaded(Box::new(long_panel)),
                deletable: false,
            })),
            ..Default::default()
        },
        &css,
    );
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
        let mut live_panel = panel();
        if role == Role::Viewer
            && let Ok(system) = &mut live_panel.system
        {
            if let Some(config) = &mut system.oem.qrab.configuration {
                config.redacted = true;
                config.state_id = None;
                config.deployment = None;
            }
            if let Some(update) = system
                .oem
                .qrab
                .guest_agent
                .as_mut()
                .and_then(|a| a.last_update.as_mut())
            {
                update.output = None;
                update.error = None;
            }
        }
        if name == "detail-network" {
            for (page_name, power_state) in [
                ("detail-paused", Some("Paused")),
                ("detail-powered-off", Some("Off")),
                ("detail-powering-on", Some("PoweringOn")),
                ("detail-unknown-power", None),
            ] {
                let mut snapshot = live_panel.clone();
                if let Ok(system) = &mut snapshot.system {
                    system.power_state = power_state.map(str::to_owned);
                }
                let mut snapshot_detail = detail.clone();
                snapshot_detail.bmc_enabled = false;
                write(
                    dir,
                    page_name,
                    page(Body::Detail(Box::new(DetailPage {
                        detail: snapshot_detail,
                        panel: PanelState::Loaded(Box::new(snapshot)),
                        deletable: true,
                    }))),
                    &css,
                );
            }
            for (page_name, connected, frozen) in [
                ("detail-needs-agent", false, false),
                ("detail-frozen-agent", true, true),
            ] {
                let mut blocked = live_panel.clone();
                if let Ok(system) = &mut blocked.system
                    && let Some(agent) = &mut system.oem.qrab.guest_agent
                {
                    agent.connected = connected;
                    agent.freeze_state = Some(if frozen { "Frozen" } else { "Thawed" }.into());
                }
                write(
                    dir,
                    page_name,
                    page(Body::Detail(Box::new(DetailPage {
                        detail: detail.clone(),
                        panel: PanelState::Loaded(Box::new(blocked)),
                        deletable: true,
                    }))),
                    &css,
                );
            }
            let mut no_actions = live_panel.clone();
            if let Ok(system) = &mut no_actions.system {
                system.actions.reset.allowable_values.clear();
            }
            write(
                dir,
                "detail-no-power-actions",
                page(Body::Detail(Box::new(DetailPage {
                    detail: detail.clone(),
                    panel: PanelState::Loaded(Box::new(no_actions)),
                    deletable: true,
                }))),
                &css,
            );
            let mut redfish_only = detail.clone();
            redfish_only.bmc_enabled = false;
            write(
                dir,
                "detail-redfish-only",
                page(Body::Detail(Box::new(DetailPage {
                    detail: redfish_only,
                    panel: PanelState::Loaded(Box::new(live_panel.clone())),
                    deletable: true,
                }))),
                &css,
            );
        }
        let body = Body::Detail(Box::new(DetailPage {
            detail,
            panel: PanelState::Loaded(Box::new(live_panel)),
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
        "launch-unavailable",
        page(Body::Launch(LaunchPage {
            namespace: "a".repeat(63),
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
                qrab_dashboard::launch::FormError::at(
                    "root_size_gib",
                    "Root disk size (GiB) must be a whole number from 2 to 1024.",
                ),
                qrab_dashboard::launch::FormError::at(
                    "nic0_security_groups",
                    "Security group SSH on eth0 must be a DNS label: 1-63 lowercase letters, digits or '-', starting and ending with a letter or digit.",
                ),
            ],
            namespace: "qrab".into(),
            storage_node: Some("kube03".into()),
            images: catalog.clone(),
            user_data_cleared: true,
        })),
        &css,
    );
    write(
        dir,
        "launch-options",
        page(Body::Launch(LaunchPage {
            values: vec![
                ("name".into(), "web".into()),
                ("cpus".into(), "2".into()),
                ("max_cpus".into(), "1".into()),
                ("memory_mib".into(), "512".into()),
                ("disk0_id".into(), "logs".into()),
                ("disk0_size_gib".into(), "0".into()),
                ("disk1_id".into(), "scratch".into()),
                ("disk1_size_gib".into(), "4".into()),
                ("disk1_base_image".into(), "cirros".into()),
                ("disk1_base_generation".into(), "invalid".into()),
            ],
            errors: vec![
                qrab_dashboard::launch::FormError::at(
                    "max_cpus",
                    "Maximum vCPUs must be at least vCPUs.",
                ),
                qrab_dashboard::launch::FormError::at(
                    "disk0_size_gib",
                    "Disk 1 size (GiB) must be a whole number from 1 to 1024.",
                ),
                qrab_dashboard::launch::FormError::at(
                    "disk1_base_generation",
                    "Disk 2 base image generation must be a whole number from 1 to 999999.",
                ),
            ],
            namespace: "qrab".into(),
            storage_node: Some("kube03".into()),
            images: catalog.clone(),
            ..Default::default()
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
    for (name, report) in [
        (
            "launch-failed-empty",
            Report {
                failed: Some(("Check that the name is free".into(), "VirtualMachine test already exists.".into())),
                ..Default::default()
            },
        ),
        (
            "launch-failed-left",
            Report {
                created: vec!["Kanidm group qrab_vm_qrab_test_admins".into(), "PersistentVolumeClaim qrab/test-state".into()],
                failed: Some(("Create the VirtualMachine".into(), "The VM service is temporarily unavailable.".into())),
                rolled_back: vec!["Kanidm group qrab_vm_qrab_test_admins".into()],
                left: vec!["PersistentVolumeClaim qrab/test-state: kept (the dashboard cannot delete PVCs); a retry with the same name reuses it while it is unbound".into()],
            },
        ),
    ] {
        write(dir, name, page(Body::Launched(Launched { vm_path: None, report })), &css);
    }
    write(
        dir,
        "console",
        page(Body::Console(ConsolePage {
            namespace: "qrab".into(),
            name: "web-1".into(),
            csrf: "preview".into(),
            audio: true,
        })),
        &css,
    );
    for (name, status, message, return_to) in [
        (
            "error-session",
            401,
            "Sign in again to continue.",
            Some("/vms/qrab/web-1/bmc"),
        ),
        (
            "error-denied",
            403,
            "The console requires the admin role on this VM.",
            Some("/vms/qrab/web-1/console"),
        ),
        (
            "error-missing",
            404,
            "This VM does not exist or is not shared with you.",
            Some("/vms/qrab/missing"),
        ),
        (
            "error-service",
            503,
            "The VM service could not complete this request.",
            Some("/vms/qrab/web-1"),
        ),
    ] {
        write(
            dir,
            name,
            Page {
                body: Body::Error(ui::ErrorPage {
                    status,
                    message: message.into(),
                    return_to: return_to.map(str::to_owned),
                }),
                ..Default::default()
            },
            &css,
        );
    }
}

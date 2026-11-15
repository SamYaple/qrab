//! Server-rendered Dioxus pages. Every VM-, guest- or user-supplied string is
//! rendered as escaped text or an escaped attribute value.
use crate::{
    access::Role,
    images::BaseImage,
    launch::{self, Report},
    model::{self, Detail, Row},
    power::Power,
    redfish::{self, Panel, ResetType},
};
use dioxus::prelude::*;

#[derive(Clone)]
pub struct UserView {
    pub name: String,
    pub csrf: String,
    pub launcher: bool,
}

#[derive(Clone)]
pub enum PanelState {
    Disabled,
    NoPod,
    Loaded(Box<Panel>),
}

#[derive(Clone)]
pub struct DetailPage {
    pub detail: Detail,
    pub panel: PanelState,
    /// A console admin on a VM launched by this dashboard.
    pub deletable: bool,
}

#[derive(Clone, Default)]
pub struct LaunchPage {
    pub values: Vec<(String, String)>,
    pub errors: Vec<String>,
    pub namespace: String,
    pub storage_node: Option<String>,
    /// The base-image catalog at render time.
    pub images: Vec<BaseImage>,
    /// The rejected form had user-data, which is never sent back.
    pub user_data_cleared: bool,
}

/// Where security groups and the default-allow rule are described.
pub const NETWORKING_DOCS: &str = "https://gitea.yapplesauce.com/infrastructure/qrab/src/branch/main/docs/networking.md#security-groups";

#[derive(Clone)]
pub struct Launched {
    pub vm_path: Option<String>,
    pub report: Report,
}

#[derive(Clone)]
pub struct ConsolePage {
    pub namespace: String,
    pub name: String,
    /// For the page script's power requests (`X-CSRF-Token`).
    pub csrf: String,
    /// The VM enables admin-bucket audio: the page offers the speaker toggle.
    pub audio: bool,
}

/// One console power button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerButton {
    /// `on`, `off` or a Redfish ResetType.
    pub action: &'static str,
    pub label: &'static str,
    /// Armed by a first click and sent by a second.
    pub disruptive: bool,
    /// A Redfish reset, shown only while `ResetActionInfo` allows it.
    pub in_pod: bool,
}

/// The console page's power buttons: Power on, every offered Redfish reset,
/// then Power off.
pub fn console_power() -> Vec<PowerButton> {
    let mut buttons = vec![PowerButton {
        action: "on",
        label: "Power on",
        disruptive: false,
        in_pod: false,
    }];
    buttons.extend(ResetType::ALL.into_iter().map(|reset| PowerButton {
        action: reset.name(),
        label: reset.label(),
        disruptive: reset.needs_confirmation(),
        in_pod: true,
    }));
    buttons.push(PowerButton {
        action: "off",
        label: "Power off",
        disruptive: true,
        in_pod: false,
    });
    buttons
}

#[derive(Clone, Default)]
pub enum Body {
    #[default]
    Welcome,
    Error(String),
    List(Vec<Row>),
    Detail(Box<DetailPage>),
    Launch(LaunchPage),
    Launched(Launched),
    Console(ConsolePage),
    Images(Vec<BaseImage>),
}

#[derive(Clone, Default)]
pub struct Page {
    pub user: Option<UserView>,
    pub notice: Option<String>,
    pub body: Body,
}

pub fn render(page: Page) -> String {
    let title = match &page.body {
        Body::Detail(d) => format!("{} · qrab VMs", d.detail.row.name),
        Body::Console(c) => format!("{} console · qrab VMs", c.name),
        Body::Launch(_) | Body::Launched(_) => "Launch a VM · qrab VMs".into(),
        Body::Images(_) => "Base images · qrab VMs".into(),
        _ => "qrab VMs".into(),
    };
    let mut dom = VirtualDom::new(app);
    dom.provide_root_context(page);
    dom.rebuild_in_place();
    let mut escaped = String::new();
    for c in title.chars() {
        match c {
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            c => escaped.push(c),
        }
    }
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{escaped}</title><link rel=\"stylesheet\" href=\"/assets/style.css\"></head>{}</html>",
        dioxus_ssr::render(&dom)
    )
}

fn sizes(current: u32, max: Option<u32>, unit: &str) -> String {
    match max {
        Some(max) => format!("{current} / {max}{unit}"),
        None => format!("{current}{unit}"),
    }
}

fn state_badge(row: &Row) -> Element {
    let class = if row.ready { "badge ok" } else { "badge" };
    let text = if row.ready {
        format!("{} · ready", row.phase)
    } else {
        format!("{} · not ready", row.phase)
    };
    rsx! { span { class: class, "{text}" } }
}

fn vm_table(rows: &[Row]) -> Element {
    rsx! {
        section { class: "card directory", div { class: "table-wrap", table {
            caption { class: "visually-hidden", "Virtual machines you can access" }
            thead { tr {
                th { scope: "col", "Virtual machine" } th { scope: "col", "State" }
                th { scope: "col", "Compute" } th { scope: "col", "Storage" }
                th { scope: "col", "Live usage" } th { scope: "col", "Location" } th { scope: "col", "Access" }
            } }
            tbody { for row in rows {
                {
                    let (cpu, mem) = model::usage_text(row.usage);
                    let cpu_size = sizes(row.cpus, row.max_cpus, "");
                    let memory_size = sizes(row.memory_mib, row.max_memory_mib, "");
                    let disks = match (row.disks, row.disk_bytes) {
                        (0, _) => "none".to_string(),
                        (n, 0) => format!("{n} static"),
                        (n, bytes) => format!("{n} · {}", model::gib(bytes)),
                    };
                    rsx! { tr {
                        td { "data-label": "Name", div { a { class: "identity-link", href: row.path(), "{row.name}" } p { class: "muted", "{row.namespace}" } } }
                        td { "data-label": "State", div { {state_badge(row)} } }
                        td { "data-label": "Compute", div {
                            span { "{cpu_size} vCPUs" }
                            span { class: "cell-detail", "{memory_size} MiB memory" }
                        } }
                        td { "data-label": "Storage", "{disks}" }
                        td { "data-label": "Live usage", div {
                            span { "{cpu} CPU" }
                            span { class: "cell-detail", "{mem} memory" }
                        } }
                        td { "data-label": "Location", div {
                            if row.node.is_empty() { span { class: "muted", "Not scheduled" } } else { span { "{row.node}" } }
                            if let Some(ip) = &row.ip { code { class: "cell-detail", "{ip}" } }
                            else { span { class: "cell-detail", "No guest IP" } }
                        } }
                        td { "data-label": "Access", div { span { class: if row.role == Role::Admin { "badge admin" } else { "badge" }, {row.role.label()} } } }

                    } }
                }
            } }
        } } }
    }
}

/// Values that are identifiers render as code; multi-line output keeps its
/// line breaks.
const CODE_KEYS: &[&str] = &["Image", "State ID", "State PVC", "Pod", "VM UID"];

fn dl(rows: Vec<(&'static str, String)>) -> Element {
    rsx! { dl { class: "identity-details",
        for (key, value) in rows {
            dt { "{key}" }
            if value.contains('\n') { dd { pre { class: "output", "{value}" } } }
            else if CODE_KEYS.contains(&key) { dd { code { "{value}" } } }
            else { dd { "{value}" } }
        }
    } }
}

fn panel_error(title: &str, error: &str) -> Element {
    rsx! { section { class: "card", h3 { "{title}" } p { class: "muted", role: "status", "Unavailable: {error}" } } }
}

fn stamp(ms: Option<u64>) -> String {
    match ms {
        Some(ms) if ms > 0 => time::OffsetDateTime::from_unix_timestamp((ms / 1000) as i64)
            .map(|t| {
                format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
                    t.year(),
                    t.month() as u8,
                    t.day(),
                    t.hour(),
                    t.minute(),
                    t.second()
                )
            })
            .unwrap_or_else(|_| "—".into()),
        _ => "—".into(),
    }
}

fn or_dash(value: &Option<String>) -> String {
    value.clone().unwrap_or_else(|| "—".into())
}

fn bmc_panel(panel: &Panel) -> Element {
    let (status, agent, config) = match &panel.system {
        Ok(system) => (
            rsx! { section { class: "card", h3 { "Status" } {dl(vec![
                ("Power state", or_dash(&system.power_state)),
                ("State", or_dash(&system.status.state)),
                ("Health", or_dash(&system.status.health)),
            ])} } },
            agent_card(system.oem.qrab.guest_agent.as_ref()),
            config_card(system.oem.qrab.configuration.as_ref()),
        ),
        Err(e) => (
            panel_error("Status", e),
            panel_error("Guest agent", e),
            panel_error("Effective configuration", e),
        ),
    };
    let events = match &panel.events {
        Ok(page) => rsx! {
            details { class: "card wide disclosure", id: "recent-events",
                summary { span { class: "disclosure-title", "Recent QMP events" }
                    span { class: "muted", "{page.entries.len()} recent" }
                    if let Some(dropped) = page.qmp_dropped.filter(|d| *d > 0) {
                        span { class: "badge warning", "{dropped} dropped" }
                    }
                }
                if let Some(dropped) = page.qmp_dropped.filter(|d| *d > 0) { p { class: "notice warning", role: "status", "{dropped} QMP event(s) were dropped before the manager recorded them." } }
                if page.entries.is_empty() { p { class: "empty", "No events recorded." } }
                else { ol { class: "events", reversed: true,
                    for entry in page.entries.iter() {
                        {
                            let data = redfish::event_data(&entry.oem.qrab);
                            rsx! { li {
                                div { class: "event-meta",
                                    time { datetime: "{entry.created}", "{entry.created}" }
                                    b { "{entry.oem.qrab.event}" }
                                    if !entry.severity.is_empty() && entry.severity != "OK" { span { class: "badge warning", "{entry.severity}" } }
                                }
                                if !data.is_empty() { code { "{data}" } }
                            } }
                        }
                    }
                } }
            }
        },
        Err(e) => panel_error("Recent QMP events", e),
    };
    let history = history_card(&panel.history);
    rsx! {
        p { class: "muted", "Read from the VM's Redfish service with your sign-in." }
        div { class: "panel-grid", {status} {agent} {history} {config} {events} }
    }
}

/// Sparkline geometry: the viewBox the polylines are drawn in.
const SPARK_W: f64 = 240.0;
const SPARK_H: f64 = 48.0;
/// Series colours are CSS classes (`spark-0`..), as the page CSP forbids inline styles.
const SPARK_COLOURS: usize = 4;

/// A value in a history unit, short enough for a legend.
pub fn history_value(value: f64, unit: &str) -> String {
    let scaled = |value: f64, suffix: &str| {
        let units = ["B", "KiB", "MiB", "GiB", "TiB"];
        let mut value = value;
        let mut index = 0;
        while value.abs() >= 1024.0 && index + 1 < units.len() {
            value /= 1024.0;
            index += 1;
        }
        let digits = if index == 0 || value.abs() >= 10.0 {
            0
        } else {
            1
        };
        format!("{value:.digits$} {}{suffix}", units[index])
    };
    match unit {
        "bytes" => scaled(value, ""),
        "bytes/s" => scaled(value, "/s"),
        "cores" if value.abs() < 1.0 => format!("{:.0} mcores", value * 1000.0),
        "cores" => format!("{value:.2} cores"),
        unit => format!("{value} {unit}"),
    }
}

/// One series as polyline points, scaled to `[start, end] x [0, max]`.
pub fn spark_points(points: &[(f64, f64)], start: f64, end: f64, max: f64) -> String {
    let span = (end - start).max(1.0);
    let max = if max > 0.0 { max } else { 1.0 };
    points
        .iter()
        .map(|(t, v)| {
            let x = ((t - start) / span).clamp(0.0, 1.0) * SPARK_W;
            let y = SPARK_H - (v / max).clamp(0.0, 1.0) * (SPARK_H - 2.0) - 1.0;
            format!("{x:.1},{y:.1}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn sparkline(metric: &str, history: &redfish::History) -> Element {
    let max = history
        .series
        .iter()
        .flat_map(|s| s.points.iter().map(|(_, v)| *v))
        .fold(0.0_f64, f64::max);
    let lines: Vec<_> = history
        .series
        .iter()
        .enumerate()
        .map(|(index, series)| {
            let latest = series
                .points
                .last()
                .map(|(_, v)| history_value(*v, &history.unit))
                .unwrap_or_else(|| "no data".into());
            (
                format!("spark-{}", index % SPARK_COLOURS),
                series.label(),
                spark_points(
                    &series.points,
                    history.start as f64,
                    history.end as f64,
                    max,
                ),
                latest,
            )
        })
        .collect();
    let has_data = history.series.iter().any(|s| !s.points.is_empty());
    let summary = format!(
        "{}, last hour, peak {}; latest {}.",
        history.name,
        history_value(max, &history.unit),
        lines
            .iter()
            .map(|(_, label, _, latest)| format!("{label} {latest}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let title = if history.name.is_empty() {
        history_title(metric).to_owned()
    } else {
        history.name.clone()
    };
    rsx! {
        figure { class: "spark", "data-metric": "{metric}",
            if !has_data { figcaption { b { "{title}" } } p { class: "muted", "No data in the last hour." } }
            else { figcaption { b { "{title}" } span { class: "muted", " · peak {history_value(max, &history.unit)}" } }
            svg { view_box: "0 0 240 48", preserve_aspect_ratio: "none", role: "img", "aria-label": "{summary}",
                line { class: "spark-axis", x1: "0", y1: "47", x2: "240", y2: "47" }
                for (class, _, points, _) in lines.iter() {
                    polyline { class: "{class}", points: "{points}" }
                }
            }
            ul { class: "spark-legend",
                for (class, label, _, latest) in lines.iter() {
                    li { span { class: "swatch {class}" } code { "{label}" } " {latest}" }
                }
            } }
        }
    }
}

/// A heading for a history metric whose answer carried none.
fn history_title(metric: &str) -> &str {
    match metric {
        "container-cpu" => "Container CPU",
        "container-memory" => "Container memory working set",
        "block-read" => "Disk read",
        "block-write" => "Disk write",
        "nic-rx" => "Network received",
        "nic-tx" => "Network sent",
        other => other,
    }
}

fn history_card(history: &[redfish::HistoryRead]) -> Element {
    rsx! {
        section { class: "card wide", h3 { "Metric history (last hour)" }
            p { class: "muted", "From the cluster Prometheus through the VM's Redfish MetricHistory: container CPU and memory from kubelet cAdvisor, disk and network rates from the Redfish export." }
            div { class: "spark-grid",
                for (metric, read) in history.iter() {
                    match read {
                        Ok(h) => sparkline(metric, h),
                        Err(e) => rsx! { figure { class: "spark", "data-metric": "{metric}",
                            figcaption { b { {history_title(metric)} } }
                            p { class: "muted", role: "status", "Unavailable: {e}" } } },
                    }
                }
            }
        }
    }
}

fn agent_card(agent: Option<&redfish::AgentStatus>) -> Element {
    let Some(s) = agent else {
        return panel_error("Guest agent", "the manager did not report it");
    };
    let mut rows = vec![
        (
            "Channel",
            if s.enabled {
                "enabled".into()
            } else {
                "disabled".into()
            },
        ),
        (
            "Connected",
            if s.connected {
                format!("yes, since {}", stamp(s.connected_since_unix_ms))
            } else {
                "no".into()
            },
        ),
        ("Version", or_dash(&s.version)),
        ("Last seen", stamp(s.last_seen_unix_ms)),
    ];
    if let Some(e) = &s.last_error {
        rows.push(("Last error", e.clone()));
    }
    if s.connected {
        let freeze = s.freeze_state.as_deref().unwrap_or("unknown");
        rows.push(("Filesystems", freeze.to_ascii_lowercase()));
        if let Some(update) = &s.last_update {
            rows.push(("Last update", update_summary(update)));
            if let Some(error) = &update.error {
                rows.push(("Update error", error.clone()));
            }
            if let Some(output) = update.output.as_ref().filter(|o| !o.is_empty()) {
                let label = if update.output_truncated {
                    "Update output (tail)"
                } else {
                    "Update output"
                };
                rows.push((label, output.clone()));
            }
        }
    }
    rsx! { section { class: "card", h3 { "Guest agent" } {dl(rows)} } }
}

fn update_summary(update: &redfish::LastUpdate) -> String {
    let mut summary = update.state.to_ascii_lowercase();
    if update.run_id > 0 {
        summary += &format!(" · run {}", update.run_id);
    }
    match (update.exit_code, update.signal) {
        (Some(code), _) => summary += &format!(" · exit {code}"),
        (None, Some(signal)) => summary += &format!(" · signal {signal}"),
        _ => {}
    }
    if let Some(end) = &update.end_time {
        summary += &format!(" · {end}");
    }
    if !update.hook_installed {
        summary += " · no hook installed";
    }
    summary
}

fn config_card(config: Option<&redfish::Config>) -> Element {
    let Some(c) = config else {
        return panel_error("Effective configuration", "the manager did not report it");
    };
    let mut rows = vec![
        ("Name", c.name.clone()),
        ("Memory", format!("{} MiB", c.memory_mib)),
        ("vCPUs", c.cpus.to_string()),
        ("Accelerator", c.accelerator.clone()),
        ("Boot", c.boot.clone()),
        ("TPM", if c.tpm { "yes".into() } else { "no".into() }),
        (
            "Guest network",
            if c.guest_network {
                "enabled".into()
            } else {
                "disabled".into()
            },
        ),
        ("Startup timeout", format!("{}s", c.startup_timeout_secs)),
        ("Shutdown grace", format!("{}s", c.shutdown_grace_secs)),
    ];
    for disk in &c.disks {
        rows.push(("Disk", redfish::describe_disk(disk)));
    }
    if let Some(h) = &c.hotplug {
        rows.push((
            "Hot-plug maximum",
            format!("{} vCPUs · {} MiB", h.max_cpus, h.max_memory_mib),
        ));
        if let Some(phase) = &h.phase {
            rows.push(("Hot-plug phase", phase.clone()));
        }
        if let Some(e) = &h.last_error {
            rows.push(("Hot-plug error", e.clone()));
        }
    }
    if let Some(id) = &c.state_id {
        rows.push(("State ID", id.clone()));
    }
    for (key, value) in c.deployment.iter().flatten() {
        let label: &'static str = match key.as_str() {
            "node" => "Node",
            "pod" => "Pod",
            "namespace" => "Namespace",
            "image" => "Image",
            _ => continue,
        };
        rows.push((label, value.clone()));
    }
    let note = if c.redacted {
        "Sensitive fields hidden for viewers"
    } else {
        "Full view"
    };
    let has_error = c
        .hotplug
        .as_ref()
        .and_then(|h| h.last_error.as_ref())
        .is_some();
    rsx! { details { class: "card wide disclosure", id: "effective-configuration", open: has_error,
        summary { span { class: "disclosure-title", "Effective configuration" }
            span { class: "muted", "Boot, devices and deployment" }
            if has_error { span { class: "badge warning", "Resize error" } }
        }
        p { class: "muted", "{note}" } {dl(rows)}
    } }
}

/// In-pod power, offered as Redfish's `Systems/1` currently allows. Power on
/// and off (`spec.running`) are on the console page.
fn power_controls(detail: &Detail, csrf: &str, panel: &PanelState) -> Element {
    let action = format!("{}/action", detail.row.path());
    let allowed = match panel {
        PanelState::Loaded(panel) => panel
            .system
            .as_ref()
            .ok()
            .map(|system| redfish::offered(&system.actions.reset.allowable_values)),
        _ => None,
    };
    // Without the BMC there is no console page, so Power on and off are here.
    let spec_power = (!detail.bmc_enabled).then_some([Power::On, Power::Off]);
    rsx! {
        section { class: "card", aria_labelledby: "power-title",
            h2 { id: "power-title", "Power" }
            if !detail.redfish_enabled {
                p { class: "muted", role: "status", "In-pod actions (restart, shut down, pause) need the VM's Redfish service, and components.redfish is not enabled for this VM. Power on sets spec.running; Power off stops the VM and keeps it off." }
            } else if detail.bmc_enabled {
                p { class: "muted", "Sent to the VM's Redfish service with your sign-in. Shut down and Force off leave the VM powered off in its Pod; Power on (on the console page) starts it again in place. Power off on the console page stops the VM and keeps it off." }
            } else {
                p { class: "muted", "Sent to the VM's Redfish service with your sign-in. Shut down and Force off leave the VM powered off in its Pod; Power on starts it again in place. Power off stops the VM and keeps it off." }
            }
            if let Some(actions) = spec_power { div { class: "actions",
                for a in actions {
                    form { method: "post", action: "{action}",
                        input { r#type: "hidden", name: "csrf", value: "{csrf}" }
                        input { r#type: "hidden", name: "action", value: a.key() }
                        if a.needs_confirmation() {
                            label { class: "confirm", input { r#type: "checkbox", name: "confirm", value: "yes", required: true } "Confirm" }
                        }
                        button { class: if a.needs_confirmation() { "danger" } else { "quiet" }, r#type: "submit", {a.label()} }
                    }
                }
            } }
            match allowed {
                None if !detail.redfish_enabled => rsx! {},
                None => rsx! { p { class: "muted", role: "status", "In-pod power actions are unavailable while the VM's Redfish service cannot be read." } },
                Some(allowed) => rsx! { div { class: "actions",
                    for a in allowed {
                        form { method: "post", action: "{action}",
                            input { r#type: "hidden", name: "csrf", value: "{csrf}" }
                            input { r#type: "hidden", name: "action", value: a.name() }
                            if a.needs_confirmation() {
                                label { class: "confirm", input { r#type: "checkbox", name: "confirm", value: "yes", required: true } "Confirm" }
                            }
                            button { class: if a.needs_confirmation() { "danger" } else { "quiet" }, r#type: "submit", {a.label()} }
                        }
                    }
                } },
            }
        }
    }
}

fn delete_form(detail: &Detail, csrf: &str) -> Element {
    let action = format!("{}/delete", detail.row.path());
    rsx! {
        section { class: "card danger-zone", aria_labelledby: "delete-title",
            h2 { id: "delete-title", "Delete this VM" }
            p { class: "muted", "Deletes the VirtualMachine; its Pod and ConfigMap go with it. The state PVC "
                code { "{detail.state_pvc}" } " is kept: it holds the VM's firmware variables and TPM identity. Ask an administrator to delete it deliberately." }
            form { class: "add-member", method: "post", action: "{action}",
                input { r#type: "hidden", name: "csrf", value: "{csrf}" }
                label { r#for: "confirm-name", "Type the VM name to confirm" }
                input { id: "confirm-name", name: "confirm_name", required: true, autocomplete: "off" }
                button { class: "danger", r#type: "submit", "Delete VM" }
            }
        }
    }
}

fn network_card(d: &Detail) -> Element {
    rsx! {
        section { class: if d.nics.is_empty() { "card" } else { "card wide" }, id: "vm-network", h2 { "Network" }
            if d.nics.is_empty() { p { "No guest NIC." } }
            else {
                div { class: "table-wrap", table {
                    thead { tr { th { scope: "col", "NIC" } th { scope: "col", "MAC" } th { scope: "col", "IP" } th { scope: "col", "Security groups" } th { scope: "col", "NIC pod" } th { scope: "col", "Ready" } } }
                    tbody { for nic in &d.nics { tr {
                        td { "data-label": "NIC", code { "{nic.name}" } }
                        td { "data-label": "MAC", code { {or_dash(&nic.mac)} } }
                        td { "data-label": "IP", code { {or_dash(&nic.ip)} } }
                        td { "data-label": "Security groups",
                            if nic.security_groups.is_empty() { div { span { class: "muted", "none" } } }
                            else { {nic.security_groups.join(", ")} }
                        }
                        td { "data-label": "NIC pod", code { {or_dash(&nic.pod_name)} } }
                        td { "data-label": "Ready", div { span { class: if nic.ready { "badge ok" } else { "badge" }, if nic.ready { "ready" } else { "not ready" } } } }
                    } } }
                } }
                p { class: "muted", "Guest traffic leaves from the NIC pod's IP; the VM pod (BMC) never carries it. A NIC that no security-group policy selects is default-allow. "
                    a { href: NETWORKING_DOCS, rel: "noreferrer", "Security groups" }
                }
            }
        }
    }
}

fn detail_page(page: &DetailPage, user: &UserView) -> Element {
    let d = &page.detail;
    let (cpu, mem) = model::usage_text(d.row.usage);
    let mut summary = vec![
        (
            "State",
            format!(
                "{} · {}",
                d.row.phase,
                if d.row.ready { "ready" } else { "not ready" }
            ),
        ),
        (
            "Desired power",
            if d.running {
                "running".into()
            } else {
                "off".into()
            },
        ),
        (
            "Node",
            if d.row.node.is_empty() {
                "—".into()
            } else {
                d.row.node.clone()
            },
        ),
        ("vCPUs", sizes(d.row.cpus, d.row.max_cpus, "")),
        (
            "Memory",
            sizes(d.row.memory_mib, d.row.max_memory_mib, " MiB"),
        ),
        ("Boots from", d.boots_from.clone()),
        ("Cloud-init", d.cloud_init.clone()),
    ];
    let metadata = vec![
        ("VM UID", d.uid.clone()),
        ("Accelerator", d.accelerator.to_string()),
        ("Boot", d.boot.clone()),
        (
            "Storage node",
            if d.storage_node.is_empty() {
                "—".into()
            } else {
                d.storage_node.clone()
            },
        ),
        ("State PVC", d.state_pvc.clone()),
        (
            "Components",
            format!(
                "BMC {} · Redfish {}",
                if d.bmc_enabled { "on" } else { "off" },
                if d.redfish_enabled { "on" } else { "off" }
            ),
        ),
    ];
    if !d.reason.is_empty() {
        summary.push(("Reason", d.reason.clone()));
    }
    if !d.message.is_empty() {
        summary.push(("Message", d.message.clone()));
    }
    let resize = d.resize.as_ref().map(|r| {
        vec![
            ("Phase", r.phase.clone()),
            (
                "Message",
                if r.message.is_empty() {
                    "—".into()
                } else {
                    r.message.clone()
                },
            ),
            (
                "Requested",
                format!("{} vCPUs · {} MiB", r.cpus, r.memory_mib),
            ),
            (
                "Applied",
                match (r.applied_cpus, r.applied_memory_mib) {
                    (Some(c), Some(m)) => format!("{c} vCPUs · {m} MiB"),
                    _ => "—".into(),
                },
            ),
            (
                "Booted with",
                format!("{} vCPUs · {} MiB", r.boot_cpus, r.boot_memory_mib),
            ),
            (
                "Ceilings",
                format!("{} vCPUs · {} MiB", r.max_cpus, r.max_memory_mib),
            ),
        ]
    });
    let admin = d.row.role == Role::Admin;
    let console = format!("{}/console", d.row.path());
    let refresh = format!("/auth/login?next={}", d.row.path());
    rsx! {
        a { class: "back", href: "/", "← All VMs" }
        div { class: "heading",
            div { p { class: "eyebrow", "VIRTUAL MACHINE · {d.row.namespace}" } h1 { "{d.row.name}" }
                div { class: "status-line", {state_badge(&d.row)}
                    span { class: if admin { "badge admin" } else { "badge" }, "{d.row.role.label()}" }
                }
            }
            div { class: "heading-actions",
                if d.bmc_enabled { a { class: "button", href: "{d.row.path()}/bmc", "Open BMC →" } }
                if admin && d.bmc_enabled { a { class: "button secondary", href: "{console}", "Open console →" } }
                if !admin && d.bmc_enabled {
                    div { class: "console-locked",
                        p { class: "muted",
                            "View-only access. Console and power controls require an admin. If your access changed, "
                            a { href: "{refresh}", "refresh your sign-in" }
                            "."
                        }
                    }
                }
            }
        }
        nav { class: "section-nav", aria_label: "On this VM page",
            a { href: "#vm-overview", "Overview" }
            a { href: "#vm-network", "Network" }
            a { href: "#vm-storage", "Storage" }
            a { href: "#vm-health", "Health & metrics" }
            if admin { a { href: "#power-title", "Power" } }
        }
        div { class: "detail-grid",
            section { class: "card", id: "vm-overview", h2 { "Overview" } {dl(summary)}
                details { class: "inline-disclosure",
                    summary { "Infrastructure details" }
                    {dl(metadata)}
                }
            }
            section { class: "card", h2 { "Live usage" }
                p { class: "muted", "From metrics.k8s.io for the whole Pod, including the BMC and Redfish sidecars." }
                {dl(vec![("CPU", cpu), ("Memory", mem)])}
            }
            {network_card(d)}
            section { class: if d.disks.is_empty() { "card" } else { "card wide" }, id: "vm-storage", h2 { "Disks" }
                if d.disks.is_empty() && d.iscsi_disks.is_empty() { p { class: "empty", "No disks. The VM boots the read-only demo ESP." } }
                if !d.disks.is_empty() {
                    div { class: "table-wrap", table {
                        thead { tr { th { scope: "col", "ID" } th { scope: "col", "Size" } th { scope: "col", "Capacity" } th { scope: "col", "Phase" } th { scope: "col", "Credential generation" } th { scope: "col", "Base image" } } }
                        tbody { for disk in &d.disks { tr {
                            td { "data-label": "ID", code { "{disk.id}" } }
                            td { "data-label": "Size", {model::gib(disk.size_bytes)} }
                            td { "data-label": "Capacity", {disk.capacity_bytes.map(model::gib).unwrap_or_else(|| "—".into())} }
                            td { "data-label": "Phase", "{disk.phase}" }
                            td { "data-label": "Credential generation", {disk.generation.map(|g| g.to_string()).unwrap_or_else(|| "—".into())} }
                            td { "data-label": "Base image", {disk.base_image.clone().unwrap_or_else(|| "blank".into())} }
                        } } }
                    } }
                }
                for id in &d.iscsi_disks { p { class: "muted", "Static iSCSI disk " code { "{id}" } } }
            }
            if let Some(rows) = resize { section { class: "card", h2 { "In-place resize" } {dl(rows)} } }
            details { class: "card wide disclosure", id: "pod-resources",
                summary { span { class: "disclosure-title", "Pod resources" }
                    span { class: "muted", "Container requests and limits" }
                }
                if let Some(pod) = &d.pod_name { p { class: "muted", "Pod " code { "{pod}" } if let Some(ip) = &d.pod_ip { " · " code { "{ip}" } } } }
                if d.containers.is_empty() { p { class: "empty", "No Pod is running." } }
                else { div { class: "table-wrap", table {
                    thead { tr { th { scope: "col", "Container" } th { scope: "col", "Kind" } th { scope: "col", "Requests" } th { scope: "col", "Limits" } } }
                    tbody { for c in &d.containers { tr {
                        td { "data-label": "Container", code { "{c.name}" } }
                        td { "data-label": "Kind", "{c.kind}" }
                        td { "data-label": "Requests", "{c.requests}" }
                        td { "data-label": "Limits", "{c.limits}" }
                    } } }
                } } }
            }
        }
        section { id: "vm-health", aria_labelledby: "bmc-title",
            h2 { id: "bmc-title", class: "subheading", "Health & metrics" }
            match &page.panel {
                PanelState::Disabled => rsx! { p { class: "muted", "The Redfish component is disabled for this VM, so its status and events are unavailable." } },
                PanelState::NoPod => rsx! { p { class: "muted", "Redfish is unavailable until the VM's Pod is running." } },
                PanelState::Loaded(panel) => bmc_panel(panel),
            }
        }
        if admin { {power_controls(d, &user.csrf, &page.panel)} }
        if page.deletable { {delete_form(d, &user.csrf)} }
    }
}

fn input_value(values: &[(String, String)], key: &str, default: &str) -> String {
    values
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| default.into())
}

fn launch_form(page: &LaunchPage, user: &UserView) -> Element {
    let v = |key: &str, default: &str| input_value(&page.values, key, default);
    let kvm = v("accelerator", "kvm") == "kvm";
    let node = page
        .storage_node
        .clone()
        .unwrap_or_else(|| "not configured".into());
    // Optional controls inside a closable disclosure use server validation:
    // native constraint validation cannot focus an invalid hidden control.
    // Keep the numeric keyboard, and reopen nonempty sections on rejection.
    let limits_open = page.values.iter().any(|(key, value)| {
        matches!(key.as_str(), "max_cpus" | "max_memory_mib") && !value.trim().is_empty()
    });
    let disks_open = page
        .values
        .iter()
        .any(|(key, value)| key.starts_with("disk") && !value.trim().is_empty());
    rsx! {
        a { class: "back", href: "/", "← All VMs" }
        div { class: "heading", div { p { class: "eyebrow", "LAUNCH" } h1 { "Launch a VM" }
            p { class: "muted", "Choose an operating system and resources. You'll be the VM's administrator. Created in namespace " code { "{page.namespace}" } "." } } }
        if !page.errors.is_empty() {
            section { class: "card", role: "alert", h2 { "Check the form" } ul { for e in &page.errors { li { "{e}" } } } }
        }
        form { class: "card launch", method: "post", action: "/launch",
            input { r#type: "hidden", name: "csrf", value: "{user.csrf}" }
            {os_fieldset(page)}
            fieldset { legend { "Machine" }
                label { r#for: "name", "Name" }
                input { id: "name", name: "name", required: true, maxlength: "50", pattern: r"[a-z0-9]([a-z0-9\-]*[a-z0-9])?", value: v("name", ""), aria_describedby: "name-help" }
                p { id: "name-help", class: "muted", "Lowercase DNS label. Group names qrab_vm_{page.namespace}_<name>_viewers must fit in 64 characters." }
                div { class: "pair",
                    div { label { r#for: "cpus", "vCPUs" } input { id: "cpus", name: "cpus", r#type: "number", min: "1", max: "256", required: true, value: v("cpus", "1") } }
                    div { label { r#for: "memory_mib", "Memory (MiB)" } input { id: "memory_mib", name: "memory_mib", r#type: "number", min: "256", max: "1048576", required: true, value: v("memory_mib", "512") } }
                }
                details { class: "inline-disclosure", id: "launch-limits", open: limits_open,
                    summary { "Hot-plug limits (optional)" }
                    p { class: "muted", "Reserve room to grow after boot. Leave blank to use the initial size as the maximum. A maximum must be at least the initial size. TCG guests cannot hot-add vCPUs." }
                    div { class: "pair",
                        div { label { r#for: "max_cpus", "Maximum vCPUs (optional)" } input { id: "max_cpus", name: "max_cpus", r#type: "text", inputmode: "numeric", value: v("max_cpus", "") } }
                        div { label { r#for: "max_memory_mib", "Maximum memory (MiB, optional)" } input { id: "max_memory_mib", name: "max_memory_mib", r#type: "text", inputmode: "numeric", value: v("max_memory_mib", "") } }
                    }
                }
                label { r#for: "accelerator", "Accelerator" }
                select { id: "accelerator", name: "accelerator",
                    option { value: "kvm", selected: kvm, "KVM (hardware)" }
                    option { value: "tcg", selected: !kvm, "TCG (emulation)" }
                }
            }
            {network_fieldset(page)}
            {cloud_init_fieldset(page)}
            details { class: "form-disclosure", id: "launch-disks", open: disks_open,
                summary { "Data disks (optional)" }
                p { class: "muted", "Provisioned by the storage service on node {node}. Use whole GiB from 1 to 1024. Leave the ID empty to skip a row." }
                for i in 0..launch::FORM_DISKS {
                    {
                        let id = format!("disk{i}_id");
                        let size = format!("disk{i}_size_gib");
                        let base = format!("disk{i}_base_image");
                        let generation = format!("disk{i}_base_generation");
                        let n = i + 1;
                        rsx! { div { class: "pair disk-row", role: "group", aria_label: "Disk {n}",
                            div { label { r#for: "{id}", "Disk {n} ID" } input { id: "{id}", name: "{id}", maxlength: "20", value: v(&id, "") } }
                            div { label { r#for: "{size}", "Size (GiB)" } input { id: "{size}", name: "{size}", r#type: "text", inputmode: "numeric", value: v(&size, "") } }
                            div { label { r#for: "{base}", "Base image (optional)" } input { id: "{base}", name: "{base}", maxlength: "32", value: v(&base, "") } }
                            div { label { r#for: "{generation}", "Generation" } input { id: "{generation}", name: "{generation}", r#type: "text", inputmode: "numeric", value: v(&generation, "") } }
                        } }
                    }
                }
            }
            div { class: "form-footer",
                button { r#type: "submit", "Launch VM" }
                a { class: "button secondary", href: "/", "Cancel" }
            }
            details { class: "inline-disclosure",
                summary { "What gets created?" }
                p { class: "muted", "The VM, its admin and viewer groups, and a 1 GiB volume for its firmware and TPM state. You join the admin group. An operating system gets its own copy-on-write disk; each network interface gets a NIC Pod whose IP is the guest's address." }
            }
        }
    }
}

/// One row per form NIC. A fresh form attaches `eth0`; a re-rendered one
/// keeps what was submitted.
fn network_fieldset(page: &LaunchPage) -> Element {
    let fresh = page.values.is_empty();
    rsx! {
        fieldset { legend { "Network" }
            for i in 0..launch::FORM_NICS {
                {
                    let nic = launch::nic_name(i);
                    let enabled = format!("nic{i}_enabled");
                    let groups = format!("nic{i}_security_groups");
                    let checked = if fresh { i == 0 } else { input_value(&page.values, &enabled, "") == "on" };
                    rsx! { div { class: "nic-row", role: "group", aria_label: "Network interface {nic}",
                        label { class: "confirm",
                            input { r#type: "checkbox", name: "{enabled}", value: "on", checked: checked }
                            "Attach a network interface ({nic})"
                        }
                        label { r#for: "{groups}", "Security groups" }
                        input { id: "{groups}", name: "{groups}", placeholder: "web, ssh", autocomplete: "off", spellcheck: "false", value: input_value(&page.values, &groups, ""), aria_describedby: "nic-help" }
                    } }
                }
            }
            p { id: "nic-help", class: "muted",
                "Optional, comma-separated names such as web, ssh. At most {launch::MAX_SECURITY_GROUPS}; use lowercase letters, digits and hyphens."
            }
            p { class: "muted",
                strong { "Without any matching policy the NIC is default-allow" }
                ": every forwarded guest port is reachable and the guest can reach anything."
            }
            details { class: "inline-disclosure",
                summary { "How network policies are applied" }
                p { class: "muted", "Each group becomes the label " code { "sg.qrab.yapplesauce.com/<group>" }
                    " on the NIC Pod. CiliumNetworkPolicies select those labels. The guest uses its NIC Pod's IP; cloud-init configures the address. See "
                    a { href: NETWORKING_DOCS, rel: "noreferrer", "docs/networking.md" } "."
                }
            }
        }
    }
}

fn cloud_init_fieldset(page: &LaunchPage) -> Element {
    rsx! {
        fieldset { legend { "Cloud-init" }
            p { class: "muted", "Set the guest's hostname or provide cloud-init configuration. Leave blank to use the defaults." }
            label { r#for: "hostname", "Hostname (optional)" }
            input { id: "hostname", name: "hostname", maxlength: "63", pattern: r"[a-z0-9]([a-z0-9\-]*[a-z0-9])?", autocomplete: "off", placeholder: "the VM name", value: input_value(&page.values, "hostname", ""), aria_describedby: "hostname-help" }
            p { id: "hostname-help", class: "muted", "DNS label. Defaults to the VM name." }
            label { r#for: "user_data", "User-data (optional)" }
            textarea { id: "user_data", name: "user_data", rows: "8", spellcheck: "false", autocomplete: "off", maxlength: "{launch::USER_DATA_LIMIT}", placeholder: "#cloud-config\nssh_authorized_keys: [ssh-ed25519 AAAA...]", aria_describedby: "user-data-help" }
            if page.user_data_cleared { p { class: "notice", role: "status", "Your user-data was not sent back with this form. Paste it again." } }
            p { id: "user-data-help", class: "muted",
                "Passed to the guest verbatim, at most 32 KiB. "
                strong { "It is stored in the VM's ConfigMap" }
                ", which anyone who can read ConfigMaps in namespace {page.namespace} can read. Use it for configuration and public keys; do not put passwords or tokens in it. The dashboard shows only its size."
            }
        }
    }
}

fn mib(bytes: u64) -> String {
    format!("{} MiB", bytes.div_ceil(1024 * 1024))
}

fn os_fieldset(page: &LaunchPage) -> Element {
    let chosen = input_value(&page.values, "os", "none");
    let root = input_value(&page.values, "root_size_gib", "1");
    let node = page.storage_node.clone();
    rsx! {
        fieldset { legend { "Operating system" }
            p { class: "muted",
                "Each VM gets its own writable copy of the selected image as its "
                code { "root" } " disk. Changes belong to this VM. "
                a { href: "/images", "About the images" }
            }
            div { class: "choices", role: "radiogroup", aria_label: "Operating system",
                label { class: "confirm",
                    input { r#type: "radio", name: "os", value: "none", checked: chosen == "none" || chosen.is_empty() }
                    "None: demo ESP only (no operating system)"
                }
                for image in &page.images {
                    {
                        let key = image.key();
                        let checked = chosen == key;
                        let text = format!("{} ({}) · {} · root disk at least {} GiB", image.title, image.label(), mib(image.size_bytes), image.min_root_gib());
                        rsx! { label { class: "confirm",
                            input { r#type: "radio", name: "os", value: "{key}", checked: checked, disabled: node.is_none() }
                            "{text}"
                        } }
                    }
                }
            }
            if page.images.is_empty() { p { class: "muted", "No base images are staged yet." } }
            if node.is_none() && !page.images.is_empty() { p { class: "muted", "Operating system images need a storage node; DASHBOARD_STORAGE_NODE is not configured." } }
            div { class: "pair",
                div {
                    label { r#for: "root_size_gib", "Root disk size (GiB)" }
                    input { id: "root_size_gib", name: "root_size_gib", r#type: "number", min: "1", max: "{launch::MAX_DISK_GIB}", value: "{root}", aria_describedby: "root-help" }
                }
            }
            p { id: "root-help", class: "muted", "Used only with an operating system. At least the image size rounded up to whole GiB; the clone can grow later but never shrink." }
        }
    }
}

fn images_page(images: &[BaseImage]) -> Element {
    rsx! {
        div { class: "heading", div { p { class: "eyebrow", "BASE IMAGES" } h1 { "Operating system images" }
            p { class: "muted", "Sealed images a new VM can boot from. Choose one on the launch form." } }
            span { class: "count", "{images.len()} staged" } }
        section { class: "card wide directory",
            if images.is_empty() { p { class: "empty", "No base images are staged yet." } }
            else { div { class: "table-wrap", table {
                caption { class: "visually-hidden", "Staged base images" }
                thead { tr {
                    th { scope: "col", "Image" } th { scope: "col", "Name" } th { scope: "col", "Size" }
                    th { scope: "col", "Raw SHA-256" } th { scope: "col", "Source" } th { scope: "col", "Boot" }
                } }
                tbody { for image in images { tr {
                    td { "data-label": "Image", div { strong { "{image.title}" } p { class: "muted", "{image.os} {image.version}" } } }
                    td { "data-label": "Name", code { {image.label()} } }
                    td { "data-label": "Size", {mib(image.size_bytes)} }
                    td { "data-label": "Raw SHA-256", code { "{image.sha256}" } }
                    td { "data-label": "Source", a { href: "{image.source_url}", rel: "noreferrer", "{image.source_url}" } }
                    td { "data-label": "Boot", {image.boot.to_uppercase()} }
                } } }
            } } }
        }
        section { class: "card wide", aria_labelledby: "how-title",
            h2 { id: "how-title", "How images get here" }
            ol {
                li { "An administrator stages an image with " code { "contrib/images/stage-base-image.sh" } ": it downloads a known release, checks the upstream SHA-256 and converts it to a raw disk." }
                li { "As root on the storage node, the import writes it to a ZFS volume and seals it as the read-only snapshot " code { "image-<name>-g<generation>@sealed" } ", checking the raw SHA-256. The script then records it here." }
                li { "Each VM launched with it gets its own copy-on-write clone of that snapshot as its " code { "root" } " disk, exported over iSCSI and booted by UEFI. The image itself never changes; a new release is a new generation." }
            }
        }
    }
}

fn launched(page: &Launched) -> Element {
    let r = &page.report;
    rsx! {
        a { class: "back", href: "/", "← All VMs" }
        section { class: "card", role: if r.failed.is_some() { "alert" } else { "status" },
            if let Some((step, error)) = &r.failed {
                h1 { "Launch failed" }
                p { "Step: " strong { "{step}" } }
                p { class: "muted", "{error}" }
            } else {
                h1 { "VM launched" }
            }
            h2 { class: "subheading", "Created" }
            if r.created.is_empty() { p { class: "empty", "Nothing was created." } }
            ul { for item in &r.created { li { "{item}" } } }
            if !r.rolled_back.is_empty() { h2 { class: "subheading", "Rolled back" } ul { for item in &r.rolled_back { li { "{item}" } } } }
            if !r.left.is_empty() { h2 { class: "subheading", "Left in place" } ul { for item in &r.left { li { "{item}" } } } }
            if let Some(path) = &page.vm_path { a { class: "button", href: "{path}", "Open the VM →" } }
        }
    }
}

fn console_page(page: &ConsolePage) -> Element {
    let back = format!("/vms/{}/{}", page.namespace, page.name);
    let socket = format!("{back}/console/ws");
    rsx! {
        a { class: "back", href: "{back}", "← Back to {page.name}" }
        div { class: "heading", div { p { class: "eyebrow", "CONSOLE · {page.namespace}" } h1 { "{page.name}" } } }
        div { class: "toolbar",
            p { id: "console-status", role: "status", "Console disconnected" }
            div { class: "actions",
                button { id: "connect", r#type: "button", "Connect console" }
                button { id: "cad", r#type: "button", class: "quiet", disabled: true, "Ctrl Alt Del" }
                button { id: "fullscreen", r#type: "button", class: "quiet", disabled: true, "Full screen" }
                if page.audio {
                    button { id: "audio", r#type: "button", class: "quiet", "aria-pressed": "false", title: "Guest audio playback, about 1.4 Mbit/s", "Audio off" }
                }
            }
        }
        div { class: "toolbar power-bar", id: "power", "data-endpoint": "{back}/power", "data-csrf": "{page.csrf}",
            p { class: "power-state", span { id: "power-chip", class: "chip", "Checking…" } span { id: "power-detail", class: "muted" } }
            div { class: "actions", role: "group", aria_label: "Power",
                for b in console_power() {
                    button { r#type: "button", class: if b.disruptive { "danger" } else { "quiet" }, "data-power": b.action, "data-label": b.label, "data-in-pod": if b.in_pod { "true" } else { "false" }, disabled: true, {b.label} }
                }
            }
        }
        div { id: "screen", "data-socket": "{socket}", p { "Open the console for video, keyboard and mouse access. It closes when your session ends." } }
        p { id: "console-error", role: "alert", hidden: true }
        script { r#type: "module", src: "/assets/console.js" }
    }
}

fn app() -> Element {
    let page = consume_context::<Page>();
    let on_images = matches!(&page.body, Body::Images(_));
    let on_vms = matches!(
        &page.body,
        Body::List(_) | Body::Detail(_) | Body::Console(_)
    );
    rsx! {
        body {
            a { class: "skip-link", href: "#main", "Skip to content" }
            header { div { class: "brand", a { href: "/", "qrab" } span { "Virtual machines" } }
                if let Some(user) = &page.user { div { class: "operator",
                    nav { class: "site-nav", aria_label: "Dashboard", a { href: "/", aria_current: on_vms.then_some("page"), "VMs" } a { href: "/images", aria_current: on_images.then_some("page"), "Images" } }
                    if user.launcher { a { class: "button", href: "/launch", "Launch VM" } }
                    span { "{user.name}" }
                    form { method: "post", action: "/auth/logout", input { r#type: "hidden", name: "csrf", value: "{user.csrf}" } button { class: "quiet", r#type: "submit", "Sign out" } }
                } }
            }
            main { id: "main", tabindex: "-1",
                if let Some(notice) = &page.notice { p { class: "notice", role: "status", "{notice}" } }
                match (&page.body, &page.user) {
                    (Body::Error(error), _) => rsx! { section { class: "card", role: "alert", h1 { "Request could not be completed" } p { "{error}" } a { href: "/", "Return to the dashboard" } } },
                    (Body::List(rows), Some(_user)) => rsx! {
                        div { class: "heading", div { p { class: "eyebrow", "VIRTUAL MACHINES" } h1 { "Your VMs" }
                            p { class: "muted", "Your shared and personal virtual machines. Open a VM to inspect it or use its BMC." } }
                            span { class: "count", "{rows.len()} shown" } }
                        if rows.is_empty() { p { class: "empty", "No VMs are shared with you yet." } } else { {vm_table(rows)} }
                    },
                    (Body::Detail(detail), Some(user)) => detail_page(detail, user),
                    (Body::Launch(launch), Some(user)) => launch_form(launch, user),
                    (Body::Launched(result), Some(_)) => launched(result),
                    (Body::Console(console), Some(_)) => console_page(console),
                    (Body::Images(images), Some(_)) => images_page(images),
                    _ => rsx! { section { class: "welcome card", p { class: "eyebrow", "QRAB" } h1 { "Your virtual machines, in one place." }
                        p { "Launch VMs, check their state and usage, and open their consoles." }
                        a { class: "button", href: "/auth/login", "Sign in with Kanidm →" }
                        p { class: "muted", "Access follows your Kanidm VM groups." } } },
                }
                footer { "qrab · Virtual machines" }
            }
        }
    }
}

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
    /// A member of the image-admin group: sees the stage and retire forms.
    pub image_admin: bool,
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
    pub errors: Vec<launch::FormError>,
    pub namespace: String,
    pub storage_node: Option<String>,
    /// The base-image catalog at render time.
    pub images: Vec<BaseImage>,
    /// VpnProfiles a NIC can use.
    pub vpn: Vec<launch::VpnChoice>,
    /// The rejected form had user-data, which is never sent back.
    pub user_data_cleared: bool,
}

impl LaunchPage {
    fn invalid(&self, field: &str) -> Option<&'static str> {
        self.errors
            .iter()
            .any(|error| error.field.as_deref() == Some(field))
            .then_some("true")
    }
    fn described_by(&self, field: &str, help: &str) -> Option<String> {
        let mut ids = Vec::new();
        if !help.is_empty() {
            ids.push(help.to_owned());
        }
        if self.invalid(field).is_some() {
            ids.push(format!("error-{field}"));
        }
        (!ids.is_empty()).then(|| ids.join(" "))
    }
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
    Error(ErrorPage),
    List(Vec<Row>),
    Detail(Box<DetailPage>),
    Launch(LaunchPage),
    Launched(Launched),
    Console(ConsolePage),
    Images(Box<crate::image_page::ImagesPage>),
    Vpn(Box<crate::vpn_page::VpnPage>),
}

#[derive(Clone)]
pub struct ErrorPage {
    pub status: u16,
    pub message: String,
    /// An allowlisted same-origin VM page, set by the response middleware.
    pub return_to: Option<String>,
}

impl ErrorPage {
    fn title(&self) -> &'static str {
        match self.status {
            400 | 413 | 422 => "Check your request",
            401 => "Sign in to continue",
            403 => "Access isn't available",
            404 if self.return_to.is_some() => "VM not found",
            404 => "Page not found",
            409 => "This action isn't available right now",
            429 => "Too many requests",
            502..=504 => "Service temporarily unavailable",
            _ => "Request could not be completed",
        }
    }
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
        Body::Launch(form) if !form.errors.is_empty() => "Check launch form · qrab VMs".into(),
        Body::Launch(_) => "Launch a VM · qrab VMs".into(),
        Body::Launched(result) if result.report.failed.is_some() => {
            "Launch failed · qrab VMs".into()
        }
        Body::Launched(_) => "VM launched · qrab VMs".into(),
        Body::Images(page) if !page.errors.is_empty() => "Check image form · qrab VMs".into(),
        Body::Images(_) => "Base images · qrab VMs".into(),
        Body::Vpn(_) => "VPN · qrab VMs".into(),
        Body::Error(error) => format!("{} · qrab VMs", error.title()),
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
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{escaped}</title><link rel=\"icon\" type=\"image/svg+xml\" href=\"/assets/favicon.svg\"><link rel=\"stylesheet\" href=\"/assets/style.css\"></head>{}</html>",
        dioxus_ssr::render(&dom)
    )
}

fn cpu_size(current: u32, max: Option<u32>) -> String {
    let unit = if current == 1 { "vCPU" } else { "vCPUs" };
    match max {
        Some(max) => format!("{current} {unit} (max {max})"),
        None => format!("{current} {unit}"),
    }
}

fn memory_unit(mib: u32) -> String {
    if mib > 0 && mib.is_multiple_of(1048576) {
        format!("{} TiB", mib / 1048576)
    } else if mib > 0 && mib.is_multiple_of(1024) {
        format!("{} GiB", mib / 1024)
    } else {
        format!("{mib} MiB")
    }
}

fn memory_size(current: u32, max: Option<u32>) -> String {
    let current = memory_unit(current);
    match max {
        Some(max) => format!("{current} (max {})", memory_unit(max)),
        None => current,
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
        section { class: "card directory vm-directory", div { class: "table-wrap", table {
            caption { class: "visually-hidden", "Virtual machines you can access" }
            thead { tr {
                th { scope: "col", "Virtual machine" } th { scope: "col", "State" }
                th { scope: "col", "Compute" } th { scope: "col", "Storage" }
                th { scope: "col", "Usage snapshot" } th { scope: "col", "Location" } th { scope: "col", "Access" }
            } }
            tbody { for row in rows {
                {
                    let (cpu, mem) = model::usage_text(row.usage);
                    let cpu_size = cpu_size(row.cpus, row.max_cpus);
                    let memory_size = memory_size(row.memory_mib, row.max_memory_mib);
                    let disks = match row.disks {
                        0 => "No disks".to_string(),
                        1 => "1 disk".to_string(),
                        n => format!("{n} disks"),
                    };
                    let capacity = match (row.disks, row.disk_bytes) {
                        (0, _) => None,
                        (_, 0) => Some("Capacity not reported".to_string()),
                        (_, bytes) => Some(format!("{} reported", model::gib(bytes))),
                    };
                    rsx! { tr {
                        td { "data-label": "Name", div { a { class: "identity-link", href: row.path(), "{row.name}" } p { class: "muted", "{row.namespace}" } } }
                        td { "data-label": "State", div { {state_badge(row)} } }
                        td { "data-label": "Compute", div {
                            span { "{cpu_size}" }
                            span { class: "cell-detail", "{memory_size} memory" }
                        } }
                        td { "data-label": "Storage", div {
                            span { "{disks}" }
                            if let Some(capacity) = capacity { span { class: "cell-detail", "{capacity}" } }
                        } }
                        td { "data-label": "Usage snapshot", div {
                            span { "CPU {cpu}" }
                            span { class: "cell-detail", "Memory {mem}" }
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
            if value.contains('\n') || key.starts_with("Update output") { dd { pre { class: "output", role: "region", aria_label: key, tabindex: "0", "{value}" } } }
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
        p { class: "muted", "Fetched when this page loads. Reload to update." }
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
        if value > 0.0 && value < 0.01 {
            return format!("<0.01 B{suffix}");
        }
        if value > 0.0 && value < 1.0 {
            let small = format!("{:.2}", (value * 100.0).round() / 100.0);
            return format!(
                "{} B{suffix}",
                small.trim_end_matches('0').trim_end_matches('.')
            );
        }
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
        "cores" if value > 0.0 && value < 0.00001 => "<0.01 mcores".into(),
        "cores" if value > 0.0 && value < 0.001 => {
            let milli = format!("{:.2}", (value * 100_000.0).round() / 100.0);
            format!(
                "{} mcores",
                milli.trim_end_matches('0').trim_end_matches('.')
            )
        }
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

/// Break at missing intervals; a lone sample gets a short visible mark.
pub fn spark_path(points: &[(f64, f64)], start: f64, end: f64, max: f64, step: u64) -> String {
    let geometry = spark_points(points, start, end, max);
    let mut path = String::new();
    let mut previous = None;
    let mut segment_len = 0;
    let mut mark = "h1";
    for ((time, _), position) in points.iter().zip(geometry.split_whitespace()) {
        let connected = previous
            .is_some_and(|last| *time >= last && (step == 0 || *time - last <= 2.0 * step as f64));
        if connected {
            path.push('L');
        } else {
            if segment_len == 1 {
                path.push_str(mark);
            }
            path.push('M');
            segment_len = 0;
        }
        path.push_str(position);
        segment_len += 1;
        previous = Some(*time);
        mark = if *time > (start + end) / 2.0 {
            "h-1"
        } else {
            "h1"
        };
    }
    if segment_len == 1 {
        path.push_str(mark);
    }
    path
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
                spark_path(
                    &series.points,
                    history.start as f64,
                    history.end as f64,
                    max,
                    history.step,
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
                for (class, _, path, _) in lines.iter() {
                    path { class: "spark-line {class}", d: "{path}" }
                }
            }
            ul { class: "spark-legend",
                for (class, label, _, latest) in lines.iter() {
                    li { svg { class: "swatch", view_box: "0 0 32 10", "aria-hidden": "true",
                        line { class: "{class}", x1: "1", y1: "5", x2: "31", y2: "5" }
                    } code { "{label}" } " {latest}" }
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
            p { class: "muted", "Gaps indicate missing samples; a short mark shows a lone sample." }
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
                let since = stamp(s.connected_since_unix_ms);
                if since == "—" {
                    "yes".into()
                } else {
                    format!("yes, since {since}")
                }
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

/// A snapshot hint only; the server still checks every submitted action.
fn power_state_reason(detail: &Detail, power: Power, state: Option<&str>) -> Option<&'static str> {
    let available = match power {
        Power::On => !detail.running || state == Some("Off"),
        Power::Off => detail.running || detail.pod_name.is_some(),
        Power::Reset(reset) => match (reset, state) {
            (_, None) => {
                return Some("Current guest power state is unavailable. Reload to update.");
            }
            (ResetType::Resume, Some(state)) => state == "Paused",
            (ResetType::Pause | ResetType::GracefulRestart | ResetType::Nmi, Some(state)) => {
                state == "On"
            }
            (ResetType::GracefulShutdown | ResetType::ForceRestart, Some(state)) => {
                matches!(state, "On" | "Paused")
            }
            (ResetType::PowerCycle, Some(state)) => matches!(state, "On" | "Paused" | "Off"),
            (ResetType::ForceOff, Some(state)) => state != "Off",
            (ResetType::On, Some(state)) => state == "Off",
        },
    };
    if available {
        return None;
    }
    Some(match power {
        Power::On => "Power on is already requested; use Resume for a paused guest.",
        Power::Off => "The VM is already stopped.",
        Power::Reset(ResetType::Resume) => "Resume needs a paused guest.",
        Power::Reset(ResetType::Pause) => "Pause needs a running guest.",
        Power::Reset(_) => "This action is unavailable in the current guest power state.",
    })
}

/// All native power forms share the same confirmation and CSRF fields.
fn power_form(
    detail: &Detail,
    csrf: &str,
    power: Power,
    restart_reason: Option<&str>,
    state: Option<&str>,
) -> Element {
    let action = format!("{}/action", detail.row.path());
    let state_reason = power_state_reason(detail, power, state);
    let reason = state_reason.or(if power == Power::Reset(ResetType::GracefulRestart) {
        restart_reason
    } else {
        None
    });
    let blocked = reason.is_some();
    let description = if state_reason.is_some() {
        Some("power-state-help")
    } else if blocked {
        Some("restart-requirement")
    } else {
        None
    };
    rsx! { form { method: "post", action: "{action}",
        input { r#type: "hidden", name: "csrf", value: "{csrf}" }
        input { r#type: "hidden", name: "action", value: power.key() }
        if power.needs_confirmation() {
            label { class: "confirm", input { r#type: "checkbox", name: "confirm", value: "yes", required: true, disabled: blocked, aria_label: "Confirm {power.label()} for {detail.row.name}" } "Confirm" }
        }
        button { class: if power.needs_confirmation() { "danger" } else { "quiet" }, r#type: "submit", disabled: blocked, title: reason, aria_describedby: description, {power.label()} }
    } }
}

/// Detail-page power controls, limited to actions currently offered by Redfish.
/// Power on/off also stay here when there is no BMC console page.
fn power_controls(detail: &Detail, csrf: &str, panel: &PanelState) -> Element {
    let state = match panel {
        PanelState::Loaded(panel) => panel
            .system
            .as_ref()
            .ok()
            .and_then(|system| system.power_state.as_deref()),
        _ => None,
    };
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
    let restart_reason = match panel {
        _ if state != Some("On")
            || !allowed
                .as_ref()
                .is_some_and(|actions| actions.contains(&ResetType::GracefulRestart)) =>
        {
            None
        }
        PanelState::Loaded(panel) => panel
            .system
            .as_ref()
            .ok()
            .and_then(|system| redfish::guest_restart_unavailable(system)),
        _ => None,
    };
    rsx! {
        section { class: "card", aria_labelledby: "power-title",
            h2 { id: "power-title", "Power" }
            p { class: "muted prose", id: "power-state-help", "Guest state: " strong { {state.unwrap_or("Unknown")} } ". Controls reflect this page's snapshot; reload after the state changes." }
            if !detail.redfish_enabled {
                p { class: "muted", role: "status", "In-pod actions (restart, shut down, pause) need the VM's Redfish service, and components.redfish is not enabled for this VM. Power on sets spec.running; Power off stops the VM and keeps it off." }
            } else if detail.bmc_enabled {
                p { class: "muted", "Sent to the VM's Redfish service with your sign-in. Shut down and Force off leave the VM powered off in its Pod; Power on (on the console page) starts it again in place. Power off on the console page stops the VM and keeps it off." }
            } else {
                p { class: "muted", "Sent to the VM's Redfish service with your sign-in. Shut down and Force off leave the VM powered off in its Pod; Power on starts it again in place. Power off stops the VM and keeps it off." }
            }
            if let Some(actions) = spec_power { div { class: "actions",
                for power in actions { {power_form(detail, csrf, power, restart_reason, state)} }
            } }
            match allowed {
                None if !detail.redfish_enabled => rsx! {},
                None => rsx! { p { class: "muted", role: "status", "In-pod power actions are unavailable while the VM's Redfish service cannot be read." } },
                Some(allowed) if allowed.is_empty() => rsx! { p { class: "muted", "No guest power actions are currently offered by this VM." } },
                Some(allowed) => {
                    let (common, more): (Vec<_>, Vec<_>) = allowed.into_iter().partition(|power|
                        matches!(power, ResetType::Resume | ResetType::Pause | ResetType::GracefulRestart | ResetType::GracefulShutdown));
                    rsx! {
                        div { class: "actions", for power in common { {power_form(detail, csrf, Power::Reset(power), restart_reason, state)} } }
                        if let Some(reason) = restart_reason { p { class: "muted prose", id: "restart-requirement", "{reason}" } }
                        if !more.is_empty() {
                            details { class: "inline-disclosure", id: "more-power-actions",
                                summary { "More power actions" }
                                p { class: "muted prose", "Reset restarts the guest immediately. Power cycle turns it off and on. NMI sends a diagnostic interrupt. Force off stops it immediately." }
                                div { class: "actions", for power in more { {power_form(detail, csrf, Power::Reset(power), restart_reason, state)} } }
                            }
                        }
                    }
                },
            }
        }
    }
}

fn delete_form(detail: &Detail, csrf: &str) -> Element {
    let action = format!("{}/delete", detail.row.path());
    rsx! {
        section { class: "card danger-zone", aria_labelledby: "delete-title",
            h2 { id: "delete-title", "Delete this VM" }
            p { class: "muted prose", "Removes this VM, its Pod and ConfigMap. The state PVC "
                code { "{detail.state_pvc}" } " is kept: it holds the VM's firmware variables and TPM identity." }
            p { class: "muted prose", "Its Kanidm access groups and its state PVC are deleted with it. Provisioned guest disks keep their data; their connections are closed as the VM is removed." }
            form { class: "delete-confirmation", method: "post", action: "{action}",
                input { r#type: "hidden", name: "csrf", value: "{csrf}" }
                label { r#for: "confirm-name", "Type " code { "{detail.row.name}" } " to confirm deletion" }
                input { id: "confirm-name", name: "confirm_name", required: true, autocomplete: "off", autocapitalize: "none", spellcheck: "false" }
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
                        td { "data-label": "NIC", div { code { "{nic.name}" }
                            if nic.tunnel_only { span { class: "cell-detail", "VPN tunnel only" } }
                            if let Some(vpn) = &nic.vpn {
                                span { class: "cell-detail", "Profile " a { href: "/vpn", "{vpn.profile}" }
                                    {match (&vpn.server, &vpn.location) {
                                        (Some(server), Some(location)) => format!(" · {server} · {location}"),
                                        (Some(server), None) => format!(" · {server}"),
                                        _ => " · no server yet".into(),
                                    }}
                                    if vpn.ready == Some(false) { " · not ready" }
                                }
                                span { class: "cell-detail",
                                    {match (vpn.forwarded_port, &vpn.port_message) {
                                        (Some(port), _) => format!("Forwarded port {port} (TCP and UDP, on the tunnel address)"),
                                        (None, Some(message)) => format!("No forwarded port: {message}"),
                                        (None, None) => "No forwarded port".into(),
                                    }}
                                }
                            }
                        } }
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
                if d.nics.iter().any(|nic| nic.tunnel_only) {
                    p { class: "muted", "On VPN interfaces, guest traffic stays inside the configured tunnel in its NIC Pod; direct guest traffic is blocked. The VM Pod never carries it." }
                }
                if d.nics.iter().any(|nic| !nic.tunnel_only) {
                    p { class: "muted", "Guest traffic leaves from the NIC pod's IP; the VM pod (BMC) never carries it. A direct interface with no matching security-group policy is default-allow." }
                }
                p { class: "muted",
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
        ("CPU allocation", cpu_size(d.row.cpus, d.row.max_cpus)),
        (
            "Memory",
            memory_size(d.row.memory_mib, d.row.max_memory_mib),
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
            section { class: "card", h2 { "Usage snapshot" }
                p { class: "muted", "Includes the VM and its control services. Reload this page to update." }
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

fn field_error(page: &LaunchPage, field: &str) -> Element {
    let messages: Vec<_> = page
        .errors
        .iter()
        .filter(|error| error.field.as_deref() == Some(field))
        .collect();
    if messages.is_empty() {
        return rsx! {};
    }
    rsx! { div { id: "error-{field}", class: "field-error",
        for error in messages { p { "{error.message}" } }
    } }
}

fn launch_form(page: &LaunchPage, user: &UserView) -> Element {
    let name_limit = launch::MAX_NAME;
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
        .any(|(key, value)| key.starts_with("disk") && !value.trim().is_empty())
        || page.errors.iter().any(|error| {
            error
                .field
                .as_deref()
                .is_some_and(|field| field.starts_with("disk"))
        });
    rsx! {
        a { class: "back", href: "/", "← All VMs" }
        div { class: "heading", div { p { class: "eyebrow", "LAUNCH" } h1 { "Launch a VM" }
            p { class: "muted", "Choose an operating system and resources. You'll be the VM's administrator. Created in namespace " code { "{page.namespace}" } "." } } }
        if !page.errors.is_empty() {
            section { class: "card form-errors", id: "form-errors", tabindex: "-1", role: "alert", aria_labelledby: "form-error-title", autofocus: !page.values.is_empty(),
                h2 { id: "form-error-title", "Check the form" }
                if page.errors.iter().any(|error| error.field.is_some()) {
                    p { class: "muted", "Choose an error to go to its field. Your settings are kept below." }
                }
                if page.user_data_cleared { p { class: "muted", "User-data is cleared for privacy; paste it again before submitting." } }
                ul { for error in &page.errors { li {
                    if let Some(field) = &error.field { a { href: "#{field}", "{error.message}" } }
                    else { "{error.message}" }
                } } }
            }
        }
        form { class: "card launch", method: "post", action: "/launch",
            input { r#type: "hidden", name: "csrf", value: "{user.csrf}" }
            {os_fieldset(page)}
            fieldset { legend { "Machine" }
                label { r#for: "name", "Name" }
                input { id: "name", name: "name", aria_invalid: page.invalid("name"), required: true, autocomplete: "off", autocapitalize: "none", spellcheck: "false", maxlength: "{name_limit}", pattern: r"[a-z0-9]([a-z0-9\-]*[a-z0-9])?", value: v("name", ""), aria_describedby: page.described_by("name", "name-help") }
                p { id: "name-help", class: "muted", "Up to {name_limit} characters. Use lowercase letters, digits and hyphens; start and end with a letter or digit." }
                {field_error(page, "name")}
                div { class: "pair",
                    div { label { r#for: "cpus", "vCPUs" } input { id: "cpus", name: "cpus", aria_invalid: page.invalid("cpus"), aria_describedby: page.described_by("cpus", ""), r#type: "number", min: "1", max: "256", required: true, value: v("cpus", "1") } {field_error(page, "cpus")} }
                    div { label { r#for: "memory_mib", "Memory (MiB)" } input { id: "memory_mib", name: "memory_mib", aria_invalid: page.invalid("memory_mib"), aria_describedby: page.described_by("memory_mib", ""), r#type: "number", min: "256", max: "1048576", required: true, value: v("memory_mib", "512") } {field_error(page, "memory_mib")} }
                }
                details { class: "inline-disclosure", id: "launch-limits", open: limits_open,
                    summary { "Hot-plug limits (optional)" }
                    p { class: "muted", "Reserve room to grow after boot. Leave blank to use the initial size as the maximum. A maximum must be at least the initial size. TCG guests cannot hot-add vCPUs." }
                    div { class: "pair",
                        div { label { r#for: "max_cpus", "Maximum vCPUs (optional)" } input { id: "max_cpus", name: "max_cpus", aria_invalid: page.invalid("max_cpus"), aria_describedby: page.described_by("max_cpus", ""), r#type: "text", inputmode: "numeric", value: v("max_cpus", "") } {field_error(page, "max_cpus")} }
                        div { label { r#for: "max_memory_mib", "Maximum memory (MiB, optional)" } input { id: "max_memory_mib", name: "max_memory_mib", aria_invalid: page.invalid("max_memory_mib"), aria_describedby: page.described_by("max_memory_mib", ""), r#type: "text", inputmode: "numeric", value: v("max_memory_mib", "") } {field_error(page, "max_memory_mib")} }
                    }
                }
                label { r#for: "accelerator", "Accelerator" }
                select { id: "accelerator", name: "accelerator", aria_invalid: page.invalid("accelerator"), aria_describedby: page.described_by("accelerator", ""),
                    option { value: "kvm", selected: kvm, "KVM (hardware)" }
                    option { value: "tcg", selected: !kvm, "TCG (emulation)" }
                }
                {field_error(page, "accelerator")}
            }
            {network_fieldset(page)}
            {cloud_init_fieldset(page)}
            details { class: "form-disclosure", id: "launch-disks", open: disks_open,
                summary { "Data disks (optional)" }
                p { id: "data-disk-help", class: "muted", "Provisioned by the storage service on node {node}. Use whole GiB from 1 to 1024. Leave every field in a row empty to skip it. IDs use lowercase letters, digits and hyphens." }
                p { id: "data-disk-image-help", class: "muted", "For a clone, enter the base image name and its generation separately. Leave both blank for an empty disk. "
                    a { href: "/images", target: "_blank", rel: "noopener noreferrer", "View staged images (opens in a new tab)" }
                }
                for i in 0..launch::FORM_DISKS {
                    {
                        let id = format!("disk{i}_id");
                        let size = format!("disk{i}_size_gib");
                        let base = format!("disk{i}_base_image");
                        let generation = format!("disk{i}_base_generation");
                        let n = i + 1;
                        let prefix = format!("disk{i}_");
                        let row_id = format!("launch-disk-{i}");
                        let row_open = i == 0
                            || page.values.iter().any(|(key, value)| key.starts_with(prefix.as_str()) && !value.trim().is_empty())
                            || page.errors.iter().any(|error| error.field.as_deref().is_some_and(|field| field.starts_with(prefix.as_str())));
                        rsx! { details { class: "disk-row", id: "{row_id}", open: row_open,
                            summary { id: "{row_id}-title", "Disk {n}" }
                            div { class: "pair", role: "group", aria_labelledby: "{row_id}-title",
                            div { label { r#for: "{id}", "Disk {n} ID" } input { id: "{id}", name: "{id}", aria_invalid: page.invalid(&id), aria_describedby: page.described_by(&id, "data-disk-help"), autocomplete: "off", autocapitalize: "none", spellcheck: "false", maxlength: "20", value: v(&id, "") } {field_error(page, &id)} }
                            div { label { r#for: "{size}", "Size (GiB)" } input { id: "{size}", name: "{size}", aria_invalid: page.invalid(&size), aria_describedby: page.described_by(&size, "data-disk-help"), r#type: "text", inputmode: "numeric", value: v(&size, "") } {field_error(page, &size)} }
                            div { label { r#for: "{base}", "Base image name (optional)" } input { id: "{base}", name: "{base}", aria_invalid: page.invalid(&base), aria_describedby: page.described_by(&base, "data-disk-image-help"), autocomplete: "off", autocapitalize: "none", spellcheck: "false", maxlength: "32", value: v(&base, "") } {field_error(page, &base)} }
                            div { label { r#for: "{generation}", "Base image generation" } input { id: "{generation}", name: "{generation}", aria_invalid: page.invalid(&generation), aria_describedby: page.described_by(&generation, "data-disk-image-help"), r#type: "text", inputmode: "numeric", value: v(&generation, "") } {field_error(page, &generation)} }
                            }
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
                    let network = format!("nic{i}_network");
                    let chosen = input_value(&page.values, &network, "direct");
                    let checked = if fresh { i == 0 } else { input_value(&page.values, &enabled, "") == "on" };
                    rsx! { div { class: "nic-row", role: "group", aria_label: "Network interface {nic}",
                        label { class: "confirm",
                            input { r#type: "checkbox", name: "{enabled}", value: "on", checked: checked }
                            "Attach a network interface ({nic})"
                        }
                        label { r#for: "{network}", "Network" }
                        select { id: "{network}", name: "{network}", aria_invalid: page.invalid(&network), aria_describedby: page.described_by(&network, "vpn-help"),
                            option { value: "direct", selected: chosen == "direct", "Direct (the NIC Pod's IP)" }
                            for choice in &page.vpn {
                                {
                                    let value = launch::vpn_value(&choice.name);
                                    let label = match &choice.used_by {
                                        Some(used) => format!("VPN {} — {} (in use by {used})", choice.name, choice.summary),
                                        None => format!("VPN {} — {}", choice.name, choice.summary),
                                    };
                                    rsx! { option { value: "{value}", selected: chosen == value, disabled: choice.used_by.is_some() && chosen != value, "{label}" } }
                                }
                            }
                        }
                        {field_error(page, &network)}
                        label { r#for: "{groups}", "Security groups" }
                        input { id: "{groups}", name: "{groups}", aria_invalid: page.invalid(&groups), placeholder: "web, ssh", autocomplete: "off", autocapitalize: "none", spellcheck: "false", value: input_value(&page.values, &groups, ""), aria_describedby: page.described_by(&groups, "nic-help") }
                        {field_error(page, &groups)}
                    } }
                }
            }
            p { id: "nic-help", class: "muted",
                "Optional, comma-separated names such as web, ssh. At most {launch::MAX_SECURITY_GROUPS}; use lowercase letters, digits and hyphens."
            }
            p { id: "vpn-help", class: "muted",
                "A VPN interface sends all of its guest traffic through that ProtonVPN profile's WireGuard tunnel, terminated in its NIC Pod (the key never reaches the VM), and joins security group "
                code { "{launch::VPN_EGRESS_GROUP}" } ". Each profile serves one interface. With a forwarded port, the guest receives it on its tunnel address. "
                if page.vpn.is_empty() { "No VPN profiles exist yet: " }
                a { href: "/vpn", "Manage VPN profiles" } "."
            }
            if launch::FORM_NICS > 1 {
                p { class: "muted", "With two interfaces the guest gets a default route on each; cloud images normally prefer the first, so put the VPN on eth0 for all traffic to use it." }
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
            input { id: "hostname", name: "hostname", aria_invalid: page.invalid("hostname"), maxlength: "63", pattern: r"[a-z0-9]([a-z0-9\-]*[a-z0-9])?", autocomplete: "off", autocapitalize: "none", spellcheck: "false", placeholder: "the VM name", value: input_value(&page.values, "hostname", ""), aria_describedby: page.described_by("hostname", "hostname-help") }
            p { id: "hostname-help", class: "muted", "DNS label. Defaults to the VM name." }
            {field_error(page, "hostname")}
            label { r#for: "user_data", "User-data (optional)" }
            textarea { id: "user_data", name: "user_data", aria_invalid: page.invalid("user_data"), rows: "8", autocapitalize: "none", spellcheck: "false", autocomplete: "off", maxlength: "{launch::USER_DATA_LIMIT}", placeholder: "#cloud-config\nssh_authorized_keys: [ssh-ed25519 AAAA...]", aria_describedby: page.described_by("user_data", "user-data-help") }
            {field_error(page, "user_data")}
            if page.user_data_cleared { p { class: "notice", role: "status", "Your user-data was not sent back with this form. Paste it again." } }
            p { id: "user-data-help", class: "muted",
                "Passed to the guest verbatim, at most 32 KiB. "
                strong { "It is stored in the VM's ConfigMap" }
                ", which anyone who can read ConfigMaps in namespace {page.namespace} can read. Use it for configuration and public keys; do not put passwords or tokens in it. The dashboard shows only its size."
            }
        }
    }
}

pub(crate) fn mib(bytes: u64) -> String {
    format!("{} MiB", bytes.div_ceil(1024 * 1024))
}

fn os_fieldset(page: &LaunchPage) -> Element {
    let chosen = input_value(&page.values, "os", "none");
    let root = input_value(&page.values, "root_size_gib", "1");
    let node = page.storage_node.clone();
    rsx! {
        fieldset { id: "operating-system", tabindex: "-1", aria_invalid: page.invalid("operating-system"), aria_describedby: page.described_by("operating-system", ""), legend { "Operating system" }
            {field_error(page, "operating-system")}
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
                    input { id: "root_size_gib", name: "root_size_gib", aria_invalid: page.invalid("root_size_gib"), r#type: "number", min: "1", max: "{launch::MAX_DISK_GIB}", value: "{root}", aria_describedby: page.described_by("root_size_gib", "root-help") }
                    {field_error(page, "root_size_gib")}
                }
            }
            p { id: "root-help", class: "muted", "Used only with an operating system. At least the image size rounded up to whole GiB; the clone can grow later but never shrink." }
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
            else { ul { for item in &r.created { li { "{item}" } } } }
            if !r.rolled_back.is_empty() { h2 { class: "subheading", "Rolled back" } ul { for item in &r.rolled_back { li { "{item}" } } } }
            if !r.left.is_empty() {
                h2 { class: "subheading", "Left in place" }
                p { class: "muted", "Read these notes before reusing the VM name." }
                ul { for item in &r.left { li { "{item}" } } }
            }
            div { class: "actions",
                if let Some(path) = &page.vm_path { a { class: "button", href: "{path}", "Open the VM →" } }
                if r.failed.is_some() { a { class: "button secondary", href: "/launch", "Back to launch form" } }
            }
        }
    }
}

fn console_power_button(button: PowerButton) -> Element {
    rsx! { button { r#type: "button", class: if button.disruptive { "danger" } else { "quiet" }, "data-power": button.action, "data-label": button.label, "data-in-pod": if button.in_pod { "true" } else { "false" }, disabled: true, {button.label} } }
}

fn console_page(page: &ConsolePage) -> Element {
    let back = format!("/vms/{}/{}", page.namespace, page.name);
    let socket = format!("{back}/console/ws");
    let login = sign_in_path(Some(&format!("{back}/console")));
    let (common, more): (Vec<_>, Vec<_>) = console_power().into_iter().partition(|button| {
        !matches!(
            button.action,
            "ForceRestart" | "ForceOff" | "PowerCycle" | "Nmi"
        )
    });
    rsx! {
        a { class: "back", href: "{back}", "← Back to {page.name}" }
        div { class: "heading", div { p { class: "eyebrow", "CONSOLE · {page.namespace}" } h1 { "{page.name}" } } }
        noscript { p { class: "notice warning", "JavaScript is required to connect to the console and use its controls. Use the Back link above to view VM details." } }
        p { id: "console-error", role: "alert", hidden: true }
        p { id: "console-recovery", hidden: true, a { class: "button", href: "{login}", "Sign in again" } }
        div { class: "toolbar",
            p { id: "console-status", role: "status", "Console disconnected" }
            div { class: "actions", id: "console-controls",
                button { id: "connect", r#type: "button", "Connect console" }
                button { id: "cad", r#type: "button", class: "quiet", disabled: true, "Ctrl Alt Del" }
                button { id: "fullscreen", r#type: "button", class: "quiet", disabled: true, "Full screen" }
                if page.audio {
                    button { id: "audio", r#type: "button", class: "quiet", "aria-pressed": "false", title: "Guest audio playback, about 1.4 Mbit/s", "Audio off" }
                }
            }
        }
        div { class: "toolbar power-bar", id: "power", "data-endpoint": "{back}/power", "data-csrf": "{page.csrf}",
            p { class: "power-state", span { id: "power-chip", class: "chip", tabindex: "-1", "Checking…" } span { id: "power-detail", class: "muted" } }
            p { id: "power-action-status", class: "power-progress", role: "status", hidden: true }
            p { id: "power-action-error", class: "power-progress power-error", role: "alert", hidden: true }
            p { id: "restart-requirement", class: "power-progress", hidden: true }
            div { class: "actions", role: "group", aria_label: "Power",
                for button in common { {console_power_button(button)} }
                details { class: "console-power-more", id: "console-power-more",
                    summary { "More power actions" }
                    p { class: "muted prose", "Reset restarts the guest immediately. Power cycle turns it off and on. NMI sends a diagnostic interrupt. Force off stops it immediately. Select an action twice within five seconds to confirm." }
                    div { class: "actions", for button in more { {console_power_button(button)} } }
                }
            }
        }
        div { id: "screen", "data-socket": "{socket}", p { "Open the console for video, keyboard and mouse access. It closes when your session ends." } }
        p { id: "console-input-help", class: "muted prose", "Tab or click the display for guest input. Ctrl+Alt+Shift (left Alt) returns to console controls; clicking outside also releases keys." }
        script { r#type: "module", src: "/assets/console.js" }
    }
}

fn sign_in_path(next: Option<&str>) -> String {
    next.map_or_else(
        || "/auth/login".to_owned(),
        |path| {
            let query = url::form_urlencoded::Serializer::new(String::new())
                .append_pair("next", path)
                .finish();
            format!("/auth/login?{query}")
        },
    )
}

fn error_page(error: &ErrorPage) -> Element {
    let login = sign_in_path(error.return_to.as_deref());
    let vm = error
        .return_to
        .as_ref()
        .map(|path| path.split('/').take(4).collect::<Vec<_>>().join("/"));
    rsx! {
        section { class: "card error-card", aria_labelledby: "error-title",
            p { class: "eyebrow", "VM DASHBOARD" }
            h1 { id: "error-title", "{error.title()}" }
            if error.status != 404 {
                if let Some(path) = &vm { p { class: "muted", "VM · " code { {path.trim_start_matches("/vms/")} } } }
            }
            p { class: "error-message", "{error.message}" }
            if error.status == 403 {
                p { class: "muted", "Ask a VM administrator to check your access. If your access recently changed, refresh your sign-in." }
            }
            div { class: "error-actions",
                if error.status == 401 {
                    a { class: "button", href: "{login}", "Sign in with Kanidm" }
                    a { class: "button secondary", href: "/", "All VMs" }
                } else {
                    if let Some(vm) = vm.filter(|_| error.status != 404) {
                        a { class: "button", href: "{vm}", "Back to VM" }
                        a { class: "button secondary", href: "/", "All VMs" }
                    } else { a { class: "button", href: "/", "All VMs" } }
                    if error.status == 403 { a { href: "{login}", "Refresh sign-in" } }
                }
            }
        }
    }
}

fn app() -> Element {
    let page = consume_context::<Page>();
    let on_images = matches!(&page.body, Body::Images(_));
    let on_vpn = matches!(&page.body, Body::Vpn(_));
    let on_vms = matches!(
        &page.body,
        Body::List(_) | Body::Detail(_) | Body::Console(_)
    );
    rsx! {
        body {
            a { class: "skip-link", href: "#main", "Skip to content" }
            header { div { class: "brand", a { href: "/", "qrab" } span { "Virtual machines" } }
                if let Some(user) = &page.user { div { class: "operator",
                    nav { class: "site-nav", aria_label: "Dashboard", a { href: "/", aria_current: on_vms.then_some("page"), "VMs" } a { href: "/images", aria_current: on_images.then_some("page"), "Images" } if user.launcher { a { href: "/vpn", aria_current: on_vpn.then_some("page"), "VPN" } } }
                    if user.launcher { a { class: "button", href: "/launch", "Launch VM" } }
                    span { "{user.name}" }
                    form { method: "post", action: "/auth/logout", input { r#type: "hidden", name: "csrf", value: "{user.csrf}" } button { class: "quiet", r#type: "submit", "Sign out" } }
                } }
            }
            main { id: "main", tabindex: "-1",
                if let Some(notice) = &page.notice { p { class: "notice", role: "status", "{notice}" } }
                match (&page.body, &page.user) {
                    (Body::Error(error), _) => error_page(error),
                    (Body::List(rows), Some(user)) => rsx! {
                        div { class: "heading", div { p { class: "eyebrow", "VIRTUAL MACHINES" } h1 { "Your VMs" }
                            p { class: "muted", "Your shared and personal virtual machines. Open a VM to inspect it or use its BMC." }
                            p { class: "muted", "Usage is a page-load snapshot for the VM and its control services. Reload to update." } }
                            span { class: "count", "{rows.len()} shown" } }
                        if rows.is_empty() { section { class: "card", aria_labelledby: "empty-vms-title",
                            h2 { id: "empty-vms-title", "No VMs are visible to your account" }
                            p { class: "muted prose", "Ask a VM administrator to check your admin or viewer access. If access was recently granted, refresh your sign-in." }
                            div { class: "actions",
                                if user.launcher { a { class: "button", href: "/launch", "Launch a VM" } }
                                a { class: "button secondary", href: "/auth/login", "Refresh sign-in" }
                            }
                        } } else { {vm_table(rows)} }
                    },
                    (Body::Detail(detail), Some(user)) => detail_page(detail, user),
                    (Body::Launch(launch), Some(user)) => launch_form(launch, user),
                    (Body::Launched(result), Some(_)) => launched(result),
                    (Body::Console(console), Some(_)) => console_page(console),
                    (Body::Images(images), Some(user)) => crate::image_page::view(images, user),
                    (Body::Vpn(vpn), Some(user)) => crate::vpn_page::view(vpn, user),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_snapshot_distinguishes_guest_state_from_persistent_power() {
        let vm = serde_json::from_value(serde_json::json!({
            "apiVersion": "qrab.yapplesauce.com/v1alpha1", "kind": "VirtualMachine",
            "metadata": {"name": "power-fixture", "namespace": "qrab"},
            "spec": {"stateVolumeClaim": "identity"}
        }))
        .unwrap();
        let mut detail = model::detail(&vm, Role::Admin, &Default::default(), None, None, &[]);
        for (state, action, enabled) in [
            (Some("On"), ResetType::Resume, false),
            (Some("On"), ResetType::Pause, true),
            (Some("Paused"), ResetType::Resume, true),
            (Some("Paused"), ResetType::Pause, false),
            (Some("Paused"), ResetType::ForceRestart, true),
            (Some("Off"), ResetType::Pause, false),
            (Some("Off"), ResetType::PowerCycle, true),
            (Some("PoweringOn"), ResetType::ForceOff, true),
            (None, ResetType::Pause, false),
        ] {
            assert_eq!(
                power_state_reason(&detail, Power::Reset(action), state).is_none(),
                enabled,
                "{state:?} {action:?}"
            );
        }
        detail.running = true;
        assert!(power_state_reason(&detail, Power::On, Some("On")).is_some());
        assert!(power_state_reason(&detail, Power::On, Some("Off")).is_none());
        detail.running = false;
        assert!(power_state_reason(&detail, Power::On, None).is_none());
        assert!(power_state_reason(&detail, Power::Off, None).is_some());
        detail.pod_name = Some("draining-pod".into());
        assert!(power_state_reason(&detail, Power::Off, None).is_none());
    }
}

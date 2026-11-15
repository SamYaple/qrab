//! The VPN page (docs/dashboard.md, "VPN"): ProtonVPN profiles, the server
//! browser and auto/manual server choice. Profiles are cluster-wide, so the
//! page is for VM launchers only; everyone else gets a 404. The page edits
//! `VpnProfile` specs only: the Proton session, keys and configs stay with
//! the broker and never reach the dashboard.
use crate::{AppState, Failure, access, auth, launch, ui};
use axum::{
    extract::{Form, Path, Query, State},
    http::HeaderMap,
    response::{Html, Redirect},
};
use dioxus::prelude::*;
use qrab_proton::{
    catalog::{CompactCatalog, CompactServer, Feature},
    crd::{ProtonSpec, VpnProfile, VpnProfileSpec},
};
use tower_sessions::Session;

/// Servers shown at once; the filter narrows the rest.
pub const SHOWN: usize = 100;

/// The server browser's filter, from the query string.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filter {
    pub country: Option<String>,
    pub feature: Option<Feature>,
    pub max_load: Option<u8>,
    pub name: Option<String>,
}

impl Filter {
    pub fn parse(query: &[(String, String)]) -> Self {
        let get = |k: &str| {
            query
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.trim())
                .filter(|v| !v.is_empty())
        };
        Self {
            country: get("country")
                .filter(|c| c.len() == 2 && c.bytes().all(|b| b.is_ascii_alphabetic()))
                .map(str::to_ascii_uppercase),
            feature: get("feature").and_then(feature),
            max_load: get("max_load")
                .and_then(|l| l.parse().ok())
                .filter(|l| *l <= 100),
            name: get("name").map(|n| n.chars().take(32).collect()),
        }
    }

    /// Whether `server` passes, by the selection's rules: up, within the
    /// plan's tier, and Secure Core or Tor only when asked for.
    pub fn admits(&self, server: &CompactServer, max_tier: u8) -> bool {
        let has = |f: Feature| server.features & f.bit() != 0;
        server.status == 1
            && server.tier <= max_tier
            && self
                .country
                .as_deref()
                .is_none_or(|c| server.exit_country.eq_ignore_ascii_case(c))
            && self.feature.is_none_or(has)
            && self.max_load.is_none_or(|l| server.load <= l)
            && self.name.as_deref().is_none_or(|n| {
                server
                    .name
                    .to_ascii_lowercase()
                    .contains(&n.to_ascii_lowercase())
            })
            && [Feature::SecureCore, Feature::Tor]
                .into_iter()
                .all(|f| !has(f) || self.feature == Some(f))
    }
}

fn feature(value: &str) -> Option<Feature> {
    Feature::ALL.into_iter().find(|f| feature_key(*f) == value)
}

pub fn feature_key(f: Feature) -> &'static str {
    match f {
        Feature::SecureCore => "secureCore",
        Feature::Tor => "tor",
        Feature::P2p => "p2p",
        Feature::Streaming => "streaming",
        Feature::Ipv6 => "ipv6",
    }
}

pub fn feature_label(f: Feature) -> &'static str {
    match f {
        Feature::SecureCore => "Secure Core",
        Feature::Tor => "Tor",
        Feature::P2p => "P2P",
        Feature::Streaming => "Streaming",
        Feature::Ipv6 => "IPv6",
    }
}

/// One profile as the page shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileRow {
    pub name: String,
    pub spec: ProtonSpec,
    pub server: Option<String>,
    pub location: Option<String>,
    pub load: Option<u8>,
    pub entry_ip: Option<String>,
    pub forwarded_port: Option<u16>,
    pub port_message: Option<String>,
    pub attached: Vec<String>,
    pub ready: Option<(bool, String, String)>,
    pub certificate_expires: Option<i64>,
}

impl ProfileRow {
    pub fn from(profile: &VpnProfile) -> Self {
        let status = profile.status.clone().unwrap_or_default();
        Self {
            name: profile.metadata.name.clone().unwrap_or_default(),
            spec: profile.spec.proton.clone(),
            server: status.server.clone(),
            location: status.exit_country.clone().map(|c| match &status.city {
                Some(city) => format!("{c} · {city}"),
                None => c,
            }),
            load: status.load,
            entry_ip: status.entry_ip.clone(),
            forwarded_port: status.forwarded_port,
            port_message: status.port_forward_message.clone(),
            attached: status
                .attached
                .iter()
                .map(|a| format!("{}/{}", a.vm, a.nic))
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect(),
            ready: status
                .ready()
                .map(|c| (c.status == "True", c.reason.clone(), c.message.clone())),
            certificate_expires: status.certificate_expires,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VpnPage {
    pub profiles: Vec<ProfileRow>,
    /// None: the broker has not published a catalog yet.
    pub catalog: Option<CatalogView>,
    pub filter: Filter,
    pub namespace: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CatalogView {
    pub updated: i64,
    pub max_tier: u8,
    pub total: usize,
    pub matching: usize,
    pub servers: Vec<CompactServer>,
    /// Exit countries with how many usable servers each has.
    pub countries: Vec<(String, usize)>,
}

impl CatalogView {
    pub fn new(catalog: &CompactCatalog, filter: &Filter) -> Self {
        let usable = Filter::default();
        let mut countries = std::collections::BTreeMap::<String, usize>::new();
        for s in &catalog.servers {
            if usable.admits(s, catalog.max_tier) {
                *countries.entry(s.exit_country.clone()).or_default() += 1;
            }
        }
        let mut servers: Vec<CompactServer> = catalog
            .servers
            .iter()
            .filter(|s| filter.admits(s, catalog.max_tier))
            .cloned()
            .collect();
        servers.sort_by(|a, b| {
            a.score
                .total_cmp(&b.score)
                .then_with(|| a.name.cmp(&b.name))
        });
        let matching = servers.len();
        servers.truncate(SHOWN);
        Self {
            updated: catalog.updated,
            max_tier: catalog.max_tier,
            total: catalog.servers.len(),
            matching,
            servers,
            countries: countries.into_iter().collect(),
        }
    }

    pub fn knows(&self, name: &str) -> bool {
        self.servers
            .iter()
            .any(|s| s.name.eq_ignore_ascii_case(name))
    }
}

/// A DNS label short enough for `vpn-<name>` and the anchor's names.
pub fn valid_name(name: &str) -> bool {
    (1..=40).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !name.starts_with('-')
        && !name.ends_with('-')
}

/// A Proton server name as the CRD admits it (`CH#12`, `US-GA#445`, `CH#18-TOR`).
pub fn valid_server(name: &str) -> bool {
    let Some((left, right)) = name.split_once('#') else {
        return false;
    };
    let (number, suffix) = match right.split_once('-') {
        Some((n, s)) => (n, Some(s)),
        None => (right, None),
    };
    (2..=24).contains(&left.len())
        && left.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && (1..=6).contains(&number.len())
        && number.bytes().all(|b| b.is_ascii_digit())
        && suffix.is_none_or(|s| {
            (1..=8).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphabetic())
        })
}

/// The settings fields of the create and edit forms.
pub fn settings(fields: &[(String, String)], mut spec: ProtonSpec) -> Result<ProtonSpec, String> {
    let field = |k: &str| launch::field(fields, k);
    let country = field("country");
    spec.country = match country {
        "" => None,
        c if c.len() == 2 && c.bytes().all(|b| b.is_ascii_alphabetic()) => {
            Some(c.to_ascii_uppercase())
        }
        _ => return Err("A country is two letters, such as CH.".into()),
    };
    let city = field("city");
    spec.city = match city.chars().count() {
        0 => None,
        1..=64 if !city.chars().any(char::is_control) => Some(city.to_owned()),
        _ => return Err("A city is at most 64 characters.".into()),
    };
    let mut features = Vec::new();
    for (key, value) in fields {
        if key == "feature" {
            let f = feature(value.trim()).ok_or("Unknown server feature.")?;
            if !features.contains(&f) {
                features.push(f);
            }
        }
    }
    spec.features = features;
    spec.net_shield = match field("net_shield") {
        "0" => 0,
        "1" | "" => 1,
        "2" => 2,
        _ => return Err("NetShield is 0, 1 or 2.".into()),
    };
    let on = |k: &str| field(k) == "on";
    spec.port_forwarding = on("port_forwarding");
    spec.moderate_nat = on("moderate_nat");
    spec.vpn_accelerator = on("vpn_accelerator");
    Ok(spec)
}

fn allowed(app: &AppState, user: &auth::User) -> Result<(), Failure> {
    if access::can_launch(&user.groups, &app.launcher_group) {
        Ok(())
    } else {
        Err(Failure::not_found())
    }
}

pub async fn page(
    State(app): State<AppState>,
    session: Session,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Html<String>, Failure> {
    let user = auth::user(&session).await?;
    allowed(&app, &user)?;
    let filter = Filter::parse(&query);
    let mut profiles: Vec<ProfileRow> = app
        .cluster
        .vpn_profiles()
        .await
        .map_err(|e| Failure::upstream(&e))?
        .iter()
        .map(ProfileRow::from)
        .collect();
    profiles.sort_by(|a, b| a.name.cmp(&b.name));
    let catalog = match app.cluster.vpn_catalog().await {
        Ok(Some(bytes)) => match qrab_proton::catalog::decode(&bytes) {
            Ok(catalog) => Some(CatalogView::new(&catalog, &filter)),
            Err(error) => {
                tracing::warn!(%error, "the ProtonVPN catalog does not decode");
                None
            }
        },
        Ok(None) => None,
        Err(e) => return Err(Failure::upstream(&e)),
    };
    let body = ui::Body::Vpn(Box::new(VpnPage {
        profiles,
        catalog,
        filter,
        namespace: app.namespace.clone(),
    }));
    crate::page(&app, &session, &user, body).await
}

async fn mutation(
    app: &AppState,
    session: &Session,
    headers: &HeaderMap,
    fields: &[(String, String)],
) -> Result<auth::User, Failure> {
    let user = crate::mutation(app, session, headers, fields).await?;
    allowed(app, &user)?;
    Ok(user)
}

async fn profile(app: &AppState, name: &str) -> Result<VpnProfile, Failure> {
    app.cluster
        .vpn_profiles()
        .await
        .map_err(|e| Failure::upstream(&e))?
        .into_iter()
        .find(|p| p.metadata.name.as_deref() == Some(name))
        .ok_or_else(|| Failure::invalid("No such VPN profile."))
}

async fn done(session: &Session, message: String) -> Result<Redirect, Failure> {
    crate::flash(session, message).await?;
    Ok(Redirect::to("/vpn"))
}

pub async fn create(
    State(app): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Redirect, Failure> {
    let user = mutation(&app, &session, &headers, &fields).await?;
    let name = launch::field(&fields, "name");
    if !valid_name(name) {
        return Err(Failure::invalid(
            "A profile name is 1-40 lowercase letters, digits and dashes.",
        ));
    }
    let spec = settings(&fields, ProtonSpec::default()).map_err(|m| Failure::invalid(&m))?;
    let mut profile = VpnProfile::new(name, VpnProfileSpec { proton: spec });
    profile.metadata.namespace = Some(app.namespace.clone());
    let result = app.cluster.create_vpn_profile(&profile).await;
    tracing::info!(subject=%user.subject, profile=%name, success=result.is_ok(), "VPN profile created");
    result.map_err(|e| Failure::upstream(&e))?;
    done(
        &session,
        format!("Created VPN profile {name}. Its NIC uses Secret vpn-{name}."),
    )
    .await
}

/// Change a profile's server choice (`server`: a name, or `auto`) or its
/// settings (`action=settings`).
pub async fn update(
    State(app): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(name): Path<String>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Redirect, Failure> {
    let user = mutation(&app, &session, &headers, &fields).await?;
    let current = profile(&app, &name).await?;
    let uid = current.metadata.uid.clone().unwrap_or_default();
    let mut spec = current.spec.proton.clone();
    let message = match launch::field(&fields, "action") {
        "settings" => {
            spec = settings(&fields, spec).map_err(|m| Failure::invalid(&m))?;
            format!("Saved the settings of {name}.")
        }
        "server" => match launch::field(&fields, "server") {
            "" | "auto" => {
                spec.server = None;
                format!("{name} now picks the best server automatically.")
            }
            server if valid_server(server) => {
                spec.server = Some(server.to_owned());
                format!("{name} now uses {server}. The broker switches the tunnel within a minute.")
            }
            _ => return Err(Failure::invalid("That is not a Proton server name.")),
        },
        _ => return Err(Failure::invalid("Unknown VPN profile change.")),
    };
    let result = app.cluster.patch_vpn_profile(&name, &uid, &spec).await;
    tracing::info!(subject=%user.subject, profile=%name, success=result.is_ok(), "VPN profile changed");
    result.map_err(|e| Failure::upstream(&e))?;
    done(&session, message).await
}

/// Use a server from the browser for a profile (`profile`, `server`).
pub async fn use_server(
    state: State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(mut fields): Form<Vec<(String, String)>>,
) -> Result<Redirect, Failure> {
    let name = launch::field(&fields, "profile").to_owned();
    if !valid_name(&name) {
        return Err(Failure::invalid("Choose a VPN profile."));
    }
    fields.push(("action".into(), "server".into()));
    update(state, session, headers, Path(name), Form(fields)).await
}

pub async fn delete(
    State(app): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(name): Path<String>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Redirect, Failure> {
    let user = mutation(&app, &session, &headers, &fields).await?;
    if launch::field(&fields, "confirm_name") != name {
        return Err(Failure::invalid(
            "Type the profile name to confirm deletion.",
        ));
    }
    let current = profile(&app, &name).await?;
    let row = ProfileRow::from(&current);
    if !row.attached.is_empty() {
        return Err(Failure::invalid(&format!(
            "{} still uses this profile; remove the VPN NIC first.",
            row.attached.join(", ")
        )));
    }
    let uid = current.metadata.uid.clone().unwrap_or_default();
    let result = app.cluster.delete_vpn_profile(&name, &uid).await;
    tracing::info!(subject=%user.subject, profile=%name, success=result.is_ok(), "VPN profile deleted");
    result.map_err(|e| Failure::upstream(&e))?;
    done(&session, format!("Deleted VPN profile {name}.")).await
}

/// Unix seconds as `2026-11-15 18:50 UTC`.
///
/// ```
/// assert_eq!(qrab_dashboard::vpn_page::when(1794849231), "2026-11-16 17:13 UTC");
/// ```
pub fn when(unix: i64) -> String {
    match time::OffsetDateTime::from_unix_timestamp(unix) {
        Ok(t) => format!(
            "{}-{:02}-{:02} {:02}:{:02} UTC",
            t.year(),
            t.month() as u8,
            t.day(),
            t.hour(),
            t.minute()
        ),
        Err(_) => unix.to_string(),
    }
}

fn features_text(bits: u32) -> String {
    let names: Vec<&str> = Feature::of(bits).into_iter().map(feature_label).collect();
    if names.is_empty() {
        "—".into()
    } else {
        names.join(", ")
    }
}

fn settings_fields(spec: &ProtonSpec, countries: &[(String, usize)]) -> Element {
    let country = spec.country.clone().unwrap_or_default();
    let city = spec.city.clone().unwrap_or_default();
    rsx! {
        div { class: "field",
            label { "Country (for Auto)" }
            select { name: "country",
                option { value: "", selected: country.is_empty(), "Any" }
                for (code, count) in countries {
                    option { value: "{code}", selected: *code == country, "{code} ({count})" }
                }
                if !country.is_empty() && !countries.iter().any(|(c, _)| *c == country) {
                    option { value: "{country}", selected: true, "{country}" }
                }
            }
        }
        div { class: "field", label { "City (for Auto)" } input { name: "city", value: "{city}", maxlength: "64" } }
        fieldset { class: "field", legend { "Server features" }
            for f in Feature::ALL {
                label { class: "check",
                    input { r#type: "checkbox", name: "feature", value: feature_key(f), checked: spec.features.contains(&f) }
                    " {feature_label(f)}"
                }
            }
        }
        div { class: "field", label { "NetShield" }
            select { name: "net_shield",
                option { value: "0", selected: spec.net_shield == 0, "Off" }
                option { value: "1", selected: spec.net_shield == 1, "Block malware" }
                option { value: "2", selected: spec.net_shield == 2, "Block malware, ads and trackers" }
            }
        }
        label { class: "check", input { r#type: "checkbox", name: "port_forwarding", checked: spec.port_forwarding } " Port forwarding (P2P servers only)" }
        label { class: "check", input { r#type: "checkbox", name: "moderate_nat", checked: spec.moderate_nat } " Moderate NAT" }
        label { class: "check", input { r#type: "checkbox", name: "vpn_accelerator", checked: spec.vpn_accelerator } " VPN Accelerator" }
    }
}

pub fn view(page: &VpnPage, user: &ui::UserView) -> Element {
    let countries = page
        .catalog
        .as_ref()
        .map(|c| c.countries.clone())
        .unwrap_or_default();
    let filter = &page.filter;
    let filter_country = filter.country.clone().unwrap_or_default();
    let filter_feature = filter.feature.map(feature_key).unwrap_or_default();
    let filter_load = filter.max_load.map(|l| l.to_string()).unwrap_or_default();
    let filter_name = filter.name.clone().unwrap_or_default();
    rsx! {
        div { class: "heading", div { p { class: "eyebrow", "PROTONVPN" } h1 { "VPN" }
            p { class: "muted", "One profile is one Proton connection for one VM NIC. Give the NIC "
                code { "vpn: {{secretName: vpn-<profile>}}" } "." } }
            span { class: "count", "{page.profiles.len()} profiles" } }
        section { class: "card wide directory", aria_labelledby: "profiles-title",
            h2 { id: "profiles-title", "Profiles" }
            if page.profiles.is_empty() { p { class: "empty", "No VPN profiles yet. Create one below." } }
            else { div { class: "table-wrap", table {
                caption { class: "visually-hidden", "VPN profiles" }
                thead { tr {
                    th { scope: "col", "Profile" } th { scope: "col", "Server" } th { scope: "col", "Location" }
                    th { scope: "col", "Load" } th { scope: "col", "Forwarded port" } th { scope: "col", "Used by" } th { scope: "col", "Status" }
                } }
                tbody { for row in &page.profiles { tr {
                    td { "data-label": "Profile", div { strong { "{row.name}" }
                        p { class: "muted", if row.spec.server.is_some() { "Fixed server" } else { "Auto" } } } }
                    td { "data-label": "Server", match &row.server { Some(s) => rsx! { code { "{s}" } if let Some(ip) = &row.entry_ip { p { class: "muted", "{ip}" } } }, None => rsx! { "—" } } }
                    td { "data-label": "Location", {row.location.clone().unwrap_or_else(|| "—".into())} }
                    td { "data-label": "Load", {row.load.map(|l| format!("{l}%")).unwrap_or_else(|| "—".into())} }
                    td { "data-label": "Forwarded port",
                        if !row.spec.port_forwarding { span { class: "muted", "Off" } }
                        else if let Some(port) = row.forwarded_port { strong { "{port}" } }
                        else { span { class: "muted", {row.port_message.clone().unwrap_or_else(|| "Pending".into())} } } }
                    td { "data-label": "Used by", if row.attached.is_empty() { span { class: "muted", "No NIC" } } else { {row.attached.join(", ")} } }
                    td { "data-label": "Status",
                        match &row.ready {
                            Some((true, _, message)) => rsx! { span { class: "badge ok", "Ready" } p { class: "muted", "{message}" } },
                            Some((false, reason, message)) => rsx! { span { class: "badge warn", "{reason}" } p { class: "muted", "{message}" } },
                            None => rsx! { span { class: "muted", "Waiting for the broker" } },
                        }
                        if let Some(at) = row.certificate_expires { p { class: "muted", "Certificate until {when(at)}" } }
                    }
                } } }
            } } }
        }
        for row in &page.profiles {
            section { class: "card wide", aria_labelledby: "edit-{row.name}",
                details { summary { id: "edit-{row.name}", "Change {row.name}" }
                    form { method: "post", action: "/vpn/{row.name}",
                        input { r#type: "hidden", name: "csrf", value: "{user.csrf}" }
                        input { r#type: "hidden", name: "action", value: "server" }
                        div { class: "field", label { "Server" }
                            input { name: "server", value: row.spec.server.clone().unwrap_or_else(|| "auto".into()), placeholder: "auto or CH#12", maxlength: "36" } }
                        button { r#type: "submit", "Use this server" }
                    }
                    form { method: "post", action: "/vpn/{row.name}",
                        input { r#type: "hidden", name: "csrf", value: "{user.csrf}" }
                        input { r#type: "hidden", name: "action", value: "settings" }
                        {settings_fields(&row.spec, &countries)}
                        button { r#type: "submit", "Save settings" }
                    }
                    form { method: "post", action: "/vpn/{row.name}/delete",
                        input { r#type: "hidden", name: "csrf", value: "{user.csrf}" }
                        div { class: "field", label { "Type {row.name} to delete it" } input { name: "confirm_name", autocomplete: "off" } }
                        button { class: "danger", r#type: "submit", "Delete profile" }
                    }
                }
            }
        }
        section { class: "card wide", aria_labelledby: "servers-title",
            h2 { id: "servers-title", "Servers" }
            match &page.catalog {
                None => rsx! { p { class: "empty", "The ProtonVPN broker has not published the server list yet. It needs a Proton session: run "
                    code { "qrab-proton login" } "." } },
                Some(catalog) => rsx! {
                    p { class: "muted", "{catalog.matching} of {catalog.total} servers match, best score first; showing up to {SHOWN}. Updated {when(catalog.updated)}." }
                    form { method: "get", action: "/vpn", class: "filters",
                        div { class: "field", label { "Country" }
                            select { name: "country",
                                option { value: "", selected: filter_country.is_empty(), "Any" }
                                for (code, count) in &catalog.countries { option { value: "{code}", selected: *code == filter_country, "{code} ({count})" } }
                            } }
                        div { class: "field", label { "Feature" }
                            select { name: "feature",
                                option { value: "", selected: filter_feature.is_empty(), "Any" }
                                for f in Feature::ALL { option { value: feature_key(f), selected: feature_key(f) == filter_feature, {feature_label(f)} } }
                            } }
                        div { class: "field", label { "Max load %" } input { name: "max_load", r#type: "number", min: "0", max: "100", value: "{filter_load}" } }
                        div { class: "field", label { "Name" } input { name: "name", value: "{filter_name}", maxlength: "32" } }
                        button { r#type: "submit", "Filter" }
                    }
                    if catalog.servers.is_empty() { p { class: "empty", "No server matches." } }
                    else { div { class: "table-wrap", table {
                        caption { class: "visually-hidden", "ProtonVPN servers" }
                        thead { tr { th { scope: "col", "Server" } th { scope: "col", "Location" } th { scope: "col", "Load" }
                            th { scope: "col", "Score" } th { scope: "col", "Features" } th { scope: "col", "Use" } } }
                        tbody { for server in &catalog.servers { tr {
                            td { "data-label": "Server", code { "{server.name}" } }
                            td { "data-label": "Location", {match &server.city { Some(c) => format!("{} · {c}", server.exit_country), None => server.exit_country.clone() }}
                                if server.entry_country != server.exit_country { p { class: "muted", "via {server.entry_country}" } } }
                            td { "data-label": "Load", "{server.load}%" }
                            td { "data-label": "Score", {format!("{:.2}", server.score)} }
                            td { "data-label": "Features", {features_text(server.features)} }
                            td { "data-label": "Use",
                                if page.profiles.is_empty() { span { class: "muted", "Create a profile first" } }
                                else { form { method: "post", action: "/vpn/use",
                                    input { r#type: "hidden", name: "csrf", value: "{user.csrf}" }
                                    input { r#type: "hidden", name: "server", value: "{server.name}" }
                                    select { name: "profile", aria_label: "Profile for {server.name}",
                                        for row in &page.profiles { option { value: "{row.name}", "{row.name}" } } }
                                    button { r#type: "submit", "Use" }
                                } }
                            }
                        } } }
                    } } }
                },
            }
        }
        section { class: "card wide", aria_labelledby: "new-title",
            h2 { id: "new-title", "New profile" }
            form { method: "post", action: "/vpn",
                input { r#type: "hidden", name: "csrf", value: "{user.csrf}" }
                div { class: "field", label { "Name" } input { name: "name", required: true, maxlength: "40", pattern: "[a-z0-9]([a-z0-9-]*[a-z0-9])?" } }
                {settings_fields(&ProtonSpec { vpn_accelerator: true, net_shield: 1, ..Default::default() }, &countries)}
                button { r#type: "submit", "Create profile" }
            }
            p { class: "muted prose", "The profile starts on Auto. Pick a server from the list above to fix it; switching moves the tunnel within a minute and changes the exit address, so open connections drop." }
        }
    }
}

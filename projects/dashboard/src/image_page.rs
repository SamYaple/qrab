//! The Images page (docs/dashboard.md, "Base images"): every BaseImage
//! generation with its import progress, for any signed-in user. Members of
//! the image-admin group also get the stage, refresh and retire forms; for
//! everyone else those POST routes are 404. The page writes only BaseImage
//! specs: the importer downloads, verifies and seals.
use crate::{
    AppState, Failure, access, auth,
    images::{self, BaseImage as Entry},
    launch::{self, FormError},
    ui,
};
use axum::{
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use dioxus::prelude::*;
use qrab_operator::image::{BaseImage, BaseImageSpec, ImageFormat, ImageSource};
use std::collections::BTreeMap;
use tower_sessions::Session;

/// One generation as the page shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct GenerationRow {
    /// The BaseImage's name; `None` for a ConfigMap-only entry.
    pub resource: Option<String>,
    pub generation: u32,
    pub title: String,
    pub os: String,
    pub version: String,
    pub phase: String,
    pub message: String,
    pub attempts: u32,
    pub size_bytes: Option<u64>,
    /// SHA-256 of the sealed raw image.
    pub sha256: Option<String>,
    pub upstream_sha256: String,
    pub source_url: String,
    pub format: Option<ImageFormat>,
    pub adopted: bool,
    pub retired: bool,
    pub launchable: bool,
}

impl GenerationRow {
    pub fn from_resource(resource: &BaseImage) -> Self {
        let status = resource.status.clone().unwrap_or_default();
        let spec = &resource.spec;
        let mut message = status.message.clone();
        if !images::well_named(resource) {
            message = format!(
                "Misnamed: the importer stages only {}. {message}",
                BaseImage::expected_name(spec)
            );
        }
        Self {
            resource: resource.metadata.name.clone(),
            generation: spec.generation,
            title: spec.title.clone(),
            os: spec.os.clone(),
            version: spec.version.clone(),
            phase: resource.phase().as_str().into(),
            message: message.trim().into(),
            attempts: status.attempts,
            size_bytes: status.size_bytes.or(status.capacity_bytes),
            sha256: status.sha256.filter(|s| !s.is_empty()),
            upstream_sha256: spec.source.sha256.clone(),
            source_url: spec.source.url.clone(),
            format: Some(spec.source.format),
            adopted: status.adopted,
            retired: spec.retired,
            launchable: images::launchable(resource),
        }
    }

    fn legacy(entry: &Entry) -> Self {
        Self {
            resource: None,
            generation: entry.generation,
            title: entry.title.clone(),
            os: entry.os.clone(),
            version: entry.version.clone(),
            phase: "Sealed".into(),
            message: format!(
                "Listed by ConfigMap {} only; adopt it as a BaseImage.",
                images::CONFIGMAP
            ),
            attempts: 0,
            size_bytes: Some(entry.size_bytes),
            sha256: Some(entry.sha256.clone()),
            upstream_sha256: entry.upstream_sha256.clone(),
            source_url: entry.source_url.clone(),
            format: None,
            adopted: false,
            retired: false,
            launchable: true,
        }
    }

    /// The download host, for a compact source column.
    pub fn source_host(&self) -> String {
        url::Url::parse(&self.source_url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_owned))
            .unwrap_or_else(|| self.source_url.clone())
    }
}

/// Every generation of one image, newest first.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageGroup {
    pub image: String,
    pub generations: Vec<GenerationRow>,
}

impl ImageGroup {
    /// The generation a refresh creates: one past the newest.
    pub fn next_generation(&self) -> u32 {
        self.generations
            .iter()
            .map(|g| g.generation)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }
}

/// Every BaseImage grouped by image name, plus the ConfigMap entries no
/// BaseImage names. Groups are sorted by name, generations newest first.
pub fn groups(resources: &[BaseImage], legacy: &[Entry]) -> Vec<ImageGroup> {
    let mut by_image: BTreeMap<String, Vec<GenerationRow>> = BTreeMap::new();
    for resource in resources {
        by_image
            .entry(resource.spec.image.clone())
            .or_default()
            .push(GenerationRow::from_resource(resource));
    }
    for entry in legacy {
        let named = resources
            .iter()
            .any(|r| r.spec.image == entry.name && r.spec.generation == entry.generation);
        if !named {
            by_image
                .entry(entry.name.clone())
                .or_default()
                .push(GenerationRow::legacy(entry));
        }
    }
    by_image
        .into_iter()
        .map(|(image, mut generations)| {
            generations.sort_by(|a, b| {
                b.generation
                    .cmp(&a.generation)
                    .then_with(|| a.resource.cmp(&b.resource))
            });
            ImageGroup { image, generations }
        })
        .collect()
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImagesPage {
    pub groups: Vec<ImageGroup>,
    /// The stage form's values (a rejected submission or a prefill).
    pub values: Vec<(String, String)>,
    pub errors: Vec<FormError>,
    /// What prefilled the stage form, e.g. `Refresh cirros`.
    pub prefill: Option<String>,
}

impl ImagesPage {
    fn value(&self, key: &str) -> String {
        launch::field(&self.values, key).to_owned()
    }

    fn invalid(&self, field: &str) -> Option<&'static str> {
        self.errors
            .iter()
            .any(|e| e.field.as_deref() == Some(field))
            .then_some("true")
    }
}

/// The stage form's prefill for `?refresh=<image>` (the next generation with
/// the newest one's text and format; URL and SHA-256 left for the new
/// release) or `?adopt=<image>:<generation>` (a ConfigMap-only entry as it
/// is, so the importer adopts the sealed generation).
pub fn prefill(
    groups: &[ImageGroup],
    query: &[(String, String)],
) -> Option<(String, Vec<(String, String)>)> {
    let pair = |k: &str, v: String| (k.to_owned(), v);
    let text = |row: &GenerationRow| {
        vec![
            pair("title", row.title.clone()),
            pair("os", row.os.clone()),
            pair("version", row.version.clone()),
        ]
    };
    let refresh = launch::field(query, "refresh");
    if !refresh.is_empty() {
        let group = groups.iter().find(|g| g.image == refresh)?;
        let newest = group.generations.first()?;
        let format = group
            .generations
            .iter()
            .find_map(|g| g.format)
            .unwrap_or(ImageFormat::Qcow2);
        let mut values = vec![
            pair("image", group.image.clone()),
            pair("generation", group.next_generation().to_string()),
        ];
        values.extend(text(newest));
        values.push(pair("format", format.as_str().into()));
        return Some((format!("Refresh {}", group.image), values));
    }
    let (image, generation) = launch::field(query, "adopt").split_once(':')?;
    let group = groups.iter().find(|g| g.image == image)?;
    let row = group
        .generations
        .iter()
        .find(|g| g.resource.is_none() && g.generation.to_string() == generation)?;
    let mut values = vec![
        pair("image", group.image.clone()),
        pair("generation", row.generation.to_string()),
    ];
    values.extend(text(row));
    values.extend([
        pair("url", row.source_url.clone()),
        pair("sha256", row.upstream_sha256.clone()),
        pair("format", ImageFormat::Qcow2.as_str().into()),
    ]);
    Some((format!("Adopt {image} g{generation}"), values))
}

/// An HTTPS download URL as the CRD admits it, without credentials.
fn https_url(raw: &str) -> bool {
    raw.starts_with("https://")
        && (9..=2048).contains(&raw.chars().count())
        && !raw.chars().any(|c| c.is_whitespace() || c.is_control())
        && url::Url::parse(raw).is_ok_and(|u| {
            u.scheme() == "https"
                && u.host_str().is_some_and(|h| !h.is_empty())
                && u.username().is_empty()
                && u.password().is_none()
        })
}

/// The stage form as a BaseImage spec, checked as the CRD checks it. The
/// SHA-256 is lowercased first.
pub fn parse_stage(fields: &[(String, String)]) -> Result<BaseImageSpec, Vec<FormError>> {
    let field = |k: &str| launch::field(fields, k);
    let mut errors = Vec::new();
    let image = field("image");
    if !images::image_name(image) {
        errors.push(FormError::at(
            "image",
            "The image name is 1-32 lowercase letters, digits or '-', starting and ending with a letter or digit.",
        ));
    }
    let generation = match field("generation").parse::<u32>() {
        Ok(g) if (1..=999_999).contains(&g) => g,
        _ => {
            errors.push(FormError::at(
                "generation",
                "The generation is a whole number from 1 to 999999.",
            ));
            0
        }
    };
    for (key, label, max) in [
        ("title", "The title", 80),
        ("os", "The OS", 32),
        ("version", "The version", 32),
    ] {
        if !images::text(field(key), max) {
            errors.push(FormError::at(
                key,
                format!("{label} is 1-{max} characters of printable text."),
            ));
        }
    }
    let url = field("url");
    if !https_url(url) {
        errors.push(FormError::at(
            "url",
            "The download URL is an https:// URL of at most 2048 characters, without spaces or credentials.",
        ));
    }
    let sha256 = field("sha256").to_ascii_lowercase();
    if !images::sha256(&sha256) {
        errors.push(FormError::at(
            "sha256",
            "The SHA-256 is 64 hexadecimal digits, as sha256sum prints it.",
        ));
    }
    let format = match field("format") {
        "qcow2" => Some(ImageFormat::Qcow2),
        "raw" => Some(ImageFormat::Raw),
        _ => {
            errors.push(FormError::at("format", "The format is qcow2 or raw."));
            None
        }
    };
    match format {
        Some(format) if errors.is_empty() => Ok(BaseImageSpec {
            image: image.into(),
            generation,
            title: field("title").into(),
            os: field("os").into(),
            version: field("version").into(),
            source: ImageSource {
                url: url.into(),
                sha256,
                format,
            },
            retired: false,
        }),
        _ => Err(errors),
    }
}

async fn sources(app: &AppState) -> Result<(Vec<BaseImage>, Vec<Entry>), Failure> {
    crate::image_sources(app)
        .await
        .map_err(|e| Failure::upstream(&e))
}

pub async fn page(
    State(app): State<AppState>,
    session: Session,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Html<String>, Failure> {
    let user = auth::user(&session).await?;
    let (resources, legacy) = sources(&app).await?;
    let mut body = ImagesPage {
        groups: groups(&resources, &legacy),
        ..Default::default()
    };
    if access::can_admin_images(&user.groups, &app.image_admin_group)
        && let Some((label, values)) = prefill(&body.groups, &query)
    {
        body.prefill = Some(label);
        body.values = values;
    }
    crate::page(&app, &session, &user, ui::Body::Images(Box::new(body))).await
}

/// CSRF and exact Origin, then an image admin; anyone else gets 404.
async fn mutation(
    app: &AppState,
    session: &Session,
    headers: &HeaderMap,
    fields: &[(String, String)],
) -> Result<auth::User, Failure> {
    let user = crate::mutation(app, session, headers, fields).await?;
    if !access::can_admin_images(&user.groups, &app.image_admin_group) {
        return Err(Failure(
            StatusCode::NOT_FOUND,
            "This page could not be found.".into(),
        ));
    }
    Ok(user)
}

/// Stage a new image or a refresh: a new BaseImage `<image>-g<generation>`.
pub async fn stage(
    State(app): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response, Failure> {
    let user = mutation(&app, &session, &headers, &fields).await?;
    let (resources, legacy) = sources(&app).await?;
    let groups = groups(&resources, &legacy);
    let spec = parse_stage(&fields).and_then(|spec| {
        let name = BaseImage::expected_name(&spec);
        let taken = resources.iter().any(|r| {
            (r.spec.image == spec.image && r.spec.generation == spec.generation)
                || r.metadata.name.as_deref() == Some(name.as_str())
        });
        if !taken {
            return Ok(spec);
        }
        let next = groups
            .iter()
            .find(|g| g.image == spec.image)
            .map_or(spec.generation + 1, ImageGroup::next_generation);
        Err(vec![FormError::at(
            "generation",
            format!(
                "{} g{} already exists, and a generation never changes. A refresh is a new generation, such as {next}.",
                spec.image, spec.generation
            ),
        )])
    });
    let spec = match spec {
        Ok(spec) => spec,
        Err(errors) => {
            let body = ImagesPage {
                groups,
                values: fields.into_iter().filter(|(k, _)| k != "csrf").collect(),
                errors,
                prefill: None,
            };
            let mut html = crate::page(&app, &session, &user, ui::Body::Images(Box::new(body)))
                .await?
                .into_response();
            *html.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(html);
        }
    };
    let name = BaseImage::expected_name(&spec);
    let title = spec.title.clone();
    let mut image = BaseImage::new(&name, spec);
    image.metadata.namespace = Some(app.namespace.clone());
    image.metadata.annotations = Some(
        [(
            qrab_operator::resources::CREATOR.to_owned(),
            user.subject.clone(),
        )]
        .into(),
    );
    image.metadata.labels =
        Some([(access::MANAGED_BY.to_owned(), access::MANAGER.to_owned())].into());
    let result = app.cluster.create_base_image(&image).await;
    tracing::info!(subject=%user.subject, image=%name, success=result.is_ok(), "BaseImage created");
    result.map_err(|e| Failure::upstream(&e))?;
    crate::flash(
        &session,
        format!(
            "Staging {title} as {name}. The importer downloads it, checks the SHA-256 before reading it, then writes and seals it; this page shows its progress."
        ),
    )
    .await?;
    Ok(Redirect::to("/images").into_response())
}

/// Retire one generation: no new clones. Irreversible (the CRD keeps a
/// retired generation retired).
pub async fn retire(
    State(app): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(name): Path<String>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Redirect, Failure> {
    let user = mutation(&app, &session, &headers, &fields).await?;
    if launch::field(&fields, "confirm") != "yes" {
        return Err(Failure::invalid(&format!(
            "Confirm retiring {name}: it cannot be undone."
        )));
    }
    let current = app
        .cluster
        .list_base_images()
        .await
        .map_err(|e| Failure::upstream(&e))?
        .into_iter()
        .find(|r| r.metadata.name.as_deref() == Some(name.as_str()))
        .ok_or_else(|| Failure::invalid("No such base image."))?;
    if current.spec.retired {
        return Err(Failure::invalid(&format!("{name} is already retired.")));
    }
    let uid = current.metadata.uid.clone().ok_or_else(Failure::internal)?;
    let result = app
        .cluster
        .patch_base_image_retired(&name, &uid, true)
        .await;
    tracing::info!(subject=%user.subject, image=%name, %uid, success=result.is_ok(), "BaseImage retired");
    result.map_err(|e| Failure::upstream(&e))?;
    crate::flash(
        &session,
        format!("Retired {name}. New VMs can no longer clone it; existing VMs keep their disks. This cannot be undone."),
    )
    .await?;
    Ok(Redirect::to("/images"))
}

/// `0123456789ab…`, with the whole digest in the title.
fn digest(value: &Option<String>) -> Element {
    match value {
        Some(full) => {
            let short: String = full.chars().take(12).collect();
            rsx! { code { title: "{full}", "{short}…" } }
        }
        None => rsx! { span { class: "muted", "—" } },
    }
}

fn phase(row: &GenerationRow) -> Element {
    let class = if row.launchable {
        "badge ok"
    } else if row.phase == "Failed" {
        "badge warn"
    } else {
        "badge"
    };
    rsx! {
        span { class: "{class}", "{row.phase}" }
        if row.launchable { p { class: "muted", "Launchable" } }
        if !row.message.is_empty() { p { class: "muted", "{row.message}" } }
        if row.attempts == 1 { p { class: "muted", "1 failed attempt" } }
        if row.attempts > 1 { p { class: "muted", "{row.attempts} failed attempts" } }
    }
}

fn group_view(group: &ImageGroup, user: &ui::UserView) -> Element {
    let heading = format!("image-{}", group.image);
    let next = group.next_generation();
    rsx! {
        section { class: "card wide directory image-directory", aria_labelledby: "{heading}",
            div { class: "heading",
                h2 { id: "{heading}", code { "{group.image}" } }
                if user.image_admin {
                    a { class: "button secondary", href: "/images?refresh={group.image}#stage", "Refresh (generation {next})" }
                }
            }
            div { class: "table-wrap", table {
                caption { class: "visually-hidden", "Generations of {group.image}" }
                thead { tr {
                    th { scope: "col", "Generation" } th { scope: "col", "Image" } th { scope: "col", "Phase" }
                    th { scope: "col", "Size" } th { scope: "col", "Raw SHA-256" } th { scope: "col", "Source" }
                    th { scope: "col", "Adopted" } th { scope: "col", "Retired" }
                } }
                tbody { for row in &group.generations { tr {
                    td { "data-label": "Generation", strong { "g{row.generation}" }
                        if let Some(name) = &row.resource { p { class: "muted", code { "{name}" } } } }
                    td { "data-label": "Image", div { strong { "{row.title}" } p { class: "muted", "{row.os} {row.version}" } } }
                    td { "data-label": "Phase", {phase(row)} }
                    td { "data-label": "Size", {row.size_bytes.map(ui::mib).unwrap_or_else(|| "—".into())} }
                    td { "data-label": "Raw SHA-256", {digest(&row.sha256)} }
                    td { "data-label": "Source",
                        a { href: "{row.source_url}", rel: "noreferrer", title: "{row.source_url}", "{row.source_host()}" }
                        if let Some(format) = row.format { p { class: "muted", {format.as_str()} } }
                    }
                    td { "data-label": "Adopted", if row.adopted { "Yes" } else { "No" } }
                    td { "data-label": "Retired",
                        if row.retired { span { class: "badge", "Retired" } } else { "No" }
                        if user.image_admin {
                            match &row.resource {
                                Some(name) if !row.retired => rsx! {
                                    form { method: "post", action: "/images/{name}/retire",
                                        input { r#type: "hidden", name: "csrf", value: "{user.csrf}" }
                                        label { class: "check", input { r#type: "checkbox", name: "confirm", value: "yes", required: true } " Retire g{row.generation} permanently" }
                                        button { class: "danger", r#type: "submit", "Retire" }
                                    }
                                },
                                None => rsx! { p { a { href: "/images?adopt={group.image}:{row.generation}#stage", "Adopt as a BaseImage" } } },
                                _ => rsx! {},
                            }
                        }
                    }
                } } }
            } }
        }
    }
}

fn text_field(page: &ImagesPage, key: &str, label: &str, input: Element) -> Element {
    let messages: Vec<&FormError> = page
        .errors
        .iter()
        .filter(|e| e.field.as_deref() == Some(key))
        .collect();
    rsx! {
        div { class: "field",
            label { r#for: "stage-{key}", "{label}" }
            {input}
            if !messages.is_empty() {
                div { id: "error-{key}", class: "field-error", for error in messages { p { "{error.message}" } } }
            }
        }
    }
}

fn stage_form(page: &ImagesPage, user: &ui::UserView) -> Element {
    let v = |k: &str| page.value(k);
    let described = |k: &str| page.invalid(k).map(|_| format!("error-{k}"));
    let format = match v("format").as_str() {
        "raw" => "raw",
        _ => "qcow2",
    };
    let heading = page
        .prefill
        .clone()
        .unwrap_or_else(|| "Stage an image".into());
    rsx! {
        section { class: "card wide", id: "stage", aria_labelledby: "stage-heading",
            h2 { id: "stage-heading", "{heading}" }
            p { class: "muted prose", "A new image, or a refresh of an existing one, is a new generation. The importer downloads the URL, checks its SHA-256 before reading the file, converts it to a raw disk and seals it; the generation's phase above shows its progress. A generation never changes after it is staged." }
            form { method: "post", action: "/images",
                input { r#type: "hidden", name: "csrf", value: "{user.csrf}" }
                div { class: "pair",
                    {text_field(page, "image", "Image name", rsx! { input { id: "stage-image", name: "image", value: v("image"), required: true, maxlength: "32", pattern: "[a-z0-9]([a-z0-9-]*[a-z0-9])?", aria_invalid: page.invalid("image"), aria_describedby: described("image") } })}
                    {text_field(page, "generation", "Generation", rsx! { input { id: "stage-generation", name: "generation", value: v("generation"), required: true, r#type: "number", min: "1", max: "999999", aria_invalid: page.invalid("generation"), aria_describedby: described("generation") } })}
                }
                {text_field(page, "title", "Title", rsx! { input { id: "stage-title", name: "title", value: v("title"), required: true, maxlength: "80", placeholder: "CirrOS 0.6.3", aria_invalid: page.invalid("title"), aria_describedby: described("title") } })}
                div { class: "pair",
                    {text_field(page, "os", "OS", rsx! { input { id: "stage-os", name: "os", value: v("os"), required: true, maxlength: "32", placeholder: "cirros", aria_invalid: page.invalid("os"), aria_describedby: described("os") } })}
                    {text_field(page, "version", "Version", rsx! { input { id: "stage-version", name: "version", value: v("version"), required: true, maxlength: "32", placeholder: "0.6.3", aria_invalid: page.invalid("version"), aria_describedby: described("version") } })}
                }
                {text_field(page, "url", "Download URL (HTTPS)", rsx! { input { id: "stage-url", name: "url", value: v("url"), required: true, r#type: "url", maxlength: "2048", placeholder: "https://", aria_invalid: page.invalid("url"), aria_describedby: described("url") } })}
                {text_field(page, "sha256", "SHA-256 of the downloaded file", rsx! { input { id: "stage-sha256", name: "sha256", value: v("sha256"), required: true, maxlength: "64", pattern: "[0-9a-fA-F]{{64}}", autocomplete: "off", spellcheck: "false", aria_invalid: page.invalid("sha256"), aria_describedby: described("sha256") } })}
                {text_field(page, "format", "Format", rsx! { select { id: "stage-format", name: "format", aria_invalid: page.invalid("format"), aria_describedby: described("format"),
                    option { value: "qcow2", selected: format == "qcow2", "qcow2" }
                    option { value: "raw", selected: format == "raw", "raw" }
                } })}
                button { r#type: "submit", "Stage image" }
            }
        }
    }
}

pub fn view(page: &ImagesPage, user: &ui::UserView) -> Element {
    let count: usize = page.groups.iter().map(|g| g.generations.len()).sum();
    rsx! {
        div { class: "heading", div { p { class: "eyebrow", "BASE IMAGES" } h1 { "Operating system images" }
            p { class: "muted", "Every image generation and its import progress, newest first. Only sealed generations that are not retired can be launched." } }
            span { class: "count", "{count} generations" } }
        if !page.errors.is_empty() {
            section { class: "card form-errors", id: "form-errors", tabindex: "-1", role: "alert", aria_labelledby: "form-error-title", autofocus: true,
                h2 { id: "form-error-title", "Check the form" }
                p { class: "muted", "Nothing was staged. Choose an error to go to its field; your values are kept below." }
                ul { for error in &page.errors { li {
                    if let Some(field) = &error.field { a { href: "#stage-{field}", "{error.message}" } }
                    else { "{error.message}" }
                } } }
            }
        }
        if page.groups.is_empty() {
            section { class: "card wide", p { class: "empty",
                if user.image_admin { "No base images are staged yet. Stage one below." }
                else { "No base images are staged yet. An image admin can stage an operating-system image." }
            } }
        }
        for group in &page.groups { {group_view(group, user)} }
        if user.image_admin { {stage_form(page, user)} }
        section { class: "card wide", aria_labelledby: "how-title",
            h2 { id: "how-title", "A separate disk for each VM" }
            p { class: "prose", "Launching from an image creates a writable root disk for that VM. Your changes stay with the VM; the image remains unchanged. A new image release is listed as a new generation." }
            details { id: "image-staging", class: "inline-disclosure", summary { "Image verification and staging" }
              ol { class: "prose",
                li { "An image admin stages a generation here with its HTTPS download URL, the upstream SHA-256 and the file format. That creates a " code { "BaseImage" } " named " code { "<image>-g<generation>" } "." }
                li { "The image importer downloads the file, checks the SHA-256 before anything parses it, converts it to a raw disk on a staging volume, and the storage host seals it as the read-only snapshot " code { "image-<image>-g<generation>@sealed" } " after hashing it itself. A generation that is already sealed is adopted as it is." }
                li { "Each VM launched with it gets its own copy-on-write clone of that snapshot as its " code { "root" } " disk, exported over iSCSI and booted by UEFI. The image itself never changes; a refresh is a new generation, and retiring a generation stops new clones while existing VMs keep theirs." }
              }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::images::tests::{cirros, resource};

    fn form(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        let mut fields: Vec<(String, String)> = [
            ("image", "cirros"),
            ("generation", "2"),
            ("title", "CirrOS 0.6.3"),
            ("os", "cirros"),
            ("version", "0.6.3"),
            (
                "url",
                "https://download.cirros-cloud.net/0.6.3/cirros-0.6.3-x86_64-disk.img",
            ),
            (
                "sha256",
                "7D6355852AEB6DBCD191BCDA7CD74F1536CFE5CBF8A10495A7283A8396E4B75B",
            ),
            ("format", "qcow2"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        for (k, v) in pairs {
            fields.retain(|(key, _)| key != k);
            fields.push((k.to_string(), v.to_string()));
        }
        fields
    }

    #[test]
    fn the_stage_form_is_checked_as_the_crd_checks_it() {
        let spec = parse_stage(&form(&[])).unwrap();
        assert_eq!(BaseImage::expected_name(&spec), "cirros-g2");
        assert_eq!(
            spec.source.sha256,
            "7d6355852aeb6dbcd191bcda7cd74f1536cfe5cbf8a10495a7283a8396e4b75b"
        );
        let long_url = format!("https://example.com/{}", "a".repeat(2030));
        for (key, value) in [
            ("image", "Bad_Name"),
            ("image", "-a"),
            ("image", &"a".repeat(33)),
            ("generation", "0"),
            ("generation", "1000000"),
            ("generation", "x"),
            ("title", ""),
            ("title", &"t".repeat(81)),
            ("os", &"o".repeat(33)),
            ("version", "a\u{7}b"),
            ("url", "http://download.example/x.img"),
            ("url", "HTTPS://download.example/x.img"),
            ("url", "https://user:pw@download.example/x.img"),
            ("url", "https://download.example/a b.img"),
            ("url", long_url.as_str()),
            ("sha256", "abc"),
            ("sha256", &"g".repeat(64)),
            ("format", "vmdk"),
        ] {
            let errors = parse_stage(&form(&[(key, value)])).unwrap_err();
            assert_eq!(errors.len(), 1, "{key}={value}");
            assert_eq!(errors[0].field.as_deref(), Some(key), "{key}={value}");
        }
        assert!(
            parse_stage(&form(&[(
                "url",
                &format!("https://e.io/{}", "a".repeat(2035))
            )]))
            .is_ok()
        );
    }

    #[test]
    fn groups_list_newest_first_and_refresh_prefills_the_next_generation() {
        let mut raw = resource("cirros", 2, "Writing", false);
        raw.spec.source.format = ImageFormat::Raw;
        let groups = groups(
            &[
                resource("cirros", 3, "Sealed", true),
                raw,
                resource("alpine", 1, "Failed", false),
            ],
            &[cirros()],
        );
        assert_eq!(
            groups.iter().map(|g| g.image.as_str()).collect::<Vec<_>>(),
            ["alpine", "cirros"]
        );
        let cirros_group = &groups[1];
        assert_eq!(
            cirros_group
                .generations
                .iter()
                .map(|g| g.generation)
                .collect::<Vec<_>>(),
            [3, 2, 1]
        );
        assert!(
            cirros_group.generations[2].resource.is_none()
                && cirros_group.generations[2].launchable
        );
        assert!(!cirros_group.generations[0].launchable && cirros_group.generations[0].retired);
        let query = |k: &str, v: &str| vec![(k.to_string(), v.to_string())];
        let (label, values) = prefill(&groups, &query("refresh", "cirros")).unwrap();
        assert_eq!(label, "Refresh cirros");
        let get = |k: &str| launch::field(&values, k).to_owned();
        assert_eq!(
            (
                get("generation"),
                get("title"),
                get("format"),
                get("url"),
                get("sha256")
            ),
            (
                "4".into(),
                "CirrOS 0.6.3".into(),
                "qcow2".into(),
                String::new(),
                String::new()
            )
        );
        let (label, values) = prefill(&groups, &query("adopt", "cirros:1")).unwrap();
        assert_eq!(label, "Adopt cirros g1");
        assert_eq!(launch::field(&values, "sha256"), cirros().upstream_sha256);
        assert!(
            prefill(&groups, &query("adopt", "cirros:3")).is_none(),
            "only ConfigMap entries"
        );
        assert!(prefill(&groups, &query("refresh", "missing")).is_none());
        assert!(prefill(&groups, &[]).is_none());
    }
}

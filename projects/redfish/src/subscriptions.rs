//! Opt-in bounded Redfish push subscriptions. Destinations are exact operator
//! configured HTTPS URLs; no bearer tokens or arbitrary headers are retained.
use super::*;
use futures::StreamExt;
use openidconnect::reqwest::{self, Url};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
};

pub(super) const COLLECTION: &str = "/redfish/v1/EventService/Subscriptions";
const LIMIT: usize = 32;
const FILE_LIMIT: usize = 128 * 1024;
pub(super) const DELIVERY_RETRY_ATTEMPTS: u32 = 2;
pub(super) const DELIVERY_RETRY_INTERVAL: u64 = 2;
const RETRIES: u32 = DELIVERY_RETRY_ATTEMPTS + 1;
const MAX_LIFETIME: u64 = 24 * 3600;

#[derive(Clone)]
pub struct SubscriptionStore(Arc<Store>);
struct Store {
    allowed: BTreeSet<String>,
    path: Option<PathBuf>,
    state: Mutex<Persisted>,
    write_failed: AtomicBool,
    writing: Mutex<()>,
    admission: Arc<Semaphore>,
    writes: AtomicU64,
    attempts: AtomicU64,
    delivered: AtomicU64,
    skipped: AtomicU64,
    batches: AtomicU64,
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Persisted {
    next_id: u64,
    entries: BTreeMap<u64, Subscription>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Subscription {
    id: u64,
    destination: String,
    context: String,
    expires: u64,
    registry_prefixes: Vec<String>,
    resource_types: Vec<String>,
    severities: Vec<String>,
    cursor: u64,
    failures: u32,
    retry_at: u64,
    missed: u64,
}

impl Default for SubscriptionStore {
    fn default() -> Self {
        Self(Arc::new(Store {
            allowed: BTreeSet::new(),
            path: None,
            state: Mutex::new(Persisted::default()),
            write_failed: AtomicBool::new(false),
            writing: Mutex::new(()),
            admission: Arc::new(Semaphore::new(1)),
            writes: AtomicU64::new(0),
            attempts: AtomicU64::new(0),
            delivered: AtomicU64::new(0),
            skipped: AtomicU64::new(0),
            batches: AtomicU64::new(0),
        }))
    }
}

fn destination(value: &str) -> Result<String, String> {
    if value.len() > 2048 {
        return Err("event destination is too long".into());
    }
    let url = Url::parse(value).map_err(|_| "event destination is not a URL")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.query().is_some()
    {
        return Err(
            "event destination must be HTTPS without credentials, query or fragment".into(),
        );
    }
    Ok(url.to_string())
}

impl SubscriptionStore {
    /// An empty allowlist disables creation and outbound push delivery.
    pub fn new(destinations: Vec<String>, path: Option<PathBuf>) -> Result<Self, String> {
        if destinations.len() > LIMIT {
            return Err("too many event destinations".into());
        }
        let allowed: BTreeSet<_> = destinations
            .iter()
            .map(|d| destination(d))
            .collect::<Result<_, _>>()?;
        if path.as_ref().is_some_and(|p| !p.is_absolute()) {
            return Err("subscription state path must be absolute".into());
        }
        let mut state = Persisted::default();
        let mut pruned = false;
        if let Some(path) = &path {
            match std::fs::symlink_metadata(path) {
                Ok(meta) => {
                    if !meta.is_file()
                        || meta.len() > FILE_LIMIT as u64
                        || meta.permissions().mode() & 0o077 != 0
                    {
                        return Err("subscription state must be a private regular file within its byte limit".into());
                    }
                    let mut bytes = Vec::new();
                    std::fs::File::open(path)
                        .map_err(|_| "cannot open subscription state")?
                        .take(FILE_LIMIT as u64 + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|_| "cannot read subscription state")?;
                    if bytes.len() > FILE_LIMIT {
                        return Err("subscription state exceeds byte limit".into());
                    }
                    state =
                        serde_json::from_slice(&bytes).map_err(|_| "invalid subscription state")?;
                    if state.entries.len() > LIMIT {
                        return Err("too many persisted subscriptions".into());
                    }
                    for (id, entry) in &state.entries {
                        if *id != entry.id
                            || *id == 0
                            || *id > state.next_id
                            || entry.context.len() > 256
                            || entry.failures >= RETRIES
                            || entry.registry_prefixes.iter().any(|v| v != "Qrab")
                            || entry.resource_types.iter().any(|v| v != "ComputerSystem")
                            || entry
                                .severities
                                .iter()
                                .any(|v| !["OK", "Warning", "Critical"].contains(&v.as_str()))
                            || entry.context.chars().any(char::is_control)
                            || entry.registry_prefixes.len() > 1
                            || entry.resource_types.len() > 1
                            || entry.severities.len() > 3
                            || entry.expires > auth::now().saturating_add(MAX_LIFETIME)
                        {
                            return Err("invalid persisted subscription".into());
                        }
                        if destination(&entry.destination)? != entry.destination {
                            return Err("invalid persisted destination".into());
                        }
                    }
                    let previous_count = state.entries.len();
                    state
                        .entries
                        .retain(|_, e| e.expires > auth::now() && allowed.contains(&e.destination));
                    pruned = state.entries.len() != previous_count;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err("cannot inspect subscription state".into()),
            }
        }
        let store = Self(Arc::new(Store {
            allowed,
            path,
            state: Mutex::new(state),
            write_failed: AtomicBool::new(false),
            writing: Mutex::new(()),
            admission: Arc::new(Semaphore::new(1)),
            writes: AtomicU64::new(0),
            attempts: AtomicU64::new(0),
            delivered: AtomicU64::new(0),
            skipped: AtomicU64::new(0),
            batches: AtomicU64::new(0),
        }));
        // Revoking a destination removes its retained subscriptions instead of
        // making the Redfish listener unavailable. Persist pruning so a later
        // configuration change cannot resurrect a revoked subscription.
        if pruned {
            store.write(&store.0.state.lock().unwrap())?;
        }
        Ok(store)
    }

    pub fn from_env() -> Result<Self, String> {
        let allowed = std::env::var("REDFISH_EVENT_DESTINATIONS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        let path = std::env::var_os("REDFISH_SUBSCRIPTIONS_FILE").map(PathBuf::from);
        Self::new(allowed, path)
    }

    pub fn enabled(&self) -> bool {
        !self.0.allowed.is_empty() && !self.0.write_failed.load(Ordering::Acquire)
    }
    pub fn persistent(&self) -> bool {
        self.0.path.is_some()
    }

    fn write(&self, state: &Persisted) -> Result<(), String> {
        let Some(path) = &self.0.path else {
            return Ok(());
        };
        let bytes = serde_json::to_vec(state).map_err(|_| "cannot serialize subscription state")?;
        if bytes.len() > FILE_LIMIT {
            return Err("subscription state exceeds byte limit".into());
        }
        let parent = path
            .parent()
            .ok_or("subscription state needs a parent directory")?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("invalid subscription filename")?;
        let temp = parent.join(format!(
            ".{name}.{}.{}.tmp",
            std::process::id(),
            state.next_id
        ));
        let result = (|| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temp)
                .map_err(|_| "cannot create subscription temporary state")?;
            file.write_all(&bytes)
                .map_err(|_| "cannot write subscription state")?;
            file.sync_all()
                .map_err(|_| "cannot synchronize subscription state")?;
            std::fs::rename(&temp, path).map_err(|_| "cannot replace subscription state")?;
            std::fs::File::open(parent)
                .and_then(|dir| dir.sync_all())
                .map_err(|_| "cannot synchronize subscription state directory")?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(temp);
        } else {
            self.0.writes.fetch_add(1, Ordering::Relaxed);
        }
        result
    }

    fn mutate<T>(
        &self,
        change: impl FnOnce(&mut Persisted) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        // Serialize mutations without holding the reader mutex across fsync.
        let _writer = self.0.writing.lock().unwrap();
        let mut next = self.0.state.lock().unwrap().clone();
        next.entries.retain(|_, e| e.expires > auth::now());
        let result = change(&mut next)?;
        if *self.0.state.lock().unwrap() == next {
            return Ok(result);
        }
        if self.write(&next).is_err() {
            self.0.write_failed.store(true, Ordering::Release);
            tracing::warn!("Event delivery paused: subscription persistence is unavailable");
            return Err(Failure::unavailable(
                "Subscription state could not be saved.",
            ));
        }
        self.0.write_failed.store(false, Ordering::Release);
        *self.0.state.lock().unwrap() = next;
        Ok(result)
    }

    async fn mutate_async<
        T: Send + 'static,
        F: FnOnce(&mut Persisted) -> Result<T, Failure> + Send + 'static,
    >(
        &self,
        wait: bool,
        change: F,
    ) -> Result<T, Failure> {
        if !self.persistent() {
            return self.mutate(change);
        }
        // HTTP writers never form an unbounded blocking-task queue. The one
        // background worker may wait with its bounded batch, without resending.
        let permit = if wait {
            self.0
                .admission
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| Failure::unavailable("Subscription persistence is unavailable."))?
        } else {
            self.0
                .admission
                .clone()
                .try_acquire_owned()
                .map_err(|_| Failure::unavailable("Subscription persistence is busy."))?
        };
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            store.mutate(change)
        })
        .await
        .map_err(|_| Failure::unavailable("Subscription persistence could not complete."))?
    }

    pub(super) fn stats(&self) -> Value {
        json!({"RetainedSubscriptions":self.0.state.lock().unwrap().entries.values().filter(|entry|entry.expires>auth::now()).count(),"StateWrites":self.0.writes.load(Ordering::Relaxed),
            "DeliveryAttempts":self.0.attempts.load(Ordering::Relaxed),"DeliveredEvents":self.0.delivered.load(Ordering::Relaxed),
            "FilteredEvents":self.0.skipped.load(Ordering::Relaxed),"CommittedDeliveryBatches":self.0.batches.load(Ordering::Relaxed),
            "PersistenceBusy":self.0.admission.available_permits()==0,"PersistenceAvailable":!self.0.write_failed.load(Ordering::Acquire)})
    }

    fn list(&self) -> Vec<Subscription> {
        self.0
            .state
            .lock()
            .unwrap()
            .entries
            .values()
            .filter(|e| e.expires > auth::now())
            .cloned()
            .collect()
    }
    fn get(&self, id: u64) -> Option<Subscription> {
        self.0
            .state
            .lock()
            .unwrap()
            .entries
            .get(&id)
            .filter(|e| e.expires > auth::now())
            .cloned()
    }
}

fn uri(id: u64) -> String {
    format!("{COLLECTION}/{id}")
}
fn id(value: &str) -> Result<u64, Failure> {
    value
        .parse::<u64>()
        .ok()
        .filter(|n| *n > 0 && n.to_string() == value)
        .ok_or_else(Failure::not_found)
}
fn invalid(message: &'static str) -> Failure {
    Failure::new(StatusCode::BAD_REQUEST,"ActionParameterNotSupported",message,"Use supported event subscription properties and an operator-allowlisted HTTPS destination.")
}

impl Subscription {
    fn json(&self, admin: bool) -> Value {
        let mut value = json!({
            "@odata.id":uri(self.id),"@odata.type":"#EventDestination.v1_15_0.EventDestination",
            "Id":self.id.to_string(),"Name":"VM event receiver","Protocol":"Redfish","SubscriptionType":"RedfishEvent",
            "EventFormatType":"Event","DeliveryRetryPolicy":"TerminateAfterRetries","VerifyCertificate":true,
            "RegistryPrefixes":self.registry_prefixes,"ResourceTypes":self.resource_types,"Severities":self.severities,
            "HttpHeaders":[],"Status":{"State":"Enabled","Health":if self.failures==0{"OK"}else{"Warning"}},
            "Oem":{"Qrab":{"@odata.type":"#QrabEventDestination.v1_0_0.QrabEventDestination",
                "Expires":rfc3339(self.expires.saturating_mul(1000)),"MissedEvents":self.missed,"ConsecutiveDeliveryFailures":self.failures}}
        });
        if admin {
            value["Destination"] = json!(self.destination);
            value["Context"] = json!(self.context);
        }
        value
    }
    fn accepts(&self, event: &VmEvent) -> bool {
        self.severities.is_empty() || self.severities.iter().any(|s| s == severity(&event.event))
    }
}

pub(super) async fn collection_handler(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    Ok(Json(collection(
        COLLECTION,
        "EventDestinationCollection",
        "Event subscriptions",
        app.subscriptions
            .list()
            .iter()
            .map(|e| link(&uri(e.id)))
            .collect(),
    )))
}

fn filter(body: &Value, name: &'static str, allowed: &[&str]) -> Result<Vec<String>, Failure> {
    let values = string_list(body, name)?.unwrap_or_default();
    if values.len() > allowed.len() || values.iter().any(|v| !allowed.contains(&v.as_str())) {
        return Err(invalid("Unsupported event filter value."));
    }
    Ok(values)
}

pub(super) async fn create(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Failure> {
    let who = admin(&app, &headers).await?;
    if !app.subscriptions.enabled() {
        return Err(Failure::unavailable(
            "Push delivery is disabled; no event destinations are configured.",
        ));
    }
    let body = action_body(&body)?;
    if body.as_object().is_some_and(|m| {
        m.keys().any(|k| {
            !matches!(
                k.as_str(),
                "Destination"
                    | "Protocol"
                    | "Context"
                    | "RegistryPrefixes"
                    | "ResourceTypes"
                    | "Severities"
                    | "EventFormatType"
                    | "SubscriptionType"
                    | "DeliveryRetryPolicy"
                    | "VerifyCertificate"
            )
        })
    }) {
        return Err(invalid("Unsupported event subscription property."));
    }
    if body["Protocol"] != "Redfish"
        || body.get("EventFormatType").is_some_and(|v| v != "Event")
        || body
            .get("SubscriptionType")
            .is_some_and(|v| v != "RedfishEvent")
        || body
            .get("DeliveryRetryPolicy")
            .is_some_and(|v| v != "TerminateAfterRetries")
        || body.get("VerifyCertificate").is_some_and(|v| v != true)
    {
        return Err(invalid(
            "Unsupported event protocol, delivery policy or certificate setting.",
        ));
    }
    let destination = body["Destination"]
        .as_str()
        .ok_or_else(|| invalid("Destination is required."))?;
    let destination = super::subscriptions::destination(destination)
        .map_err(|_| invalid("Invalid event destination."))?;
    if !app.subscriptions.0.allowed.contains(&destination) {
        return Err(Failure::new(
            StatusCode::FORBIDDEN,
            "InsufficientPrivilege",
            "The event destination is not operator-allowlisted.",
            "Ask the operator to configure this exact HTTPS destination.",
        ));
    }
    let context = match body.get("Context") {
        None => "",
        Some(v) => v
            .as_str()
            .ok_or_else(|| invalid("Context must be a string."))?,
    };
    if context.len() > 256 || context.chars().any(char::is_control) {
        return Err(invalid(
            "Context exceeds its bound or contains control characters.",
        ));
    }
    let entry = Subscription {
        id: 0,
        destination,
        context: context.into(),
        expires: who.expires.min(auth::now().saturating_add(MAX_LIFETIME)),
        registry_prefixes: filter(&body, "RegistryPrefixes", &["Qrab"])?,
        resource_types: filter(&body, "ResourceTypes", &["ComputerSystem"])?,
        severities: filter(&body, "Severities", &["OK", "Warning", "Critical"])?,
        cursor: app.control.event_summaries(None, 1).latest,
        failures: 0,
        retry_at: 0,
        missed: 0,
    };
    let entry =
        app.subscriptions
            .mutate_async(false, move |state| {
                if state.entries.len() >= LIMIT {
                    return Err(Failure::unavailable(
                        "The event subscription limit has been reached.",
                    ));
                }
                state.next_id = state.next_id.checked_add(1).ok_or_else(|| {
                    Failure::unavailable("The subscription ID range is exhausted.")
                })?;
                let entry = Subscription {
                    id: state.next_id,
                    ..entry
                };
                state.entries.insert(entry.id, entry.clone());
                Ok(entry)
            })
            .await?;
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, uri(entry.id))],
        Json(entry.json(true)),
    )
        .into_response())
}

pub(super) async fn get(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(value): Path<String>,
) -> Result<Json<Value>, Failure> {
    let who = principal(&app, &headers).await?;
    let entry = app
        .subscriptions
        .get(id(&value)?)
        .ok_or_else(Failure::not_found)?;
    Ok(Json(entry.json(who.role == Role::Admin)))
}

pub(super) async fn delete(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(value): Path<String>,
) -> Result<StatusCode, Failure> {
    admin(&app, &headers).await?;
    let id = id(&value)?;
    app.subscriptions
        .mutate_async(false, move |s| {
            s.entries
                .remove(&id)
                .map(|_| ())
                .ok_or_else(Failure::not_found)
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn outbound(event: &VmEvent, context: &str) -> Vec<u8> {
    // Construct only the sanitized envelope. Never clone a potentially 8KiB
    // QMP payload merely to discard it before delivery.
    serde_json::to_vec(&json!({"@odata.type":"#Event.v1_10_0.Event","Id":event.seq.to_string(),"Name":"qrab VM event","Context":context,
        "Events":[{"EventType":"Other","EventId":event.seq.to_string(),"EventTimestamp":event_time(event),"Severity":severity(&event.event),
            "Message":event.event,"MessageId":"Qrab.1.0.QmpEvent","MessageArgs":[event.event],"OriginOfCondition":link(SYSTEM),
            "Oem":{"Qrab":{"@odata.type":"#QrabEvent.v1_0_0.QrabEvent","QapiEvent":event.event,"Sequence":event.seq}}}]})).expect("event JSON serializes")
}

struct DeliveryUpdate {
    id: u64,
    expected_cursor: u64,
    next_cursor: u64,
    missed: u64,
    skipped: u64,
    success: bool,
    delivered: bool,
}

async fn deliver(
    app: &AppState,
    client: &reqwest::Client,
    entry: Subscription,
) -> Option<DeliveryUpdate> {
    if !app.subscriptions.enabled()
        || entry.retry_at > auth::now()
        || app.subscriptions.get(entry.id).is_none()
    {
        return None;
    }
    let latest = app.control.event_summaries(None, 1).latest;
    let cursor = if entry.cursor > latest {
        0
    } else {
        entry.cursor
    };
    // Skip nonmatching events in one bounded page instead of one disk commit
    // and one 250ms timer per filtered event.
    let page = app.control.event_summaries(Some(cursor), 64);
    if page.events.is_empty() {
        return None;
    }
    let matched = page.events.iter().position(|event| entry.accepts(event));
    let Some(index) = matched else {
        return Some(DeliveryUpdate {
            id: entry.id,
            expected_cursor: entry.cursor,
            next_cursor: page.next,
            missed: page.missed,
            skipped: page.events.len() as u64,
            success: true,
            delivered: false,
        });
    };
    let event = &page.events[index];
    app.subscriptions.0.attempts.fetch_add(1, Ordering::Relaxed);
    let response = client
        .post(&entry.destination)
        .header(header::CONTENT_TYPE.as_str(), "application/json")
        .body(outbound(event, &entry.context))
        .send()
        .await;
    let success = match response {
        Ok(mut response) if response.status().is_success() => {
            // Bounded drain allows connection reuse for small webhook replies.
            // A receiver already accepted the request; response-body errors
            // must not trigger duplicate delivery of an acknowledged event.
            let mut drained = 0usize;
            while let Ok(Some(chunk)) = response.chunk().await {
                drained = drained.saturating_add(chunk.len());
                if drained > 4096 {
                    break;
                }
            }
            true
        }
        _ => false,
    };
    let next_cursor = if success {
        event.seq
    } else if index > 0 {
        page.events[index - 1].seq
    } else {
        entry.cursor
    };
    Some(DeliveryUpdate {
        id: entry.id,
        expected_cursor: entry.cursor,
        next_cursor,
        missed: page.missed,
        skipped: index as u64,
        success,
        delivered: success,
    })
}

impl SubscriptionStore {
    async fn commit_delivery_batch(&self, updates: Vec<DeliveryUpdate>) -> Result<(), Failure> {
        if updates.is_empty() {
            return Ok(());
        }
        let delivered = updates.iter().filter(|u| u.delivered).count() as u64;
        let skipped = updates.iter().map(|u| u.skipped).sum::<u64>();
        self.mutate_async(true, move |state| {
            for update in updates {
                if let Some(current) = state.entries.get_mut(&update.id) {
                    if current.cursor != update.expected_cursor {
                        continue;
                    }
                    current.cursor = update.next_cursor;
                    current.missed = current.missed.saturating_add(update.missed);
                    if update.success {
                        current.failures = 0;
                        current.retry_at = 0;
                    } else {
                        current.failures = current.failures.saturating_add(1);
                        current.retry_at = auth::now().saturating_add(DELIVERY_RETRY_INTERVAL);
                    }
                    if current.failures >= RETRIES {
                        state.entries.remove(&update.id);
                    }
                }
            }
            Ok(())
        })
        .await?;
        self.0.delivered.fetch_add(delivered, Ordering::Relaxed);
        self.0.skipped.fetch_add(skipped, Ordering::Relaxed);
        self.0.batches.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

/// One worker, two in-flight HTTPS deliveries, three attempts per event. The
/// caller owns the handle and aborts it during service shutdown.
pub fn spawn(app: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if !app.subscriptions.enabled() {
            return;
        }
        let client = match reqwest::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .pool_max_idle_per_host(2)
            .build()
        {
            Ok(c) => c,
            Err(_) => return,
        };
        loop {
            if !app.subscriptions.enabled() {
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
            let entries = app.subscriptions.list();
            let mut pending = futures::stream::iter(entries)
                .map(|entry| deliver(&app, &client, entry))
                .buffer_unordered(2);
            let mut batch = Vec::with_capacity(LIMIT);
            let mut flush_at = tokio::time::Instant::now() + Duration::from_secs(1);
            loop {
                let next = if batch.is_empty() {
                    Ok(pending.next().await)
                } else {
                    tokio::time::timeout_at(flush_at, pending.next()).await
                };
                match next {
                    Ok(Some(Some(update))) => {
                        if batch.is_empty() {
                            flush_at = tokio::time::Instant::now() + Duration::from_secs(1);
                        }
                        batch.push(update);
                        if batch.len() == LIMIT {
                            let _ = app
                                .subscriptions
                                .commit_delivery_batch(std::mem::take(&mut batch))
                                .await;
                        }
                    }
                    Ok(Some(None)) => {}
                    Ok(None) => break,
                    Err(_) => {
                        let _ = app
                            .subscriptions
                            .commit_delivery_batch(std::mem::take(&mut batch))
                            .await;
                    }
                }
            }
            let _ = app.subscriptions.commit_delivery_batch(batch).await;
            let latest = app.control.event_summaries(None, 1).latest;
            let entries = app.subscriptions.list();
            if latest > 0
                && entries
                    .iter()
                    .any(|entry| entry.cursor != latest && entry.retry_at <= auth::now())
            {
                // Backlogs are drained at bounded delivery concurrency without
                // an unconditional 250ms delay between every event.
                tokio::task::yield_now().await;
                continue;
            }
            let retry = entries
                .iter()
                .filter(|entry| entry.retry_at > auth::now())
                .map(|entry| entry.retry_at.saturating_sub(auth::now()))
                .min()
                .unwrap_or(5)
                .min(5);
            tokio::select! {
                _=app.control.wait_events(latest)=>{},
                _=tokio::time::sleep(Duration::from_secs(retry))=>{},
            }
        }
    })
}

#[cfg(test)]
pub(crate) async fn performance_experiment(legacy: bool, rounds: usize) -> Value {
    let directory = std::env::temp_dir().join(format!(
        "qrab-redfish-checkpoint-performance-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir(&directory).unwrap();
    let store = SubscriptionStore::new(
        vec!["https://events.example/receiver".into()],
        Some(directory.join("subscriptions.json")),
    )
    .unwrap();
    store
        .mutate(|state| {
            state.next_id = LIMIT as u64;
            for id in 1..=LIMIT as u64 {
                state.entries.insert(
                    id,
                    Subscription {
                        id,
                        destination: "https://events.example/receiver".into(),
                        context: String::new(),
                        expires: auth::now() + 3600,
                        registry_prefixes: Vec::new(),
                        resource_types: Vec::new(),
                        severities: Vec::new(),
                        cursor: 0,
                        failures: 0,
                        retry_at: 0,
                        missed: 0,
                    },
                );
            }
            Ok(())
        })
        .unwrap();
    let writes = store.0.writes.load(Ordering::Relaxed);
    let started = std::time::Instant::now();
    for round in 0..rounds {
        if legacy {
            for id in 1..=LIMIT as u64 {
                store
                    .mutate(|state| {
                        state.entries.get_mut(&id).unwrap().cursor = round as u64 + 1;
                        Ok(())
                    })
                    .unwrap();
            }
        } else {
            let updates = (1..=LIMIT as u64)
                .map(|id| DeliveryUpdate {
                    id,
                    expected_cursor: round as u64,
                    next_cursor: round as u64 + 1,
                    missed: 0,
                    skipped: 0,
                    success: true,
                    delivered: true,
                })
                .collect();
            store.commit_delivery_batch(updates).await.unwrap();
        }
    }
    let result = json!({"rounds":rounds,"events":rounds*LIMIT,"durable_writes":store.0.writes.load(Ordering::Relaxed)-writes,"elapsed_us":started.elapsed().as_micros()});
    std::fs::remove_dir_all(directory).unwrap();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry() -> Subscription {
        Subscription {
            id: 1,
            destination: "https://events.example/receiver".into(),
            context: "private".into(),
            expires: auth::now() + 60,
            registry_prefixes: vec![],
            resource_types: vec![],
            severities: vec!["Critical".into()],
            cursor: 0,
            failures: 0,
            retry_at: 0,
            missed: 0,
        }
    }
    #[test]
    fn exact_https_destinations_and_outbound_redaction() {
        assert!(destination("http://events.example/").is_err());
        assert!(destination("https://user:pass@events.example/").is_err());
        assert!(destination("https://events.example/?token=secret").is_err());
        let store = SubscriptionStore::new(vec![entry().destination], None).unwrap();
        assert!(store.enabled());
        assert!(!store.0.allowed.contains("https://events.example/other"));
        let event = VmEvent {
            seq: 1,
            received_unix_ms: 0,
            qemu_time: None,
            event: "BLOCK_IO_ERROR".into(),
            data: json!({"filename":"/private/disk","password":"secret"}),
            truncated: false,
        };
        assert!(entry().accepts(&event));
        let body = String::from_utf8(outbound(&event, "context")).unwrap();
        assert!(body.contains("BLOCK_IO_ERROR"));
        assert!(!body.contains("secret") && !body.contains("/private"));
        assert!(!entry().json(false).to_string().contains("events.example"));
        assert!(!entry().json(false).to_string().contains("private"));
    }
    #[test]
    fn persistence_is_private_bounded_and_rechecks_allowlist() {
        let dir = std::env::temp_dir().join(format!("qrab-subscriptions-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("subscriptions.json");
        let store = SubscriptionStore::new(vec![entry().destination], Some(path.clone())).unwrap();
        store
            .mutate(|s| {
                s.next_id = 1;
                s.entries.insert(1, entry());
                Ok(())
            })
            .unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let loaded = SubscriptionStore::new(vec![entry().destination], Some(path.clone())).unwrap();
        assert_eq!(loaded.list().len(), 1);
        let revoked =
            SubscriptionStore::new(vec!["https://other.example/".into()], Some(path.clone()))
                .unwrap();
        assert!(revoked.list().is_empty());
        let restored =
            SubscriptionStore::new(vec![entry().destination], Some(path.clone())).unwrap();
        assert!(restored.list().is_empty());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(SubscriptionStore::new(vec![entry().destination], Some(path)).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn lost_persistence_pauses_delivery_instead_of_replaying_without_progress() {
        let dir = std::env::temp_dir().join(format!(
            "qrab-subscriptions-unavailable-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let store = SubscriptionStore::new(vec![entry().destination], Some(dir.join("state.json")))
            .unwrap();
        std::fs::remove_dir(&dir).unwrap();
        assert!(store
            .mutate(|s| {
                s.next_id = 1;
                s.entries.insert(1, entry());
                Ok(())
            })
            .is_err());
        assert!(!store.enabled());
        assert!(store.list().is_empty());
    }
    #[tokio::test]
    async fn a_delivery_batch_has_one_durable_checkpoint_and_noop_callbacks_do_not_write() {
        let dir =
            std::env::temp_dir().join(format!("qrab-subscriptions-batch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let store = SubscriptionStore::new(vec![entry().destination], Some(dir.join("state.json")))
            .unwrap();
        store
            .mutate(|state| {
                state.next_id = LIMIT as u64;
                for id in 1..=LIMIT as u64 {
                    state.entries.insert(id, Subscription { id, ..entry() });
                }
                Ok(())
            })
            .unwrap();
        let updates = (1..=LIMIT as u64)
            .map(|id| DeliveryUpdate {
                id,
                expected_cursor: 0,
                next_cursor: 1,
                missed: 0,
                skipped: 0,
                success: true,
                delivered: true,
            })
            .collect();
        store.commit_delivery_batch(updates).await.unwrap();
        assert_eq!(
            store.0.writes.load(Ordering::Relaxed),
            2,
            "one setup write plus one32-event checkpoint"
        );
        assert!(store.list().iter().all(|entry| entry.cursor == 1));
        store.mutate(|_| Ok(())).unwrap();
        assert_eq!(store.0.writes.load(Ordering::Relaxed), 2);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn persistent_writer_admission_is_bounded_without_blocking_readers() {
        let dir = std::env::temp_dir().join(format!(
            "qrab-subscriptions-admission-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let store = SubscriptionStore::new(vec![entry().destination], Some(dir.join("state.json")))
            .unwrap();
        let permit = store.0.admission.clone().acquire_owned().await.unwrap();
        let result = store.mutate_async(false, |_| Ok(())).await;
        assert!(
            result.is_err(),
            "a busy HTTP writer must be rejected rather than queued"
        );
        assert!(
            store.list().is_empty(),
            "readers remain independent of persistence admission"
        );
        drop(permit);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

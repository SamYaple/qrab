//! Bounded native numeric telemetry triggers. Only logging is advertised;
//! thresholds never imply delivery of a webhook or a generated metric report.
use super::*;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Mutex,
};

pub(super) const COLLECTION: &str = "/redfish/v1/TelemetryService/Triggers";
pub(super) const LOG_SERVICE: &str = "/redfish/v1/Managers/1/LogServices/Telemetry";
pub(super) const ENTRIES: &str = "/redfish/v1/Managers/1/LogServices/Telemetry/Entries";
const MAX_TRIGGERS: usize = 16;
const MAX_ALERTS: usize = 128;

#[derive(Clone, Default)]
pub struct TriggerStore(Arc<Mutex<Registry>>);
#[derive(Default)]
struct Registry {
    next: u64,
    triggers: BTreeMap<u64, Trigger>,
    alert_sequence: u64,
    alerts: VecDeque<Alert>,
}
#[derive(Clone)]
struct Trigger {
    id: u64,
    name: String,
    metric: Metric,
    thresholds: Vec<Threshold>,
    enabled: bool,
    previous: Option<(u64, u64)>,
}
#[derive(Clone, Copy)]
struct Threshold {
    name: &'static str,
    reading: u64,
    increasing: bool,
    critical: bool,
}
#[derive(Clone, Copy)]
enum Metric {
    Cpus,
    Memory,
    ReadBytes,
    WriteBytes,
    ReadOps,
    WriteOps,
    FlushOps,
    ReadTime,
    WriteTime,
}
#[derive(Clone)]
struct Alert {
    sequence: u64,
    timestamp: u64,
    trigger_id: u64,
    metric: Metric,
    threshold: Threshold,
    value: u64,
}

impl Metric {
    fn name(self) -> &'static str {
        match self {
            Self::Cpus => "LogicalProcessors",
            Self::Memory => "MemoryBytes",
            Self::ReadBytes => "BlockReadBytes",
            Self::WriteBytes => "BlockWriteBytes",
            Self::ReadOps => "BlockReadOperations",
            Self::WriteOps => "BlockWriteOperations",
            Self::FlushOps => "BlockFlushOperations",
            Self::ReadTime => "BlockReadTimeNanoseconds",
            Self::WriteTime => "BlockWriteTimeNanoseconds",
        }
    }
    fn parse(property: &str) -> Option<Self> {
        [
            Self::Cpus,
            Self::Memory,
            Self::ReadBytes,
            Self::WriteBytes,
            Self::ReadOps,
            Self::WriteOps,
            Self::FlushOps,
            Self::ReadTime,
            Self::WriteTime,
        ]
        .into_iter()
        .find(|m| m.property() == property)
    }
    fn property(self) -> String {
        format!("{}#/{}", super::telemetry::METRICS, self.name())
    }
    fn counter(self) -> bool {
        !matches!(self, Self::Cpus | Self::Memory)
    }
    fn reading(self, s: &pod::VmMetrics) -> u64 {
        match self {
            Self::Cpus => s.logical_processors,
            Self::Memory => s.memory_bytes,
            Self::ReadBytes => s.block_read_bytes,
            Self::WriteBytes => s.block_write_bytes,
            Self::ReadOps => s.block_read_operations,
            Self::WriteOps => s.block_write_operations,
            Self::FlushOps => s.block_flush_operations,
            Self::ReadTime => s.block_read_time_ns,
            Self::WriteTime => s.block_write_time_ns,
        }
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
    Failure::new(StatusCode::BAD_REQUEST,"PropertyValueNotInList",message,"Use one canonical VM MetricProperty, unsigned integer numeric thresholds, Increasing/Decreasing activation, zero dwell/hysteresis and LogToLogService only.")
}

impl Trigger {
    fn json(&self) -> Value {
        let mut thresholds = serde_json::Map::new();
        for threshold in &self.thresholds {
            thresholds.insert(threshold.name.into(),json!({"Reading":threshold.reading,
            "Activation":if threshold.increasing{"Increasing"}else{"Decreasing"},"DwellTime":"PT0S"}));
        }
        json!({
            "@odata.id":uri(self.id),"@odata.type":"#Triggers.v1_4_0.Triggers","Id":self.id.to_string(),"Name":self.name,
            "MetricType":"Numeric","MetricProperties":[self.metric.property()],"MetricIds":[self.metric.name()],
            "NumericThresholds":thresholds,"TriggerActions":["LogToLogService"],"TriggerEnabled":self.enabled,
            "HysteresisDuration":"PT0S","HysteresisReading":0,"Status":{"State":if self.enabled{"Enabled"}else{"Disabled"}},
            "Oem":{"Qrab":{"@odata.type":"#QrabTriggers.v1_0_0.QrabTriggers","Persistent":false,"BaselineRequired":true}}
        })
    }

    fn parse(body: &Value, id: u64) -> Result<Self, Failure> {
        let object = body
            .as_object()
            .ok_or_else(|| invalid("A trigger must be a JSON object."))?;
        if object.keys().any(|k| {
            !matches!(
                k.as_str(),
                "Name"
                    | "MetricType"
                    | "MetricProperties"
                    | "MetricIds"
                    | "NumericThresholds"
                    | "TriggerActions"
                    | "TriggerEnabled"
                    | "HysteresisDuration"
                    | "HysteresisReading"
            )
        }) {
            return Err(invalid("Unsupported trigger property."));
        }
        if body.get("MetricType").is_some_and(|v| v != "Numeric")
            || body.get("HysteresisDuration").is_some_and(|v| v != "PT0S")
            || body
                .get("HysteresisReading")
                .is_some_and(|v| v.as_u64() != Some(0))
        {
            return Err(invalid(
                "Only numeric triggers with zero hysteresis are supported.",
            ));
        }
        let properties = body["MetricProperties"]
            .as_array()
            .filter(|v| v.len() == 1)
            .ok_or_else(|| invalid("Exactly one canonical MetricProperty is required."))?;
        let metric = properties[0]
            .as_str()
            .and_then(Metric::parse)
            .ok_or_else(|| invalid("The metric property is not supported."))?;
        if body
            .get("MetricIds")
            .is_some_and(|v| v != &json!([metric.name()]))
            || body
                .get("TriggerActions")
                .is_some_and(|v| v != &json!(["LogToLogService"]))
        {
            return Err(invalid(
                "Only the selected metric and LogToLogService action are supported.",
            ));
        }
        let name = body
            .get("Name")
            .map(|v| v.as_str().ok_or_else(|| invalid("Name must be a string.")))
            .transpose()?
            .unwrap_or("VM numeric threshold");
        if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
            return Err(invalid(
                "Name exceeds its bound or contains control characters.",
            ));
        }
        let enabled = body
            .get("TriggerEnabled")
            .map(|v| {
                v.as_bool()
                    .ok_or_else(|| invalid("TriggerEnabled must be a boolean."))
            })
            .transpose()?
            .unwrap_or(true);
        let numeric = body["NumericThresholds"]
            .as_object()
            .filter(|v| !v.is_empty() && v.len() <= 4)
            .ok_or_else(|| invalid("One to four numeric thresholds are required."))?;
        let mut thresholds = Vec::new();
        for (name, value) in numeric {
            let (name, critical, default_increasing) = match name.as_str() {
                "LowerCritical" => ("LowerCritical", true, false),
                "LowerWarning" => ("LowerWarning", false, false),
                "UpperWarning" => ("UpperWarning", false, true),
                "UpperCritical" => ("UpperCritical", true, true),
                _ => return Err(invalid("Unsupported numeric threshold name.")),
            };
            let object = value
                .as_object()
                .ok_or_else(|| invalid("A threshold must be an object."))?;
            if object
                .keys()
                .any(|k| !matches!(k.as_str(), "Reading" | "Activation" | "DwellTime"))
                || value.get("DwellTime").is_some_and(|v| v != "PT0S")
            {
                return Err(invalid("Only zero-dwell numeric thresholds are supported."));
            }
            let reading = value["Reading"]
                .as_u64()
                .ok_or_else(|| invalid("Threshold readings must be unsigned 64-bit integers."))?;
            let increasing = match value.get("Activation").and_then(Value::as_str) {
                None if !object.contains_key("Activation") => default_increasing,
                Some("Increasing") => true,
                Some("Decreasing") => false,
                _ => {
                    return Err(invalid(
                        "Only Increasing or Decreasing activation is supported.",
                    ))
                }
            };
            thresholds.push(Threshold {
                name,
                reading,
                increasing,
                critical,
            });
        }
        Ok(Self {
            id,
            name: name.into(),
            metric,
            thresholds,
            enabled,
            previous: None,
        })
    }
}

impl Alert {
    fn json(&self) -> Value {
        json!({
            "@odata.id":format!("{ENTRIES}/{}",self.sequence),"@odata.type":"#LogEntry.v1_17_0.LogEntry",
            "Id":self.sequence.to_string(),"Name":"Numeric telemetry threshold crossed","EntryType":"Oem",
            "OemRecordFormat":"QrabThreshold.v1_0_0","Created":rfc3339(self.timestamp),
            "Severity":if self.threshold.critical{"Critical"}else{"Warning"},"Message":"A numeric telemetry threshold was crossed.",
            "Links":{"OriginOfCondition":link(&uri(self.trigger_id))},
            "Oem":{"Qrab":{"@odata.type":"#QrabThreshold.v1_0_0.QrabThreshold","MetricProperty":self.metric.property(),
                "Threshold":self.threshold.name,"Reading":self.value,"ThresholdReading":self.threshold.reading,
                "Activation":if self.threshold.increasing{"Increasing"}else{"Decreasing"}}}
        })
    }
}

impl TriggerStore {
    /// Bounded inspection used to avoid idle telemetry control requests.
    pub fn has_enabled(&self) -> bool {
        self.0
            .lock()
            .unwrap()
            .triggers
            .values()
            .any(|trigger| trigger.enabled)
    }

    /// The first observation establishes a baseline. Duplicate/stale samples
    /// are ignored and a decreasing cumulative counter establishes a new epoch
    /// without activating a threshold on the reset itself.
    pub fn evaluate(&self, sample: &pod::VmMetrics) {
        let mut registry = self.0.lock().unwrap();
        let mut alerts = Vec::new();
        for trigger in registry.triggers.values_mut() {
            if !trigger.enabled {
                continue;
            }
            let value = trigger.metric.reading(sample);
            let Some((at, previous)) = trigger.previous else {
                trigger.previous = Some((sample.observed_unix_ms, value));
                continue;
            };
            if sample.observed_unix_ms <= at {
                continue;
            }
            trigger.previous = Some((sample.observed_unix_ms, value));
            if trigger.metric.counter() && value < previous {
                continue;
            }
            for threshold in &trigger.thresholds {
                let crossed = if threshold.increasing {
                    previous <= threshold.reading && value > threshold.reading
                } else {
                    previous >= threshold.reading && value < threshold.reading
                };
                if crossed {
                    alerts.push(Alert {
                        sequence: 0,
                        timestamp: sample.observed_unix_ms,
                        trigger_id: trigger.id,
                        metric: trigger.metric,
                        threshold: *threshold,
                        value,
                    });
                }
            }
        }
        for mut alert in alerts {
            let Some(sequence) = registry.alert_sequence.checked_add(1) else {
                break;
            };
            registry.alert_sequence = sequence;
            alert.sequence = sequence;
            if registry.alerts.len() == MAX_ALERTS {
                registry.alerts.pop_front();
            }
            registry.alerts.push_back(alert);
        }
    }
    fn insert(&self, body: &Value) -> Result<Trigger, Failure> {
        let mut registry = self.0.lock().unwrap();
        if registry.triggers.len() >= MAX_TRIGGERS {
            return Err(Failure::unavailable(
                "The telemetry trigger limit has been reached.",
            ));
        }
        let next = registry
            .next
            .checked_add(1)
            .ok_or_else(|| Failure::unavailable("The trigger ID range is exhausted."))?;
        let trigger = Trigger::parse(body, next)?;
        registry.next = next;
        registry.triggers.insert(next, trigger.clone());
        Ok(trigger)
    }
    pub fn alerts(&self) -> Vec<Value> {
        self.0
            .lock()
            .unwrap()
            .alerts
            .iter()
            .map(Alert::json)
            .collect()
    }
}

pub(super) async fn collection_handler(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let members = app
        .triggers
        .0
        .lock()
        .unwrap()
        .triggers
        .keys()
        .map(|id| link(&uri(*id)))
        .collect();
    Ok(Json(collection(
        COLLECTION,
        "TriggersCollection",
        "VM numeric triggers",
        members,
    )))
}
pub(super) async fn create(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Failure> {
    admin(&app, &headers).await?;
    let trigger = app.triggers.insert(&action_body(&body)?)?;
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, uri(trigger.id))],
        Json(trigger.json()),
    )
        .into_response())
}
pub(super) async fn get(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(value): Path<String>,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let registry = app.triggers.0.lock().unwrap();
    Ok(Json(
        registry
            .triggers
            .get(&id(&value)?)
            .ok_or_else(Failure::not_found)?
            .json(),
    ))
}
pub(super) async fn patch(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(value): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, Failure> {
    admin(&app, &headers).await?;
    let body = action_body(&body)?;
    let id = id(&value)?;
    let mut registry = app.triggers.0.lock().unwrap();
    let previous = registry.triggers.get(&id).ok_or_else(Failure::not_found)?;
    let old = previous.json();
    let mut configuration = serde_json::Map::new();
    for name in [
        "Name",
        "MetricType",
        "MetricProperties",
        "MetricIds",
        "NumericThresholds",
        "TriggerActions",
        "TriggerEnabled",
        "HysteresisDuration",
        "HysteresisReading",
    ] {
        configuration.insert(name.into(), old[name].clone());
    }
    // NumericThresholds is replaced as a unit; changing a threshold deliberately
    // establishes a fresh baseline rather than firing on configuration changes.
    for (key, value) in body.as_object().unwrap() {
        configuration.insert(key.clone(), value.clone());
    }
    let trigger = Trigger::parse(&Value::Object(configuration), id)?;
    let json = trigger.json();
    registry.triggers.insert(id, trigger);
    Ok(Json(json))
}
pub(super) async fn delete(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(value): Path<String>,
) -> Result<StatusCode, Failure> {
    admin(&app, &headers).await?;
    app.triggers
        .0
        .lock()
        .unwrap()
        .triggers
        .remove(&id(&value)?)
        .ok_or_else(Failure::not_found)?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn log_service(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    Ok(Json(
        json!({"@odata.id":LOG_SERVICE,"@odata.type":"#LogService.v1_7_0.LogService","Id":"Telemetry","Name":"Telemetry threshold crossings",
        "ServiceEnabled":true,"LogEntryType":"OEM","LogPurposes":["Telemetry"],"MaxNumberOfRecords":MAX_ALERTS,"OverWritePolicy":"WrapsWhenFull","Entries":link(ENTRIES),
        "Oem":{"Qrab":{"@odata.type":"#QrabThresholdLog.v1_0_0.QrabThresholdLog","Persistent":false,"MaxTriggers":MAX_TRIGGERS,"PushDelivery":false}}}),
    ))
}
pub(super) async fn entries(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let entries = app.triggers.alerts();
    Ok(Json(collection(
        ENTRIES,
        "LogEntryCollection",
        "Telemetry threshold crossings",
        entries,
    )))
}
pub(super) async fn entry(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(value): Path<String>,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let id = id(&value)?;
    app.triggers
        .0
        .lock()
        .unwrap()
        .alerts
        .iter()
        .find(|a| a.sequence == id)
        .map(|a| Json(a.json()))
        .ok_or_else(Failure::not_found)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spec(metric: &str, reading: u64, activation: &str) -> Value {
        json!({"MetricProperties":[format!("{}#/{metric}",super::super::telemetry::METRICS)],"NumericThresholds":{"UpperCritical":{"Reading":reading,"Activation":activation,"DwellTime":"PT0S"}}})
    }
    #[test]
    fn exact_u64_crossings_ignore_duplicates_and_counter_resets() {
        let store = TriggerStore::default();
        let threshold = 9_007_199_254_740_993;
        store
            .insert(&spec("BlockReadBytes", threshold, "Increasing"))
            .unwrap();
        let mut sample = pod::VmMetrics {
            observed_unix_ms: 1,
            block_read_bytes: threshold - 1,
            ..Default::default()
        };
        store.evaluate(&sample);
        sample.observed_unix_ms = 2;
        sample.block_read_bytes = threshold;
        store.evaluate(&sample);
        assert!(store.alerts().is_empty());
        sample.observed_unix_ms = 3;
        sample.block_read_bytes = threshold + 1;
        store.evaluate(&sample);
        store.evaluate(&sample);
        assert_eq!(store.alerts().len(), 1);
        assert_eq!(
            store.alerts()[0]["Oem"]["Qrab"]["ThresholdReading"],
            threshold
        );
        sample.observed_unix_ms = 4;
        sample.block_read_bytes = 0;
        store.evaluate(&sample);
        assert_eq!(store.alerts().len(), 1);
        let decreasing = TriggerStore::default();
        decreasing
            .insert(&spec("BlockReadBytes", 100, "Decreasing"))
            .unwrap();
        sample.observed_unix_ms = 1;
        sample.block_read_bytes = 101;
        decreasing.evaluate(&sample);
        sample.observed_unix_ms = 2;
        sample.block_read_bytes = 0;
        decreasing.evaluate(&sample);
        assert!(decreasing.alerts().is_empty());
    }
    #[test]
    fn gauges_can_fall_and_alert_storage_and_trigger_counts_are_bounded() {
        let store = TriggerStore::default();
        store
            .insert(&spec("MemoryBytes", 100, "Decreasing"))
            .unwrap();
        let mut sample = pod::VmMetrics::default();
        for index in 0..300 {
            sample.observed_unix_ms = index + 1;
            sample.memory_bytes = if index % 2 == 0 { 101 } else { 99 };
            store.evaluate(&sample);
        }
        assert_eq!(store.alerts().len(), MAX_ALERTS);
        assert_eq!(store.alerts().last().unwrap()["Severity"], "Critical");
        for _ in 1..MAX_TRIGGERS {
            store.insert(&spec("MemoryBytes", 1, "Increasing")).unwrap();
        }
        assert!(store.insert(&spec("MemoryBytes", 1, "Increasing")).is_err());
    }
    #[test]
    fn unsupported_actions_dwell_and_non_integer_thresholds_are_rejected() {
        let mut body = spec("MemoryBytes", 100, "Increasing");
        body["TriggerActions"] = json!(["RedfishEvent"]);
        assert!(Trigger::parse(&body, 1).is_err());
        body = spec("MemoryBytes", 100, "Either");
        assert!(Trigger::parse(&body, 1).is_err());
        body = spec("MemoryBytes", 100, "Increasing");
        body["NumericThresholds"]["UpperCritical"]["DwellTime"] = json!("PT1S");
        assert!(Trigger::parse(&body, 1).is_err());
        body = spec("MemoryBytes", 100, "Increasing");
        body["NumericThresholds"]["UpperCritical"]["Reading"] = json!(1.5);
        assert!(Trigger::parse(&body, 1).is_err());
        body = spec("MemoryBytes", u64::MAX, "Increasing");
        assert!(Trigger::parse(&body, 1).is_ok());
    }
}

//! Compact bounded telemetry history; periodic sampling is explicitly opt-in.
use super::*;

pub(super) const SERVICE: &str = "/redfish/v1/TelemetryService";
const REPORTS: &str = "/redfish/v1/TelemetryService/MetricReports";
pub(super) const REPORT: &str = "/redfish/v1/TelemetryService/MetricReports/VM";
pub(super) const METRICS: &str = "/redfish/v1/Systems/1/Oem/Qrab/Metrics";

pub(super) async fn service(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    Ok(Json(json!({
        "@odata.id":SERVICE,"@odata.type":"#TelemetryService.v1_4_0.TelemetryService",
        "Id":"TelemetryService","Name":"VM telemetry","ServiceEnabled":true,
        "MetricReports":link(REPORTS),"MetricDefinitions":link(DEFINITIONS),"MetricReportDefinitions":link(REPORT_DEFINITIONS),"Triggers":link(triggers::COLLECTION),"LogService":link(triggers::LOG_SERVICE)
    })))
}

pub(super) async fn reports(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    Ok(Json(collection(
        REPORTS,
        "MetricReportCollection",
        "VM metric reports",
        {
            let sequences: Vec<_> = app
                .telemetry
                .0
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .reports
                .iter()
                .map(|(n, _)| *n)
                .collect();
            std::iter::once(link(REPORT))
                .chain(
                    sequences
                        .into_iter()
                        .map(|n| link(&format!("{HISTORY}-{n}"))),
                )
                .collect()
        },
    )))
}

async fn sample(app: &AppState, headers: &HeaderMap) -> Result<pod::VmMetrics, Failure> {
    principal(app, headers).await?;
    let sample = app
        .control
        .metrics()
        .await
        .map_err(|_| Failure::unavailable("Live VM metrics are unavailable."))?;
    observe(app, &sample);
    Ok(sample)
}

/// Every successful on-request sample, including a Prometheus scrape's,
/// feeds the same triggers and bounded history.
pub(super) fn observe(app: &AppState, sample: &pod::VmMetrics) {
    app.triggers.evaluate(sample);
    app.telemetry.record(sample);
}

pub(super) async fn metrics(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    let sample = sample(&app, &headers).await?;
    Ok(Json(json!({
        "@odata.id":METRICS,"@odata.type":"#QrabMetrics.v1_0_0.QrabMetrics",
        "Id":"Metrics","Name":"Live QMP VM metrics","Timestamp":rfc3339(sample.observed_unix_ms),
        "LogicalProcessors":sample.logical_processors,"MemoryBytes":sample.memory_bytes,
        "BlockReadBytes":sample.block_read_bytes,"BlockWriteBytes":sample.block_write_bytes,
        "BlockReadOperations":sample.block_read_operations,"BlockWriteOperations":sample.block_write_operations,
        "BlockFlushOperations":sample.block_flush_operations,
        "BlockReadTimeNanoseconds":sample.block_read_time_ns,"BlockWriteTimeNanoseconds":sample.block_write_time_ns
    })))
}

pub(super) fn report(sample: &pod::VmMetrics) -> Value {
    let timestamp = rfc3339(sample.observed_unix_ms);
    let values: Vec<_> = [
        ("LogicalProcessors", sample.logical_processors),
        ("MemoryBytes", sample.memory_bytes),
        ("BlockReadBytes", sample.block_read_bytes),
        ("BlockWriteBytes", sample.block_write_bytes),
        ("BlockReadOperations", sample.block_read_operations),
        ("BlockWriteOperations", sample.block_write_operations),
        ("BlockFlushOperations", sample.block_flush_operations),
        ("BlockReadTimeNanoseconds", sample.block_read_time_ns),
        ("BlockWriteTimeNanoseconds", sample.block_write_time_ns),
    ]
    .into_iter()
    .map(|(name, value)| {
        json!({
            "MetricId":name,"MetricProperty":format!("{METRICS}#/{name}"),"MetricDefinition":link(&format!("{DEFINITIONS}/{name}")),
            "MetricValue":value.to_string(),"Timestamp":timestamp
        })
    })
    .collect();
    json!({
        "@odata.id":REPORT,"@odata.type":"#MetricReport.v1_5_2.MetricReport",
        "Id":"VM","Name":"Live QMP VM report","Timestamp":timestamp,"MetricValues":values,
        "MetricReportDefinition":link(&format!("{REPORT_DEFINITIONS}/VM"))
    })
}

pub(super) async fn vm_report(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    Ok(Json(report(&sample(&app, &headers).await?)))
}

pub(super) const DEFINITIONS: &str = "/redfish/v1/TelemetryService/MetricDefinitions";
pub(super) const REPORT_DEFINITIONS: &str = "/redfish/v1/TelemetryService/MetricReportDefinitions";
const HISTORY: &str = "/redfish/v1/TelemetryService/MetricReports/History";
const HISTORY_CAPACITY: usize = 128;

#[derive(Clone, Default)]
pub struct TelemetryStore(Arc<std::sync::Mutex<TelemetryHistory>>, Option<u64>);
#[derive(Default)]
struct TelemetryHistory {
    sequence: u64,
    ignored: u64,
    background_requests: u64,
    reports: std::collections::VecDeque<(u64, pod::VmMetrics)>,
}

impl TelemetryStore {
    pub fn from_env() -> Result<Self, String> {
        match std::env::var("REDFISH_TELEMETRY_INTERVAL_SECS") {
            Ok(value) => Self::with_interval_seconds(Some(value.parse().map_err(|_| {
                "REDFISH_TELEMETRY_INTERVAL_SECS must be an integer from 5 to 3600"
            })?)),
            Err(std::env::VarError::NotPresent) => Ok(Self::default()),
            Err(_) => Err("REDFISH_TELEMETRY_INTERVAL_SECS must be valid UTF-8".into()),
        }
    }

    pub fn with_interval_seconds(interval: Option<u64>) -> Result<Self, String> {
        if interval.is_some_and(|n| !(5..=3600).contains(&n)) {
            return Err("REDFISH_TELEMETRY_INTERVAL_SECS must be from 5 to 3600".into());
        }
        Ok(Self(Arc::default(), interval))
    }

    pub(super) fn record(&self, sample: &pod::VmMetrics) {
        let mut history = self.0.lock().unwrap_or_else(|p| p.into_inner());
        // Duplicate/stale cached samples are identified before allocating or
        // formatting a JSON report. Keep only compact authoritative values.
        if history
            .reports
            .back()
            .is_some_and(|(_, previous)| previous.observed_unix_ms >= sample.observed_unix_ms)
        {
            history.ignored = history.ignored.saturating_add(1);
            return;
        }
        let Some(sequence) = history.sequence.checked_add(1) else {
            return;
        };
        history.sequence = sequence;
        if history.reports.len() == HISTORY_CAPACITY {
            history.reports.pop_front();
        }
        history.reports.push_back((sequence, sample.clone()));
    }

    pub(super) fn stats(&self) -> Value {
        let history = self.0.lock().unwrap_or_else(|p| p.into_inner());
        json!({"RetainedSamples":history.reports.len(),"HistoryCapacity":HISTORY_CAPACITY,
            "RetainedSampleBytes":history.reports.len()*std::mem::size_of::<(u64,pod::VmMetrics)>(),
            "RecordedSamples":history.sequence,"IgnoredDuplicateOrStaleSamples":history.ignored,
            "BackgroundMetricRequests":history.background_requests,"PeriodicIntervalSeconds":self.1})
    }
}

fn history_report(sequence: u64, sample: &pod::VmMetrics) -> Value {
    let mut value = report(sample);
    value["@odata.id"] = json!(format!("{HISTORY}-{sequence}"));
    value["Id"] = json!(format!("History-{sequence}"));
    value["ReportSequence"] = json!(sequence.to_string());
    value
}

fn metric_specs() -> [(&'static str, &'static str, &'static str); 9] {
    [
        ("LogicalProcessors", "{CPU}", "Gauge"),
        ("MemoryBytes", "By", "Gauge"),
        ("BlockReadBytes", "By", "Counter"),
        ("BlockWriteBytes", "By", "Counter"),
        ("BlockReadOperations", "{operation}", "Counter"),
        ("BlockWriteOperations", "{operation}", "Counter"),
        ("BlockFlushOperations", "{operation}", "Counter"),
        ("BlockReadTimeNanoseconds", "ns", "Counter"),
        ("BlockWriteTimeNanoseconds", "ns", "Counter"),
    ]
}

pub(super) async fn definitions(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    Ok(Json(collection(
        DEFINITIONS,
        "MetricDefinitionCollection",
        "VM metric definitions",
        metric_specs()
            .iter()
            .map(|(name, _, _)| link(&format!("{DEFINITIONS}/{name}")))
            .collect(),
    )))
}

pub(super) async fn definition(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let (_, units, kind) = metric_specs()
        .into_iter()
        .find(|(name, _, _)| *name == id)
        .ok_or_else(Failure::not_found)?;
    Ok(Json(
        json!({"@odata.id":format!("{DEFINITIONS}/{id}"),"@odata.type":"#MetricDefinition.v1_3_5.MetricDefinition",
        "Id":id,"Name":id,"MetricDataType":"Integer","MetricType":kind,"Implementation":"DigitalMeter","Units":units,
        "MetricProperties":[format!("{METRICS}#/{id}")]}),
    ))
}

pub(super) async fn report_definitions(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    Ok(Json(collection(
        REPORT_DEFINITIONS,
        "MetricReportDefinitionCollection",
        "VM report definitions",
        vec![link(&format!("{REPORT_DEFINITIONS}/VM"))],
    )))
}

pub(super) async fn report_definition(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let mut definition = json!({"@odata.id":format!("{REPORT_DEFINITIONS}/VM"),"@odata.type":"#MetricReportDefinition.v1_4_4.MetricReportDefinition",
        "Id":"VM","Name":"VM metric report","MetricReportDefinitionEnabled":true,
        "MetricReportDefinitionType":if app.telemetry.1.is_some(){"Periodic"}else{"OnRequest"},
        "ReportUpdates":"Overwrite","ReportActions":[],"MetricReport":link(REPORT),
        "Metrics":metric_specs().iter().map(|(name,_,_)|json!({"MetricId":name,"MetricProperties":[format!("{METRICS}#/{name}")]})).collect::<Vec<_>>()});
    if let Some(seconds) = app.telemetry.1 {
        definition["Schedule"] = json!({"RecurrenceInterval":format!("PT{seconds}S")});
    }
    Ok(Json(definition))
}

pub(super) async fn historical_report(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let sequence = id
        .strip_prefix("History-")
        .and_then(|s| s.parse::<u64>().ok())
        .ok_or_else(Failure::not_found)?;
    let sample = app
        .telemetry
        .0
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .reports
        .iter()
        .find(|(n, _)| *n == sequence)
        .map(|(_, sample)| sample.clone())
        .ok_or_else(Failure::not_found)?;
    // JSON formatting happens after releasing the history mutex.
    Ok(Json(history_report(sequence, &sample)))
}

async fn sampler_tick(
    control: &pod::Backend,
    telemetry: &TelemetryStore,
    triggers: &triggers::TriggerStore,
    periodic: bool,
) {
    if !periodic && !triggers.has_enabled() {
        return;
    }
    {
        let mut history = telemetry.0.lock().unwrap_or_else(|p| p.into_inner());
        history.background_requests = history.background_requests.saturating_add(1);
    }
    if let Ok(sample) = control.metrics().await {
        triggers.evaluate(&sample);
        if periodic {
            telemetry.record(&sample);
        }
    }
}

/// Idle OnRequest services issue no metric requests. Enabled triggers still
/// sample every 5 seconds; explicit periodic history uses its declared cadence.
pub fn spawn_sampler(app: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let period = app.telemetry.1.map(Duration::from_secs);
        let mut next_periodic = tokio::time::Instant::now();
        let mut next_trigger_check = next_periodic;
        loop {
            let now = tokio::time::Instant::now();
            let periodic = period.is_some() && now >= next_periodic;
            let trigger_check = now >= next_trigger_check;
            if periodic || trigger_check {
                sampler_tick(&app.control, &app.telemetry, &app.triggers, periodic).await;
            }
            let after = tokio::time::Instant::now();
            if periodic {
                let period = period.unwrap();
                next_periodic += period;
                if next_periodic <= after {
                    next_periodic = after + period;
                }
            }
            if trigger_check {
                next_trigger_check += Duration::from_secs(5);
                if next_trigger_check <= after {
                    next_trigger_check = after + Duration::from_secs(5);
                }
            }
            let next = if period.is_some() {
                next_periodic.min(next_trigger_check)
            } else {
                next_trigger_check
            };
            tokio::time::sleep_until(next).await;
        }
    })
}

pub(super) async fn resources(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let local = pod::ResourceUsage::sample();
    let manager = app.control.resource_usage().await.ok();
    Ok(Json(
        json!({"@odata.id":"/redfish/v1/Managers/1/Oem/Qrab/Resources","@odata.type":"#QrabServiceResources.v1_0_0.QrabServiceResources",
        "Id":"Resources","Name":"Service resource accounting","Redfish":local,"Manager":manager,
        "Performance":{"Telemetry":app.telemetry.stats(),"Subscriptions":app.subscriptions.stats(),"Tasks":app.tasks.stats()}}),
    ))
}

#[cfg(test)]
#[path = "../../../contrib/experiments/redfish-performance.rs"]
mod performance;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_preserves_integer_counters_and_sample_timestamp() {
        let sample = pod::VmMetrics {
            observed_unix_ms: 1_700_000_000_123,
            memory_bytes: 1_073_741_824,
            block_read_bytes: 9_007_199_254_740_993,
            ..Default::default()
        };
        let report = report(&sample);
        assert_eq!(report["MetricValues"].as_array().unwrap().len(), 9);
        let values = report["MetricValues"].as_array().unwrap();
        let read = values
            .iter()
            .find(|v| v["MetricId"] == "BlockReadBytes")
            .unwrap();
        assert_eq!(read["MetricValue"], "9007199254740993");
        assert_eq!(read["MetricProperty"], format!("{METRICS}#/BlockReadBytes"));
        assert_eq!(read["Timestamp"], report["Timestamp"]);
    }
    #[test]
    fn history_is_compact_bounded_and_rejects_duplicate_or_stale_samples() {
        let store = TelemetryStore::default();
        for timestamp in 1..=200 {
            store.record(&pod::VmMetrics {
                observed_unix_ms: timestamp,
                logical_processors: 2,
                ..Default::default()
            });
        }
        store.record(&pod::VmMetrics {
            observed_unix_ms: 200,
            ..Default::default()
        });
        store.record(&pod::VmMetrics {
            observed_unix_ms: 199,
            ..Default::default()
        });
        let history = store.0.lock().unwrap();
        assert_eq!(history.reports.len(), HISTORY_CAPACITY);
        assert_eq!(history.sequence, 200);
        assert_eq!(history.ignored, 2);
        let (sequence, sample) = history.reports.back().unwrap();
        let value = history_report(*sequence, sample);
        assert_eq!(value["Id"], "History-200");
        assert_eq!(value["MetricValues"][0]["MetricValue"], "2");
        assert!(std::mem::size_of::<(u64, pod::VmMetrics)>() * HISTORY_CAPACITY < 16 * 1024);
    }

    #[tokio::test]
    async fn idle_on_request_sampling_issues_no_control_requests_or_fake_history() {
        let (control, receiver) = pod::control_channel();
        drop(receiver);
        let backend = pod::Backend::Local(control);
        let store = TelemetryStore::default();
        let triggers = triggers::TriggerStore::default();
        sampler_tick(&backend, &store, &triggers, false).await;
        assert_eq!(store.stats()["BackgroundMetricRequests"], 0);
        assert_eq!(store.stats()["RetainedSamples"], 0);
        sampler_tick(&backend, &store, &triggers, true).await;
        assert_eq!(store.stats()["BackgroundMetricRequests"], 1);
        assert_eq!(store.stats()["RetainedSamples"], 0);
        for seconds in [0, 4, 3601] {
            assert!(TelemetryStore::with_interval_seconds(Some(seconds)).is_err());
        }
        assert!(TelemetryStore::with_interval_seconds(Some(7)).is_ok());
    }
}

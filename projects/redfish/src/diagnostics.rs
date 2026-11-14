//! Bounded diagnostic bundles. Event payloads and free-form manager errors are
//! deliberately excluded because they can contain storage paths or endpoints.
use super::tasks::{Outcome, Record, ATTACHMENT_LIMIT};
use super::*;

pub(super) const SERVICE: &str = "/redfish/v1/Managers/1/LogServices/Diagnostics";
pub(super) const ENTRIES: &str = "/redfish/v1/Managers/1/LogServices/Diagnostics/Entries";
pub(super) const COLLECT: &str =
    "/redfish/v1/Managers/1/LogServices/Diagnostics/Actions/LogService.CollectDiagnosticData";

pub(super) fn entry_uri(id: u64) -> String {
    format!("{ENTRIES}/{id}")
}
fn attachment_uri(id: u64) -> String {
    format!("{ENTRIES}/{id}/attachment")
}

pub(super) fn entry_json(record: &Record) -> Value {
    let size = match &record.outcome {
        Some(Outcome::Diagnostic { bytes }) => bytes.len(),
        _ => 0,
    };
    json!({
        "@odata.id":entry_uri(record.id),"@odata.type":"#LogEntry.v1_17_0.LogEntry",
        "Id":record.id.to_string(),"Name":"VM diagnostic bundle","EntryType":"Oem",
        "OemRecordFormat":"QrabDiagnostic.v1_0_0","Severity":"OK",
        "Message":"Redacted VM diagnostics collected.","DiagnosticDataType":"Manager",
        "AdditionalDataURI":attachment_uri(record.id),"AdditionalDataSizeBytes":size,
        "Links":{"OriginOfCondition":link(MANAGER)}
    })
}

pub(super) async fn service(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    Ok(Json(json!({
        "@odata.id":SERVICE,"@odata.type":"#LogService.v1_7_0.LogService",
        "Id":"Diagnostics","Name":"VM diagnostic bundles","ServiceEnabled":true,
        "LogEntryType":"OEM","LogPurposes":["Diagnostic"],"MaxNumberOfRecords":super::tasks::CAPACITY,
        "OverWritePolicy":"WrapsWhenFull","Entries":link(ENTRIES),
        "Actions":{"#LogService.CollectDiagnosticData":{
            "target":COLLECT,"DiagnosticDataType@Redfish.AllowableValues":["Manager"]
        }}
    })))
}

pub(super) async fn entries(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    Ok(Json(collection(
        ENTRIES,
        "LogEntryCollection",
        "VM diagnostic entries",
        app.tasks
            .diagnostics()
            .iter()
            .map(|r| link(&entry_uri(r.id)))
            .collect(),
    )))
}

fn diagnostic(app: &AppState, id: &str) -> Result<Record, Failure> {
    let parsed = id
        .parse::<u64>()
        .ok()
        .filter(|n| n.to_string() == id)
        .ok_or_else(Failure::not_found)?;
    let record = app.tasks.get(parsed).ok_or_else(Failure::not_found)?;
    if !matches!(record.outcome, Some(Outcome::Diagnostic { .. })) {
        return Err(Failure::not_found());
    }
    Ok(record)
}

pub(super) async fn entry(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    Ok(Json(entry_json(&diagnostic(&app, &id)?)))
}

pub(super) async fn attachment(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, Failure> {
    // Bundles are redacted anyway; restrict downloads to operators so additions
    // to the format do not silently broaden the viewer contract.
    admin(&app, &headers).await?;
    let record = diagnostic(&app, &id)?;
    let Some(Outcome::Diagnostic { bytes }) = record.outcome else {
        unreachable!()
    };
    Ok((
        [
            (header::CONTENT_TYPE, "application/json"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"qrab-diagnostics.json\"",
            ),
        ],
        bytes,
    )
        .into_response())
}

fn status_summary(status: &pod::Status) -> Value {
    json!({
        "phase":status.phase,"ready":status.ready,"qemu_version":status.qemu_version,
        "run_state":status.run_state,"exit_code":status.exit_code,"observed_unix_ms":status.observed_unix_ms
    })
}

fn config_summary(view: &pod::ConfigView) -> pod::ConfigView {
    let mut view = view.redacted();
    if let Some(hotplug) = &mut view.hotplug {
        hotplug.last_error = None;
    }
    view
}

fn event_summary(page: &pod::EventPage) -> Value {
    let entries: Vec<_> = page
        .events
        .iter()
        .map(|event| {
            json!({
                "seq":event.seq,"received_unix_ms":event.received_unix_ms,
                "qemu_time":event.qemu_time,"event":event.event,"truncated":event.truncated
            })
        })
        .collect();
    json!({"events":entries,"latest":page.latest,"missed":page.missed,
        "rejected":page.rejected,"qmp_dropped":page.qmp_dropped,"payloads_omitted":true})
}

async fn collect_bytes(control: &pod::Backend) -> Result<Vec<u8>, &'static str> {
    let (status, config, metrics) =
        tokio::join!(control.status(), control.config_view(), control.metrics());
    let snapshot = json!({
        "format":"QrabDiagnostic.v1_0_0","redacted":true,
        "status":status.as_ref().ok().map(status_summary),
        "configuration":config.as_ref().ok().map(config_summary),
        "metrics":metrics.as_ref().ok(),
        "events":event_summary(&control.event_summaries(None,64)),
        "availability":{"status":status.is_ok(),"configuration":config.is_ok(),"metrics":metrics.is_ok()}
    });
    let bytes =
        serde_json::to_vec_pretty(&snapshot).map_err(|_| "Diagnostic serialization failed.")?;
    if bytes.len() > ATTACHMENT_LIMIT {
        return Err("The diagnostic bundle exceeds its byte limit.");
    }
    Ok(bytes)
}

pub(super) async fn collect(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Failure> {
    let who = admin(&app, &headers).await?;
    let body = action_body(&body)?;
    if body.get("DiagnosticDataType").is_none() {
        return Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "ActionParameterMissing",
            "DiagnosticDataType is required.",
            "Send DiagnosticDataType Manager.",
        ));
    }
    if body["DiagnosticDataType"] != "Manager" {
        return Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "ActionParameterNotSupported",
            "Only Manager diagnostic data is supported.",
            "Send DiagnosticDataType Manager.",
        ));
    }
    if body
        .as_object()
        .is_some_and(|m| m.keys().any(|key| key != "DiagnosticDataType"))
    {
        return Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "ActionParameterUnknown",
            "Unknown diagnostic parameter.",
            "Send only DiagnosticDataType.",
        ));
    }
    let id = app.tasks.begin("Collect manager diagnostics", COLLECT)?;
    let response = super::tasks::accepted(&app.tasks, id);
    let store = app.tasks.clone();
    let control = app.control.clone();
    tracing::info!(subject=%who.subject,task_id=id,"Redfish diagnostic collection requested");
    tokio::spawn(async move {
        let outcome =
            match tokio::time::timeout(Duration::from_secs(10), collect_bytes(&control)).await {
                Ok(Ok(bytes)) => Outcome::Diagnostic {
                    bytes: Bytes::from(bytes),
                },
                Ok(Err(message)) => Outcome::Failed { message },
                Err(_) => Outcome::Failed {
                    message: "Diagnostic collection timed out.",
                },
            };
        store.finish(id, outcome);
    });
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_redaction_removes_free_form_status_and_event_payloads() {
        let status:pod::Status = serde_json::from_value(json!({
            "phase":"running","ready":true,"manager_pid":1,"manager_start_ticks":2,
            "qemu_pid":3,"qemu_start_ticks":4,"qemu_version":[11,1,0],
            "run_state":"running","observed_unix_ms":0,"exit_code":null,"detail":"secret endpoint /private/path"
        })).unwrap();
        assert!(!status_summary(&status).to_string().contains("secret"));
        let page = pod::EventPage {
            events: vec![pod::VmEvent {
                seq: 1,
                received_unix_ms: 2,
                qemu_time: None,
                event: "BLOCK_IO_ERROR".into(),
                data: json!({"filename":"/private/path","secret":"chap password"}),
                truncated: false,
            }],
            next: 1,
            latest: 1,
            missed: 0,
            rejected: 0,
            qmp_dropped: 0,
            epoch: None,
        };
        let events = event_summary(&page).to_string();
        assert!(events.contains("BLOCK_IO_ERROR"));
        assert!(!events.contains("private") && !events.contains("password"));
    }

    #[test]
    fn diagnostic_attachment_is_bounded_and_discoverable_from_task() {
        let store = super::super::tasks::TaskStore::default();
        let id = store.begin("diagnostic", COLLECT).unwrap();
        store.finish(
            id,
            Outcome::Diagnostic {
                bytes: Bytes::from(vec![b'x'; ATTACHMENT_LIMIT]),
            },
        );
        let record = store.get(id).unwrap();
        let entry = entry_json(&record);
        assert_eq!(entry["AdditionalDataSizeBytes"], ATTACHMENT_LIMIT);
        assert_eq!(entry["AdditionalDataURI"], attachment_uri(id));
        assert_eq!(
            record.json(false)["Links"]["CreatedResources"][0]["@odata.id"],
            entry_uri(id)
        );
        assert_eq!(store.diagnostics().len(), 1);
    }
    #[tokio::test]
    async fn diagnostic_collection_uses_metadata_projection_with_the_same_event_window() {
        let (control, receiver) = pod::control_channel();
        for index in 0..260 {
            receiver.record_event(serde_json::from_value(json!({"event":"BLOCK_IO_ERROR","data":{"index":index,"private":"storage-endpoint-credential"},"timestamp":{"seconds":1,"microseconds":0}})).unwrap());
        }
        let backend = pod::Backend::Local(control);
        let raw = backend.events(Some(0), 64);
        let summary = backend.event_summaries(Some(0), 64);
        assert_eq!(
            (
                summary.latest,
                summary.next,
                summary.missed,
                summary.rejected,
                summary.qmp_dropped
            ),
            (
                raw.latest,
                raw.next,
                raw.missed,
                raw.rejected,
                raw.qmp_dropped
            )
        );
        assert!(summary.missed > 0);
        for (raw, summary) in raw.events.iter().zip(&summary.events) {
            assert_eq!(
                (
                    summary.seq,
                    summary.received_unix_ms,
                    summary.qemu_time,
                    &summary.event,
                    summary.truncated
                ),
                (
                    raw.seq,
                    raw.received_unix_ms,
                    raw.qemu_time,
                    &raw.event,
                    raw.truncated
                )
            );
            assert!(summary.data.is_null());
            assert!(!raw.data.is_null());
        }
        drop(receiver);
        let bytes = collect_bytes(&backend).await.unwrap();
        let bundle: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(bundle["events"]["events"].as_array().unwrap().len(), 64);
        assert!(!String::from_utf8(bytes)
            .unwrap()
            .contains("storage-endpoint-credential"));
    }
}

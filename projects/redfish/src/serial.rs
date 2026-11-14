//! Guest serial data can contain credentials; every route is admin-only.
use super::*;
pub(super) const SERVICE: &str = "/redfish/v1/Managers/1/LogServices/Serial";
const ENTRIES: &str = "/redfish/v1/Managers/1/LogServices/Serial/Entries";
const ENTRY: &str = "/redfish/v1/Managers/1/LogServices/Serial/Entries/Tail";
const ATTACHMENT: &str = "/redfish/v1/Managers/1/LogServices/Serial/Entries/Tail/attachment";
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(SERVICE, get(service))
        .route(ENTRIES, get(entries))
        .route(ENTRY, get(entry))
        .route(ATTACHMENT, get(attachment))
}
async fn service(State(app): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, Failure> {
    admin(&app, &headers).await?;
    Ok(Json(
        json!({"@odata.id":SERVICE,"@odata.type":"#LogService.v1_7_0.LogService","Id":"Serial","Name":"Guest serial tail",
        "ServiceEnabled":true,"LogEntryType":"Oem","OverWritePolicy":"WrapsWhenFull","MaxNumberOfRecords":1,"Entries":link(ENTRIES),
        "Oem":{"Qrab":{"@odata.type":"#QrabSerialLog.v1_0_0.QrabSerialLog","CapacityBytes":pod::SERIAL_CAPACITY}}}),
    ))
}
async fn entries(State(app): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, Failure> {
    admin(&app, &headers).await?;
    Ok(Json(collection(
        ENTRIES,
        "LogEntryCollection",
        "Guest serial tail",
        vec![link(ENTRY)],
    )))
}
async fn entry(State(app): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, Failure> {
    admin(&app, &headers).await?;
    let tail = app
        .control
        .serial()
        .await
        .map_err(|_| Failure::unavailable("Serial history is unavailable."))?;
    Ok(Json(
        json!({"@odata.id":ENTRY,"@odata.type":"#LogEntry.v1_17_0.LogEntry","Id":"Tail","Name":"Guest serial tail","EntryType":"Oem",
        "OemRecordFormat":"QrabSerialTail","AdditionalDataURI":ATTACHMENT,"AdditionalDataSizeBytes":tail.text.len(),
        "Oem":{"Qrab":{"@odata.type":"#QrabSerialTail.v1_0_0.QrabSerialTail","DroppedBytes":tail.dropped_bytes,"RetainedBytes":tail.retained_bytes}}}),
    ))
}
async fn attachment(State(app): State<AppState>, headers: HeaderMap) -> Result<Response, Failure> {
    admin(&app, &headers).await?;
    let tail = app
        .control
        .serial()
        .await
        .map_err(|_| Failure::unavailable("Serial history is unavailable."))?;
    Ok((
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=guest-serial.txt",
            ),
        ],
        tail.text,
    )
        .into_response())
}

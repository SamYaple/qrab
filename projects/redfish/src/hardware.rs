//! Native, live hardware resources backed by the manager's sanitized QMP view.
use super::*;

pub(super) const STORAGE: &str = "/redfish/v1/Systems/1/Storage";
pub(super) const FIRMWARE: &str = "/redfish/v1/UpdateService/FirmwareInventory";
pub(super) const TRUSTED: &str = "/redfish/v1/Chassis/1/TrustedComponents";
const PROCESSORS: &str = "/redfish/v1/Systems/1/Processors";

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(PROCESSORS, get(processors))
        .route("/redfish/v1/Systems/1/Processors/{id}", get(processor))
        .route(STORAGE, get(storage_collection))
        .route("/redfish/v1/Systems/1/Storage/1", get(storage))
        .route("/redfish/v1/Systems/1/Storage/1/Volumes", get(volumes))
        .route("/redfish/v1/Systems/1/Storage/1/Volumes/{id}", get(volume))
        .route("/redfish/v1/Chassis/1/Drives/{id}", get(drive))
        .route(
            "/redfish/v1/Chassis/1/Drives/{id}/Metrics",
            get(drive_metrics),
        )
        .route(TRUSTED, get(trusted_collection))
        .route(
            "/redfish/v1/Chassis/1/TrustedComponents/TPM",
            get(trusted_component),
        )
        .route(FIRMWARE, get(firmware_collection))
        .route(
            "/redfish/v1/UpdateService/FirmwareInventory/{id}",
            get(firmware),
        )
}

async fn snapshot(app: &AppState, headers: &HeaderMap) -> Result<pod::HardwareInventory, Failure> {
    principal(app, headers).await?;
    app.control
        .inventory()
        .await
        .map_err(|_| Failure::unavailable("Live hardware inventory is unavailable."))
}

fn drive_path(id: &str) -> String {
    format!("/redfish/v1/Chassis/1/Drives/{id}")
}
fn volume_path(id: &str) -> String {
    format!("{STORAGE}/1/Volumes/{id}")
}
fn device_status(drive: &pod::DriveInventory) -> Value {
    json!({"State":if drive.media_present { "Enabled" } else { "Absent" },"Health":if drive.io_failed { "Critical" } else { "OK" }})
}

pub(super) fn processor_resource(cpu: &pod::ProcessorInventory) -> Value {
    let mut value = json!({
        "@odata.id":format!("{PROCESSORS}/{}",cpu.id),"@odata.type":"#Processor.v1_18_0.Processor",
        "Id":cpu.id,"Name":cpu.socket.map(|s| format!("Virtual CPU socket {s}")).unwrap_or_else(|| "Virtual processors (topology unavailable)".into()),
        "ProcessorType":"CPU","TotalThreads":cpu.total_threads
    });
    if let Some(cores) = cpu.total_cores {
        value["TotalCores"] = json!(cores);
    }
    if let Some(model) = &cpu.model {
        value["Model"] = json!(model);
    }
    if let Some(socket) = cpu.socket {
        value["Socket"] = json!(socket.to_string());
    }
    value
}

async fn processors(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    let view = snapshot(&app, &headers).await?;
    Ok(Json(collection(
        PROCESSORS,
        "ProcessorCollection",
        "Virtual processor sockets",
        view.processors
            .iter()
            .map(|cpu| link(&format!("{PROCESSORS}/{}", cpu.id)))
            .collect(),
    )))
}
async fn processor(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Failure> {
    snapshot(&app, &headers)
        .await?
        .processors
        .iter()
        .find(|cpu| cpu.id == id)
        .map(processor_resource)
        .map(Json)
        .ok_or_else(Failure::not_found)
}
async fn storage_collection(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    let view = snapshot(&app, &headers).await?;
    Ok(Json(collection(
        STORAGE,
        "StorageCollection",
        "Virtual storage collection",
        if view.drives.is_empty() {
            vec![]
        } else {
            vec![link(&format!("{STORAGE}/1"))]
        },
    )))
}
async fn storage(State(app): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, Failure> {
    let view = snapshot(&app, &headers).await?;
    if view.drives.is_empty() {
        return Err(Failure::not_found());
    }
    Ok(Json(json!({
        "@odata.id":format!("{STORAGE}/1"),"@odata.type":"#Storage.v1_18_0.Storage","Id":"1","Name":"QEMU virtual storage",
        "Drives":view.drives.iter().map(|d| link(&drive_path(&d.id))).collect::<Vec<_>>(),
        "Drives@odata.count":view.drives.len(),"Volumes":link(&format!("{STORAGE}/1/Volumes"))
    })))
}
async fn volumes(State(app): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, Failure> {
    let view = snapshot(&app, &headers).await?;
    if view.drives.is_empty() {
        return Err(Failure::not_found());
    }
    Ok(Json(collection(
        &format!("{STORAGE}/1/Volumes"),
        "VolumeCollection",
        "Guest-visible virtual volumes",
        view.drives
            .iter()
            .filter(|d| d.media_present)
            .map(|d| link(&volume_path(&d.id)))
            .collect(),
    )))
}

pub(super) fn drive_resource(drive: &pod::DriveInventory) -> Value {
    let path = drive_path(&drive.id);
    let mut value = json!({
        "@odata.id":path,"@odata.type":"#Drive.v1_21_0.Drive","Id":drive.id,"Name":format!("Virtual drive {}",drive.id),
        "Manufacturer":"QEMU","Status":device_status(drive),
        "Links":{"Volumes":if drive.media_present { vec![link(&volume_path(&drive.id))] } else { vec![] }},
        "Oem":{"Qrab":{"@odata.type":"#QrabDrive.v1_0_0.QrabDrive","ReadOnly":drive.read_only,"Removable":drive.removable,
            "MediaPresent":drive.media_present,"Format":drive.format}}
    });
    if let Some(capacity) = drive.capacity_bytes {
        value["CapacityBytes"] = json!(capacity);
    }
    if drive.counters.is_some() {
        value["Metrics"] = link(&format!("{path}/Metrics"));
    }
    value
}
async fn drive(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Failure> {
    snapshot(&app, &headers)
        .await?
        .drives
        .iter()
        .find(|d| d.id == id)
        .map(drive_resource)
        .map(Json)
        .ok_or_else(Failure::not_found)
}
async fn volume(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Failure> {
    let view = snapshot(&app, &headers).await?;
    let d = view
        .drives
        .iter()
        .find(|d| d.id == id && d.media_present)
        .ok_or_else(Failure::not_found)?;
    let mut value = json!({
        "@odata.id":volume_path(&d.id),"@odata.type":"#Volume.v1_10_0.Volume","Id":d.id,"Name":format!("Virtual volume {}",d.id),
        "Encrypted":d.encrypted,"Status":device_status(d),"RAIDType":"None",
        "AccessCapabilities":if d.read_only { vec!["Read"] } else { vec!["Read","Write"] },
        "Links":{"Drives":[link(&drive_path(&d.id))]}
    });
    if let Some(capacity) = d.capacity_bytes {
        value["CapacityBytes"] = json!(capacity);
    }
    Ok(Json(value))
}
async fn drive_metrics(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Failure> {
    let view = snapshot(&app, &headers).await?;
    let d = view
        .drives
        .iter()
        .find(|d| d.id == id)
        .ok_or_else(Failure::not_found)?;
    let c = d
        .counters
        .as_ref()
        .ok_or_else(|| Failure::unavailable("Drive I/O counters are unavailable."))?;
    Ok(Json(json!({
        "@odata.id":format!("{}/Metrics",drive_path(&d.id)),"@odata.type":"#DriveMetrics.v1_2_0.DriveMetrics",
        "Id":"Metrics","Name":format!("Drive {} metrics",d.id),
        "ReadIOKiBytes":c.read_bytes/1024,"WriteIOKiBytes":c.write_bytes/1024,
        "Oem":{"Qrab":{"@odata.type":"#QrabDriveMetrics.v1_0_0.QrabDriveMetrics",
            "Timestamp":rfc3339(view.observed_unix_ms),"ReadBytes":c.read_bytes,"WriteBytes":c.write_bytes,
            "ReadOperations":c.read_operations,"WriteOperations":c.write_operations,"FlushOperations":c.flush_operations}}
    })))
}

async fn configured(app: &AppState, headers: &HeaderMap) -> Result<pod::ConfigView, Failure> {
    principal(app, headers).await?;
    app.control
        .config_view()
        .await
        .map(|v| v.redacted())
        .map_err(|_| Failure::unavailable("VM configuration is unavailable."))
}
async fn trusted_collection(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    let view = configured(&app, &headers).await?;
    Ok(Json(collection(
        TRUSTED,
        "TrustedComponentCollection",
        "Virtual trusted components",
        if view.tpm {
            vec![link(&format!("{TRUSTED}/TPM"))]
        } else {
            vec![]
        },
    )))
}
async fn trusted_component(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    let view = configured(&app, &headers).await?;
    if !view.tpm {
        return Err(Failure::not_found());
    }
    let status = app.control.status().await.ok();
    // Process identities remain inside the manager. No physical manufacturer,
    // endorsement certificate or attestation capability is inferred from swtpm.
    let active = status
        .as_ref()
        .is_some_and(|s| s.swtpm_pid.is_some() && s.phase == pod::Phase::Running);
    Ok(Json(json!({
        "@odata.id":format!("{TRUSTED}/TPM"),"@odata.type":"#TrustedComponent.v1_3_0.TrustedComponent",
        "Id":"TPM","Name":"Virtual TPM 2.0","Model":"swtpm TPM 2.0 (tpm-tis)",
        "Status":{"State":if active {"Enabled"} else {"UnavailableOffline"}},
        "Links":{"ComponentsProtected":[link(SYSTEM)]}
    })))
}

fn firmware_items(view: &pod::ConfigView, status: Option<&pod::Status>) -> Vec<Value> {
    let item = |id: &str, name: &str, version: Option<String>| {
        let mut value = json!({"@odata.id":format!("{FIRMWARE}/{id}"),"@odata.type":"#SoftwareInventory.v1_12_0.SoftwareInventory",
            "Id":id,"Name":name,"Updateable":false});
        if let Some(version) = version {
            value["Version"] = json!(version);
        }
        value
    };
    let mut items = vec![item(
        "Redfish",
        "qrab Redfish service",
        Some(env!("CARGO_PKG_VERSION").into()),
    )];
    if let Some((major, minor, micro)) = status.and_then(|s| s.qemu_version) {
        items.push(item(
            "QEMU",
            "QEMU virtual platform",
            Some(format!("{major}.{minor}.{micro}")),
        ));
    }
    if view.boot == pod::BootMode::Uefi {
        items.push(item("UEFI", "UEFI firmware", None));
    }
    if view.tpm {
        items.push(item("TPM", "Software TPM", None));
    }
    items
}
async fn firmware_collection(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    let view = configured(&app, &headers).await?;
    let status = app.control.status().await.ok();
    Ok(Json(collection(
        FIRMWARE,
        "SoftwareInventoryCollection",
        "Platform firmware and software inventory",
        firmware_items(&view, status.as_ref())
            .iter()
            .map(|v| link(v["@odata.id"].as_str().unwrap()))
            .collect(),
    )))
}
async fn firmware(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Failure> {
    let view = configured(&app, &headers).await?;
    let status = app.control.status().await.ok();
    firmware_items(&view, status.as_ref())
        .into_iter()
        .find(|v| v["Id"] == id)
        .map(Json)
        .ok_or_else(Failure::not_found)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_drive_capacity_and_links_are_exact_and_unknowns_are_omitted() {
        let drive = pod::DriveInventory {
            id: "iscsi-root".into(),
            capacity_bytes: Some(10_737_418_240),
            read_only: true,
            removable: false,
            media_present: true,
            encrypted: false,
            format: Some("raw".into()),
            io_failed: true,
            counters: None,
        };
        let value = drive_resource(&drive);
        assert_eq!(value["CapacityBytes"], 10_737_418_240u64);
        assert_eq!(value["Status"]["Health"], "Critical");
        assert_eq!(
            value["Links"]["Volumes"][0]["@odata.id"],
            volume_path("iscsi-root")
        );
        assert!(
            value.get("Protocol").is_none()
                && value.get("MediaType").is_none()
                && value.get("Metrics").is_none()
        );
    }
    #[test]
    fn cpu_resource_uses_reported_socket_model_and_topology() {
        let cpu = pod::ProcessorInventory {
            id: "socket-2".into(),
            socket: Some(2),
            total_threads: 8,
            total_cores: Some(4),
            model: Some("qemu64-x86_64-cpu".into()),
        };
        let value = processor_resource(&cpu);
        assert_eq!(value["Socket"], "2");
        assert_eq!(value["TotalCores"], 4);
        assert_eq!(value["TotalThreads"], 8);
        assert!(value.get("MaxSpeedMHz").is_none());
    }
}

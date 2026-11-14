//! Current Secure Boot state from guest EFI runtime variables. Read-only until
//! a supported authenticated-variable management path exists.
use super::*;

pub(super) const SECURE_BOOT: &str = "/redfish/v1/Systems/1/SecureBoot";

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(SECURE_BOOT, get(secure_boot).patch(patch_secure_boot))
        .route(
            SECURE_BOOT_SETTINGS,
            get(secure_boot_settings).patch(patch_secure_boot),
        )
        .route(ARTIFACTS, get(artifacts))
        .route(STAGED, get(staged))
        .route(STAGE, post(stage))
}

fn mode(state: &pod::FirmwareState) -> Option<&'static str> {
    match (state.setup_mode, state.audit_mode, state.deployed_mode) {
        (_, Some(true), Some(true)) => None,
        (_, Some(true), _) => Some("AuditMode"),
        (_, _, Some(true)) => Some("DeployedMode"),
        (Some(true), Some(false), Some(false)) => Some("SetupMode"),
        (Some(false), Some(false), Some(false)) => Some("UserMode"),
        _ => None,
    }
}

pub(super) fn resource(snapshot: &pod::FirmwareSnapshot) -> Value {
    let state = &snapshot.state;
    json!({
        "@odata.id":SECURE_BOOT,"@odata.type":"#SecureBoot.v1_1_0.SecureBoot",
        "Id":"SecureBoot","Name":"UEFI Secure Boot",
        "SecureBootCurrentBoot":state.secure_boot.map(|enabled|if enabled {"Enabled"}else{"Disabled"}),
        "SecureBootMode":mode(state),
        "Oem":{"Qrab":{"@odata.type":"#QrabSecureBoot.v1_0_0.QrabSecureBoot",
            "EfiRuntimeAvailable":state.efi_runtime_available,"Observed":rfc3339(snapshot.observed_unix_ms)}}
    })
}

async fn secure_boot(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let snapshot = app.control.agent_firmware().await.ok();
    let observed = snapshot.is_some();
    let mut value = resource(&snapshot.unwrap_or_default());
    if !observed {
        value["Oem"]["Qrab"]["EfiRuntimeAvailable"] = Value::Null;
        value["Oem"]["Qrab"]["Observed"] = Value::Null;
    }
    value["Oem"]["Qrab"]["CurrentBootStateAvailable"] = json!(observed);
    value["SecureBootDatabases"] = link(super::secure_boot::DATABASES);
    value["Actions"] = json!({"Oem":{"#QrabSecureBoot.UpdateAuthenticatedDatabase":{"target":super::secure_boot::AUTH_UPDATE}}});
    if let Ok(settings) = app.control.secure_boot_settings().await {
        value["Oem"]["Qrab"]["ManagementSupported"] = json!(settings.supported);
        if settings.supported {
            value["SecureBootEnable"] = json!(settings.current_setting);
            value["@Redfish.Settings"] = json!({"@odata.type":"#Settings.v1_4_0.Settings","SettingsObject":link(SECURE_BOOT_SETTINGS)});
            value["Oem"]["Qrab"]["PendingColdBoot"] = json!(settings.pending_cold_boot);
        }
    }
    Ok(Json(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_efi_information_never_becomes_disabled_or_a_writable_setting() {
        let resource = resource(&pod::FirmwareSnapshot::default());
        assert_eq!(resource["SecureBootCurrentBoot"], Value::Null);
        assert_eq!(resource["SecureBootMode"], Value::Null);
        assert!(resource.get("SecureBootEnable").is_none());
        assert!(resource.get("Actions").is_none());
    }
    #[test]
    fn modes_follow_all_observed_variables_instead_of_inventing_user_mode() {
        let mut snapshot = pod::FirmwareSnapshot {
            state: pod::FirmwareState {
                efi_runtime_available: true,
                secure_boot: Some(true),
                setup_mode: Some(false),
                audit_mode: Some(false),
                deployed_mode: Some(false),
            },
            ..Default::default()
        };
        assert_eq!(resource(&snapshot)["SecureBootMode"], "UserMode");
        assert_eq!(resource(&snapshot)["SecureBootCurrentBoot"], "Enabled");
        snapshot.state.deployed_mode = Some(true);
        assert_eq!(resource(&snapshot)["SecureBootMode"], "DeployedMode");
        snapshot.state.audit_mode = Some(true);
        assert_eq!(resource(&snapshot)["SecureBootMode"], Value::Null);
        snapshot.state.deployed_mode = None;
        snapshot.state.audit_mode = None;
        assert_eq!(resource(&snapshot)["SecureBootMode"], Value::Null);
    }
}

pub(super) const ARTIFACTS: &str = "/redfish/v1/UpdateService/Oem/Qrab/Artifacts";
pub(super) const STAGED: &str = "/redfish/v1/UpdateService/Oem/Qrab/StagedFirmware";
pub(super) const STAGE: &str =
    "/redfish/v1/UpdateService/Actions/Oem/QrabUpdateService.StageFirmware";

async fn artifacts(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let catalog = app
        .control
        .firmware_catalog()
        .await
        .map_err(|_| Failure::unavailable("The approved firmware catalog is unavailable."))?;
    Ok(Json(
        json!({"@odata.id":ARTIFACTS,"@odata.type":"#QrabFirmwareCatalog.v1_0_0.QrabFirmwareCatalog",
        "Id":"Artifacts","Name":"Approved immutable firmware artifacts","Enabled":catalog.enabled,
        "Artifacts":catalog.artifacts}),
    ))
}

async fn staged(State(app): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let status = app
        .control
        .firmware_stage_status()
        .await
        .map_err(|_| Failure::unavailable("The staged firmware status is unavailable."))?;
    Ok(Json(
        json!({"@odata.id":STAGED,"@odata.type":"#QrabFirmwareStage.v1_0_0.QrabFirmwareStage",
        "Id":"StagedFirmware","Name":"Firmware activation status","Enabled":status.enabled,
        "Artifact":status.staged,"Activation":status.activation}),
    ))
}

fn stage_request(body: &Bytes) -> Result<String, Failure> {
    let object = action_body(body)?;
    let id = object
        .get("ArtifactId")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            Failure::new(
                StatusCode::BAD_REQUEST,
                "ActionParameterMissing",
                "ArtifactId is required.",
                "Use an ID from the approved firmware catalog.",
            )
        })?;
    if object.as_object().is_none_or(|o| o.len() != 1)
        || id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        || id == "."
        || id == ".."
    {
        return Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "ActionParameterValueFormatError",
            "The firmware staging request is invalid.",
            "Provide only an ArtifactId from the approved catalog.",
        ));
    }
    Ok(id.into())
}

async fn stage(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Failure> {
    admin(&app, &headers).await?;
    let artifact = stage_request(&body)?;
    let id = app.tasks.begin("Stage approved firmware", STAGE)?;
    let tasks = app.tasks.clone();
    let control = app.control.clone();
    tokio::spawn(async move {
        let outcome = match control.firmware_stage(artifact).await {
            Ok(status) => tasks::Outcome::Completed {
                output: Some(
                    json!({
                        "Activation":status.activation,"Artifact":status.staged
                    })
                    .to_string(),
                ),
            },
            Err(_) => tasks::Outcome::Failed {
                message: "Firmware staging failed validation or the manager is unavailable.",
            },
        };
        tasks.finish(id, outcome);
    });
    Ok(tasks::accepted(&app.tasks, id))
}

#[cfg(test)]
mod stage_tests {
    use super::*;
    #[test]
    fn staging_accepts_only_explicit_catalog_identity() {
        assert_eq!(
            stage_request(&Bytes::from_static(br#"{"ArtifactId":"ovmf-x64-v2"}"#)).unwrap(),
            "ovmf-x64-v2"
        );
        for body in [
            br#"{"ArtifactId":"../other"}"#.as_slice(),
            br#"{"ArtifactId":"https://arbitrary/image"}"#,
            br#"{"ArtifactId":"approved","ImageURI":"https://arbitrary"}"#,
            br#"{}"#,
            br#"{"ArtifactId":1}"#,
        ] {
            assert!(stage_request(&Bytes::copy_from_slice(body)).is_err());
        }
    }
}

const SECURE_BOOT_SETTINGS: &str = "/redfish/v1/Systems/1/SecureBoot/Settings";
async fn secure_boot_settings(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let settings = app
        .control
        .secure_boot_settings()
        .await
        .map_err(|_| Failure::unavailable("Secure Boot settings are unavailable."))?;
    if !settings.supported {
        return Err(Failure::not_found());
    }
    Ok(Json(
        json!({"@odata.id":SECURE_BOOT_SETTINGS,"@odata.type":"#SecureBoot.v1_1_0.SecureBoot",
        "Id":"Settings","Name":"Secure Boot next cold boot settings","SecureBootEnable":settings.enabled,
        "Oem":{"Qrab":{"@odata.type":"#QrabSecureBootSettings.v1_0_0.QrabSecureBootSettings",
            "PendingColdBoot":settings.pending_cold_boot,"Generation":settings.generation,"AppliedGeneration":settings.applied_generation,
            "Activation":"ColdManagerBoot"}}}),
    ))
}
fn requested_secure_boot(body: &Bytes) -> Result<bool, Failure> {
    let value = action_body(body)?;
    let object = value.as_object().unwrap();
    if object.len() != 1 || !object.contains_key("SecureBootEnable") {
        return Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "PropertyNotWritable",
            "Only SecureBootEnable may be changed.",
            "Supply a boolean SecureBootEnable; activation requires a cold manager boot.",
        ));
    }
    object["SecureBootEnable"].as_bool().ok_or_else(|| {
        Failure::new(
            StatusCode::BAD_REQUEST,
            "PropertyValueTypeError",
            "SecureBootEnable must be a boolean.",
            "Supply true or false.",
        )
    })
}
async fn patch_secure_boot(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Failure> {
    admin(&app, &headers).await?;
    let enabled = requested_secure_boot(&body)?;
    let settings = app
        .control
        .secure_boot_settings()
        .await
        .map_err(|_| Failure::unavailable("Secure Boot management is unavailable."))?;
    if !settings.supported {
        return Err(Failure::new(StatusCode::CONFLICT,"OperationNotAllowed","This VM has no explicit secure-capable firmware configuration.","Select secure-capable firmware and enroll the required keys before enabling Secure Boot."));
    }
    let id = app.tasks.begin("Stage Secure Boot settings", SECURE_BOOT)?;
    let store = app.tasks.clone();
    let control = app.control.clone();
    tokio::spawn(async move {
        let outcome=match control.stage_secure_boot(enabled).await {
            Ok(settings)=>tasks::Outcome::Completed{output:Some(json!({"SecureBootEnable":settings.enabled,"PendingColdBoot":settings.pending_cold_boot,"Generation":settings.generation}).to_string())},
            Err(_)=>tasks::Outcome::Failed{message:"Secure Boot staging failed validation or persistent-state ownership checks; no live variable store was changed."},
        };
        store.finish(id, outcome);
    });
    Ok(tasks::accepted(&app.tasks, id))
}

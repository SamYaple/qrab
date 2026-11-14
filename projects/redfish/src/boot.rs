//! Native UEFI BootOptions and BootNext through the guest EFI runtime API.
//! Device bootindex values are not firmware Boot#### references.
use super::*;

pub(super) const OPTIONS: &str = "/redfish/v1/Systems/1/BootOptions";
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(OPTIONS, get(options))
        .route("/redfish/v1/Systems/1/BootOptions/{id}", get(option))
}
async fn snapshot(app: &AppState) -> Result<pod::BootSnapshot, Failure> {
    app.control
        .agent_boot()
        .await
        .map_err(|_| Failure::unavailable("Guest UEFI boot state is unavailable."))
}
pub(super) fn system_boot(snapshot: &pod::BootSnapshot) -> Value {
    let s = &snapshot.state;
    let mut value = json!({"BootOptions":link(OPTIONS),
        "Oem":{"Qrab":{"@odata.type":"#QrabBoot.v1_0_0.QrabBoot",
            "EfiRuntimeAvailable":s.efi_runtime_available,"Observed":rfc3339(snapshot.observed_unix_ms)}}});
    if s.efi_runtime_available {
        value["BootOrder"] = json!(s.boot_order);
        value["BootNext"] = json!(s.boot_next);
        value["BootSourceOverrideEnabled"] = json!(if s.boot_next.is_some() {
            "Once"
        } else {
            "Disabled"
        });
        value["BootSourceOverrideTarget"] = json!(if s.boot_next.is_some() {
            "UefiBootNext"
        } else {
            "None"
        });
        value["BootSourceOverrideMode"] = json!("UEFI");
        value["BootSourceOverrideEnabled@Redfish.AllowableValues"] = json!(["Disabled", "Once"]);
        value["BootSourceOverrideTarget@Redfish.AllowableValues"] = json!(["None", "UefiBootNext"]);
    }
    value
}
fn resource(option: &pod::BootOption) -> Value {
    json!({"@odata.id":format!("{OPTIONS}/{}",option.reference),"@odata.type":"#BootOption.v1_0_6.BootOption",
        "Id":option.reference,"Name":format!("UEFI {}",option.reference),"BootOptionReference":option.reference,
        "DisplayName":option.description,"BootOptionEnabled":option.active})
}
async fn options(State(app): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let snapshot = snapshot(&app).await?;
    Ok(Json(collection(
        OPTIONS,
        "BootOptionCollection",
        "UEFI boot options",
        snapshot
            .state
            .options
            .iter()
            .map(|o| link(&format!("{OPTIONS}/{}", o.reference)))
            .collect(),
    )))
}
async fn option(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    snapshot(&app)
        .await?
        .state
        .options
        .iter()
        .find(|o| o.reference == id)
        .map(resource)
        .map(Json)
        .ok_or_else(Failure::not_found)
}
fn property_error(message: impl Into<String>) -> Failure {
    Failure::new(StatusCode::BAD_REQUEST,"PropertyValueNotInList",message,
        "Use BootSourceOverrideEnabled=Once, BootSourceOverrideTarget=UefiBootNext and an active BootNext reference; Disabled clears BootNext.")
}
fn parse_patch(body: &Bytes) -> Result<Option<String>, Failure> {
    let value = action_body(body)?;
    let top = value.as_object().unwrap();
    if top.len() != 1 || !top.contains_key("Boot") {
        return Err(property_error(
            "Only Boot settings may be changed by this request.",
        ));
    }
    let Some(boot) = top["Boot"].as_object() else {
        return Err(property_error("Boot must be an object."));
    };
    for key in boot.keys() {
        if ![
            "BootSourceOverrideEnabled",
            "BootSourceOverrideTarget",
            "BootSourceOverrideMode",
            "BootNext",
        ]
        .contains(&key.as_str())
        {
            return Err(property_error(format!(
                "The Boot property {key} is not writable."
            )));
        }
    }
    if boot
        .get("BootSourceOverrideMode")
        .is_some_and(|v| v != "UEFI")
    {
        return Err(property_error("Only UEFI boot overrides are supported."));
    }
    match boot
        .get("BootSourceOverrideEnabled")
        .and_then(Value::as_str)
    {
        Some("Disabled") => {
            if boot.get("BootNext").is_some_and(|v| !v.is_null())
                || boot
                    .get("BootSourceOverrideTarget")
                    .is_some_and(|v| v != "None" && v != "UefiBootNext")
            {
                return Err(property_error(
                    "A disabled override cannot select a BootNext option.",
                ));
            }
            Ok(None)
        }
        Some("Once") => {
            if boot.get("BootSourceOverrideTarget").and_then(Value::as_str) != Some("UefiBootNext")
            {
                return Err(property_error("One-time boot requires UefiBootNext."));
            }
            let reference = boot
                .get("BootNext")
                .and_then(Value::as_str)
                .filter(|r| {
                    r.len() == 8
                        && r.starts_with("Boot")
                        && r.as_bytes()[4..]
                            .iter()
                            .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(b))
                })
                .ok_or_else(|| property_error("BootNext must name an existing Boot#### option."))?;
            Ok(Some(reference.into()))
        }
        _ => Err(property_error(
            "Only Once and Disabled boot overrides are supported.",
        )),
    }
}

/// Attach as PATCH on SYSTEM. Only the fixed guest BootNext variable can be
/// written, and success follows confirmed EFI readback on the agent worker.
pub(super) async fn patch(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, Failure> {
    admin(&app, &headers).await?;
    let reference = parse_patch(&body)?;
    let current = snapshot(&app).await?;
    if !current.state.efi_runtime_available {
        return Err(Failure::new(
            StatusCode::CONFLICT,
            "OperationNotAllowed",
            "The guest EFI runtime interface is unavailable.",
            "Boot a UEFI guest with the qrab guest agent.",
        ));
    }
    if reference.as_ref().is_some_and(|r| {
        !current
            .state
            .options
            .iter()
            .any(|o| &o.reference == r && o.active)
    }) {
        return Err(property_error(
            "BootNext must reference an active existing UEFI boot option.",
        ));
    }
    app.control
        .agent_boot_next(reference)
        .await
        .map_err(|e| Failure::vm(&e, "The guest did not accept and confirm UEFI BootNext."))?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_once_uses_actual_boot_next_not_qemu_bootindex() {
        let snapshot = pod::BootSnapshot {
            state: pod::BootState {
                efi_runtime_available: true,
                boot_order: vec!["Boot000A".into()],
                boot_next: Some("Boot000A".into()),
                options: vec![pod::BootOption {
                    reference: "Boot000A".into(),
                    description: "Linux".into(),
                    active: true,
                }],
            },
            ..Default::default()
        };
        let boot = system_boot(&snapshot);
        assert_eq!(boot["BootNext"], "Boot000A");
        assert_eq!(boot["BootSourceOverrideEnabled"], "Once");
        assert_eq!(boot["BootSourceOverrideTarget"], "UefiBootNext");
        assert_eq!(
            resource(&snapshot.state.options[0])["BootOptionReference"],
            "Boot000A"
        );
        assert!(boot.get("UefiBootNext").is_none());
    }
    #[test]
    fn unsupported_or_unrelated_patches_do_not_appear_to_succeed() {
        let valid=Bytes::from_static(br#"{"Boot":{"BootSourceOverrideEnabled":"Once","BootSourceOverrideTarget":"UefiBootNext","BootNext":"Boot000A"}}"#);
        assert_eq!(parse_patch(&valid).unwrap(), Some("Boot000A".into()));
        assert_eq!(
            parse_patch(&Bytes::from_static(
                br#"{"Boot":{"BootSourceOverrideEnabled":"Disabled"}}"#
            ))
            .unwrap(),
            None
        );
        for body in [
            br#"{"Boot":{"BootSourceOverrideEnabled":"Continuous"}}"#.as_slice(),
            br#"{"Boot":{"BootSourceOverrideEnabled":"Once","BootSourceOverrideTarget":"Cd"}}"#
                .as_slice(),
            br#"{"Boot":{"BootSourceOverrideEnabled":"Disabled","BootNext":"Boot000A"}}"#
                .as_slice(),
            br#"{"Boot":{"BootSourceOverrideEnabled":"Disabled"},"Name":"silently ignored"}"#
                .as_slice(),
        ] {
            assert!(parse_patch(&Bytes::copy_from_slice(body)).is_err());
        }
    }
}

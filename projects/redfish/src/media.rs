//! Native Redfish virtual media over the manager's explicit local-media policy.
use super::*;

pub(super) const COLLECTION: &str = "/redfish/v1/Systems/1/VirtualMedia";
const CD: &str = "/redfish/v1/Systems/1/VirtualMedia/CD";
const INSERT: &str = "/redfish/v1/Systems/1/VirtualMedia/CD/Actions/VirtualMedia.InsertMedia";
const EJECT: &str = "/redfish/v1/Systems/1/VirtualMedia/CD/Actions/VirtualMedia.EjectMedia";

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(COLLECTION, get(collection_resource))
        .route(CD, get(cd))
        .route(INSERT, post(insert))
        .route(EJECT, post(eject))
}

fn failure(error: manager::Error) -> Failure {
    match error {
        manager::Error::Config(message) => Failure::new(
            StatusCode::BAD_REQUEST,
            "ActionParameterValueFormatError",
            message,
            "Use a permitted local image filename and read-only media.",
        ),
        error => Failure::vm(
            &error,
            "The virtual media operation could not be completed.",
        ),
    }
}
async fn state(app: &AppState) -> Result<pod::MediaStatus, Failure> {
    app.control
        .media(pod::MediaOperation::Status)
        .await
        .map_err(failure)
}
async fn collection_resource(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let status = state(&app).await?;
    Ok(Json(collection(
        COLLECTION,
        "VirtualMediaCollection",
        "Virtual media",
        if status.enabled {
            vec![link(CD)]
        } else {
            vec![]
        },
    )))
}
fn resource(status: &pod::MediaStatus) -> Value {
    let mut value = json!({
        "@odata.id":CD,"@odata.type":"#VirtualMedia.v1_6_0.VirtualMedia","Id":"CD","Name":"Read-only virtual CD-ROM",
        "MediaTypes":["CD","DVD"],"Inserted":status.inserted,"WriteProtected":true,
        "ConnectedVia":if status.inserted {"URI"} else {"NotConnected"},
        "Actions":{"#VirtualMedia.InsertMedia":{"target":INSERT},"#VirtualMedia.EjectMedia":{"target":EJECT}}
    });
    if let Some(name) = &status.image_name {
        value["Image"] = json!(name);
        value["ImageName"] = json!(name);
    }
    value
}
async fn cd(State(app): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let status = state(&app).await?;
    if !status.enabled {
        return Err(Failure::not_found());
    }
    Ok(Json(resource(&status)))
}

fn insert_image(body: &Bytes) -> Result<String, Failure> {
    let value = action_body(body)?;
    let object = value.as_object().unwrap();
    for key in object.keys() {
        if !["Image", "Inserted", "WriteProtected"].contains(&key.as_str()) {
            return Err(Failure::new(
                StatusCode::BAD_REQUEST,
                "ActionParameterNotSupported",
                format!("The parameter {key} is not supported."),
                "Use Image and optional Inserted=true and WriteProtected=true.",
            ));
        }
    }
    let Some(image) = object.get("Image") else {
        return Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "ActionParameterMissing",
            "Image is required.",
            "Supply a local image filename.",
        ));
    };
    let Some(image) = image.as_str() else {
        return Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "ActionParameterValueTypeError",
            "Image must be a string.",
            "Supply a local image filename.",
        ));
    };
    for key in ["Inserted", "WriteProtected"] {
        if object.get(key).is_some_and(|v| v != true) {
            return Err(Failure::new(
                StatusCode::BAD_REQUEST,
                "ActionParameterNotSupported",
                format!("{key} must be true."),
                "Only inserted read-only media is supported.",
            ));
        }
    }
    Ok(image.to_owned())
}
async fn insert(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, Failure> {
    admin(&app, &headers).await?;
    let image = insert_image(&body)?;
    app.control
        .media(pod::MediaOperation::Insert { image })
        .await
        .map_err(failure)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn eject(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, Failure> {
    admin(&app, &headers).await?;
    if !action_body(&body)?.as_object().unwrap().is_empty() {
        return Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "ActionParameterNotSupported",
            "EjectMedia takes no parameters.",
            "Send an empty object.",
        ));
    }
    app.control
        .media(pod::MediaOperation::Eject)
        .await
        .map_err(failure)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn insert_does_not_silently_accept_writable_media_or_download_credentials() {
        assert_eq!(
            insert_image(&Bytes::from_static(
                br#"{"Image":"rescue.iso","WriteProtected":true}"#
            ))
            .unwrap(),
            "rescue.iso"
        );
        for body in [
            br#"{"Image":"rescue.iso","WriteProtected":false}"#.as_slice(),
            br#"{"Image":"rescue.iso","Inserted":false}"#.as_slice(),
            br#"{"Image":"rescue.iso","UserName":"secret"}"#.as_slice(),
            br#"{"Image":7}"#.as_slice(),
        ] {
            assert!(insert_image(&Bytes::copy_from_slice(body)).is_err());
        }
    }
    #[test]
    fn virtual_media_resource_contains_only_safe_backend_fields() {
        let value = resource(&pod::MediaStatus {
            enabled: true,
            connected: true,
            inserted: true,
            image_name: Some("rescue.iso".into()),
            size_bytes: Some(4096),
        });
        assert_eq!(value["Image"], "rescue.iso");
        assert_eq!(value["WriteProtected"], true);
        assert_eq!(
            value["Actions"]["#VirtualMedia.EjectMedia"]["target"],
            EJECT
        );
    }
}

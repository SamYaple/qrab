//! Public Secure Boot trust material from fixed guest EFI databases. No keys,
//! variable paths, raw write API or mutation actions are exposed by inventory.
use super::*;

pub(super) const DATABASES: &str = "/redfish/v1/Systems/1/SecureBoot/SecureBootDatabases";

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            AUTH_UPDATE,
            post(authenticated_update).layer(DefaultBodyLimit::max(AUTH_BODY_LIMIT)),
        )
        .route(DATABASES, get(databases))
        .route(
            "/redfish/v1/Systems/1/SecureBoot/SecureBootDatabases/{database}",
            get(database),
        )
        .route(
            "/redfish/v1/Systems/1/SecureBoot/SecureBootDatabases/{database}/Signatures",
            get(signatures),
        )
        .route(
            "/redfish/v1/Systems/1/SecureBoot/SecureBootDatabases/{database}/Signatures/{id}",
            get(signature),
        )
        .route(
            "/redfish/v1/Systems/1/SecureBoot/SecureBootDatabases/{database}/Certificates",
            get(certificates),
        )
        .route(
            "/redfish/v1/Systems/1/SecureBoot/SecureBootDatabases/{database}/Certificates/{id}",
            get(certificate),
        )
}
fn path(database: &str) -> String {
    format!("{DATABASES}/{database}")
}
fn valid_database(database: &str) -> bool {
    ["PK", "KEK", "db", "dbx"].contains(&database)
}
async fn snapshot(
    app: &AppState,
    headers: &HeaderMap,
) -> Result<pod::SecureBootDatabaseSnapshot, Failure> {
    principal(app, headers).await?;
    app.control
        .agent_secure_boot_databases()
        .await
        .map_err(|_| Failure::unavailable("Guest Secure Boot databases are unavailable."))
}
async fn selected(
    app: &AppState,
    headers: &HeaderMap,
    id: &str,
) -> Result<pod::SecureBootDatabaseInventory, Failure> {
    principal(app, headers).await?;
    if !valid_database(id) {
        return Err(Failure::not_found());
    }
    snapshot(app, headers)
        .await?
        .databases
        .into_iter()
        .find(|d| d.id == id)
        .ok_or_else(Failure::not_found)
}
fn database_resource(database: &pod::SecureBootDatabaseInventory) -> Value {
    json!({"@odata.id":path(&database.id),"@odata.type":"#SecureBootDatabase.v1_0_1.SecureBootDatabase",
        "Id":database.id,"DatabaseId":database.id,"Name":format!("UEFI {} database",database.id),
        "Certificates":link(&format!("{}/Certificates",path(&database.id))),
        "Signatures":link(&format!("{}/Signatures",path(&database.id))),
        "Oem":{"Qrab":{"@odata.type":"#QrabSecureBootDatabase.v1_0_0.QrabSecureBootDatabase",
            "Present":database.present,"Attributes":database.attributes,"ReadError":database.error,
            "UnsupportedSignatureTypes":database.unsupported_types}}})
}
fn signature_resource(database: &str, signature: &pod::SecureBootSignatureInventory) -> Value {
    json!({"@odata.id":format!("{}/Signatures/{}",path(database),signature.id),"@odata.type":"#Signature.v1_0_3.Signature",
        "Id":signature.id,"Name":signature.signature_type,"SignatureTypeRegistry":"UEFI",
        "SignatureType":signature.signature_type,"SignatureString":signature.data_hex.to_ascii_uppercase(),
        "UefiSignatureOwner":signature.owner_guid})
}
fn certificate_resource(
    database: &str,
    signature: &pod::SecureBootSignatureInventory,
) -> Option<Value> {
    let pem = signature.certificate_pem.as_ref()?;
    let fingerprint = signature
        .fingerprint_sha256
        .as_bytes()
        .chunks_exact(2)
        .map(|p| std::str::from_utf8(p).unwrap_or("").to_ascii_uppercase())
        .collect::<Vec<_>>()
        .join(":");
    Some(
        json!({"@odata.id":format!("{}/Certificates/{}",path(database),signature.id),"@odata.type":"#Certificate.v1_9_0.Certificate",
        "Id":signature.id,"Name":"UEFI X.509 certificate","CertificateType":"PEM","CertificateString":pem,
        "Fingerprint":fingerprint,"FingerprintHashAlgorithm":"TPM_ALG_SHA256","UefiSignatureOwner":signature.owner_guid}),
    )
}
async fn databases(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    let snapshot = snapshot(&app, &headers).await?;
    let mut resource = collection(
        DATABASES,
        "SecureBootDatabaseCollection",
        "UEFI Secure Boot databases",
        snapshot
            .databases
            .iter()
            .map(|d| link(&path(&d.id)))
            .collect(),
    );
    resource["Oem"] = json!({"Qrab":{"@odata.type":"#QrabSecureBootDatabases.v1_0_0.QrabSecureBootDatabases",
        "EfiRuntimeAvailable":snapshot.efi_runtime_available,"Observed":rfc3339(snapshot.observed_unix_ms)}});
    Ok(Json(resource))
}
async fn database(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Failure> {
    Ok(Json(database_resource(
        &selected(&app, &headers, &id).await?,
    )))
}
async fn signatures(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Failure> {
    let database = selected(&app, &headers, &id).await?;
    let uri = format!("{}/Signatures", path(&id));
    Ok(Json(collection(
        &uri,
        "SignatureCollection",
        "UEFI signatures and hashes",
        database
            .signatures
            .iter()
            .map(|s| link(&format!("{uri}/{}", s.id)))
            .collect(),
    )))
}
async fn signature(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path((database, id)): Path<(String, String)>,
) -> Result<Json<Value>, Failure> {
    selected(&app, &headers, &database)
        .await?
        .signatures
        .iter()
        .find(|s| s.id == id)
        .map(|s| Json(signature_resource(&database, s)))
        .ok_or_else(Failure::not_found)
}
async fn certificates(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Failure> {
    let database = selected(&app, &headers, &id).await?;
    let uri = format!("{}/Certificates", path(&id));
    Ok(Json(collection(
        &uri,
        "CertificateCollection",
        "UEFI public certificates",
        database
            .signatures
            .iter()
            .filter(|s| s.certificate_pem.is_some())
            .map(|s| link(&format!("{uri}/{}", s.id)))
            .collect(),
    )))
}
async fn certificate(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path((database, id)): Path<(String, String)>,
) -> Result<Json<Value>, Failure> {
    selected(&app, &headers, &database)
        .await?
        .signatures
        .iter()
        .find(|s| s.id == id)
        .and_then(|s| certificate_resource(&database, s))
        .map(Json)
        .ok_or_else(Failure::not_found)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash() -> pod::SecureBootSignatureInventory {
        pod::SecureBootSignatureInventory {
            id: "opaque-stable-id".into(),
            signature_type: "EFI_CERT_SHA256_GUID".into(),
            owner_guid: "12345678-1234-5678-90ab-cdef12345678".into(),
            data_hex: "ab".repeat(32),
            certificate_pem: None,
            fingerprint_sha256: "cd".repeat(32),
        }
    }
    #[test]
    fn native_signature_shape_preserves_owner_and_does_not_fabricate_a_certificate() {
        let item = hash();
        let signature = signature_resource("dbx", &item);
        assert_eq!(signature["SignatureTypeRegistry"], "UEFI");
        assert_eq!(signature["SignatureString"], "AB".repeat(32));
        assert_eq!(signature["UefiSignatureOwner"], item.owner_guid);
        assert!(certificate_resource("dbx", &item).is_none());
    }
    #[test]
    fn certificate_shape_contains_public_pem_and_fingerprint_only() {
        let mut item = hash();
        item.signature_type = "EFI_CERT_X509_GUID".into();
        item.certificate_pem = Some("PUBLIC-PEM".into());
        let certificate = certificate_resource("PK", &item).unwrap();
        assert_eq!(certificate["CertificateType"], "PEM");
        assert_eq!(certificate["Fingerprint"], vec!["CD"; 32].join(":"));
        assert!(certificate.get("Actions").is_none());
        assert!(certificate.get("PrivateKey").is_none());
    }
}

pub(super) const AUTH_UPDATE: &str =
    "/redfish/v1/Systems/1/SecureBoot/Actions/Oem/QrabSecureBoot.UpdateAuthenticatedDatabase";
const AUTH_BODY_LIMIT: usize = 192 * 1024;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthenticatedUpdateBody {
    #[serde(rename = "DatabaseId")]
    database_id: String,
    #[serde(rename = "PayloadBase64")]
    payload_base64: String,
    #[serde(default, rename = "Append")]
    append: bool,
}
fn authenticated_request(body: &Bytes) -> Result<pod::SecureBootUpdateRequest, Failure> {
    use base64::Engine;
    let malformed = || {
        Failure::new(StatusCode::BAD_REQUEST,"ActionParameterValueFormatError",
        "The authenticated database update is invalid.","Provide only DatabaseId, PayloadBase64 and optional Append using a signed EFI .auth payload of at most 128KiB.")
    };
    if body.len() > AUTH_BODY_LIMIT {
        return Err(malformed());
    }
    let request: AuthenticatedUpdateBody = serde_json::from_slice(body).map_err(|_| malformed())?;
    if !valid_database(&request.database_id)
        || request.payload_base64.len() > (128usize * 1024).div_ceil(3) * 4
    {
        return Err(malformed());
    }
    let payload = base64::engine::general_purpose::STANDARD
        .decode(request.payload_base64)
        .map_err(|_| malformed())?;
    if payload.len() > 128 * 1024 {
        return Err(malformed());
    }
    let request = pod::SecureBootUpdateRequest {
        database_id: request.database_id,
        payload_hex: payload.iter().map(|byte| format!("{byte:02x}")).collect(),
        append: request.append,
    };
    // Validate before sending, but never expose the signed payload or parser
    // details in a Redfish error. Firmware performs actual authorization.
    pod::validate_secure_boot_update(&request).map_err(|_| malformed())?;
    Ok(request)
}
async fn authenticated_update(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Failure> {
    admin(&app, &headers).await?;
    let request = authenticated_request(&body)?;
    let snapshot=app.control.agent_secure_boot_update(request).await.map_err(|_|Failure::new(
        StatusCode::CONFLICT,"OperationNotAllowed","The guest or firmware denied the authenticated update, or its outcome is unknown.",
        "Inspect current Secure Boot databases and kernel protection before another signed update; restoration failure is an unknown outcome, and firmware authorization is never bypassed."))?;
    Ok(Json(
        json!({"@odata.id":DATABASES,"@odata.type":"#QrabAuthenticatedDatabaseUpdate.v1_0_0.QrabAuthenticatedDatabaseUpdate",
        "Id":"AuthenticatedDatabaseUpdate","Name":"Firmware-authorized database update",
        "Observed":rfc3339(snapshot.observed_unix_ms),"DatabaseCount":snapshot.databases.len()}),
    ))
}

#[cfg(test)]
mod authenticated_tests {
    use super::*;
    fn large_request() -> Bytes {
        use base64::Engine;
        let cms = [
            0x30, 0x12, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x02, 0xa0,
            0x05, 0x30, 0x03, 0x02, 0x01, 0x01,
        ];
        let mut payload = vec![0xea, 0x07, 11, 14, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        payload.extend_from_slice(&(24u32 + cms.len() as u32).to_le_bytes());
        payload.extend_from_slice(&[0, 2, 0xf1, 0x0e]);
        payload.extend_from_slice(&[
            0x9d, 0xd2, 0xaf, 0x4a, 0xdf, 0x68, 0xee, 0x49, 0x8a, 0xa9, 0x34, 0x7d, 0x37, 0x56,
            0x65, 0xa7,
        ]);
        payload.extend_from_slice(&cms);
        payload.extend_from_slice(&[
            0x26, 0x16, 0xc4, 0xc1, 0x4c, 0x50, 0x92, 0x40, 0xac, 0xa9, 0x41, 0xf9, 0x36, 0x93,
            0x43, 0x28,
        ]);
        payload.extend_from_slice(&(28u32 + 48 * 128).to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(&48u32.to_le_bytes());
        for _ in 0..128 {
            payload.extend_from_slice(&[0; 16]);
            payload.extend_from_slice(&[7; 32]);
        }
        Bytes::from(json!({"DatabaseId":"dbx","PayloadBase64":base64::engine::general_purpose::STANDARD.encode(payload)}).to_string())
    }
    #[tokio::test]
    async fn authenticated_route_admits_bounded_payloads_above_default_four_kib() {
        use tower::ServiceExt;
        let body = large_request();
        assert!(body.len() > 4096);
        let router = Router::new()
            .route(
                "/auth",
                post(|body: Bytes| async move {
                    authenticated_request(&body).map(|_| StatusCode::NO_CONTENT)
                })
                .layer(DefaultBodyLimit::max(AUTH_BODY_LIMIT)),
            )
            .layer(DefaultBodyLimit::max(4096));
        let response = router
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/auth")
                    .body(axum::body::Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }
    #[test]
    fn signed_updates_cannot_select_arbitrary_names_or_echo_payloads_in_errors() {
        let mut body: Value = serde_json::from_slice(&large_request()).unwrap();
        body["DatabaseId"] = json!("../arbitrary");
        assert!(authenticated_request(&Bytes::from(body.to_string())).is_err());
        body["DatabaseId"] = json!("PK");
        body["Append"] = json!(true);
        assert!(authenticated_request(&Bytes::from(body.to_string())).is_err());
        body["DatabaseId"] = json!("db");
        body["Append"] = json!(false);
        body["PayloadBase64"] = json!("private-key-secret-marker");
        let error = authenticated_request(&Bytes::from(body.to_string())).unwrap_err();
        assert!(!error.message.contains("private-key-secret-marker"));
    }
}

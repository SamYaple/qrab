//! OData discovery for the implemented standard resources (DSP0266 section 8.4).
//! References select the namespaces actually emitted, without copying remote
//! schemas or downloading anything at request time. Qrab OEM schema definitions
//! are not published yet; this document deliberately does not invent them.
use crate::{has_credentials, principal, AppState, Failure};
use axum::{
    extract::State,
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use std::{fmt::Write, sync::OnceLock};

const METADATA: &str = "/redfish/v1/$metadata";
const ODATA: &str = "/redfish/v1/odata";

/// Standard resource namespaces emitted by this service. Versions refer to
/// namespace revisions, not a promise that remote publication files are immutable.
const VERSIONED: &[(&str, &str)] = &[
    ("AccountService", "v1_18_0"),
    ("ActionInfo", "v1_4_2"),
    ("BootOption", "v1_0_6"),
    ("Certificate", "v1_9_0"),
    ("Chassis", "v1_25_0"),
    ("ComputerSystem", "v1_22_0"),
    ("Drive", "v1_21_0"),
    ("DriveMetrics", "v1_2_0"),
    ("EthernetInterface", "v1_12_0"),
    ("Event", "v1_10_0"),
    ("EventDestination", "v1_15_0"),
    ("EventService", "v1_10_0"),
    ("LogEntry", "v1_17_0"),
    ("LogService", "v1_7_0"),
    ("Manager", "v1_19_0"),
    ("Memory", "v1_20_0"),
    ("Message", "v1_2_0"),
    ("MetricDefinition", "v1_3_5"),
    ("MetricReport", "v1_5_2"),
    ("MetricReportDefinition", "v1_4_4"),
    ("Processor", "v1_18_0"),
    ("Resource", "v1_0_0"),
    ("Role", "v1_3_0"),
    ("SecureBoot", "v1_1_0"),
    ("SecureBootDatabase", "v1_0_1"),
    ("ServiceRoot", "v1_17_0"),
    ("Session", "v1_8_0"),
    ("SessionService", "v1_1_9"),
    ("Settings", "v1_4_0"),
    ("Signature", "v1_0_3"),
    ("SimpleStorage", "v1_3_2"),
    ("SoftwareInventory", "v1_12_0"),
    ("Storage", "v1_18_0"),
    ("Task", "v1_7_4"),
    ("TaskService", "v1_2_1"),
    ("TelemetryService", "v1_4_0"),
    ("Triggers", "v1_4_0"),
    ("TrustedComponent", "v1_3_0"),
    ("UpdateService", "v1_16_0"),
    ("VirtualMedia", "v1_6_0"),
    ("Volume", "v1_10_0"),
];

const COLLECTIONS: &[&str] = &[
    "BootOptionCollection",
    "CertificateCollection",
    "ChassisCollection",
    "ComputerSystemCollection",
    "EthernetInterfaceCollection",
    "EventDestinationCollection",
    "LogEntryCollection",
    "LogServiceCollection",
    "ManagerCollection",
    "MemoryCollection",
    "MetricDefinitionCollection",
    "MetricReportCollection",
    "MetricReportDefinitionCollection",
    "ProcessorCollection",
    "RoleCollection",
    "SecureBootDatabaseCollection",
    "SessionCollection",
    "SignatureCollection",
    "SimpleStorageCollection",
    "SoftwareInventoryCollection",
    "StorageCollection",
    "TaskCollection",
    "TriggersCollection",
    "TrustedComponentCollection",
    "VirtualMediaCollection",
    "VolumeCollection",
];

// Redfish collections are themselves singleton resources in the OData model;
// DSP0266 requires kind=Singleton for service-document entries, not EntitySet.
const TOP_LEVEL: &[(&str, &str, &str)] = &[
    ("Service", "ServiceRoot.v1_17_0.ServiceRoot", "/redfish/v1/"),
    (
        "Systems",
        "ComputerSystemCollection.ComputerSystemCollection",
        "/redfish/v1/Systems",
    ),
    (
        "Managers",
        "ManagerCollection.ManagerCollection",
        "/redfish/v1/Managers",
    ),
    (
        "Chassis",
        "ChassisCollection.ChassisCollection",
        "/redfish/v1/Chassis",
    ),
    (
        "AccountService",
        "AccountService.v1_18_0.AccountService",
        "/redfish/v1/AccountService",
    ),
    (
        "SessionService",
        "SessionService.v1_1_9.SessionService",
        "/redfish/v1/SessionService",
    ),
    (
        "EventService",
        "EventService.v1_10_0.EventService",
        "/redfish/v1/EventService",
    ),
    (
        "TelemetryService",
        "TelemetryService.v1_4_0.TelemetryService",
        "/redfish/v1/TelemetryService",
    ),
    (
        "Tasks",
        "TaskService.v1_2_1.TaskService",
        "/redfish/v1/TaskService",
    ),
    (
        "UpdateService",
        "UpdateService.v1_16_0.UpdateService",
        "/redfish/v1/UpdateService",
    ),
];

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route(METADATA, get(metadata))
        .route(ODATA, get(odata))
}

async fn authorize(app: &AppState, headers: &HeaderMap) -> Result<(), Failure> {
    if !app.auth.public_discovery() || has_credentials(headers) {
        principal(app, headers).await?;
    }
    Ok(())
}

async fn metadata(State(app): State<AppState>, headers: HeaderMap) -> Result<Response, Failure> {
    authorize(&app, &headers).await?;
    Ok((
        [(header::CONTENT_TYPE, "application/xml;charset=utf-8")],
        metadata_document(),
    )
        .into_response())
}

async fn odata(State(app): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, Failure> {
    authorize(&app, &headers).await?;
    Ok(Json(service_document()))
}

fn service_document() -> Value {
    json!({"@odata.context":METADATA,"value":TOP_LEVEL.iter().map(|(name,_,url)| {
        json!({"name":name,"kind":"Singleton","url":url})
    }).collect::<Vec<_>>()})
}

fn metadata_document() -> &'static str {
    static DOCUMENT: OnceLock<String> = OnceLock::new();
    DOCUMENT.get_or_init(|| {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<edmx:Edmx xmlns:edmx=\"http://docs.oasis-open.org/odata/ns/edmx\" Version=\"4.0\">\n");
        for (name, version) in VERSIONED {
            writeln!(xml, "  <edmx:Reference Uri=\"https://redfish.dmtf.org/schemas/v1/{name}_v1.xml\"><edmx:Include Namespace=\"{name}\"/><edmx:Include Namespace=\"{name}.{version}\"/></edmx:Reference>").unwrap();
        }
        for name in COLLECTIONS {
            writeln!(xml, "  <edmx:Reference Uri=\"https://redfish.dmtf.org/schemas/v1/{name}_v1.xml\"><edmx:Include Namespace=\"{name}\"/></edmx:Reference>").unwrap();
        }
        xml.push_str("  <edmx:Reference Uri=\"https://redfish.dmtf.org/schemas/v1/RedfishExtensions_v1.xml\"><edmx:Include Namespace=\"RedfishExtensions.v1_0_0\" Alias=\"Redfish\"/><edmx:Include Namespace=\"Validation.v1_0_0\" Alias=\"Validation\"/></edmx:Reference>\n");
        xml.push_str("  <!-- Qrab OEM schema definitions are not yet published. -->\n  <edmx:DataServices>\n    <Schema xmlns=\"http://docs.oasis-open.org/odata/ns/edm\" Namespace=\"QrabService\">\n      <EntityContainer Name=\"Service\">\n");
        for (name, resource_type, _) in TOP_LEVEL {
            writeln!(xml, "        <Singleton Name=\"{name}\" Type=\"{resource_type}\"/>").unwrap();
        }
        xml.push_str("      </EntityContainer>\n    </Schema>\n  </edmx:DataServices>\n</edmx:Edmx>\n");
        xml
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_document_has_only_actual_top_level_resources() {
        let doc = service_document();
        assert_eq!(doc["@odata.context"], METADATA);
        assert_eq!(doc["value"][0]["url"], "/redfish/v1/");
        for resource in doc["value"].as_array().unwrap() {
            assert_eq!(resource["kind"], "Singleton");
            assert!(!matches!(
                resource["name"].as_str().unwrap(),
                "Fabrics" | "Registries" | "JsonSchemas" | "Accounts"
            ));
        }
        let xml = metadata_document();
        assert!(xml.contains("<EntityContainer Name=\"Service\">"));
        assert!(!xml.contains(" Extends="));
        assert!(xml.contains("Type=\"ServiceRoot.v1_17_0.ServiceRoot\""));
        assert!(xml.ends_with("</edmx:Edmx>\n"));
    }

    #[test]
    fn metadata_covers_every_literal_standard_type_emitted_by_source() {
        let xml = metadata_document();
        for entry in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let source = std::fs::read_to_string(path).unwrap();
            for literal in source.split('"').skip(1).step_by(2) {
                if literal.len() > "Collection".len()
                    && literal.ends_with("Collection")
                    && literal.bytes().all(|c| c.is_ascii_alphanumeric())
                {
                    assert!(
                        xml.contains(&format!("Namespace=\"{literal}\"")),
                        "missing metadata collection namespace {literal}"
                    );
                }
            }
            for fragment in source.split('#').skip(1) {
                let Some((namespace, rest)) = fragment.split_once('.') else {
                    continue;
                };
                if namespace.starts_with("Qrab")
                    || !namespace.bytes().all(|c| c.is_ascii_alphanumeric())
                {
                    continue;
                }
                let Some((version, _)) = rest.split_once('.') else {
                    continue;
                };
                if !version.starts_with('v')
                    || !version[1..]
                        .bytes()
                        .all(|c| c.is_ascii_digit() || c == b'_')
                {
                    continue;
                }
                assert!(
                    xml.contains(&format!("Namespace=\"{namespace}.{version}\"")),
                    "missing metadata namespace {namespace}.{version}"
                );
            }
        }
    }
}

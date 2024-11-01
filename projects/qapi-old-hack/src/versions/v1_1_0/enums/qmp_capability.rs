use serde::Serialize;

#[derive(Debug, Serialize)]
pub enum QMPCapability {
    #[serde(rename = "oob")]
    OOB,
}

use serde::{Deserialize, Serialize};
//use chrono::DateTime;

#[derive(Debug, Deserialize)]
#[serde(tag = "event")]
pub struct Event {
    pub timestamp: Timestamp,
    pub event: String,
}

#[derive(Debug, Deserialize)]
pub struct Timestamp {
    pub seconds: i64,
    pub microseconds: u32,
}

//impl std::fmt::Display for Event {
//    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//        let t = DateTime::from_timestamp(self.timestamp.seconds, self.timestamp.microseconds)
//            .expect("Failed to convert to timestamp");
//        write!(f, "Event: {}, Timestamp: {}", self.event, t).unwrap();
//        Ok(())
//    }
//}

#[derive(Serialize, Deserialize, Debug)]
pub struct QOMListArgs {
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct QOMListTypesArgs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implements: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#abstract: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "execute")]
pub enum QMPCommand {
    #[serde(rename = "qmp_capabilities")]
    QmpCapabilities,
    #[serde(rename = "query-cpus-fast")]
    QueryCpusFast,
    #[serde(rename = "system_reset")]
    SystemReset,
    #[serde(rename = "system_powerdown")]
    SystemPowerdown,
    #[serde(rename = "query-machines")]
    QueryMachines,
    #[serde(rename = "query-status")]
    QueryStatus,
    #[serde(rename = "query-current-machine")]
    QueryCurrentMachine,
    #[serde(rename = "x-exit-preconfig")]
    XExitPreconfig,
    #[serde(rename = "qom-list")]
    QOMList { arguments: QOMListArgs },
    #[serde(rename = "qom-list-types")]
    QOMListTypes { arguments: QOMListTypesArgs },
}

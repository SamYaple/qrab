use serde::{Deserialize, Serialize};
use surrealdb::sql::{Datetime, Thing};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Event {
    pub id: Thing,
    pub created_at: Datetime,
    pub msg: String,
    pub timestamp: Datetime,
}

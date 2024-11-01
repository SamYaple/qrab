use serde::{Deserialize, Serialize};
use surrealdb::sql::{Datetime, Thing};

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
pub struct VM {
    pub id: Thing,
    pub created_at: Datetime,
    pub name: String,
}

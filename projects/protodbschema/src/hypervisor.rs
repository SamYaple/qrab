use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use surrealdb::sql::{Datetime, Thing};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Hypervisor {
    pub id: Thing,
    pub created_at: Datetime,
    pub hostname: Hostname,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Hostname(String);
impl Hostname {
    pub fn new(name: String) -> Result<Self> {
        if Hostname::is_valid(&name) {
            Ok(Self(name))
        } else {
            Err(anyhow! {"Invalid hostname provided"})
        }
    }

    // RFC 952 -- RFC 1123 -- RFC 2181
    pub fn is_valid(name: &str) -> bool {
        // The "root label" is does not count toward total length
        // Trim trailing root label, `.`, if it exists
        let name = if name.ends_with('.') {
            &name[..name.len() - 1]
        } else {
            name
        };

        if name.len() > 253 {
            return false;
        }
        let labels: Vec<&str> = name.split('.').collect();
        if labels.is_empty() {
            return false;
        }
        for label in labels {
            if label.len() > 63 || label.is_empty() {
                return false;
            }

            if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return false;
            }

            if label.starts_with('-') || label.ends_with('-') {
                return false;
            }
        }
        if name.chars().next().unwrap_or(' ').is_digit(10) {
            return false;
        }
        true
    }
}

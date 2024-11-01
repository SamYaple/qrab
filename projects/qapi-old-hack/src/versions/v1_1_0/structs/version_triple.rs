use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct VersionTriple {
    major: i32,
    minor: i32,
    micro: i32,
}

impl VersionTriple {
    pub fn major(&self) -> i32 { self.major }
    pub fn minor(&self) -> i32 { self.minor }
    pub fn micro(&self) -> i32 { self.micro }
}

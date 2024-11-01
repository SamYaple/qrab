use serde::Serialize;

use super::enums::QMPCapability;

#[derive(Debug, Serialize)]
pub struct QmpCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    capabilities: Option<Vec<QMPCapability>>,
}

impl crate::traits::Command for QmpCapabilities {
    fn name() -> &'static str { "qmp_capabilities" }
    fn skip_serializing_arguments(&self) -> bool { self.capabilities.is_none() }
}

#[derive(Debug)]
pub struct Builder {
    capabilities: Option<Vec<QMPCapability>>,
}

impl Builder {
    pub fn new() -> Self {
        Self { capabilities: None }
    }

    pub fn capabilities(mut self, arg: Vec<QMPCapability>) -> Self {
        self.capabilities = Some(arg);
        self
    }

    pub fn build(self) -> QmpCapabilities {
        QmpCapabilities {
            capabilities: self.capabilities
        }
    }
}

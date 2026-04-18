use serde::{Deserialize, Serialize};

/// Panel types available in the dashboard layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanelType {
    SecurityHero,
    Attestation,
    PowerControls,
    Config,
    Console,
    Serial,
    Events,
    CpuInfo,
    Status,
}

/// Panel width in the grid layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanelWidth {
    Full,
    Half,
    Third,
}

impl PanelWidth {
    pub fn css_class(&self) -> &'static str {
        match self {
            PanelWidth::Full => "panel-full",
            PanelWidth::Half => "panel-half",
            PanelWidth::Third => "panel-third",
        }
    }
}

/// Configuration for a single panel in the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelConfig {
    pub panel: PanelType,
    pub width: PanelWidth,
    pub collapsed: bool,
}

/// Configurable dashboard layout. Panels render in order, respecting width.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardLayout {
    pub panels: Vec<PanelConfig>,
}

impl Default for DashboardLayout {
    fn default() -> Self {
        Self {
            panels: vec![
                PanelConfig {
                    panel: PanelType::SecurityHero,
                    width: PanelWidth::Full,
                    collapsed: false,
                },
                PanelConfig {
                    panel: PanelType::Attestation,
                    width: PanelWidth::Full,
                    collapsed: false,
                },
                PanelConfig {
                    panel: PanelType::PowerControls,
                    width: PanelWidth::Third,
                    collapsed: false,
                },
                PanelConfig {
                    panel: PanelType::Status,
                    width: PanelWidth::Third,
                    collapsed: false,
                },
                PanelConfig {
                    panel: PanelType::CpuInfo,
                    width: PanelWidth::Third,
                    collapsed: false,
                },
                PanelConfig {
                    panel: PanelType::Config,
                    width: PanelWidth::Full,
                    collapsed: false,
                },
                PanelConfig {
                    panel: PanelType::Console,
                    width: PanelWidth::Full,
                    collapsed: false,
                },
                PanelConfig {
                    panel: PanelType::Serial,
                    width: PanelWidth::Full,
                    collapsed: false,
                },
                PanelConfig {
                    panel: PanelType::Events,
                    width: PanelWidth::Full,
                    collapsed: false,
                },
            ],
        }
    }
}

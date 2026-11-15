//! `VpnProfile`: one Proton connection for one VPN NIC.
//!
//! The broker keeps Secret `vpn-<profile>` (key `wg.conf`) for it; a NIC uses
//! it with `vpn: {secretName: vpn-<profile>}`. One profile is one key and one
//! Proton connection, so it serves one NIC at a time.
use crate::catalog::Feature;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[kube(
    group = "qrab.yapplesauce.com",
    version = "v1alpha1",
    kind = "VpnProfile",
    namespaced,
    status = "VpnProfileStatus",
    shortname = "vpnp",
    printcolumn = r#"{"name":"Server","type":"string","jsonPath":".status.server"}"#,
    printcolumn = r#"{"name":"Country","type":"string","jsonPath":".status.exitCountry"}"#,
    printcolumn = r#"{"name":"Load","type":"integer","jsonPath":".status.load"}"#,
    printcolumn = r#"{"name":"Ready","type":"string","jsonPath":".status.conditions[?(@.type==\"Ready\")].status"}"#
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VpnProfileSpec {
    pub proton: ProtonSpec,
}

/// Which Proton server, and the connection features bound to the key.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProtonSpec {
    /// A Proton server name (`CH#12`, `US-GA#445`). Without it the broker
    /// picks the best eligible server and keeps it while it stays eligible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(regex(pattern = r"^[A-Za-z0-9-]{2,24}#[0-9]{1,6}(-[A-Za-z]{1,8})?$"))]
    pub server: Option<String>,
    /// Exit country (ISO 3166 alpha-2) for automatic selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(regex(pattern = r"^[A-Za-z]{2}$"))]
    pub country: Option<String>,
    /// City for automatic selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 64))]
    pub city: Option<String>,
    /// Required server features. Secure Core and Tor servers are used only
    /// when listed here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(length(max = 5))]
    pub features: Vec<Feature>,
    /// NetShield DNS filtering: 0 off, 1 malware, 2 malware, ads and trackers.
    #[serde(default = "net_shield_default")]
    #[schemars(range(min = 0, max = 2))]
    pub net_shield: u8,
    /// Forward one public port to the guest (P2P servers only; qrab-netd
    /// renews the mapping).
    #[serde(default)]
    pub port_forwarding: bool,
    /// Moderate NAT, for games and calls that need it.
    #[serde(default)]
    pub moderate_nat: bool,
    /// VPN Accelerator.
    #[serde(default = "yes")]
    pub vpn_accelerator: bool,
}

fn net_shield_default() -> u8 {
    1
}
fn yes() -> bool {
    true
}

impl ProtonSpec {
    pub fn criteria(&self) -> crate::catalog::Criteria {
        crate::catalog::Criteria {
            server: self.server.clone(),
            country: self.country.clone(),
            city: self.city.clone(),
            features: self.features.clone(),
            port_forwarding: self.port_forwarding,
        }
    }

    pub fn connection_features(&self) -> crate::wg::Features {
        crate::wg::Features {
            net_shield: self.net_shield.min(2),
            port_forwarding: self.port_forwarding,
            moderate_nat: self.moderate_nat,
            vpn_accelerator: self.vpn_accelerator,
        }
    }
}

/// A standard condition (`Ready`).
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    #[serde(rename = "type")]
    pub type_: String,
    pub status: String,
    pub reason: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct VpnProfileStatus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,
    /// The Secret a NIC references: `vpn-<profile>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logical_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physical_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_country: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_country: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub server_features: Vec<Feature>,
    /// The WireGuard public key the certificate covers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    /// Unix seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate_expires: Option<i64>,
    /// Unix seconds: when the broker renews the certificate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate_renews: Option<i64>,
    /// The connection features the certificate binds, as certified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certified_features: Option<crate::wg::Features>,
    /// SHA-256 of the applied wg.conf, so unchanged configs are not rewritten.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,
}

impl VpnProfileStatus {
    pub fn ready(&self) -> Option<&Condition> {
        self.conditions.iter().find(|c| c.type_ == "Ready")
    }
}

/// The Secret a profile's NIC references.
///
/// ```
/// assert_eq!(qrab_proton::crd::secret_name("ch-p2p"), "vpn-ch-p2p");
/// ```
pub fn secret_name(profile: &str) -> String {
    format!("vpn-{profile}")
}

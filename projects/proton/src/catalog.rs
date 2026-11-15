//! Proton's server catalog and how a profile picks a server.
//!
//! A *logical* server (`CH#12`) is what users choose; it has one or more
//! *physical* servers, each with its own entry IP and WireGuard key. Auto
//! selection filters by tier, status, features and location, then takes the
//! lowest Score, Proton's own ranking. A working choice is kept while it stays
//! eligible: a tunnel is never moved just because scores shifted.
//!
//! ```
//! use qrab_proton::catalog::{Catalog, Criteria, Feature};
//! let catalog: Catalog = serde_json::from_str(r#"{"LogicalServers": [
//!   {"ID": "a", "Name": "CH#1", "EntryCountry": "CH", "ExitCountry": "CH", "City": "Zurich",
//!    "Tier": 2, "Features": 4, "Status": 1, "Load": 40, "Score": 1.2,
//!    "Servers": [{"ID": "a1", "EntryIP": "185.159.157.1", "X25519PublicKey": "xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=", "Status": 1}]},
//!   {"ID": "b", "Name": "CH#2", "EntryCountry": "CH", "ExitCountry": "CH", "City": "Zurich",
//!    "Tier": 2, "Features": 0, "Status": 1, "Load": 10, "Score": 0.9,
//!    "Servers": [{"ID": "b1", "EntryIP": "185.159.157.2", "X25519PublicKey": "xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=", "Status": 1}]}
//! ]}"#).unwrap();
//! // Auto: the best score.
//! let best = catalog.select(&Criteria::default(), 2, None).unwrap();
//! assert_eq!(best.name, "CH#2");
//! // Port forwarding needs a P2P server.
//! let p2p = catalog.select(&Criteria { port_forwarding: true, ..Default::default() }, 2, None).unwrap();
//! assert_eq!(p2p.name, "CH#1");
//! assert!(p2p.features & Feature::P2p.bit() != 0);
//! ```
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

/// Logical-server feature bits, as Proton numbers them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Feature {
    SecureCore,
    Tor,
    P2p,
    Streaming,
    Ipv6,
}

impl Feature {
    pub const ALL: [Feature; 5] = [
        Feature::SecureCore,
        Feature::Tor,
        Feature::P2p,
        Feature::Streaming,
        Feature::Ipv6,
    ];

    pub fn bit(self) -> u32 {
        match self {
            Feature::SecureCore => 1,
            Feature::Tor => 2,
            Feature::P2p => 4,
            Feature::Streaming => 8,
            Feature::Ipv6 => 16,
        }
    }

    /// The features set in `bits`.
    pub fn of(bits: u32) -> Vec<Feature> {
        Self::ALL
            .into_iter()
            .filter(|f| bits & f.bit() != 0)
            .collect()
    }
}

/// A physical server behind a logical one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Physical {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "EntryIP")]
    pub entry_ip: String,
    #[serde(rename = "X25519PublicKey", default)]
    pub x25519_public_key: Option<String>,
    #[serde(default)]
    pub status: u8,
}

/// A logical server, the unit users choose.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Logical {
    #[serde(rename = "ID")]
    pub id: String,
    pub name: String,
    pub entry_country: String,
    pub exit_country: String,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub tier: u8,
    #[serde(default)]
    pub features: u32,
    #[serde(default)]
    pub status: u8,
    #[serde(default)]
    pub load: u8,
    #[serde(default)]
    pub score: f64,
    #[serde(default)]
    pub servers: Vec<Physical>,
}

/// One `/vpn/v1/loads` row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Load {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(default)]
    pub load: u8,
    #[serde(default)]
    pub score: f64,
    #[serde(default)]
    pub status: Option<u8>,
}

/// The whole catalog, as `/vpn/v1/logicals` returns it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Catalog {
    #[serde(rename = "LogicalServers", default)]
    pub servers: Vec<Logical>,
}

/// What a profile asks for. `server` names a logical server (manual); without
/// it the best eligible server matching the rest is chosen (auto).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Criteria {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// Exit country, ISO 3166 alpha-2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    /// Required features. Secure Core and Tor servers are chosen only when
    /// asked for, since they change the route, not just add to it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<Feature>,
    /// Port forwarding works only on P2P servers.
    #[serde(default)]
    pub port_forwarding: bool,
}

/// The current choice, kept while it stays eligible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Current<'a> {
    pub logical: &'a str,
    pub physical: &'a str,
}

/// A selected server: everything a `wg.conf` and a status need.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub name: String,
    pub logical_id: String,
    pub physical_id: String,
    pub entry_ip: Ipv4Addr,
    pub public_key: String,
    pub entry_country: String,
    pub exit_country: String,
    pub city: Option<String>,
    pub load: u8,
    pub score: f64,
    pub features: u32,
    pub tier: u8,
}

/// A WireGuard key: 32 bytes in standard base64.
fn is_key(value: &str) -> bool {
    value.len() == 44
        && value.ends_with('=')
        && value[..43]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
}

impl Physical {
    /// Usable for WireGuard: up, an IPv4 entry and a well-formed key.
    fn usable(&self) -> Option<(Ipv4Addr, &str)> {
        let key = self.x25519_public_key.as_deref().filter(|k| is_key(k))?;
        let ip = self.entry_ip.parse().ok()?;
        (self.status == 1).then_some((ip, key))
    }
}

impl Logical {
    /// Why this server cannot serve `criteria` on an account of `max_tier`,
    /// or `None` when it can. Location filters are not checked here.
    pub fn refusal(&self, criteria: &Criteria, max_tier: u8) -> Option<String> {
        let name = &self.name;
        if self.tier > max_tier {
            return Some(format!(
                "{name} needs tier {}, the account has {max_tier}",
                self.tier
            ));
        }
        if self.status != 1 {
            return Some(format!("{name} is in maintenance"));
        }
        if !self.servers.iter().any(|s| s.usable().is_some()) {
            return Some(format!("{name} has no WireGuard server up"));
        }
        let mut wanted: Vec<Feature> = criteria.features.clone();
        if criteria.port_forwarding {
            wanted.push(Feature::P2p);
        }
        for feature in &wanted {
            if self.features & feature.bit() == 0 {
                let why = if *feature == Feature::P2p && criteria.port_forwarding {
                    " (port forwarding needs it)"
                } else {
                    ""
                };
                return Some(format!("{name} lacks {feature:?}{why}"));
            }
        }
        for route in [Feature::SecureCore, Feature::Tor] {
            if self.features & route.bit() != 0 && !wanted.contains(&route) {
                return Some(format!(
                    "{name} is a {route:?} server, which was not asked for"
                ));
            }
        }
        None
    }

    fn located(&self, criteria: &Criteria) -> bool {
        let same = |want: &Option<String>, have: Option<&str>| {
            want.as_deref()
                .is_none_or(|w| have.is_some_and(|h| h.eq_ignore_ascii_case(w)))
        };
        same(&criteria.country, Some(&self.exit_country))
            && same(&criteria.city, self.city.as_deref())
    }

    fn choose(&self, keep: Option<&str>) -> Option<Choice> {
        let physical = keep
            .and_then(|id| {
                self.servers
                    .iter()
                    .find(|s| s.id == id && s.usable().is_some())
            })
            .or_else(|| self.servers.iter().find(|s| s.usable().is_some()))?;
        let (entry_ip, key) = physical.usable()?;
        Some(Choice {
            name: self.name.clone(),
            logical_id: self.id.clone(),
            physical_id: physical.id.clone(),
            entry_ip,
            public_key: key.to_owned(),
            entry_country: self.entry_country.clone(),
            exit_country: self.exit_country.clone(),
            city: self.city.clone(),
            load: self.load,
            score: self.score,
            features: self.features,
            tier: self.tier,
        })
    }
}

impl Catalog {
    /// Fold fresh loads and scores (and status, when given) into the catalog.
    pub fn apply_loads(&mut self, loads: &[Load]) {
        let by_id: std::collections::HashMap<&str, &Load> =
            loads.iter().map(|l| (l.id.as_str(), l)).collect();
        for logical in &mut self.servers {
            if let Some(load) = by_id.get(logical.id.as_str()) {
                logical.load = load.load;
                logical.score = load.score;
                if let Some(status) = load.status {
                    logical.status = status;
                }
            }
        }
    }

    pub fn find(&self, name: &str) -> Option<&Logical> {
        self.servers
            .iter()
            .find(|l| l.name.eq_ignore_ascii_case(name))
    }

    /// Every server eligible for `criteria`, best score first (ties by name).
    pub fn eligible(&self, criteria: &Criteria, max_tier: u8) -> Vec<&Logical> {
        let mut found: Vec<&Logical> = self
            .servers
            .iter()
            .filter(|l| l.located(criteria) && l.refusal(criteria, max_tier).is_none())
            .collect();
        found.sort_by(|a, b| {
            a.score
                .total_cmp(&b.score)
                .then_with(|| a.name.cmp(&b.name))
        });
        found
    }

    /// Pick a server for `criteria`, keeping `current` while it is eligible.
    pub fn select(
        &self,
        criteria: &Criteria,
        max_tier: u8,
        current: Option<Current<'_>>,
    ) -> Result<Choice, String> {
        if let Some(name) = &criteria.server {
            let logical = self
                .find(name)
                .ok_or_else(|| format!("no Proton server is named {name}"))?;
            if let Some(why) = logical.refusal(criteria, max_tier) {
                return Err(why);
            }
            let keep = current
                .filter(|c| c.logical == logical.id)
                .map(|c| c.physical);
            return logical
                .choose(keep)
                .ok_or_else(|| format!("{name} has no WireGuard server up"));
        }
        if let Some(current) = current {
            if let Some(logical) = self.servers.iter().find(|l| {
                l.id == current.logical
                    && l.located(criteria)
                    && l.refusal(criteria, max_tier).is_none()
            }) {
                if let Some(choice) = logical.choose(Some(current.physical)) {
                    return Ok(choice);
                }
            }
        }
        self.eligible(criteria, max_tier)
            .first()
            .and_then(|l| l.choose(None))
            .ok_or_else(|| "no Proton server matches the profile".to_owned())
    }
}

/// One server as the dashboard shows it: no entry IPs or keys.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompactServer {
    #[serde(rename = "n")]
    pub name: String,
    #[serde(rename = "e")]
    pub entry_country: String,
    #[serde(rename = "x")]
    pub exit_country: String,
    #[serde(rename = "c", default, skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(rename = "t")]
    pub tier: u8,
    #[serde(rename = "f")]
    pub features: u32,
    #[serde(rename = "s")]
    pub status: u8,
    #[serde(rename = "l")]
    pub load: u8,
    #[serde(rename = "r")]
    pub score: f64,
}

/// The catalog the broker publishes in ConfigMap `proton-catalog`
/// (gzip-compressed JSON under `catalog.json.gz`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactCatalog {
    /// Unix seconds.
    pub updated: i64,
    pub max_tier: u8,
    pub servers: Vec<CompactServer>,
}

impl Catalog {
    /// The servers an account of `max_tier` can use, without addresses or keys.
    pub fn compact(&self, updated: i64, max_tier: u8) -> CompactCatalog {
        CompactCatalog {
            updated,
            max_tier,
            servers: self
                .servers
                .iter()
                .filter(|l| l.tier <= max_tier)
                .map(|l| CompactServer {
                    name: l.name.clone(),
                    entry_country: l.entry_country.clone(),
                    exit_country: l.exit_country.clone(),
                    city: l.city.clone(),
                    tier: l.tier,
                    features: l.features,
                    status: l.status,
                    load: l.load,
                    // Two decimals are all a ranking needs.
                    score: (l.score * 100.0).round() / 100.0,
                })
                .collect(),
        }
    }
}

/// Largest compressed catalog published: a ConfigMap holds at most 1 MiB,
/// and binaryData is base64 (4/3).
pub const MAX_ENCODED: usize = 700 * 1024;

pub fn encode(catalog: &CompactCatalog) -> Result<Vec<u8>, String> {
    use std::io::Write;
    let json = serde_json::to_vec(catalog).map_err(|e| e.to_string())?;
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    gz.write_all(&json).map_err(|e| e.to_string())?;
    let bytes = gz.finish().map_err(|e| e.to_string())?;
    if bytes.len() > MAX_ENCODED {
        return Err(format!(
            "the catalog compresses to {} bytes, over {MAX_ENCODED}",
            bytes.len()
        ));
    }
    Ok(bytes)
}

/// Read what [`encode`] wrote; bounded, so a hostile ConfigMap cannot
/// balloon the reader.
pub fn decode(bytes: &[u8]) -> Result<CompactCatalog, String> {
    use std::io::Read;
    let mut json = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .take(64 * 1024 * 1024)
        .read_to_end(&mut json)
        .map_err(|e| e.to_string())?;
    serde_json::from_slice(&json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=";

    fn logical(name: &str, tier: u8, features: u32, status: u8, score: f64) -> Logical {
        let id = name.replace('#', "-");
        Logical {
            servers: vec![
                Physical {
                    id: format!("{id}-1"),
                    entry_ip: "203.0.113.1".into(),
                    x25519_public_key: Some(KEY.into()),
                    status: 1,
                },
                Physical {
                    id: format!("{id}-2"),
                    entry_ip: "203.0.113.2".into(),
                    x25519_public_key: Some(KEY.into()),
                    status: 1,
                },
            ],
            id,
            name: name.into(),
            entry_country: name[..2].into(),
            exit_country: name[..2].into(),
            city: Some("City".into()),
            tier,
            features,
            status,
            load: 50,
            score,
        }
    }

    fn catalog() -> Catalog {
        Catalog {
            servers: vec![
                logical("CH#1", 2, Feature::P2p.bit(), 1, 2.0),
                logical("CH#2", 2, 0, 1, 1.0),
                logical("CH#3", 2, 0, 0, 0.1),
                logical("SE#1", 0, Feature::P2p.bit(), 1, 0.5),
                logical("US#1", 3, 0, 1, 0.05),
                logical("IS-CH#1", 2, Feature::SecureCore.bit(), 1, 0.01),
                logical("CH#9", 2, Feature::Tor.bit(), 1, 0.02),
            ],
        }
    }

    #[test]
    fn auto_takes_the_best_eligible_score() {
        let c = catalog();
        let all = Criteria::default();
        // US#1 (tier 3), IS-CH#1 (Secure Core), CH#9 (Tor) and CH#3 (down) are out.
        assert_eq!(c.select(&all, 2, None).unwrap().name, "SE#1");
        let ch = Criteria {
            country: Some("ch".into()),
            ..Default::default()
        };
        assert_eq!(c.select(&ch, 2, None).unwrap().name, "CH#2");
        let pf = Criteria {
            country: Some("CH".into()),
            port_forwarding: true,
            ..Default::default()
        };
        assert_eq!(c.select(&pf, 2, None).unwrap().name, "CH#1");
        let sc = Criteria {
            features: vec![Feature::SecureCore],
            ..Default::default()
        };
        assert_eq!(c.select(&sc, 2, None).unwrap().name, "IS-CH#1");
        let none = Criteria {
            country: Some("JP".into()),
            ..Default::default()
        };
        assert!(c.select(&none, 2, None).is_err());
        // Tier 0 sees only free servers.
        assert_eq!(c.select(&all, 0, None).unwrap().name, "SE#1");
        // Ties break by name.
        let mut tied = catalog();
        tied.servers.iter_mut().for_each(|l| l.score = 1.0);
        assert_eq!(tied.select(&all, 2, None).unwrap().name, "CH#1");
    }

    #[test]
    fn a_working_choice_is_kept_until_it_is_no_longer_eligible() {
        let mut c = catalog();
        let all = Criteria::default();
        let current = Current {
            logical: "CH-1",
            physical: "CH-1-2",
        };
        let kept = c.select(&all, 2, Some(current.clone())).unwrap();
        assert_eq!(
            (kept.name.as_str(), kept.physical_id.as_str()),
            ("CH#1", "CH-1-2")
        );
        assert_eq!(kept.entry_ip, Ipv4Addr::new(203, 0, 113, 2));
        // Its physical server goes down: same logical, the other physical.
        c.servers[0].servers[1].status = 0;
        let moved = c.select(&all, 2, Some(current.clone())).unwrap();
        assert_eq!(
            (moved.name.as_str(), moved.physical_id.as_str()),
            ("CH#1", "CH-1-1")
        );
        // The logical goes into maintenance: the best other server.
        c.apply_loads(&[Load {
            id: "CH-1".into(),
            load: 0,
            score: 0.0,
            status: Some(0),
        }]);
        assert_eq!(c.select(&all, 2, Some(current)).unwrap().name, "SE#1");
    }

    #[test]
    fn a_named_server_is_used_or_refused_with_the_reason() {
        let c = catalog();
        let named = |server: &str, port_forwarding: bool| Criteria {
            server: Some(server.into()),
            port_forwarding,
            ..Default::default()
        };
        assert_eq!(
            c.select(&named("ch#2", false), 2, None).unwrap().name,
            "CH#2"
        );
        // A manual choice ignores the location filters.
        let mut elsewhere = named("CH#2", false);
        elsewhere.country = Some("SE".into());
        assert_eq!(c.select(&elsewhere, 2, None).unwrap().name, "CH#2");
        for (criteria, reason) in [
            (named("CH#2", true), "port forwarding needs it"),
            (named("CH#3", false), "maintenance"),
            (named("US#1", false), "needs tier 3"),
            (named("XX#1", false), "no Proton server is named"),
        ] {
            let error = c.select(&criteria, 2, None).unwrap_err();
            assert!(error.contains(reason), "{reason}: {error}");
        }
    }

    #[test]
    fn a_full_size_catalog_fits_a_configmap_and_round_trips() {
        // Proton listed 18832 logical servers on 2026-11-15.
        let countries = ["US", "CH", "SE", "DE", "NL", "JP", "GB", "CA", "FR", "IS"];
        let servers = (0..20_000)
            .map(|i| {
                let country = countries[i % countries.len()];
                let mut l = logical(
                    &format!("{country}#{i}"),
                    (i % 3) as u8,
                    (i % 32) as u32,
                    1,
                    0.0,
                );
                l.score = (i % 997) as f64 / 7.0;
                l.load = (i % 100) as u8;
                l.city = Some(format!("City {}", i % 400));
                l
            })
            .collect();
        let compact = Catalog { servers }.compact(1, 2);
        let bytes = encode(&compact).unwrap();
        assert!(bytes.len() < MAX_ENCODED, "{} bytes", bytes.len());
        assert_eq!(decode(&bytes).unwrap(), compact);
        // No address or key leaves for the dashboard.
        let json = String::from_utf8(serde_json::to_vec(&compact).unwrap()).unwrap();
        assert!(!json.contains("203.0.113") && !json.contains(KEY));
        assert!(decode(b"not gzip").is_err());
    }

    #[test]
    fn unusable_physical_servers_are_skipped() {
        let mut c = catalog();
        for (field, value) in [
            ("ip", "2001:db8::1"),
            ("key", "short"),
            ("ip", "host.example"),
        ] {
            let server = &mut c.servers[1].servers[0];
            match field {
                "ip" => server.entry_ip = value.into(),
                _ => server.x25519_public_key = Some(value.into()),
            }
            let choice = c
                .select(
                    &Criteria {
                        server: Some("CH#2".into()),
                        ..Default::default()
                    },
                    2,
                    None,
                )
                .unwrap();
            assert_eq!(choice.physical_id, "CH-2-2", "{field}={value}");
        }
        c.servers[1].servers[1].status = 0;
        assert!(c
            .select(
                &Criteria {
                    server: Some("CH#2".into()),
                    ..Default::default()
                },
                2,
                None
            )
            .unwrap_err()
            .contains("no WireGuard server up"));
    }
}

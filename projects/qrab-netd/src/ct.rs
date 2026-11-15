//! Conntrack handoff for live migration (Cilium fork P4, done from the
//! node). Cilium's conntrack is per node: after a NIC moves, its established
//! flows have no entries on the new node and are judged as new, so a reply or
//! server push the security groups would not admit as new is dropped (and a
//! flow whose other side stays silent never recovers). The source node's agent
//! exports the NIC address's entries from Cilium's global IPv4 maps; the
//! target node's agent rebases their clock fields and imports them.
//!
//! Entries are copied as Cilium's raw map records (`struct ipv4_ct_tuple`,
//! 14 bytes, and `struct ct_entry`, 56 bytes, in forks/cilium v1.20.2).
//!
//! ```
//! use qrab_netd::ct::{rebase, select, Entry};
//! let guest = "10.244.128.7".parse().unwrap();
//! let mut key = [0u8; 14];
//! key[0..4].copy_from_slice(&[203, 0, 113, 5]); // daddr
//! key[4..8].copy_from_slice(&[10, 244, 128, 7]); // saddr
//! let mut value = [0u8; 56];
//! value[32..36].copy_from_slice(&1_000u32.to_le_bytes()); // lifetime
//! let entry = Entry { map: 't', key, value };
//! let other = Entry { key: [9; 14], ..entry.clone() };
//! assert_eq!(select(vec![entry.clone(), other], guest).len(), 1);
//! // Exported at the source's second 900, imported at the target's 5000:
//! // the 100 seconds left are kept.
//! let moved = rebase(&entry, 900, 5000).unwrap();
//! assert_eq!(u32::from_le_bytes(moved.value[32..36].try_into().unwrap()), 5100);
//! ```
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

/// Cilium's global IPv4 conntrack maps (TCP, and everything else).
pub const MAPS: [(char, &str); 2] = [('t', "cilium_ct4_global"), ('a', "cilium_ct_any4_global")];
/// Where the agent pins them.
pub const PIN_DIR: &str = "/sys/fs/bpf/tc/globals";
pub const KEY_LEN: usize = 14;
pub const VALUE_LEN: usize = 56;
const LIFETIME: usize = 32;
const REV_NAT: usize = 38;
const TX_REPORT: usize = 48;
const RX_REPORT: usize = 52;
/// Most entries one NIC's export carries (it must fit one switchover frame).
pub const MAX_ENTRIES: usize = 384;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// 't' (TCP map) or 'a' (the other map).
    pub map: char,
    pub key: [u8; KEY_LEN],
    pub value: [u8; VALUE_LEN],
}

impl Entry {
    fn addresses(&self) -> [Ipv4Addr; 2] {
        let addr = |at: usize| {
            Ipv4Addr::new(
                self.key[at],
                self.key[at + 1],
                self.key[at + 2],
                self.key[at + 3],
            )
        };
        [addr(0), addr(4)]
    }

    fn field(&self, at: usize) -> u32 {
        u32::from_le_bytes(self.value[at..at + 4].try_into().expect("4 bytes"))
    }

    fn set_field(&mut self, at: usize, value: u32) {
        self.value[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    /// A service entry: its reverse-NAT index names a node-local service ID.
    fn is_service(&self) -> bool {
        self.value[REV_NAT..REV_NAT + 2] != [0, 0]
    }
}

/// One NIC's entries and the exporting node's monotonic clock (seconds,
/// Cilium's `ktime` clock source).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Export {
    pub now: u64,
    /// `[map, key hex, value hex]`.
    pub entries: Vec<(char, String, String)>,
    /// Entries beyond [`MAX_ENTRIES`] were left out.
    #[serde(default)]
    pub truncated: bool,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != 2 * N {
        return None;
    }
    let mut out = [0u8; N];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(out)
}

impl Export {
    pub fn new(now: u64, mut entries: Vec<Entry>) -> Self {
        let truncated = entries.len() > MAX_ENTRIES;
        entries.truncate(MAX_ENTRIES);
        Self {
            now,
            entries: entries
                .iter()
                .map(|e| (e.map, hex(&e.key), hex(&e.value)))
                .collect(),
            truncated,
        }
    }

    pub fn decode(&self) -> Result<Vec<Entry>, String> {
        if self.entries.len() > MAX_ENTRIES {
            return Err(format!("more than {MAX_ENTRIES} conntrack entries"));
        }
        self.entries
            .iter()
            .map(|(map, key, value)| {
                if !MAPS.iter().any(|(m, _)| m == map) {
                    return Err(format!("unknown conntrack map {map:?}"));
                }
                Ok(Entry {
                    map: *map,
                    key: unhex(key).ok_or("malformed conntrack key")?,
                    value: unhex(value).ok_or("malformed conntrack value")?,
                })
            })
            .collect()
    }
}

/// `bpftool -j map dump` output: `[{"key": ["0x0a", …], "value": [...]}, …]`.
pub fn parse_dump(map: char, json: &str) -> Result<Vec<Entry>, String> {
    #[derive(Deserialize)]
    struct Record {
        key: Vec<String>,
        value: Vec<String>,
    }
    let records: Vec<Record> =
        serde_json::from_str(json).map_err(|e| format!("bpftool dump: {e}"))?;
    let bytes = |field: &[String]| -> Option<Vec<u8>> {
        field
            .iter()
            .map(|b| u8::from_str_radix(b.trim_start_matches("0x"), 16).ok())
            .collect()
    };
    records
        .iter()
        .map(|r| {
            Ok(Entry {
                map,
                key: bytes(&r.key)
                    .and_then(|k| k.try_into().ok())
                    .ok_or("a conntrack key is not 14 bytes")?,
                value: bytes(&r.value)
                    .and_then(|v| v.try_into().ok())
                    .ok_or("a conntrack value is not 56 bytes")?,
            })
        })
        .collect()
}

/// The entries of flows to or from `ip`, service entries excepted.
pub fn select(entries: Vec<Entry>, ip: Ipv4Addr) -> Vec<Entry> {
    entries
        .into_iter()
        .filter(|e| e.addresses().contains(&ip) && !e.is_service())
        .collect()
}

/// `entry` moved from a clock reading `from` to one reading `to`: its
/// lifetime and report stamps keep their distance from "now". `None` once
/// the entry has expired at the source.
pub fn rebase(entry: &Entry, from: u64, to: u64) -> Option<Entry> {
    let mut moved = entry.clone();
    let shift = |stamp: u32| -> u32 {
        let left = i64::from(stamp) - i64::try_from(from).unwrap_or(i64::MAX);
        (i64::try_from(to).unwrap_or(0) + left).clamp(0, i64::from(u32::MAX)) as u32
    };
    if u64::from(entry.field(LIFETIME)) <= from {
        return None;
    }
    moved.set_field(LIFETIME, shift(entry.field(LIFETIME)));
    for at in [TX_REPORT, RX_REPORT] {
        if entry.field(at) != 0 {
            moved.set_field(at, shift(entry.field(at)));
        }
    }
    Some(moved)
}

/// Whether the target node may take `entry` for the NIC at `ip`. An
/// imported entry is trusted as established by Cilium, so it must belong to
/// this NIC, and its other side must not be local to this node (a local Pod
/// or the host), whose own policy would otherwise be bypassed.
pub fn importable(
    entry: &Entry,
    ip: Ipv4Addr,
    is_local: impl Fn(Ipv4Addr) -> bool,
) -> Result<(), String> {
    let [d, s] = entry.addresses();
    let other = match (d == ip, s == ip) {
        (true, false) => s,
        (false, true) => d,
        _ => return Err(format!("entry {d} <- {s} is not a flow of {ip}")),
    };
    if entry.is_service() {
        return Err("service entries are node-local".into());
    }
    if other.is_loopback() || other.is_unspecified() || other.is_broadcast() || is_local(other) {
        return Err(format!("{other} is local to this node"));
    }
    Ok(())
}

/// `bpftool batch` input writing `entries` into the pinned maps.
pub fn batch(entries: &[Entry]) -> String {
    let bytes = |b: &[u8]| {
        b.iter()
            .map(|x| format!("0x{x:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    entries
        .iter()
        .map(|e| {
            let map = MAPS
                .iter()
                .find(|(m, _)| *m == e.map)
                .map(|(_, n)| *n)
                .expect("known map");
            format!(
                "map update pinned {PIN_DIR}/{map} key {} value {} any\n",
                bytes(&e.key),
                bytes(&e.value)
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(d: [u8; 4], s: [u8; 4], lifetime: u32) -> Entry {
        let mut key = [0u8; KEY_LEN];
        key[0..4].copy_from_slice(&d);
        key[4..8].copy_from_slice(&s);
        key[8..10].copy_from_slice(&5000u16.to_be_bytes());
        key[12] = 6;
        let mut value = [0u8; VALUE_LEN];
        value[LIFETIME..LIFETIME + 4].copy_from_slice(&lifetime.to_le_bytes());
        Entry {
            map: 't',
            key,
            value,
        }
    }
    const GUEST: Ipv4Addr = Ipv4Addr::new(10, 244, 128, 7);

    #[test]
    fn a_bpftool_dump_round_trips_through_an_export() {
        let json = r#"[{"key":["0x0a","0xf4","0x80","0x07","0xcb","0x00","0x71","0x05","0x13","0x88","0xd4","0x31","0x06","0x01"],
          "value":["0x00","0x00","0x00","0x00","0x00","0x00","0x00","0x00","0x00","0x00","0x00","0x00","0x00","0x00","0x00","0x00",
          "0x05","0x00","0x00","0x00","0x00","0x00","0x00","0x00","0x40","0x02","0x00","0x00","0x00","0x00","0x00","0x00",
          "0x50","0xf1","0x30","0x00","0x13","0x00","0x00","0x00","0x00","0x00","0x1b","0x1b","0x01","0x00","0x00","0x00",
          "0x46","0xf1","0x30","0x00","0x46","0xf1","0x30","0x00"]}]"#;
        let parsed = parse_dump('t', json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(
            parsed[0].addresses(),
            [GUEST, Ipv4Addr::new(203, 0, 113, 5)]
        );
        assert_eq!(parsed[0].field(LIFETIME), 0x0030_f150);
        let export = Export::new(3_200_000, select(parsed.clone(), GUEST));
        let wire = serde_json::to_string(&export).unwrap();
        let back: Export = serde_json::from_str(&wire).unwrap();
        assert_eq!(back.decode().unwrap(), parsed);
        // A wrong-sized record is refused, not truncated.
        assert!(parse_dump('t', r#"[{"key":["0x01"],"value":[]}]"#).is_err());
    }

    #[test]
    fn selection_keeps_only_this_nics_non_service_flows() {
        let mine = entry([203, 0, 113, 5], [10, 244, 128, 7], 100);
        let reverse = entry([10, 244, 128, 7], [203, 0, 113, 5], 100);
        let others = entry([203, 0, 113, 5], [10, 244, 128, 8], 100);
        let mut service = mine.clone();
        service.value[REV_NAT] = 3;
        let chosen = select(vec![mine.clone(), reverse.clone(), others, service], GUEST);
        assert_eq!(chosen, [mine, reverse]);
    }

    #[test]
    fn rebasing_keeps_the_time_left_and_drops_expired_entries() {
        let mut e = entry([203, 0, 113, 5], [10, 244, 128, 7], 1_000);
        e.set_field(TX_REPORT, 990);
        let moved = rebase(&e, 950, 20_000).unwrap();
        assert_eq!(moved.field(LIFETIME), 20_050);
        assert_eq!(moved.field(TX_REPORT), 20_040);
        assert_eq!(moved.field(RX_REPORT), 0, "an unset stamp stays unset");
        assert_eq!(moved.key, e.key);
        assert!(rebase(&e, 1_000, 20_000).is_none(), "expired");
        // A target clock behind the source's never underflows.
        assert_eq!(rebase(&e, 950, 10).unwrap().field(LIFETIME), 60);
    }

    #[test]
    fn imports_must_belong_to_the_nic_and_not_touch_local_endpoints() {
        let local = |ip: Ipv4Addr| {
            ip == Ipv4Addr::new(10, 244, 4, 9) || ip == Ipv4Addr::new(10, 203, 0, 13)
        };
        assert!(importable(&entry([203, 0, 113, 5], [10, 244, 128, 7], 9), GUEST, local).is_ok());
        assert!(importable(&entry([10, 244, 128, 7], [10, 244, 0, 5], 9), GUEST, local).is_ok());
        for (bad, why) in [
            (
                entry([203, 0, 113, 5], [10, 244, 128, 8], 9),
                "not a flow of",
            ),
            (
                entry([10, 244, 128, 7], [10, 244, 128, 7], 9),
                "not a flow of",
            ),
            (entry([10, 244, 4, 9], [10, 244, 128, 7], 9), "local"),
            (entry([10, 203, 0, 13], [10, 244, 128, 7], 9), "local"),
            (entry([127, 0, 0, 1], [10, 244, 128, 7], 9), "local"),
        ] {
            assert!(
                importable(&bad, GUEST, local).unwrap_err().contains(why),
                "{why}"
            );
        }
        let mut service = entry([203, 0, 113, 5], [10, 244, 128, 7], 9);
        service.value[REV_NAT + 1] = 1;
        assert!(importable(&service, GUEST, local)
            .unwrap_err()
            .contains("service"));
    }

    #[test]
    fn exports_are_bounded_and_malformed_ones_refused() {
        let many: Vec<_> = (0..MAX_ENTRIES + 5)
            .map(|_| entry([203, 0, 113, 5], [10, 244, 128, 7], 9))
            .collect();
        let export = Export::new(1, many);
        assert!(export.truncated && export.entries.len() == MAX_ENTRIES);
        for bad in [
            Export {
                now: 1,
                entries: vec![('x', "00".repeat(14), "00".repeat(56))],
                truncated: false,
            },
            Export {
                now: 1,
                entries: vec![('t', "00".repeat(13), "00".repeat(56))],
                truncated: false,
            },
            Export {
                now: 1,
                entries: vec![('t', "zz".repeat(14), "00".repeat(56))],
                truncated: false,
            },
        ] {
            assert!(bad.decode().is_err());
        }
        let line = batch(&[entry([203, 0, 113, 5], [10, 244, 128, 7], 9)]);
        assert!(line.starts_with("map update pinned /sys/fs/bpf/tc/globals/cilium_ct4_global key 0xcb 0x00 0x71 0x05 0x0a 0xf4"));
        assert!(line.ends_with(" any\n"));
    }
}

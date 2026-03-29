use qapi_macros::qapi;
use serde;
use serde_json;
use std::collections::HashMap;
use std::str::FromStr;
fn runtime_version() -> &'static str {
    "5.1"
}
// path begin:	qapi/common.json
/// An enumeration of the I/O operation types
#[qapi(since = "2.1")]
pub enum IoOperationType {
    /// read operation
    #[qapi(name = "read")]
    Read,
    /// write operation
    #[qapi(name = "write")]
    Write,
}
/// An enumeration of three options: on, off, and auto
#[qapi(since = "2.2")]
pub enum OnOffAuto {
    /// QEMU selects the value between on and off
    #[qapi(name = "auto")]
    Auto,
    /// Enabled
    #[qapi(name = "on")]
    On,
    /// Disabled
    #[qapi(name = "off")]
    Off,
}
/// An enumeration of three values: on, off, and split
#[qapi(since = "2.6")]
pub enum OnOffSplit {
    /// Enabled
    #[qapi(name = "on")]
    On,
    /// Disabled
    #[qapi(name = "off")]
    Off,
    /// Mixed
    #[qapi(name = "split")]
    Split,
}
/// This is a string value or the explicit lack of a string (null
/// pointer in C).  Intended for cases when 'optional absent' already
/// has a different meaning.
#[qapi(since = "2.10")]
pub enum StrOrNull {
    /// the string value
    #[qapi(name = "s")]
    S(String),
    /// no string value
    #[qapi(name = "n")]
    N(NULL_REPLACE_NULL),
}
/// An enumeration of options for specifying a PCI BAR
#[qapi(name = "OffAutoPCIBAR")]
#[qapi(since = "2.12")]
pub enum OffAutoPcibar {
    /// The specified feature is disabled
    #[qapi(name = "off")]
    Off,
    /// The PCI BAR for the feature is automatically selected
    #[qapi(name = "auto")]
    Auto,
    /// PCI BAR0 is used for the feature
    #[qapi(name = "bar0")]
    Bar0,
    /// PCI BAR1 is used for the feature
    #[qapi(name = "bar1")]
    Bar1,
    /// PCI BAR2 is used for the feature
    #[qapi(name = "bar2")]
    Bar2,
    /// PCI BAR3 is used for the feature
    #[qapi(name = "bar3")]
    Bar3,
    /// PCI BAR4 is used for the feature
    #[qapi(name = "bar4")]
    Bar4,
    /// PCI BAR5 is used for the feature
    #[qapi(name = "bar5")]
    Bar5,
}
/// An enumeration of PCIe link speeds in units of GT/s
#[qapi(name = "PCIELinkSpeed")]
#[qapi(since = "4.0")]
pub enum PcieLinkSpeed {
    /// 2.5GT/s
    #[qapi(name = "2_5")]
    _25,
    /// 5.0GT/s
    #[qapi(name = "5")]
    _5,
    /// 8.0GT/s
    #[qapi(name = "8")]
    _8,
    /// 16.0GT/s
    #[qapi(name = "16")]
    _16,
    /// 32.0GT/s (since 9.0)
    #[qapi(name = "32")]
    #[qapi(since = "9.0")]
    _32,
    /// 64.0GT/s (since 9.0)
    #[qapi(name = "64")]
    #[qapi(since = "9.0")]
    _64,
}
/// An enumeration of PCIe link width
#[qapi(name = "PCIELinkWidth")]
#[qapi(since = "4.0")]
pub enum PcieLinkWidth {
    /// x1
    #[qapi(name = "1")]
    _1,
    /// x2
    #[qapi(name = "2")]
    _2,
    /// x4
    #[qapi(name = "4")]
    _4,
    /// x8
    #[qapi(name = "8")]
    _8,
    /// x12
    #[qapi(name = "12")]
    _12,
    /// x16
    #[qapi(name = "16")]
    _16,
    /// x32
    #[qapi(name = "32")]
    _32,
}
/// Host memory policy types
#[qapi(since = "2.1")]
pub enum HostMemPolicy {
    /// restore default policy, remove any nondefault policy
    #[qapi(name = "default")]
    Default,
    /// set the preferred host nodes for allocation
    #[qapi(name = "preferred")]
    Preferred,
    /// a strict policy that restricts memory allocation to the host
    /// nodes specified
    #[qapi(name = "bind")]
    Bind,
    /// memory allocations are interleaved across the set of
    /// host nodes specified
    #[qapi(name = "interleave")]
    Interleave,
}
/// Indicates whether a netfilter is attached to a netdev's transmit
/// queue or receive queue or both.
#[qapi(since = "2.5")]
pub enum NetFilterDirection {
    /// the filter is attached both to the receive and the transmit
    /// queue of the netdev (default).
    #[qapi(name = "all")]
    All,
    /// the filter is attached to the receive queue of the netdev,
    /// where it will receive packets sent to the netdev.
    #[qapi(name = "rx")]
    Rx,
    /// the filter is attached to the transmit queue of the netdev,
    /// where it will receive packets sent by the netdev.
    #[qapi(name = "tx")]
    Tx,
}
/// Keys to toggle input-linux between host and guest.
#[qapi(since = "4.0")]
pub enum GrabToggleKeys {
    #[qapi(name = "ctrl-ctrl")]
    CtrlCtrl,
    #[qapi(name = "alt-alt")]
    AltAlt,
    #[qapi(name = "shift-shift")]
    ShiftShift,
    #[qapi(name = "meta-meta")]
    MetaMeta,
    #[qapi(name = "scrolllock")]
    Scrolllock,
    #[qapi(name = "ctrl-scrolllock")]
    CtrlScrolllock,
}
#[qapi(since = "6.2")]
pub struct HumanReadableText {
    /// Formatted output intended for humans.
    #[qapi(name = "human-readable-text")]
    pub human_readable_text: String,
}
// path end:	qapi/common.json

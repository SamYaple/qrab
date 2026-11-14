#![allow(unused_imports, non_camel_case_types, dead_code)]
use crate as qapi;
use qapi_macros::qapi;
use std::collections::HashMap;
/// This action can be used to test transaction failure.
///
#[qapi(added = "1.6.0")]
pub struct Abort {}

#[qapi(added = "6.2.0")]
pub struct AbortWrapper {
    pub data: Abort,
}

/// Information about support for MSHV acceleration
///
#[qapi(added = "10.2.0")]
pub enum Accelerator {
    /// Apple Hypervisor.framework
    #[qapi(name = "hvf")]
    Hvf,
    /// KVM
    #[qapi(name = "kvm")]
    Kvm,
    /// Hyper-V
    #[qapi(name = "mshv")]
    Mshv,
    /// NetBSD NVMM
    #[qapi(name = "nvmm")]
    Nvmm,
    /// QTest (dummy accelerator)
    #[qapi(name = "qtest")]
    Qtest,
    /// TCG (dynamic translation)
    #[qapi(name = "tcg")]
    Tcg,
    /// Windows Hypervisor Platform
    #[qapi(name = "whpx")]
    Whpx,
    /// Xen
    #[qapi(name = "xen")]
    Xen,
}

/// Information about support for various accelerators
///
#[qapi(added = "10.2.0")]
pub struct AcceleratorInfo {
    /// the accelerator that is in use
    pub enabled: Accelerator,
    /// the list of accelerators that are built into this
    /// executable
    pub present: Vec<Accelerator>,
}

/// Properties for acpi-generic-initiator objects.
///
#[qapi(added = "9.0.0")]
pub struct AcpiGenericInitiatorProperties {
    /// NUMA node associated with the PCI device
    pub node: u32,
    /// PCI device ID to be associated with the node
    #[qapi(name = "pci-dev")]
    pub pci_dev: String,
}

/// Properties for acpi-generic-port objects.
///
#[qapi(added = "9.2.0")]
pub struct AcpiGenericPortProperties {
    /// Similar to a NUMA node ID, but instead of providing a
    /// reference point used for defining NUMA distances and access
    /// characteristics to memory or from an initiator (e.g. CPU), this
    /// node defines the boundary point between non-discoverable system
    /// buses which must be described by firmware, and a discoverable
    /// bus.  NUMA distances and access characteristics are defined to
    /// and from that point.  For system software to establish full
    /// initiator to target characteristics this information must be
    /// combined with information retrieved from the discoverable part
    /// of the path.  An example would use CDAT (see UEFI.org)
    /// information read from devices and switches in conjunction with
    /// link characteristics read from PCIe Configuration space.  To get
    /// the full path latency from CPU to CXL attached DRAM CXL device:
    /// Add the latency from CPU to Generic Port (from HMAT indexed via
    /// the node ID in this SRAT structure) to that for CXL bus links,
    /// the latency across intermediate switches and from the EP port to
    /// the actual memory.  Bandwidth is more complex as there may be
    /// interleaving across multiple devices and shared links in the
    /// path.
    pub node: u32,
    /// QOM path of the PCI bus of the hostbridge associated with
    /// this SRAT Generic Port Affinity Structure.  This is the same as
    /// the bus parameter for the root ports attached to this host
    /// bridge.  The resulting SRAT Generic Port Affinity Structure will
    /// refer to the ACPI object in DSDT that represents the host bridge
    /// (e.g.  ACPI0016 for CXL host bridges).  See ACPI 6.5 Section
    /// 5.2.16.7 for more information.
    #[qapi(name = "pci-bus")]
    pub pci_bus: String,
}

/// OSPM Status Indication for a device.  For description of possible
/// values of @source and @status fields see "_OST (OSPM Status
/// Indication)" chapter of ACPI5.0 spec.
///
#[qapi(name = "ACPIOSTInfo")]
#[qapi(added = "2.1.0")]
pub struct AcpiostInfo {
    /// device ID associated with slot
    pub device: Option<String>,
    /// slot ID, unique per slot of a given @slot-type
    pub slot: String,
    /// type of the slot
    #[qapi(name = "slot-type")]
    pub slot_type: AcpiSlotType,
    /// an integer containing the source event
    pub source: i64,
    /// an integer containing the status code
    pub status: i64,
}

#[qapi(name = "ACPISlotType")]
#[qapi(added = "2.1.0")]
pub enum AcpiSlotType {
    /// logical CPU slot (since 2.7)
    #[qapi(name = "CPU")]
    #[qapi(added = "2.7.0")]
    Cpu,
    /// memory slot
    #[qapi(name = "DIMM")]
    Dimm,
}

/// Specify an ACPI table on the command line to load.
///
/// At most one of @file and @data can be specified.  The list of files
/// specified by any one of them is loaded and concatenated in order.
/// If both are omitted, @data is implied.
///
/// Other fields / optargs can be used to override fields of the generic
/// ACPI table header; refer to the ACPI specification 5.0, section
/// 5.2.6 System Description Table Header.  If a header field is not
/// overridden, then the corresponding value from the concatenated blob
/// is used (in case of @file), or it is filled in with a hard-coded
/// value (in case of @data).
///
/// String fields are copied into the matching ACPI member from lowest
/// address upwards, and silently truncated / NUL-padded to length.
///
#[qapi(added = "1.5.0")]
pub struct AcpiTableOptions {
    /// identifier of the utility that created the table
    /// (4 bytes)
    pub asl_compiler_id: Option<String>,
    /// revision number of the utility that created the
    /// table (4 bytes)
    pub asl_compiler_rev: Option<u32>,
    /// colon (:) separated list of pathnames to load and concatenate
    /// as table data.  The resultant binary blob must not have an ACPI
    /// table header.  At least one file is required.  This field
    /// excludes @file.
    pub data: Option<String>,
    /// colon (:) separated list of pathnames to load and concatenate
    /// as table data.  The resultant binary blob is expected to have an
    /// ACPI table header.  At least one file is required.  This field
    /// excludes @data.
    pub file: Option<String>,
    /// OEM identifier (6 bytes)
    pub oem_id: Option<String>,
    /// OEM-supplied revision number (4 bytes)
    pub oem_rev: Option<u32>,
    /// OEM table identifier (8 bytes)
    pub oem_table_id: Option<String>,
    /// table revision number (dependent on signature, 1 byte)
    pub rev: Option<u8>,
    /// table signature / identifier (4 bytes)
    pub sig: Option<String>,
}

/// An enumeration of transactional completion modes.
///
#[qapi(added = "2.5.0")]
pub enum ActionCompletionMode {
    /// If any Action fails after the Transaction succeeds, cancel
    /// all Actions.  Actions do not complete until all Actions are
    /// ready to complete.  May be rejected by Actions that do not
    /// support this completion mode.
    #[qapi(name = "grouped")]
    Grouped,
    /// Do not attempt to cancel any other Actions if any
    /// Actions fail after the Transaction request succeeds.  All
    /// Actions that can complete successfully will do so without
    /// waiting on others.  This is the default.
    #[qapi(name = "individual")]
    Individual,
}

/// Information about a file descriptor that was added to an fd set.
///
#[qapi(added = "1.2.0")]
pub struct AddfdInfo {
    /// The file descriptor that was received via SCM rights and added
    /// to the fd set.
    pub fd: i64,
    /// The ID of the fd set that @fd was added to.
    #[qapi(name = "fdset-id")]
    pub fdset_id: i64,
}

/// Attach mode for a default XDP program
///
#[qapi(name = "AFXDPMode")]
#[qapi(condition = "CONFIG_AF_XDP")]
#[qapi(added = "8.2.0")]
pub enum AfxdpMode {
    /// DRV mode, program is attached to a driver, packets are
    /// passed to the socket without allocation of skb.
    #[qapi(name = "native")]
    Native,
    /// generic mode, no driver support necessary
    #[qapi(name = "skb")]
    Skb,
}

/// Parameters for self-announce timers
///
#[qapi(added = "4.0.0")]
pub struct AnnounceParameters {
    /// A name to be used to identify an instance of announce-timers
    /// and to allow it to modified later.  Not for use as part of the
    /// migration parameters.  (Since 4.1)
    #[qapi(added = "4.1.0")]
    pub id: Option<String>,
    /// Initial delay (in ms) before sending the first GARP/RARP
    /// announcement
    pub initial: i64,
    /// An optional list of interface names, which restricts
    /// the announcement to the listed interfaces.  (Since 4.1)
    #[qapi(added = "4.1.0")]
    pub interfaces: Option<Vec<String>>,
    /// Maximum delay (in ms) between GARP/RARP announcement packets
    pub max: i64,
    /// Number of self-announcement attempts
    pub rounds: i64,
    /// Delay increase (in ms) after each self-announcement attempt
    pub step: i64,
}

/// Options of an audio backend.
///
#[qapi(added = "4.0.0")]
pub struct Audiodev {
    /// the backend driver to use
    #[qapi(discriminator)]
    pub driver: AudiodevDriver,
    /// identifier of the backend
    pub id: String,
    /// timer period (in microseconds, 0: use lowest
    /// possible)
    #[qapi(name = "timer-period")]
    pub timer_period: Option<u32>,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<AudiodevBranch>,
}

/// Options of the ALSA audio backend.
///
#[qapi(added = "4.0.0")]
pub struct AudiodevAlsaOptions {
    /// options of the capture stream
    pub r#in: Option<AudiodevAlsaPerDirectionOptions>,
    /// options of the playback stream
    pub out: Option<AudiodevAlsaPerDirectionOptions>,
    /// set the threshold (in microseconds) when playback starts
    pub threshold: Option<u32>,
}

/// Options of the ALSA backend that are used for both playback and
/// recording.
///
#[qapi(added = "4.0.0")]
pub struct AudiodevAlsaPerDirectionOptions {
    /// the buffer length in microseconds
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    /// number of channels when using fixed settings (default 2)
    pub channels: Option<u32>,
    /// the name of the ALSA device to use (default 'default')
    pub dev: Option<String>,
    /// use fixed settings for host input/output.  When
    /// off, frequency, channels and format must not be specified
    /// (default true)
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    /// sample format to use when using fixed settings (default
    /// s16)
    pub format: Option<AudioFormat>,
    /// frequency to use when using fixed settings (default
    /// 44100)
    pub frequency: Option<u32>,
    /// use QEMU's mixing engine to mix all streams inside
    /// QEMU and convert audio formats when not supported by the
    /// backend.  When set to off, fixed-settings must be also off
    /// (default on, since 4.2)
    #[qapi(name = "mixing-engine")]
    #[qapi(added = "4.2.0")]
    pub mixing_engine: Option<bool>,
    /// the period length in microseconds
    #[qapi(name = "period-length")]
    pub period_length: Option<u32>,
    /// attempt to use poll mode, falling back to non-polling
    /// access on failure (default false)
    #[qapi(name = "try-poll")]
    pub try_poll: Option<bool>,
    /// number of voices to use (default 1)
    pub voices: Option<u32>,
}

#[qapi(added = "4.0.0")]
pub enum AudiodevBranch {
    #[qapi(name = "alsa")]
    #[qapi(condition = "CONFIG_AUDIO_ALSA")]
    Alsa(AudiodevAlsaOptions),
    #[qapi(name = "coreaudio")]
    #[qapi(condition = "CONFIG_AUDIO_COREAUDIO")]
    Coreaudio(AudiodevCoreaudioOptions),
    #[qapi(name = "dbus")]
    #[qapi(condition = "CONFIG_DBUS_DISPLAY")]
    #[qapi(added = "7.0.0")]
    Dbus(AudiodevDBusOptions),
    #[qapi(name = "dsound")]
    #[qapi(condition = "CONFIG_AUDIO_DSOUND")]
    Dsound(AudiodevDsoundOptions),
    #[qapi(name = "jack")]
    #[qapi(condition = "CONFIG_AUDIO_JACK")]
    #[qapi(added = "5.1.0")]
    Jack(AudiodevJackOptions),
    #[qapi(name = "none")]
    None(AudiodevGenericOptions),
    #[qapi(name = "oss")]
    #[qapi(condition = "CONFIG_AUDIO_OSS")]
    Oss(AudiodevOssOptions),
    #[qapi(name = "pa")]
    #[qapi(condition = "CONFIG_AUDIO_PA")]
    Pa(AudiodevPaOptions),
    #[qapi(name = "pipewire")]
    #[qapi(condition = "CONFIG_AUDIO_PIPEWIRE")]
    #[qapi(added = "8.1.0")]
    Pipewire(AudiodevPipewireOptions),
    #[qapi(name = "sdl")]
    #[qapi(condition = "CONFIG_AUDIO_SDL")]
    Sdl(AudiodevSdlOptions),
    #[qapi(name = "sndio")]
    #[qapi(condition = "CONFIG_AUDIO_SNDIO")]
    #[qapi(added = "7.2.0")]
    Sndio(AudiodevSndioOptions),
    #[qapi(name = "spice")]
    #[qapi(condition = "CONFIG_SPICE")]
    Spice(AudiodevGenericOptions),
    #[qapi(name = "wav")]
    Wav(AudiodevWavOptions),
}

/// Options of the coreaudio audio backend.
///
#[qapi(added = "4.0.0")]
pub struct AudiodevCoreaudioOptions {
    /// options of the capture stream
    pub r#in: Option<AudiodevCoreaudioPerDirectionOptions>,
    /// options of the playback stream
    pub out: Option<AudiodevCoreaudioPerDirectionOptions>,
}

/// Options of the Core Audio backend that are used for both playback
/// and recording.
///
#[qapi(added = "4.0.0")]
pub struct AudiodevCoreaudioPerDirectionOptions {
    /// number of buffers
    #[qapi(name = "buffer-count")]
    pub buffer_count: Option<u32>,
    /// the buffer length in microseconds
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    /// number of channels when using fixed settings (default 2)
    pub channels: Option<u32>,
    /// use fixed settings for host input/output.  When
    /// off, frequency, channels and format must not be specified
    /// (default true)
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    /// sample format to use when using fixed settings (default
    /// s16)
    pub format: Option<AudioFormat>,
    /// frequency to use when using fixed settings (default
    /// 44100)
    pub frequency: Option<u32>,
    /// use QEMU's mixing engine to mix all streams inside
    /// QEMU and convert audio formats when not supported by the
    /// backend.  When set to off, fixed-settings must be also off
    /// (default on, since 4.2)
    #[qapi(name = "mixing-engine")]
    #[qapi(added = "4.2.0")]
    pub mixing_engine: Option<bool>,
    /// number of voices to use (default 1)
    pub voices: Option<u32>,
}

/// Options of the D-Bus audio backend.
///
#[qapi(added = "10.0.0")]
pub struct AudiodevDBusOptions {
    /// options of the capture stream
    pub r#in: Option<AudiodevPerDirectionOptions>,
    /// set the number of samples per read/write calls
    /// (default to 480, 10ms at 48kHz).
    pub nsamples: Option<u32>,
    /// options of the playback stream
    pub out: Option<AudiodevPerDirectionOptions>,
}

/// An enumeration of possible audio backend drivers.
///
#[qapi(added = "4.0.0")]
pub enum AudiodevDriver {
    #[qapi(name = "alsa")]
    #[qapi(condition = "CONFIG_AUDIO_ALSA")]
    Alsa,
    #[qapi(name = "coreaudio")]
    #[qapi(condition = "CONFIG_AUDIO_COREAUDIO")]
    Coreaudio,
    #[qapi(name = "dbus")]
    #[qapi(condition = "CONFIG_DBUS_DISPLAY")]
    #[qapi(added = "7.0.0")]
    Dbus,
    #[qapi(name = "dsound")]
    #[qapi(condition = "CONFIG_AUDIO_DSOUND")]
    Dsound,
    /// JACK audio backend (since 5.1)
    #[qapi(name = "jack")]
    #[qapi(condition = "CONFIG_AUDIO_JACK")]
    #[qapi(added = "5.1.0")]
    Jack,
    #[qapi(name = "none")]
    None,
    #[qapi(name = "oss")]
    #[qapi(condition = "CONFIG_AUDIO_OSS")]
    Oss,
    #[qapi(name = "pa")]
    #[qapi(condition = "CONFIG_AUDIO_PA")]
    Pa,
    #[qapi(name = "pipewire")]
    #[qapi(condition = "CONFIG_AUDIO_PIPEWIRE")]
    #[qapi(added = "8.1.0")]
    Pipewire,
    #[qapi(name = "sdl")]
    #[qapi(condition = "CONFIG_AUDIO_SDL")]
    Sdl,
    #[qapi(name = "sndio")]
    #[qapi(condition = "CONFIG_AUDIO_SNDIO")]
    #[qapi(added = "7.2.0")]
    Sndio,
    #[qapi(name = "spice")]
    #[qapi(condition = "CONFIG_SPICE")]
    Spice,
    #[qapi(name = "wav")]
    Wav,
}

/// Options of the DirectSound audio backend.
///
#[qapi(added = "4.0.0")]
pub struct AudiodevDsoundOptions {
    /// options of the capture stream
    pub r#in: Option<AudiodevPerDirectionOptions>,
    /// add extra latency to playback in microseconds (default
    /// 10000)
    pub latency: Option<u32>,
    /// options of the playback stream
    pub out: Option<AudiodevPerDirectionOptions>,
}

/// Generic driver-specific options.
///
#[qapi(added = "4.0.0")]
pub struct AudiodevGenericOptions {
    /// options of the capture stream
    pub r#in: Option<AudiodevPerDirectionOptions>,
    /// options of the playback stream
    pub out: Option<AudiodevPerDirectionOptions>,
}

/// Options of the JACK audio backend.
///
#[qapi(added = "5.1.0")]
pub struct AudiodevJackOptions {
    /// options of the capture stream
    pub r#in: Option<AudiodevJackPerDirectionOptions>,
    /// options of the playback stream
    pub out: Option<AudiodevJackPerDirectionOptions>,
}

/// Options of the JACK backend that are used for both playback and
/// recording.
///
#[qapi(added = "5.1.0")]
pub struct AudiodevJackPerDirectionOptions {
    /// the buffer length in microseconds
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    /// number of channels when using fixed settings (default 2)
    pub channels: Option<u32>,
    /// the client name to use.  The server will modify this
    /// name to create a unique variant, if needed unless @exact-name is
    /// true (default: the guest's name)
    #[qapi(name = "client-name")]
    pub client_name: Option<String>,
    /// if set, a regular expression of JACK client port
    /// name(s) to monitor for and automatically connect to
    #[qapi(name = "connect-ports")]
    pub connect_ports: Option<String>,
    /// use the exact name requested otherwise JACK
    /// automatically generates a unique one, if needed (default: false)
    #[qapi(name = "exact-name")]
    pub exact_name: Option<bool>,
    /// use fixed settings for host input/output.  When
    /// off, frequency, channels and format must not be specified
    /// (default true)
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    /// sample format to use when using fixed settings (default
    /// s16)
    pub format: Option<AudioFormat>,
    /// frequency to use when using fixed settings (default
    /// 44100)
    pub frequency: Option<u32>,
    /// use QEMU's mixing engine to mix all streams inside
    /// QEMU and convert audio formats when not supported by the
    /// backend.  When set to off, fixed-settings must be also off
    /// (default on, since 4.2)
    #[qapi(name = "mixing-engine")]
    pub mixing_engine: Option<bool>,
    /// select from among several possible concurrent server
    /// instances (default: environment variable $JACK_DEFAULT_SERVER if
    /// set, else "default")
    #[qapi(name = "server-name")]
    pub server_name: Option<String>,
    /// start a jack server process if one is not already
    /// present (default: false)
    #[qapi(name = "start-server")]
    pub start_server: Option<bool>,
    /// number of voices to use (default 1)
    pub voices: Option<u32>,
}

/// Options of the OSS audio backend.
///
#[qapi(added = "4.0.0")]
pub struct AudiodevOssOptions {
    /// set the timing policy of the device (between 0 and 10,
    /// where smaller number means smaller latency but higher CPU usage)
    /// or -1 to use fragment mode (option ignored on some platforms)
    /// (default 5)
    #[qapi(name = "dsp-policy")]
    pub dsp_policy: Option<u32>,
    /// open device in exclusive mode (vmix won't work) (default
    /// false)
    pub exclusive: Option<bool>,
    /// options of the capture stream
    pub r#in: Option<AudiodevOssPerDirectionOptions>,
    /// options of the playback stream
    pub out: Option<AudiodevOssPerDirectionOptions>,
    /// try using memory-mapped access, falling back to
    /// non-memory-mapped access on failure (default true)
    #[qapi(name = "try-mmap")]
    pub try_mmap: Option<bool>,
}

/// Options of the OSS backend that are used for both playback and
/// recording.
///
#[qapi(added = "4.0.0")]
pub struct AudiodevOssPerDirectionOptions {
    /// number of buffers
    #[qapi(name = "buffer-count")]
    pub buffer_count: Option<u32>,
    /// the buffer length in microseconds
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    /// number of channels when using fixed settings (default 2)
    pub channels: Option<u32>,
    /// file name of the OSS device (default '/dev/dsp')
    pub dev: Option<String>,
    /// use fixed settings for host input/output.  When
    /// off, frequency, channels and format must not be specified
    /// (default true)
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    /// sample format to use when using fixed settings (default
    /// s16)
    pub format: Option<AudioFormat>,
    /// frequency to use when using fixed settings (default
    /// 44100)
    pub frequency: Option<u32>,
    /// use QEMU's mixing engine to mix all streams inside
    /// QEMU and convert audio formats when not supported by the
    /// backend.  When set to off, fixed-settings must be also off
    /// (default on, since 4.2)
    #[qapi(name = "mixing-engine")]
    #[qapi(added = "4.2.0")]
    pub mixing_engine: Option<bool>,
    /// attempt to use poll mode, falling back to non-polling
    /// access on failure (default true)
    #[qapi(name = "try-poll")]
    pub try_poll: Option<bool>,
    /// number of voices to use (default 1)
    pub voices: Option<u32>,
}

/// Options of the PulseAudio audio backend.
///
#[qapi(added = "4.0.0")]
pub struct AudiodevPaOptions {
    /// options of the capture stream
    pub r#in: Option<AudiodevPaPerDirectionOptions>,
    /// options of the playback stream
    pub out: Option<AudiodevPaPerDirectionOptions>,
    /// PulseAudio server address (default: let PulseAudio choose)
    pub server: Option<String>,
}

/// Options of the Pulseaudio backend that are used for both playback
/// and recording.
///
#[qapi(added = "4.0.0")]
pub struct AudiodevPaPerDirectionOptions {
    /// the buffer length in microseconds
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    /// number of channels when using fixed settings (default 2)
    pub channels: Option<u32>,
    /// use fixed settings for host input/output.  When
    /// off, frequency, channels and format must not be specified
    /// (default true)
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    /// sample format to use when using fixed settings (default
    /// s16)
    pub format: Option<AudioFormat>,
    /// frequency to use when using fixed settings (default
    /// 44100)
    pub frequency: Option<u32>,
    /// latency you want PulseAudio to achieve in microseconds
    /// (default 15000)
    pub latency: Option<u32>,
    /// use QEMU's mixing engine to mix all streams inside
    /// QEMU and convert audio formats when not supported by the
    /// backend.  When set to off, fixed-settings must be also off
    /// (default on, since 4.2)
    #[qapi(name = "mixing-engine")]
    #[qapi(added = "4.2.0")]
    pub mixing_engine: Option<bool>,
    /// name of the sink/source to use
    pub name: Option<String>,
    /// name of the PulseAudio stream created by QEMU.  Can be
    /// used to identify the stream in PulseAudio when you create
    /// multiple PulseAudio devices or run multiple QEMU instances
    /// (default: audiodev's id, since 4.2)
    #[qapi(name = "stream-name")]
    #[qapi(added = "4.2.0")]
    pub stream_name: Option<String>,
    /// number of voices to use (default 1)
    pub voices: Option<u32>,
}

/// General audio backend options that are used for both playback and
/// recording.
///
#[qapi(added = "4.0.0")]
pub struct AudiodevPerDirectionOptions {
    /// the buffer length in microseconds
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    /// number of channels when using fixed settings (default 2)
    pub channels: Option<u32>,
    /// use fixed settings for host input/output.  When
    /// off, frequency, channels and format must not be specified
    /// (default true)
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    /// sample format to use when using fixed settings (default
    /// s16)
    pub format: Option<AudioFormat>,
    /// frequency to use when using fixed settings (default
    /// 44100)
    pub frequency: Option<u32>,
    /// use QEMU's mixing engine to mix all streams inside
    /// QEMU and convert audio formats when not supported by the
    /// backend.  When set to off, fixed-settings must be also off
    /// (default on, since 4.2)
    #[qapi(name = "mixing-engine")]
    #[qapi(added = "4.2.0")]
    pub mixing_engine: Option<bool>,
    /// number of voices to use (default 1)
    pub voices: Option<u32>,
}

/// Options of the PipeWire audio backend.
///
#[qapi(added = "8.1.0")]
pub struct AudiodevPipewireOptions {
    /// options of the capture stream
    pub r#in: Option<AudiodevPipewirePerDirectionOptions>,
    /// options of the playback stream
    pub out: Option<AudiodevPipewirePerDirectionOptions>,
}

/// Options of the PipeWire backend that are used for both playback and
/// recording.
///
#[qapi(added = "8.1.0")]
pub struct AudiodevPipewirePerDirectionOptions {
    /// the buffer length in microseconds
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    /// number of channels when using fixed settings (default 2)
    pub channels: Option<u32>,
    /// use fixed settings for host input/output.  When
    /// off, frequency, channels and format must not be specified
    /// (default true)
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    /// sample format to use when using fixed settings (default
    /// s16)
    pub format: Option<AudioFormat>,
    /// frequency to use when using fixed settings (default
    /// 44100)
    pub frequency: Option<u32>,
    /// latency you want PipeWire to achieve in microseconds
    /// (default 46000)
    pub latency: Option<u32>,
    /// use QEMU's mixing engine to mix all streams inside
    /// QEMU and convert audio formats when not supported by the
    /// backend.  When set to off, fixed-settings must be also off
    /// (default on, since 4.2)
    #[qapi(name = "mixing-engine")]
    pub mixing_engine: Option<bool>,
    /// name of the sink/source to use
    pub name: Option<String>,
    /// name of the PipeWire stream created by QEMU.  Can be
    /// used to identify the stream in PipeWire when you create multiple
    /// PipeWire devices or run multiple QEMU instances (default:
    /// audiodev's id)
    #[qapi(name = "stream-name")]
    pub stream_name: Option<String>,
    /// number of voices to use (default 1)
    pub voices: Option<u32>,
}

/// Options of the SDL audio backend.
///
#[qapi(added = "6.0.0")]
pub struct AudiodevSdlOptions {
    /// options of the recording stream
    pub r#in: Option<AudiodevSdlPerDirectionOptions>,
    /// options of the playback stream
    pub out: Option<AudiodevSdlPerDirectionOptions>,
}

/// Options of the SDL audio backend that are used for both playback and
/// recording.
///
#[qapi(added = "6.0.0")]
pub struct AudiodevSdlPerDirectionOptions {
    /// number of buffers (default 4)
    #[qapi(name = "buffer-count")]
    pub buffer_count: Option<u32>,
    /// the buffer length in microseconds
    #[qapi(name = "buffer-length")]
    pub buffer_length: Option<u32>,
    /// number of channels when using fixed settings (default 2)
    pub channels: Option<u32>,
    /// use fixed settings for host input/output.  When
    /// off, frequency, channels and format must not be specified
    /// (default true)
    #[qapi(name = "fixed-settings")]
    pub fixed_settings: Option<bool>,
    /// sample format to use when using fixed settings (default
    /// s16)
    pub format: Option<AudioFormat>,
    /// frequency to use when using fixed settings (default
    /// 44100)
    pub frequency: Option<u32>,
    /// use QEMU's mixing engine to mix all streams inside
    /// QEMU and convert audio formats when not supported by the
    /// backend.  When set to off, fixed-settings must be also off
    /// (default on, since 4.2)
    #[qapi(name = "mixing-engine")]
    pub mixing_engine: Option<bool>,
    /// number of voices to use (default 1)
    pub voices: Option<u32>,
}

/// Options of the sndio audio backend.
///
#[qapi(added = "7.2.0")]
pub struct AudiodevSndioOptions {
    /// the name of the sndio device to use (default 'default')
    pub dev: Option<String>,
    /// options of the capture stream
    pub r#in: Option<AudiodevPerDirectionOptions>,
    /// play buffer size (in microseconds)
    pub latency: Option<u32>,
    /// options of the playback stream
    pub out: Option<AudiodevPerDirectionOptions>,
}

/// Options of the wav audio backend.
///
#[qapi(added = "4.0.0")]
pub struct AudiodevWavOptions {
    /// options of the capture stream
    pub r#in: Option<AudiodevPerDirectionOptions>,
    /// options of the playback stream
    pub out: Option<AudiodevPerDirectionOptions>,
    /// name of the wav file to record (default 'qemu.wav')
    pub path: Option<String>,
}

/// An enumeration of possible audio formats.
///
#[qapi(added = "4.0.0")]
pub enum AudioFormat {
    /// single precision floating-point (since 5.0)
    #[qapi(name = "f32")]
    #[qapi(added = "5.0.0")]
    F32,
    /// signed 16 bit integer
    #[qapi(name = "s16")]
    S16,
    /// signed 32 bit integer
    #[qapi(name = "s32")]
    S32,
    /// signed 8 bit integer
    #[qapi(name = "s8")]
    S8,
    /// unsigned 16 bit integer
    #[qapi(name = "u16")]
    U16,
    /// unsigned 32 bit integer
    #[qapi(name = "u32")]
    U32,
    /// unsigned 8 bit integer
    #[qapi(name = "u8")]
    U8,
}

/// Properties for authz-listfile objects.
///
#[qapi(added = "6.0.0")]
pub struct AuthZListFileProperties {
    /// File name to load the configuration from.  The file must
    /// contain valid JSON for `AuthZListProperties`.
    pub filename: String,
    /// If true, inotify is used to monitor the file,
    /// automatically reloading changes.  If an error occurs during
    /// reloading, all authorizations will fail until the file is next
    /// successfully loaded.  (default: true if the binary was built
    /// with CONFIG_INOTIFY1, false otherwise)
    pub refresh: Option<bool>,
}

/// Properties for authz-list objects.
///
#[qapi(added = "6.0.0")]
pub struct AuthZListProperties {
    /// Default policy to apply when no rule matches (default:
    /// deny)
    pub policy: Option<QAuthZListPolicy>,
    /// Authorization rules based on matching user
    pub rules: Option<Vec<QAuthZListRule>>,
}

/// Properties for authz-pam objects.
///
#[qapi(name = "AuthZPAMProperties")]
#[qapi(added = "6.0.0")]
pub struct AuthZpamProperties {
    /// PAM service name to use for authorization
    pub service: String,
}

/// Properties for authz-simple objects.
///
#[qapi(added = "6.0.0")]
pub struct AuthZSimpleProperties {
    /// Identifies the allowed user.  Its format depends on the
    /// network service that authorization object is associated with.
    /// For authorizing based on TLS x509 certificates, the identity
    /// must be the x509 distinguished name.
    pub identity: String,
}

#[qapi(added = "4.2.0")]
pub struct BackupCommon {
    #[qapi(name = "auto-dismiss")]
    pub auto_dismiss: Option<bool>,
    #[qapi(name = "auto-finalize")]
    pub auto_finalize: Option<bool>,
    pub bitmap: Option<String>,
    #[qapi(name = "bitmap-mode")]
    pub bitmap_mode: Option<BitmapSyncMode>,
    pub compress: Option<bool>,
    pub device: String,
    #[qapi(name = "discard-source")]
    #[qapi(added = "9.1.0")]
    pub discard_source: Option<bool>,
    #[qapi(name = "filter-node-name")]
    pub filter_node_name: Option<String>,
    #[qapi(name = "job-id")]
    pub job_id: Option<String>,
    #[qapi(name = "on-cbw-error")]
    #[qapi(added = "10.1.0")]
    pub on_cbw_error: Option<OnCbwError>,
    #[qapi(name = "on-source-error")]
    pub on_source_error: Option<BlockdevOnError>,
    #[qapi(name = "on-target-error")]
    pub on_target_error: Option<BlockdevOnError>,
    pub speed: Option<i64>,
    pub sync: MirrorSyncMode,
    #[qapi(name = "x-perf")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "6.0.0")]
    pub x_perf: Option<BackupPerf>,
}

/// Optional parameters for backup.  These parameters don't affect
/// functionality, but may significantly affect performance.
///
#[qapi(added = "6.0.0")]
pub struct BackupPerf {
    /// Maximum request length for the sustained background
    /// copying process.  Doesn't influence copy-before-write
    /// operations.  0 means unlimited.  If max-chunk is non-zero then
    /// it should not be less than job cluster size which is calculated
    /// as maximum of target image cluster size and 64k.  Default 0.
    #[qapi(name = "max-chunk")]
    pub max_chunk: Option<i64>,
    /// Maximum number of parallel requests for the sustained
    /// background copying process.  Doesn't influence copy-before-write
    /// operations.  Default 64.
    #[qapi(name = "max-workers")]
    pub max_workers: Option<i64>,
    /// Minimum size of blocks used by copy-before-write
    /// and background copy operations.  Has to be a power of 2.  No
    /// effect if smaller than the maximum of the target's cluster size
    /// and 64 KiB.  Default 0.  (Since 9.2)
    #[qapi(name = "min-cluster-size")]
    #[qapi(added = "9.2.0")]
    pub min_cluster_size: Option<u64>,
    /// Use copy offloading.  Default false.
    #[qapi(name = "use-copy-range")]
    pub use_copy_range: Option<bool>,
}

/// Information about the guest balloon device.
///
#[qapi(added = "1.0.0")]
pub struct BalloonInfo {
    /// the logical size of the VM in bytes.  Formula used:
    /// logical_vm_size = vm_ram_size - balloon_size
    pub actual: i64,
    #[qapi(removed = "1.4.0")]
    pub free_mem: Option<i64>,
    #[qapi(removed = "1.4.0")]
    pub major_page_faults: Option<i64>,
    #[qapi(removed = "1.4.0")]
    pub mem_swapped_in: Option<i64>,
    #[qapi(removed = "1.4.0")]
    pub mem_swapped_out: Option<i64>,
    #[qapi(removed = "1.4.0")]
    pub minor_page_faults: Option<i64>,
    #[qapi(removed = "1.4.0")]
    pub total_mem: Option<i64>,
}

/// Policy that BIOS should use to interpret cylinder/head/sector
/// addresses.  Note that Bochs BIOS and SeaBIOS will not actually
/// translate logical CHS to physical; instead, they will use logical
/// block addressing.
///
#[qapi(added = "2.0.0")]
pub enum BiosAtaTranslation {
    /// If cylinder/heads/sizes are passed, choose between none and
    /// LBA depending on the size of the disk.  If they are not passed,
    /// choose none if QEMU can guess that the disk had 16 or fewer
    /// heads, large if QEMU can guess that the disk had 131072 or fewer
    /// tracks across all heads (i.e. cylinders*heads<131072), otherwise
    /// LBA.
    #[qapi(name = "auto")]
    Auto,
    /// The number of cylinders per head is scaled down to 1024 by
    /// correspondingly scaling up the number of heads.
    #[qapi(name = "large")]
    Large,
    /// Assume 63 sectors per track and one of 16, 32, 64, 128 or 255
    /// heads (if fewer than 255 are enough to cover the whole disk with
    /// 1024 cylinders/head).  The number of cylinders/head is then
    /// computed based on the number of sectors and heads.
    #[qapi(name = "lba")]
    Lba,
    /// The physical disk geometry is equal to the logical geometry.
    #[qapi(name = "none")]
    None,
    /// Same as @large, but first convert a 16-head geometry to
    /// 15-head, by proportionally scaling up the number of
    /// cylinders/head.
    #[qapi(name = "rechs")]
    Rechs,
}

#[qapi(added = "5.2.0")]
pub struct BitmapMigrationBitmapAlias {
    /// An alias name for migration (for example the bitmap name on
    /// the opposite site).
    pub alias: String,
    /// The name of the bitmap.
    pub name: String,
    /// Allows the modification of the migrated bitmap.
    /// (since 6.0)
    #[qapi(added = "6.0.0")]
    pub transform: Option<BitmapMigrationBitmapAliasTransform>,
}

#[qapi(added = "6.0.0")]
pub struct BitmapMigrationBitmapAliasTransform {
    /// If present, the bitmap will be made persistent or
    /// transient depending on this parameter.
    pub persistent: Option<bool>,
}

/// Maps a block node name and the bitmaps it has to aliases for dirty
/// bitmap migration.
///
#[qapi(added = "5.2.0")]
pub struct BitmapMigrationNodeAlias {
    /// An alias block node name for migration (for example the node
    /// name on the opposite site).
    pub alias: String,
    /// Mappings for the bitmaps on this node.
    pub bitmaps: Vec<BitmapMigrationBitmapAlias>,
    /// A block node name.
    #[qapi(name = "node-name")]
    pub node_name: String,
}

/// An enumeration of possible behaviors for the synchronization of a
/// bitmap when used for data copy operations.
///
#[qapi(added = "4.2.0")]
pub enum BitmapSyncMode {
    /// The bitmap is always synchronized with the operation,
    /// regardless of whether or not the operation was successful.
    #[qapi(name = "always")]
    Always,
    /// The bitmap is never synchronized with the operation, and is
    /// treated solely as a read-only manifest of blocks to copy.
    #[qapi(name = "never")]
    Never,
    /// The bitmap is only synced when the operation is
    /// successful.  This is the behavior always used for incremental
    /// backups.
    #[qapi(name = "on-success")]
    OnSuccess,
}

/// Trigger events supported by blkdebug.
///
#[qapi(added = "2.0.0")]
pub enum BlkdebugEvent {
    #[qapi(name = "cluster_alloc")]
    ClusterAlloc,
    #[qapi(name = "cluster_alloc_bytes")]
    ClusterAllocBytes,
    /// an allocation of file space for a cluster
    /// (since 4.1)
    #[qapi(name = "cluster_alloc_space")]
    #[qapi(added = "4.1.0")]
    ClusterAllocSpace,
    #[qapi(name = "cluster_free")]
    ClusterFree,
    /// a write due to copy-on-read (since 2.11)
    #[qapi(name = "cor_write")]
    #[qapi(added = "2.11.0")]
    CorWrite,
    #[qapi(name = "cow_read")]
    CowRead,
    #[qapi(name = "cow_write")]
    CowWrite,
    #[qapi(name = "empty_image_prepare")]
    #[qapi(added = "2.2.0")]
    EmptyImagePrepare,
    #[qapi(name = "flush_to_disk")]
    FlushToDisk,
    #[qapi(name = "flush_to_os")]
    FlushToOs,
    #[qapi(name = "l1_grow_activate_table")]
    L1GrowActivateTable,
    #[qapi(name = "l1_grow_alloc_table")]
    L1GrowAllocTable,
    #[qapi(name = "l1_grow_write_table")]
    L1GrowWriteTable,
    /// discard the l2 tables.  (since 2.11)
    #[qapi(name = "l1_shrink_free_l2_clusters")]
    #[qapi(added = "2.11.0")]
    L1ShrinkFreeL2Clusters,
    /// write zeros to the l1 table to shrink image.
    /// (since 2.11)
    #[qapi(name = "l1_shrink_write_table")]
    #[qapi(added = "2.11.0")]
    L1ShrinkWriteTable,
    #[qapi(name = "l1_update")]
    L1Update,
    #[qapi(name = "l2_alloc_cow_read")]
    L2AllocCowRead,
    #[qapi(name = "l2_alloc_write")]
    L2AllocWrite,
    #[qapi(name = "l2_load")]
    L2Load,
    #[qapi(name = "l2_update")]
    L2Update,
    #[qapi(name = "l2_update_compressed")]
    L2UpdateCompressed,
    /// triggers once at creation of the blkdebug node (since 4.1)
    #[qapi(name = "none")]
    #[qapi(added = "4.1.0")]
    None,
    #[qapi(name = "pwritev")]
    #[qapi(added = "2.2.0")]
    Pwritev,
    #[qapi(name = "pwritev_done")]
    #[qapi(added = "2.2.0")]
    PwritevDone,
    #[qapi(name = "pwritev_rmw_after_head")]
    #[qapi(added = "2.2.0")]
    PwritevRmwAfterHead,
    #[qapi(name = "pwritev_rmw_after_tail")]
    #[qapi(added = "2.2.0")]
    PwritevRmwAfterTail,
    #[qapi(name = "pwritev_rmw_head")]
    #[qapi(added = "2.2.0")]
    PwritevRmwHead,
    #[qapi(name = "pwritev_rmw_tail")]
    #[qapi(added = "2.2.0")]
    PwritevRmwTail,
    #[qapi(name = "pwritev_zero")]
    #[qapi(added = "2.2.0")]
    PwritevZero,
    #[qapi(name = "read_aio")]
    ReadAio,
    #[qapi(name = "read_backing_aio")]
    ReadBackingAio,
    #[qapi(name = "read_compressed")]
    ReadCompressed,
    #[qapi(name = "refblock_alloc")]
    RefblockAlloc,
    #[qapi(name = "refblock_alloc_hookup")]
    RefblockAllocHookup,
    #[qapi(name = "refblock_alloc_switch_table")]
    RefblockAllocSwitchTable,
    #[qapi(name = "refblock_alloc_write")]
    RefblockAllocWrite,
    #[qapi(name = "refblock_alloc_write_blocks")]
    RefblockAllocWriteBlocks,
    #[qapi(name = "refblock_alloc_write_table")]
    RefblockAllocWriteTable,
    #[qapi(name = "refblock_load")]
    RefblockLoad,
    #[qapi(name = "refblock_update")]
    RefblockUpdate,
    #[qapi(name = "refblock_update_part")]
    RefblockUpdatePart,
    #[qapi(name = "reftable_grow")]
    ReftableGrow,
    #[qapi(name = "reftable_load")]
    ReftableLoad,
    #[qapi(name = "reftable_update")]
    ReftableUpdate,
    #[qapi(name = "vmstate_load")]
    VmstateLoad,
    #[qapi(name = "vmstate_save")]
    VmstateSave,
    #[qapi(name = "write_aio")]
    WriteAio,
    #[qapi(name = "write_compressed")]
    WriteCompressed,
}

/// Describes a single error injection for blkdebug.
///
#[qapi(added = "2.0.0")]
pub struct BlkdebugInjectErrorOptions {
    /// request delay before completion in nanoseconds
    /// (default: 0, since: 11.1)
    #[qapi(name = "delay-ns")]
    #[qapi(added = "10.0.10")]
    pub delay_ns: Option<i64>,
    /// error identifier (errno) to be returned; defaults to EIO
    pub errno: Option<i64>,
    /// trigger event
    pub event: BlkdebugEvent,
    /// fail immediately; defaults to false
    pub immediately: Option<bool>,
    /// the type of I/O operations on which this error should be
    /// injected; defaults to "all read, write, write-zeroes, discard,
    /// and flush operations" (since: 4.1)
    #[qapi(added = "4.1.0")]
    pub iotype: Option<BlkdebugIoType>,
    /// disables further events after this one has been triggered;
    /// defaults to false
    pub once: Option<bool>,
    /// specifies the sector index which has to be affected in
    /// order to actually trigger the event; defaults to "any sector"
    pub sector: Option<i64>,
    /// the state identifier blkdebug needs to be in to actually
    /// trigger the event; defaults to "any"
    pub state: Option<i64>,
}

/// Kinds of I/O that blkdebug can inject errors in.
///
#[qapi(name = "BlkdebugIOType")]
#[qapi(added = "4.1.0")]
pub enum BlkdebugIoType {
    /// .bdrv_co_block_status()
    #[qapi(name = "block-status")]
    BlockStatus,
    /// .bdrv_co_pdiscard()
    #[qapi(name = "discard")]
    Discard,
    /// .bdrv_co_flush_to_disk()
    #[qapi(name = "flush")]
    Flush,
    /// .bdrv_co_preadv()
    #[qapi(name = "read")]
    Read,
    /// .bdrv_co_pwritev()
    #[qapi(name = "write")]
    Write,
    /// .bdrv_co_pwrite_zeroes()
    #[qapi(name = "write-zeroes")]
    WriteZeroes,
}

/// Describes a single state-change event for blkdebug.
///
#[qapi(added = "2.0.0")]
pub struct BlkdebugSetStateOptions {
    /// trigger event
    pub event: BlkdebugEvent,
    /// the state identifier blkdebug is supposed to assume if
    /// this event is triggered
    pub new_state: i64,
    /// the current state identifier blkdebug needs to be in;
    /// defaults to "any"
    pub state: Option<i64>,
}

/// Information about all nodes in the block graph starting at some
/// node, annotated with information about that node in relation to its
/// parent.
///
#[qapi(added = "8.0.0")]
pub struct BlockChildInfo {
    /// Block graph information starting at this node
    pub info: Box<BlockGraphInfo>,
    /// Child name of the root node in the `BlockGraphInfo` struct,
    /// in its role as the child of some undescribed parent node
    pub name: String,
}

/// A discriminated record of operations that can be performed with
#[qapi(added = "1.1.0")]
#[qapi(removed = "1.6.0")]
pub struct BlockdevAction {
    #[qapi(discriminator)]
    #[qapi(name = "type")]
    pub r_type: BlockdevActionType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<BlockdevActionBranch>,
}

#[qapi(added = "1.1.0")]
#[qapi(removed = "1.6.0")]
pub enum BlockdevActionBranch {
    #[qapi(name = "blockdev-snapshot-sync")]
    BlockdevSnapshotSync(BlockdevSnapshot),
}

#[qapi(added = "1.1.0")]
#[qapi(removed = "1.6.0")]
pub enum BlockdevActionType {
    #[qapi(name = "blockdev-snapshot-sync")]
    BlockdevSnapshotSync,
}

/// Selects the AIO backend to handle I/O requests
///
#[qapi(added = "1.7.0")]
pub enum BlockdevAioOptions {
    /// Use linux io_uring (since 5.0)
    #[qapi(name = "io_uring")]
    #[qapi(condition = "defined(CONFIG_LINUX_IO_URING)")]
    #[qapi(added = "5.0.0")]
    IoUring,
    /// Use native AIO backend (only Linux and Windows)
    #[qapi(name = "native")]
    Native,
    /// Use QEMU's thread pool
    #[qapi(name = "threads")]
    Threads,
}

/// Options for amending an image format
///
#[qapi(added = "5.1.0")]
pub struct BlockdevAmendOptions {
    /// Block driver of the node to amend.
    #[qapi(discriminator)]
    pub driver: BlockdevDriver,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<BlockdevAmendOptionsBranch>,
}

#[qapi(added = "5.1.0")]
pub enum BlockdevAmendOptionsBranch {
    #[qapi(name = "blkdebug")]
    Blkdebug,
    /// Since 3.0
    #[qapi(name = "blklogwrites")]
    Blklogwrites,
    /// Since 4.2
    #[qapi(name = "blkreplay")]
    Blkreplay,
    #[qapi(name = "blkverify")]
    Blkverify,
    #[qapi(name = "bochs")]
    Bochs,
    #[qapi(name = "cloop")]
    Cloop,
    /// Since 5.0
    #[qapi(name = "compress")]
    Compress,
    /// Since 6.2
    #[qapi(name = "copy-before-write")]
    #[qapi(added = "6.2.0")]
    CopyBeforeWrite,
    /// Since 3.0
    #[qapi(name = "copy-on-read")]
    CopyOnRead,
    #[qapi(name = "dmg")]
    Dmg,
    #[qapi(name = "file")]
    File,
    #[qapi(name = "ftp")]
    Ftp,
    #[qapi(name = "ftps")]
    Ftps,
    #[qapi(name = "gluster")]
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "9.2.0"))]
    #[qapi(removed = "11.1.0")]
    Gluster,
    #[qapi(name = "host_cdrom")]
    #[qapi(condition = "defined(HAVE_HOST_BLOCK_DEVICE)")]
    HostCdrom,
    #[qapi(name = "host_device")]
    #[qapi(condition = "defined(HAVE_HOST_BLOCK_DEVICE)")]
    HostDevice,
    #[qapi(name = "http")]
    Http,
    #[qapi(name = "https")]
    Https,
    #[qapi(name = "io_uring")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    IoUring,
    #[qapi(name = "iscsi")]
    Iscsi,
    #[qapi(name = "luks")]
    Luks(BlockdevAmendOptionsLuks),
    #[qapi(name = "nbd")]
    Nbd,
    #[qapi(name = "nfs")]
    Nfs,
    #[qapi(name = "null-aio")]
    NullAio,
    #[qapi(name = "null-co")]
    NullCo,
    /// Since 2.12
    #[qapi(name = "nvme")]
    Nvme,
    #[qapi(name = "nvme-io_uring")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    NvmeIoUring,
    #[qapi(name = "parallels")]
    Parallels,
    #[qapi(name = "preallocate")]
    #[qapi(added = "6.0.0")]
    Preallocate,
    #[qapi(name = "qcow")]
    Qcow,
    #[qapi(name = "qcow2")]
    Qcow2(BlockdevAmendOptionsQcow2),
    #[qapi(name = "qed")]
    Qed,
    #[qapi(name = "quorum")]
    Quorum,
    #[qapi(name = "raw")]
    Raw,
    #[qapi(name = "rbd")]
    Rbd,
    #[qapi(name = "replication")]
    #[qapi(condition = "defined(CONFIG_REPLICATION)")]
    Replication,
    #[qapi(name = "sheepdog")]
    #[qapi(removed = "6.1.0")]
    Sheepdog,
    /// Since 7.0
    #[qapi(name = "snapshot-access")]
    #[qapi(added = "7.0.0")]
    SnapshotAccess,
    #[qapi(name = "ssh")]
    Ssh,
    /// Since 2.11
    #[qapi(name = "throttle")]
    Throttle,
    #[qapi(name = "vdi")]
    Vdi,
    #[qapi(name = "vhdx")]
    Vhdx,
    #[qapi(name = "virtio-blk-vfio-pci")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    VirtioBlkVfioPci,
    #[qapi(name = "virtio-blk-vhost-user")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    VirtioBlkVhostUser,
    #[qapi(name = "virtio-blk-vhost-vdpa")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    VirtioBlkVhostVdpa,
    #[qapi(name = "vmdk")]
    Vmdk,
    #[qapi(name = "vpc")]
    Vpc,
    #[qapi(name = "vvfat")]
    Vvfat,
}

/// Driver specific image amend options for LUKS.
///
#[qapi(name = "BlockdevAmendOptionsLUKS")]
#[qapi(added = "5.1.0")]
pub struct BlockdevAmendOptionsLuks {
    /// Optional (for activation only).  Number of milliseconds
    /// to spend in PBKDF passphrase processing for the newly activated
    /// keyslot.  Currently defaults to 2000.
    #[qapi(name = "iter-time")]
    pub iter_time: Option<i64>,
    /// Optional.  ID of the keyslot to activate/deactivate.  For
    /// keyslot activation, keyslot should not be active already (this
    /// is unsafe to update an active keyslot), but possible if 'force'
    /// parameter is given.  If keyslot is not given, first free keyslot
    /// will be written.
    ///
    /// For keyslot deactivation, this parameter specifies the exact
    /// keyslot to deactivate
    pub keyslot: Option<i64>,
    /// The ID of a QCryptoSecret object providing the password
    /// to be written into added active keyslots
    #[qapi(name = "new-secret")]
    pub new_secret: Option<String>,
    /// Optional (for deactivation only).  If given will
    /// deactivate all keyslots that match password located in
    /// QCryptoSecret with this ID
    #[qapi(name = "old-secret")]
    pub old_secret: Option<String>,
    /// Optional.  The ID of a QCryptoSecret object providing the
    /// password to use to retrieve current master key.  Defaults to the
    /// same secret that was used to open the image
    pub secret: Option<String>,
    /// the desired state of the keyslots
    pub state: QCryptoBlockLuksKeyslotState,
}

/// Driver specific image amend options for qcow2.  For now, only
/// encryption options can be amended
///
#[qapi(added = "5.1.0")]
pub struct BlockdevAmendOptionsQcow2 {
    /// Encryption options to be amended
    pub encrypt: Option<QCryptoBlockAmendOptions>,
}

/// Start a point-in-time copy of a block device to a new destination.  The
/// status of ongoing blockdev-backup operations can be checked with
/// query-block-jobs where the BlockJobInfo.type field has the value 'backup'.
/// The operation can be stopped before it has completed using the
/// block-job-cancel command.
///
/// For the arguments, see the documentation of BlockdevBackup.
///
#[qapi(name = "blockdev-backup")]
#[qapi(command)]
#[qapi(returns = "()")]
#[qapi(added = "2.3.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockdevBackup {
    #[qapi(name = "auto-dismiss")]
    #[qapi(added = "2.12.0")]
    pub auto_dismiss: Option<bool>,
    #[qapi(name = "auto-finalize")]
    #[qapi(added = "2.12.0")]
    pub auto_finalize: Option<bool>,
    #[qapi(added = "3.1.0")]
    pub bitmap: Option<String>,
    #[qapi(name = "bitmap-mode")]
    #[qapi(added = "4.2.0")]
    pub bitmap_mode: Option<BitmapSyncMode>,
    #[qapi(added = "2.8.0")]
    pub compress: Option<bool>,
    pub device: String,
    #[qapi(name = "discard-source")]
    #[qapi(added = "9.1.0")]
    pub discard_source: Option<bool>,
    #[qapi(name = "filter-node-name")]
    #[qapi(added = "4.2.0")]
    pub filter_node_name: Option<String>,
    #[qapi(name = "job-id")]
    #[qapi(added = "2.7.0")]
    pub job_id: Option<String>,
    #[qapi(name = "on-cbw-error")]
    #[qapi(added = "10.1.0")]
    pub on_cbw_error: Option<OnCbwError>,
    #[qapi(name = "on-source-error")]
    pub on_source_error: Option<BlockdevOnError>,
    #[qapi(name = "on-target-error")]
    pub on_target_error: Option<BlockdevOnError>,
    pub speed: Option<i64>,
    pub sync: MirrorSyncMode,
    /// the device name or node-name of the backup target node.
    pub target: String,
    #[qapi(name = "x-perf")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "6.0.0")]
    pub x_perf: Option<BackupPerf>,
}

#[qapi(added = "6.2.0")]
pub struct BlockdevBackupWrapper {
    pub data: BlockdevBackup,
}

/// Cache mode information for a block device
///
#[qapi(added = "2.3.0")]
pub struct BlockdevCacheInfo {
    /// true if the host page cache is bypassed (O_DIRECT)
    pub direct: bool,
    /// true if flush requests are ignored for the device
    #[qapi(name = "no-flush")]
    pub no_flush: bool,
    /// true if writeback mode is enabled
    pub writeback: bool,
}

/// Includes cache-related options for block devices
///
#[qapi(added = "1.7.0")]
pub struct BlockdevCacheOptions {
    /// enables use of O_DIRECT (bypass the host page cache;
    /// default: false)
    pub direct: Option<bool>,
    /// ignore any flush requests for the device (default: false)
    #[qapi(name = "no-flush")]
    pub no_flush: Option<bool>,
    /// optional enables writeback mode for any caches (default: true)
    #[qapi(removed = "2.6.0")]
    pub writeback: Option<bool>,
}

/// Specifies the new read-only mode of a block device subject to the
/// `blockdev-change-medium` command.
///
#[qapi(added = "2.5.0")]
pub enum BlockdevChangeReadOnlyMode {
    /// Makes the device read-only
    #[qapi(name = "read-only")]
    ReadOnly,
    /// Makes the device writable
    #[qapi(name = "read-write")]
    ReadWrite,
    /// Retains the current read-only mode
    #[qapi(name = "retain")]
    Retain,
}

#[qapi(added = "10.1.0")]
pub struct BlockdevChild {
    /// The name of the child, for example 'file' or 'backing'.
    pub child: String,
    /// The name of the child's block driver node.
    #[qapi(name = "node-name")]
    pub node_name: String,
}

/// This is used for all drivers that don't support creating images.
///
#[qapi(added = "2.12.0")]
#[qapi(removed = "3.0.0")]
pub struct BlockdevCreateNotSupported {}

/// Options for creating an image format on a given node.
///
#[qapi(added = "2.12.0")]
pub struct BlockdevCreateOptions {
    /// block driver to create the image format
    #[qapi(discriminator)]
    pub driver: BlockdevDriver,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<BlockdevCreateOptionsBranch>,
}

#[qapi(added = "2.12.0")]
pub enum BlockdevCreateOptionsBranch {
    #[qapi(name = "blkdebug")]
    Blkdebug,
    /// Since 3.0
    #[qapi(name = "blklogwrites")]
    #[qapi(added = "3.0.0")]
    Blklogwrites,
    /// Since 4.2
    #[qapi(name = "blkreplay")]
    #[qapi(added = "4.2.0")]
    Blkreplay,
    #[qapi(name = "blkverify")]
    Blkverify,
    #[qapi(name = "bochs")]
    Bochs,
    #[qapi(name = "cloop")]
    Cloop,
    /// Since 5.0
    #[qapi(name = "compress")]
    #[qapi(added = "5.0.0")]
    Compress,
    /// Since 6.2
    #[qapi(name = "copy-before-write")]
    #[qapi(added = "6.2.0")]
    CopyBeforeWrite,
    /// Since 3.0
    #[qapi(name = "copy-on-read")]
    #[qapi(added = "3.0.0")]
    CopyOnRead,
    #[qapi(name = "dmg")]
    Dmg,
    #[qapi(name = "file")]
    File(BlockdevCreateOptionsFile),
    #[qapi(name = "ftp")]
    Ftp,
    #[qapi(name = "ftps")]
    Ftps,
    #[qapi(name = "gluster")]
    #[qapi(removed = "11.1.0")]
    Gluster(BlockdevCreateOptionsGluster),
    #[qapi(name = "host_cdrom")]
    #[qapi(condition = "defined(HAVE_HOST_BLOCK_DEVICE)")]
    HostCdrom,
    #[qapi(name = "host_device")]
    #[qapi(condition = "defined(HAVE_HOST_BLOCK_DEVICE)")]
    HostDevice,
    #[qapi(name = "http")]
    Http,
    #[qapi(name = "https")]
    Https,
    #[qapi(name = "io_uring")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    IoUring,
    #[qapi(name = "iscsi")]
    Iscsi,
    #[qapi(name = "luks")]
    Luks(BlockdevCreateOptionsLuks),
    #[qapi(name = "nbd")]
    Nbd,
    #[qapi(name = "nfs")]
    Nfs(BlockdevCreateOptionsNfs),
    #[qapi(name = "null-aio")]
    NullAio,
    #[qapi(name = "null-co")]
    NullCo,
    /// Since 2.12
    #[qapi(name = "nvme")]
    Nvme,
    #[qapi(name = "nvme-io_uring")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    NvmeIoUring,
    #[qapi(name = "parallels")]
    Parallels(BlockdevCreateOptionsParallels),
    #[qapi(name = "preallocate")]
    #[qapi(added = "6.0.0")]
    Preallocate,
    #[qapi(name = "qcow")]
    Qcow(BlockdevCreateOptionsQcow),
    #[qapi(name = "qcow2")]
    Qcow2(BlockdevCreateOptionsQcow2),
    #[qapi(name = "qed")]
    Qed(BlockdevCreateOptionsQed),
    #[qapi(name = "quorum")]
    Quorum,
    #[qapi(name = "raw")]
    Raw,
    #[qapi(name = "rbd")]
    Rbd(BlockdevCreateOptionsRbd),
    #[qapi(name = "replication")]
    #[qapi(condition = "defined(CONFIG_REPLICATION)")]
    Replication,
    #[qapi(name = "sheepdog")]
    #[qapi(removed = "6.1.0")]
    Sheepdog(BlockdevCreateOptionsSheepdog),
    /// Since 7.0
    #[qapi(name = "snapshot-access")]
    #[qapi(added = "7.0.0")]
    SnapshotAccess,
    #[qapi(name = "ssh")]
    Ssh(BlockdevCreateOptionsSsh),
    /// Since 2.11
    #[qapi(name = "throttle")]
    Throttle,
    #[qapi(name = "vdi")]
    Vdi(BlockdevCreateOptionsVdi),
    #[qapi(name = "vhdx")]
    Vhdx(BlockdevCreateOptionsVhdx),
    #[qapi(name = "virtio-blk-vfio-pci")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    VirtioBlkVfioPci,
    #[qapi(name = "virtio-blk-vhost-user")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    VirtioBlkVhostUser,
    #[qapi(name = "virtio-blk-vhost-vdpa")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    VirtioBlkVhostVdpa,
    #[qapi(name = "vmdk")]
    Vmdk(BlockdevCreateOptionsVmdk),
    #[qapi(name = "vpc")]
    Vpc(BlockdevCreateOptionsVpc),
    #[qapi(name = "vvfat")]
    Vvfat,
    /// Since 2.10
    #[qapi(name = "vxhs")]
    #[qapi(removed = "5.1.0")]
    Vxhs,
}

/// Driver specific image creation options for file.
///
#[qapi(added = "2.12.0")]
pub struct BlockdevCreateOptionsFile {
    /// Extent size hint to add to the image file; 0 for
    /// not adding an extent size hint (default: 1 MB, since 5.1)
    #[qapi(name = "extent-size-hint")]
    #[qapi(added = "5.1.0")]
    pub extent_size_hint: Option<u64>,
    /// Filename for the new image file
    pub filename: String,
    /// Turn off copy-on-write (valid only on btrfs; default: off)
    pub nocow: Option<bool>,
    /// Preallocation mode for the new image (default: off;
    /// allowed values: off, falloc (if CONFIG_POSIX_FALLOCATE), full
    /// (if CONFIG_POSIX))
    pub preallocation: Option<PreallocMode>,
    /// Size of the virtual disk in bytes
    pub size: u64,
}

/// Driver specific image creation options for gluster.
///
#[qapi(added = "2.12.0")]
#[qapi(removed = "11.1.0")]
pub struct BlockdevCreateOptionsGluster {
    /// Where to store the new image file
    pub location: BlockdevOptionsGluster,
    /// Preallocation mode for the new image (default: off;
    /// allowed values: off, falloc (if CONFIG_GLUSTERFS_FALLOCATE),
    /// full (if CONFIG_GLUSTERFS_ZEROFILL))
    pub preallocation: Option<PreallocMode>,
    /// Size of the virtual disk in bytes
    pub size: u64,
}

/// Driver specific image creation options for LUKS.
///
#[qapi(name = "BlockdevCreateOptionsLUKS")]
#[qapi(added = "2.12.0")]
pub struct BlockdevCreateOptionsLuks {
    /// the cipher algorithm for data encryption.  Currently
    /// defaults to 'aes-256'.
    #[qapi(name = "cipher-alg")]
    pub cipher_alg: Option<QCryptoCipherAlgo>,
    /// the cipher mode for data encryption.  Currently
    /// defaults to 'xts'
    #[qapi(name = "cipher-mode")]
    pub cipher_mode: Option<QCryptoCipherMode>,
    /// create a detached LUKS header.  (since 9.0)
    #[qapi(name = "detached-header")]
    #[qapi(added = "9.0.0")]
    #[qapi(removed = "9.1.0")]
    pub detached_header: Option<bool>,
    /// Node to create the image format on, mandatory except when
    /// 'preallocation' is not requested
    pub file: Option<BlockdevRef>,
    /// the master key hash algorithm.  Currently defaults to
    /// 'sha256'
    #[qapi(name = "hash-alg")]
    pub hash_alg: Option<QCryptoHashAlgo>,
    /// Block device holding a detached LUKS header.  (since 9.0)
    #[qapi(added = "9.0.0")]
    pub header: Option<BlockdevRef>,
    /// number of milliseconds to spend in PBKDF passphrase
    /// processing.  Currently defaults to 2000.  (since 2.8)
    #[qapi(name = "iter-time")]
    pub iter_time: Option<i64>,
    /// the initialization vector generator.  Currently defaults
    /// to 'plain64'
    #[qapi(name = "ivgen-alg")]
    pub ivgen_alg: Option<QCryptoIvGenAlgo>,
    /// the initialization vector generator hash.
    /// Currently defaults to 'sha256'
    #[qapi(name = "ivgen-hash-alg")]
    pub ivgen_hash_alg: Option<QCryptoHashAlgo>,
    /// the ID of a QCryptoSecret object providing the
    /// decryption key.  Mandatory except when probing image for
    /// metadata only.
    #[qapi(name = "key-secret")]
    pub key_secret: Option<String>,
    /// Preallocation mode for the new image (since: 4.2)
    /// (default: off; allowed values: off, metadata, falloc, full)
    #[qapi(added = "4.2.0")]
    pub preallocation: Option<PreallocMode>,
    /// Size of the virtual disk in bytes
    pub size: u64,
}

/// Driver specific image creation options for NFS.
///
#[qapi(added = "2.12.0")]
pub struct BlockdevCreateOptionsNfs {
    /// Where to store the new image file
    pub location: BlockdevOptionsNfs,
    /// Size of the virtual disk in bytes
    pub size: u64,
}

/// Driver specific image creation options for parallels.
///
#[qapi(added = "2.12.0")]
pub struct BlockdevCreateOptionsParallels {
    /// Cluster size in bytes (default: 1 MB)
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<u64>,
    /// Node to create the image format on
    pub file: BlockdevRef,
    /// Size of the virtual disk in bytes
    pub size: u64,
}

/// Driver specific image creation options for qcow.
///
#[qapi(added = "2.12.0")]
pub struct BlockdevCreateOptionsQcow {
    /// File name of the backing file if a backing file
    /// should be used
    #[qapi(name = "backing-file")]
    pub backing_file: Option<String>,
    /// Encryption options if the image should be encrypted
    pub encrypt: Option<QCryptoBlockCreateOptions>,
    /// Node to create the image format on
    pub file: BlockdevRef,
    /// Size of the virtual disk in bytes
    pub size: u64,
}

/// Driver specific image creation options for qcow2.
///
#[qapi(added = "2.12.0")]
pub struct BlockdevCreateOptionsQcow2 {
    /// File name of the backing file if a backing file
    /// should be used
    #[qapi(name = "backing-file")]
    pub backing_file: Option<String>,
    /// Name of the block driver to use for the backing file
    #[qapi(name = "backing-fmt")]
    pub backing_fmt: Option<BlockdevDriver>,
    /// qcow2 cluster size in bytes (default: 65536)
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<u64>,
    /// The image cluster compression method
    /// (default: zlib, since 5.1)
    #[qapi(name = "compression-type")]
    #[qapi(added = "5.1.0")]
    pub compression_type: Option<Qcow2CompressionType>,
    /// Node to use as an external data file in which all guest
    /// data is stored so that only metadata remains in the qcow2 file
    /// (since: 4.0)
    #[qapi(name = "data-file")]
    #[qapi(added = "4.0.0")]
    pub data_file: Option<BlockdevRef>,
    /// True if the external data file must stay valid as a
    /// standalone (read-only) raw image without looking at qcow2
    /// metadata (default: false; since: 4.0)
    #[qapi(name = "data-file-raw")]
    #[qapi(added = "4.0.0")]
    pub data_file_raw: Option<bool>,
    /// Encryption options if the image should be encrypted
    pub encrypt: Option<QCryptoBlockCreateOptions>,
    /// True to make the image have extended L2 entries
    /// (default: false; since 5.2)
    #[qapi(name = "extended-l2")]
    #[qapi(added = "5.2.0")]
    pub extended_l2: Option<bool>,
    /// Node to create the image format on
    pub file: BlockdevRef,
    /// True if refcounts may be updated lazily
    /// (default: off)
    #[qapi(name = "lazy-refcounts")]
    pub lazy_refcounts: Option<bool>,
    /// Preallocation mode for the new image (default: off;
    /// allowed values: off, falloc, full, metadata)
    pub preallocation: Option<PreallocMode>,
    /// Width of reference counts in bits (default: 16)
    #[qapi(name = "refcount-bits")]
    pub refcount_bits: Option<i64>,
    /// Size of the virtual disk in bytes
    pub size: u64,
    /// Compatibility level (default: v3)
    pub version: Option<BlockdevQcow2Version>,
}

/// Driver specific image creation options for qed.
///
#[qapi(added = "2.12.0")]
pub struct BlockdevCreateOptionsQed {
    /// File name of the backing file if a backing file
    /// should be used
    #[qapi(name = "backing-file")]
    pub backing_file: Option<String>,
    /// Name of the block driver to use for the backing file
    #[qapi(name = "backing-fmt")]
    pub backing_fmt: Option<BlockdevDriver>,
    /// Cluster size in bytes (default: 65536)
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<u64>,
    /// Node to create the image format on
    pub file: BlockdevRef,
    /// Size of the virtual disk in bytes
    pub size: u64,
    /// L1/L2 table size (in clusters)
    #[qapi(name = "table-size")]
    pub table_size: Option<i64>,
}

/// Driver specific image creation options for rbd/Ceph.
///
#[qapi(added = "2.12.0")]
pub struct BlockdevCreateOptionsRbd {
    /// RBD object size
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<u64>,
    /// Image encryption options.  (Since 6.1)
    #[qapi(added = "6.1.0")]
    pub encrypt: Option<RbdEncryptionCreateOptions>,
    /// Where to store the new image file.  This location cannot
    /// point to a snapshot.
    pub location: BlockdevOptionsRbd,
    /// Size of the virtual disk in bytes
    pub size: u64,
}

/// Driver specific image creation options for Sheepdog.
///
#[qapi(added = "2.12.0")]
#[qapi(removed = "6.1.0")]
pub struct BlockdevCreateOptionsSheepdog {
    /// File name of a base image
    #[qapi(name = "backing-file")]
    pub backing_file: Option<String>,
    /// Where to store the new image file
    pub location: BlockdevOptionsSheepdog,
    /// Object size of the image
    #[qapi(name = "object-size")]
    pub object_size: Option<u64>,
    /// Preallocation mode for the new image (default: off;
    /// allowed values: off, full)
    pub preallocation: Option<PreallocMode>,
    /// Redundancy of the image
    pub redundancy: Option<SheepdogRedundancy>,
    /// Size of the virtual disk in bytes
    pub size: u64,
}

/// Driver specific image creation options for SSH.
///
#[qapi(added = "2.12.0")]
pub struct BlockdevCreateOptionsSsh {
    /// Where to store the new image file
    pub location: BlockdevOptionsSsh,
    /// Size of the virtual disk in bytes
    pub size: u64,
}

/// Driver specific image creation options for VDI.
///
#[qapi(added = "2.12.0")]
pub struct BlockdevCreateOptionsVdi {
    /// Node to create the image format on
    pub file: BlockdevRef,
    /// Preallocation mode for the new image (default: off;
    /// allowed values: off, metadata)
    pub preallocation: Option<PreallocMode>,
    /// Size of the virtual disk in bytes
    pub size: u64,
}

/// Driver specific image creation options for vhdx.
///
#[qapi(added = "2.12.0")]
pub struct BlockdevCreateOptionsVhdx {
    /// Block size in bytes, must be a multiple of 1 MB and not
    /// larger than 256 MB (default: automatically choose a block size
    /// depending on the image size)
    #[qapi(name = "block-size")]
    pub block_size: Option<u64>,
    /// Force use of payload blocks of type 'ZERO'.
    /// Non-standard, but default.  Do not set to 'off' when using
    /// 'qemu-img convert' with subformat=dynamic.
    #[qapi(name = "block-state-zero")]
    pub block_state_zero: Option<bool>,
    /// Node to create the image format on
    pub file: BlockdevRef,
    /// Log size in bytes, must be a multiple of 1 MB (default: 1
    /// MB)
    #[qapi(name = "log-size")]
    pub log_size: Option<u64>,
    /// Size of the virtual disk in bytes
    pub size: u64,
    /// vhdx subformat (default: dynamic)
    pub subformat: Option<BlockdevVhdxSubformat>,
}

/// Driver specific image creation options for VMDK.
///
#[qapi(added = "4.0.0")]
pub struct BlockdevCreateOptionsVmdk {
    /// The adapter type used to fill in the descriptor.
    /// Default: ide.
    #[qapi(name = "adapter-type")]
    pub adapter_type: Option<BlockdevVmdkAdapterType>,
    /// The path of backing file.  Default: no backing file
    /// is used.
    #[qapi(name = "backing-file")]
    pub backing_file: Option<String>,
    /// Where to store the data extents.  Required for
    /// monolithcFlat, twoGbMaxExtentSparse and twoGbMaxExtentFlat
    /// formats.  For monolithicFlat, only one entry is required; for
    /// twoGbMaxExtent* formats, the number of entries required is
    /// calculated as extent_number = virtual_size / 2GB.  Providing
    /// more extents than will be used is an error.
    pub extents: Option<Vec<BlockdevRef>>,
    /// Where to store the new image file.  This refers to the image
    /// file for monolithcSparse and streamOptimized format, or the
    /// descriptor file for other formats.
    pub file: BlockdevRef,
    /// Hardware version.  The meaningful options are "4" or
    /// "6".  Default: "4".
    pub hwversion: Option<String>,
    /// Size of the virtual disk in bytes
    pub size: u64,
    /// The subformat of the VMDK image.  Default:
    /// "monolithicSparse".
    pub subformat: Option<BlockdevVmdkSubformat>,
    /// VMware guest tools version.  Default: "2147483647"
    /// (Since 6.2)
    #[qapi(added = "6.2.0")]
    pub toolsversion: Option<String>,
    /// Whether to enable zeroed-grain feature for sparse
    /// subformats.  Default: false.
    #[qapi(name = "zeroed-grain")]
    pub zeroed_grain: Option<bool>,
}

/// Driver specific image creation options for vpc (VHD).
///
#[qapi(added = "2.12.0")]
pub struct BlockdevCreateOptionsVpc {
    /// Node to create the image format on
    pub file: BlockdevRef,
    /// Force use of the exact byte size instead of rounding to
    /// the next size that can be represented in CHS geometry
    /// (default: false)
    #[qapi(name = "force-size")]
    pub force_size: Option<bool>,
    /// Size of the virtual disk in bytes
    pub size: u64,
    /// vhdx subformat (default: dynamic)
    pub subformat: Option<BlockdevVpcSubformat>,
}

/// Describes the operation mode for the automatic conversion of plain
/// zero writes by the OS to driver specific optimized zero write
/// commands.
///
#[qapi(added = "2.1.0")]
pub enum BlockdevDetectZeroesOptions {
    /// Disabled (default)
    #[qapi(name = "off")]
    Off,
    /// Enabled
    #[qapi(name = "on")]
    On,
    /// Enabled and even try to unmap blocks if possible.  This
    /// requires also that `BlockdevDiscardOptions` is set to unmap for
    /// this device.
    #[qapi(name = "unmap")]
    Unmap,
}

/// Determines how to handle discard requests.
///
#[qapi(added = "1.7.0")]
pub enum BlockdevDiscardOptions {
    /// Ignore the request
    #[qapi(name = "ignore")]
    Ignore,
    /// Forward as an unmap request
    #[qapi(name = "unmap")]
    Unmap,
}

/// Drivers that are supported in block device operations.
///
#[qapi(added = "2.0.0")]
pub enum BlockdevDriver {
    #[qapi(name = "archipelago")]
    #[qapi(added = "2.2.0")]
    #[qapi(removed = "2.9.0")]
    Archipelago,
    #[qapi(name = "blkdebug")]
    Blkdebug,
    /// Since 3.0
    #[qapi(name = "blklogwrites")]
    #[qapi(added = "3.0.0")]
    Blklogwrites,
    /// Since 4.2
    #[qapi(name = "blkreplay")]
    #[qapi(added = "4.2.0")]
    Blkreplay,
    #[qapi(name = "blkverify")]
    Blkverify,
    #[qapi(name = "bochs")]
    Bochs,
    #[qapi(name = "cloop")]
    Cloop,
    /// Since 5.0
    #[qapi(name = "compress")]
    #[qapi(added = "5.0.0")]
    Compress,
    /// Since 6.2
    #[qapi(name = "copy-before-write")]
    #[qapi(added = "6.2.0")]
    CopyBeforeWrite,
    /// Since 3.0
    #[qapi(name = "copy-on-read")]
    #[qapi(added = "3.0.0")]
    CopyOnRead,
    #[qapi(name = "cow")]
    #[qapi(removed = "2.2.0")]
    Cow,
    #[qapi(name = "dmg")]
    Dmg,
    #[qapi(name = "file")]
    File,
    #[qapi(name = "ftp")]
    Ftp,
    #[qapi(name = "ftps")]
    Ftps,
    /// Since 2.7
    #[qapi(name = "gluster")]
    #[qapi(feature = "deprecated")]
    #[qapi(added = "2.7.0")]
    #[qapi(deprecated(added = "9.2.0"))]
    #[qapi(removed = "11.1.0")]
    Gluster,
    /// Since 2.1
    #[qapi(name = "host_cdrom")]
    #[qapi(condition = "defined(HAVE_HOST_BLOCK_DEVICE)")]
    #[qapi(added = "2.1.0")]
    HostCdrom,
    /// Since 2.1
    #[qapi(name = "host_device")]
    #[qapi(condition = "defined(HAVE_HOST_BLOCK_DEVICE)")]
    #[qapi(added = "2.1.0")]
    HostDevice,
    /// deprecated since 2.3
    #[qapi(name = "host_floppy")]
    #[qapi(added = "2.1.0")]
    #[qapi(removed = "2.5.0")]
    HostFloppy,
    #[qapi(name = "http")]
    Http,
    #[qapi(name = "https")]
    Https,
    #[qapi(name = "io_uring")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    IoUring,
    #[qapi(name = "iscsi")]
    #[qapi(added = "2.9.0")]
    Iscsi,
    #[qapi(name = "luks")]
    #[qapi(added = "2.6.0")]
    Luks,
    /// Since 2.8
    #[qapi(name = "nbd")]
    #[qapi(added = "2.8.0")]
    Nbd,
    /// Since 2.8
    #[qapi(name = "nfs")]
    #[qapi(added = "2.8.0")]
    Nfs,
    #[qapi(name = "null-aio")]
    #[qapi(added = "2.2.0")]
    NullAio,
    #[qapi(name = "null-co")]
    #[qapi(added = "2.2.0")]
    NullCo,
    /// Since 2.12
    #[qapi(name = "nvme")]
    #[qapi(added = "2.12.0")]
    Nvme,
    #[qapi(name = "nvme-io_uring")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    NvmeIoUring,
    #[qapi(name = "parallels")]
    Parallels,
    #[qapi(name = "preallocate")]
    #[qapi(added = "6.0.0")]
    Preallocate,
    #[qapi(name = "qcow")]
    Qcow,
    #[qapi(name = "qcow2")]
    Qcow2,
    #[qapi(name = "qed")]
    Qed,
    #[qapi(name = "quorum")]
    Quorum,
    #[qapi(name = "raw")]
    Raw,
    #[qapi(name = "rbd")]
    #[qapi(added = "2.9.0")]
    Rbd,
    /// Since 2.8
    #[qapi(name = "replication")]
    #[qapi(condition = "defined(CONFIG_REPLICATION)")]
    #[qapi(added = "2.8.0")]
    Replication,
    #[qapi(name = "sheepdog")]
    #[qapi(added = "2.9.0")]
    #[qapi(removed = "6.1.0")]
    Sheepdog,
    /// Since 7.0
    #[qapi(name = "snapshot-access")]
    #[qapi(added = "7.0.0")]
    SnapshotAccess,
    /// Since 2.8
    #[qapi(name = "ssh")]
    #[qapi(added = "2.8.0")]
    Ssh,
    #[qapi(name = "tftp")]
    #[qapi(removed = "2.8.0")]
    Tftp,
    /// Since 2.11
    #[qapi(name = "throttle")]
    #[qapi(added = "2.11.0")]
    Throttle,
    #[qapi(name = "vdi")]
    Vdi,
    #[qapi(name = "vhdx")]
    Vhdx,
    #[qapi(name = "virtio-blk-vfio-pci")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    VirtioBlkVfioPci,
    #[qapi(name = "virtio-blk-vhost-user")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    VirtioBlkVhostUser,
    #[qapi(name = "virtio-blk-vhost-vdpa")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    VirtioBlkVhostVdpa,
    #[qapi(name = "vmdk")]
    Vmdk,
    #[qapi(name = "vpc")]
    Vpc,
    #[qapi(name = "vvfat")]
    Vvfat,
    /// Since 2.10
    #[qapi(name = "vxhs")]
    #[qapi(added = "2.10.0")]
    #[qapi(removed = "5.1.0")]
    Vxhs,
}

/// Information about the backing device for a block device.
///
#[qapi(added = "1.0.0")]
pub struct BlockDeviceInfo {
    /// true if the backend is active; typical cases for inactive
    /// backends are on the migration source instance after migration
    /// completes and on the destination before it completes.
    /// (since: 10.0)
    #[qapi(added = "10.0.0")]
    pub active: Option<bool>,
    /// the name of the backing file (for copy-on-write)
    pub backing_file: Option<String>,
    /// number of files in the backing file chain
    /// (since: 1.2)
    #[qapi(added = "1.2.0")]
    pub backing_file_depth: Option<i64>,
    /// total throughput limit in bytes per second is specified
    #[qapi(added = "1.1.0")]
    pub bps: Option<i64>,
    /// total throughput limit during bursts, in bytes (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub bps_max: Option<i64>,
    /// maximum length of the @bps_max burst period, in
    /// seconds.  (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub bps_max_length: Option<i64>,
    /// read throughput limit in bytes per second is specified
    #[qapi(added = "1.1.0")]
    pub bps_rd: Option<i64>,
    /// read throughput limit during bursts, in bytes
    /// (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub bps_rd_max: Option<i64>,
    /// maximum length of the @bps_rd_max burst period,
    /// in seconds.  (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub bps_rd_max_length: Option<i64>,
    /// write throughput limit in bytes per second is specified
    #[qapi(added = "1.1.0")]
    pub bps_wr: Option<i64>,
    /// write throughput limit during bursts, in bytes
    /// (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub bps_wr_max: Option<i64>,
    /// maximum length of the @bps_wr_max burst period,
    /// in seconds.  (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub bps_wr_max_length: Option<i64>,
    /// the cache mode used for the block device (since: 2.3)
    #[qapi(added = "2.3.0")]
    pub cache: Option<BlockdevCacheInfo>,
    /// Information about child block nodes.  (since: 10.1)
    #[qapi(added = "10.1.0")]
    pub children: Option<Vec<BlockdevChild>>,
    /// detect and optimize zero writes (Since 2.1)
    #[qapi(added = "2.1.0")]
    pub detect_zeroes: Option<BlockdevDetectZeroesOptions>,
    /// dirty bitmaps information (only present if node has
    /// one or more dirty bitmaps) (Since 4.2)
    #[qapi(name = "dirty-bitmaps")]
    #[qapi(added = "4.2.0")]
    pub dirty_bitmaps: Option<Vec<BlockDirtyInfo>>,
    /// the name of the block format used to open the backing device.
    /// As of 0.14 this can be: 'blkdebug', 'bochs', 'cloop', 'cow',
    /// 'dmg', 'file', 'file', 'ftp', 'ftps', 'host_cdrom',
    /// 'host_device', 'http', 'https', 'luks', 'nbd', 'parallels',
    /// 'qcow', 'qcow2', 'raw', 'vdi', 'vmdk', 'vpc', 'vvfat' 2.2:
    /// 'archipelago' added, 'cow' dropped 2.3: 'host_floppy' deprecated
    /// 2.5: 'host_floppy' dropped 2.6: 'luks' added 2.8: 'replication'
    /// added, 'tftp' dropped 2.9: 'archipelago' dropped
    pub drv: String,
    /// true if the backing device is encrypted
    pub encrypted: bool,
    /// Deprecated; always false
    #[qapi(feature = "deprecated")]
    #[qapi(added = "1.2.0")]
    #[qapi(deprecated(added = "5.0.0"))]
    #[qapi(removed = "6.0.0")]
    pub encryption_key_missing: Option<bool>,
    /// the filename of the backing device
    pub file: String,
    /// throttle group name (Since 2.4)
    #[qapi(added = "2.4.0")]
    pub group: Option<String>,
    /// the info of image used (since: 1.6)
    #[qapi(added = "1.6.0")]
    pub image: Option<ImageInfo>,
    /// total I/O operations per second is specified
    #[qapi(added = "1.1.0")]
    pub iops: Option<i64>,
    /// total I/O operations per second during bursts, in bytes
    /// (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub iops_max: Option<i64>,
    /// maximum length of the @iops burst period, in
    /// seconds.  (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub iops_max_length: Option<i64>,
    /// read I/O operations per second is specified
    #[qapi(added = "1.1.0")]
    pub iops_rd: Option<i64>,
    /// read I/O operations per second during bursts, in bytes
    /// (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub iops_rd_max: Option<i64>,
    /// maximum length of the @iops_rd_max burst
    /// period, in seconds.  (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub iops_rd_max_length: Option<i64>,
    /// an I/O size in bytes (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub iops_size: Option<i64>,
    /// write I/O operations per second is specified
    #[qapi(added = "1.1.0")]
    pub iops_wr: Option<i64>,
    /// write I/O operations per second during bursts, in
    /// bytes (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub iops_wr_max: Option<i64>,
    /// maximum length of the @iops_wr_max burst
    /// period, in seconds.  (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub iops_wr_max_length: Option<i64>,
    /// the name of the block driver node (Since 2.0)
    #[qapi(name = "node-name")]
    #[qapi(added = "2.0.0")]
    pub node_name: Option<String>,
    /// true if the backing device was open read-only
    pub ro: bool,
    /// configured write threshold for the device.  0 if
    /// disabled.  (Since 2.3)
    #[qapi(added = "2.3.0")]
    pub write_threshold: Option<i64>,
}

/// An enumeration of block device I/O status.
///
#[qapi(added = "1.0.0")]
pub enum BlockDeviceIoStatus {
    /// The last I/O operation has failed
    #[qapi(name = "failed")]
    Failed,
    /// The last I/O operation has failed due to a no-space
    /// condition
    #[qapi(name = "nospace")]
    Nospace,
    /// The last I/O operation has succeeded
    #[qapi(name = "ok")]
    Ok,
}

/// Entry in the metadata map of the device (returned by "qemu-img map")
///
#[qapi(added = "1.7.0")]
#[qapi(removed = "5.2.0")]
pub struct BlockDeviceMapEntry {
    /// reading the image will actually read data from a file (in particular,
    /// if @offset is present this means that the sectors are not simply
    /// preallocated, but contain actual data in raw format)
    pub data: bool,
    /// Number of layers (0 = top image, 1 = top image's backing file, etc.)
    /// before reaching one for which the range is allocated.  The value is
    /// in the range 0 to the depth of the image chain - 1.
    pub depth: i64,
    /// Length of the range described by this entry (in bytes)
    pub length: i64,
    /// if present, the image file stores the data for this range in
    /// raw format at the given offset.
    pub offset: Option<i64>,
    /// Offset in the image of the first byte described by this entry
    /// (in bytes)
    pub start: i64,
    /// the sectors in this range read as zeros
    pub zero: bool,
}

/// Statistics of a virtual block device or a block backing device.
///
#[qapi(added = "1.0.0")]
pub struct BlockDeviceStats {
    /// Whether failed operations are included in the
    /// latency and last access statistics (Since 2.5)
    #[qapi(added = "2.5.0")]
    pub account_failed: Option<bool>,
    /// Whether invalid operations are included in the
    /// last access statistics (Since 2.5)
    #[qapi(added = "2.5.0")]
    pub account_invalid: Option<bool>,
    /// The number of failed flush operations
    /// performed by the device (Since 2.5)
    #[qapi(added = "2.5.0")]
    pub failed_flush_operations: Option<i64>,
    /// The number of failed read operations
    /// performed by the device (Since 2.5)
    #[qapi(added = "2.5.0")]
    pub failed_rd_operations: Option<i64>,
    /// The number of failed unmap operations
    /// performed by the device (Since 4.2)
    #[qapi(added = "4.2.0")]
    pub failed_unmap_operations: Option<i64>,
    /// The number of failed write operations
    /// performed by the device (Since 2.5)
    #[qapi(added = "2.5.0")]
    pub failed_wr_operations: Option<i64>,
    /// The number of failed zone append
    /// write operations performed by the zoned devices (since 8.1)
    #[qapi(added = "8.1.0")]
    pub failed_zone_append_operations: Option<i64>,
    /// `BlockLatencyHistogramInfo`.  (Since 4.0)
    #[qapi(added = "4.0.0")]
    pub flush_latency_histogram: Option<BlockLatencyHistogramInfo>,
    /// The number of cache flush operations performed by
    /// the device (since 0.15)
    pub flush_operations: i64,
    /// Total time spent on cache flushes in
    /// nanoseconds (since 0.15).
    pub flush_total_time_ns: i64,
    /// Time since the last I/O operation, in nanoseconds.
    /// If the field is absent it means that there haven't been any
    /// operations yet (Since 2.5).
    #[qapi(added = "2.5.0")]
    pub idle_time_ns: Option<i64>,
    /// The number of invalid flush operations
    /// performed by the device (Since 2.5)
    #[qapi(added = "2.5.0")]
    pub invalid_flush_operations: Option<i64>,
    /// The number of invalid read operations
    /// performed by the device (Since 2.5)
    #[qapi(added = "2.5.0")]
    pub invalid_rd_operations: Option<i64>,
    /// The number of invalid unmap operations
    /// performed by the device (Since 4.2)
    #[qapi(added = "4.2.0")]
    pub invalid_unmap_operations: Option<i64>,
    /// The number of invalid write operations
    /// performed by the device (Since 2.5)
    #[qapi(added = "2.5.0")]
    pub invalid_wr_operations: Option<i64>,
    /// The number of invalid zone append
    /// operations performed by the zoned device (since 8.1)
    #[qapi(added = "8.1.0")]
    pub invalid_zone_append_operations: Option<i64>,
    /// The number of bytes read by the device.
    pub rd_bytes: i64,
    /// `BlockLatencyHistogramInfo`.  (Since 4.0)
    #[qapi(added = "4.0.0")]
    pub rd_latency_histogram: Option<BlockLatencyHistogramInfo>,
    /// Number of read requests that have been merged into
    /// another request (Since 2.3).
    #[qapi(added = "2.3.0")]
    pub rd_merged: Option<i64>,
    /// The number of read operations performed by the
    /// device.
    pub rd_operations: i64,
    /// Total time spent on reads in nanoseconds
    /// (since 0.15)
    pub rd_total_time_ns: i64,
    /// Statistics specific to the set of previously defined
    /// intervals of time (Since 2.5)
    #[qapi(added = "2.5.0")]
    pub timed_stats: Option<Vec<BlockDeviceTimedStats>>,
    /// The number of bytes unmapped by the device (Since 4.2)
    #[qapi(added = "4.2.0")]
    pub unmap_bytes: Option<i64>,
    /// Number of unmap requests that have been merged into
    /// another request (Since 4.2)
    #[qapi(added = "4.2.0")]
    pub unmap_merged: Option<i64>,
    /// The number of unmap operations performed by the
    /// device (Since 4.2)
    #[qapi(added = "4.2.0")]
    pub unmap_operations: Option<i64>,
    /// Total time spent on unmap operations in
    /// nanoseconds (Since 4.2)
    #[qapi(added = "4.2.0")]
    pub unmap_total_time_ns: Option<i64>,
    /// The number of bytes written by the device.
    pub wr_bytes: i64,
    /// The offset after the greatest byte written to
    /// the device.  The intended use of this information is for
    /// growable sparse files (like qcow2) that are used on top of a
    /// physical device.
    pub wr_highest_offset: i64,
    /// `BlockLatencyHistogramInfo`.  (Since 4.0)
    #[qapi(added = "4.0.0")]
    pub wr_latency_histogram: Option<BlockLatencyHistogramInfo>,
    /// Number of write requests that have been merged into
    /// another request (Since 2.3).
    #[qapi(added = "2.3.0")]
    pub wr_merged: Option<i64>,
    /// The number of write operations performed by the
    /// device.
    pub wr_operations: i64,
    /// Total time spent on writes in nanoseconds
    /// (since 0.15)
    pub wr_total_time_ns: i64,
    /// @BlockLatencyHistogramInfo. (Since 2.12)
    #[qapi(added = "2.12.0")]
    #[qapi(removed = "4.0.0")]
    pub x_flush_latency_histogram: Option<BlockLatencyHistogramInfo>,
    /// @BlockLatencyHistogramInfo. (Since 2.12)
    #[qapi(added = "2.12.0")]
    #[qapi(removed = "4.0.0")]
    pub x_rd_latency_histogram: Option<BlockLatencyHistogramInfo>,
    /// @BlockLatencyHistogramInfo. (Since 2.12)
    #[qapi(added = "2.12.0")]
    #[qapi(removed = "4.0.0")]
    pub x_wr_latency_histogram: Option<BlockLatencyHistogramInfo>,
    /// The number of bytes appended by the zoned
    /// devices (since 8.1)
    #[qapi(added = "8.1.0")]
    pub zone_append_bytes: Option<i64>,
    /// `BlockLatencyHistogramInfo`.
    /// (since 8.1)
    #[qapi(added = "8.1.0")]
    pub zone_append_latency_histogram: Option<BlockLatencyHistogramInfo>,
    /// Number of zone append requests that have been
    /// merged into another request (since 8.1)
    #[qapi(added = "8.1.0")]
    pub zone_append_merged: Option<i64>,
    /// The number of zone append operations
    /// performed by the zoned devices (since 8.1)
    #[qapi(added = "8.1.0")]
    pub zone_append_operations: Option<i64>,
    /// Total time spent on zone append writes
    /// in nanoseconds (since 8.1)
    #[qapi(added = "8.1.0")]
    pub zone_append_total_time_ns: Option<i64>,
}

/// Statistics of a block device during a given interval of time.
///
#[qapi(added = "2.5.0")]
pub struct BlockDeviceTimedStats {
    /// Average latency of flush operations in the
    /// defined interval, in nanoseconds.
    pub avg_flush_latency_ns: i64,
    /// Average latency of read operations in the
    /// defined interval, in nanoseconds.
    pub avg_rd_latency_ns: i64,
    /// Average number of pending read operations in
    /// the defined interval.
    pub avg_rd_queue_depth: f64,
    /// Average latency of write operations in the
    /// defined interval, in nanoseconds.
    pub avg_wr_latency_ns: i64,
    /// Average number of pending write operations in
    /// the defined interval.
    pub avg_wr_queue_depth: f64,
    /// Average latency of zone append
    /// operations in the defined interval, in nanoseconds (since 8.1)
    #[qapi(added = "8.1.0")]
    pub avg_zone_append_latency_ns: Option<i64>,
    /// Average number of pending zone append
    /// operations in the defined interval (since 8.1).
    #[qapi(added = "8.1.0")]
    pub avg_zone_append_queue_depth: Option<f64>,
    /// Interval used for calculating the statistics, in
    /// seconds.
    pub interval_length: i64,
    /// Maximum latency of flush operations in the
    /// defined interval, in nanoseconds.
    pub max_flush_latency_ns: i64,
    /// Maximum latency of read operations in the
    /// defined interval, in nanoseconds.
    pub max_rd_latency_ns: i64,
    /// Maximum latency of write operations in the
    /// defined interval, in nanoseconds.
    pub max_wr_latency_ns: i64,
    /// Maximum latency of zone append
    /// operations in the defined interval, in nanoseconds (since 8.1)
    #[qapi(added = "8.1.0")]
    pub max_zone_append_latency_ns: Option<i64>,
    /// Minimum latency of flush operations in the
    /// defined interval, in nanoseconds.
    pub min_flush_latency_ns: i64,
    /// Minimum latency of read operations in the
    /// defined interval, in nanoseconds.
    pub min_rd_latency_ns: i64,
    /// Minimum latency of write operations in the
    /// defined interval, in nanoseconds.
    pub min_wr_latency_ns: i64,
    /// Minimum latency of zone append
    /// operations in the defined interval, in nanoseconds (since 8.1)
    #[qapi(added = "8.1.0")]
    pub min_zone_append_latency_ns: Option<i64>,
}

/// An enumeration of possible behaviors for errors on I/O operations.
/// The exact meaning depends on whether the I/O was initiated by a
/// guest or by a block job
///
#[qapi(added = "1.3.0")]
pub enum BlockdevOnError {
    /// inherit the error handling policy of the backend (since: 2.7)
    #[qapi(name = "auto")]
    #[qapi(added = "2.7.0")]
    Auto,
    /// same as @stop on ENOSPC, same as @report otherwise.
    #[qapi(name = "enospc")]
    Enospc,
    /// ignore the error, only report a QMP event (`BLOCK_IO_ERROR`
    /// or `BLOCK_JOB_ERROR`).  The backup, mirror and commit block jobs
    /// retry the failing request later and may still complete
    /// successfully.  The stream block job continues to stream and will
    /// complete with an error.
    #[qapi(name = "ignore")]
    Ignore,
    /// for guest operations, report the error to the guest; for
    /// jobs, cancel the job
    #[qapi(name = "report")]
    Report,
    /// for guest operations, stop the virtual machine; for jobs,
    /// pause the job
    #[qapi(name = "stop")]
    Stop,
}

/// Options for creating a block device.  Many options are available for
/// all block devices, independent of the block driver:
///
#[qapi(added = "1.7.0")]
pub struct BlockdevOptions {
    /// whether the block node should be activated (default: true).
    /// Having inactive block nodes is useful primarily for migration
    /// because it allows opening an image on the destination while the
    /// source is still holding locks for it.  (Since 10.0)
    #[qapi(added = "10.0.0")]
    pub active: Option<bool>,
    /// optional AIO backend (default: threads)
    #[qapi(removed = "2.8.0")]
    pub aio: Option<BlockdevAioOptions>,
    /// if true and @read-only is false, QEMU may
    /// automatically decide not to open the image read-write as
    /// requested, but fall back to read-only instead (and switch
    /// between the modes later), e.g. depending on whether the image
    /// file is writable or whether a writing user is attached to the
    /// node (default: false, since 3.1)
    #[qapi(name = "auto-read-only")]
    #[qapi(added = "3.1.0")]
    pub auto_read_only: Option<bool>,
    /// cache-related options
    pub cache: Option<BlockdevCacheOptions>,
    /// detect and optimize zero writes (Since 2.1)
    /// (default: off)
    #[qapi(name = "detect-zeroes")]
    #[qapi(added = "2.1.0")]
    pub detect_zeroes: Option<BlockdevDetectZeroesOptions>,
    /// discard-related options (default: ignore)
    pub discard: Option<BlockdevDiscardOptions>,
    /// block driver name
    #[qapi(discriminator)]
    pub driver: BlockdevDriver,
    /// force share all permission on added nodes.  Requires
    /// read-only=true.  (Since 2.10)
    #[qapi(name = "force-share")]
    #[qapi(added = "2.10.0")]
    pub force_share: Option<bool>,
    /// optional id by which the new block device can be referred to.
    /// This option is only allowed on the top level of blockdev-add.
    /// A BlockBackend will be created by blockdev-add if and only if
    /// this option is given.
    #[qapi(removed = "2.8.0")]
    pub id: Option<String>,
    /// the node name of the new node.  This option is required
    /// on the top level of `blockdev-add`.  Valid node names start with
    /// an alphabetic character and may contain only alphanumeric
    /// characters, '-', '.' and '_'.  Their maximum length is 31
    /// characters.  (Since 2.0)
    #[qapi(name = "node-name")]
    #[qapi(added = "2.0.0")]
    pub node_name: Option<String>,
    /// whether the block device should be read-only (default:
    /// false).  Note that some block drivers support only read-only
    /// access, either generally or in certain configurations.  In this
    /// case, the default value does not work and the option must be
    /// specified explicitly.
    #[qapi(name = "read-only")]
    pub read_only: Option<bool>,
    /// optional how to handle read errors on the device
    /// (default: report)
    #[qapi(removed = "2.7.0")]
    pub rerror: Option<BlockdevOnError>,
    /// optional whether to include failed
    /// operations when computing latency and last
    /// access statistics (default: true) (Since 2.5)
    #[qapi(name = "stats-account-failed")]
    #[qapi(added = "2.5.0")]
    #[qapi(removed = "2.7.0")]
    pub stats_account_failed: Option<bool>,
    /// optional whether to include invalid
    /// operations when computing last access statistics
    /// (default: true) (Since 2.5)
    #[qapi(name = "stats-account-invalid")]
    #[qapi(added = "2.5.0")]
    #[qapi(removed = "2.7.0")]
    pub stats_account_invalid: Option<bool>,
    /// optional list of intervals for collecting I/O
    /// statistics, in seconds (default: none) (Since 2.5)
    #[qapi(name = "stats-intervals")]
    #[qapi(added = "2.5.0")]
    #[qapi(removed = "2.7.0")]
    pub stats_intervals: Option<Vec<i64>>,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<Box<BlockdevOptionsBranch>>,
    /// optional how to handle write errors on the device
    /// (default: enospc)
    #[qapi(removed = "2.7.0")]
    pub werror: Option<BlockdevOnError>,
}

/// Driver specific block device options for Archipelago.
///
#[qapi(added = "2.2.0")]
#[qapi(removed = "2.9.0")]
pub struct BlockdevOptionsArchipelago {
    /// optional The port number on which mapperd is
    /// listening. This is optional
    /// and if not specified, QEMU will make Archipelago
    /// use the default port (1001).
    pub mport: Option<i64>,
    /// optional The name of the shared memory segment
    /// Archipelago stack is using. This is optional
    /// and if not specified, QEMU will make Archipelago
    /// use the default value, 'archipelago'.
    pub segment: Option<String>,
    /// Name of the Archipelago volume image
    pub volume: String,
    /// optional The port number on which vlmcd is
    /// listening. This is optional
    /// and if not specified, QEMU will make Archipelago
    /// use the default port (501).
    pub vport: Option<i64>,
}

/// Options that are available for all block devices, independent of the block
/// driver.
///
#[qapi(added = "1.7.0")]
#[qapi(removed = "2.6.0")]
pub struct BlockdevOptionsBase {
    /// optional AIO backend (default: threads)
    pub aio: Option<BlockdevAioOptions>,
    /// optional cache-related options
    pub cache: Option<BlockdevCacheOptions>,
    /// optional detect and optimize zero writes (Since 2.1)
    /// (default: off)
    #[qapi(name = "detect-zeroes")]
    #[qapi(added = "2.1.0")]
    pub detect_zeroes: Option<BlockdevDetectZeroesOptions>,
    /// optional discard-related options (default: ignore)
    pub discard: Option<BlockdevDiscardOptions>,
    /// block driver name
    pub driver: BlockdevDriver,
    /// optional id by which the new block device can be referred to.
    /// This option is only allowed on the top level of blockdev-add.
    /// A BlockBackend will be created by blockdev-add if and only if
    /// this option is given.
    pub id: Option<String>,
    /// optional the name of a block driver state node (Since 2.0).
    /// This option is required on the top level of blockdev-add if
    /// the @id option is not given there.
    #[qapi(name = "node-name")]
    #[qapi(added = "2.0.0")]
    pub node_name: Option<String>,
    /// optional whether the block device should be read-only
    /// (default: false)
    #[qapi(name = "read-only")]
    pub read_only: Option<bool>,
    /// optional how to handle read errors on the device
    /// (default: report)
    pub rerror: Option<BlockdevOnError>,
    /// optional whether to include failed
    /// operations when computing latency and last
    /// access statistics (default: true) (Since 2.5)
    #[qapi(name = "stats-account-failed")]
    #[qapi(added = "2.5.0")]
    pub stats_account_failed: Option<bool>,
    /// optional whether to include invalid
    /// operations when computing last access statistics
    /// (default: true) (Since 2.5)
    #[qapi(name = "stats-account-invalid")]
    #[qapi(added = "2.5.0")]
    pub stats_account_invalid: Option<bool>,
    /// optional list of intervals for collecting I/O
    /// statistics, in seconds (default: none) (Since 2.5)
    #[qapi(name = "stats-intervals")]
    #[qapi(added = "2.5.0")]
    pub stats_intervals: Option<Vec<i64>>,
    /// optional how to handle write errors on the device
    /// (default: enospc)
    pub werror: Option<BlockdevOnError>,
}

/// Driver specific block device options for blkdebug.
///
#[qapi(added = "2.0.0")]
pub struct BlockdevOptionsBlkdebug {
    /// required alignment for requests in bytes, must be positive
    /// power of 2, or 0 for default
    pub align: Option<i64>,
    /// filename of the configuration file
    pub config: Option<String>,
    /// underlying raw block device (or image file)
    pub image: Box<BlockdevRef>,
    /// array of error injection descriptions
    #[qapi(name = "inject-error")]
    pub inject_error: Option<Vec<BlkdebugInjectErrorOptions>>,
    /// maximum size for discard requests in bytes, must be
    /// positive multiple of @align, of @opt-discard, and of the
    /// underlying file's request alignment (but need not be a power of
    /// 2), or 0 for default (since 2.10)
    #[qapi(name = "max-discard")]
    #[qapi(added = "2.9.1")]
    pub max_discard: Option<i32>,
    /// maximum size for I/O transfers in bytes, must be
    /// positive multiple of @align and of the underlying file's request
    /// alignment (but need not be a power of 2), or 0 for default
    /// (since 2.10)
    #[qapi(name = "max-transfer")]
    #[qapi(added = "2.9.1")]
    pub max_transfer: Option<i32>,
    /// maximum size for write zero requests in bytes, must
    /// be positive multiple of @align, of @opt-write-zero, and of the
    /// underlying file's request alignment (but need not be a power of
    /// 2), or 0 for default (since 2.10)
    #[qapi(name = "max-write-zero")]
    #[qapi(added = "2.9.1")]
    pub max_write_zero: Option<i32>,
    /// preferred alignment for discard requests in bytes,
    /// must be positive multiple of @align and of the underlying file's
    /// request alignment (but need not be a power of 2), or 0 for
    /// default (since 2.10)
    #[qapi(name = "opt-discard")]
    #[qapi(added = "2.9.1")]
    pub opt_discard: Option<i32>,
    /// preferred alignment for write zero requests in
    /// bytes, must be positive multiple of @align and of the underlying
    /// file's request alignment (but need not be a power of 2), or 0
    /// for default (since 2.10)
    #[qapi(name = "opt-write-zero")]
    #[qapi(added = "2.9.1")]
    pub opt_write_zero: Option<i32>,
    /// array of state-change descriptions
    #[qapi(name = "set-state")]
    pub set_state: Option<Vec<BlkdebugSetStateOptions>>,
    /// Permissions to take on @image in addition to what
    /// is necessary anyway (which depends on how the blkdebug node is
    /// used).  Defaults to none.  (since 5.0)
    #[qapi(name = "take-child-perms")]
    #[qapi(added = "4.2.1")]
    pub take_child_perms: Option<Vec<BlockPermission>>,
    /// Permissions not to share on @image in addition
    /// to what cannot be shared anyway (which depends on how the
    /// blkdebug node is used).  Defaults to none.  (since 5.0)
    #[qapi(name = "unshare-child-perms")]
    #[qapi(added = "4.2.1")]
    pub unshare_child_perms: Option<Vec<BlockPermission>>,
}

/// Driver specific block device options for blklogwrites.
///
#[qapi(added = "3.0.0")]
pub struct BlockdevOptionsBlklogwrites {
    /// block device
    pub file: Box<BlockdevRef>,
    /// block device used to log writes to @file
    pub log: Box<BlockdevRef>,
    /// append to an existing log (default: false)
    #[qapi(name = "log-append")]
    pub log_append: Option<bool>,
    /// sector size used in logging writes to @file,
    /// determines granularity of offsets and sizes of writes
    /// (default: 512)
    #[qapi(name = "log-sector-size")]
    pub log_sector_size: Option<u32>,
    /// interval of write requests after which
    /// the log super block is updated to disk (default: 4096)
    #[qapi(name = "log-super-update-interval")]
    pub log_super_update_interval: Option<u64>,
}

/// Driver specific block device options for blkreplay.
///
#[qapi(added = "4.2.0")]
pub struct BlockdevOptionsBlkreplay {
    /// disk image which should be controlled with blkreplay
    pub image: Box<BlockdevRef>,
}

/// Driver specific block device options for blkverify.
///
#[qapi(added = "2.0.0")]
pub struct BlockdevOptionsBlkverify {
    /// raw image used for verification
    pub raw: Box<BlockdevRef>,
    /// block device to be tested
    pub test: Box<BlockdevRef>,
}

#[qapi(added = "1.7.0")]
pub enum BlockdevOptionsBranch {
    #[qapi(name = "archipelago")]
    #[qapi(added = "2.2.0")]
    #[qapi(removed = "2.9.0")]
    Archipelago(BlockdevOptionsArchipelago),
    #[qapi(name = "blkdebug")]
    #[qapi(added = "2.0.0")]
    Blkdebug(BlockdevOptionsBlkdebug),
    #[qapi(name = "blklogwrites")]
    #[qapi(added = "3.0.0")]
    Blklogwrites(BlockdevOptionsBlklogwrites),
    #[qapi(name = "blkreplay")]
    #[qapi(added = "4.2.0")]
    Blkreplay(BlockdevOptionsBlkreplay),
    #[qapi(name = "blkverify")]
    #[qapi(added = "2.0.0")]
    Blkverify(BlockdevOptionsBlkverify),
    #[qapi(name = "bochs")]
    Bochs(BlockdevOptionsGenericFormat),
    #[qapi(name = "cloop")]
    Cloop(BlockdevOptionsGenericFormat),
    #[qapi(name = "compress")]
    #[qapi(added = "5.0.0")]
    Compress(BlockdevOptionsGenericFormat),
    #[qapi(name = "copy-before-write")]
    #[qapi(added = "6.2.0")]
    CopyBeforeWrite(BlockdevOptionsCbw),
    #[qapi(name = "copy-on-read")]
    #[qapi(added = "3.0.0")]
    CopyOnRead(BlockdevOptionsCor),
    #[qapi(name = "cow")]
    #[qapi(removed = "2.2.0")]
    Cow(BlockdevOptionsGenericCowFormat),
    #[qapi(name = "dmg")]
    Dmg(BlockdevOptionsGenericFormat),
    #[qapi(name = "file")]
    File(BlockdevOptionsFile),
    #[qapi(name = "ftp")]
    Ftp(BlockdevOptionsCurlFtp),
    #[qapi(name = "ftps")]
    Ftps(BlockdevOptionsCurlFtps),
    #[qapi(name = "gluster")]
    #[qapi(added = "2.7.0")]
    #[qapi(removed = "11.1.0")]
    Gluster(BlockdevOptionsGluster),
    #[qapi(name = "host_cdrom")]
    #[qapi(condition = "defined(HAVE_HOST_BLOCK_DEVICE)")]
    #[qapi(added = "2.1.0")]
    HostCdrom(BlockdevOptionsFile),
    #[qapi(name = "host_device")]
    #[qapi(condition = "defined(HAVE_HOST_BLOCK_DEVICE)")]
    #[qapi(added = "2.1.0")]
    HostDevice(BlockdevOptionsFile),
    #[qapi(name = "host_floppy")]
    #[qapi(added = "2.1.0")]
    #[qapi(removed = "2.5.0")]
    HostFloppy(BlockdevOptionsFile),
    #[qapi(name = "http")]
    Http(BlockdevOptionsCurlHttp),
    #[qapi(name = "https")]
    Https(BlockdevOptionsCurlHttps),
    #[qapi(name = "io_uring")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    IoUring(BlockdevOptionsIoUring),
    #[qapi(name = "iscsi")]
    #[qapi(added = "2.9.0")]
    Iscsi(BlockdevOptionsIscsi),
    #[qapi(name = "luks")]
    #[qapi(added = "2.6.0")]
    Luks(BlockdevOptionsLuks),
    #[qapi(name = "nbd")]
    #[qapi(added = "2.8.0")]
    Nbd(BlockdevOptionsNbd),
    #[qapi(name = "nfs")]
    #[qapi(added = "2.8.0")]
    Nfs(BlockdevOptionsNfs),
    #[qapi(name = "null-aio")]
    #[qapi(added = "2.2.0")]
    NullAio(BlockdevOptionsNull),
    #[qapi(name = "null-co")]
    #[qapi(added = "2.2.0")]
    NullCo(BlockdevOptionsNull),
    #[qapi(name = "nvme")]
    #[qapi(added = "2.12.0")]
    Nvme(BlockdevOptionsNvMe),
    #[qapi(name = "nvme-io_uring")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    NvmeIoUring(BlockdevOptionsNvmeIoUring),
    #[qapi(name = "parallels")]
    Parallels(BlockdevOptionsGenericFormat),
    #[qapi(name = "preallocate")]
    #[qapi(added = "6.0.0")]
    Preallocate(BlockdevOptionsPreallocate),
    #[qapi(name = "qcow")]
    Qcow(BlockdevOptionsQcow),
    #[qapi(name = "qcow2")]
    Qcow2(BlockdevOptionsQcow2),
    #[qapi(name = "qed")]
    Qed(BlockdevOptionsGenericCowFormat),
    #[qapi(name = "quorum")]
    #[qapi(added = "2.0.0")]
    Quorum(BlockdevOptionsQuorum),
    #[qapi(name = "raw")]
    Raw(BlockdevOptionsRaw),
    #[qapi(name = "rbd")]
    #[qapi(added = "2.9.0")]
    Rbd(BlockdevOptionsRbd),
    #[qapi(name = "replication")]
    #[qapi(condition = "defined(CONFIG_REPLICATION)")]
    #[qapi(added = "2.8.0")]
    Replication(BlockdevOptionsReplication),
    #[qapi(name = "sheepdog")]
    #[qapi(added = "2.9.0")]
    #[qapi(removed = "6.1.0")]
    Sheepdog(BlockdevOptionsSheepdog),
    #[qapi(name = "snapshot-access")]
    #[qapi(added = "7.0.0")]
    SnapshotAccess(BlockdevOptionsGenericFormat),
    #[qapi(name = "ssh")]
    #[qapi(added = "2.8.0")]
    Ssh(BlockdevOptionsSsh),
    #[qapi(name = "tftp")]
    #[qapi(removed = "2.8.0")]
    Tftp(BlockdevOptionsFile),
    #[qapi(name = "throttle")]
    #[qapi(added = "2.11.0")]
    Throttle(BlockdevOptionsThrottle),
    #[qapi(name = "vdi")]
    Vdi(BlockdevOptionsGenericFormat),
    #[qapi(name = "vhdx")]
    Vhdx(BlockdevOptionsGenericFormat),
    #[qapi(name = "virtio-blk-vfio-pci")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    VirtioBlkVfioPci(BlockdevOptionsVirtioBlkVfioPci),
    #[qapi(name = "virtio-blk-vhost-user")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    VirtioBlkVhostUser(BlockdevOptionsVirtioBlkVhostUser),
    #[qapi(name = "virtio-blk-vhost-vdpa")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    VirtioBlkVhostVdpa(BlockdevOptionsVirtioBlkVhostVdpa),
    #[qapi(name = "vmdk")]
    Vmdk(BlockdevOptionsGenericCowFormat),
    #[qapi(name = "vpc")]
    Vpc(BlockdevOptionsGenericFormat),
    #[qapi(name = "vvfat")]
    Vvfat(BlockdevOptionsVvfat),
    #[qapi(name = "vxhs")]
    #[qapi(added = "2.10.0")]
    #[qapi(removed = "5.1.0")]
    Vxhs(BlockdevOptionsVxHs),
}

/// Driver specific block device options for the copy-before-write
/// driver, which does so called copy-before-write operations: when data
/// is written to the filter, the filter first reads corresponding
/// blocks from its file child and copies them to @target child.  After
/// successfully copying, the write request is propagated to file child.
/// If copying fails, the original write request is failed too and no
/// data is written to file child.
///
#[qapi(added = "6.2.0")]
pub struct BlockdevOptionsCbw {
    /// If specified, copy-before-write filter will do
    /// copy-before-write operations only for dirty regions of the
    /// bitmap.  Bitmap size must be equal to length of file and target
    /// child of the filter.  Note also, that bitmap is used only to
    /// initialize internal bitmap of the process, so further
    /// modifications (or removing) of specified bitmap doesn't
    /// influence the filter.  (Since 7.0)
    #[qapi(added = "7.0.0")]
    pub bitmap: Option<BlockDirtyBitmap>,
    /// Zero means no limit.  Non-zero sets the timeout in
    /// seconds for copy-before-write operation.  When a timeout occurs,
    /// the respective copy-before-write operation will fail, and the
    #[qapi(name = "cbw-timeout")]
    #[qapi(added = "7.1.0")]
    pub cbw_timeout: Option<u32>,
    /// reference to or definition of the data source block device
    pub file: Box<BlockdevRef>,
    /// Minimum size of blocks used by copy-before-write
    /// operations.  Has to be a power of 2.  No effect if smaller than
    /// the maximum of the target's cluster size and 64 KiB.  Default 0.
    /// (Since 9.2)
    #[qapi(name = "min-cluster-size")]
    #[qapi(added = "9.2.0")]
    pub min_cluster_size: Option<u64>,
    /// parameter will decide how this failure is handled.
    /// Default 0.  (Since 7.1)
    #[qapi(name = "on-cbw-error")]
    #[qapi(added = "7.1.0")]
    pub on_cbw_error: Option<OnCbwError>,
    /// The target for copy-before-write operations.
    pub target: Box<BlockdevRef>,
}

/// Driver specific block device options for the copy-on-read driver.
///
#[qapi(added = "6.0.0")]
pub struct BlockdevOptionsCor {
    /// The name of a non-filter node (allocation-bearing layer)
    /// that limits the COR operations in the backing chain (inclusive),
    /// so that no data below this node will be copied by this filter.
    /// If option is absent, the limit is not applied, so that data from
    /// all backing layers may be copied.
    pub bottom: Option<String>,
    /// reference to or definition of the data source block device
    pub file: Box<BlockdevRef>,
}

/// Driver specific block device options for the curl backend.
///
#[qapi(added = "2.8.0")]
#[qapi(removed = "2.9.0")]
pub struct BlockdevOptionsCurl {
    /// path to the image file
    pub filename: String,
}

/// Driver specific block device options shared by all protocols
/// supported by the curl backend.
///
#[qapi(added = "2.9.0")]
pub struct BlockdevOptionsCurlBase {
    /// ID of a QCryptoSecret object providing a password
    /// for authentication (defaults to no password)
    #[qapi(name = "password-secret")]
    pub password_secret: Option<String>,
    /// ID of a QCryptoSecret object providing a
    /// password for proxy authentication (defaults to no password)
    #[qapi(name = "proxy-password-secret")]
    pub proxy_password_secret: Option<String>,
    /// Username for proxy authentication (defaults to
    /// none)
    #[qapi(name = "proxy-username")]
    pub proxy_username: Option<String>,
    /// Size of the read-ahead cache; must be a multiple of 512
    /// (defaults to 256 kB)
    pub readahead: Option<i64>,
    /// Timeout for connections, in seconds (defaults to 5)
    pub timeout: Option<i64>,
    /// URL of the image file
    pub url: String,
    /// Username for authentication (defaults to none)
    pub username: Option<String>,
}

/// Driver specific block device options for FTP connections over the
/// curl backend.  URLs must start with "ftp://".
///
#[qapi(added = "2.9.0")]
pub struct BlockdevOptionsCurlFtp {
    /// ID of a QCryptoSecret object providing a password
    /// for authentication (defaults to no password)
    #[qapi(name = "password-secret")]
    pub password_secret: Option<String>,
    /// ID of a QCryptoSecret object providing a
    /// password for proxy authentication (defaults to no password)
    #[qapi(name = "proxy-password-secret")]
    pub proxy_password_secret: Option<String>,
    /// Username for proxy authentication (defaults to
    /// none)
    #[qapi(name = "proxy-username")]
    pub proxy_username: Option<String>,
    /// Size of the read-ahead cache; must be a multiple of 512
    /// (defaults to 256 kB)
    pub readahead: Option<i64>,
    /// Timeout for connections, in seconds (defaults to 5)
    pub timeout: Option<i64>,
    /// URL of the image file
    pub url: String,
    /// Username for authentication (defaults to none)
    pub username: Option<String>,
}

/// Driver specific block device options for FTPS connections over the
/// curl backend.  URLs must start with "ftps://".
///
#[qapi(added = "2.9.0")]
pub struct BlockdevOptionsCurlFtps {
    /// ID of a QCryptoSecret object providing a password
    /// for authentication (defaults to no password)
    #[qapi(name = "password-secret")]
    pub password_secret: Option<String>,
    /// ID of a QCryptoSecret object providing a
    /// password for proxy authentication (defaults to no password)
    #[qapi(name = "proxy-password-secret")]
    pub proxy_password_secret: Option<String>,
    /// Username for proxy authentication (defaults to
    /// none)
    #[qapi(name = "proxy-username")]
    pub proxy_username: Option<String>,
    /// Size of the read-ahead cache; must be a multiple of 512
    /// (defaults to 256 kB)
    pub readahead: Option<i64>,
    /// Whether to verify the SSL certificate's validity
    /// (defaults to true)
    pub sslverify: Option<bool>,
    /// Timeout for connections, in seconds (defaults to 5)
    pub timeout: Option<i64>,
    /// URL of the image file
    pub url: String,
    /// Username for authentication (defaults to none)
    pub username: Option<String>,
}

/// Driver specific block device options for HTTP connections over the
/// curl backend.  URLs must start with "http://".
///
#[qapi(added = "2.9.0")]
pub struct BlockdevOptionsCurlHttp {
    /// List of cookies to set; format is "name1=content1;
    /// name2=content2;" as explained by CURLOPT_COOKIE(3).  Defaults to
    /// no cookies.
    pub cookie: Option<String>,
    /// ID of a QCryptoSecret object providing the cookie
    /// data in a secure way.  See @cookie for the format.  (since 2.10)
    #[qapi(name = "cookie-secret")]
    #[qapi(added = "2.10.0")]
    pub cookie_secret: Option<String>,
    /// Don't issue a HEAD HTTP request to discover if the
    /// http server supports range requests and rely only on GET
    /// requests.  This is especially useful for S3 presigned URLs where
    /// HEAD requests are unauthorized.  (default: false; since 11.0)
    #[qapi(name = "force-range")]
    #[qapi(added = "11.0.0")]
    pub force_range: Option<bool>,
    /// ID of a QCryptoSecret object providing a password
    /// for authentication (defaults to no password)
    #[qapi(name = "password-secret")]
    pub password_secret: Option<String>,
    /// ID of a QCryptoSecret object providing a
    /// password for proxy authentication (defaults to no password)
    #[qapi(name = "proxy-password-secret")]
    pub proxy_password_secret: Option<String>,
    /// Username for proxy authentication (defaults to
    /// none)
    #[qapi(name = "proxy-username")]
    pub proxy_username: Option<String>,
    /// Size of the read-ahead cache; must be a multiple of 512
    /// (defaults to 256 kB)
    pub readahead: Option<i64>,
    /// Timeout for connections, in seconds (defaults to 5)
    pub timeout: Option<i64>,
    /// URL of the image file
    pub url: String,
    /// Username for authentication (defaults to none)
    pub username: Option<String>,
}

/// Driver specific block device options for HTTPS connections over the
/// curl backend.  URLs must start with "https://".
///
#[qapi(added = "2.9.0")]
pub struct BlockdevOptionsCurlHttps {
    /// List of cookies to set; format is "name1=content1;
    /// name2=content2;" as explained by CURLOPT_COOKIE(3).  Defaults to
    /// no cookies.
    pub cookie: Option<String>,
    /// ID of a QCryptoSecret object providing the cookie
    /// data in a secure way.  See @cookie for the format.  (since 2.10)
    #[qapi(name = "cookie-secret")]
    #[qapi(added = "2.10.0")]
    pub cookie_secret: Option<String>,
    /// Don't issue a HEAD HTTP request to discover if the
    /// http server supports range requests and rely only on GET
    /// requests.  This is especially useful for S3 presigned URLs where
    /// HEAD requests are unauthorized.  (default: false; since 11.0)
    #[qapi(name = "force-range")]
    #[qapi(added = "11.0.0")]
    pub force_range: Option<bool>,
    /// ID of a QCryptoSecret object providing a password
    /// for authentication (defaults to no password)
    #[qapi(name = "password-secret")]
    pub password_secret: Option<String>,
    /// ID of a QCryptoSecret object providing a
    /// password for proxy authentication (defaults to no password)
    #[qapi(name = "proxy-password-secret")]
    pub proxy_password_secret: Option<String>,
    /// Username for proxy authentication (defaults to
    /// none)
    #[qapi(name = "proxy-username")]
    pub proxy_username: Option<String>,
    /// Size of the read-ahead cache; must be a multiple of 512
    /// (defaults to 256 kB)
    pub readahead: Option<i64>,
    /// Whether to verify the SSL certificate's validity
    /// (defaults to true)
    pub sslverify: Option<bool>,
    /// Timeout for connections, in seconds (defaults to 5)
    pub timeout: Option<i64>,
    /// URL of the image file
    pub url: String,
    /// Username for authentication (defaults to none)
    pub username: Option<String>,
}

/// Driver specific block device options for the file backend.
///
#[qapi(feature = "dynamic-auto-read-only", if = "defined(CONFIG_POSIX)")]
#[qapi(feature = "dynamic-auto-read-only", if = "CONFIG_POSIX")]
#[qapi(added = "1.7.0")]
pub struct BlockdevOptionsFile {
    /// AIO backend (default: threads) (since: 2.8)
    #[qapi(added = "2.8.0")]
    pub aio: Option<BlockdevAioOptions>,
    #[qapi(name = "aio-max-batch")]
    #[qapi(added = "6.2.0")]
    pub aio_max_batch: Option<i64>,
    /// invalidate page cache during live migration.  This prevents
    /// stale data on the migration destination with cache.direct=off.
    /// Currently only supported on Linux hosts.
    /// (default: on, since: 4.0)
    #[qapi(name = "drop-cache")]
    #[qapi(condition = "defined(CONFIG_LINUX)")]
    #[qapi(added = "4.0.0")]
    pub drop_cache: Option<bool>,
    /// path to the image file
    pub filename: String,
    /// whether to enable file locking. If set to 'auto', only enable
    /// when Open File Descriptor (OFD) locking API is available
    /// (default: auto, since 2.10)
    #[qapi(added = "2.10.0")]
    pub locking: Option<OnOffAuto>,
    /// the id for the object that will handle persistent reservations
    /// for this device (default: none, forward the commands via SG_IO;
    #[qapi(name = "pr-manager")]
    #[qapi(added = "2.11.0")]
    pub pr_manager: Option<String>,
    /// whether to check that page cache was dropped on live
    /// migration.  May cause noticeable delays if the image
    /// file is large, do not use in production.
    /// (default: off) (since: 3.0)
    #[qapi(name = "x-check-cache-dropped")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "3.0.0")]
    pub x_check_cache_dropped: Option<bool>,
}

/// Driver specific block device options for image format that have no
/// option besides their data source and an optional backing file.
///
#[qapi(name = "BlockdevOptionsGenericCOWFormat")]
#[qapi(added = "1.7.0")]
pub struct BlockdevOptionsGenericCowFormat {
    /// reference to or definition of the backing file block
    /// device, null disables the backing file entirely.  Defaults to
    /// the backing file stored the image file.
    pub backing: Option<BlockdevRefOrNull>,
    /// reference to or definition of the data source block device
    pub file: BlockdevRef,
}

/// Driver specific block device options for image format that have no
/// option besides their data source.
///
#[qapi(added = "1.7.0")]
pub struct BlockdevOptionsGenericFormat {
    /// reference to or definition of the data source block device
    pub file: Box<BlockdevRef>,
}

/// Driver specific block device options for Gluster
///
#[qapi(added = "2.7.0")]
#[qapi(removed = "11.1.0")]
pub struct BlockdevOptionsGluster {
    /// libgfapi log level (default '4' which is Error) (Since 2.8)
    #[qapi(added = "2.8.0")]
    pub debug: Option<i64>,
    /// optional libgfapi log level (default '4' which is Error)
    #[qapi(name = "debug-level")]
    #[qapi(removed = "2.8.0")]
    pub debug_level: Option<i64>,
    /// libgfapi log file (default /dev/stderr) (Since 2.8)
    #[qapi(added = "2.8.0")]
    pub logfile: Option<String>,
    /// absolute path to image file in gluster volume
    pub path: String,
    /// gluster servers description
    pub server: Vec<SocketAddress>,
    /// name of gluster volume where VM image resides
    pub volume: String,
}

/// Driver specific block device options for the io_uring backend.
///
#[qapi(condition = "CONFIG_BLKIO")]
#[qapi(added = "7.2.0")]
pub struct BlockdevOptionsIoUring {
    /// path to the image file
    pub filename: String,
}

/// Driver specific block device options for iscsi
///
#[qapi(added = "2.9.0")]
pub struct BlockdevOptionsIscsi {
    /// The desired header digest.  Defaults to none-crc32c.
    #[qapi(name = "header-digest")]
    pub header_digest: Option<IscsiHeaderDigest>,
    /// The iqn name we want to identify to the target as.
    /// If this option is not specified, an initiator name is generated
    /// automatically.
    #[qapi(name = "initiator-name")]
    pub initiator_name: Option<String>,
    /// LUN to connect to.  Defaults to 0.
    pub lun: Option<i64>,
    /// The ID of a QCryptoSecret object providing the
    /// password for the login.  This option is required if @user is
    /// specified.
    #[qapi(name = "password-secret")]
    pub password_secret: Option<String>,
    /// The address of the iscsi portal
    pub portal: String,
    /// The target iqn name
    pub target: String,
    /// Timeout in seconds after which a request will timeout.  0
    /// means no timeout and is the default.
    pub timeout: Option<i64>,
    /// The iscsi transport type
    pub transport: IscsiTransport,
    /// User name to log in with.  If omitted, no CHAP authentication
    /// is performed.
    pub user: Option<String>,
}

/// Driver specific block device options for LUKS.
///
#[qapi(name = "BlockdevOptionsLUKS")]
#[qapi(added = "2.6.0")]
pub struct BlockdevOptionsLuks {
    /// reference to or definition of the data source block device
    pub file: BlockdevRef,
    /// block device holding a detached LUKS header.  (since 9.0)
    #[qapi(added = "9.0.0")]
    pub header: Option<BlockdevRef>,
    /// the ID of a QCryptoSecret object providing the
    /// decryption key.  Mandatory except when doing a metadata-only
    /// probe of the image.  (since 2.6)
    #[qapi(name = "key-secret")]
    pub key_secret: Option<String>,
}

/// Driver specific block device options for NBD.
///
#[qapi(added = "2.8.0")]
pub struct BlockdevOptionsNbd {
    /// export name
    pub export: Option<String>,
    #[qapi(name = "open-timeout")]
    #[qapi(added = "7.0.0")]
    pub open_timeout: Option<u32>,
    /// On an unexpected disconnect, the nbd client tries to
    /// connect again until succeeding or encountering a serious
    /// error.  During the first @reconnect-delay seconds, all
    /// requests are paused and will be rerun on a successful
    /// reconnect. After that time, any delayed requests and all
    /// future requests before a successful reconnect will
    /// immediately fail. Default 0 (Since 4.2)
    #[qapi(name = "reconnect-delay")]
    #[qapi(added = "4.2.0")]
    pub reconnect_delay: Option<u32>,
    /// NBD server address
    pub server: SocketAddress,
    /// TLS credentials ID
    #[qapi(name = "tls-creds")]
    pub tls_creds: Option<String>,
    #[qapi(name = "tls-hostname")]
    #[qapi(added = "7.0.0")]
    pub tls_hostname: Option<String>,
    /// A metadata context name such as "qemu:dirty-bitmap:NAME"
    /// or "qemu:allocation-depth" to query in place of the
    /// traditional "base:allocation" block status (see
    /// NBD_OPT_LIST_META_CONTEXT in the NBD protocol; and
    /// yes, naming this option x-context would have made
    /// more sense) (since 3.0)
    #[qapi(name = "x-dirty-bitmap")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "3.0.0")]
    pub x_dirty_bitmap: Option<String>,
}

/// Driver specific block device option for NFS
///
#[qapi(added = "2.8.0")]
pub struct BlockdevOptionsNfs {
    /// set the NFS debug level (max 2) (defaults to libnfs default)
    pub debug: Option<i64>,
    /// GID value to use when talking to the server (defaults to
    /// 65534 on Windows and getgid() in unix)
    pub group: Option<i64>,
    /// set the pagecache size in bytes (defaults to
    /// libnfs default)
    #[qapi(name = "page-cache-size")]
    pub page_cache_size: Option<i64>,
    /// path of the image on the host
    pub path: String,
    /// set the readahead size in bytes (defaults to libnfs
    /// default)
    #[qapi(name = "readahead-size")]
    pub readahead_size: Option<i64>,
    /// host address
    pub server: NfsServer,
    /// number of SYNs during the session establishment
    /// (defaults to libnfs default)
    #[qapi(name = "tcp-syn-count")]
    pub tcp_syn_count: Option<i64>,
    /// UID value to use when talking to the server (defaults to
    /// 65534 on Windows and getuid() on unix)
    pub user: Option<i64>,
}

/// Driver specific block device options for the null backend.
///
#[qapi(added = "2.2.0")]
pub struct BlockdevOptionsNull {
    /// emulated latency (in nanoseconds) in processing
    /// requests.  Default to zero which completes requests immediately.
    /// (Since 2.4)
    #[qapi(name = "latency-ns")]
    #[qapi(added = "2.4.0")]
    pub latency_ns: Option<u64>,
    /// if true, reads from the device produce zeroes; if
    /// false, the buffer is left unchanged.
    /// (default: false; since: 4.1)
    #[qapi(name = "read-zeroes")]
    #[qapi(added = "4.1.0")]
    pub read_zeroes: Option<bool>,
    /// size of the device in bytes.
    pub size: Option<i64>,
}

/// Driver specific block device options for the NVMe backend.
///
#[qapi(name = "BlockdevOptionsNVMe")]
#[qapi(added = "2.12.0")]
pub struct BlockdevOptionsNvMe {
    /// controller address of the NVMe device.
    pub device: String,
    /// namespace number of the device, starting from 1.
    pub namespace: i64,
}

/// Driver specific block device options for the nvme-io_uring backend.
///
#[qapi(condition = "CONFIG_BLKIO")]
#[qapi(added = "7.2.0")]
pub struct BlockdevOptionsNvmeIoUring {
    /// path to the NVMe namespace's character device (e.g.
    /// /dev/ng0n1).
    pub path: String,
}

/// Filter driver intended to be inserted between format and protocol
/// node and do preallocation in protocol node on write.
///
#[qapi(added = "6.0.0")]
pub struct BlockdevOptionsPreallocate {
    /// reference to or definition of the data source block device
    pub file: Box<BlockdevRef>,
    /// on preallocation, align file length to this number,
    /// default 1048576 (1M)
    #[qapi(name = "prealloc-align")]
    pub prealloc_align: Option<i64>,
    /// how much to preallocate, default 134217728 (128M)
    #[qapi(name = "prealloc-size")]
    pub prealloc_size: Option<i64>,
}

/// Driver specific block device options for qcow.
///
#[qapi(added = "2.10.0")]
pub struct BlockdevOptionsQcow {
    /// reference to or definition of the backing file block
    /// device, null disables the backing file entirely.  Defaults to
    /// the backing file stored the image file.
    pub backing: Option<Box<BlockdevRefOrNull>>,
    /// Image decryption options.  Mandatory for encrypted images,
    /// except when doing a metadata-only probe of the image.
    pub encrypt: Option<BlockdevQcowEncryption>,
    /// reference to or definition of the data source block device
    pub file: Box<BlockdevRef>,
}

/// Driver specific block device options for qcow2.
///
#[qapi(added = "1.7.0")]
pub struct BlockdevOptionsQcow2 {
    /// reference to or definition of the backing file block
    /// device, null disables the backing file entirely.  Defaults to
    /// the backing file stored the image file.
    pub backing: Option<Box<BlockdevRefOrNull>>,
    /// clean unused entries in the L2 and refcount
    /// caches.  The interval is in seconds.  The default value is 600
    /// on supporting platforms, and 0 on other platforms.  0 disables
    /// this feature.  (since 2.5)
    #[qapi(name = "cache-clean-interval")]
    #[qapi(added = "2.5.0")]
    pub cache_clean_interval: Option<i64>,
    /// the maximum total size of the L2 table and refcount
    /// block caches in bytes (since 2.2)
    #[qapi(name = "cache-size")]
    #[qapi(added = "2.2.0")]
    pub cache_size: Option<i64>,
    /// reference to or definition of the external data file.
    /// This may only be specified for images that require an external
    /// data file.  If it is not specified for such an image, the data
    /// file name is loaded from the image file.  (since 4.0)
    #[qapi(name = "data-file")]
    #[qapi(added = "4.0.0")]
    pub data_file: Option<Box<BlockdevRef>>,
    /// when enabled, data clusters will remain
    /// preallocated when they are no longer used, e.g. because they are
    /// discarded or converted to zero clusters.  As usual, whether the
    /// old data is discarded or kept on the protocol level (i.e. in the
    /// image file) depends on the setting of the pass-discard-request
    /// option.  Keeping the clusters preallocated prevents qcow2
    /// fragmentation that would otherwise be caused by freeing and
    /// re-allocating them later.  Besides potential performance
    /// degradation, such fragmentation can lead to increased allocation
    /// of clusters past the end of the image file, resulting in image
    /// files whose file length can grow much larger than their guest
    /// disk size would suggest.  If image file length is of concern
    /// (e.g. when storing qcow2 images directly on block devices), you
    /// should consider enabling this option.  (since 8.1)
    #[qapi(name = "discard-no-unref")]
    #[qapi(added = "8.1.0")]
    pub discard_no_unref: Option<bool>,
    /// Image decryption options.  Mandatory for encrypted images,
    /// except when doing a metadata-only probe of the image.
    /// (since 2.10)
    #[qapi(added = "2.10.0")]
    pub encrypt: Option<BlockdevQcow2Encryption>,
    /// reference to or definition of the data source block device
    pub file: Box<BlockdevRef>,
    /// the size of each entry in the L2 cache in
    /// bytes.  It must be a power of two between 512 and the cluster
    /// size.  The default value is the cluster size (since 2.12)
    #[qapi(name = "l2-cache-entry-size")]
    #[qapi(added = "2.12.0")]
    pub l2_cache_entry_size: Option<i64>,
    /// the maximum size of the L2 table cache in bytes
    /// (since 2.2)
    #[qapi(name = "l2-cache-size")]
    #[qapi(added = "2.2.0")]
    pub l2_cache_size: Option<i64>,
    /// whether to enable the lazy refcounts feature
    /// (default is taken from the image file)
    #[qapi(name = "lazy-refcounts")]
    pub lazy_refcounts: Option<bool>,
    /// which overlap checks to perform for writes to the
    /// image, defaults to 'cached' (since 2.2)
    #[qapi(name = "overlap-check")]
    #[qapi(added = "2.2.0")]
    pub overlap_check: Option<Qcow2OverlapChecks>,
    /// whether discard requests for the data source
    /// should be issued on other occasions where a cluster gets freed
    #[qapi(name = "pass-discard-other")]
    pub pass_discard_other: Option<bool>,
    /// whether discard requests to the qcow2 device
    /// should be forwarded to the data source
    #[qapi(name = "pass-discard-request")]
    pub pass_discard_request: Option<bool>,
    /// whether discard requests for the data source
    /// should be issued when a snapshot operation (e.g. deleting a
    /// snapshot) frees clusters in the qcow2 file
    #[qapi(name = "pass-discard-snapshot")]
    pub pass_discard_snapshot: Option<bool>,
    /// the maximum size of the refcount block cache
    /// in bytes (since 2.2)
    #[qapi(name = "refcount-cache-size")]
    #[qapi(added = "2.2.0")]
    pub refcount_cache_size: Option<i64>,
}

/// Driver specific block device options for Quorum
///
#[qapi(added = "2.0.0")]
pub struct BlockdevOptionsQuorum {
    /// true if the driver must print content mismatch set to
    /// false by default
    pub blkverify: Option<bool>,
    /// the children block devices to use
    pub children: Box<Vec<BlockdevRef>>,
    /// choose read pattern and set to quorum by default
    /// (Since 2.2)
    #[qapi(name = "read-pattern")]
    #[qapi(added = "2.2.0")]
    pub read_pattern: Option<QuorumReadPattern>,
    /// rewrite corrupted data when quorum is reached
    /// (Since 2.1)
    #[qapi(name = "rewrite-corrupted")]
    #[qapi(added = "2.1.0")]
    pub rewrite_corrupted: Option<bool>,
    /// the vote limit under which a read will fail
    #[qapi(name = "vote-threshold")]
    pub vote_threshold: i64,
}

/// Driver specific block device options for the raw driver.
///
#[qapi(added = "2.8.0")]
pub struct BlockdevOptionsRaw {
    /// reference to or definition of the data source block device
    pub file: Box<BlockdevRef>,
    /// position where the block device starts
    pub offset: Option<i64>,
    /// the assumed size of the device
    pub size: Option<i64>,
}

#[qapi(added = "2.9.0")]
pub struct BlockdevOptionsRbd {
    /// Acceptable authentication modes.  This maps
    /// to Ceph configuration option "auth_client_required".
    /// (Since 3.0)
    #[qapi(name = "auth-client-required")]
    #[qapi(added = "3.0.0")]
    pub auth_client_required: Option<Vec<RbdAuthMode>>,
    /// path to Ceph configuration file.  Values in the configuration
    /// file will be overridden by options specified via QAPI.
    pub conf: Option<String>,
    /// Image encryption options.  (Since 6.1)
    #[qapi(added = "6.1.0")]
    pub encrypt: Option<RbdEncryptionOptions>,
    /// Image name in the Ceph pool.
    pub image: String,
    /// ID of a QCryptoSecret object providing a key for cephx
    /// authentication.  This maps to Ceph configuration option "key".
    /// (Since 3.0)
    #[qapi(name = "key-secret")]
    #[qapi(added = "3.0.0")]
    pub key_secret: Option<String>,
    /// Rados namespace name in the Ceph pool.  (Since 5.0)
    #[qapi(added = "5.0.0")]
    pub namespace: Option<String>,
    /// Ceph pool name.
    pub pool: String,
    /// Monitor host address and port.  This maps to the "mon_host"
    /// Ceph option.
    pub server: Option<Vec<InetSocketAddressBase>>,
    /// Ceph snapshot name.
    pub snapshot: Option<String>,
    /// Ceph id name.
    pub user: Option<String>,
}

/// Driver specific block device options for replication
///
#[qapi(condition = "defined(CONFIG_REPLICATION)")]
#[qapi(added = "2.8.0")]
pub struct BlockdevOptionsReplication {
    /// reference to or definition of the data source block device
    pub file: Box<BlockdevRef>,
    /// the replication mode
    pub mode: ReplicationMode,
    /// In secondary mode, node name or device ID of the root node
    /// who owns the replication node chain.  Must not be given in
    /// primary mode.
    #[qapi(name = "top-id")]
    pub top_id: Option<String>,
}

/// Driver specific block device options for sheepdog
///
#[qapi(added = "2.9.0")]
#[qapi(removed = "6.1.0")]
pub struct BlockdevOptionsSheepdog {
    /// The Sheepdog server to connect to
    pub server: SocketAddress,
    /// Snapshot ID
    #[qapi(name = "snap-id")]
    pub snap_id: Option<u32>,
    /// Snapshot tag name
    ///
    /// Only one of @snap-id and @tag may be present.
    pub tag: Option<String>,
    /// Virtual disk image name
    pub vdi: String,
}

#[qapi(added = "2.8.0")]
pub struct BlockdevOptionsSsh {
    /// Defines how and what to check the host key against
    /// (default: known_hosts)
    #[qapi(name = "host-key-check")]
    #[qapi(added = "2.12.0")]
    pub host_key_check: Option<SshHostKeyCheck>,
    /// path to the image on the host
    pub path: String,
    /// host address
    pub server: InetSocketAddress,
    /// user as which to connect, defaults to current local user name
    pub user: Option<String>,
}

/// Driver specific block device options for the throttle driver
///
#[qapi(added = "2.11.0")]
pub struct BlockdevOptionsThrottle {
    /// reference to or definition of the data source block device
    pub file: Box<BlockdevRef>,
    /// the name of the throttle-group object to use.  It
    /// must already exist.
    #[qapi(name = "throttle-group")]
    pub throttle_group: String,
}

/// Driver specific block device options for the virtio-blk-vfio-pci
/// backend.
///
#[qapi(condition = "CONFIG_BLKIO")]
#[qapi(added = "7.2.0")]
pub struct BlockdevOptionsVirtioBlkVfioPci {
    /// path to the PCI device's sysfs directory (e.g.
    /// /sys/bus/pci/devices/0000:00:01.0).
    pub path: String,
}

/// Driver specific block device options for the virtio-blk-vhost-user
/// backend.
///
#[qapi(condition = "CONFIG_BLKIO")]
#[qapi(added = "7.2.0")]
pub struct BlockdevOptionsVirtioBlkVhostUser {
    /// path to the vhost-user UNIX domain socket.
    pub path: String,
}

/// Driver specific block device options for the virtio-blk-vhost-vdpa backend.
///
#[qapi(condition = "CONFIG_BLKIO")]
#[qapi(feature = "fdset", if = "CONFIG_BLKIO_VHOST_VDPA_FD")]
#[qapi(added = "7.2.0")]
pub struct BlockdevOptionsVirtioBlkVhostVdpa {
    /// path to the vhost-vdpa character device.
    pub path: String,
}

/// Driver specific block device options for the vvfat protocol.
///
#[qapi(name = "BlockdevOptionsVVFAT")]
#[qapi(added = "1.7.0")]
pub struct BlockdevOptionsVvfat {
    /// directory to be exported as FAT image
    pub dir: String,
    /// FAT type: 12, 16 or 32
    #[qapi(name = "fat-type")]
    pub fat_type: Option<i64>,
    /// whether to export a floppy image (true) or partitioned hard
    /// disk (false; default)
    pub floppy: Option<bool>,
    /// set the volume label, limited to 11 bytes.  FAT16 and FAT32
    /// traditionally have some restrictions on labels, which are
    /// ignored by most operating systems.  Defaults to "QEMU VVFAT".
    /// (since 2.4)
    #[qapi(added = "2.4.0")]
    pub label: Option<String>,
    /// whether to allow write operations (default: false)
    pub rw: Option<bool>,
}

/// Driver specific block device options for VxHS
///
#[qapi(name = "BlockdevOptionsVxHS")]
#[qapi(added = "2.10.0")]
#[qapi(removed = "5.1.0")]
pub struct BlockdevOptionsVxHs {
    /// vxhs server IP, port
    pub server: InetSocketAddressBase,
    /// TLS credentials ID
    #[qapi(name = "tls-creds")]
    pub tls_creds: Option<String>,
    /// UUID of VxHS volume
    #[qapi(name = "vdisk-id")]
    pub vdisk_id: String,
}

#[qapi(added = "2.10.0")]
pub struct BlockdevQcow2Encryption {
    /// encryption format
    #[qapi(discriminator)]
    pub format: BlockdevQcow2EncryptionFormat,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<BlockdevQcow2EncryptionBranch>,
}

#[qapi(added = "2.10.0")]
pub enum BlockdevQcow2EncryptionBranch {
    #[qapi(name = "aes")]
    Aes(QCryptoBlockOptionsQCow),
    #[qapi(name = "luks")]
    Luks(QCryptoBlockOptionsLuks),
}

#[qapi(added = "2.10.0")]
pub enum BlockdevQcow2EncryptionFormat {
    /// AES-CBC with plain64 initialization vectors
    #[qapi(name = "aes")]
    Aes,
    #[qapi(name = "luks")]
    Luks,
}

#[qapi(added = "2.12.0")]
pub enum BlockdevQcow2Version {
    /// The original QCOW2 format as introduced in QEMU 0.10 (version
    /// 2)
    #[qapi(name = "v2")]
    V2,
    /// The extended QCOW2 format as introduced in QEMU 1.1 (version 3)
    #[qapi(name = "v3")]
    V3,
}

#[qapi(added = "2.10.0")]
pub struct BlockdevQcowEncryption {
    /// encryption format
    #[qapi(discriminator)]
    pub format: BlockdevQcowEncryptionFormat,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<BlockdevQcowEncryptionBranch>,
}

#[qapi(added = "2.10.0")]
pub enum BlockdevQcowEncryptionBranch {
    #[qapi(name = "aes")]
    Aes(QCryptoBlockOptionsQCow),
}

#[qapi(added = "2.10.0")]
pub enum BlockdevQcowEncryptionFormat {
    /// AES-CBC with plain64 initialization vectors
    #[qapi(name = "aes")]
    Aes,
}

/// Reference to a block device.
///
#[qapi(added = "2.4.0")]
pub enum BlockdevRef {
    /// defines a new block device inline
    #[qapi(name = "definition")]
    Definition(BlockdevOptions),
    /// references the ID of an existing block device
    #[qapi(name = "reference")]
    Reference(String),
}

#[qapi(added = "1.7.0")]
#[qapi(removed = "2.4.0")]
pub enum BlockdevRefBranch {
    #[qapi(name = "definition")]
    Definition(BlockdevOptions),
    #[qapi(name = "reference")]
    Reference(String),
}

/// Reference to a block device.
///
#[qapi(added = "2.10.0")]
pub enum BlockdevRefOrNull {
    /// defines a new block device inline
    #[qapi(name = "definition")]
    Definition(BlockdevOptions),
    /// No block device should be referenced (since 2.10)
    #[qapi(name = "null")]
    Null(()),
    /// references the ID of an existing block device.  An empty
    /// string means that no block device should be referenced.
    /// Deprecated; use null instead.
    #[qapi(name = "reference")]
    Reference(String),
}

#[qapi(added = "1.7.0")]
#[qapi(removed = "2.4.0")]
pub enum BlockdevRefType {
    #[qapi(name = "definition")]
    Definition,
    #[qapi(name = "reference")]
    Reference,
}

/// Generates a snapshot of a block device.
///
/// For the arguments, see the documentation of BlockdevSnapshot.
///
#[qapi(name = "blockdev-snapshot")]
#[qapi(command)]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockdevSnapshot {
    /// optional the name of the device to generate the snapshot from.
    #[qapi(removed = "2.5.0")]
    pub device: Option<String>,
    /// optional the format of the snapshot image, default is 'qcow2'.
    #[qapi(removed = "2.5.0")]
    pub format: Option<String>,
    /// optional whether and how QEMU should create a new image, default is
    /// 'absolute-paths'.
    ///
    #[qapi(removed = "2.5.0")]
    pub mode: Option<NewImageMode>,
    /// device or node name that will have a snapshot taken.
    #[qapi(added = "2.5.0")]
    pub node: Option<String>,
    /// optional graph node name to generate the snapshot from (Since 2.0)
    #[qapi(name = "node-name")]
    #[qapi(added = "2.0.0")]
    #[qapi(removed = "2.5.0")]
    pub node_name: Option<String>,
    /// reference to the existing block device that will become
    /// the overlay of @node, as part of taking the snapshot.  It must
    /// not have a current backing file (this can be achieved by passing
    /// "backing": null to `blockdev-add`).
    #[qapi(added = "2.5.0")]
    pub overlay: Option<String>,
    /// the target of the new image. A new file will be created.
    #[qapi(name = "snapshot-file")]
    #[qapi(removed = "2.5.0")]
    pub snapshot_file: Option<String>,
    /// optional the graph node name of the new image (Since 2.0)
    #[qapi(name = "snapshot-node-name")]
    #[qapi(added = "2.0.0")]
    #[qapi(removed = "2.5.0")]
    pub snapshot_node_name: Option<String>,
}

#[qapi(added = "1.7.0")]
pub struct BlockdevSnapshotInternal {
    /// the device name or node-name of a root node to generate the
    /// snapshot from
    pub device: String,
    /// the name of the internal snapshot to be created
    pub name: String,
}

#[qapi(added = "6.2.0")]
pub struct BlockdevSnapshotInternalWrapper {
    pub data: BlockdevSnapshotInternal,
}

/// Either @device or @node-name must be set but not both.
///
#[qapi(command)]
#[qapi(name = "blockdev-snapshot-sync")]
#[qapi(returns = "()")]
#[qapi(added = "1.1.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockdevSnapshotSync {
    /// optional the name of the device to generate the snapshot from.
    pub device: Option<String>,
    /// optional the format of the snapshot image, default is 'qcow2'.
    pub format: Option<String>,
    /// optional whether and how QEMU should create a new image, default is
    /// 'absolute-paths'.
    ///
    pub mode: Option<NewImageMode>,
    /// optional graph node name to generate the snapshot from (Since 2.0)
    #[qapi(name = "node-name")]
    #[qapi(added = "2.0.0")]
    pub node_name: Option<String>,
    /// the target of the new image. A new file will be created.
    #[qapi(name = "snapshot-file")]
    pub snapshot_file: String,
    /// optional the graph node name of the new image (Since 2.0)
    #[qapi(name = "snapshot-node-name")]
    #[qapi(added = "2.0.0")]
    pub snapshot_node_name: Option<String>,
}

#[qapi(added = "6.2.0")]
pub struct BlockdevSnapshotSyncWrapper {
    pub data: BlockdevSnapshotSync,
}

#[qapi(added = "6.2.0")]
pub struct BlockdevSnapshotWrapper {
    pub data: BlockdevSnapshot,
}

#[qapi(added = "2.12.0")]
pub enum BlockdevVhdxSubformat {
    /// Growing image file
    #[qapi(name = "dynamic")]
    Dynamic,
    /// Preallocated fixed-size image file
    #[qapi(name = "fixed")]
    Fixed,
}

/// Adapter type info for VMDK images
///
#[qapi(added = "4.0.0")]
pub enum BlockdevVmdkAdapterType {
    #[qapi(name = "buslogic")]
    Buslogic,
    #[qapi(name = "ide")]
    Ide,
    #[qapi(name = "legacyESX")]
    LegacyEsx,
    #[qapi(name = "lsilogic")]
    Lsilogic,
}

/// Subformat options for VMDK images
///
#[qapi(added = "4.0.0")]
pub enum BlockdevVmdkSubformat {
    /// Single flat data image and a descriptor file
    #[qapi(name = "monolithicFlat")]
    MonolithicFlat,
    /// Single file image with sparse cluster allocation
    #[qapi(name = "monolithicSparse")]
    MonolithicSparse,
    /// Single file image sparse cluster allocation,
    /// optimized for streaming over network.
    #[qapi(name = "streamOptimized")]
    StreamOptimized,
    /// Data is split into 2GB (per virtual LBA) flat
    /// extent files, in addition to a descriptor file
    #[qapi(name = "twoGbMaxExtentFlat")]
    TwoGbMaxExtentFlat,
    /// Data is split into 2GB (per virtual LBA)
    /// sparse extent files, in addition to a descriptor file
    #[qapi(name = "twoGbMaxExtentSparse")]
    TwoGbMaxExtentSparse,
}

#[qapi(added = "2.12.0")]
pub enum BlockdevVpcSubformat {
    /// Growing image file
    #[qapi(name = "dynamic")]
    Dynamic,
    /// Preallocated fixed-size image file
    #[qapi(name = "fixed")]
    Fixed,
}

#[qapi(added = "2.4.0")]
pub struct BlockDirtyBitmap {
    /// name of the dirty bitmap
    pub name: String,
    /// name of device/node which the bitmap is tracking
    pub node: String,
}

#[qapi(name = "block-dirty-bitmap-add")]
#[qapi(command)]
#[qapi(returns = "()")]
#[qapi(added = "2.4.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockDirtyBitmapAdd {
    /// ignored and deprecated since 2.12.
    /// Currently, all dirty tracking bitmaps are loaded from Qcow2 on
    /// open.
    #[qapi(added = "2.10.0")]
    #[qapi(removed = "4.2.0")]
    pub autoload: Option<bool>,
    /// the bitmap is created in the disabled state, which means
    /// that it will not track drive changes.  The bitmap may be enabled
    /// with `block-dirty-bitmap-enable`.  Default is false.
    /// (Since: 4.0)
    #[qapi(added = "4.0.0")]
    pub disabled: Option<bool>,
    /// the bitmap granularity, default is 64k for
    /// `block-dirty-bitmap-add`
    pub granularity: Option<u32>,
    /// name of the dirty bitmap (must be less than 1024 bytes)
    pub name: String,
    /// name of device/node which the bitmap is tracking
    pub node: String,
    /// the bitmap is persistent, i.e. it will be saved to the
    /// corresponding block device image file on its close.  For now
    /// only Qcow2 disks support persistent bitmaps.  Default is false
    /// for `block-dirty-bitmap-add`.  This fails if the node is
    /// read-only or inactive, since such a bitmap could never be
    /// stored.  (Since: 2.10)
    #[qapi(added = "2.10.0")]
    pub persistent: Option<bool>,
    /// the bitmap is created in the disabled state, which means that
    /// it will not track drive changes. The bitmap may be enabled with
    /// x-block-dirty-bitmap-enable. Default is false. (Since: 3.0)
    #[qapi(name = "x-disabled")]
    #[qapi(added = "3.0.0")]
    #[qapi(removed = "4.0.0")]
    pub x_disabled: Option<bool>,
}

#[qapi(added = "6.2.0")]
pub struct BlockDirtyBitmapAddWrapper {
    pub data: BlockDirtyBitmapAdd,
}

#[qapi(name = "block-dirty-bitmap-merge")]
#[qapi(command)]
#[qapi(returns = "()")]
#[qapi(added = "3.0.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct BlockDirtyBitmapMerge {
    /// name(s) of the source dirty bitmap(s) at @node and/or
    /// fully specified `BlockDirtyBitmap` elements.  The latter are
    /// supported since 4.1.
    #[qapi(added = "4.0.0")]
    pub bitmaps: Option<Vec<BlockDirtyBitmapOrStr>>,
    /// name of the destination dirty bitmap
    #[qapi(removed = "4.0.0")]
    pub dst_name: Option<String>,
    /// name of device/node which the @target bitmap is tracking
    pub node: String,
    /// name of the source dirty bitmap
    #[qapi(removed = "4.0.0")]
    pub src_name: Option<String>,
    /// name of the destination dirty bitmap
    #[qapi(added = "4.0.0")]
    pub target: Option<String>,
}

#[qapi(added = "4.1.0")]
#[qapi(removed = "7.1.0")]
pub enum BlockDirtyBitmapMergeSource {
    /// bitmap with specified node
    #[qapi(name = "external")]
    External(BlockDirtyBitmap),
    /// name of the bitmap, attached to the same node as target bitmap.
    #[qapi(name = "local")]
    Local(String),
}

#[qapi(added = "6.2.0")]
pub struct BlockDirtyBitmapMergeWrapper {
    pub data: BlockDirtyBitmapMerge,
}

#[qapi(added = "7.1.0")]
pub enum BlockDirtyBitmapOrStr {
    /// bitmap with specified node
    #[qapi(name = "external")]
    External(BlockDirtyBitmap),
    /// name of the bitmap, attached to the same node as target
    /// bitmap.
    #[qapi(name = "local")]
    Local(String),
}

/// SHA256 hash of dirty bitmap data
///
#[qapi(added = "2.10.0")]
pub struct BlockDirtyBitmapSha256 {
    /// ASCII representation of SHA256 bitmap hash
    pub sha256: String,
}

#[qapi(added = "6.2.0")]
pub struct BlockDirtyBitmapWrapper {
    pub data: BlockDirtyBitmap,
}

/// Block dirty bitmap information.
///
#[qapi(added = "1.3.0")]
pub struct BlockDirtyInfo {
    /// true if the bitmap is in-use by some operation (NBD or jobs)
    /// and cannot be modified via QMP or used by another operation.
    /// (since 4.0)
    #[qapi(added = "4.0.0")]
    pub busy: Option<bool>,
    /// number of dirty bytes according to the dirty bitmap
    pub count: i64,
    /// granularity of the dirty bitmap in bytes (since 1.4)
    #[qapi(added = "1.4.0")]
    pub granularity: Option<u32>,
    /// true if this is a persistent bitmap that was
    /// improperly stored.  Implies @persistent to be true; @recording
    /// and @busy to be false.  This bitmap cannot be used.  To remove
    /// it, use `block-dirty-bitmap-remove`.  (Since 4.0)
    #[qapi(added = "4.0.0")]
    pub inconsistent: Option<bool>,
    /// the name of the dirty bitmap (Since 2.4)
    #[qapi(added = "2.4.0")]
    pub name: Option<String>,
    /// true if the bitmap was stored on disk, is scheduled to
    /// be stored on disk, or both.  (since 4.0)
    #[qapi(added = "4.0.0")]
    pub persistent: Option<bool>,
    /// true if the bitmap is recording new writes from the
    /// guest.  (since 4.0)
    #[qapi(added = "4.0.0")]
    pub recording: Option<bool>,
    /// Deprecated in favor of @recording and @locked. (since 2.4)
    #[qapi(feature = "deprecated")]
    #[qapi(added = "2.4.0")]
    #[qapi(deprecated(added = "5.0.0"))]
    #[qapi(removed = "6.0.0")]
    pub status: Option<DirtyBitmapStatus>,
}

/// An enumeration of action that has been taken when a DISK I/O occurs
///
#[qapi(added = "2.1.0")]
pub enum BlockErrorAction {
    /// error has been ignored
    #[qapi(name = "ignore")]
    Ignore,
    /// error has been reported to the device
    #[qapi(name = "report")]
    Report,
    /// error caused VM to be stopped
    #[qapi(name = "stop")]
    Stop,
}

/// Describes a block export, i.e. how single node should be exported on an
/// external interface.
///
#[qapi(added = "5.0.0")]
#[qapi(removed = "5.2.0")]
pub struct BlockExport {
    #[qapi(discriminator)]
    pub r#type: BlockExportType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<BlockExportBranch>,
}

#[qapi(added = "5.0.0")]
#[qapi(removed = "5.2.0")]
pub enum BlockExportBranch {
    #[qapi(name = "nbd")]
    Nbd(BlockExportNbd),
}

/// Information about a single block export.
///
#[qapi(added = "5.2.0")]
pub struct BlockExportInfo {
    /// The unique identifier for the block export
    pub id: String,
    /// The node name of the block node that is exported
    #[qapi(name = "node-name")]
    pub node_name: String,
    /// True if the export is shutting down (e.g. after a
    /// `block-export-del` command, but before the shutdown has
    /// completed)
    #[qapi(name = "shutting-down")]
    pub shutting_down: bool,
    /// The block export type
    pub r#type: BlockExportType,
}

/// Specify a single or multiple I/O threads in which to run a block
/// export's I/O.
///
#[qapi(added = "11.0.0")]
pub enum BlockExportIothreads {
    /// Use multi-threading across the given set of I/O threads,
    /// which must not be empty.  Note: Passing a single I/O thread via
    /// this variant is still treated as multi-threading, which is
    /// different from using the @single variant.  In particular, even
    /// if there only is a single I/O thread in the set, export types
    /// that do not support multi-threading will generally reject this
    /// variant, and BlockExportOptions.fixed-iothread is always
    /// incompatible with it.
    #[qapi(name = "multi")]
    Multi(Vec<String>),
    /// Run the export's I/O in the given single I/O thread.
    #[qapi(name = "single")]
    Single(String),
}

/// An NBD block export.
///
#[qapi(added = "5.0.0")]
#[qapi(removed = "5.2.0")]
pub struct BlockExportNbd {
    /// Also export the dirty bitmap reachable from @device, so the
    /// NBD client can use NBD_OPT_SET_META_CONTEXT with
    /// "qemu:dirty-bitmap:NAME" to inspect the bitmap. (since 4.0)
    pub bitmap: Option<String>,
    /// Free-form description of the export, up to 4096 bytes.
    /// (Since 5.0)
    pub description: Option<String>,
    /// The device name or node name of the node to be exported
    pub device: String,
    /// Export name. If unspecified, the @device parameter is used as the
    /// export name. (Since 2.12)
    pub name: Option<String>,
    /// Whether clients should be able to write to the device via the
    /// NBD connection (default false).
    pub writable: Option<bool>,
}

/// Describes a block export, i.e. how single node should be exported on
/// an external interface.
///
#[qapi(added = "5.2.0")]
pub struct BlockExportOptions {
    /// If true, the export allows the exported node to be
    /// inactive.  If it is created for an inactive block node, the node
    /// remains inactive.  If the export type doesn't support running on
    /// an inactive node, an error is returned.  If false, inactive
    /// block nodes are automatically activated before creating the
    /// export and trying to inactivate them later fails.
    /// (since: 10.0; default: false)
    #[qapi(name = "allow-inactive")]
    #[qapi(added = "10.0.0")]
    pub allow_inactive: Option<bool>,
    /// True prevents the block node from being moved to
    /// another thread while the export is active.  If true and
    #[qapi(name = "fixed-iothread")]
    pub fixed_iothread: Option<bool>,
    /// A unique identifier for the block export (across all export
    /// types)
    pub id: String,
    /// is given, export creation fails if the block node
    /// cannot be moved to the iothread.  Must not be true when giving
    /// multiple iothreads for @iothread.  The default is false.
    /// (since: 5.2)
    pub iothread: Option<BlockExportIothreads>,
    /// The node name of the block node to be exported
    /// (since: 5.2)
    #[qapi(name = "node-name")]
    pub node_name: String,
    /// Block export type
    #[qapi(discriminator)]
    pub r#type: BlockExportType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<BlockExportOptionsBranch>,
    /// True if clients should be able to write to the export
    /// (default false)
    pub writable: Option<bool>,
    /// If true, caches are flushed after every write request
    /// to the export before completion is signalled.  (since: 5.2;
    /// default: false)
    pub writethrough: Option<bool>,
}

#[qapi(added = "5.2.0")]
pub enum BlockExportOptionsBranch {
    #[qapi(name = "fuse")]
    #[qapi(condition = "defined(CONFIG_FUSE)")]
    #[qapi(added = "6.0.0")]
    Fuse(BlockExportOptionsFuse),
    #[qapi(name = "nbd")]
    Nbd(BlockExportOptionsNbd),
    #[qapi(name = "vduse-blk")]
    #[qapi(condition = "CONFIG_VDUSE_BLK_EXPORT")]
    #[qapi(added = "7.1.0")]
    VduseBlk(BlockExportOptionsVduseBlk),
    #[qapi(name = "vhost-user-blk")]
    #[qapi(condition = "CONFIG_VHOST_USER_BLK_SERVER")]
    VhostUserBlk(BlockExportOptionsVhostUserBlk),
}

/// Options for exporting a block graph node on some (file) mountpoint
/// as a raw image.
///
/// Multi-threading note: The FUSE export supports multi-threading.
/// Currently, requests are distributed across these threads in a
/// round-robin fashion, i.e. independently of the CPU core from which a
/// request originates.
///
#[qapi(condition = "defined(CONFIG_FUSE)")]
#[qapi(added = "6.0.0")]
pub struct BlockExportOptionsFuse {
    /// If this is off, only QEMU's user is allowed access to
    /// this export.  That cannot be changed even with chmod or chown.
    /// Enabling this option will allow other users access to the export
    /// with the FUSE mount option "allow_other".  Note that using
    /// allow_other as a non-root user requires user_allow_other to be
    /// enabled in the global fuse.conf configuration file.  In auto
    /// mode (the default), the FUSE export driver will first attempt to
    /// mount the export with allow_other, and if that fails, try again
    /// without.  (since 6.1; default: auto)
    #[qapi(name = "allow-other")]
    #[qapi(added = "6.1.0")]
    pub allow_other: Option<FuseExportAllowOther>,
    /// Whether writes beyond the EOF should grow the block node
    /// accordingly.  (default: false)
    pub growable: Option<bool>,
    /// Path on which to export the block device via FUSE.
    /// This must point to an existing regular file.
    pub mountpoint: String,
}

/// An NBD block export (distinct options used in the NBD branch of
/// `block-export-add`).
///
#[qapi(added = "5.2.0")]
pub struct BlockExportOptionsNbd {
    /// Also export the allocation depth map for @device,
    /// so the NBD client can use NBD_OPT_SET_META_CONTEXT with the
    /// metadata context name "qemu:allocation-depth" to inspect
    /// allocation details.  (since 5.2)
    #[qapi(name = "allocation-depth")]
    pub allocation_depth: Option<bool>,
    /// Also export each of the named dirty bitmaps reachable from
    pub bitmaps: Option<Vec<BlockDirtyBitmapOrStr>>,
    /// Free-form description of the export, up to 4096 bytes.
    /// (Since 5.0)
    pub description: Option<String>,
    /// Export name.  If unspecified, the @device parameter is used
    /// as the export name.  (Since 2.12)
    pub name: Option<String>,
}

/// An NBD block export (common options shared between `nbd-server-add`
/// and the NBD branch of `block-export-add`).
///
#[qapi(added = "5.2.0")]
pub struct BlockExportOptionsNbdBase {
    /// Free-form description of the export, up to 4096 bytes.
    /// (Since 5.0)
    pub description: Option<String>,
    /// Export name.  If unspecified, the @device parameter is used
    /// as the export name.  (Since 2.12)
    pub name: Option<String>,
}

/// A vduse-blk block export.
///
#[qapi(added = "7.1.0")]
pub struct BlockExportOptionsVduseBlk {
    /// Logical block size in bytes.  Range [512,
    /// PAGE_SIZE] and must be power of 2.  Defaults to 512 bytes.
    #[qapi(name = "logical-block-size")]
    pub logical_block_size: Option<u64>,
    /// the name of VDUSE device (must be unique across the host).
    pub name: String,
    /// the number of virtqueues.  Defaults to 1.
    #[qapi(name = "num-queues")]
    pub num_queues: Option<u16>,
    /// the size of virtqueue.  Defaults to 256.
    #[qapi(name = "queue-size")]
    pub queue_size: Option<u16>,
    /// the serial number of virtio block device.  Defaults to
    /// empty string.
    pub serial: Option<String>,
}

/// A vhost-user-blk block export.
///
#[qapi(added = "5.2.0")]
pub struct BlockExportOptionsVhostUserBlk {
    /// The vhost-user socket on which to listen.  Both 'unix' and
    /// 'fd' `SocketAddress` types are supported.  Passed fds must be
    /// UNIX domain sockets.
    pub addr: SocketAddress,
    /// Logical block size in bytes.  Defaults to 512
    /// bytes.
    #[qapi(name = "logical-block-size")]
    pub logical_block_size: Option<u64>,
    /// Number of request virtqueues.  Must be greater than 0.
    /// Defaults to 1.
    #[qapi(name = "num-queues")]
    pub num_queues: Option<u16>,
}

/// Mode for removing a block export.
///
#[qapi(added = "5.2.0")]
pub enum BlockExportRemoveMode {
    /// Drop all connections immediately and remove export.
    ///
    /// Potential additional modes to be added in the future:
    ///
    /// hide: Just hide export from new clients, leave existing connections
    /// as is.  Remove export after all clients are disconnected.
    ///
    /// soft: Hide export from new clients, answer with ESHUTDOWN for all
    /// further requests from existing clients.
    #[qapi(name = "hard")]
    Hard,
    /// Remove export if there are no existing connections, fail
    /// otherwise.
    #[qapi(name = "safe")]
    Safe,
}

/// An enumeration of block export types
///
#[qapi(added = "5.0.0")]
pub enum BlockExportType {
    /// FUSE export (since: 6.0)
    #[qapi(name = "fuse")]
    #[qapi(condition = "defined(CONFIG_FUSE)")]
    #[qapi(added = "6.0.0")]
    Fuse,
    /// NBD export
    #[qapi(name = "nbd")]
    Nbd,
    /// vduse-blk export (since 7.1)
    #[qapi(name = "vduse-blk")]
    #[qapi(condition = "CONFIG_VDUSE_BLK_EXPORT")]
    #[qapi(added = "7.1.0")]
    VduseBlk,
    /// vhost-user-blk export (since 5.2)
    #[qapi(name = "vhost-user-blk")]
    #[qapi(condition = "CONFIG_VHOST_USER_BLK_SERVER")]
    #[qapi(added = "5.2.0")]
    VhostUserBlk,
}

/// Information about all nodes in a block (sub)graph in the form of
/// `BlockNodeInfo` data.  The base `BlockNodeInfo` struct contains the
/// information for the (sub)graph's root node.
///
#[qapi(added = "8.0.0")]
pub struct BlockGraphInfo {
    /// actual size on disk in bytes of the image
    #[qapi(name = "actual-size")]
    pub actual_size: Option<i64>,
    /// name of the backing file
    #[qapi(name = "backing-filename")]
    pub backing_filename: Option<String>,
    /// the format of the backing file
    #[qapi(name = "backing-filename-format")]
    pub backing_filename_format: Option<String>,
    /// Array of links to this node's child nodes' information
    pub children: Box<Vec<BlockChildInfo>>,
    /// size of a cluster in bytes
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<i64>,
    /// true if the image is compressed (Since 1.7)
    pub compressed: Option<bool>,
    /// true if image is not cleanly closed
    #[qapi(name = "dirty-flag")]
    pub dirty_flag: Option<bool>,
    /// true if the image is encrypted
    pub encrypted: Option<bool>,
    /// name of the image file
    pub filename: String,
    /// format of the image file
    pub format: String,
    /// structure supplying additional format-specific
    /// information (since 1.7)
    #[qapi(name = "format-specific")]
    pub format_specific: Option<ImageInfoSpecific>,
    /// full path of the backing file
    #[qapi(name = "full-backing-filename")]
    pub full_backing_filename: Option<String>,
    /// block limits that are used for I/O on the node (Since 10.2)
    #[qapi(added = "10.2.0")]
    pub limits: Option<BlockLimitsInfo>,
    /// list of VM snapshots
    pub snapshots: Option<Vec<SnapshotInfo>>,
    /// maximum capacity in bytes of the image
    #[qapi(name = "virtual-size")]
    pub virtual_size: i64,
}

/// Block device information.  This structure describes a virtual device
/// and the backing device associated with it.
///
#[qapi(added = "1.0.0")]
pub struct BlockInfo {
    /// The device name associated with the virtual device.
    pub device: String,
    /// optional dirty bitmap information (only present if the dirty
    /// bitmap is enabled)
    #[qapi(added = "1.3.0")]
    #[qapi(removed = "2.0.0")]
    pub dirty: Option<BlockDirtyInfo>,
    /// dirty bitmaps information (only present if the
    /// driver has one or more dirty bitmaps) (Since 2.0)
    /// Deprecated in 4.2; see BlockDeviceInfo instead.
    #[qapi(name = "dirty-bitmaps")]
    #[qapi(feature = "deprecated")]
    #[qapi(added = "2.0.0")]
    #[qapi(deprecated(added = "5.0.0"))]
    #[qapi(removed = "6.0.0")]
    pub dirty_bitmaps: Option<Vec<BlockDirtyInfo>>,
    /// `BlockDeviceInfo` describing the device if media is
    /// present
    pub inserted: Option<BlockDeviceInfo>,
    /// `BlockDeviceIoStatus`.  Only present if the device
    /// supports it and the VM is configured to stop on errors
    /// (supported device models: virtio-blk, IDE, SCSI except
    /// scsi-generic)
    #[qapi(name = "io-status")]
    pub io_status: Option<BlockDeviceIoStatus>,
    /// True if the guest has locked this device from having its
    /// media removed
    pub locked: bool,
    /// The qdev ID, or if no ID is assigned, the QOM path of the
    /// block device.  (since 2.10)
    #[qapi(added = "2.10.0")]
    pub qdev: Option<String>,
    /// True if the device supports removable media.
    pub removable: bool,
    /// True if the device's tray is open (only present if it
    /// has a tray)
    pub tray_open: Option<bool>,
    /// This field is returned only for compatibility reasons, it
    /// should not be used (always returns 'unknown')
    pub r#type: String,
}

/// A set of parameters describing block throttling.
///
#[qapi(name = "BlockIOThrottle")]
#[qapi(added = "2.7.0")]
pub struct BlockIoThrottle {
    /// total throughput limit in bytes per second
    pub bps: i64,
    /// total throughput limit during bursts,
    /// in bytes (Since 1.7)
    pub bps_max: Option<i64>,
    /// maximum length of the @bps_max burst
    /// period, in seconds. It must only
    /// be set if @bps_max is set as well.
    /// Defaults to 1. (Since 2.6)
    pub bps_max_length: Option<i64>,
    /// read throughput limit in bytes per second
    pub bps_rd: i64,
    /// read throughput limit during bursts,
    /// in bytes (Since 1.7)
    pub bps_rd_max: Option<i64>,
    /// maximum length of the @bps_rd_max
    /// burst period, in seconds. It must only
    /// be set if @bps_rd_max is set as well.
    /// Defaults to 1. (Since 2.6)
    pub bps_rd_max_length: Option<i64>,
    /// write throughput limit in bytes per second
    pub bps_wr: i64,
    /// write throughput limit during bursts,
    /// in bytes (Since 1.7)
    pub bps_wr_max: Option<i64>,
    /// maximum length of the @bps_wr_max
    /// burst period, in seconds. It must only
    /// be set if @bps_wr_max is set as well.
    /// Defaults to 1. (Since 2.6)
    pub bps_wr_max_length: Option<i64>,
    /// Block device name (deprecated, use @id instead)
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "5.0.0"))]
    pub device: Option<String>,
    /// throttle group name (Since 2.4)
    pub group: Option<String>,
    /// The name or QOM path of the guest device (since: 2.8)
    #[qapi(added = "2.8.0")]
    pub id: Option<String>,
    /// total I/O operations per second
    pub iops: i64,
    /// total I/O operations per second during bursts,
    /// in bytes (Since 1.7)
    pub iops_max: Option<i64>,
    /// maximum length of the @iops burst
    /// period, in seconds. It must only
    /// be set if @iops_max is set as well.
    /// Defaults to 1. (Since 2.6)
    pub iops_max_length: Option<i64>,
    /// read I/O operations per second
    pub iops_rd: i64,
    /// read I/O operations per second during bursts,
    /// in bytes (Since 1.7)
    pub iops_rd_max: Option<i64>,
    /// maximum length of the @iops_rd_max
    /// burst period, in seconds. It must only
    /// be set if @iops_rd_max is set as well.
    /// Defaults to 1. (Since 2.6)
    pub iops_rd_max_length: Option<i64>,
    /// an I/O size in bytes (Since 1.7)
    pub iops_size: Option<i64>,
    /// write I/O operations per second
    pub iops_wr: i64,
    /// write I/O operations per second during bursts,
    /// in bytes (Since 1.7)
    pub iops_wr_max: Option<i64>,
    /// maximum length of the @iops_wr_max
    /// burst period, in seconds. It must only
    /// be set if @iops_wr_max is set as well.
    /// Defaults to 1. (Since 2.6)
    pub iops_wr_max_length: Option<i64>,
}

/// Block job options that can be changed after job creation.
///
#[qapi(added = "8.2.0")]
pub struct BlockJobChangeOptions {
    /// The job identifier
    pub id: String,
    /// The job type
    #[qapi(discriminator)]
    pub r#type: JobType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<BlockJobChangeOptionsBranch>,
}

#[qapi(added = "8.2.0")]
pub enum BlockJobChangeOptionsBranch {
    /// image options amend job type, see `x-blockdev-amend`
    /// (since 5.1)
    #[qapi(name = "amend")]
    Amend,
    /// drive backup job type, see `drive-backup`
    #[qapi(name = "backup")]
    Backup,
    /// block commit job type, see `block-commit`
    #[qapi(name = "commit")]
    Commit,
    /// image creation job type, see `blockdev-create` (since 3.0)
    #[qapi(name = "create")]
    Create,
    #[qapi(name = "mirror")]
    Mirror(BlockJobChangeOptionsMirror),
    /// snapshot delete job type, see `snapshot-delete`
    /// (since 6.0)
    #[qapi(name = "snapshot-delete")]
    SnapshotDelete,
    /// snapshot load job type, see `snapshot-load`
    /// (since 6.0)
    #[qapi(name = "snapshot-load")]
    SnapshotLoad,
    /// snapshot save job type, see `snapshot-save`
    /// (since 6.0)
    #[qapi(name = "snapshot-save")]
    SnapshotSave,
    /// block stream job type, see `block-stream`
    #[qapi(name = "stream")]
    Stream,
}

#[qapi(added = "8.2.0")]
pub struct BlockJobChangeOptionsMirror {
    /// Switch to this copy mode.  Currently, only the switch
    /// from 'background' to 'write-blocking' is implemented.
    #[qapi(name = "copy-mode")]
    pub copy_mode: MirrorCopyMode,
}

/// Information about a long-running block device operation.
///
#[qapi(added = "1.1.0")]
pub struct BlockJobInfo {
    /// Job will dismiss itself when CONCLUDED, and
    /// disappear.  (since 2.12)
    #[qapi(name = "auto-dismiss")]
    #[qapi(added = "2.12.0")]
    pub auto_dismiss: Option<bool>,
    /// Job will finalize itself when PENDING, moving to the
    /// CONCLUDED state.  (since 2.12)
    #[qapi(name = "auto-finalize")]
    #[qapi(added = "2.12.0")]
    pub auto_finalize: Option<bool>,
    /// false if the job is known to be in a quiescent state, with no
    /// pending I/O.  (Since 1.3)
    #[qapi(added = "1.3.0")]
    pub busy: Option<bool>,
    /// The job identifier.  Originally the device name but other
    /// values are allowed since QEMU 2.7
    pub device: String,
    /// Error information if the job did not complete successfully.
    /// Not set if the job completed successfully.  (since 2.12.1)
    #[qapi(added = "2.12.1")]
    pub error: Option<String>,
    /// the status of the job (since 1.3)
    #[qapi(name = "io-status")]
    #[qapi(added = "1.3.0")]
    pub io_status: Option<BlockDeviceIoStatus>,
    /// Estimated @offset value at the completion of the job.  This
    /// value can arbitrarily change while the job is running, in both
    /// directions.
    ///
    ///   .: The value is monotonically increasing.
    pub len: i64,
    /// Progress made until now.  The unit is arbitrary and the
    /// value can only meaningfully be used for the ratio of @offset to
    pub offset: i64,
    /// whether the job is paused or, if @busy is true, will pause
    /// itself as soon as possible.  (Since 1.3)
    #[qapi(added = "1.3.0")]
    pub paused: Option<bool>,
    /// true if the job may be completed (since 2.2)
    #[qapi(added = "2.2.0")]
    pub ready: Option<bool>,
    /// the rate limit, bytes per second
    pub speed: i64,
    /// Current job state/status (since 2.12)
    #[qapi(added = "2.12.0")]
    pub status: Option<JobStatus>,
    /// the job type ('stream' for image streaming)
    #[qapi(discriminator)]
    pub r#type: JobType,
    #[qapi(union)]
    #[qapi(flatten)]
    #[qapi(added = "8.2.0")]
    pub u: Option<BlockJobInfoBranch>,
}

#[qapi(added = "8.2.0")]
pub enum BlockJobInfoBranch {
    /// image options amend job type, see `x-blockdev-amend`
    /// (since 5.1)
    #[qapi(name = "amend")]
    Amend,
    /// drive backup job type, see `drive-backup`
    #[qapi(name = "backup")]
    Backup,
    /// block commit job type, see `block-commit`
    #[qapi(name = "commit")]
    Commit,
    /// image creation job type, see `blockdev-create` (since 3.0)
    #[qapi(name = "create")]
    Create,
    #[qapi(name = "mirror")]
    Mirror(BlockJobInfoMirror),
    /// snapshot delete job type, see `snapshot-delete`
    /// (since 6.0)
    #[qapi(name = "snapshot-delete")]
    SnapshotDelete,
    /// snapshot load job type, see `snapshot-load`
    /// (since 6.0)
    #[qapi(name = "snapshot-load")]
    SnapshotLoad,
    /// snapshot save job type, see `snapshot-save`
    /// (since 6.0)
    #[qapi(name = "snapshot-save")]
    SnapshotSave,
    /// block stream job type, see `block-stream`
    #[qapi(name = "stream")]
    Stream,
}

/// Information specific to mirror block jobs.
///
#[qapi(added = "8.2.0")]
pub struct BlockJobInfoMirror {
    /// Whether the source is actively synced to the
    /// target, i.e. same data and new writes are done synchronously to
    /// both.
    #[qapi(name = "actively-synced")]
    pub actively_synced: bool,
}

/// Indicates the present state of a given blockjob in its lifetime.
///
#[qapi(added = "2.12.0")]
#[qapi(removed = "3.0.0")]
pub enum BlockJobStatus {
    /// The job is in the process of being aborted, and will finish with
    /// an error. The job will afterwards report that it is @concluded.
    /// This status may not be visible to the management process.
    #[qapi(name = "aborting")]
    Aborting,
    /// The job has finished all work. If manual was set to true, the job
    /// will remain in the query list until it is dismissed.
    #[qapi(name = "concluded")]
    Concluded,
    /// The job has been created, but not yet started.
    #[qapi(name = "created")]
    Created,
    /// The job is in the process of being dismantled. This state should not
    /// ever be visible externally.
    #[qapi(name = "null")]
    Null,
    /// The job is running, but paused. The pause may be requested by
    /// either the QMP user or by internal processes.
    #[qapi(name = "paused")]
    Paused,
    /// The job has finished its work, but has finalization steps that it
    /// needs to make prior to completing. These changes may require
    /// manual intervention by the management process if manual was set
    /// to true. These changes may still fail.
    #[qapi(name = "pending")]
    Pending,
    /// The job is running, but is ready for the user to signal completion.
    /// This is used for long-running jobs like mirror that are designed to
    /// run indefinitely.
    #[qapi(name = "ready")]
    Ready,
    /// The job is currently running.
    #[qapi(name = "running")]
    Running,
    /// The job is ready, but paused. This is nearly identical to @paused.
    /// The job may return to @ready or otherwise be canceled.
    #[qapi(name = "standby")]
    Standby,
    /// Erroneous, default state. Should not ever be visible.
    #[qapi(name = "undefined")]
    Undefined,
    /// The job is waiting for other jobs in the transaction to converge
    /// to the waiting state. This status will likely not be visible for
    /// the last job in a transaction.
    #[qapi(name = "waiting")]
    Waiting,
}

/// Type of a block job.
///
#[qapi(added = "1.7.0")]
#[qapi(removed = "3.0.0")]
pub enum BlockJobType {
    /// drive backup job type, see "drive-backup"
    #[qapi(name = "backup")]
    Backup,
    /// block commit job type, see "block-commit"
    #[qapi(name = "commit")]
    Commit,
    /// drive mirror job type, see "drive-mirror"
    #[qapi(name = "mirror")]
    Mirror,
    /// block stream job type, see "block-stream"
    #[qapi(name = "stream")]
    Stream,
}

/// Represents command verbs that can be applied to a blockjob.
///
#[qapi(added = "2.12.0")]
#[qapi(removed = "3.0.0")]
pub enum BlockJobVerb {
    /// see @block-job-cancel
    #[qapi(name = "cancel")]
    Cancel,
    /// see @block-job-complete
    #[qapi(name = "complete")]
    Complete,
    /// see @block-job-dismiss
    #[qapi(name = "dismiss")]
    Dismiss,
    /// see @block-job-finalize
    #[qapi(name = "finalize")]
    Finalize,
    /// see @block-job-pause
    #[qapi(name = "pause")]
    Pause,
    /// see @block-job-resume
    #[qapi(name = "resume")]
    Resume,
    /// see @block-job-set-speed
    #[qapi(name = "set-speed")]
    SetSpeed,
}

/// Block latency histogram.
///
#[qapi(added = "2.12.0")]
pub struct BlockLatencyHistogramInfo {
    /// list of io request counts corresponding to histogram intervals.
    /// len(@bins) = len(@boundaries) + 1
    /// For the example above, @bins may be something like [3, 1, 5, 2],
    /// and corresponding histogram looks like:
    ///
    /// ::
    ///
    /// 5|           *
    /// 4|           *
    /// 3| *         *
    /// 2| *         *    *
    /// 1| *    *    *    *
    /// +------------------
    /// 10   50   100
    pub bins: Vec<u64>,
    /// list of interval boundary values in nanoseconds, all greater
    /// than zero and in ascending order.
    /// For example, the list [10, 50, 100] produces the following
    /// histogram intervals: [0, 10), [10, 50), [50, 100), [100, +inf).
    pub boundaries: Vec<u64>,
}

#[qapi(added = "10.2.0")]
pub struct BlockLimitsInfo {
    #[qapi(name = "discard-alignment")]
    pub discard_alignment: Option<u32>,
    #[qapi(name = "max-discard")]
    pub max_discard: Option<u64>,
    #[qapi(name = "max-hw-iov")]
    pub max_hw_iov: Option<i64>,
    #[qapi(name = "max-hw-transfer")]
    pub max_hw_transfer: Option<u32>,
    #[qapi(name = "max-iov")]
    pub max_iov: i64,
    #[qapi(name = "max-transfer")]
    pub max_transfer: Option<u32>,
    #[qapi(name = "max-write-zeroes")]
    pub max_write_zeroes: Option<u64>,
    #[qapi(name = "min-mem-alignment")]
    pub min_mem_alignment: u64,
    #[qapi(name = "opt-mem-alignment")]
    pub opt_mem_alignment: u64,
    #[qapi(name = "opt-transfer")]
    pub opt_transfer: Option<u32>,
    #[qapi(name = "request-alignment")]
    pub request_alignment: u32,
    #[qapi(name = "write-zeroes-alignment")]
    pub write_zeroes_alignment: Option<u32>,
}

/// Image file size calculation information.  This structure describes
/// the size requirements for creating a new image file.
///
/// The size requirements depend on the new image file format.  File
/// size always equals virtual disk size for the 'raw' format, even for
/// sparse POSIX files.  Compact formats such as 'qcow2' represent
/// unallocated and zero regions efficiently so file size may be smaller
/// than virtual disk size.
///
/// The values are upper bounds that are guaranteed to fit the new image
/// file.  Subsequent modification, such as internal snapshot or further
/// bitmap creation, may require additional space and is not covered
/// here.
///
#[qapi(added = "2.10.0")]
pub struct BlockMeasureInfo {
    /// Additional size required if all the top-level bitmap
    /// metadata in the source image were to be copied to the
    /// destination, present only when source and destination both
    /// support persistent bitmaps.  (since 5.1)
    #[qapi(added = "5.1.0")]
    pub bitmaps: Option<i64>,
    /// Image file size, in bytes, once data has been
    /// written to all sectors, when copying just guest-visible
    /// contents.
    #[qapi(name = "fully-allocated")]
    pub fully_allocated: i64,
    /// Size required for a new image file, in bytes, when
    /// copying just allocated guest-visible contents.
    pub required: i64,
}

/// Information about a QEMU image file
///
#[qapi(added = "8.0.0")]
pub struct BlockNodeInfo {
    /// actual size on disk in bytes of the image
    #[qapi(name = "actual-size")]
    pub actual_size: Option<i64>,
    /// name of the backing file
    #[qapi(name = "backing-filename")]
    pub backing_filename: Option<String>,
    /// the format of the backing file
    #[qapi(name = "backing-filename-format")]
    pub backing_filename_format: Option<String>,
    /// size of a cluster in bytes
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<i64>,
    /// true if the image is compressed (Since 1.7)
    pub compressed: Option<bool>,
    /// true if image is not cleanly closed
    #[qapi(name = "dirty-flag")]
    pub dirty_flag: Option<bool>,
    /// true if the image is encrypted
    pub encrypted: Option<bool>,
    /// name of the image file
    pub filename: String,
    /// format of the image file
    pub format: String,
    /// structure supplying additional format-specific
    /// information (since 1.7)
    #[qapi(name = "format-specific")]
    pub format_specific: Option<ImageInfoSpecific>,
    /// full path of the backing file
    #[qapi(name = "full-backing-filename")]
    pub full_backing_filename: Option<String>,
    /// block limits that are used for I/O on the node (Since 10.2)
    #[qapi(added = "10.2.0")]
    pub limits: Option<BlockLimitsInfo>,
    /// list of VM snapshots
    pub snapshots: Option<Vec<SnapshotInfo>>,
    /// maximum capacity in bytes of the image
    #[qapi(name = "virtual-size")]
    pub virtual_size: i64,
}

/// Enum of base block permissions.
///
#[qapi(added = "4.0.0")]
pub enum BlockPermission {
    /// A user that has the "permission" of consistent
    /// reads is guaranteed that their view of the contents of the block
    /// device is complete and self-consistent, representing the
    /// contents of a disk at a specific point.  For most block devices
    /// (including their backing files) this is true, but the property
    /// cannot be maintained in a few situations like for intermediate
    /// nodes of a commit block job.
    #[qapi(name = "consistent-read")]
    ConsistentRead,
    /// This permission is required to change the node that this
    /// BdrvChild points to.
    #[qapi(name = "graph-mod")]
    #[qapi(removed = "7.0.0")]
    GraphMod,
    /// This permission is required to change the size of a block
    /// node.
    #[qapi(name = "resize")]
    Resize,
    /// This permission is required to change the visible disk
    /// contents.
    #[qapi(name = "write")]
    Write,
    /// This permission (which is weaker than
    /// BLK_PERM_WRITE) is both enough and required for writes to the
    /// block node when the caller promises that the visible disk
    /// content doesn't change.  As the BLK_PERM_WRITE permission is
    /// strictly stronger, either is sufficient to perform an unchanging
    /// write.
    #[qapi(name = "write-unchanged")]
    WriteUnchanged,
}

/// Statistics of a virtual block device or a block backing device.
///
#[qapi(added = "1.0.0")]
pub struct BlockStats {
    /// This describes the backing block device if it has one.
    /// (Since 2.0)
    #[qapi(added = "2.0.0")]
    pub backing: Option<Box<BlockStats>>,
    /// If the stats are for a virtual block device, the name
    /// corresponding to the virtual block device.
    pub device: Option<String>,
    /// Optional driver-specific stats.  (Since 4.2)
    #[qapi(name = "driver-specific")]
    #[qapi(added = "4.2.0")]
    pub driver_specific: Option<BlockStatsSpecific>,
    /// The node name of the device.  (Since 2.3)
    #[qapi(name = "node-name")]
    #[qapi(added = "2.3.0")]
    pub node_name: Option<String>,
    /// This describes the file block device if it has one.
    /// Contains recursively the statistics of the underlying protocol
    /// (e.g. the host file for a qcow2 image).  If there is no
    /// underlying protocol, this field is omitted
    pub parent: Option<Box<BlockStats>>,
    /// The qdev ID, or if no ID is assigned, the QOM path of the
    /// block device.  (since 3.0)
    #[qapi(added = "3.0.0")]
    pub qdev: Option<String>,
    /// A `BlockDeviceStats` for the device.
    pub stats: BlockDeviceStats,
}

/// Block driver specific statistics
///
#[qapi(added = "4.2.0")]
pub struct BlockStatsSpecific {
    /// block driver name
    #[qapi(discriminator)]
    pub driver: BlockdevDriver,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<BlockStatsSpecificBranch>,
}

#[qapi(added = "4.2.0")]
pub enum BlockStatsSpecificBranch {
    #[qapi(name = "blkdebug")]
    Blkdebug,
    /// Since 3.0
    #[qapi(name = "blklogwrites")]
    Blklogwrites,
    /// Since 4.2
    #[qapi(name = "blkreplay")]
    Blkreplay,
    #[qapi(name = "blkverify")]
    Blkverify,
    #[qapi(name = "bochs")]
    Bochs,
    #[qapi(name = "cloop")]
    Cloop,
    /// Since 5.0
    #[qapi(name = "compress")]
    #[qapi(added = "5.0.0")]
    Compress,
    /// Since 6.2
    #[qapi(name = "copy-before-write")]
    #[qapi(added = "6.2.0")]
    CopyBeforeWrite,
    /// Since 3.0
    #[qapi(name = "copy-on-read")]
    CopyOnRead,
    #[qapi(name = "dmg")]
    Dmg,
    #[qapi(name = "file")]
    File(BlockStatsSpecificFile),
    #[qapi(name = "ftp")]
    Ftp,
    #[qapi(name = "ftps")]
    Ftps,
    #[qapi(name = "gluster")]
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "9.2.0"))]
    #[qapi(removed = "11.1.0")]
    Gluster,
    #[qapi(name = "host_cdrom")]
    #[qapi(condition = "defined(HAVE_HOST_BLOCK_DEVICE)")]
    HostCdrom,
    #[qapi(name = "host_device")]
    #[qapi(condition = "defined(HAVE_HOST_BLOCK_DEVICE)")]
    HostDevice(BlockStatsSpecificFile),
    #[qapi(name = "http")]
    Http,
    #[qapi(name = "https")]
    Https,
    #[qapi(name = "io_uring")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    IoUring,
    #[qapi(name = "iscsi")]
    Iscsi,
    #[qapi(name = "luks")]
    Luks,
    #[qapi(name = "nbd")]
    Nbd,
    #[qapi(name = "nfs")]
    Nfs,
    #[qapi(name = "null-aio")]
    NullAio,
    #[qapi(name = "null-co")]
    NullCo,
    /// Since 2.12
    #[qapi(name = "nvme")]
    Nvme(BlockStatsSpecificNvme),
    #[qapi(name = "nvme-io_uring")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    NvmeIoUring,
    #[qapi(name = "parallels")]
    Parallels,
    #[qapi(name = "preallocate")]
    #[qapi(added = "6.0.0")]
    Preallocate,
    #[qapi(name = "qcow")]
    Qcow,
    #[qapi(name = "qcow2")]
    Qcow2,
    #[qapi(name = "qed")]
    Qed,
    #[qapi(name = "quorum")]
    Quorum,
    #[qapi(name = "raw")]
    Raw,
    #[qapi(name = "rbd")]
    Rbd,
    #[qapi(name = "replication")]
    #[qapi(condition = "defined(CONFIG_REPLICATION)")]
    Replication,
    #[qapi(name = "sheepdog")]
    #[qapi(removed = "6.1.0")]
    Sheepdog,
    /// Since 7.0
    #[qapi(name = "snapshot-access")]
    #[qapi(added = "7.0.0")]
    SnapshotAccess,
    #[qapi(name = "ssh")]
    Ssh,
    /// Since 2.11
    #[qapi(name = "throttle")]
    Throttle,
    #[qapi(name = "vdi")]
    Vdi,
    #[qapi(name = "vhdx")]
    Vhdx,
    #[qapi(name = "virtio-blk-vfio-pci")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    VirtioBlkVfioPci,
    #[qapi(name = "virtio-blk-vhost-user")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    VirtioBlkVhostUser,
    #[qapi(name = "virtio-blk-vhost-vdpa")]
    #[qapi(condition = "CONFIG_BLKIO")]
    #[qapi(added = "7.2.0")]
    VirtioBlkVhostVdpa,
    #[qapi(name = "vmdk")]
    Vmdk,
    #[qapi(name = "vpc")]
    Vpc,
    #[qapi(name = "vvfat")]
    Vvfat,
    /// Since 2.10
    #[qapi(name = "vxhs")]
    #[qapi(removed = "5.1.0")]
    Vxhs,
}

/// File driver statistics
///
#[qapi(added = "4.2.0")]
pub struct BlockStatsSpecificFile {
    /// The number of bytes discarded by the driver.
    #[qapi(name = "discard-bytes-ok")]
    pub discard_bytes_ok: u64,
    /// The number of failed discard operations
    /// performed by the driver.
    #[qapi(name = "discard-nb-failed")]
    pub discard_nb_failed: u64,
    /// The number of successful discard operations
    /// performed by the driver.
    #[qapi(name = "discard-nb-ok")]
    pub discard_nb_ok: u64,
}

/// NVMe driver statistics
///
#[qapi(added = "5.2.0")]
pub struct BlockStatsSpecificNvme {
    /// The number of aligned accesses performed by the
    /// driver.
    #[qapi(name = "aligned-accesses")]
    pub aligned_accesses: u64,
    /// The number of completion errors.
    #[qapi(name = "completion-errors")]
    pub completion_errors: u64,
    /// The number of unaligned accesses performed by
    /// the driver.
    #[qapi(name = "unaligned-accesses")]
    pub unaligned_accesses: u64,
}

/// Schema for virtual machine boot configuration.
///
#[qapi(added = "7.1.0")]
pub struct BootConfiguration {
    /// Whether to show a boot menu
    pub menu: Option<bool>,
    /// Boot order to apply on first boot
    pub once: Option<String>,
    /// Boot order (a=floppy, c=hard disk, d=CD-ROM, n=network)
    pub order: Option<String>,
    /// Timeout before guest reboots after boot fails
    #[qapi(name = "reboot-timeout")]
    pub reboot_timeout: Option<i64>,
    /// The name of the file to be passed to the firmware as logo
    /// picture, if @menu is true.
    pub splash: Option<String>,
    /// How long to show the logo picture, in milliseconds
    #[qapi(name = "splash-time")]
    pub splash_time: Option<i64>,
    /// Whether to attempt booting from devices not included in the
    /// boot order
    pub strict: Option<bool>,
}

/// Caches a system may have.  The enumeration value here is the
/// combination of cache level and cache type.
///
#[qapi(added = "9.2.0")]
pub enum CacheLevelAndType {
    /// L1 data cache.
    #[qapi(name = "l1d")]
    L1d,
    /// L1 instruction cache.
    #[qapi(name = "l1i")]
    L1i,
    /// L2 (unified) cache.
    #[qapi(name = "l2")]
    L2,
    /// L3 (unified) cache
    #[qapi(name = "l3")]
    L3,
}

/// Properties for can-host-socketcan objects.
///
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "6.0.0")]
pub struct CanHostSocketcanProperties {
    /// object ID of the can-bus object to connect to the host
    /// interface
    pub canbus: String,
    /// interface name of the host system CAN bus to connect to
    pub r#if: String,
}

/// Configuration info for the new chardev backend.
///
#[qapi(added = "1.4.0")]
pub struct ChardevBackend {
    #[qapi(discriminator)]
    #[qapi(name = "type")]
    #[qapi(removed = "6.2.0")]
    pub r_type: Option<ChardevBackendType>,
    /// backend type
    #[qapi(discriminator)]
    #[qapi(added = "6.2.0")]
    pub r#type: Option<ChardevBackendKind>,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<ChardevBackendBranch>,
}

#[qapi(added = "1.4.0")]
pub enum ChardevBackendBranch {
    #[qapi(name = "braille")]
    #[qapi(condition = "CONFIG_BRLAPI")]
    #[qapi(added = "1.5.0")]
    Braille(ChardevCommonWrapper),
    #[qapi(name = "console")]
    #[qapi(condition = "CONFIG_WIN32")]
    #[qapi(added = "1.5.0")]
    Console(ChardevCommonWrapper),
    #[qapi(name = "dbus")]
    #[qapi(condition = "CONFIG_DBUS_DISPLAY")]
    #[qapi(added = "7.0.0")]
    Dbus(ChardevDBusWrapper),
    #[qapi(name = "file")]
    File(ChardevFileWrapper),
    #[qapi(name = "hub")]
    #[qapi(added = "10.0.0")]
    Hub(ChardevHubWrapper),
    #[qapi(name = "memory")]
    #[qapi(added = "1.5.0")]
    Memory(ChardevRingbufWrapper),
    #[qapi(name = "msmouse")]
    #[qapi(added = "1.5.0")]
    Msmouse(ChardevCommonWrapper),
    #[qapi(name = "mux")]
    #[qapi(added = "1.5.0")]
    Mux(ChardevMuxWrapper),
    #[qapi(name = "null")]
    Null(ChardevCommonWrapper),
    #[qapi(name = "parallel")]
    #[qapi(condition = "HAVE_CHARDEV_PARALLEL")]
    Parallel(ChardevHostdevWrapper),
    #[qapi(name = "pipe")]
    #[qapi(added = "1.5.0")]
    Pipe(ChardevHostdevWrapper),
    #[qapi(name = "pty")]
    Pty(ChardevPtyWrapper),
    #[qapi(name = "qemu-vdagent")]
    #[qapi(condition = "defined(CONFIG_SPICE_PROTOCOL)")]
    #[qapi(added = "6.1.0")]
    QemuVdagent(ChardevQemuVdAgentWrapper),
    #[qapi(name = "ringbuf")]
    #[qapi(added = "1.5.3")]
    Ringbuf(ChardevRingbufWrapper),
    #[qapi(name = "serial")]
    #[qapi(condition = "HAVE_CHARDEV_SERIAL")]
    Serial(ChardevHostdevWrapper),
    #[qapi(name = "socket")]
    Socket(ChardevSocketWrapper),
    #[qapi(name = "spiceport")]
    #[qapi(condition = "defined(CONFIG_SPICE)")]
    #[qapi(added = "1.5.0")]
    Spiceport(ChardevSpicePortWrapper),
    #[qapi(name = "spicevmc")]
    #[qapi(condition = "defined(CONFIG_SPICE)")]
    #[qapi(added = "1.5.0")]
    Spicevmc(ChardevSpiceChannelWrapper),
    #[qapi(name = "stdio")]
    #[qapi(added = "1.5.0")]
    Stdio(ChardevStdioWrapper),
    #[qapi(name = "testdev")]
    #[qapi(added = "2.2.0")]
    Testdev(ChardevCommonWrapper),
    #[qapi(name = "udp")]
    #[qapi(added = "1.5.0")]
    Udp(ChardevUdpWrapper),
    #[qapi(name = "vc")]
    #[qapi(added = "1.5.0")]
    Vc(ChardevVcWrapper),
    #[qapi(name = "wctablet")]
    #[qapi(added = "2.9.0")]
    Wctablet(ChardevCommonWrapper),
}

/// Information about a character device backend
///
#[qapi(added = "2.0.0")]
pub struct ChardevBackendInfo {
    /// The backend name
    pub name: String,
}

#[qapi(added = "6.2.0")]
pub enum ChardevBackendKind {
    /// Since 1.5
    #[qapi(name = "braille")]
    #[qapi(condition = "CONFIG_BRLAPI")]
    Braille,
    /// Since 1.5
    #[qapi(name = "console")]
    #[qapi(condition = "CONFIG_WIN32")]
    Console,
    /// Since 7.0
    #[qapi(name = "dbus")]
    #[qapi(condition = "CONFIG_DBUS_DISPLAY")]
    #[qapi(added = "7.0.0")]
    Dbus,
    #[qapi(name = "file")]
    File,
    #[qapi(name = "hub")]
    #[qapi(added = "10.0.0")]
    Hub,
    /// Since 1.5
    #[qapi(name = "memory")]
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "9.0.0"))]
    Memory,
    /// Since 1.5
    #[qapi(name = "msmouse")]
    Msmouse,
    /// Since 1.5
    #[qapi(name = "mux")]
    Mux,
    #[qapi(name = "null")]
    Null,
    #[qapi(name = "parallel")]
    #[qapi(condition = "HAVE_CHARDEV_PARALLEL")]
    Parallel,
    /// Since 1.5
    #[qapi(name = "pipe")]
    Pipe,
    #[qapi(name = "pty")]
    Pty,
    /// Since 6.1
    #[qapi(name = "qemu-vdagent")]
    #[qapi(condition = "CONFIG_SPICE_PROTOCOL")]
    QemuVdagent,
    /// Since 1.6
    #[qapi(name = "ringbuf")]
    Ringbuf,
    #[qapi(name = "serial")]
    #[qapi(condition = "HAVE_CHARDEV_SERIAL")]
    Serial,
    #[qapi(name = "socket")]
    Socket,
    /// Since 1.5
    #[qapi(name = "spiceport")]
    #[qapi(condition = "CONFIG_SPICE")]
    Spiceport,
    /// Since 1.5
    #[qapi(name = "spicevmc")]
    #[qapi(condition = "CONFIG_SPICE")]
    Spicevmc,
    /// Since 1.5
    #[qapi(name = "stdio")]
    Stdio,
    /// Since 2.2
    #[qapi(name = "testdev")]
    Testdev,
    /// Since 1.5
    #[qapi(name = "udp")]
    Udp,
    /// v1.5
    #[qapi(name = "vc")]
    Vc,
    /// Since 2.9
    #[qapi(name = "wctablet")]
    Wctablet,
}

#[qapi(added = "1.4.0")]
#[qapi(removed = "6.2.0")]
pub enum ChardevBackendType {
    #[qapi(name = "braille")]
    #[qapi(added = "1.5.0")]
    Braille,
    #[qapi(name = "console")]
    #[qapi(added = "1.5.0")]
    Console,
    #[qapi(name = "file")]
    File,
    #[qapi(name = "memory")]
    #[qapi(added = "1.5.0")]
    Memory,
    #[qapi(name = "msmouse")]
    #[qapi(added = "1.5.0")]
    Msmouse,
    #[qapi(name = "mux")]
    #[qapi(added = "1.5.0")]
    Mux,
    #[qapi(name = "null")]
    Null,
    #[qapi(name = "parallel")]
    Parallel,
    #[qapi(name = "pipe")]
    #[qapi(added = "1.5.0")]
    Pipe,
    #[qapi(name = "pty")]
    Pty,
    #[qapi(name = "qemu-vdagent")]
    #[qapi(added = "6.1.0")]
    QemuVdagent,
    #[qapi(name = "ringbuf")]
    #[qapi(added = "1.5.3")]
    Ringbuf,
    #[qapi(name = "serial")]
    Serial,
    #[qapi(name = "socket")]
    Socket,
    #[qapi(name = "spiceport")]
    #[qapi(added = "1.5.0")]
    Spiceport,
    #[qapi(name = "spicevmc")]
    #[qapi(added = "1.5.0")]
    Spicevmc,
    #[qapi(name = "stdio")]
    #[qapi(added = "1.5.0")]
    Stdio,
    #[qapi(name = "testdev")]
    #[qapi(added = "2.2.0")]
    Testdev,
    #[qapi(name = "udp")]
    #[qapi(added = "1.5.0")]
    Udp,
    #[qapi(name = "vc")]
    #[qapi(added = "1.5.0")]
    Vc,
    #[qapi(name = "wctablet")]
    #[qapi(added = "2.9.0")]
    Wctablet,
}

/// Configuration shared across all chardev backends
///
#[qapi(added = "2.6.0")]
pub struct ChardevCommon {
    /// true to append instead of truncate (default to false to
    /// truncate)
    pub logappend: Option<bool>,
    /// The name of a logfile to save output
    pub logfile: Option<String>,
    /// true to insert timestamps into logfile
    /// (default false) (since 11.0)
    #[qapi(added = "11.0.0")]
    pub logtimestamp: Option<bool>,
}

#[qapi(added = "6.2.0")]
pub struct ChardevCommonWrapper {
    /// Configuration shared across all chardev backends
    pub data: ChardevCommon,
}

/// Configuration info for DBus chardevs.
///
#[qapi(condition = "CONFIG_DBUS_DISPLAY")]
#[qapi(added = "7.0.0")]
pub struct ChardevDBus {
    /// character encoding the guest is expected to use
    /// (since 11.1)
    #[qapi(added = "11.1.0")]
    pub encoding: Option<ChardevVcEncoding>,
    /// true to append instead of truncate (default to false to
    /// truncate)
    pub logappend: Option<bool>,
    /// The name of a logfile to save output
    pub logfile: Option<String>,
    /// true to insert timestamps into logfile
    /// (default false) (since 11.0)
    #[qapi(added = "11.0.0")]
    pub logtimestamp: Option<bool>,
    /// name of the channel (following docs/spice-port-fqdn.txt)
    pub name: String,
}

#[qapi(condition = "CONFIG_DBUS_DISPLAY")]
#[qapi(added = "7.0.0")]
pub struct ChardevDBusWrapper {
    /// Configuration info for DBus chardevs
    pub data: ChardevDBus,
}

/// Configuration info for the new chardev backend.
///
#[qapi(added = "1.4.0")]
#[qapi(removed = "2.6.0")]
pub struct ChardevDummy {}

/// Configuration info for file chardevs.
///
#[qapi(added = "1.4.0")]
pub struct ChardevFile {
    /// Open the file in append mode (default false to truncate)
    /// (Since 2.6)
    #[qapi(added = "2.6.0")]
    pub append: Option<bool>,
    /// The name of the input file
    pub r#in: Option<String>,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(added = "2.6.0")]
    pub logappend: Option<bool>,
    /// The name of a logfile to save output
    #[qapi(added = "2.6.0")]
    pub logfile: Option<String>,
    /// true to insert timestamps into logfile
    /// (default false) (since 11.0)
    #[qapi(added = "11.0.0")]
    pub logtimestamp: Option<bool>,
    /// The name of the output file
    pub out: String,
}

#[qapi(added = "6.2.0")]
pub struct ChardevFileWrapper {
    /// Configuration info for file chardevs
    pub data: ChardevFile,
}

/// Configuration info for device and pipe chardevs.
///
#[qapi(added = "1.4.0")]
pub struct ChardevHostdev {
    /// The name of the special file for the device, i.e.
    /// /dev/ttyS0 on Unix or COM1: on Windows
    pub device: String,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(added = "2.6.0")]
    pub logappend: Option<bool>,
    /// The name of a logfile to save output
    #[qapi(added = "2.6.0")]
    pub logfile: Option<String>,
    /// true to insert timestamps into logfile
    /// (default false) (since 11.0)
    #[qapi(added = "11.0.0")]
    pub logtimestamp: Option<bool>,
}

#[qapi(added = "6.2.0")]
pub struct ChardevHostdevWrapper {
    /// Configuration info for device and pipe chardevs
    pub data: ChardevHostdev,
}

/// Configuration info for hub chardevs.
///
#[qapi(added = "10.0.0")]
pub struct ChardevHub {
    /// IDs to be added to this hub (maximum 4 devices).
    pub chardevs: Vec<String>,
    /// true to append instead of truncate (default to false to
    /// truncate)
    pub logappend: Option<bool>,
    /// The name of a logfile to save output
    pub logfile: Option<String>,
    /// true to insert timestamps into logfile
    /// (default false) (since 11.0)
    #[qapi(added = "11.0.0")]
    pub logtimestamp: Option<bool>,
}

#[qapi(added = "10.0.0")]
pub struct ChardevHubWrapper {
    /// Configuration info for hub chardevs
    pub data: ChardevHub,
}

/// Information about a character device.
///
#[qapi(added = "1.0.0")]
pub struct ChardevInfo {
    /// the filename of the character device
    pub filename: String,
    /// shows whether the frontend device attached to this
    /// backend (e.g. with the chardev=... option) is in open or closed
    /// state (since 2.1)
    ///
    /// .. note:: @filename is encoded using the QEMU command line character
    /// device encoding.  See the QEMU man page for details.
    #[qapi(name = "frontend-open")]
    #[qapi(added = "2.1.0")]
    pub frontend_open: Option<bool>,
    /// the label of the character device
    pub label: String,
}

/// Configuration info for mux chardevs.
///
#[qapi(added = "1.5.0")]
pub struct ChardevMux {
    /// name of the base chardev.
    pub chardev: String,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(added = "2.6.0")]
    pub logappend: Option<bool>,
    /// The name of a logfile to save output
    #[qapi(added = "2.6.0")]
    pub logfile: Option<String>,
    /// true to insert timestamps into logfile
    /// (default false) (since 11.0)
    #[qapi(added = "11.0.0")]
    pub logtimestamp: Option<bool>,
}

#[qapi(added = "6.2.0")]
pub struct ChardevMuxWrapper {
    /// Configuration info for mux chardevs
    pub data: ChardevMux,
}

/// Configuration info for pty implementation.
///
#[qapi(added = "9.2.0")]
pub struct ChardevPty {
    /// true to append instead of truncate (default to false to
    /// truncate)
    pub logappend: Option<bool>,
    /// The name of a logfile to save output
    pub logfile: Option<String>,
    /// true to insert timestamps into logfile
    /// (default false) (since 11.0)
    #[qapi(added = "11.0.0")]
    pub logtimestamp: Option<bool>,
    /// optional path to create a symbolic link that points to the
    /// allocated PTY
    pub path: Option<String>,
}

#[qapi(added = "9.2.0")]
pub struct ChardevPtyWrapper {
    /// Configuration info for pty chardevs
    pub data: ChardevPty,
}

/// Configuration info for QEMU vdagent implementation.
///
#[qapi(name = "ChardevQemuVDAgent")]
#[qapi(condition = "defined(CONFIG_SPICE_PROTOCOL)")]
#[qapi(added = "6.1.0")]
pub struct ChardevQemuVdAgent {
    /// enable/disable clipboard, default is disabled.
    pub clipboard: Option<bool>,
    /// true to append instead of truncate (default to false to
    /// truncate)
    pub logappend: Option<bool>,
    /// The name of a logfile to save output
    pub logfile: Option<String>,
    /// true to insert timestamps into logfile
    /// (default false) (since 11.0)
    #[qapi(added = "11.0.0")]
    pub logtimestamp: Option<bool>,
    /// enable/disable mouse, default is enabled.
    pub mouse: Option<bool>,
}

#[qapi(name = "ChardevQemuVDAgentWrapper")]
#[qapi(condition = "CONFIG_SPICE_PROTOCOL")]
#[qapi(added = "6.2.0")]
pub struct ChardevQemuVdAgentWrapper {
    /// Configuration info for QEMU vdagent implementation
    pub data: ChardevQemuVdAgent,
}

/// Return info about the chardev backend just created.
///
#[qapi(added = "1.4.0")]
pub struct ChardevReturn {
    /// name of the slave pseudoterminal device, present if and only
    /// if a chardev of type 'pty' was created
    pub pty: Option<String>,
}

/// Configuration info for ring buffer chardevs.
///
#[qapi(added = "1.5.0")]
pub struct ChardevRingbuf {
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(added = "2.6.0")]
    pub logappend: Option<bool>,
    /// The name of a logfile to save output
    #[qapi(added = "2.6.0")]
    pub logfile: Option<String>,
    /// true to insert timestamps into logfile
    /// (default false) (since 11.0)
    #[qapi(added = "11.0.0")]
    pub logtimestamp: Option<bool>,
    /// ring buffer size, must be power of two, default is 65536
    pub size: Option<i64>,
}

#[qapi(added = "6.2.0")]
pub struct ChardevRingbufWrapper {
    /// Configuration info for ring buffer chardevs
    pub data: ChardevRingbuf,
}

/// Configuration info for (stream) socket chardevs.
///
#[qapi(added = "1.4.0")]
pub struct ChardevSocket {
    /// socket address to listen on (server=true) or connect to
    /// (server=false)
    pub addr: SocketAddressLegacy,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(added = "2.6.0")]
    pub logappend: Option<bool>,
    /// The name of a logfile to save output
    #[qapi(added = "2.6.0")]
    pub logfile: Option<String>,
    /// true to insert timestamps into logfile
    /// (default false) (since 11.0)
    #[qapi(added = "11.0.0")]
    pub logtimestamp: Option<bool>,
    /// set TCP_NODELAY socket option (default: false)
    pub nodelay: Option<bool>,
    /// For a client socket, if a socket is disconnected, then
    /// attempt a reconnect after the given number of seconds.  Setting
    /// this to zero disables this function.  (default: 0) (Since: 2.2)
    #[qapi(feature = "deprecated")]
    #[qapi(added = "2.2.0")]
    #[qapi(deprecated(added = "9.2.0"))]
    #[qapi(removed = "10.2.0")]
    pub reconnect: Option<i64>,
    /// For a client socket, if a socket is disconnected,
    /// then attempt a reconnect after the given number of milliseconds.
    /// Setting this to zero disables this function.
    /// (default: 0) (Since: 9.2)
    #[qapi(name = "reconnect-ms")]
    #[qapi(added = "9.2.0")]
    pub reconnect_ms: Option<i64>,
    /// create server socket (default: true)
    pub server: Option<bool>,
    /// enable telnet protocol on server sockets (default: false)
    pub telnet: Option<bool>,
    /// the ID of the QAuthZ authorization object against which
    /// the client's x509 distinguished name will be validated.  This
    /// object is only resolved at time of use, so can be deleted and
    /// recreated on the fly while the chardev server is active.  If
    /// missing, it will default to denying access (since 4.0)
    #[qapi(name = "tls-authz")]
    #[qapi(added = "4.0.0")]
    pub tls_authz: Option<String>,
    /// the ID of the TLS credentials object (since 2.6)
    #[qapi(name = "tls-creds")]
    #[qapi(added = "2.6.0")]
    pub tls_creds: Option<String>,
    /// enable tn3270 protocol on server sockets (default: false)
    /// (Since: 2.10)
    #[qapi(added = "2.10.0")]
    pub tn3270: Option<bool>,
    /// wait for incoming connection on server sockets (default:
    /// false).  Silently ignored with server: false.  This use is
    /// deprecated.
    pub wait: Option<bool>,
    /// enable websocket protocol on server sockets
    /// (default: false) (Since: 3.1)
    #[qapi(added = "3.1.0")]
    pub websocket: Option<bool>,
}

#[qapi(added = "6.2.0")]
pub struct ChardevSocketWrapper {
    /// Configuration info for (stream) socket chardevs
    pub data: ChardevSocket,
}

/// Configuration info for spice vm channel chardevs.
///
#[qapi(condition = "defined(CONFIG_SPICE)")]
#[qapi(added = "1.5.0")]
pub struct ChardevSpiceChannel {
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(added = "2.6.0")]
    pub logappend: Option<bool>,
    /// The name of a logfile to save output
    #[qapi(added = "2.6.0")]
    pub logfile: Option<String>,
    /// true to insert timestamps into logfile
    /// (default false) (since 11.0)
    #[qapi(added = "11.0.0")]
    pub logtimestamp: Option<bool>,
    /// kind of channel (for example vdagent).
    pub r#type: String,
}

#[qapi(condition = "CONFIG_SPICE")]
#[qapi(added = "6.2.0")]
pub struct ChardevSpiceChannelWrapper {
    /// Configuration info for spice vm channel chardevs
    pub data: ChardevSpiceChannel,
}

/// Configuration info for spice port chardevs.
///
#[qapi(condition = "defined(CONFIG_SPICE)")]
#[qapi(added = "1.5.0")]
pub struct ChardevSpicePort {
    /// name of the channel (see docs/spice-port-fqdn.txt)
    pub fqdn: String,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(added = "2.6.0")]
    pub logappend: Option<bool>,
    /// The name of a logfile to save output
    #[qapi(added = "2.6.0")]
    pub logfile: Option<String>,
    /// true to insert timestamps into logfile
    /// (default false) (since 11.0)
    #[qapi(added = "11.0.0")]
    pub logtimestamp: Option<bool>,
}

#[qapi(condition = "CONFIG_SPICE")]
#[qapi(added = "6.2.0")]
pub struct ChardevSpicePortWrapper {
    /// Configuration info for spice port chardevs
    pub data: ChardevSpicePort,
}

/// Configuration info for stdio chardevs.
///
#[qapi(added = "1.5.0")]
pub struct ChardevStdio {
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(added = "2.6.0")]
    pub logappend: Option<bool>,
    /// The name of a logfile to save output
    #[qapi(added = "2.6.0")]
    pub logfile: Option<String>,
    /// true to insert timestamps into logfile
    /// (default false) (since 11.0)
    #[qapi(added = "11.0.0")]
    pub logtimestamp: Option<bool>,
    /// Allow signals (such as SIGINT triggered by ^C) be delivered
    /// to QEMU.  Default: true.
    pub signal: Option<bool>,
}

#[qapi(added = "6.2.0")]
pub struct ChardevStdioWrapper {
    /// Configuration info for stdio chardevs
    pub data: ChardevStdio,
}

/// Configuration info for datagram socket chardevs.
///
#[qapi(added = "1.5.0")]
pub struct ChardevUdp {
    /// local address
    pub local: Option<SocketAddressLegacy>,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(added = "2.6.0")]
    pub logappend: Option<bool>,
    /// The name of a logfile to save output
    #[qapi(added = "2.6.0")]
    pub logfile: Option<String>,
    /// true to insert timestamps into logfile
    /// (default false) (since 11.0)
    #[qapi(added = "11.0.0")]
    pub logtimestamp: Option<bool>,
    /// remote address
    pub remote: SocketAddressLegacy,
}

#[qapi(added = "6.2.0")]
pub struct ChardevUdpWrapper {
    /// Configuration info for datagram socket chardevs
    pub data: ChardevUdp,
}

/// Configuration info for virtual console chardevs.
///
#[qapi(name = "ChardevVC")]
#[qapi(added = "1.5.0")]
pub struct ChardevVc {
    /// console width, in chars
    pub cols: Option<i64>,
    /// character encoding the guest is expected to use
    /// (since 11.1)
    ///
    /// .. note:: The options are only effective when the VNC or SDL
    /// graphical display backend is active.  They are ignored with the
    /// GTK, Spice, VNC and D-Bus display backends.
    #[qapi(added = "11.1.0")]
    pub encoding: Option<ChardevVcEncoding>,
    /// console height, in pixels
    pub height: Option<i64>,
    /// true to append instead of truncate (default to false to
    /// truncate)
    #[qapi(added = "2.6.0")]
    pub logappend: Option<bool>,
    /// The name of a logfile to save output
    #[qapi(added = "2.6.0")]
    pub logfile: Option<String>,
    /// true to insert timestamps into logfile
    /// (default false) (since 11.0)
    #[qapi(added = "11.0.0")]
    pub logtimestamp: Option<bool>,
    /// console height, in chars
    pub rows: Option<i64>,
    /// console width, in pixels
    pub width: Option<i64>,
}

/// Character encoding expected from the guest on a virtual console.
///
#[qapi(name = "ChardevVCEncoding")]
#[qapi(added = "11.1.0")]
pub enum ChardevVcEncoding {
    /// CP437 (8-bit Extended ASCII / VGA).  Every byte maps
    /// directly to a glyph; suitable for DOS or other guests that
    /// output raw CP437.
    #[qapi(name = "cp437")]
    Cp437,
    /// UTF-8.  Multi-byte sequences are decoded and then mapped
    /// back to CP437 glyphs for display; unmappable codepoints are
    /// shown as '?'.  Suitable for modern Linux guests.
    #[qapi(name = "utf8")]
    Utf8,
}

#[qapi(name = "ChardevVCWrapper")]
#[qapi(added = "6.2.0")]
pub struct ChardevVcWrapper {
    /// Configuration info for virtual console chardevs
    pub data: ChardevVc,
}

/// Properties for colo-compare objects.
///
#[qapi(added = "6.0.0")]
pub struct ColoCompareProperties {
    /// the maximum time to hold a packet from @primary_in
    /// for comparison with an incoming packet on @secondary_in in
    /// milliseconds (default: 3000)
    pub compare_timeout: Option<u64>,
    /// the interval at which colo-compare checks
    /// whether packets from @primary have timed out, in milliseconds
    /// (default: 3000)
    pub expired_scan_cycle: Option<u32>,
    /// name of the iothread to run in
    pub iothread: String,
    /// the maximum number of packets to keep in the queue
    /// for comparing with incoming packets from @secondary_in.  If the
    /// queue is full and additional packets are received, the
    /// additional packets are dropped.  (default: 1024)
    pub max_queue_size: Option<u32>,
    /// name of the character device backend to be used to
    /// communicate with the remote colo-frame (only for Xen COLO)
    pub notify_dev: Option<String>,
    /// name of the character device backend to use for output
    pub outdev: String,
    /// name of the character device backend to use for the
    /// primary input (incoming packets are redirected to @outdev)
    pub primary_in: String,
    /// name of the character device backend to use for
    /// secondary input (incoming packets are only compared to the input
    /// on @primary_in and then dropped)
    pub secondary_in: String,
    /// if true, vnet header support is enabled
    /// (default: false)
    pub vnet_hdr_support: Option<bool>,
}

/// The reason for a COLO exit.
///
#[qapi(name = "COLOExitReason")]
#[qapi(added = "3.1.0")]
pub enum ColoExitReason {
    /// COLO exit is due to an internal error.
    #[qapi(name = "error")]
    Error,
    /// failover has never happened.  This state does not occur in
    /// the `COLO_EXIT` event, and is only visible in the result of
    /// `query-colo-status`.
    #[qapi(name = "none")]
    None,
    /// COLO is currently handling a failover (since 4.0).
    #[qapi(name = "processing")]
    #[qapi(added = "4.0.0")]
    Processing,
    /// COLO exit is due to an external request.
    #[qapi(name = "request")]
    Request,
}

/// The message transmission between Primary side and Secondary side.
///
#[qapi(name = "COLOMessage")]
#[qapi(added = "2.8.0")]
pub enum ColoMessage {
    /// Secondary VM (SVM) is ready for checkpointing
    #[qapi(name = "checkpoint-ready")]
    CheckpointReady,
    /// SVM gets PVM's checkpoint request
    #[qapi(name = "checkpoint-reply")]
    CheckpointReply,
    /// Primary VM (PVM) tells SVM to prepare for
    /// checkpointing
    #[qapi(name = "checkpoint-request")]
    CheckpointRequest,
    /// VM's state has been loaded by SVM.
    #[qapi(name = "vmstate-loaded")]
    VmstateLoaded,
    /// VM's state has been received by SVM.
    #[qapi(name = "vmstate-received")]
    VmstateReceived,
    /// VM's state will be sent by PVM.
    #[qapi(name = "vmstate-send")]
    VmstateSend,
    /// The total size of VMstate.
    #[qapi(name = "vmstate-size")]
    VmstateSize,
}

/// The COLO current mode.
///
#[qapi(name = "COLOMode")]
#[qapi(added = "2.8.0")]
pub enum ColoMode {
    /// COLO is disabled.
    #[qapi(name = "none")]
    #[qapi(added = "3.1.0")]
    None,
    /// COLO node in primary side.
    #[qapi(name = "primary")]
    Primary,
    /// COLO node in slave side.
    #[qapi(name = "secondary")]
    Secondary,
    /// unknown mode
    #[qapi(name = "unknown")]
    #[qapi(removed = "3.1.0")]
    Unknown,
}

/// The result format for `query-colo-status`.
///
#[qapi(name = "COLOStatus")]
#[qapi(condition = "CONFIG_REPLICATION")]
#[qapi(added = "3.1.0")]
pub struct ColoStatus {
    /// COLO last running mode.  If COLO is running, this field
    /// will return same like mode field, after failover we can use this
    /// field to get last colo mode.  (since 4.0)
    #[qapi(name = "last-mode")]
    #[qapi(added = "4.0.0")]
    pub last_mode: Option<ColoMode>,
    /// COLO running mode.  If COLO is running, this field will
    /// return 'primary' or 'secondary'.
    pub mode: ColoMode,
    /// describes the reason for the COLO exit.
    pub reason: ColoExitReason,
}

/// Reasons that caused one command to be dropped.
///
#[qapi(added = "2.12.0")]
#[qapi(removed = "4.0.0")]
pub enum CommandDropReason {
    /// the command queue is full. This can only occur when
    /// the client sends a new non-oob command before the
    /// response to the previous non-oob command has been
    /// received.
    #[qapi(name = "queue-full")]
    QueueFull,
}

/// Information about a QMP command
///
#[qapi(added = "1.0.0")]
pub struct CommandInfo {
    /// The command name
    pub name: String,
}

/// Details about a command line option, including its list of parameter
/// details
///
#[qapi(added = "1.5.0")]
pub struct CommandLineOptionInfo {
    /// option name
    pub option: String,
    /// an array of `CommandLineParameterInfo`
    pub parameters: Vec<CommandLineParameterInfo>,
}

/// Details about a single parameter of a command line option.
///
#[qapi(added = "1.5.0")]
pub struct CommandLineParameterInfo {
    /// default value string (since 2.1)
    #[qapi(added = "2.1.0")]
    pub default: Option<String>,
    /// human readable text string, not suitable for parsing.
    pub help: Option<String>,
    /// parameter name
    pub name: String,
    /// parameter `CommandLineParameterType`
    pub r#type: CommandLineParameterType,
}

/// Possible types for an option parameter.
///
#[qapi(added = "1.5.0")]
pub enum CommandLineParameterType {
    /// accepts "on" or "off"
    #[qapi(name = "boolean")]
    Boolean,
    /// accepts a number
    #[qapi(name = "number")]
    Number,
    /// accepts a number followed by an optional suffix (K)ilo,
    /// (M)ega, (G)iga, (T)era
    #[qapi(name = "size")]
    Size,
    /// accepts a character string
    #[qapi(name = "string")]
    String,
}

/// Policy for handling deprecated management interfaces.
///
/// This is intended for testing users of the management interfaces.
///
/// Limitation: covers only syntactic aspects of QMP, i.e. stuff tagged
/// with feature 'deprecated' or 'unstable'.  We may want to extend it
/// to cover semantic aspects and CLI.
///
/// Limitation: deprecated-output policy @hide is not implemented for
/// enumeration values.  They behave the same as with policy @accept.
///
#[qapi(added = "6.0.0")]
pub struct CompatPolicy {
    /// how to handle deprecated input (default 'accept')
    #[qapi(name = "deprecated-input")]
    pub deprecated_input: Option<CompatPolicyInput>,
    /// how to handle deprecated output (default
    /// 'accept')
    #[qapi(name = "deprecated-output")]
    pub deprecated_output: Option<CompatPolicyOutput>,
    /// how to handle unstable input (default 'accept')
    /// (since 6.2)
    #[qapi(name = "unstable-input")]
    #[qapi(added = "6.2.0")]
    pub unstable_input: Option<CompatPolicyInput>,
    /// how to handle unstable output (default 'accept')
    /// (since 6.2)
    #[qapi(name = "unstable-output")]
    #[qapi(added = "6.2.0")]
    pub unstable_output: Option<CompatPolicyOutput>,
}

/// Policy for handling "funny" input.
///
#[qapi(added = "6.0.0")]
pub enum CompatPolicyInput {
    /// Accept silently
    #[qapi(name = "accept")]
    Accept,
    /// abort() the process
    #[qapi(name = "crash")]
    Crash,
    /// Reject with an error
    #[qapi(name = "reject")]
    Reject,
}

/// Policy for handling "funny" output.
///
#[qapi(added = "6.0.0")]
pub enum CompatPolicyOutput {
    /// Pass on unchanged
    #[qapi(name = "accept")]
    Accept,
    /// Filter out
    #[qapi(name = "hide")]
    Hide,
}

/// Property default values specific to a machine type, for use by
/// scripts/compare-machine-types.
///
#[qapi(added = "9.1.0")]
pub struct CompatProperty {
    /// name of its property to which the default applies
    pub property: String,
    /// name of the QOM type to which the default applies
    #[qapi(name = "qom-type")]
    pub qom_type: String,
    /// the default value (machine-specific default can overwrite
    /// the "default" default, to avoid this use -machine none)
    pub value: String,
}

/// Detailed migration compression statistics
///
#[qapi(added = "3.1.0")]
pub struct CompressionStats {
    /// count of times that no free thread was available to compress
    /// data
    pub busy: i64,
    /// rate of thread busy
    #[qapi(name = "busy-rate")]
    pub busy_rate: f64,
    /// amount of bytes after compression
    #[qapi(name = "compressed-size")]
    pub compressed_size: i64,
    /// rate of compressed size
    #[qapi(name = "compression-rate")]
    pub compression_rate: f64,
    /// amount of pages compressed and transferred to the target VM
    pub pages: i64,
}

/// Virtual CPU definition.
///
#[qapi(
    condition = "defined(TARGET_PPC) || defined(TARGET_ARM) || defined(TARGET_I386) || defined(TARGET_S390X) || defined(TARGET_MIPS)"
)]
#[qapi(added = "1.2.0")]
pub struct CpuDefinitionInfo {
    /// Name of CPU model this model is an alias for.  The target
    /// of the CPU model alias may change depending on the machine type.
    /// Management software is supposed to translate CPU model aliases
    /// in the VM configuration, because aliases may stop being
    /// migration-safe in the future (since 4.1)
    #[qapi(name = "alias-of")]
    #[qapi(added = "4.1.0")]
    pub alias_of: Option<String>,
    /// If true, this CPU model is deprecated and may be
    /// removed in some future version of QEMU according to the QEMU
    /// deprecation policy.  (since 5.2)
    #[qapi(added = "5.2.0")]
    pub deprecated: Option<bool>,
    /// whether a CPU definition can be safely used for
    /// migration in combination with a QEMU compatibility machine when
    /// migrating between different QEMU versions and between hosts with
    /// different sets of (hardware or software) capabilities.  If not
    /// provided, information is not available and callers should not
    /// assume the CPU definition to be migration-safe.  (since 2.8)
    #[qapi(name = "migration-safe")]
    #[qapi(added = "2.8.0")]
    pub migration_safe: Option<bool>,
    /// the name of the CPU definition
    pub name: String,
    /// whether a CPU definition is static and will not change
    /// depending on QEMU version, machine type, machine options and
    /// accelerator options.  A static model is always migration-safe.
    /// (since 2.8)
    #[qapi(added = "2.8.0")]
    pub r#static: Option<bool>,
    /// Type name that can be used as argument to
    /// `device-list-properties`, to introspect properties configurable
    /// using -cpu or -global.  (since 2.9)
    #[qapi(added = "2.9.0")]
    pub typename: Option<String>,
    /// is an empty list, the CPU model is runnable
    /// using the current host and machine-type.  If @unavailable-features
    /// is not present, runnability information for the CPU is not
    /// available.
    #[qapi(name = "unavailable-features")]
    #[qapi(added = "2.8.0")]
    pub unavailable_features: Option<Vec<String>>,
}

#[qapi(added = "1.0.0")]
#[qapi(removed = "6.0.0")]
pub struct CpuInfo {
    #[qapi(discriminator)]
    #[qapi(added = "2.6.0")]
    pub arch: Option<CpuInfoArch>,
    #[qapi(name = "CPU")]
    pub cpu: i64,
    pub current: bool,
    pub halted: bool,
    #[qapi(removed = "2.6.0")]
    pub nip: Option<i64>,
    #[qapi(removed = "2.6.0")]
    pub npc: Option<i64>,
    #[qapi(name = "PC")]
    #[qapi(removed = "2.6.0")]
    pub pc: Option<i64>,
    #[qapi(name = "PC")]
    #[qapi(removed = "1.1.0")]
    pub pc_alt: Option<i64>,
    #[qapi(added = "2.10.0")]
    pub props: Option<CpuInstanceProperties>,
    #[qapi(added = "2.4.0")]
    pub qom_path: Option<String>,
    pub thread_id: i64,
    #[qapi(union)]
    #[qapi(flatten)]
    #[qapi(added = "2.6.0")]
    pub u: Option<CpuInfoBranch>,
}

/// An enumeration of cpu types that enable additional information during
#[qapi(added = "2.6.0")]
#[qapi(removed = "6.0.0")]
pub enum CpuInfoArch {
    #[qapi(name = "mips")]
    Mips,
    #[qapi(name = "other")]
    Other,
    #[qapi(name = "ppc")]
    Ppc,
    /// since 2.12
    #[qapi(name = "riscv")]
    #[qapi(added = "2.12.0")]
    Riscv,
    /// since 2.12
    #[qapi(name = "s390")]
    #[qapi(added = "2.12.0")]
    S390,
    #[qapi(name = "sparc")]
    Sparc,
    #[qapi(name = "tricore")]
    Tricore,
    #[qapi(name = "x86")]
    X86,
}

#[qapi(added = "2.6.0")]
#[qapi(removed = "6.0.0")]
pub enum CpuInfoBranch {
    #[qapi(name = "mips")]
    Mips(CpuInfoMips),
    #[qapi(name = "other")]
    Other,
    #[qapi(name = "ppc")]
    Ppc(CpuInfoPpc),
    #[qapi(name = "riscv")]
    #[qapi(added = "2.12.0")]
    Riscv(CpuInfoRiscv),
    #[qapi(name = "s390")]
    #[qapi(added = "2.12.0")]
    S390(CpuInfoS390),
    #[qapi(name = "sparc")]
    Sparc(CpuInfoSparc),
    #[qapi(name = "tricore")]
    Tricore(CpuInfoTricore),
    #[qapi(name = "x86")]
    X86(CpuInfoX86),
}

/// Information about a virtual CPU
///
#[qapi(added = "2.12.0")]
pub struct CpuInfoFast {
    /// base architecture of the cpu; deprecated since 3.0.0 in favor
    /// of @target
    #[qapi(discriminator)]
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "5.0.0"))]
    #[qapi(removed = "6.0.0")]
    pub arch: Option<CpuInfoArch>,
    /// index of the virtual CPU
    #[qapi(name = "cpu-index")]
    pub cpu_index: i64,
    /// properties associated with a virtual CPU, e.g. the socket id
    pub props: Option<CpuInstanceProperties>,
    /// path to the CPU object in the QOM tree
    #[qapi(name = "qom-path")]
    pub qom_path: String,
    /// QOM type name of the CPU (since 10.1)
    #[qapi(name = "qom-type")]
    #[qapi(added = "10.1.0")]
    pub qom_type: Option<String>,
    /// the QEMU system emulation target, which determines which
    /// additional fields will be listed (since 3.0)
    #[qapi(discriminator)]
    #[qapi(added = "3.0.0")]
    pub target: Option<SysEmuTarget>,
    /// ID of the underlying host thread
    #[qapi(name = "thread-id")]
    pub thread_id: i64,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<CpuInfoFastBranch>,
}

#[qapi(added = "2.12.0")]
pub enum CpuInfoFastBranch {
    #[qapi(name = "aarch64")]
    #[qapi(added = "3.0.0")]
    Aarch64,
    #[qapi(name = "alpha")]
    #[qapi(added = "3.0.0")]
    Alpha,
    #[qapi(name = "arm")]
    #[qapi(added = "3.0.0")]
    Arm,
    /// since 5.1
    #[qapi(name = "avr")]
    #[qapi(added = "5.1.0")]
    Avr,
    #[qapi(name = "cris")]
    #[qapi(added = "3.0.0")]
    #[qapi(removed = "9.2.0")]
    Cris,
    /// since 11.0
    ///
    /// .. note:: The resulting QMP strings can be appended to the
    /// "qemu-system-" prefix to produce the corresponding QEMU
    /// executable name.  This is true even for "qemu-system-x86_64".
    #[qapi(name = "hexagon")]
    #[qapi(added = "11.0.0")]
    Hexagon,
    #[qapi(name = "hppa")]
    #[qapi(added = "3.0.0")]
    Hppa,
    #[qapi(name = "i386")]
    #[qapi(added = "3.0.0")]
    I386,
    #[qapi(name = "lm32")]
    #[qapi(added = "3.0.0")]
    #[qapi(removed = "6.1.0")]
    Lm32,
    /// since 7.1
    #[qapi(name = "loongarch64")]
    #[qapi(added = "7.1.0")]
    Loongarch64,
    #[qapi(name = "m68k")]
    #[qapi(added = "3.0.0")]
    M68k,
    #[qapi(name = "microblaze")]
    #[qapi(added = "3.0.0")]
    Microblaze,
    #[qapi(name = "microblazeel")]
    #[qapi(added = "3.0.0")]
    #[qapi(removed = "11.0.0")]
    Microblazeel,
    #[qapi(name = "mips")]
    Mips,
    #[qapi(name = "mips64")]
    #[qapi(added = "3.0.0")]
    Mips64,
    #[qapi(name = "mips64el")]
    #[qapi(added = "3.0.0")]
    Mips64el,
    #[qapi(name = "mipsel")]
    #[qapi(added = "3.0.0")]
    Mipsel,
    #[qapi(name = "moxie")]
    #[qapi(added = "3.0.0")]
    #[qapi(removed = "6.1.0")]
    Moxie,
    #[qapi(name = "nios2")]
    #[qapi(added = "3.0.0")]
    #[qapi(removed = "9.1.0")]
    Nios2,
    #[qapi(name = "or1k")]
    #[qapi(added = "3.0.0")]
    Or1k,
    #[qapi(name = "other")]
    #[qapi(removed = "3.0.0")]
    Other(CpuInfoOther),
    #[qapi(name = "ppc")]
    Ppc,
    #[qapi(name = "ppc64")]
    #[qapi(added = "3.0.0")]
    Ppc64,
    #[qapi(name = "ppcemb")]
    #[qapi(added = "3.0.0")]
    #[qapi(removed = "3.1.0")]
    Ppcemb,
    #[qapi(name = "riscv")]
    #[qapi(removed = "3.0.0")]
    Riscv(CpuInfoOther),
    #[qapi(name = "riscv32")]
    #[qapi(added = "3.0.0")]
    Riscv32,
    #[qapi(name = "riscv64")]
    #[qapi(added = "3.0.0")]
    Riscv64,
    /// since 5.0
    #[qapi(name = "rx")]
    #[qapi(added = "5.0.0")]
    Rx,
    #[qapi(name = "s390")]
    #[qapi(removed = "3.0.0")]
    S390(CpuInfoS390),
    #[qapi(name = "s390x")]
    #[qapi(added = "3.0.0")]
    S390x(CpuInfoS390),
    #[qapi(name = "sh4")]
    #[qapi(added = "3.0.0")]
    Sh4,
    #[qapi(name = "sh4eb")]
    #[qapi(added = "3.0.0")]
    Sh4eb,
    #[qapi(name = "sparc")]
    Sparc,
    #[qapi(name = "sparc64")]
    #[qapi(added = "3.0.0")]
    Sparc64,
    #[qapi(name = "tricore")]
    Tricore,
    #[qapi(name = "unicore32")]
    #[qapi(added = "3.0.0")]
    #[qapi(removed = "6.1.0")]
    Unicore32,
    #[qapi(name = "x86")]
    #[qapi(removed = "3.0.0")]
    X86(CpuInfoOther),
    #[qapi(name = "x86_64")]
    #[qapi(added = "3.0.0")]
    X8664,
    #[qapi(name = "xtensa")]
    #[qapi(added = "3.0.0")]
    Xtensa,
    #[qapi(name = "xtensaeb")]
    #[qapi(added = "3.0.0")]
    Xtensaeb,
}

/// Additional information about a virtual MIPS CPU
///
#[qapi(name = "CpuInfoMIPS")]
#[qapi(added = "2.6.0")]
#[qapi(removed = "6.0.0")]
pub struct CpuInfoMips {
    /// the instruction pointer
    #[qapi(name = "PC")]
    pub pc: i64,
}

/// No additional information is available about the virtual CPU
///
#[qapi(added = "2.6.0")]
#[qapi(removed = "3.0.0")]
pub struct CpuInfoOther {}

/// Additional information about a virtual PPC CPU
///
#[qapi(name = "CpuInfoPPC")]
#[qapi(added = "2.6.0")]
#[qapi(removed = "6.0.0")]
pub struct CpuInfoPpc {
    /// the instruction pointer
    pub nip: i64,
}

/// Additional information about a virtual RISCV CPU
///
#[qapi(name = "CpuInfoRISCV")]
#[qapi(added = "2.12.0")]
#[qapi(removed = "6.0.0")]
pub struct CpuInfoRiscv {
    /// the instruction pointer
    pub pc: i64,
}

/// Additional information about a virtual S390 CPU
///
#[qapi(added = "2.12.0")]
pub struct CpuInfoS390 {
    /// the virtual CPU's state
    #[qapi(name = "cpu-state")]
    pub cpu_state: S390CpuState,
    /// the virtual CPU's dedication (since 8.2)
    #[qapi(added = "8.2.0")]
    pub dedicated: Option<bool>,
    /// the virtual CPU's entitlement (since 8.2)
    #[qapi(added = "8.2.0")]
    pub entitlement: Option<S390CpuEntitlement>,
}

/// Additional information about a virtual SPARC CPU
///
#[qapi(name = "CpuInfoSPARC")]
#[qapi(added = "2.6.0")]
#[qapi(removed = "6.0.0")]
pub struct CpuInfoSparc {
    /// the NPC component of the instruction pointer
    pub npc: i64,
    /// the PC component of the instruction pointer
    pub pc: i64,
}

/// Additional information about a virtual Tricore CPU
///
#[qapi(added = "2.6.0")]
#[qapi(removed = "6.0.0")]
pub struct CpuInfoTricore {
    /// the instruction pointer
    #[qapi(name = "PC")]
    pub pc: i64,
}

/// Additional information about a virtual i386 or x86_64 CPU
///
#[qapi(added = "2.6.0")]
#[qapi(removed = "6.0.0")]
pub struct CpuInfoX86 {
    /// the 64-bit instruction pointer
    pub pc: i64,
}

/// Properties identifying a CPU.
///
/// Which members are optional and which mandatory depends on the
/// architecture and board.
///
/// For s390x see :ref:`cpu-topology-s390x`.
///
/// The ids other than the node-id specify the position of the CPU
/// within the CPU topology (as defined by the machine property "smp",
/// thus see also type `SMPConfiguration`)
///
#[qapi(added = "2.7.0")]
pub struct CpuInstanceProperties {
    /// book number within parent container the CPU belongs to
    /// (since 8.2)
    #[qapi(name = "book-id")]
    #[qapi(added = "8.2.0")]
    pub book_id: Option<i64>,
    /// cluster number within the parent container the CPU
    /// belongs to (since 7.1)
    #[qapi(name = "cluster-id")]
    #[qapi(added = "7.1.0")]
    pub cluster_id: Option<i64>,
    /// core number within the parent container the CPU belongs to
    #[qapi(name = "core-id")]
    pub core_id: Option<i64>,
    /// die number within the parent container the CPU belongs to
    /// (since 4.1)
    #[qapi(name = "die-id")]
    #[qapi(added = "4.1.0")]
    pub die_id: Option<i64>,
    /// drawer number within CPU topology the CPU belongs to
    /// (since 8.2)
    #[qapi(name = "drawer-id")]
    #[qapi(added = "8.2.0")]
    pub drawer_id: Option<i64>,
    /// module number within the parent container the CPU
    /// belongs to (since 9.1)
    #[qapi(name = "module-id")]
    #[qapi(added = "9.1.0")]
    pub module_id: Option<i64>,
    /// NUMA node ID the CPU belongs to
    #[qapi(name = "node-id")]
    pub node_id: Option<i64>,
    /// socket number within parent container the CPU belongs to
    #[qapi(name = "socket-id")]
    pub socket_id: Option<i64>,
    /// thread number within the core the CPU  belongs to
    #[qapi(name = "thread-id")]
    pub thread_id: Option<i64>,
}

/// The result of a CPU model baseline.
///
#[qapi(condition = "defined(TARGET_S390X)")]
#[qapi(added = "2.8.0")]
pub struct CpuModelBaselineInfo {
    /// the baselined `CpuModelInfo`.
    pub model: CpuModelInfo,
}

/// The result of a CPU model comparison.
///
#[qapi(condition = "defined(TARGET_S390X)")]
#[qapi(added = "2.8.0")]
pub struct CpuModelCompareInfo {
    /// is a list of QOM property names that led to
    /// both CPUs not being detected as identical.  For identical models,
    /// this list is empty.  If a QOM property is read-only, that means
    /// there's no known way to make the CPU models identical.  If the
    /// special property name "type" is included, the models are by
    /// definition not identical and cannot be made identical.
    #[qapi(name = "responsible-properties")]
    pub responsible_properties: Vec<String>,
    /// The result of the compare operation.
    pub result: CpuModelCompareResult,
}

/// An enumeration of CPU model comparison results.  The result is
/// usually calculated using e.g. CPU features or CPU generations.
///
#[qapi(added = "2.8.0")]
pub enum CpuModelCompareResult {
    /// If model A is identical to model B, model A is
    /// guaranteed to run where model B runs and the other way around.
    #[qapi(name = "identical")]
    Identical,
    /// If model A is incompatible to model B, model A is not
    /// guaranteed to run where model B runs and the other way around.
    #[qapi(name = "incompatible")]
    Incompatible,
    /// If model A is a subset of model B, model A is guaranteed to
    /// run where model B runs.  There are no guarantees about the other
    /// way.
    #[qapi(name = "subset")]
    Subset,
    /// If model A is a superset of model B, model B is
    /// guaranteed to run where model A runs.  There are no guarantees
    /// about the other way.
    #[qapi(name = "superset")]
    Superset,
}

/// The result of a cpu model expansion.
///
#[qapi(condition = "defined(TARGET_S390X) || defined(TARGET_I386)")]
#[qapi(added = "2.8.0")]
pub struct CpuModelExpansionInfo {
    /// an optional list of properties that are flagged
    /// as deprecated by the CPU vendor.  The list depends on the
    /// `CpuModelExpansionType`: "static" properties are a subset of the
    /// enabled-properties for the expanded model; "full" properties are
    /// a set of properties that are deprecated across all models for
    /// the architecture.  (since: 10.1 -- since 9.1 on s390x --).
    #[qapi(name = "deprecated-props")]
    #[qapi(condition = "TARGET_S390X")]
    #[qapi(added = "9.1.0")]
    pub deprecated_props: Option<Vec<String>>,
    /// the expanded `CpuModelInfo`.
    pub model: CpuModelInfo,
}

/// An enumeration of CPU model expansion types.
///
#[qapi(added = "2.8.0")]
pub enum CpuModelExpansionType {
    /// Expand all properties.  The produced model is not guaranteed
    /// to be migration-safe, but allows tooling to get an insight and
    /// work with model details.
    ///
    /// .. note:: When a non-migration-safe CPU model is expanded in static
    /// mode, some features enabled by the CPU model may be omitted,
    /// because they can't be implemented by a static CPU model
    /// definition (e.g. cache info passthrough and PMU passthrough in
    /// x86).  If you need an accurate representation of the features
    /// enabled by a non-migration-safe CPU model, use @full.  If you
    /// need a static representation that will keep ABI compatibility
    /// even when changing QEMU version or machine-type, use @static (but
    /// keep in mind that some features may be omitted).
    #[qapi(name = "full")]
    Full,
    /// Expand to a static CPU model, a combination of a static
    /// base model name and property delta changes.  As the static base
    /// model will never change, the expanded CPU model will be the
    /// same, independent of QEMU version, machine type, machine
    /// options, and accelerator options.  Therefore, the resulting
    /// model can be used by tooling without having to specify a
    /// compatibility machine - e.g. when displaying the "host" model.
    /// The @static CPU models are migration-safe.
    #[qapi(name = "static")]
    Static,
}

/// Virtual CPU model.
///
/// A CPU model consists of the name of a CPU definition, to which delta
/// changes are applied (e.g. features added/removed).  Most magic
/// values that an architecture might require should be hidden behind
/// the name.  However, if required, architectures can expose relevant
/// properties.
///
#[qapi(added = "2.8.0")]
pub struct CpuModelInfo {
    /// the name of the CPU definition the model is based on
    pub name: String,
    /// a dictionary of QOM properties to be applied
    pub props: Option<serde_json::Value>,
}

/// The result of a CPU polarization query.
///
#[qapi(condition = "(TARGET_S390X && CONFIG_KVM)")]
#[qapi(added = "8.2.0")]
pub struct CpuPolarizationInfo {
    /// the CPU polarization
    pub polarization: S390CpuPolarization,
}

/// An enumeration of CPU entitlements that can be assumed by a virtual
/// S390 CPU
///
#[qapi(added = "8.2.0")]
#[qapi(removed = "9.2.0")]
pub enum CpuS390Entitlement {
    #[qapi(name = "auto")]
    Auto,
    #[qapi(name = "high")]
    High,
    #[qapi(name = "low")]
    Low,
    #[qapi(name = "medium")]
    Medium,
}

/// An enumeration of CPU polarization that can be assumed by a virtual
/// S390 CPU
///
#[qapi(condition = "TARGET_S390X")]
#[qapi(added = "8.2.0")]
#[qapi(removed = "9.2.0")]
pub enum CpuS390Polarization {
    #[qapi(name = "horizontal")]
    Horizontal,
    #[qapi(name = "vertical")]
    Vertical,
}

/// An enumeration of cpu states that can be assumed by a virtual S390
/// CPU
///
#[qapi(added = "2.12.0")]
#[qapi(removed = "9.2.0")]
pub enum CpuS390State {
    #[qapi(name = "check-stop")]
    CheckStop,
    #[qapi(name = "load")]
    Load,
    #[qapi(name = "operating")]
    Operating,
    #[qapi(name = "stopped")]
    Stopped,
    #[qapi(name = "uninitialized")]
    Uninitialized,
}

/// An enumeration of CPU topology levels.
///
#[qapi(added = "9.2.0")]
pub enum CpuTopologyLevel {
    /// book level.  The @books option in `SMPConfiguration` is used
    /// to configure the topology of this level.
    #[qapi(name = "book")]
    Book,
    /// cluster level.  The @clusters option in `SMPConfiguration`
    /// is used to configure the topology of this level.
    #[qapi(name = "cluster")]
    Cluster,
    /// core level.  The @cores option in `SMPConfiguration` is used
    /// to configure the topology of this level.
    #[qapi(name = "core")]
    Core,
    /// default level.  Some architectures will have default
    /// topology settings (e.g., cache topology), and this special
    /// level means following the architecture-specific settings.
    #[qapi(name = "default")]
    Default,
    /// die level.  The @dies option in `SMPConfiguration` is used to
    /// configure the topology of this level.
    #[qapi(name = "die")]
    Die,
    /// drawer level.  The @drawers option in `SMPConfiguration` is
    /// used to configure the topology of this level.
    #[qapi(name = "drawer")]
    Drawer,
    /// module level.  The @modules option in `SMPConfiguration` is
    /// used to configure the topology of this level.
    #[qapi(name = "module")]
    Module,
    /// socket level, which would also be called package level.
    /// The @sockets option in `SMPConfiguration` is used to configure
    /// the topology of this level.
    #[qapi(name = "socket")]
    Socket,
    /// thread level, which would also be called SMT level or
    /// logical processor level.  The @threads option in
    /// `SMPConfiguration` is used to configure the topology of this
    /// level.
    #[qapi(name = "thread")]
    Thread,
}

/// Properties for cryptodev-backend and cryptodev-backend-builtin
/// objects.
///
#[qapi(added = "6.0.0")]
pub struct CryptodevBackendProperties {
    /// the number of queues for the cryptodev backend.  Ignored
    /// for cryptodev-backend and must be 1 for
    /// cryptodev-backend-builtin.  (default: 1)
    pub queues: Option<u32>,
    /// limit total bytes per second (Since 8.0)
    #[qapi(name = "throttle-bps")]
    #[qapi(added = "8.0.0")]
    pub throttle_bps: Option<u64>,
    /// limit total operations per second (Since 8.0)
    #[qapi(name = "throttle-ops")]
    #[qapi(added = "8.0.0")]
    pub throttle_ops: Option<u64>,
}

/// Properties for cryptodev-vhost-user objects.
///
#[qapi(condition = "CONFIG_VHOST_CRYPTO")]
#[qapi(added = "6.0.0")]
pub struct CryptodevVhostUserProperties {
    /// the name of a Unix domain socket character device that
    /// connects to the vhost-user server
    pub chardev: String,
    /// the number of queues for the cryptodev backend.  Ignored
    /// for cryptodev-backend and must be 1 for
    /// cryptodev-backend-builtin.  (default: 1)
    pub queues: Option<u32>,
    /// limit total bytes per second (Since 8.0)
    #[qapi(name = "throttle-bps")]
    #[qapi(added = "8.0.0")]
    pub throttle_bps: Option<u64>,
    /// limit total operations per second (Since 8.0)
    #[qapi(name = "throttle-ops")]
    #[qapi(added = "8.0.0")]
    pub throttle_ops: Option<u64>,
}

/// Information describing the running machine parameters.
///
#[qapi(added = "4.0.0")]
pub struct CurrentMachineParams {
    /// true if the machine supports wake up from
    /// suspend
    #[qapi(name = "wakeup-suspend-support")]
    pub wakeup_suspend_support: bool,
}

/// Common event base for a CXL Event (CXL r3.2 8.2.10.2.1
/// Table 8-55 Common Event Record Format).
///
#[qapi(name = "CXLCommonEventBase")]
#[qapi(added = "11.0.0")]
pub struct CxlCommonEventBase {
    /// Event Record Flags.  See CXL r3.2 Table 8-55 Common Event
    /// Record Format, Event Record Flags for subfield definitions.
    pub flags: u32,
    /// ID of the device head from where the event originated.
    #[qapi(name = "head-id")]
    pub head_id: Option<u8>,
    /// Logical Device (LD) ID of LD from where the event
    /// originated.
    #[qapi(name = "ld-id")]
    pub ld_id: Option<u16>,
    /// event log to add the event to
    pub log: CxlEventLog,
    /// Maintenance operation class the device requests to
    /// initiate.
    #[qapi(name = "maint-op-class")]
    pub maint_op_class: Option<u8>,
    /// Maintenance operation subclass the device
    /// requests to initiate.
    #[qapi(name = "maint-op-subclass")]
    pub maint_op_subclass: Option<u8>,
    /// CXL type 3 device canonical QOM path
    pub path: String,
}

/// Type of CXL correctable error to inject
///
#[qapi(added = "8.0.0")]
pub enum CxlCorErrorType {
    /// Data ECC error on CXL.cache
    #[qapi(name = "cache-data-ecc")]
    CacheDataEcc,
    /// Received poison from a peer on CXL.cache.
    #[qapi(name = "cache-poison-received")]
    CachePoisonReceived,
    /// Component specific and applicable to 68 byte Flit
    /// mode only.
    #[qapi(name = "crc-threshold")]
    CrcThreshold,
    /// Data ECC error on CXL.mem
    #[qapi(name = "mem-data-ecc")]
    MemDataEcc,
    /// Received poison from a peer on CXL.mem
    #[qapi(name = "mem-poison-received")]
    MemPoisonReceived,
    /// Received error indication from the physical layer.
    #[qapi(name = "physical")]
    Physical,
    /// Retry threshold hit in the Local Retry State
    /// Machine, 68B Flits only.
    #[qapi(name = "retry-threshold")]
    RetryThreshold,
}

/// Event record for a DRAM Event (CXL r3.2 8.2.10.2.1.2).
///
#[qapi(name = "CXLDRAMEvent")]
#[qapi(added = "11.0.0")]
pub struct CxldramEvent {
    /// Bank of the memory event location.  A single bank is accessed
    /// per read or write of the memory.
    pub bank: Option<u8>,
    /// Bank group of the memory event location, incorporating
    /// a number of banks.
    #[qapi(name = "bank-group")]
    pub bank_group: Option<u8>,
    /// The channel of the memory event location.  A channel is an
    /// interface that can be independently accessed for a transaction.
    pub channel: Option<u8>,
    /// Advanced programmable corrected memory error
    /// threshold event flags.
    #[qapi(name = "cme-ev-flags")]
    pub cme_ev_flags: Option<u8>,
    /// Column address within the DRAM.
    pub column: Option<u16>,
    /// Device specific component identifier for the event.
    /// May describe a field replaceable sub-component of the device.
    #[qapi(name = "component-id")]
    pub component_id: Option<String>,
    /// Bits within each nibble.  Used in order of bits
    /// set in the nibble-mask.  Up to 4 nibbles may be covered.
    #[qapi(name = "correction-mask")]
    pub correction_mask: Option<Vec<u64>>,
    /// Corrected volatile memory error count at event.
    #[qapi(name = "cvme-count")]
    pub cvme_count: Option<u32>,
    /// Memory Event Descriptor with additional memory event
    /// information.  See CXL r3.2 Table 8-58 DRAM Event Record, Memory
    /// Event Descriptor for bit definitions.
    pub descriptor: u8,
    /// Device Physical Address (relative to @path device).  Note
    /// lower bits include some flags.  See CXL r3.2 Table 8-58 DRAM
    /// Event Record, Physical Address.
    pub dpa: u64,
    /// Event Record Flags.  See CXL r3.2 Table 8-55 Common Event
    /// Record Format, Event Record Flags for subfield definitions.
    pub flags: u32,
    /// ID of the device head from where the event originated.
    #[qapi(name = "head-id")]
    pub head_id: Option<u8>,
    /// This flag specifies whether the device-specific
    /// component identifier format follows PLDM.
    #[qapi(name = "is-comp-id-pldm")]
    pub is_comp_id_pldm: Option<bool>,
    /// Logical Device (LD) ID of LD from where the event
    /// originated.
    #[qapi(name = "ld-id")]
    pub ld_id: Option<u16>,
    /// event log to add the event to
    pub log: CxlEventLog,
    /// Maintenance operation class the device requests to
    /// initiate.
    #[qapi(name = "maint-op-class")]
    pub maint_op_class: Option<u8>,
    /// Maintenance operation subclass the device
    /// requests to initiate.
    #[qapi(name = "maint-op-subclass")]
    pub maint_op_subclass: Option<u8>,
    /// Identifies one or more nibbles that the error affects
    #[qapi(name = "nibble-mask")]
    pub nibble_mask: Option<u32>,
    /// CXL type 3 device canonical QOM path
    pub path: String,
    /// The rank of the memory event location.  A rank is a set of
    /// memory devices on a channel that together execute a transaction.
    pub rank: Option<u8>,
    /// Row address within the DRAM.
    pub row: Option<u32>,
    /// The sub-channel of the memory event location.
    #[qapi(name = "sub-channel")]
    pub sub_channel: Option<u8>,
    /// Memory event sub-type.
    #[qapi(name = "sub-type")]
    pub sub_type: u8,
    /// Type of first transaction that caused the event
    /// to occur.  See CXL r3.2 Table 8-58 DRAM Event Record,
    /// Transaction Type for possible values.
    #[qapi(name = "transaction-type")]
    pub transaction_type: u8,
    /// Type of memory event that occurred.  See CXL r3.2 Table 8-58
    /// DRAM Event Record, Memory Event Type for possible values.
    pub r#type: u8,
}

/// A single dynamic capacity extent.  This is a contiguous allocation
/// of memory by Device Physical Address within a single Dynamic
/// Capacity Region on a CXL Type 3 Device.
///
#[qapi(added = "9.1.0")]
pub struct CxlDynamicCapacityExtent {
    /// The length of the extent in bytes.
    pub len: u64,
    /// The offset (in bytes) to the start of the region where the
    /// extent belongs to.
    pub offset: u64,
}

/// CXL has a number of separate event logs for different types of
/// events.  Each such event log is handled and signaled independently.
///
#[qapi(added = "8.1.0")]
pub enum CxlEventLog {
    /// Failure Event Log
    #[qapi(name = "failure")]
    Failure,
    /// Fatal Event Log
    #[qapi(name = "fatal")]
    Fatal,
    /// Information Event Log
    #[qapi(name = "informational")]
    Informational,
    /// Warning Event Log
    #[qapi(name = "warning")]
    Warning,
}

/// The policy to use for selecting which extents comprise the released
/// capacity, defined in the "Flags" field in Compute Express Link (CXL)
/// Specification, Revision 3.1, Table 7-71.
///
#[qapi(added = "9.1.0")]
pub enum CxlExtentRemovalPolicy {
    /// Extent list of capacity to release is included in
    /// the request payload.
    #[qapi(name = "prescriptive")]
    Prescriptive,
    /// Extents are selected by the device based on tag, with
    /// no requirement for contiguous extents.
    #[qapi(name = "tag-based")]
    TagBased,
}

/// The policy to use for selecting which extents comprise the added
/// capacity, as defined in Compute Express Link (CXL) Specification,
/// Revision 3.1, Table 7-70.
///
#[qapi(added = "9.1.0")]
pub enum CxlExtentSelectionPolicy {
    /// Device is responsible for allocating the requested
    /// memory capacity but must do so as a single contiguous
    /// extent.
    #[qapi(name = "contiguous")]
    Contiguous,
    /// Capacity has already been allocated to a
    /// different host using free, contiguous or prescriptive policy
    /// with a known tag.  This policy then instructs the device to make
    /// the capacity with the specified tag available to an additional
    /// host.  Capacity is implicit as it matches that already
    /// associated with the tag.  Note that the extent list (and hence
    /// Device Physical Addresses) used are per host, so a device may
    /// use different representations on each host.  The ordering of the
    /// extents provided to each host is indicated to the host using per
    /// extent sequence numbers generated by the device.  Has a similar
    /// meaning for temporal sharing, but in that case there may be only
    /// one host involved.
    #[qapi(name = "enable-shared-access")]
    EnableSharedAccess,
    /// Device is responsible for allocating the requested memory
    /// capacity and is free to do this using any combination of
    /// supported extents.
    #[qapi(name = "free")]
    Free,
    /// The precise set of extents to be allocated is
    /// specified by the command.  Thus allocation is being managed
    /// by the issuer of the allocation command, not the device.
    #[qapi(name = "prescriptive")]
    Prescriptive,
}

/// Create a CXL Fixed Memory Window
///
#[qapi(name = "CXLFixedMemoryWindowOptions")]
#[qapi(added = "7.1.0")]
pub struct CxlFixedMemoryWindowOptions {
    /// Number of contiguous bytes for which
    /// accesses will go to a given interleave target.  Accepted values
    /// [256, 512, 1k, 2k, 4k, 8k, 16k]
    #[qapi(name = "interleave-granularity")]
    pub interleave_granularity: Option<u64>,
    /// Size of the Fixed Memory Window in bytes.  Must be a multiple
    /// of 256MiB.
    pub size: u64,
    /// Target root bridge IDs from -device ...,id=<ID> for each
    /// root bridge.
    pub targets: Vec<String>,
}

/// List of CXL Fixed Memory Windows.
///
#[qapi(name = "CXLFMWProperties")]
#[qapi(added = "7.1.0")]
pub struct CxlfmwProperties {
    /// List of `CXLFixedMemoryWindowOptions`
    #[qapi(name = "cxl-fmw")]
    pub cxl_fmw: Vec<CxlFixedMemoryWindowOptions>,
}

/// Event record for a General Media Event (CXL r3.2 8.2.10.2.1.1).
///
#[qapi(name = "CXLGeneralMediaEvent")]
#[qapi(added = "11.0.0")]
pub struct CxlGeneralMediaEvent {
    /// The channel of the memory event location.  A channel is an
    /// interface that can be independently accessed for a transaction.
    pub channel: Option<u8>,
    /// Corrected memory error count at event.
    #[qapi(name = "cme-count")]
    pub cme_count: Option<u32>,
    /// Advanced programmable corrected memory error
    /// threshold event flags.
    #[qapi(name = "cme-ev-flags")]
    pub cme_ev_flags: Option<u8>,
    /// Device specific component identifier for the event.
    /// May describe a field replaceable sub-component of the device.
    #[qapi(name = "component-id")]
    pub component_id: Option<String>,
    /// Memory Event Descriptor with additional memory event
    /// information.  See CXL r3.2 Table 8-57 General Media Event
    /// Record, Memory Event Descriptor for bit definitions.
    pub descriptor: u8,
    /// Bitmask that represents all devices in the rank associated
    /// with the memory event location.
    pub device: Option<u32>,
    /// Device Physical Address (relative to @path device).  Note
    /// lower bits include some flags.  See CXL r3.2 Table 8-57 General
    /// Media Event Record, Physical Address.
    pub dpa: u64,
    /// Event Record Flags.  See CXL r3.2 Table 8-55 Common Event
    /// Record Format, Event Record Flags for subfield definitions.
    pub flags: u32,
    /// ID of the device head from where the event originated.
    #[qapi(name = "head-id")]
    pub head_id: Option<u8>,
    /// This flag specifies whether the device-specific
    /// component identifier format follows PLDM.
    #[qapi(name = "is-comp-id-pldm")]
    pub is_comp_id_pldm: Option<bool>,
    /// Logical Device (LD) ID of LD from where the event
    /// originated.
    #[qapi(name = "ld-id")]
    pub ld_id: Option<u16>,
    /// event log to add the event to
    pub log: CxlEventLog,
    /// Maintenance operation class the device requests to
    /// initiate.
    #[qapi(name = "maint-op-class")]
    pub maint_op_class: Option<u8>,
    /// Maintenance operation subclass the device
    /// requests to initiate.
    #[qapi(name = "maint-op-subclass")]
    pub maint_op_subclass: Option<u8>,
    /// CXL type 3 device canonical QOM path
    pub path: String,
    /// The rank of the memory event location.  A rank is a set of
    /// memory devices on a channel that together execute a transaction.
    pub rank: Option<u8>,
    /// Memory event sub-type.
    #[qapi(name = "sub-type")]
    pub sub_type: u8,
    /// Type of first transaction that caused the event
    /// to occur.  See CXL r3-2 Table 8-57 General Media Event Record,
    /// Transaction Type for possible values.
    #[qapi(name = "transaction-type")]
    pub transaction_type: u8,
    /// Type of memory event that occurred.  See CXL r3.2 Table 8-57
    /// General Media Event Record, Memory Event Type for possible
    /// values.
    pub r#type: u8,
}

#[qapi(name = "CXLMemModuleEvent")]
#[qapi(added = "11.0.0")]
pub struct CxlMemModuleEvent {
    #[qapi(name = "additional-status")]
    pub additional_status: u8,
    #[qapi(name = "component-id")]
    pub component_id: Option<String>,
    #[qapi(name = "corrected-persistent-error-count")]
    pub corrected_persistent_error_count: u32,
    #[qapi(name = "corrected-volatile-error-count")]
    pub corrected_volatile_error_count: u32,
    #[qapi(name = "dirty-shutdown-count")]
    pub dirty_shutdown_count: u32,
    /// Event Record Flags.  See CXL r3.2 Table 8-55 Common Event
    /// Record Format, Event Record Flags for subfield definitions.
    pub flags: u32,
    /// ID of the device head from where the event originated.
    #[qapi(name = "head-id")]
    pub head_id: Option<u8>,
    #[qapi(name = "health-status")]
    pub health_status: u8,
    #[qapi(name = "is-comp-id-pldm")]
    pub is_comp_id_pldm: Option<bool>,
    /// Logical Device (LD) ID of LD from where the event
    /// originated.
    #[qapi(name = "ld-id")]
    pub ld_id: Option<u16>,
    #[qapi(name = "life-used")]
    pub life_used: u8,
    /// event log to add the event to
    pub log: CxlEventLog,
    /// Maintenance operation class the device requests to
    /// initiate.
    #[qapi(name = "maint-op-class")]
    pub maint_op_class: Option<u8>,
    /// Maintenance operation subclass the device
    /// requests to initiate.
    #[qapi(name = "maint-op-subclass")]
    pub maint_op_subclass: Option<u8>,
    #[qapi(name = "media-status")]
    pub media_status: u8,
    /// CXL type 3 device canonical QOM path
    pub path: String,
    #[qapi(name = "sub-type")]
    pub sub_type: u8,
    pub temperature: i16,
    pub r#type: u8,
}

/// Record of a single error including header log.
///
#[qapi(name = "CXLUncorErrorRecord")]
#[qapi(added = "8.0.0")]
pub struct CxlUncorErrorRecord {
    /// 16 DWORD of header.
    pub header: Vec<u32>,
    /// Type of error
    pub r#type: CxlUncorErrorType,
}

/// Type of uncorrectable CXL error to inject.  These errors are
/// reported via an AER uncorrectable internal error with additional
/// information logged at the CXL device.
///
#[qapi(added = "8.0.0")]
pub enum CxlUncorErrorType {
    /// Address parity or other errors associated
    /// with the address field on CXL.cache
    #[qapi(name = "cache-address-parity")]
    CacheAddressParity,
    /// Byte enable parity or other byte enable errors on
    /// CXL.cache
    #[qapi(name = "cache-be-parity")]
    CacheBeParity,
    /// ECC error on CXL.cache
    #[qapi(name = "cache-data-ecc")]
    CacheDataEcc,
    /// Data error such as data parity or data ECC error
    /// CXL.cache
    #[qapi(name = "cache-data-parity")]
    CacheDataParity,
    /// Integrity and data encryption rx error.
    #[qapi(name = "cxl-ide-rx")]
    CxlIdeRx,
    /// Integrity and data encryption tx error.
    #[qapi(name = "cxl-ide-tx")]
    CxlIdeTx,
    /// Component specific error
    #[qapi(name = "internal")]
    Internal,
    /// Address parity or other errors associated with
    /// the address field on CXL.mem
    #[qapi(name = "mem-address-parity")]
    MemAddressParity,
    /// Byte enable parity or other byte enable errors on
    /// CXL.mem.
    #[qapi(name = "mem-be-parity")]
    MemBeParity,
    /// Data ECC error on CXL.mem.
    #[qapi(name = "mem-data-ecc")]
    MemDataEcc,
    /// Data error such as data parity or data ECC error
    /// on CXL.mem
    #[qapi(name = "mem-data-parity")]
    MemDataParity,
    /// Received poison from the peer.
    #[qapi(name = "poison-received")]
    PoisonReceived,
    /// Buffer overflows (first 3 bits of header log
    /// indicate which)
    #[qapi(name = "receiver-overflow")]
    ReceiverOverflow,
    /// REINIT threshold hit.
    #[qapi(name = "reinit-threshold")]
    ReinitThreshold,
    /// Received unrecognized encoding.
    #[qapi(name = "rsvd-encoding")]
    RsvdEncoding,
}

/// An enumeration of data format.
///
#[qapi(added = "1.4.0")]
pub enum DataFormat {
    /// Data is Base64 encoded binary (RFC 3548)
    #[qapi(name = "base64")]
    Base64,
    /// Data is a UTF-8 string (RFC 3629)
    #[qapi(name = "utf8")]
    Utf8,
}

/// Properties for dbus-vmstate objects.
///
#[qapi(name = "DBusVMStateProperties")]
#[qapi(added = "6.0.0")]
pub struct DBusVmStateProperties {
    /// the name of the DBus bus to connect to
    pub addr: String,
    /// a comma separated list of DBus IDs of helpers whose data
    /// should be included in the VM state on migration
    #[qapi(name = "id-list")]
    pub id_list: Option<String>,
}

/// Information about device properties.
///
#[qapi(added = "1.2.0")]
#[qapi(removed = "2.12.0")]
pub struct DevicePropertyInfo {
    /// if specified, the description of the property.
    /// (since 2.2)
    #[qapi(added = "2.2.0")]
    pub description: Option<String>,
    /// the name of the property
    pub name: String,
    /// the typename of the property
    pub r#type: String,
}

/// An enumeration of possible states that a dirty bitmap can report to the user.
///
#[qapi(added = "2.4.0")]
#[qapi(removed = "6.0.0")]
pub enum DirtyBitmapStatus {
    /// The bitmap is actively monitoring for new writes, and can be cleared,
    /// deleted, or used for backup operations.
    #[qapi(name = "active")]
    Active,
    /// The bitmap is not currently recording new writes by the guest.
    /// This is requested explicitly via @block-dirty-bitmap-disable.
    /// It can still be cleared, deleted, or used for backup operations.
    #[qapi(name = "disabled")]
    Disabled,
    /// The bitmap is currently in-use by some operation and is immutable.
    /// If the bitmap was @active prior to the operation, new writes by the
    /// guest are being recorded in a temporary buffer, and will not be lost.
    /// Generally, bitmaps are cleared on successful use in an operation and
    /// the temporary buffer is committed into the bitmap. On failure, the
    /// temporary buffer is merged back into the bitmap without first
    /// clearing it.
    /// Please refer to the documentation for each bitmap-using operation,
    /// See also @blockdev-backup, @drive-backup.
    #[qapi(name = "frozen")]
    Frozen,
    /// This is a persistent dirty bitmap that was marked in-use on
    /// disk, and is unusable by QEMU. It can only be deleted.
    /// Please rely on the inconsistent field in @BlockDirtyInfo
    /// instead, as the status field is deprecated. (Since 4.0)
    #[qapi(name = "inconsistent")]
    #[qapi(added = "4.0.0")]
    Inconsistent,
    /// The bitmap is currently in-use by some operation and is immutable.
    /// If the bitmap was @active prior to the operation, it is still
    /// recording new writes. If the bitmap was @disabled, it is not
    /// recording new writes. (Since 2.12)
    #[qapi(name = "locked")]
    #[qapi(added = "2.12.0")]
    Locked,
}

/// Dirty page rate limit information of a virtual CPU.
///
#[qapi(added = "7.1.0")]
pub struct DirtyLimitInfo {
    /// index of a virtual CPU.
    #[qapi(name = "cpu-index")]
    pub cpu_index: i64,
    /// current dirty page rate (MB/s) for a virtual CPU.
    #[qapi(name = "current-rate")]
    pub current_rate: u64,
    /// upper limit of dirty page rate (MB/s) for a virtual
    /// CPU, 0 means unlimited.
    #[qapi(name = "limit-rate")]
    pub limit_rate: u64,
}

/// Information about measured dirty page rate.
///
#[qapi(added = "5.2.0")]
pub struct DirtyRateInfo {
    /// time period for which dirty page rate was measured,
    /// expressed and rounded down to @calc-time-unit.
    #[qapi(name = "calc-time")]
    pub calc_time: i64,
    /// time unit of @calc-time  (Since 8.2)
    #[qapi(name = "calc-time-unit")]
    #[qapi(added = "8.2.0")]
    pub calc_time_unit: Option<TimeUnit>,
    /// an estimate of the dirty page rate of the VM in units
    /// of MiB/s.  Value is present only when @status is 'measured'.
    #[qapi(name = "dirty-rate")]
    pub dirty_rate: Option<i64>,
    /// mode that was used to measure dirty page rate (Since 6.2)
    #[qapi(added = "6.2.0")]
    pub mode: Option<DirtyRateMeasureMode>,
    /// number of sampled pages per GiB of guest memory.
    /// Valid only in page-sampling mode (Since 6.1)
    #[qapi(name = "sample-pages")]
    #[qapi(added = "6.1.0")]
    pub sample_pages: Option<u64>,
    /// start time in units of second for calculation
    #[qapi(name = "start-time")]
    pub start_time: i64,
    /// current status of dirty page rate measurements
    pub status: DirtyRateStatus,
    /// dirty rate for each vCPU if dirty-ring mode was
    /// specified (Since 6.2)
    #[qapi(name = "vcpu-dirty-rate")]
    #[qapi(added = "6.2.0")]
    pub vcpu_dirty_rate: Option<Vec<DirtyRateVcpu>>,
}

/// Method used to measure dirty page rate.  Differences between
/// available methods are explained in `calc-dirty-rate`.
///
#[qapi(added = "6.2.0")]
pub enum DirtyRateMeasureMode {
    /// use dirty bitmap
    #[qapi(name = "dirty-bitmap")]
    DirtyBitmap,
    /// use dirty ring
    #[qapi(name = "dirty-ring")]
    DirtyRing,
    /// use page sampling
    #[qapi(name = "page-sampling")]
    PageSampling,
}

/// Dirty page rate measurement status.
///
#[qapi(added = "5.2.0")]
pub enum DirtyRateStatus {
    /// dirty page rate is measured and the results are available
    #[qapi(name = "measured")]
    Measured,
    /// measuring thread is running
    #[qapi(name = "measuring")]
    Measuring,
    /// measuring thread has not been started yet
    #[qapi(name = "unstarted")]
    Unstarted,
}

/// Dirty rate of vcpu.
///
#[qapi(added = "6.2.0")]
pub struct DirtyRateVcpu {
    /// dirty rate.
    #[qapi(name = "dirty-rate")]
    pub dirty_rate: i64,
    /// vcpu index.
    pub id: i64,
}

/// Cocoa display options.
///
#[qapi(added = "7.0.0")]
pub struct DisplayCocoa {
    /// Capture all key presses, including system combos.  This
    /// requires accessibility permissions, since it performs a global
    /// grab on key events.  (default: off)  See
    /// https://support.apple.com/en-in/guide/mac-help/mh32356/mac
    #[qapi(name = "full-grab")]
    pub full_grab: Option<bool>,
    /// Enable/disable forwarding of left command key to
    /// guest.  Allows command-tab window switching on the host without
    /// sending this key to the guest when "off".  Defaults to "on"
    #[qapi(name = "left-command-key")]
    pub left_command_key: Option<bool>,
    /// Swap the Option and Command keys so that their key
    /// codes match their position on non-Mac keyboards and you can use
    /// Meta/Super and Alt where you expect them.  (default: off)
    #[qapi(name = "swap-opt-cmd")]
    pub swap_opt_cmd: Option<bool>,
    /// Apply interpolation to smooth output when
    /// zoom-to-fit is enabled.  Defaults to "off".  (Since 9.0)
    #[qapi(name = "zoom-interpolation")]
    #[qapi(added = "9.0.0")]
    pub zoom_interpolation: Option<bool>,
    /// Zoom guest display to fit into the host window.  When
    /// turned off the host window will be resized instead.  Defaults to
    /// "off".  (Since 8.2)
    #[qapi(name = "zoom-to-fit")]
    #[qapi(added = "8.2.0")]
    pub zoom_to_fit: Option<bool>,
}

/// Curses display options.
///
#[qapi(added = "4.0.0")]
pub struct DisplayCurses {
    /// Font charset used by guest (default: CP437).
    pub charset: Option<String>,
}

/// DBus display options.
///
#[qapi(added = "7.0.0")]
pub struct DisplayDBus {
    /// The D-Bus bus address (default to the session bus).
    pub addr: Option<String>,
    /// Use the specified DBus audiodev to export audio.
    pub audiodev: Option<String>,
    /// Whether to use peer-to-peer connections (accepted through
    /// `add_client`).
    pub p2p: Option<bool>,
    /// Which DRM render node should be used.  Default is the
    /// first available node on the host.
    pub rendernode: Option<String>,
}

/// EGL headless display options.
///
#[qapi(name = "DisplayEGLHeadless")]
#[qapi(added = "3.1.0")]
pub struct DisplayEglHeadless {
    /// Which DRM render node should be used.  Default is the
    /// first available node on the host.
    pub rendernode: Option<String>,
}

/// Display OpenGL mode.
///
#[qapi(name = "DisplayGLMode")]
#[qapi(added = "3.0.0")]
pub enum DisplayGlMode {
    /// Use OpenGL with Core (desktop) Context.
    #[qapi(name = "core")]
    Core,
    /// Use OpenGL with ES (embedded systems) Context.
    #[qapi(name = "es")]
    Es,
    /// Disable OpenGL (default).
    #[qapi(name = "off")]
    Off,
    /// Use OpenGL, pick context type automatically.  Would better be
    /// named 'auto' but is called 'on' for backward compatibility with
    /// bool type.
    #[qapi(name = "on")]
    On,
}

/// GTK display options.
///
#[qapi(name = "DisplayGTK")]
#[qapi(added = "2.12.0")]
pub struct DisplayGtk {
    /// Enable host-guest clipboard sharing.  Defaults to "off".
    /// (Since 11.1)
    #[qapi(added = "11.1.0")]
    pub clipboard: Option<bool>,
    /// Grab keyboard input on mouse hover.
    #[qapi(name = "grab-on-hover")]
    pub grab_on_hover: Option<bool>,
    /// Keep width/height aspect ratio of guest content
    /// when resizing host window.  Defaults to "on".  (Since 10.1)
    #[qapi(name = "keep-aspect-ratio")]
    #[qapi(added = "10.1.0")]
    pub keep_aspect_ratio: Option<bool>,
    /// Set preferred scale of the display.  Defaults to 1.0.
    /// (Since 10.1)
    #[qapi(added = "10.1.0")]
    pub scale: Option<f64>,
    /// Display the main window menubar.  Defaults to "on".
    /// (Since 8.0)
    #[qapi(name = "show-menubar")]
    #[qapi(added = "7.2.0")]
    pub show_menubar: Option<bool>,
    /// Display the tab bar for switching between the various
    /// graphical interfaces (e.g. VGA and virtual console character
    /// devices) by default.  (Since 7.1)
    #[qapi(name = "show-tabs")]
    #[qapi(added = "7.1.0")]
    pub show_tabs: Option<bool>,
    /// Zoom guest display to fit into the host window.  When
    /// turned off the host window will be resized instead.  In case the
    /// display device can notify the guest on window resizes
    /// (virtio-gpu) this will default to "on", assuming the guest will
    /// resize the display to match the window size then.  Otherwise it
    /// defaults to "off".  (Since 3.1)
    #[qapi(name = "zoom-to-fit")]
    #[qapi(added = "3.1.0")]
    pub zoom_to_fit: Option<bool>,
}

/// Empty struct for displays without config options.
///
#[qapi(added = "2.12.0")]
#[qapi(removed = "3.0.0")]
pub struct DisplayNoOpts {}

/// Display (user interface) options.
///
#[qapi(added = "2.12.0")]
pub struct DisplayOptions {
    /// Start user interface in fullscreen mode
    /// (default: off).
    #[qapi(name = "full-screen")]
    pub full_screen: Option<bool>,
    /// Enable OpenGL support (default: off).
    pub gl: Option<DisplayGlMode>,
    /// Force showing the mouse cursor (default: off).
    /// (since: 5.0)
    #[qapi(name = "show-cursor")]
    #[qapi(added = "5.0.0")]
    pub show_cursor: Option<bool>,
    /// Which `DisplayType` QEMU should use.
    #[qapi(discriminator)]
    pub r#type: DisplayType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<DisplayOptionsBranch>,
    /// Allow to quit QEMU with window close button
    /// (default: on).
    #[qapi(name = "window-close")]
    pub window_close: Option<bool>,
}

#[qapi(added = "2.12.0")]
pub enum DisplayOptionsBranch {
    /// The Cocoa user interface.
    #[qapi(name = "cocoa")]
    #[qapi(condition = "defined(CONFIG_COCOA)")]
    Cocoa(DisplayCocoa),
    #[qapi(name = "curses")]
    #[qapi(condition = "defined(CONFIG_CURSES)")]
    Curses(DisplayCurses),
    #[qapi(name = "dbus")]
    #[qapi(condition = "CONFIG_DBUS_DISPLAY")]
    #[qapi(added = "7.0.0")]
    Dbus(DisplayDBus),
    /// The default user interface, selecting from the first
    /// available of gtk, sdl, cocoa, and vnc.
    #[qapi(name = "default")]
    Default,
    #[qapi(name = "egl-headless")]
    #[qapi(condition = "defined(CONFIG_OPENGL) && defined(CONFIG_GBM)")]
    EglHeadless(DisplayEglHeadless),
    #[qapi(name = "gtk")]
    #[qapi(condition = "defined(CONFIG_GTK)")]
    Gtk(DisplayGtk),
    /// No user interface or video output display.  The guest will
    /// still see an emulated graphics card, but its output will not be
    /// displayed to the QEMU user.
    #[qapi(name = "none")]
    None,
    /// The SDL user interface.
    #[qapi(name = "sdl")]
    #[qapi(condition = "defined(CONFIG_SDL)")]
    Sdl(DisplaySdl),
    /// Set up a Spice server and run the default associated
    /// application to connect to it.  The server will redirect the
    /// serial console and QEMU monitors.  (Since 4.0)
    #[qapi(name = "spice-app")]
    #[qapi(condition = "defined(CONFIG_SPICE)")]
    #[qapi(added = "4.0.0")]
    SpiceApp,
}

/// Display protocols which support changing password options.
///
#[qapi(added = "7.0.0")]
pub enum DisplayProtocol {
    #[qapi(name = "spice")]
    Spice,
    #[qapi(name = "vnc")]
    Vnc,
}

/// Options of the display configuration reload.
///
#[qapi(added = "6.0.0")]
pub struct DisplayReloadOptions {
    /// Specify the display type.
    #[qapi(discriminator)]
    pub r#type: DisplayReloadType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<DisplayReloadOptionsBranch>,
}

#[qapi(added = "6.0.0")]
pub enum DisplayReloadOptionsBranch {
    #[qapi(name = "vnc")]
    Vnc(DisplayReloadOptionsVnc),
}

/// Specify the VNC reload options.
///
#[qapi(name = "DisplayReloadOptionsVNC")]
#[qapi(added = "6.0.0")]
pub struct DisplayReloadOptionsVnc {
    /// reload tls certs or not.
    #[qapi(name = "tls-certs")]
    pub tls_certs: Option<bool>,
}

/// Available DisplayReload types.
///
#[qapi(added = "6.0.0")]
pub enum DisplayReloadType {
    /// VNC display
    #[qapi(name = "vnc")]
    Vnc,
}

/// SDL2 display options.
///
#[qapi(name = "DisplaySDL")]
#[qapi(added = "7.1.0")]
pub struct DisplaySdl {
    /// Modifier keys that should be pressed together with the
    /// "G" key to release the mouse grab.
    #[qapi(name = "grab-mod")]
    pub grab_mod: Option<HotKeyMod>,
}

/// Display (user interface) type.
///
#[qapi(added = "2.12.0")]
pub enum DisplayType {
    /// The Cocoa user interface.
    #[qapi(name = "cocoa")]
    #[qapi(condition = "defined(CONFIG_COCOA)")]
    Cocoa,
    /// Display video output via curses.  For graphics device
    /// models which support a text mode, QEMU can display this output
    /// using a curses/ncurses interface.  Nothing is displayed when the
    /// graphics device is in graphical mode or if the graphics device
    /// does not support a text mode.  Generally only the VGA device
    /// models support text mode.
    #[qapi(name = "curses")]
    #[qapi(condition = "defined(CONFIG_CURSES)")]
    Curses,
    /// Start a D-Bus service for the display.  (Since 7.0)
    #[qapi(name = "dbus")]
    #[qapi(condition = "CONFIG_DBUS_DISPLAY")]
    #[qapi(added = "7.0.0")]
    Dbus,
    /// The default user interface, selecting from the first
    /// available of gtk, sdl, cocoa, and vnc.
    #[qapi(name = "default")]
    Default,
    /// No user interface, offload GL operations to a local
    /// DRI device.  Graphical display need to be paired with VNC or
    /// Spice.  (Since 3.1)
    #[qapi(name = "egl-headless")]
    #[qapi(condition = "defined(CONFIG_OPENGL) && defined(CONFIG_GBM)")]
    EglHeadless,
    /// The GTK user interface.
    #[qapi(name = "gtk")]
    #[qapi(condition = "defined(CONFIG_GTK)")]
    Gtk,
    /// No user interface or video output display.  The guest will
    /// still see an emulated graphics card, but its output will not be
    /// displayed to the QEMU user.
    #[qapi(name = "none")]
    None,
    /// The SDL user interface.
    #[qapi(name = "sdl")]
    #[qapi(condition = "defined(CONFIG_SDL)")]
    Sdl,
    /// Set up a Spice server and run the default associated
    /// application to connect to it.  The server will redirect the
    /// serial console and QEMU monitors.  (Since 4.0)
    #[qapi(name = "spice-app")]
    #[qapi(condition = "defined(CONFIG_SPICE)")]
    #[qapi(added = "4.0.0")]
    SpiceApp,
}

/// Options of the display configuration reload.
///
#[qapi(added = "7.1.0")]
pub struct DisplayUpdateOptions {
    /// Specify the display type.
    #[qapi(discriminator)]
    pub r#type: DisplayUpdateType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<DisplayUpdateOptionsBranch>,
}

#[qapi(added = "7.1.0")]
pub enum DisplayUpdateOptionsBranch {
    #[qapi(name = "vnc")]
    Vnc(DisplayUpdateOptionsVnc),
}

/// Specify the VNC reload options.
///
#[qapi(name = "DisplayUpdateOptionsVNC")]
#[qapi(added = "7.1.0")]
pub struct DisplayUpdateOptionsVnc {
    /// If specified, change set of addresses to listen for
    /// connections.  Addresses configured for websockets are not
    /// touched.
    pub addresses: Option<Vec<SocketAddress>>,
}

/// Available DisplayUpdate types.
///
#[qapi(added = "7.1.0")]
pub enum DisplayUpdateType {
    /// VNC display
    #[qapi(name = "vnc")]
    Vnc,
}

#[qapi(name = "drive-backup")]
#[qapi(command)]
#[qapi(returns = "()")]
#[qapi(added = "1.6.0")]
#[qapi(allow_preconfig(added = "7.1.0"))]
pub struct DriveBackup {
    #[qapi(name = "auto-dismiss")]
    #[qapi(added = "2.12.0")]
    pub auto_dismiss: Option<bool>,
    #[qapi(name = "auto-finalize")]
    #[qapi(added = "2.12.0")]
    pub auto_finalize: Option<bool>,
    #[qapi(added = "2.4.0")]
    pub bitmap: Option<String>,
    #[qapi(name = "bitmap-mode")]
    #[qapi(added = "4.2.0")]
    pub bitmap_mode: Option<BitmapSyncMode>,
    #[qapi(added = "2.8.0")]
    pub compress: Option<bool>,
    pub device: String,
    #[qapi(name = "discard-source")]
    #[qapi(added = "9.1.0")]
    pub discard_source: Option<bool>,
    #[qapi(name = "filter-node-name")]
    #[qapi(added = "4.2.0")]
    pub filter_node_name: Option<String>,
    /// the format of the new destination, default is to probe if
    pub format: Option<String>,
    #[qapi(name = "job-id")]
    #[qapi(added = "2.7.0")]
    pub job_id: Option<String>,
    /// whether and how QEMU should create a new image, default is
    /// 'absolute-paths'.
    pub mode: Option<NewImageMode>,
    #[qapi(name = "on-cbw-error")]
    #[qapi(added = "10.1.0")]
    pub on_cbw_error: Option<OnCbwError>,
    #[qapi(name = "on-source-error")]
    pub on_source_error: Option<BlockdevOnError>,
    #[qapi(name = "on-target-error")]
    pub on_target_error: Option<BlockdevOnError>,
    pub speed: Option<i64>,
    pub sync: MirrorSyncMode,
    /// the target of the new image.  If the file exists, or if it
    /// is a device, the existing file/device will be used as the new
    /// destination.  If it does not exist, a new file will be created.
    pub target: String,
    #[qapi(name = "x-perf")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "6.0.0")]
    pub x_perf: Option<BackupPerf>,
}

#[qapi(added = "6.2.0")]
pub struct DriveBackupWrapper {
    pub data: DriveBackup,
}

/// Not used by QMP; hack to let us use BlockGraphInfoList internally
///
#[qapi(added = "8.0.0")]
pub struct DummyBlockCoreForceArrays {
    #[qapi(name = "unused-block-graph-info")]
    pub unused_block_graph_info: Vec<BlockGraphInfo>,
}

/// Not used by QMP; hack to let us use X86CPUFeatureWordInfoList
/// internally
///
#[qapi(added = "2.5.0")]
pub struct DummyForceArrays {
    pub unused: Vec<X86cpuFeatureWordInfo>,
}

/// Not used by QMP; hack to let us use IOThreadVirtQueueMappingList and
/// VirtIOGPUOutputList internally
///
#[qapi(added = "9.0.0")]
pub struct DummyVirtioForceArrays {
    #[qapi(name = "unused-iothread-vq-mapping")]
    pub unused_iothread_vq_mapping: Vec<IoThreadVirtQueueMapping>,
    #[qapi(name = "unused-virtio-gpu-output")]
    #[qapi(added = "10.1.0")]
    pub unused_virtio_gpu_output: Option<Vec<VirtIogpuOutput>>,
}

/// A list of the available formats for dump-guest-memory
///
#[qapi(added = "2.0.0")]
pub struct DumpGuestMemoryCapability {
    /// the available formats for `dump-guest-memory`
    pub formats: Vec<DumpGuestMemoryFormat>,
}

/// An enumeration of guest-memory-dump's format.
///
#[qapi(added = "2.0.0")]
pub enum DumpGuestMemoryFormat {
    /// elf format
    #[qapi(name = "elf")]
    Elf,
    /// makedumpfile flattened, kdump-compressed format with lzo
    /// compression
    #[qapi(name = "kdump-lzo")]
    KdumpLzo,
    /// raw assembled kdump-compressed format with lzo
    /// compression (since 8.2)
    #[qapi(name = "kdump-raw-lzo")]
    #[qapi(added = "8.2.0")]
    KdumpRawLzo,
    /// raw assembled kdump-compressed format with snappy
    /// compression (since 8.2)
    #[qapi(name = "kdump-raw-snappy")]
    #[qapi(added = "8.2.0")]
    KdumpRawSnappy,
    /// raw assembled kdump-compressed format with zlib
    /// compression (since 8.2)
    #[qapi(name = "kdump-raw-zlib")]
    #[qapi(added = "8.2.0")]
    KdumpRawZlib,
    /// makedumpfile flattened, kdump-compressed format with
    /// snappy compression
    #[qapi(name = "kdump-snappy")]
    KdumpSnappy,
    /// makedumpfile flattened, kdump-compressed format with
    /// zlib compression
    #[qapi(name = "kdump-zlib")]
    KdumpZlib,
    /// Windows full crashdump format, can be used instead of ELF
    /// converting (since 2.13)
    #[qapi(name = "win-dmp")]
    #[qapi(feature = "allowed-by-guest")]
    #[qapi(added = "3.0.0")]
    WinDmp,
}

/// The result format for `query-dump`.
///
#[qapi(added = "2.6.0")]
pub struct DumpQueryResult {
    /// bytes written in latest dump (uncompressed)
    pub completed: i64,
    /// enum of `DumpStatus`, which shows current dump status
    pub status: DumpStatus,
    /// total bytes to be written in latest dump (uncompressed)
    pub total: i64,
}

/// Describe the status of a long-running background guest memory dump.
///
#[qapi(added = "2.6.0")]
pub enum DumpStatus {
    /// there is one dump running in background.
    #[qapi(name = "active")]
    Active,
    /// the last dump has finished successfully.
    #[qapi(name = "completed")]
    Completed,
    /// the last dump has failed.
    #[qapi(name = "failed")]
    Failed,
    /// no `dump-guest-memory` has started yet.
    #[qapi(name = "none")]
    None,
}

/// An eBPF ELF object.
///
#[qapi(condition = "CONFIG_EBPF")]
#[qapi(added = "9.0.0")]
pub struct EbpfObject {
    /// the eBPF object encoded in base64
    pub object: String,
}

/// The eBPF programs that can be gotten with `request-ebpf`.
///
#[qapi(name = "EbpfProgramID")]
#[qapi(condition = "CONFIG_EBPF")]
#[qapi(added = "9.0.0")]
pub enum EbpfProgramId {
    /// Receive side scaling, technology that allows steering traffic
    /// between queues by calculation hash.  Users may set up
    /// indirection table and hash/packet types configurations.  Used
    /// with virtio-net.
    #[qapi(name = "rss")]
    Rss,
}

#[qapi(added = "10.0.0")]
pub enum EndianMode {
    /// Big endianness
    #[qapi(name = "big")]
    Big,
    /// Little endianness
    #[qapi(name = "little")]
    Little,
    /// Endianness not specified
    #[qapi(name = "unspecified")]
    Unspecified,
}

/// QEMU error classes
///
#[qapi(added = "1.2.0")]
#[qapi(removed = "2.6.0")]
pub enum ErrorClass {
    /// the requested command has not been found
    CommandNotFound,
    /// the requested operation can't be fulfilled because the
    /// selected device is encrypted
    DeviceEncrypted,
    /// a device has failed to be become active
    DeviceNotActive,
    /// the requested device has not been found
    DeviceNotFound,
    /// this is used for errors that don't require a specific error
    /// class. This should be the default case for most errors
    GenericError,
    /// the requested operation can't be fulfilled because a
    /// required KVM capability is missing
    #[qapi(name = "KVMMissingCap")]
    KvmMissingCap,
    /// the requested operation can't be fulfilled because a
    /// migration process is expected
    #[qapi(removed = "1.3.0")]
    MigrationExpected,
}

/// Information about a QMP event
///
#[qapi(added = "1.2.0")]
#[qapi(removed = "6.0.0")]
pub struct EventInfo {
    /// The event name
    pub name: String,
}

/// Common properties for event loops
///
#[qapi(added = "7.1.0")]
pub struct EventLoopBaseProperties {
    /// maximum number of requests in a batch for the AIO
    /// engine, 0 means that the engine will use its default.
    /// (default: 0)
    #[qapi(name = "aio-max-batch")]
    pub aio_max_batch: Option<i64>,
    /// maximum number of threads the thread pool can
    /// contain (default:64)
    #[qapi(name = "thread-pool-max")]
    pub thread_pool_max: Option<i64>,
    /// minimum number of threads reserved in the thread
    /// pool (default:0)
    #[qapi(name = "thread-pool-min")]
    pub thread_pool_min: Option<i64>,
}

/// Information about a Xen event channel port
///
#[qapi(condition = "TARGET_I386")]
#[qapi(added = "8.0.0")]
pub struct EvtchnInfo {
    /// port is masked
    pub masked: bool,
    /// port is currently active pending delivery
    pub pending: bool,
    /// the port number
    pub port: u16,
    /// remote domain for interdomain ports
    #[qapi(name = "remote-domain")]
    pub remote_domain: String,
    /// remote port ID, or virq/pirq number
    pub target: u16,
    /// the port type
    pub r#type: EvtchnPortType,
    /// target vCPU for this port
    pub vcpu: u32,
}

/// An enumeration of Xen event channel port types.
///
#[qapi(condition = "TARGET_I386")]
#[qapi(added = "8.0.0")]
pub enum EvtchnPortType {
    /// The port is unused.
    #[qapi(name = "closed")]
    Closed,
    /// The port is connected as an interdomain interrupt.
    #[qapi(name = "interdomain")]
    Interdomain,
    /// The post is an inter-processor interrupt (IPI).
    #[qapi(name = "ipi")]
    Ipi,
    /// The port is bound to a physical IRQ (PIRQ).
    #[qapi(name = "pirq")]
    Pirq,
    /// The port is allocated and ready to be bound.
    #[qapi(name = "unbound")]
    Unbound,
    /// The port is bound to a virtual IRQ (VIRQ).
    #[qapi(name = "virq")]
    Virq,
}

/// General options for `expire_password`.
///
#[qapi(added = "7.0.0")]
pub struct ExpirePasswordOptions {
    /// - 'vnc' to modify the VNC server expiration
    /// - 'spice' to modify the Spice server expiration
    #[qapi(discriminator)]
    pub protocol: DisplayProtocol,
    /// when to expire the password.
    ///
    /// - 'now' to expire the password immediately
    /// - 'never' to cancel password expiration
    /// - '+INT' where INT is the number of seconds from now (integer)
    /// - 'INT' where INT is the absolute time in seconds
    ///
    /// .. note:: Time is relative to the server and currently there is no
    /// way to coordinate server time with client time.  It is not
    /// recommended to use the absolute time version of the @time
    /// parameter unless you're sure you are on the same machine as the
    /// QEMU instance.
    pub time: String,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<ExpirePasswordOptionsBranch>,
}

#[qapi(added = "7.0.0")]
pub enum ExpirePasswordOptionsBranch {
    #[qapi(name = "spice")]
    Spice,
    #[qapi(name = "vnc")]
    Vnc(ExpirePasswordOptionsVnc),
}

/// Options for `expire_password` specific to the VNC protocol.
///
#[qapi(added = "7.0.0")]
pub struct ExpirePasswordOptionsVnc {
    /// The id of the display where the expiration should be
    /// changed.  Defaults to the first.
    pub display: Option<String>,
}

/// An enumeration of COLO failover status
///
#[qapi(added = "2.8.0")]
pub enum FailoverStatus {
    /// in the process of doing failover
    #[qapi(name = "active")]
    Active,
    /// finish the process of failover
    #[qapi(name = "completed")]
    Completed,
    /// no failover has ever happened
    #[qapi(name = "none")]
    None,
    /// restart the failover process, from 'none' -> 'completed'
    /// (Since 2.9)
    #[qapi(name = "relaunch")]
    #[qapi(added = "2.9.0")]
    Relaunch,
    /// got failover requirement but not handled
    #[qapi(name = "require")]
    Require,
}

/// Information about a file descriptor that belongs to an fd set.
///
#[qapi(added = "1.2.0")]
pub struct FdsetFdInfo {
    /// The file descriptor value.
    pub fd: i64,
    /// A free-form string that can be used to describe the fd.
    pub opaque: Option<String>,
}

/// Information about an fd set.
///
#[qapi(added = "1.2.0")]
pub struct FdsetInfo {
    /// A list of file descriptors that belong to this fd set.
    pub fds: Vec<FdsetFdInfo>,
    /// The ID of the fd set.
    #[qapi(name = "fdset-id")]
    pub fdset_id: i64,
}

/// A file descriptor name or number.
///
#[qapi(added = "9.0.0")]
pub struct FdSocketAddress {
    /// decimal is for file descriptor number, otherwise it's a file
    /// descriptor name.  Named file descriptors are permitted in
    /// monitor commands, in combination with the `getfd` command.
    /// Decimal file descriptors are permitted at startup or other
    /// contexts where no monitor context is active.
    pub str: String,
}

#[qapi(added = "9.0.0")]
pub struct FdSocketAddressWrapper {
    /// file descriptor name or number
    pub data: FdSocketAddress,
}

#[qapi(added = "8.2.0")]
pub struct FileMigrationArgs {
    /// The file to receive the migration stream
    pub filename: String,
    /// The file offset where the migration stream will start
    pub offset: u64,
}

/// Properties for filter-buffer objects.
///
#[qapi(added = "6.0.0")]
pub struct FilterBufferProperties {
    /// where to insert the filter relative to the filter given in
    pub insert: Option<NetfilterInsert>,
    /// a non-zero interval in microseconds.  All packets
    /// arriving in the given interval are delayed until the end of the
    /// interval.
    pub interval: u32,
    /// id of the network device backend to filter
    pub netdev: String,
    /// specifies where the filter should be inserted in the
    /// filter list.  "head" means the filter is inserted at the head of
    /// the filter list, before any existing filters.  "tail" means the
    /// filter is inserted at the tail of the filter list, behind any
    /// existing filters (default).  "id=<id>" means the filter is
    /// inserted before or behind the filter specified by <id>,
    /// depending on the @insert property.  (default: "tail")
    ///
    ///   .: Ignored if @position is "head" or "tail". (default: behind)
    pub position: Option<String>,
    /// indicates which queue(s) to filter (default: all)
    pub queue: Option<NetFilterDirection>,
    /// indicates whether the filter is enabled ("on") or disabled
    /// ("off") (default: "on")
    pub status: Option<String>,
}

/// Properties for filter-dump objects.
///
#[qapi(added = "6.0.0")]
pub struct FilterDumpProperties {
    /// the filename where the dumped packets should be stored
    pub file: String,
    /// where to insert the filter relative to the filter given in
    pub insert: Option<NetfilterInsert>,
    /// maximum number of bytes in a packet that are stored
    /// (default: 65536)
    pub maxlen: Option<u32>,
    /// id of the network device backend to filter
    pub netdev: String,
    /// specifies where the filter should be inserted in the
    /// filter list.  "head" means the filter is inserted at the head of
    /// the filter list, before any existing filters.  "tail" means the
    /// filter is inserted at the tail of the filter list, behind any
    /// existing filters (default).  "id=<id>" means the filter is
    /// inserted before or behind the filter specified by <id>,
    /// depending on the @insert property.  (default: "tail")
    ///
    ///   .: Ignored if @position is "head" or "tail". (default: behind)
    pub position: Option<String>,
    /// indicates which queue(s) to filter (default: all)
    pub queue: Option<NetFilterDirection>,
    /// indicates whether the filter is enabled ("on") or disabled
    /// ("off") (default: "on")
    pub status: Option<String>,
}

/// Properties for filter-mirror objects.
///
#[qapi(added = "6.0.0")]
pub struct FilterMirrorProperties {
    /// where to insert the filter relative to the filter given in
    pub insert: Option<NetfilterInsert>,
    /// id of the network device backend to filter
    pub netdev: String,
    /// the name of a character device backend to which all
    /// incoming packets are mirrored
    pub outdev: String,
    /// specifies where the filter should be inserted in the
    /// filter list.  "head" means the filter is inserted at the head of
    /// the filter list, before any existing filters.  "tail" means the
    /// filter is inserted at the tail of the filter list, behind any
    /// existing filters (default).  "id=<id>" means the filter is
    /// inserted before or behind the filter specified by <id>,
    /// depending on the @insert property.  (default: "tail")
    ///
    ///   .: Ignored if @position is "head" or "tail". (default: behind)
    pub position: Option<String>,
    /// indicates which queue(s) to filter (default: all)
    pub queue: Option<NetFilterDirection>,
    /// indicates whether the filter is enabled ("on") or disabled
    /// ("off") (default: "on")
    pub status: Option<String>,
    /// if true, vnet header support is enabled
    /// (default: false)
    pub vnet_hdr_support: Option<bool>,
}

/// Properties for filter-redirector objects.
///
/// At least one of @indev or @outdev must be present.  If both are
/// present, they must not refer to the same character device backend.
///
#[qapi(added = "6.0.0")]
pub struct FilterRedirectorProperties {
    /// the name of a character device backend from which packets
    /// are received and redirected to the filtered network device
    pub indev: Option<String>,
    /// where to insert the filter relative to the filter given in
    pub insert: Option<NetfilterInsert>,
    /// id of the network device backend to filter
    pub netdev: String,
    /// the name of a character device backend to which all
    /// incoming packets are redirected
    pub outdev: Option<String>,
    /// specifies where the filter should be inserted in the
    /// filter list.  "head" means the filter is inserted at the head of
    /// the filter list, before any existing filters.  "tail" means the
    /// filter is inserted at the tail of the filter list, behind any
    /// existing filters (default).  "id=<id>" means the filter is
    /// inserted before or behind the filter specified by <id>,
    /// depending on the @insert property.  (default: "tail")
    ///
    ///   .: Ignored if @position is "head" or "tail". (default: behind)
    pub position: Option<String>,
    /// indicates which queue(s) to filter (default: all)
    pub queue: Option<NetFilterDirection>,
    /// indicates whether the filter is enabled ("on") or disabled
    /// ("off") (default: "on")
    pub status: Option<String>,
    /// if true, vnet header support is enabled
    /// (default: false)
    pub vnet_hdr_support: Option<bool>,
}

/// Properties for filter-rewriter objects.
///
#[qapi(added = "6.0.0")]
pub struct FilterRewriterProperties {
    /// where to insert the filter relative to the filter given in
    pub insert: Option<NetfilterInsert>,
    /// id of the network device backend to filter
    pub netdev: String,
    /// specifies where the filter should be inserted in the
    /// filter list.  "head" means the filter is inserted at the head of
    /// the filter list, before any existing filters.  "tail" means the
    /// filter is inserted at the tail of the filter list, behind any
    /// existing filters (default).  "id=<id>" means the filter is
    /// inserted before or behind the filter specified by <id>,
    /// depending on the @insert property.  (default: "tail")
    ///
    ///   .: Ignored if @position is "head" or "tail". (default: behind)
    pub position: Option<String>,
    /// indicates which queue(s) to filter (default: all)
    pub queue: Option<NetFilterDirection>,
    /// indicates whether the filter is enabled ("on") or disabled
    /// ("off") (default: "on")
    pub status: Option<String>,
    /// if true, vnet header support is enabled
    /// (default: false)
    pub vnet_hdr_support: Option<bool>,
}

#[qapi(added = "10.2.0")]
pub struct FirmwareLog {
    /// Firmware debug log, in base64 encoding.  First and last log
    /// line might be incomplete.
    pub log: String,
    /// Firmware version.
    pub version: Option<String>,
}

/// Type of floppy drive to be emulated by the Floppy Disk Controller.
///
#[qapi(added = "2.6.0")]
pub enum FloppyDriveType {
    /// 1.2MB 5.25" drive
    #[qapi(name = "120")]
    _120,
    /// 1.44MB 3.5" drive
    #[qapi(name = "144")]
    _144,
    /// 2.88MB 3.5" drive
    #[qapi(name = "288")]
    _288,
    /// Automatically determined by inserted media at boot
    #[qapi(name = "auto")]
    Auto,
    /// No drive connected
    #[qapi(name = "none")]
    None,
}

/// Possible allow_other modes for FUSE exports.
///
#[qapi(added = "6.1.0")]
pub enum FuseExportAllowOther {
    /// Try mounting with allow_other first, and if that fails, retry
    /// without allow_other.
    #[qapi(name = "auto")]
    Auto,
    /// Do not pass allow_other as a mount option.
    #[qapi(name = "off")]
    Off,
    /// Pass allow_other as a mount option.
    #[qapi(name = "on")]
    On,
}

/// The struct describes capability for a specific GIC (Generic
/// Interrupt Controller) version.  These bits are not only decided by
/// QEMU/KVM software version, but also decided by the hardware that the
/// program is running upon.
///
#[qapi(name = "GICCapability")]
#[qapi(condition = "defined(TARGET_ARM)")]
#[qapi(added = "2.6.0")]
pub struct GicCapability {
    /// whether current QEMU/hardware supports emulated GIC
    /// device in user space.
    pub emulated: bool,
    /// whether current QEMU/hardware supports hardware accelerated
    /// GIC device in kernel.
    pub kernel: bool,
    /// version of GIC to be described.  Currently, only 2 and 3
    /// are supported.
    pub version: i64,
}

/// Captures the address of a socket
///
/// Details for connecting to a gluster server
///
#[qapi(added = "2.7.0")]
#[qapi(removed = "2.9.0")]
pub struct GlusterServer {
    /// Transport type used for gluster connection
    ///
    /// This is similar to SocketAddress, only distinction:
    ///
    /// 1. GlusterServer is a flat union, SocketAddress is a simple union.
    /// A flat union is nicer than simple because it avoids nesting
    /// (i.e. more {}) on the wire.
    ///
    /// 2. GlusterServer lacks case 'fd', since gluster doesn't let you
    /// pass in a file descriptor.
    ///
    /// GlusterServer is actually not Gluster-specific, its a
    /// compatibility evolved into an alternate for SocketAddress.
    #[qapi(discriminator)]
    pub r#type: GlusterTransport,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<GlusterServerBranch>,
}

#[qapi(added = "2.7.0")]
#[qapi(removed = "2.9.0")]
pub enum GlusterServerBranch {
    #[qapi(name = "tcp")]
    Tcp(InetSocketAddress),
    #[qapi(name = "unix")]
    Unix(UnixSocketAddress),
}

/// An enumeration of Gluster transport types
///
#[qapi(added = "2.7.0")]
#[qapi(removed = "2.9.0")]
pub enum GlusterTransport {
    /// TCP   - Transmission Control Protocol
    #[qapi(name = "tcp")]
    Tcp,
    /// UNIX  - Unix domain socket
    #[qapi(name = "unix")]
    Unix,
}

/// Key combinations to toggle input-linux between host and guest.
///
#[qapi(added = "4.0.0")]
pub enum GrabToggleKeys {
    /// left and right alt key
    #[qapi(name = "alt-alt")]
    AltAlt,
    /// left and right control key
    #[qapi(name = "ctrl-ctrl")]
    CtrlCtrl,
    /// either control key and scroll lock key
    #[qapi(name = "ctrl-scrolllock")]
    CtrlScrolllock,
    /// left and right meta key
    #[qapi(name = "meta-meta")]
    MetaMeta,
    /// scroll lock key
    #[qapi(name = "scrolllock")]
    Scrolllock,
    /// left and right shift key
    #[qapi(name = "shift-shift")]
    #[qapi(added = "4.2.0")]
    ShiftShift,
}

#[qapi(added = "9.0.0")]
pub enum GranuleMode {
    /// granule page size of 16KiB
    #[qapi(name = "16k")]
    _16k,
    /// granule page size of 4KiB
    #[qapi(name = "4k")]
    _4k,
    /// granule page size of 64KiB
    #[qapi(name = "64k")]
    _64k,
    /// granule page size of 8KiB
    #[qapi(name = "8k")]
    _8k,
    /// granule matches the host page size
    #[qapi(name = "host")]
    Host,
}

/// An enumeration of the actions taken when guest OS panic is detected
///
#[qapi(added = "2.1.0")]
pub enum GuestPanicAction {
    /// system pauses
    #[qapi(name = "pause")]
    Pause,
    /// system powers off (since 2.8)
    #[qapi(name = "poweroff")]
    #[qapi(added = "2.8.0")]
    Poweroff,
    /// system continues to run (since 5.0)
    #[qapi(name = "run")]
    #[qapi(added = "5.0.0")]
    Run,
}

/// Information about a guest panic
///
#[qapi(added = "2.9.0")]
pub struct GuestPanicInformation {
    /// Crash type that defines the hypervisor specific information
    #[qapi(discriminator)]
    pub r#type: GuestPanicInformationType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<GuestPanicInformationBranch>,
}

#[qapi(added = "2.9.0")]
pub enum GuestPanicInformationBranch {
    #[qapi(name = "hyper-v")]
    HyperV(GuestPanicInformationHyperV),
    #[qapi(name = "s390")]
    #[qapi(added = "2.12.0")]
    S390(GuestPanicInformationS390),
    #[qapi(name = "sev")]
    #[qapi(added = "11.0.0")]
    Sev(GuestPanicInformationSev),
    #[qapi(name = "tdx")]
    #[qapi(added = "10.1.0")]
    Tdx(GuestPanicInformationTdx),
}

/// Hyper-V specific guest panic information (HV crash MSRs)
///
#[qapi(added = "2.9.0")]
pub struct GuestPanicInformationHyperV {
    /// for Windows, `STOP` code for the guest crash.  For Linux,
    /// an error code.
    pub arg1: u64,
    /// for Windows, first argument of the `STOP`.  For Linux, the
    /// guest OS ID, which has the kernel version in bits 16-47 and
    /// 0x8100 in bits 48-63.
    pub arg2: u64,
    /// for Windows, second argument of the `STOP`.  For Linux, the
    /// program counter of the guest.
    pub arg3: u64,
    /// for Windows, third argument of the `STOP`.  For Linux, the
    /// RAX register (x86) or the stack pointer (aarch64) of the guest.
    pub arg4: u64,
    /// for Windows, fourth argument of the `STOP`.  For x86 Linux,
    /// the stack pointer of the guest.
    pub arg5: u64,
}

/// S390 specific guest panic information (PSW)
///
#[qapi(added = "2.12.0")]
pub struct GuestPanicInformationS390 {
    /// core id of the CPU that crashed
    pub core: u32,
    /// guest instruction address
    #[qapi(name = "psw-addr")]
    pub psw_addr: u64,
    /// control fields of guest PSW
    #[qapi(name = "psw-mask")]
    pub psw_mask: u64,
    /// guest crash reason
    pub reason: S390CrashReason,
}

/// Information for AMD SEV-specific termination request (GHCB MSR
/// contents)
///
#[qapi(added = "11.0.0")]
pub struct GuestPanicInformationSev {
    /// The reason code provided by the guest
    pub code: u32,
    /// The reason code set provided by the guest
    pub set: u32,
}

/// TDX Guest panic information specific to TDX, as specified in the
/// "Guest-Hypervisor Communication Interface (GHCI) Specification",
/// section TDG.VP.VMCALL<ReportFatalError>.
///
#[qapi(added = "10.1.0")]
pub struct GuestPanicInformationTdx {
    /// TD-specific error code
    #[qapi(name = "error-code")]
    pub error_code: u32,
    /// guest-physical address of a page that contains more verbose
    /// error information, as zero-terminated string.  Present when the
    /// "GPA valid" bit (bit 63) is set in @error-code.
    pub gpa: Option<u64>,
    /// Human-readable error message provided by the guest.  Not
    /// to be trusted.
    pub message: String,
}

/// An enumeration of the guest panic information types
///
#[qapi(added = "2.9.0")]
pub enum GuestPanicInformationType {
    /// hyper-v guest panic information type
    #[qapi(name = "hyper-v")]
    HyperV,
    /// s390 guest panic information type (Since: 2.12)
    #[qapi(name = "s390")]
    #[qapi(added = "2.12.0")]
    S390,
    /// AMD SEV-ES guest termination information type (Since: 11.0)
    #[qapi(name = "sev")]
    #[qapi(added = "11.0.0")]
    Sev,
    /// tdx guest panic information type (Since: 10.1)
    #[qapi(name = "tdx")]
    #[qapi(added = "10.1.0")]
    Tdx,
}

/// GUID information.
///
#[qapi(added = "2.9.0")]
pub struct GuidInfo {
    /// the globally unique identifier
    pub guid: String,
}

/// Cache associativity in the Memory Side Cache Information Structure
/// of HMAT
///
/// For more information of `HmatCacheAssociativity`, see chapter
/// 5.2.27.5: Table 5-147 of ACPI 6.3 spec.
///
#[qapi(added = "5.0.0")]
pub enum HmatCacheAssociativity {
    /// Complex Cache Indexing (implementation specific)
    #[qapi(name = "complex")]
    Complex,
    /// Direct Mapped
    #[qapi(name = "direct")]
    Direct,
    /// None (no memory side cache in this proximity domain, or cache
    /// associativity unknown)
    #[qapi(name = "none")]
    None,
}

/// Cache write policy in the Memory Side Cache Information Structure of
/// HMAT
///
/// For more information of `HmatCacheWritePolicy`, see chapter
/// 5.2.27.5: Table 5-147: Field "Cache Attributes" of ACPI 6.3 spec.
///
#[qapi(added = "5.0.0")]
pub enum HmatCacheWritePolicy {
    /// None (no memory side cache in this proximity domain, or cache
    /// write policy unknown)
    #[qapi(name = "none")]
    None,
    /// Write Back (WB)
    #[qapi(name = "write-back")]
    WriteBack,
    /// Write Through (WT)
    #[qapi(name = "write-through")]
    WriteThrough,
}

/// Data type in the System Locality Latency and Bandwidth Information
/// Structure of HMAT (Heterogeneous Memory Attribute Table)
///
/// For more information about `HmatLBDataType`, see chapter 5.2.27.4:
/// Table 5-146: Field "Data Type" of ACPI 6.3 spec.
///
#[qapi(name = "HmatLBDataType")]
#[qapi(added = "5.0.0")]
pub enum HmatLbDataType {
    /// access bandwidth (Bytes per second)
    #[qapi(name = "access-bandwidth")]
    AccessBandwidth,
    /// access latency (nanoseconds)
    #[qapi(name = "access-latency")]
    AccessLatency,
    /// read bandwidth (Bytes per second)
    #[qapi(name = "read-bandwidth")]
    ReadBandwidth,
    /// read latency (nanoseconds)
    #[qapi(name = "read-latency")]
    ReadLatency,
    /// write bandwidth (Bytes per second)
    #[qapi(name = "write-bandwidth")]
    WriteBandwidth,
    /// write latency (nanoseconds)
    #[qapi(name = "write-latency")]
    WriteLatency,
}

/// The memory hierarchy in the System Locality Latency and Bandwidth
/// Information Structure of HMAT (Heterogeneous Memory Attribute Table)
///
/// For more information about `HmatLBMemoryHierarchy`, see chapter
/// 5.2.27.4: Table 5-146: Field "Flags" of ACPI 6.3 spec.
///
#[qapi(name = "HmatLBMemoryHierarchy")]
#[qapi(added = "5.0.0")]
pub enum HmatLbMemoryHierarchy {
    /// first level of memory side cache
    #[qapi(name = "first-level")]
    FirstLevel,
    /// the structure represents the memory performance
    #[qapi(name = "memory")]
    Memory,
    /// second level of memory side cache
    #[qapi(name = "second-level")]
    SecondLevel,
    /// third level of memory side cache
    #[qapi(name = "third-level")]
    ThirdLevel,
}

/// Host memory policy types
///
#[qapi(added = "2.1.0")]
pub enum HostMemPolicy {
    /// a strict policy that restricts memory allocation to the host
    /// nodes specified
    #[qapi(name = "bind")]
    Bind,
    /// restore default policy, remove any nondefault policy
    #[qapi(name = "default")]
    Default,
    /// memory allocations are interleaved across the set of
    /// host nodes specified
    #[qapi(name = "interleave")]
    Interleave,
    /// set the preferred host nodes for allocation
    #[qapi(name = "preferred")]
    Preferred,
}

/// Set of modifier keys that need to be held for shortcut key actions.
///
#[qapi(added = "7.1.0")]
pub enum HotKeyMod {
    #[qapi(name = "lctrl-lalt")]
    LctrlLalt,
    #[qapi(name = "lshift-lctrl-lalt")]
    LshiftLctrlLalt,
    #[qapi(name = "rctrl")]
    Rctrl,
}

#[qapi(name = "HotpluggableCPU")]
#[qapi(added = "2.7.0")]
pub struct HotpluggableCpu {
    /// list of properties to pass for hotplugging a CPU with
    /// `device_add`
    pub props: CpuInstanceProperties,
    /// link to existing CPU object if CPU is present or omitted
    /// if CPU is not present.
    ///
    /// .. note:: Management should be prepared to pass through additional
    /// properties with `device_add`.
    #[qapi(name = "qom-path")]
    pub qom_path: Option<String>,
    /// CPU object type for usage with `device_add` command
    pub r#type: String,
    /// number of logical VCPU threads `HotpluggableCPU`
    /// provides
    #[qapi(name = "vcpus-count")]
    pub vcpus_count: i64,
}

#[qapi(added = "6.2.0")]
pub struct HumanReadableText {
    /// Formatted output intended for humans.
    #[qapi(name = "human-readable-text")]
    pub human_readable_text: String,
}

/// hv-balloon provided memory state information
///
#[qapi(added = "8.2.0")]
pub struct HvBalloonDeviceInfo {
    /// device's ID
    pub id: Option<String>,
    /// the maximum size of memory that the device can provide
    #[qapi(name = "max-size")]
    pub max_size: u64,
    /// physical address in memory, where device is mapped
    pub memaddr: Option<u64>,
    /// memory backend linked with device
    pub memdev: Option<String>,
}

#[qapi(added = "8.2.0")]
pub struct HvBalloonDeviceInfoWrapper {
    /// hv-balloon provided memory state information
    pub data: HvBalloonDeviceInfo,
}

/// hv-balloon guest-provided memory status information.
///
#[qapi(added = "8.2.0")]
pub struct HvBalloonInfo {
    /// the amount of the memory inside the guest available for
    /// new allocations ("free")
    pub available: u64,
    /// the amount of memory in use inside the guest plus the
    /// amount of the memory unusable inside the guest (ballooned out,
    /// offline, etc.)
    pub committed: u64,
}

/// Properties common to objects that handle IGVM files.
///
#[qapi(condition = "CONFIG_IGVM")]
#[qapi(added = "10.1.0")]
pub struct IgvmCfgProperties {
    /// IGVM file to use to configure guest
    pub file: String,
}

/// Information about a QEMU image file check
///
#[qapi(added = "1.5.0")]
pub struct ImageCheck {
    /// total number of allocated clusters, this field
    /// is present if the driver for the image format supports it
    #[qapi(name = "allocated-clusters")]
    pub allocated_clusters: Option<i64>,
    /// number of unexpected errors occurred during check
    #[qapi(name = "check-errors")]
    pub check_errors: i64,
    /// total number of compressed clusters, this
    /// field is present if the driver for the image format supports it
    #[qapi(name = "compressed-clusters")]
    pub compressed_clusters: Option<i64>,
    /// number of corruptions found during the check if any
    pub corruptions: Option<i64>,
    /// number of corruptions fixed during the check if
    /// any
    #[qapi(name = "corruptions-fixed")]
    pub corruptions_fixed: Option<i64>,
    /// name of the image file checked
    pub filename: String,
    /// format of the image file checked
    pub format: String,
    /// total number of fragmented clusters, this
    /// field is present if the driver for the image format supports it
    #[qapi(name = "fragmented-clusters")]
    pub fragmented_clusters: Option<i64>,
    /// offset (in bytes) where the image ends, this
    /// field is present if the driver for the image format supports it
    #[qapi(name = "image-end-offset")]
    pub image_end_offset: Option<i64>,
    /// number of leaks found during the check if any
    pub leaks: Option<i64>,
    /// number of leaks fixed during the check if any
    #[qapi(name = "leaks-fixed")]
    pub leaks_fixed: Option<i64>,
    /// total number of clusters, this field is present if
    /// the driver for the image format supports it
    #[qapi(name = "total-clusters")]
    pub total_clusters: Option<i64>,
}

/// Supported image format types.
///
#[qapi(added = "7.1.0")]
pub enum ImageFormat {
    /// PNG format
    #[qapi(name = "png")]
    Png,
    /// PPM format
    #[qapi(name = "ppm")]
    Ppm,
}

/// Information about a QEMU image file, and potentially its backing
/// image
///
#[qapi(added = "1.3.0")]
pub struct ImageInfo {
    /// actual size on disk in bytes of the image
    #[qapi(name = "actual-size")]
    pub actual_size: Option<i64>,
    /// name of the backing file
    #[qapi(name = "backing-filename")]
    pub backing_filename: Option<String>,
    /// the format of the backing file
    #[qapi(name = "backing-filename-format")]
    pub backing_filename_format: Option<String>,
    /// info of the backing image
    #[qapi(name = "backing-image")]
    #[qapi(added = "1.6.0")]
    pub backing_image: Option<Box<ImageInfo>>,
    /// size of a cluster in bytes
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<i64>,
    /// true if the image is compressed (Since 1.7)
    #[qapi(added = "1.7.0")]
    pub compressed: Option<bool>,
    /// true if image is not cleanly closed
    #[qapi(name = "dirty-flag")]
    pub dirty_flag: Option<bool>,
    /// true if the image is encrypted
    pub encrypted: Option<bool>,
    /// name of the image file
    pub filename: String,
    /// format of the image file
    pub format: String,
    /// structure supplying additional format-specific
    /// information (since 1.7)
    #[qapi(name = "format-specific")]
    #[qapi(added = "1.7.0")]
    pub format_specific: Option<ImageInfoSpecific>,
    /// full path of the backing file
    #[qapi(name = "full-backing-filename")]
    pub full_backing_filename: Option<String>,
    /// block limits that are used for I/O on the node (Since 10.2)
    #[qapi(added = "10.2.0")]
    pub limits: Option<BlockLimitsInfo>,
    /// list of VM snapshots
    pub snapshots: Option<Vec<SnapshotInfo>>,
    /// maximum capacity in bytes of the image
    #[qapi(name = "virtual-size")]
    pub virtual_size: i64,
}

/// A discriminated record of image format specific information
/// structures.
///
#[qapi(added = "1.7.0")]
pub struct ImageInfoSpecific {
    #[qapi(discriminator)]
    #[qapi(name = "type")]
    #[qapi(removed = "6.2.0")]
    pub r_type: Option<ImageInfoSpecificType>,
    /// block driver name
    #[qapi(discriminator)]
    #[qapi(added = "6.2.0")]
    pub r#type: Option<ImageInfoSpecificKind>,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<ImageInfoSpecificBranch>,
}

#[qapi(added = "1.7.0")]
pub enum ImageInfoSpecificBranch {
    #[qapi(name = "file")]
    #[qapi(added = "8.0.0")]
    File(ImageInfoSpecificFileWrapper),
    #[qapi(name = "luks")]
    #[qapi(added = "2.7.0")]
    Luks(ImageInfoSpecificLuksWrapper),
    #[qapi(name = "qcow2")]
    Qcow2(ImageInfoSpecificQCow2Wrapper),
    #[qapi(name = "rbd")]
    #[qapi(added = "6.1.0")]
    Rbd(ImageInfoSpecificRbdWrapper),
    #[qapi(name = "vmdk")]
    Vmdk(ImageInfoSpecificVmdkWrapper),
}

#[qapi(added = "8.0.0")]
pub struct ImageInfoSpecificFile {
    /// Extent size hint (if available)
    #[qapi(name = "extent-size-hint")]
    pub extent_size_hint: Option<u64>,
}

#[qapi(added = "8.0.0")]
pub struct ImageInfoSpecificFileWrapper {
    /// image information specific to files
    pub data: ImageInfoSpecificFile,
}

#[qapi(added = "6.2.0")]
pub enum ImageInfoSpecificKind {
    /// Since 8.0
    #[qapi(name = "file")]
    #[qapi(added = "8.0.0")]
    File,
    /// Since 2.7
    #[qapi(name = "luks")]
    Luks,
    #[qapi(name = "qcow2")]
    Qcow2,
    /// Since 6.1
    #[qapi(name = "rbd")]
    Rbd,
    #[qapi(name = "vmdk")]
    Vmdk,
}

#[qapi(name = "ImageInfoSpecificLUKSWrapper")]
#[qapi(added = "6.2.0")]
pub struct ImageInfoSpecificLuksWrapper {
    /// image information specific to LUKS
    pub data: QCryptoBlockInfoLuks,
}

#[qapi(added = "1.7.0")]
pub struct ImageInfoSpecificQCow2 {
    /// A list of qcow2 bitmap details (since 4.0)
    #[qapi(added = "4.0.0")]
    pub bitmaps: Option<Vec<Qcow2BitmapInfo>>,
    /// compatibility level
    pub compat: String,
    /// the image cluster compression method (since 5.1)
    #[qapi(name = "compression-type")]
    #[qapi(added = "5.1.0")]
    pub compression_type: Option<Qcow2CompressionType>,
    /// true if the image has been marked corrupt; only valid for
    /// compat >= 1.1 (since 2.2)
    #[qapi(added = "2.2.0")]
    pub corrupt: Option<bool>,
    /// the filename of the external data file that is stored in
    /// the image and used as a default for opening the image
    /// (since: 4.0)
    #[qapi(name = "data-file")]
    #[qapi(added = "4.0.0")]
    pub data_file: Option<String>,
    /// True if the external data file must stay valid as a
    /// standalone (read-only) raw image without looking at qcow2
    /// metadata (since: 4.0)
    #[qapi(name = "data-file-raw")]
    #[qapi(added = "4.0.0")]
    pub data_file_raw: Option<bool>,
    /// details about encryption parameters; only set if image is
    /// encrypted (since 2.10)
    #[qapi(added = "2.10.0")]
    pub encrypt: Option<ImageInfoSpecificQCow2Encryption>,
    /// true if the image has extended L2 entries; only valid
    /// for compat >= 1.1 (since 5.2)
    #[qapi(name = "extended-l2")]
    #[qapi(added = "5.2.0")]
    pub extended_l2: Option<bool>,
    /// on or off; only valid for compat >= 1.1
    #[qapi(name = "lazy-refcounts")]
    pub lazy_refcounts: Option<bool>,
    /// width of a refcount entry in bits (since 2.3)
    #[qapi(name = "refcount-bits")]
    #[qapi(added = "2.3.0")]
    pub refcount_bits: Option<i64>,
}

#[qapi(added = "2.10.0")]
pub struct ImageInfoSpecificQCow2Encryption {
    /// The encryption format
    #[qapi(discriminator)]
    pub format: BlockdevQcow2EncryptionFormat,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<ImageInfoSpecificQCow2EncryptionBranch>,
}

#[qapi(added = "2.10.0")]
pub struct ImageInfoSpecificQCow2EncryptionBase {
    /// The encryption format
    pub format: BlockdevQcow2EncryptionFormat,
}

#[qapi(added = "2.10.0")]
pub enum ImageInfoSpecificQCow2EncryptionBranch {
    /// AES-CBC with plain64 initialization vectors
    #[qapi(name = "aes")]
    Aes,
    #[qapi(name = "luks")]
    Luks(QCryptoBlockInfoLuks),
}

#[qapi(added = "6.2.0")]
pub struct ImageInfoSpecificQCow2Wrapper {
    /// image information specific to QCOW2
    pub data: ImageInfoSpecificQCow2,
}

#[qapi(added = "6.1.0")]
pub struct ImageInfoSpecificRbd {
    /// Image encryption format.  If encryption is
    /// enabled for the image (see encrypted in BlockNodeInfo), this is
    /// the actual format in which the image is accessed.  If encryption
    /// is not enabled, this is the result of probing when the image was
    /// opened, to give a suggestion which encryption format could be
    /// enabled.  Note that probing results can be changed by the guest
    /// by writing a (possibly partial) encryption format header to the
    /// image, so don't treat this information as trusted if the guest
    /// is not trusted.
    #[qapi(name = "encryption-format")]
    pub encryption_format: Option<RbdImageEncryptionFormat>,
}

#[qapi(added = "6.2.0")]
pub struct ImageInfoSpecificRbdWrapper {
    /// image information specific to RBD
    pub data: ImageInfoSpecificRbd,
}

#[qapi(added = "1.7.0")]
#[qapi(removed = "6.2.0")]
pub enum ImageInfoSpecificType {
    #[qapi(name = "luks")]
    #[qapi(added = "2.7.0")]
    Luks,
    #[qapi(name = "qcow2")]
    Qcow2,
    #[qapi(name = "rbd")]
    #[qapi(added = "6.1.0")]
    Rbd,
    #[qapi(name = "vmdk")]
    Vmdk,
}

#[qapi(added = "1.7.0")]
pub struct ImageInfoSpecificVmdk {
    /// Content id of image
    pub cid: i64,
    /// The create type of VMDK image
    #[qapi(name = "create-type")]
    pub create_type: String,
    /// List of extent files
    pub extents: Vec<VmdkExtentInfo>,
    /// Parent VMDK image's cid
    #[qapi(name = "parent-cid")]
    pub parent_cid: i64,
}

#[qapi(added = "6.2.0")]
pub struct ImageInfoSpecificVmdkWrapper {
    /// image information specific to VMDK
    pub data: ImageInfoSpecificVmdk,
}

/// Captures a socket address or address range in the Internet
/// namespace.
///
#[qapi(added = "1.3.0")]
pub struct InetSocketAddress {
    /// host part of the address
    pub host: String,
    /// whether to accept IPv4 addresses, default try both IPv4 and
    /// IPv6
    pub ipv4: Option<bool>,
    /// whether to accept IPv6 addresses, default try both IPv4 and
    /// IPv6
    pub ipv6: Option<bool>,
    /// enable keep-alive when connecting to/listening on this
    /// socket.
    /// (Since 4.2, not supported for listening sockets until 10.1)
    #[qapi(name = "keep-alive")]
    #[qapi(added = "4.2.0")]
    pub keep_alive: Option<bool>,
    /// number of keep-alive packets sent before the
    /// connection is closed.  Only supported for TCP sockets on systems
    /// where TCP_KEEPCNT socket option is defined (this includes Linux,
    /// Windows, macOS, FreeBSD, but not OpenBSD).  When set to 0,
    /// system setting is used.  (Since 10.1)
    #[qapi(name = "keep-alive-count")]
    #[qapi(condition = "HAVE_TCP_KEEPCNT")]
    #[qapi(added = "10.1.0")]
    pub keep_alive_count: Option<u32>,
    /// time in seconds the connection needs to be idle
    /// before sending a keepalive packet.  Only supported for TCP
    /// sockets on systems where TCP_KEEPIDLE socket option is defined
    /// (this includes Linux, Windows, macOS, FreeBSD, but not OpenBSD).
    /// When set to 0, system setting is used.  (Since 10.1)
    #[qapi(name = "keep-alive-idle")]
    #[qapi(condition = "HAVE_TCP_KEEPIDLE")]
    #[qapi(added = "10.1.0")]
    pub keep_alive_idle: Option<u32>,
    /// time in seconds between keep-alive packets.
    /// Only supported for TCP sockets on systems where TCP_KEEPINTVL is
    /// defined (this includes Linux, Windows, macOS, FreeBSD, but not
    /// OpenBSD).  When set to 0, system setting is used.  (Since 10.1)
    #[qapi(name = "keep-alive-interval")]
    #[qapi(condition = "HAVE_TCP_KEEPINTVL")]
    #[qapi(added = "10.1.0")]
    pub keep_alive_interval: Option<u32>,
    /// enable multi-path TCP.  (Since 6.1)
    #[qapi(condition = "defined(IPPROTO_MPTCP)")]
    #[qapi(added = "6.1.0")]
    pub mptcp: Option<bool>,
    /// true if the host/port are guaranteed to be numeric, false
    /// if name resolution should be attempted.  Defaults to false.
    /// (Since 2.9)
    #[qapi(added = "2.9.0")]
    pub numeric: Option<bool>,
    /// port part of the address, or lowest port if @to is present
    pub port: String,
    /// If present, this is range of possible addresses, with port
    /// between @port and @to.
    pub to: Option<u16>,
}

#[qapi(added = "2.9.0")]
pub struct InetSocketAddressBase {
    pub host: String,
    pub port: String,
}

#[qapi(added = "6.2.0")]
pub struct InetSocketAddressWrapper {
    /// internet domain socket address
    pub data: InetSocketAddress,
}

/// Position axis of a pointer input device (mouse, tablet).
///
#[qapi(added = "2.0.0")]
pub enum InputAxis {
    #[qapi(name = "x")]
    X,
    #[qapi(name = "y")]
    Y,
}

/// Properties for input-barrier objects.
///
#[qapi(added = "6.0.0")]
pub struct InputBarrierProperties {
    /// the height of secondary screen in pixels (default: "1080")
    pub height: Option<String>,
    /// the screen name as declared in the screens section of
    /// barrier.conf
    pub name: String,
    /// TCP port of the Barrier server (default: "24800")
    pub port: Option<String>,
    /// hostname of the Barrier server (default: "localhost")
    pub server: Option<String>,
    /// the width of secondary screen in pixels (default: "1920")
    pub width: Option<String>,
    /// x coordinate of the leftmost pixel on the guest screen
    /// (default: "0")
    #[qapi(name = "x-origin")]
    pub x_origin: Option<String>,
    /// y coordinate of the topmost pixel on the guest screen
    /// (default: "0")
    #[qapi(name = "y-origin")]
    pub y_origin: Option<String>,
}

/// Pointer button input event.
///
#[qapi(added = "2.0.0")]
pub struct InputBtnEvent {
    /// Which button this event is for.
    pub button: InputButton,
    /// True for key-down and false for key-up events.
    pub down: bool,
}

#[qapi(added = "6.2.0")]
pub struct InputBtnEventWrapper {
    /// Pointer button input event
    pub data: InputBtnEvent,
}

/// Button of a pointer input device (mouse, tablet).
///
#[qapi(added = "2.0.0")]
pub enum InputButton {
    /// rear side button of a 5-button mouse (since 2.9)
    #[qapi(name = "extra")]
    #[qapi(added = "2.9.0")]
    Extra,
    #[qapi(name = "left")]
    Left,
    #[qapi(name = "middle")]
    Middle,
    #[qapi(name = "right")]
    Right,
    /// front side button of a 5-button mouse (since 2.9)
    #[qapi(name = "side")]
    #[qapi(added = "2.9.0")]
    Side,
    /// screen contact on a multi-touch device (since 8.1)
    #[qapi(name = "touch")]
    #[qapi(added = "8.1.0")]
    Touch,
    #[qapi(name = "wheel-down")]
    WheelDown,
    #[qapi(name = "wheel-left")]
    #[qapi(added = "7.0.0")]
    WheelLeft,
    #[qapi(name = "wheel-right")]
    #[qapi(added = "7.0.0")]
    WheelRight,
    #[qapi(name = "wheel-up")]
    WheelUp,
}

/// Input event union.
///
#[qapi(added = "2.0.0")]
pub struct InputEvent {
    #[qapi(discriminator)]
    #[qapi(name = "type")]
    #[qapi(removed = "6.2.0")]
    pub r_type: Option<InputEventType>,
    /// the type of input event
    #[qapi(discriminator)]
    #[qapi(added = "6.2.0")]
    pub r#type: Option<InputEventKind>,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<InputEventBranch>,
}

#[qapi(added = "2.0.0")]
pub enum InputEventBranch {
    #[qapi(name = "abs")]
    Abs(InputMoveEventWrapper),
    #[qapi(name = "btn")]
    Btn(InputBtnEventWrapper),
    #[qapi(name = "key")]
    Key(InputKeyEventWrapper),
    #[qapi(name = "mtt")]
    #[qapi(added = "8.1.0")]
    Mtt(InputMultiTouchEventWrapper),
    #[qapi(name = "rel")]
    Rel(InputMoveEventWrapper),
}

#[qapi(added = "6.2.0")]
pub enum InputEventKind {
    /// an absolute pointer motion input event
    #[qapi(name = "abs")]
    Abs,
    /// a pointer button input event
    #[qapi(name = "btn")]
    Btn,
    /// a keyboard input event
    #[qapi(name = "key")]
    Key,
    /// a multi-touch input event
    #[qapi(name = "mtt")]
    #[qapi(added = "8.1.0")]
    Mtt,
    /// a relative pointer motion input event
    #[qapi(name = "rel")]
    Rel,
}

#[qapi(added = "2.0.0")]
#[qapi(removed = "6.2.0")]
pub enum InputEventType {
    #[qapi(name = "abs")]
    Abs,
    #[qapi(name = "btn")]
    Btn,
    #[qapi(name = "key")]
    Key,
    #[qapi(name = "rel")]
    Rel,
}

/// Keyboard input event.
///
#[qapi(added = "2.0.0")]
pub struct InputKeyEvent {
    /// True for key-down and false for key-up events.
    pub down: bool,
    /// Which key this event is for.
    pub key: KeyValue,
}

#[qapi(added = "6.2.0")]
pub struct InputKeyEventWrapper {
    /// Keyboard input event
    pub data: InputKeyEvent,
}

/// Properties for input-linux objects.
///
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "6.0.0")]
pub struct InputLinuxProperties {
    /// the path of the host evdev device to use
    pub evdev: String,
    /// the key or key combination that toggles device grab
    /// (default: ctrl-ctrl)
    #[qapi(name = "grab-toggle")]
    pub grab_toggle: Option<GrabToggleKeys>,
    /// if true, grab is toggled for all devices (e.g. both
    /// keyboard and mouse) instead of just one device (default: false)
    pub grab_all: Option<bool>,
    /// enables auto-repeat events (default: false)
    pub repeat: Option<bool>,
}

/// Pointer motion input event.
///
#[qapi(added = "2.0.0")]
pub struct InputMoveEvent {
    /// Which axis is referenced by @value.
    pub axis: InputAxis,
    /// Pointer position.  For absolute coordinates the valid range
    /// is 0 to 0x7fff.
    pub value: i64,
}

#[qapi(added = "6.2.0")]
pub struct InputMoveEventWrapper {
    /// Pointer motion input event
    pub data: InputMoveEvent,
}

/// MultiTouch input event.
///
#[qapi(added = "8.1.0")]
pub struct InputMultiTouchEvent {
    /// Which axis is referenced by @value.
    pub axis: InputAxis,
    /// Which slot has generated the event.
    pub slot: i64,
    /// ID to correlate this event with previously generated
    /// events.
    #[qapi(name = "tracking-id")]
    pub tracking_id: i64,
    /// The type of multi-touch event.
    pub r#type: InputMultiTouchType,
    /// Contact position.
    pub value: i64,
}

#[qapi(added = "8.1.0")]
pub struct InputMultiTouchEventWrapper {
    /// MultiTouch input event
    pub data: InputMultiTouchEvent,
}

/// Type of a multi-touch event.
///
#[qapi(added = "8.1.0")]
pub enum InputMultiTouchType {
    /// A new touch event sequence has just started.
    #[qapi(name = "begin")]
    Begin,
    /// A touch event sequence has been canceled.
    #[qapi(name = "cancel")]
    Cancel,
    /// Absolute position data.
    #[qapi(name = "data")]
    Data,
    /// A touch event sequence has finished.
    #[qapi(name = "end")]
    End,
    /// A touch event sequence has been updated.
    #[qapi(name = "update")]
    Update,
}

#[qapi(added = "6.2.0")]
pub struct IntWrapper {
    /// a numeric key code
    pub data: i64,
}

/// Properties for iommufd objects.
///
#[qapi(name = "IOMMUFDProperties")]
#[qapi(added = "9.0.0")]
pub struct IommufdProperties {
    /// file descriptor name previously passed via `getfd` command,
    /// which represents a pre-opened /dev/iommu.  This allows the
    /// iommufd object to be shared across several subsystems (VFIO,
    /// VDPA, ...), and the file descriptor to be shared with other
    /// process, e.g. DPDK.  (default: QEMU opens /dev/iommu by itself)
    pub fd: Option<String>,
}

/// An enumeration of the I/O operation types
///
#[qapi(added = "2.1.0")]
pub enum IoOperationType {
    /// read operation
    #[qapi(name = "read")]
    Read,
    /// write operation
    #[qapi(name = "write")]
    Write,
}

/// Information about an iothread
///
#[qapi(name = "IOThreadInfo")]
#[qapi(added = "2.0.0")]
pub struct IoThreadInfo {
    /// maximum number of requests in a batch for the AIO
    /// engine, 0 means that the engine will use its default (since 6.1)
    #[qapi(name = "aio-max-batch")]
    #[qapi(added = "6.1.0")]
    pub aio_max_batch: Option<i64>,
    /// the identifier of the iothread
    pub id: String,
    /// how many ns will be added to polling time, 0 means that
    /// it's not configured (since 2.9)
    #[qapi(name = "poll-grow")]
    #[qapi(added = "2.9.0")]
    pub poll_grow: Option<i64>,
    /// maximum polling time in ns, 0 means polling is
    /// disabled (since 2.9)
    #[qapi(name = "poll-max-ns")]
    #[qapi(added = "2.9.0")]
    pub poll_max_ns: Option<i64>,
    /// how many ns will be removed from polling time, 0 means
    /// that it's not configured (since 2.9)
    #[qapi(name = "poll-shrink")]
    #[qapi(added = "2.9.0")]
    pub poll_shrink: Option<i64>,
    /// the weight factor for adaptive polling.
    /// Determines how much the current event interval contributes to
    /// the next polling time calculation.  Valid values are 1 or
    /// greater (since 11.1)
    #[qapi(name = "poll-weight")]
    #[qapi(added = "11.1.0")]
    pub poll_weight: Option<i64>,
    /// ID of the underlying host thread
    #[qapi(name = "thread-id")]
    pub thread_id: i64,
}

/// Properties for iothread objects.
///
#[qapi(added = "6.0.0")]
pub struct IothreadProperties {
    /// maximum number of requests in a batch for the AIO
    /// engine, 0 means that the engine will use its default.
    /// (default: 0)
    #[qapi(name = "aio-max-batch")]
    #[qapi(added = "6.1.0")]
    pub aio_max_batch: Option<i64>,
    /// the multiplier used to increase the polling time when
    /// the algorithm detects it is missing events due to not polling
    /// long enough.  0 selects a default behaviour (default: 0)
    #[qapi(name = "poll-grow")]
    pub poll_grow: Option<i64>,
    /// the maximum number of nanoseconds to busy wait for
    /// events.  0 means polling is disabled (default: 32768 on POSIX
    /// hosts, 0 otherwise)
    #[qapi(name = "poll-max-ns")]
    pub poll_max_ns: Option<i64>,
    /// the divisor used to decrease the polling time when the
    /// algorithm detects it is spending too long polling without
    /// encountering events.  0 selects a default behaviour (default: 0)
    #[qapi(name = "poll-shrink")]
    pub poll_shrink: Option<i64>,
    /// the weight factor for adaptive polling.  Determines
    /// how much the most recent event interval affects the next
    /// polling duration calculation.  If set to 0, the system default
    /// value of 3 is used.  Typical values: 1 (high weight on recent
    /// interval), 2-4 (moderate weight on recent interval).
    /// (default: 0) (since 11.1)
    ///
    /// The @aio-max-batch option is available since 6.1.
    #[qapi(name = "poll-weight")]
    #[qapi(added = "11.1.0")]
    pub poll_weight: Option<i64>,
    /// maximum number of threads the thread pool can
    /// contain (default:64)
    #[qapi(name = "thread-pool-max")]
    #[qapi(added = "7.1.0")]
    pub thread_pool_max: Option<i64>,
    /// minimum number of threads reserved in the thread
    /// pool (default:0)
    #[qapi(name = "thread-pool-min")]
    #[qapi(added = "7.1.0")]
    pub thread_pool_min: Option<i64>,
}

/// Describes the subset of virtqueues assigned to an IOThread.
///
#[qapi(name = "IOThreadVirtQueueMapping")]
#[qapi(added = "9.0.0")]
pub struct IoThreadVirtQueueMapping {
    /// the id of IOThread object
    pub iothread: String,
    /// an optional array of virtqueue indices that will be handled by
    /// this IOThread.  When absent, virtqueues are assigned round-robin
    /// across all IOThreadVirtQueueMappings provided.  Either all
    /// IOThreadVirtQueueMappings must have @vqs or none of them must
    /// have it.
    pub vqs: Option<Vec<u16>>,
}

/// An enumeration of header digests supported by libiscsi
///
#[qapi(added = "2.9.0")]
pub enum IscsiHeaderDigest {
    #[qapi(name = "crc32c")]
    Crc32c,
    #[qapi(name = "crc32c-none")]
    Crc32cNone,
    #[qapi(name = "none")]
    None,
    #[qapi(name = "none-crc32c")]
    NoneCrc32c,
}

/// An enumeration of libiscsi transport types
///
#[qapi(added = "2.9.0")]
pub enum IscsiTransport {
    #[qapi(name = "iser")]
    Iser,
    #[qapi(name = "tcp")]
    Tcp,
}

/// Information about a job.
///
#[qapi(added = "3.0.0")]
pub struct JobInfo {
    /// to @total-progress.  The value is
    /// monotonically increasing.
    #[qapi(name = "current-progress")]
    pub current_progress: i64,
    /// If this field is present, the job failed; if it is still
    /// missing in the CONCLUDED state, this indicates successful
    /// completion.
    ///
    /// The value is a human-readable error message to describe the
    /// reason for the job failure.  It should not be parsed by
    /// applications.
    pub error: Option<String>,
    /// The job identifier
    pub id: String,
    /// Current job state/status
    pub status: JobStatus,
    /// Estimated @current-progress value at the completion
    /// of the job.  This value can arbitrarily change while the job is
    /// running, in both directions.
    #[qapi(name = "total-progress")]
    pub total_progress: i64,
    /// The kind of job that is being performed
    pub r#type: JobType,
}

/// Indicates the present state of a given job in its lifetime.
///
#[qapi(added = "3.0.0")]
pub enum JobStatus {
    /// The job is in the process of being aborted, and will
    /// finish with an error.  The job will afterwards report that it is
    #[qapi(name = "aborting")]
    Aborting,
    /// The job has finished all work.  If auto-dismiss was set
    /// to false, the job will remain in this state until it is
    /// dismissed via `job-dismiss`.
    #[qapi(name = "concluded")]
    Concluded,
    /// The job has been created, but not yet started.
    #[qapi(name = "created")]
    Created,
    /// The job is in the process of being dismantled.  This state
    /// should not ever be visible externally.
    #[qapi(name = "null")]
    Null,
    /// The job is running, but paused.  The pause may be requested
    /// by either the QMP user or by internal processes.
    ///
    ///   .: The job may return to @ready or otherwise be canceled.
    #[qapi(name = "paused")]
    Paused,
    /// The job has finished its work, but has finalization steps
    /// that it needs to make prior to completing.  These changes will
    /// require manual intervention via `job-finalize` if auto-finalize
    /// was set to false.  These pending changes may still fail.
    #[qapi(name = "pending")]
    Pending,
    /// The job is running, but is ready for the user to signal
    /// completion.  This is used for long-running jobs like mirror that
    /// are designed to run indefinitely.
    #[qapi(name = "ready")]
    Ready,
    /// The job is currently running.
    #[qapi(name = "running")]
    Running,
    /// The job is ready, but paused.  This is nearly identical to
    #[qapi(name = "standby")]
    Standby,
    /// Erroneous, default state.  Should not ever be visible.
    #[qapi(name = "undefined")]
    Undefined,
    /// The job is waiting for other jobs in the transaction to
    /// converge to the waiting state.  This status will likely not be
    /// visible for the last job in a transaction.
    #[qapi(name = "waiting")]
    Waiting,
}

/// Type of a background job.
///
#[qapi(added = "3.0.0")]
pub enum JobType {
    /// image options amend job type, see `x-blockdev-amend`
    /// (since 5.1)
    #[qapi(name = "amend")]
    #[qapi(added = "5.1.0")]
    Amend,
    /// drive backup job type, see `drive-backup`
    #[qapi(name = "backup")]
    Backup,
    /// block commit job type, see `block-commit`
    #[qapi(name = "commit")]
    Commit,
    /// image creation job type, see `blockdev-create` (since 3.0)
    #[qapi(name = "create")]
    Create,
    /// drive mirror job type, see `drive-mirror`
    #[qapi(name = "mirror")]
    Mirror,
    /// snapshot delete job type, see `snapshot-delete`
    /// (since 6.0)
    #[qapi(name = "snapshot-delete")]
    #[qapi(added = "6.0.0")]
    SnapshotDelete,
    /// snapshot load job type, see `snapshot-load`
    /// (since 6.0)
    #[qapi(name = "snapshot-load")]
    #[qapi(added = "6.0.0")]
    SnapshotLoad,
    /// snapshot save job type, see `snapshot-save`
    /// (since 6.0)
    #[qapi(name = "snapshot-save")]
    #[qapi(added = "6.0.0")]
    SnapshotSave,
    /// block stream job type, see `block-stream`
    #[qapi(name = "stream")]
    Stream,
}

/// Represents command verbs that can be applied to a job.
///
#[qapi(added = "3.0.0")]
pub enum JobVerb {
    /// see `job-cancel`
    #[qapi(name = "cancel")]
    Cancel,
    /// see `block-job-change` (since 8.2)
    #[qapi(name = "change")]
    #[qapi(added = "8.2.0")]
    Change,
    /// see `job-complete`
    #[qapi(name = "complete")]
    Complete,
    /// see `job-dismiss`
    #[qapi(name = "dismiss")]
    Dismiss,
    /// see `job-finalize`
    #[qapi(name = "finalize")]
    Finalize,
    /// see `job-pause`
    #[qapi(name = "pause")]
    Pause,
    /// see `job-resume`
    #[qapi(name = "resume")]
    Resume,
    /// see `block-job-set-speed`
    #[qapi(name = "set-speed")]
    SetSpeed,
}

/// The four primitive and two structured types according to RFC 8259
/// section 1, plus 'int' (split off 'number'), plus the obvious top
/// type 'value'.
///
#[qapi(name = "JSONType")]
#[qapi(added = "2.5.0")]
pub enum JsonType {
    /// JSON array
    #[qapi(name = "array")]
    Array,
    /// literal ``false`` or ``true``
    #[qapi(name = "boolean")]
    Boolean,
    /// JSON number that is an integer
    #[qapi(name = "int")]
    Int,
    /// literal ``null``
    #[qapi(name = "null")]
    Null,
    /// JSON number
    #[qapi(name = "number")]
    Number,
    /// JSON object
    #[qapi(name = "object")]
    Object,
    /// JSON string
    #[qapi(name = "string")]
    String,
    /// any JSON value
    #[qapi(name = "value")]
    Value,
}

/// Represents a keyboard key.
///
#[qapi(added = "1.3.0")]
pub struct KeyValue {
    #[qapi(discriminator)]
    #[qapi(name = "type")]
    #[qapi(removed = "6.2.0")]
    pub r_type: Option<KeyValueType>,
    /// key encoding
    #[qapi(discriminator)]
    #[qapi(added = "6.2.0")]
    pub r#type: Option<KeyValueKind>,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<KeyValueBranch>,
}

#[qapi(added = "1.3.0")]
pub enum KeyValueBranch {
    #[qapi(name = "number")]
    Number(IntWrapper),
    #[qapi(name = "qcode")]
    Qcode(QKeyCodeWrapper),
}

#[qapi(added = "6.2.0")]
pub enum KeyValueKind {
    #[qapi(name = "number")]
    Number,
    #[qapi(name = "qcode")]
    Qcode,
}

#[qapi(added = "1.3.0")]
#[qapi(removed = "6.2.0")]
pub enum KeyValueType {
    #[qapi(name = "number")]
    Number,
    #[qapi(name = "qcode")]
    Qcode,
}

/// Information about support for KVM acceleration
///
#[qapi(added = "1.0.0")]
pub struct KvmInfo {
    /// true if KVM acceleration is active
    pub enabled: bool,
    /// true if KVM acceleration is built into this executable
    pub present: bool,
}

/// Policy for handling lost ticks in timer devices.  Ticks end up
/// getting lost when, for example, the guest is paused.
///
#[qapi(added = "2.0.0")]
pub enum LostTickPolicy {
    /// continue to deliver ticks at the normal rate.  The guest OS
    /// will not notice anything is amiss, as from its point of view
    /// time will have continued to flow normally.  The time in the
    /// guest should now be behind the time in the host by exactly the
    /// amount of time during which ticks have been missed.
    #[qapi(name = "delay")]
    Delay,
    /// throw away the missed ticks and continue with future
    /// injection normally.  The guest OS will see the timer jump ahead
    /// by a potentially quite significant amount all at once, as if the
    /// intervening chunk of time had simply not existed; needless to
    /// say, such a sudden jump can easily confuse a guest OS which is
    /// not specifically prepared to deal with it.  Assuming the guest
    /// OS can deal correctly with the time jump, the time in the guest
    /// and in the host should now match.
    #[qapi(name = "discard")]
    Discard,
    /// merge the missed tick(s) into one tick and inject.  Guest time
    /// may be delayed, depending on how the OS reacts to the merging
    /// of ticks
    #[qapi(name = "merge")]
    #[qapi(removed = "4.1.0")]
    Merge,
    /// deliver ticks at a higher rate to catch up with the missed
    /// ticks.  The guest OS will not notice anything is amiss, as from
    /// its point of view time will have continued to flow normally.
    /// Once the timer has managed to catch up with all the missing
    /// ticks, the time in the guest and in the host should match.
    #[qapi(name = "slew")]
    Slew,
}

/// Information describing a machine.
///
#[qapi(added = "1.2.0")]
pub struct MachineInfo {
    /// machine type supports ACPI (since 8.0)
    #[qapi(added = "8.0.0")]
    pub acpi: Option<bool>,
    /// an alias for the machine name
    pub alias: Option<String>,
    #[qapi(name = "compat-props")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "9.1.0")]
    pub compat_props: Option<Vec<CompatProperty>>,
    /// maximum number of CPUs supported by the machine type
    /// (since 1.5)
    #[qapi(name = "cpu-max")]
    #[qapi(added = "1.5.0")]
    pub cpu_max: Option<i64>,
    /// default CPU model typename if none is requested
    /// via the -cpu argument.  (since 4.2)
    #[qapi(name = "default-cpu-type")]
    #[qapi(added = "4.2.0")]
    pub default_cpu_type: Option<String>,
    /// the default ID of initial RAM memory backend (since
    /// 5.2)
    #[qapi(name = "default-ram-id")]
    #[qapi(added = "5.2.0")]
    pub default_ram_id: Option<String>,
    /// if true, the machine type is deprecated and may be
    /// removed in future versions of QEMU according to the QEMU
    /// deprecation policy (since 4.1)
    #[qapi(added = "4.1.0")]
    pub deprecated: Option<bool>,
    /// cpu hotplug via -device is supported (since 2.7)
    #[qapi(name = "hotpluggable-cpus")]
    #[qapi(added = "2.7.0")]
    pub hotpluggable_cpus: Option<bool>,
    /// whether the machine is default
    #[qapi(name = "is-default")]
    pub is_default: Option<bool>,
    /// the name of the machine
    pub name: String,
    /// true if '-numa node,mem' option is supported by
    /// the machine type and false otherwise (since 4.1)
    #[qapi(name = "numa-mem-supported")]
    #[qapi(added = "4.1.0")]
    pub numa_mem_supported: Option<bool>,
}

/// Properties for the main-loop object.
///
#[qapi(added = "7.1.0")]
pub struct MainLoopProperties {
    /// maximum number of requests in a batch for the AIO
    /// engine, 0 means that the engine will use its default.
    /// (default: 0)
    #[qapi(name = "aio-max-batch")]
    pub aio_max_batch: Option<i64>,
    /// maximum number of threads the thread pool can
    /// contain (default:64)
    #[qapi(name = "thread-pool-max")]
    pub thread_pool_max: Option<i64>,
    /// minimum number of threads reserved in the thread
    /// pool (default:0)
    #[qapi(name = "thread-pool-min")]
    pub thread_pool_min: Option<i64>,
}

/// Mapping information from a virtual block range to a host file range
///
#[qapi(added = "2.6.0")]
pub struct MapEntry {
    /// true if the data is stored compressed (since 8.2)
    #[qapi(added = "8.2.0")]
    pub compressed: Option<bool>,
    /// reading the image will actually read data from a file (in
    /// particular, if @offset is present this means that the sectors
    /// are not simply preallocated, but contain actual data in raw
    /// format)
    pub data: bool,
    /// number of layers (0 = top image, 1 = top image's backing
    /// file, ..., n - 1 = bottom image (where n is the number of images
    /// in the chain)) before reaching one for which the range is
    /// allocated
    pub depth: i64,
    /// filename that is referred to by @offset
    pub filename: Option<String>,
    /// the number of bytes of the mapped virtual range
    pub length: i64,
    /// if present, the image file stores the data for this range
    /// in raw format at the given (host) offset
    pub offset: Option<i64>,
    /// true if this layer provides the data, false if adding a
    /// backing layer could impact this region (since 6.1)
    #[qapi(added = "6.1.0")]
    pub present: Option<bool>,
    /// virtual (guest) offset of the first byte described by this
    /// entry
    pub start: i64,
    /// whether the virtual blocks read as zeroes
    pub zero: bool,
}

/// Information about memory backend
///
#[qapi(added = "2.1.0")]
pub struct Memdev {
    /// whether memory backend's memory is included in a core dump
    pub dump: bool,
    /// host nodes for its memory policy
    #[qapi(name = "host-nodes")]
    pub host_nodes: Vec<u16>,
    /// backend's ID if backend has 'id' property (since 2.9)
    #[qapi(added = "2.9.0")]
    pub id: Option<String>,
    /// whether memory merge support is enabled
    pub merge: bool,
    /// memory policy of memory backend
    pub policy: HostMemPolicy,
    /// whether memory was preallocated
    pub prealloc: bool,
    /// whether swap space (or huge pages) was reserved if
    /// applicable.  This corresponds to the user configuration and not
    /// the actual behavior implemented in the OS to perform the
    /// reservation.  For example, Linux will never reserve swap space
    /// for shared file mappings.  (since 6.1)
    #[qapi(added = "6.1.0")]
    pub reserve: Option<bool>,
    /// whether memory is private to QEMU or shared (since 6.1)
    #[qapi(added = "6.1.0")]
    pub share: Option<bool>,
    /// memory backend size
    pub size: u64,
}

/// Properties for memory-backend-epc objects.
///
/// The @merge boolean option is false by default with epc
///
/// The @dump boolean option is false by default with epc
///
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "6.2.0")]
pub struct MemoryBackendEpcProperties {
    /// if true, include the memory in core dumps (default depends on
    /// the machine type)
    pub dump: Option<bool>,
    /// the list of NUMA host nodes to bind the memory to
    #[qapi(name = "host-nodes")]
    pub host_nodes: Option<Vec<u16>>,
    /// if true, mark the memory as mergeable (default depends on
    /// the machine type)
    pub merge: Option<bool>,
    /// the NUMA policy (default: 'default')
    pub policy: Option<HostMemPolicy>,
    /// if true, preallocate memory (default: false)
    pub prealloc: Option<bool>,
    /// thread context to use for creation of
    /// preallocation threads (default: none) (since 7.2)
    #[qapi(name = "prealloc-context")]
    #[qapi(added = "7.2.0")]
    pub prealloc_context: Option<String>,
    /// number of CPU threads to use for prealloc
    /// (default: 1)
    #[qapi(name = "prealloc-threads")]
    pub prealloc_threads: Option<u32>,
    /// if true, reserve swap space (or huge pages) if applicable
    /// (default: true) (since 6.1)
    pub reserve: Option<bool>,
    /// if false, the memory is private to QEMU; if true, it is
    /// shared (default false for backends memory-backend-file and
    /// memory-backend-ram, true for backends memory-backend-epc,
    /// memory-backend-memfd, and memory-backend-shm)
    pub share: Option<bool>,
    /// size of the memory region in bytes
    pub size: u64,
    /// if true, the canonical path
    /// is used for ramblock-id.  Disable this for 4.0 machine types or
    /// older to allow migration with newer QEMU versions.
    /// (default: false generally, but true for machine types <= 4.0)
    ///
    /// .. note:: prealloc=true and reserve=false cannot be set at the same
    /// time.  With reserve=true, the behavior depends on the operating
    /// system: for example, Linux will not reserve swap space for shared
    /// file mappings -- "not applicable".  In contrast, reserve=false
    /// will bail out if it cannot be configured accordingly.
    #[qapi(name = "x-use-canonical-path-for-ramblock-id")]
    pub x_use_canonical_path_for_ramblock_id: Option<bool>,
}

/// Properties for memory-backend-file objects.
///
#[qapi(added = "6.0.0")]
pub struct MemoryBackendFileProperties {
    /// the base address alignment when QEMU mmap(2)s @mem-path. Some
    /// backend stores specified by @mem-path require an alignment different
    /// than the default one used by QEMU, e.g. the device DAX /dev/dax0.0
    /// requires 2M alignment rather than 4K. In such cases, users can
    /// specify the required alignment via this option.
    /// 0 selects a default alignment (currently the page size). (default: 0)
    pub align: Option<u64>,
    /// if true, the file contents can be destroyed when QEMU exits,
    /// to avoid unnecessarily flushing data to the backing file. Note
    /// that ``discard-data`` is only an optimization, and QEMU might
    /// not discard file contents if it aborts unexpectedly or is
    /// terminated using SIGKILL. (default: false)
    #[qapi(name = "discard-data")]
    pub discard_data: Option<bool>,
    /// if true, include the memory in core dumps (default depends on
    /// the machine type)
    pub dump: Option<bool>,
    /// the list of NUMA host nodes to bind the memory to
    #[qapi(name = "host-nodes")]
    pub host_nodes: Option<Vec<u16>>,
    /// the path to either a shared memory or huge page filesystem mount
    #[qapi(name = "mem-path")]
    pub mem_path: String,
    /// if true, mark the memory as mergeable (default depends on
    /// the machine type)
    pub merge: Option<bool>,
    #[qapi(added = "8.1.0")]
    pub offset: Option<u64>,
    /// specifies whether the backing file specified by @mem-path is in
    /// host persistent memory that can be accessed using the SNIA NVM
    /// programming model (e.g. Intel NVDIMM).
    #[qapi(condition = "defined(CONFIG_LIBPMEM)")]
    pub pmem: Option<bool>,
    /// the NUMA policy (default: 'default')
    pub policy: Option<HostMemPolicy>,
    /// if true, preallocate memory (default: false)
    pub prealloc: Option<bool>,
    /// thread context to use for creation of
    /// preallocation threads (default: none) (since 7.2)
    #[qapi(name = "prealloc-context")]
    #[qapi(added = "7.2.0")]
    pub prealloc_context: Option<String>,
    /// number of CPU threads to use for prealloc
    /// (default: 1)
    #[qapi(name = "prealloc-threads")]
    pub prealloc_threads: Option<u32>,
    /// if true, the backing file is opened read-only; if false, it is
    /// opened read-write. (default: false)
    pub readonly: Option<bool>,
    /// if true, reserve swap space (or huge pages) if applicable
    /// (default: true) (since 6.1)
    #[qapi(added = "6.1.0")]
    pub reserve: Option<bool>,
    #[qapi(added = "8.2.0")]
    pub rom: Option<OnOffAuto>,
    /// if false, the memory is private to QEMU; if true, it is
    /// shared (default false for backends memory-backend-file and
    /// memory-backend-ram, true for backends memory-backend-epc,
    /// memory-backend-memfd, and memory-backend-shm)
    pub share: Option<bool>,
    /// size of the memory region in bytes
    pub size: u64,
    /// if true, the canonical path
    /// is used for ramblock-id.  Disable this for 4.0 machine types or
    /// older to allow migration with newer QEMU versions.
    /// (default: false generally, but true for machine types <= 4.0)
    ///
    /// .. note:: prealloc=true and reserve=false cannot be set at the same
    /// time.  With reserve=true, the behavior depends on the operating
    /// system: for example, Linux will not reserve swap space for shared
    /// file mappings -- "not applicable".  In contrast, reserve=false
    /// will bail out if it cannot be configured accordingly.
    #[qapi(name = "x-use-canonical-path-for-ramblock-id")]
    pub x_use_canonical_path_for_ramblock_id: Option<bool>,
}

/// Properties for memory-backend-memfd objects.
///
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "6.0.0")]
pub struct MemoryBackendMemfdProperties {
    /// if true, include the memory in core dumps (default depends on
    /// the machine type)
    pub dump: Option<bool>,
    /// the list of NUMA host nodes to bind the memory to
    #[qapi(name = "host-nodes")]
    pub host_nodes: Option<Vec<u16>>,
    /// if true, the file to be created resides in the hugetlbfs
    /// filesystem (default: false)
    pub hugetlb: Option<bool>,
    /// the hugetlb page size on systems that support multiple
    /// hugetlb page sizes (it must be a power of 2 value supported by
    /// the system).  0 selects a default page size.  This option is
    /// ignored if @hugetlb is false.  (default: 0)
    pub hugetlbsize: Option<u64>,
    /// if true, mark the memory as mergeable (default depends on
    /// the machine type)
    pub merge: Option<bool>,
    /// the NUMA policy (default: 'default')
    pub policy: Option<HostMemPolicy>,
    /// if true, preallocate memory (default: false)
    pub prealloc: Option<bool>,
    /// thread context to use for creation of
    /// preallocation threads (default: none) (since 7.2)
    #[qapi(name = "prealloc-context")]
    #[qapi(added = "7.2.0")]
    pub prealloc_context: Option<String>,
    /// number of CPU threads to use for prealloc
    /// (default: 1)
    #[qapi(name = "prealloc-threads")]
    pub prealloc_threads: Option<u32>,
    /// if true, reserve swap space (or huge pages) if applicable
    /// (default: true) (since 6.1)
    #[qapi(added = "6.1.0")]
    pub reserve: Option<bool>,
    /// if true, create a sealed-file, which will block further
    /// resizing of the memory (default: true)
    pub seal: Option<bool>,
    /// if false, the memory is private to QEMU; if true, it is
    /// shared (default false for backends memory-backend-file and
    /// memory-backend-ram, true for backends memory-backend-epc,
    /// memory-backend-memfd, and memory-backend-shm)
    pub share: Option<bool>,
    /// size of the memory region in bytes
    pub size: u64,
    /// if true, the canonical path
    /// is used for ramblock-id.  Disable this for 4.0 machine types or
    /// older to allow migration with newer QEMU versions.
    /// (default: false generally, but true for machine types <= 4.0)
    ///
    /// .. note:: prealloc=true and reserve=false cannot be set at the same
    /// time.  With reserve=true, the behavior depends on the operating
    /// system: for example, Linux will not reserve swap space for shared
    /// file mappings -- "not applicable".  In contrast, reserve=false
    /// will bail out if it cannot be configured accordingly.
    #[qapi(name = "x-use-canonical-path-for-ramblock-id")]
    pub x_use_canonical_path_for_ramblock_id: Option<bool>,
}

/// Properties for objects of classes derived from memory-backend.
///
#[qapi(added = "6.0.0")]
pub struct MemoryBackendProperties {
    /// if true, include the memory in core dumps (default depends on
    /// the machine type)
    pub dump: Option<bool>,
    /// the list of NUMA host nodes to bind the memory to
    #[qapi(name = "host-nodes")]
    pub host_nodes: Option<Vec<u16>>,
    /// if true, mark the memory as mergeable (default depends on
    /// the machine type)
    pub merge: Option<bool>,
    /// the NUMA policy (default: 'default')
    pub policy: Option<HostMemPolicy>,
    /// if true, preallocate memory (default: false)
    pub prealloc: Option<bool>,
    /// thread context to use for creation of
    /// preallocation threads (default: none) (since 7.2)
    #[qapi(name = "prealloc-context")]
    #[qapi(added = "7.2.0")]
    pub prealloc_context: Option<String>,
    /// number of CPU threads to use for prealloc
    /// (default: 1)
    #[qapi(name = "prealloc-threads")]
    pub prealloc_threads: Option<u32>,
    /// if true, reserve swap space (or huge pages) if applicable
    /// (default: true) (since 6.1)
    #[qapi(added = "6.1.0")]
    pub reserve: Option<bool>,
    /// if false, the memory is private to QEMU; if true, it is
    /// shared (default false for backends memory-backend-file and
    /// memory-backend-ram, true for backends memory-backend-epc,
    /// memory-backend-memfd, and memory-backend-shm)
    pub share: Option<bool>,
    /// size of the memory region in bytes
    pub size: u64,
    /// if true, the canonical path
    /// is used for ramblock-id.  Disable this for 4.0 machine types or
    /// older to allow migration with newer QEMU versions.
    /// (default: false generally, but true for machine types <= 4.0)
    ///
    /// .. note:: prealloc=true and reserve=false cannot be set at the same
    /// time.  With reserve=true, the behavior depends on the operating
    /// system: for example, Linux will not reserve swap space for shared
    /// file mappings -- "not applicable".  In contrast, reserve=false
    /// will bail out if it cannot be configured accordingly.
    #[qapi(name = "x-use-canonical-path-for-ramblock-id")]
    pub x_use_canonical_path_for_ramblock_id: Option<bool>,
}

/// Properties for memory-backend-shm objects.
///
/// This memory backend supports only shared memory, which is the
/// default.
///
#[qapi(condition = "CONFIG_POSIX")]
#[qapi(added = "9.1.0")]
pub struct MemoryBackendShmProperties {
    /// if true, include the memory in core dumps (default depends on
    /// the machine type)
    pub dump: Option<bool>,
    /// the list of NUMA host nodes to bind the memory to
    #[qapi(name = "host-nodes")]
    pub host_nodes: Option<Vec<u16>>,
    /// if true, mark the memory as mergeable (default depends on
    /// the machine type)
    pub merge: Option<bool>,
    /// the NUMA policy (default: 'default')
    pub policy: Option<HostMemPolicy>,
    /// if true, preallocate memory (default: false)
    pub prealloc: Option<bool>,
    /// thread context to use for creation of
    /// preallocation threads (default: none) (since 7.2)
    #[qapi(name = "prealloc-context")]
    pub prealloc_context: Option<String>,
    /// number of CPU threads to use for prealloc
    /// (default: 1)
    #[qapi(name = "prealloc-threads")]
    pub prealloc_threads: Option<u32>,
    /// if true, reserve swap space (or huge pages) if applicable
    /// (default: true) (since 6.1)
    pub reserve: Option<bool>,
    /// if false, the memory is private to QEMU; if true, it is
    /// shared (default false for backends memory-backend-file and
    /// memory-backend-ram, true for backends memory-backend-epc,
    /// memory-backend-memfd, and memory-backend-shm)
    pub share: Option<bool>,
    /// size of the memory region in bytes
    pub size: u64,
    /// if true, the canonical path
    /// is used for ramblock-id.  Disable this for 4.0 machine types or
    /// older to allow migration with newer QEMU versions.
    /// (default: false generally, but true for machine types <= 4.0)
    ///
    /// .. note:: prealloc=true and reserve=false cannot be set at the same
    /// time.  With reserve=true, the behavior depends on the operating
    /// system: for example, Linux will not reserve swap space for shared
    /// file mappings -- "not applicable".  In contrast, reserve=false
    /// will bail out if it cannot be configured accordingly.
    #[qapi(name = "x-use-canonical-path-for-ramblock-id")]
    pub x_use_canonical_path_for_ramblock_id: Option<bool>,
}

/// Union containing information about a memory device
///
#[qapi(added = "2.1.0")]
pub struct MemoryDeviceInfo {
    #[qapi(discriminator)]
    #[qapi(name = "type")]
    #[qapi(removed = "6.2.0")]
    pub r_type: Option<MemoryDeviceInfoType>,
    /// memory device type
    #[qapi(discriminator)]
    #[qapi(added = "6.2.0")]
    pub r#type: Option<MemoryDeviceInfoKind>,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<MemoryDeviceInfoBranch>,
}

#[qapi(added = "2.1.0")]
pub enum MemoryDeviceInfoBranch {
    #[qapi(name = "dimm")]
    Dimm(PcdimmDeviceInfoWrapper),
    #[qapi(name = "hv-balloon")]
    #[qapi(added = "8.2.0")]
    HvBalloon(HvBalloonDeviceInfoWrapper),
    #[qapi(name = "nvdimm")]
    #[qapi(added = "2.12.0")]
    Nvdimm(PcdimmDeviceInfoWrapper),
    #[qapi(name = "sgx-epc")]
    #[qapi(added = "6.2.0")]
    SgxEpc(SgxEpcDeviceInfoWrapper),
    #[qapi(name = "sp-mem")]
    #[qapi(added = "11.1.0")]
    SpMem(SpMemDeviceInfoWrapper),
    #[qapi(name = "virtio-mem")]
    #[qapi(added = "5.1.0")]
    VirtioMem(VirtioMemDeviceInfoWrapper),
    #[qapi(name = "virtio-pmem")]
    #[qapi(added = "4.1.0")]
    VirtioPmem(VirtioPmemDeviceInfoWrapper),
}

#[qapi(added = "6.2.0")]
pub enum MemoryDeviceInfoKind {
    #[qapi(name = "dimm")]
    Dimm,
    /// since 8.2.
    #[qapi(name = "hv-balloon")]
    #[qapi(added = "8.2.0")]
    HvBalloon,
    /// since 2.12
    #[qapi(name = "nvdimm")]
    Nvdimm,
    /// since 6.2.
    #[qapi(name = "sgx-epc")]
    SgxEpc,
    /// since 11.1.
    #[qapi(name = "sp-mem")]
    #[qapi(added = "11.1.0")]
    SpMem,
    /// since 5.1
    #[qapi(name = "virtio-mem")]
    VirtioMem,
    /// since 4.1
    #[qapi(name = "virtio-pmem")]
    VirtioPmem,
}

#[qapi(added = "2.1.0")]
#[qapi(removed = "6.2.0")]
pub enum MemoryDeviceInfoType {
    #[qapi(name = "dimm")]
    Dimm,
    #[qapi(name = "nvdimm")]
    #[qapi(added = "2.12.0")]
    Nvdimm,
    #[qapi(name = "virtio-mem")]
    #[qapi(added = "5.1.0")]
    VirtioMem,
    #[qapi(name = "virtio-pmem")]
    #[qapi(added = "4.1.0")]
    VirtioPmem,
}

/// Actions taken by QEMU in response to a hardware memory failure.
///
#[qapi(added = "5.2.0")]
pub enum MemoryFailureAction {
    /// the failure is unrecoverable.  This occurs for
    /// action-required failures if the recipient is the hypervisor;
    /// QEMU will exit.
    #[qapi(name = "fatal")]
    Fatal,
    /// the memory failure could be ignored.  This will only be the
    /// case for action-optional failures.
    #[qapi(name = "ignore")]
    Ignore,
    /// memory failure occurred in guest memory, the guest enabled
    /// MCE handling mechanism, and QEMU could inject the MCE into the
    /// guest successfully.
    #[qapi(name = "inject")]
    Inject,
    /// the failure is unrecoverable but confined to the guest.
    /// This occurs if the recipient is a guest guest which is not ready
    /// to handle memory failures.
    #[qapi(name = "reset")]
    Reset,
}

/// Additional information on memory failures.
///
#[qapi(added = "5.2.0")]
pub struct MemoryFailureFlags {
    /// whether a memory failure event is action-required
    /// or action-optional (e.g. a failure during memory scrub).
    #[qapi(name = "action-required")]
    pub action_required: bool,
    /// whether the failure occurred while the previous failure
    /// was still in progress.
    pub recursive: bool,
}

/// Hardware memory failure occurs, handled by recipient.
///
#[qapi(added = "5.2.0")]
pub enum MemoryFailureRecipient {
    /// memory failure at guest memory,
    #[qapi(name = "guest")]
    Guest,
    /// memory failure at QEMU process address space.  (none
    /// guest memory, but used by QEMU itself).
    #[qapi(name = "hypervisor")]
    Hypervisor,
}

/// Actual memory information in bytes.
///
#[qapi(added = "2.11.0")]
pub struct MemoryInfo {
    /// size of "base" memory specified with command line
    /// option -m.
    #[qapi(name = "base-memory")]
    pub base_memory: u64,
    /// size of memory that can be hot-unplugged.  This
    /// field is omitted if target doesn't support memory hotplug (i.e.
    /// CONFIG_MEM_DEVICE not defined at build time).
    #[qapi(name = "plugged-memory")]
    pub plugged_memory: Option<u64>,
}

/// Schema for memory size configuration.
///
#[qapi(added = "7.1.0")]
pub struct MemorySizeConfiguration {
    /// maximum hotpluggable memory size in bytes
    #[qapi(name = "max-size")]
    pub max_size: Option<u64>,
    /// memory size in bytes
    pub size: Option<u64>,
    /// number of available memory slots for hotplug
    pub slots: Option<u64>,
}

#[qapi(added = "8.2.0")]
pub enum MigMode {
    #[qapi(name = "cpr-exec")]
    #[qapi(added = "10.2.0")]
    CprExec,
    #[qapi(name = "cpr-reboot")]
    CprReboot,
    #[qapi(name = "cpr-transfer")]
    #[qapi(added = "10.0.0")]
    CprTransfer,
    #[qapi(name = "normal")]
    Normal,
}

/// Migration endpoint configuration.
///
#[qapi(added = "8.2.0")]
pub struct MigrationAddress {
    /// The migration stream transport mechanism
    #[qapi(discriminator)]
    pub transport: MigrationAddressType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<MigrationAddressBranch>,
}

#[qapi(added = "8.2.0")]
pub enum MigrationAddressBranch {
    #[qapi(name = "exec")]
    Exec(MigrationExecCommand),
    #[qapi(name = "file")]
    File(FileMigrationArgs),
    #[qapi(name = "rdma")]
    Rdma(InetSocketAddress),
    #[qapi(name = "socket")]
    Socket(SocketAddress),
}

/// The migration stream transport mechanisms.
///
#[qapi(added = "8.2.0")]
pub enum MigrationAddressType {
    /// Direct the migration stream to another process.
    #[qapi(name = "exec")]
    Exec,
    /// Direct the migration stream to a file.
    #[qapi(name = "file")]
    File,
    /// Migrate via RDMA.
    #[qapi(name = "rdma")]
    Rdma,
    /// Migrate via socket.
    #[qapi(name = "socket")]
    Socket,
}

/// Migration capabilities enumeration
///
#[qapi(added = "1.2.0")]
pub enum MigrationCapability {
    /// If enabled, QEMU will automatically throttle down the guest
    /// to speed up convergence of RAM migration. (since 1.6)
    #[qapi(name = "auto-converge")]
    #[qapi(added = "1.6.0")]
    AutoConverge,
    /// If enabled, the migration stream will be a snapshot
    /// of the VM exactly at the point when the migration
    /// procedure starts. The VM RAM is saved with running VM.
    /// (since 6.0)
    #[qapi(name = "background-snapshot")]
    #[qapi(added = "6.0.0")]
    BackgroundSnapshot,
    /// If enabled, QEMU will also migrate the contents of all block
    /// devices.  Default is disabled.  A possible alternative uses
    /// mirror jobs to a builtin NBD server on the destination, which
    /// offers more flexibility.
    /// (Since 2.10)
    #[qapi(name = "block")]
    #[qapi(feature = "deprecated")]
    #[qapi(added = "2.10.0")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    Block,
    /// Use multiple compression threads to accelerate live migration.
    /// This feature can help to reduce the migration traffic, by sending
    /// compressed pages. Please note that if compress and xbzrle are both
    /// on, compress only takes effect in the ram bulk stage, after that,
    /// it will be disabled and only xbzrle takes effect, this can help to
    /// minimize migration traffic. The feature is disabled by default.
    /// (since 2.4 )
    #[qapi(name = "compress")]
    #[qapi(feature = "deprecated")]
    #[qapi(added = "2.4.0")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    Compress,
    /// If enabled, QEMU will migrate named dirty bitmaps.
    /// (since 2.12)
    #[qapi(name = "dirty-bitmaps")]
    #[qapi(added = "2.12.0")]
    DirtyBitmaps,
    #[qapi(name = "dirty-limit")]
    #[qapi(added = "8.1.0")]
    DirtyLimit,
    /// generate events for each migration state change
    /// (since 2.4 )
    #[qapi(name = "events")]
    #[qapi(added = "2.4.0")]
    Events,
    /// If enabled, the destination will not activate block
    /// devices (and thus take locks) immediately at the end of migration.
    /// (since 3.0)
    #[qapi(name = "late-block-activate")]
    #[qapi(added = "3.0.0")]
    LateBlockActivate,
    #[qapi(name = "mapped-ram")]
    #[qapi(added = "9.0.0")]
    MappedRam,
    /// Use more than one fd for migration (since 4.0)
    #[qapi(name = "multifd")]
    #[qapi(added = "4.0.0")]
    Multifd,
    /// Pause outgoing migration before serialising device
    /// state and before disabling block IO (since 2.11)
    #[qapi(name = "pause-before-switchover")]
    #[qapi(added = "2.11.0")]
    PauseBeforeSwitchover,
    /// Calculate downtime for postcopy live migration
    /// (since 3.0)
    #[qapi(name = "postcopy-blocktime")]
    #[qapi(added = "3.0.0")]
    PostcopyBlocktime,
    #[qapi(name = "postcopy-preempt")]
    #[qapi(added = "7.1.0")]
    PostcopyPreempt,
    /// Start executing on the migration target before all of RAM has
    /// been migrated, pulling the remaining pages along as needed. The
    /// capacity must have the same setting on both source and target
    /// or migration will not even start. NOTE: If the migration fails during
    /// postcopy the VM will fail.  (since 2.6)
    #[qapi(name = "postcopy-ram")]
    #[qapi(added = "2.6.0")]
    PostcopyRam,
    /// Controls whether or not the entire VM memory footprint is
    /// mlock()'d on demand or all at once. Refer to docs/rdma.txt for usage.
    /// Disabled by default. (since 2.0)
    #[qapi(name = "rdma-pin-all")]
    #[qapi(added = "2.0.0")]
    RdmaPinAll,
    /// if enabled, qemu will free the migrated ram pages on the source
    /// during postcopy-ram migration. (since 2.9)
    #[qapi(name = "release-ram")]
    #[qapi(added = "2.9.0")]
    ReleaseRam,
    /// If enabled, migration will use the return path even
    /// for precopy. (since 2.10)
    #[qapi(name = "return-path")]
    #[qapi(added = "2.10.0")]
    ReturnPath,
    #[qapi(name = "switchover-ack")]
    #[qapi(added = "8.1.0")]
    SwitchoverAck,
    /// Send the UUID of the source to allow the destination
    /// to ensure it is the same. (since 4.2)
    #[qapi(name = "validate-uuid")]
    #[qapi(added = "4.2.0")]
    ValidateUuid,
    /// If enabled, migration will never end, and the state of the VM on the
    /// primary side will be migrated continuously to the VM on secondary
    /// side, this process is called COarse-Grain LOck Stepping (COLO) for
    /// Non-stop Service. (since 2.8)
    #[qapi(name = "x-colo")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "2.8.0")]
    XColo,
    /// If enabled, QEMU will not migrate shared memory (since 4.0)
    #[qapi(name = "x-ignore-shared")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "4.0.0")]
    XIgnoreShared,
    /// Use more than one fd for migration (since 2.11)
    #[qapi(name = "x-multifd")]
    #[qapi(added = "2.11.0")]
    #[qapi(removed = "4.0.0")]
    XMultifd,
    /// Start executing on the migration target before all of RAM has
    /// been migrated, pulling the remaining pages along as needed. NOTE: If
    /// the migration fails during postcopy the VM will fail.  (since 2.5)
    #[qapi(name = "x-postcopy-ram")]
    #[qapi(added = "2.5.0")]
    #[qapi(removed = "2.6.0")]
    XPostcopyRam,
    /// Controls whether or not the entire VM memory footprint is
    /// mlock()'d on demand or all at once. Refer to docs/rdma.txt for usage.
    /// Disabled by default. Experimental: may (or may not) be renamed after
    /// further testing is complete. (since 1.6)
    #[qapi(name = "x-rdma-pin-all")]
    #[qapi(added = "1.6.0")]
    #[qapi(removed = "2.0.0")]
    XRdmaPinAll,
    /// Migration supports xbzrle (Xor Based Zero Run Length Encoding).
    /// This feature allows us to minimize migration traffic for certain work
    /// loads, by sending compressed difference of the pages
    #[qapi(name = "xbzrle")]
    Xbzrle,
    /// During storage migration encode blocks of zeroes efficiently. This
    /// essentially saves 1MB of zeroes per block on the wire. Enabling requires
    /// source and target VM to support this feature. To enable it is sufficient
    /// to enable the capability on the source VM. The feature is disabled by
    /// default. (since 1.6)
    #[qapi(name = "zero-blocks")]
    #[qapi(feature = "deprecated")]
    #[qapi(added = "1.6.0")]
    #[qapi(deprecated(added = "9.2.0"))]
    #[qapi(removed = "11.0.0")]
    ZeroBlocks,
    #[qapi(name = "zero-copy-send")]
    #[qapi(added = "7.1.0")]
    ZeroCopySend,
}

/// Migration capability information
///
#[qapi(added = "1.2.0")]
pub struct MigrationCapabilityStatus {
    /// capability enum
    pub capability: MigrationCapability,
    /// capability state bool
    pub state: bool,
}

/// Migration stream channel parameters.
///
#[qapi(added = "8.2.0")]
pub struct MigrationChannel {
    /// Migration endpoint configuration on destination interface.
    pub addr: MigrationAddress,
    /// Channel type for transferring packet information.
    #[qapi(name = "channel-type")]
    pub channel_type: MigrationChannelType,
}

/// The migration channel-type request options.
///
#[qapi(added = "8.2.0")]
pub enum MigrationChannelType {
    /// Checkpoint and restart state channel.
    #[qapi(name = "cpr")]
    #[qapi(added = "10.0.0")]
    Cpr,
    /// Main outbound migration channel.
    #[qapi(name = "main")]
    Main,
}

#[qapi(added = "8.2.0")]
pub struct MigrationExecCommand {
    /// command (list head) and arguments to execute.
    pub args: Vec<String>,
}

/// Information about current migration process.
///
#[qapi(added = "1.0.0")]
pub struct MigrationInfo {
    #[qapi(feature = "deprecated")]
    #[qapi(added = "6.0.0")]
    #[qapi(removed = "6.1.0")]
    pub blocked: Option<bool>,
    /// A list of reasons an outgoing migration is
    /// blocked.  Present and non-empty when migration is blocked.
    /// (since 6.0)
    #[qapi(name = "blocked-reasons")]
    #[qapi(added = "6.0.0")]
    pub blocked_reasons: Option<Vec<String>>,
    /// migration compression statistics, only returned if
    /// compression feature is on and status is 'active' or 'completed'
    /// (Since 3.1)
    #[qapi(feature = "deprecated")]
    #[qapi(added = "3.1.0")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub compression: Option<CompressionStats>,
    /// percentage of time guest cpus are being
    /// throttled during auto-converge.  This is only present when
    /// auto-converge has started throttling guest cpus.  (Since 2.7)
    #[qapi(name = "cpu-throttle-percentage")]
    #[qapi(added = "2.7.0")]
    pub cpu_throttle_percentage: Option<i64>,
    /// Estimated average dirty ring full time
    /// (in microseconds) for each dirty ring full round.  The value
    /// equals the dirty ring memory size divided by the average dirty
    /// page rate of the virtual CPU, which can be used to observe the
    /// average memory load of the virtual CPU indirectly.  Note that
    /// zero means guest doesn't dirty memory.  (Since 8.1)
    #[qapi(name = "dirty-limit-ring-full-time")]
    #[qapi(added = "8.1.0")]
    pub dirty_limit_ring_full_time: Option<u64>,
    /// Maximum throttle time (in
    /// microseconds) of virtual CPUs each dirty ring full round, which
    /// shows how MigrationCapability dirty-limit affects the guest
    /// during live migration.  (Since 8.1)
    #[qapi(name = "dirty-limit-throttle-time-per-round")]
    #[qapi(added = "8.1.0")]
    pub dirty_limit_throttle_time_per_round: Option<u64>,
    /// @MigrationStats containing detailed disk migration status,
    /// only returned if status is 'active' and it is a block migration
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub disk: Option<MigrationStats>,
    /// only present when migration finishes correctly total
    /// downtime in milliseconds for the guest.  (since 1.3)
    #[qapi(added = "1.3.0")]
    pub downtime: Option<i64>,
    /// the human readable error description string.  Clients
    /// should not attempt to parse the error strings.  (Since 2.7)
    #[qapi(name = "error-desc")]
    #[qapi(added = "2.7.0")]
    pub error_desc: Option<String>,
    /// only present while migration is active expected
    /// downtime in milliseconds for the guest in last walk of the dirty
    /// bitmap.  (since 1.3)
    #[qapi(name = "expected-downtime")]
    #[qapi(added = "1.3.0")]
    pub expected_downtime: Option<i64>,
    /// total time when all vCPU were blocked during
    /// postcopy live migration.  This is only present when the
    /// postcopy-blocktime migration capability is enabled.  (Since 3.0)
    #[qapi(name = "postcopy-blocktime")]
    #[qapi(added = "3.0.0")]
    pub postcopy_blocktime: Option<u32>,
    #[qapi(name = "postcopy-latency")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "10.1.0")]
    pub postcopy_latency: Option<u64>,
    #[qapi(name = "postcopy-latency-dist")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "10.1.0")]
    pub postcopy_latency_dist: Option<Vec<u64>>,
    #[qapi(name = "postcopy-non-vcpu-latency")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "10.1.0")]
    pub postcopy_non_vcpu_latency: Option<u64>,
    /// list of the postcopy blocktime per vCPU.
    /// This is only present when the postcopy-blocktime migration
    /// capability is enabled.  (Since 3.0)
    #[qapi(name = "postcopy-vcpu-blocktime")]
    #[qapi(added = "3.0.0")]
    pub postcopy_vcpu_blocktime: Option<Vec<u32>>,
    #[qapi(name = "postcopy-vcpu-latency")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "10.1.0")]
    pub postcopy_vcpu_latency: Option<Vec<u64>>,
    /// @MigrationStats containing detailed migration status, only
    /// returned if status is 'active' or 'completed'(since 1.2)
    pub ram: Option<MigrationRamStats>,
    #[qapi(added = "11.1.0")]
    pub remaining: Option<u64>,
    /// amount of setup time in milliseconds *before* the
    /// iterations begin but *after* the QMP command is issued.  This is
    /// designed to provide an accounting of any activities (such as
    /// RDMA pinning) which may be expensive, but do not actually occur
    /// during the iterative migration rounds themselves.  (since 1.6)
    #[qapi(name = "setup-time")]
    #[qapi(added = "1.6.0")]
    pub setup_time: Option<i64>,
    /// Only used for tcp, to know what the real port is
    /// (Since 4.0)
    #[qapi(name = "socket-address")]
    #[qapi(added = "4.0.0")]
    pub socket_address: Option<Vec<SocketAddress>>,
    /// @MigrationStatus describing the current migration status.
    /// If this field is not returned, no migration process has been
    /// initiated
    pub status: Option<MigrationStatus>,
    /// total amount of milliseconds since migration started.
    /// If migration has ended, it returns the total migration time.
    /// (since 1.2)
    #[qapi(name = "total-time")]
    #[qapi(added = "1.2.0")]
    pub total_time: Option<i64>,
    /// @VfioStats containing detailed VFIO devices migration
    /// statistics, only returned if VFIO device is present, migration
    /// is supported by all VFIO devices and status is 'active' or
    /// 'completed' (since 5.2)
    #[qapi(added = "5.2.0")]
    pub vfio: Option<VfioStats>,
    /// optional percentage of time guest cpus are being
    /// throttled during auto-converge. This is only present when auto-converge
    /// has started throttling guest cpus. (Since 2.5)
    #[qapi(name = "x-cpu-throttle-percentage")]
    #[qapi(added = "2.5.0")]
    #[qapi(removed = "2.7.0")]
    pub x_cpu_throttle_percentage: Option<i64>,
    /// @XBZRLECacheStats containing detailed XBZRLE
    /// migration statistics, only returned if XBZRLE feature is on and
    /// status is 'active' or 'completed' (since 1.2)
    #[qapi(name = "xbzrle-cache")]
    #[qapi(added = "1.2.0")]
    pub xbzrle_cache: Option<XbzrleCacheStats>,
}

#[qapi(added = "2.4.0")]
pub enum MigrationParameter {
    #[qapi(name = "announce-initial")]
    #[qapi(added = "4.0.0")]
    AnnounceInitial,
    #[qapi(name = "announce-max")]
    #[qapi(added = "4.0.0")]
    AnnounceMax,
    #[qapi(name = "announce-rounds")]
    #[qapi(added = "4.0.0")]
    AnnounceRounds,
    #[qapi(name = "announce-step")]
    #[qapi(added = "4.0.0")]
    AnnounceStep,
    #[qapi(name = "avail-switchover-bandwidth")]
    #[qapi(added = "8.2.0")]
    AvailSwitchoverBandwidth,
    #[qapi(name = "block-bitmap-mapping")]
    #[qapi(added = "5.2.0")]
    BlockBitmapMapping,
    #[qapi(name = "block-incremental")]
    #[qapi(feature = "deprecated")]
    #[qapi(added = "2.10.0")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    BlockIncremental,
    #[qapi(name = "compress-level")]
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    CompressLevel,
    #[qapi(name = "compress-threads")]
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    CompressThreads,
    #[qapi(name = "compress-wait-thread")]
    #[qapi(feature = "deprecated")]
    #[qapi(added = "3.1.0")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    CompressWaitThread,
    #[qapi(name = "cpr-exec-command")]
    #[qapi(added = "10.2.0")]
    CprExecCommand,
    #[qapi(name = "cpu-throttle-increment")]
    #[qapi(added = "2.7.0")]
    CpuThrottleIncrement,
    #[qapi(name = "cpu-throttle-initial")]
    #[qapi(added = "2.7.0")]
    CpuThrottleInitial,
    #[qapi(name = "cpu-throttle-tailslow")]
    #[qapi(added = "5.1.0")]
    CpuThrottleTailslow,
    #[qapi(name = "decompress-threads")]
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    DecompressThreads,
    #[qapi(name = "direct-io")]
    #[qapi(added = "9.1.0")]
    DirectIo,
    #[qapi(name = "downtime-limit")]
    #[qapi(added = "2.8.0")]
    DowntimeLimit,
    #[qapi(name = "max-bandwidth")]
    #[qapi(added = "2.8.0")]
    MaxBandwidth,
    #[qapi(name = "max-cpu-throttle")]
    #[qapi(added = "3.1.0")]
    MaxCpuThrottle,
    #[qapi(name = "max-postcopy-bandwidth")]
    #[qapi(added = "3.0.0")]
    MaxPostcopyBandwidth,
    #[qapi(name = "mode")]
    #[qapi(added = "8.2.0")]
    Mode,
    #[qapi(name = "multifd-channels")]
    #[qapi(added = "4.0.0")]
    MultifdChannels,
    #[qapi(name = "multifd-compression")]
    #[qapi(added = "5.0.0")]
    MultifdCompression,
    #[qapi(name = "multifd-qatzip-level")]
    #[qapi(added = "9.2.0")]
    MultifdQatzipLevel,
    #[qapi(name = "multifd-zlib-level")]
    #[qapi(added = "5.0.0")]
    MultifdZlibLevel,
    #[qapi(name = "multifd-zstd-level")]
    #[qapi(added = "5.0.0")]
    MultifdZstdLevel,
    #[qapi(name = "throttle-trigger-threshold")]
    #[qapi(added = "5.0.0")]
    ThrottleTriggerThreshold,
    #[qapi(name = "tls-authz")]
    #[qapi(added = "4.0.0")]
    TlsAuthz,
    #[qapi(name = "tls-creds")]
    #[qapi(added = "2.7.0")]
    TlsCreds,
    #[qapi(name = "tls-hostname")]
    #[qapi(added = "2.7.0")]
    TlsHostname,
    #[qapi(name = "vcpu-dirty-limit")]
    #[qapi(added = "8.1.0")]
    VcpuDirtyLimit,
    #[qapi(name = "x-checkpoint-delay")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "2.8.0")]
    XCheckpointDelay,
    #[qapi(name = "x-cpu-throttle-increment")]
    #[qapi(added = "2.5.0")]
    #[qapi(removed = "2.7.0")]
    XCpuThrottleIncrement,
    #[qapi(name = "x-cpu-throttle-initial")]
    #[qapi(added = "2.5.0")]
    #[qapi(removed = "2.7.0")]
    XCpuThrottleInitial,
    #[qapi(name = "x-multifd-channels")]
    #[qapi(added = "2.11.0")]
    #[qapi(removed = "4.0.0")]
    XMultifdChannels,
    #[qapi(name = "x-multifd-page-count")]
    #[qapi(added = "2.11.0")]
    #[qapi(removed = "4.0.0")]
    XMultifdPageCount,
    #[qapi(name = "x-rdma-chunk-size")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "11.1.0")]
    XRdmaChunkSize,
    #[qapi(name = "x-vcpu-dirty-limit-period")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "8.1.0")]
    XVcpuDirtyLimitPeriod,
    #[qapi(name = "xbzrle-cache-size")]
    #[qapi(added = "2.11.0")]
    XbzrleCacheSize,
    #[qapi(name = "zero-page-detection")]
    #[qapi(added = "9.0.0")]
    ZeroPageDetection,
}

#[qapi(added = "2.4.0")]
pub struct MigrationParameters {
    #[qapi(name = "announce-initial")]
    #[qapi(added = "4.0.0")]
    pub announce_initial: Option<u64>,
    #[qapi(name = "announce-max")]
    #[qapi(added = "4.0.0")]
    pub announce_max: Option<u64>,
    #[qapi(name = "announce-rounds")]
    #[qapi(added = "4.0.0")]
    pub announce_rounds: Option<u64>,
    #[qapi(name = "announce-step")]
    #[qapi(added = "4.0.0")]
    pub announce_step: Option<u64>,
    #[qapi(name = "avail-switchover-bandwidth")]
    #[qapi(added = "8.2.0")]
    pub avail_switchover_bandwidth: Option<u64>,
    #[qapi(name = "block-bitmap-mapping")]
    #[qapi(added = "5.2.0")]
    pub block_bitmap_mapping: Option<Vec<BitmapMigrationNodeAlias>>,
    #[qapi(name = "block-incremental")]
    #[qapi(feature = "deprecated")]
    #[qapi(added = "2.10.0")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub block_incremental: Option<bool>,
    #[qapi(name = "compress-level")]
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub compress_level: Option<u8>,
    #[qapi(name = "compress-threads")]
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub compress_threads: Option<u8>,
    #[qapi(name = "compress-wait-thread")]
    #[qapi(feature = "deprecated")]
    #[qapi(added = "3.1.0")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub compress_wait_thread: Option<bool>,
    #[qapi(name = "cpr-exec-command")]
    #[qapi(added = "10.2.0")]
    pub cpr_exec_command: Option<Vec<String>>,
    #[qapi(name = "cpu-throttle-increment")]
    #[qapi(added = "2.7.0")]
    pub cpu_throttle_increment: Option<u8>,
    #[qapi(name = "cpu-throttle-initial")]
    #[qapi(added = "2.7.0")]
    pub cpu_throttle_initial: Option<u8>,
    #[qapi(name = "cpu-throttle-tailslow")]
    #[qapi(added = "5.1.0")]
    pub cpu_throttle_tailslow: Option<bool>,
    #[qapi(name = "decompress-threads")]
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "8.2.0"))]
    #[qapi(removed = "9.1.0")]
    pub decompress_threads: Option<u8>,
    #[qapi(name = "direct-io")]
    #[qapi(added = "9.1.0")]
    pub direct_io: Option<bool>,
    #[qapi(name = "downtime-limit")]
    #[qapi(added = "2.8.0")]
    pub downtime_limit: Option<u64>,
    #[qapi(name = "max-bandwidth")]
    #[qapi(added = "2.8.0")]
    pub max_bandwidth: Option<u64>,
    #[qapi(name = "max-cpu-throttle")]
    #[qapi(added = "3.1.0")]
    pub max_cpu_throttle: Option<u8>,
    #[qapi(name = "max-postcopy-bandwidth")]
    #[qapi(added = "3.0.0")]
    pub max_postcopy_bandwidth: Option<u64>,
    #[qapi(added = "8.2.0")]
    pub mode: Option<MigMode>,
    #[qapi(name = "multifd-channels")]
    #[qapi(added = "4.0.0")]
    pub multifd_channels: Option<u8>,
    #[qapi(name = "multifd-compression")]
    #[qapi(added = "5.0.0")]
    pub multifd_compression: Option<MultiFdCompression>,
    #[qapi(name = "multifd-qatzip-level")]
    #[qapi(added = "9.2.0")]
    pub multifd_qatzip_level: Option<u8>,
    #[qapi(name = "multifd-zlib-level")]
    #[qapi(added = "5.0.0")]
    pub multifd_zlib_level: Option<u8>,
    #[qapi(name = "multifd-zstd-level")]
    #[qapi(added = "5.0.0")]
    pub multifd_zstd_level: Option<u8>,
    #[qapi(name = "throttle-trigger-threshold")]
    #[qapi(added = "5.0.0")]
    pub throttle_trigger_threshold: Option<u8>,
    #[qapi(name = "tls-authz")]
    #[qapi(added = "4.0.0")]
    pub tls_authz: Option<StrOrNull>,
    #[qapi(name = "tls-creds")]
    #[qapi(added = "2.7.0")]
    pub tls_creds: Option<StrOrNull>,
    #[qapi(name = "tls-hostname")]
    #[qapi(added = "2.7.0")]
    pub tls_hostname: Option<StrOrNull>,
    #[qapi(name = "vcpu-dirty-limit")]
    #[qapi(added = "8.1.0")]
    pub vcpu_dirty_limit: Option<u64>,
    #[qapi(name = "x-checkpoint-delay")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "2.8.0")]
    pub x_checkpoint_delay: Option<u32>,
    #[qapi(name = "x-cpu-throttle-increment")]
    #[qapi(added = "2.5.0")]
    #[qapi(removed = "2.7.0")]
    pub x_cpu_throttle_increment: Option<i64>,
    #[qapi(name = "x-cpu-throttle-initial")]
    #[qapi(added = "2.5.0")]
    #[qapi(removed = "2.7.0")]
    pub x_cpu_throttle_initial: Option<i64>,
    #[qapi(name = "x-multifd-channels")]
    #[qapi(added = "2.11.0")]
    #[qapi(removed = "4.0.0")]
    pub x_multifd_channels: Option<u8>,
    #[qapi(name = "x-multifd-page-count")]
    #[qapi(added = "2.11.0")]
    #[qapi(removed = "4.0.0")]
    pub x_multifd_page_count: Option<u32>,
    #[qapi(name = "x-rdma-chunk-size")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "11.1.0")]
    pub x_rdma_chunk_size: Option<u64>,
    #[qapi(name = "x-vcpu-dirty-limit-period")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "8.1.0")]
    pub x_vcpu_dirty_limit_period: Option<u64>,
    #[qapi(name = "xbzrle-cache-size")]
    #[qapi(added = "2.11.0")]
    pub xbzrle_cache_size: Option<u64>,
    #[qapi(name = "zero-page-detection")]
    #[qapi(added = "9.0.0")]
    pub zero_page_detection: Option<ZeroPageDetection>,
}

/// Detailed migration status.
///
#[qapi(name = "MigrationRAMStats")]
#[qapi(added = "11.1.0")]
pub struct MigrationRamStats {
    /// number of pages dirtied by second by the guest
    /// (since 1.3)
    #[qapi(name = "dirty-pages-rate")]
    pub dirty_pages_rate: i64,
    /// number of times that dirty ram was synchronized
    /// (since 2.1)
    #[qapi(name = "dirty-sync-count")]
    pub dirty_sync_count: i64,
    /// Number of times dirty RAM
    /// synchronization could not avoid copying dirty pages.  This is
    /// between 0 and @dirty-sync-count * @multifd-channels.
    /// (since 7.1)
    #[qapi(name = "dirty-sync-missed-zero-copy")]
    pub dirty_sync_missed_zero_copy: u64,
    /// The number of bytes sent while the guest is paused
    /// (since 7.0).
    #[qapi(name = "downtime-bytes")]
    pub downtime_bytes: u64,
    /// number of duplicate (zero) pages (since 1.2)
    pub duplicate: i64,
    /// throughput in megabits/sec.  (since 1.6)
    pub mbps: f64,
    /// The number of bytes sent through multifd (since 3.0)
    #[qapi(name = "multifd-bytes")]
    pub multifd_bytes: u64,
    /// number of normal pages (since 1.2)
    pub normal: i64,
    /// number of normal bytes sent (since 1.2)
    #[qapi(name = "normal-bytes")]
    pub normal_bytes: i64,
    /// The number of bytes per page for the various page-based
    /// statistics (since 2.10)
    #[qapi(name = "page-size")]
    pub page_size: i64,
    /// the number of memory pages transferred per second
    /// (Since 4.0)
    #[qapi(name = "pages-per-second")]
    pub pages_per_second: u64,
    /// The number of bytes sent during the post-copy phase
    /// (since 7.0).
    #[qapi(name = "postcopy-bytes")]
    pub postcopy_bytes: u64,
    /// The number of page requests received from the
    /// destination (since 2.7)
    #[qapi(name = "postcopy-requests")]
    pub postcopy_requests: i64,
    /// The number of bytes sent in the pre-copy phase
    /// (since 7.0).
    #[qapi(name = "precopy-bytes")]
    pub precopy_bytes: u64,
    /// amount of bytes remaining to be transferred to the
    /// target VM
    pub remaining: i64,
    /// total amount of bytes involved in the migration process
    pub total: i64,
    /// amount of bytes already transferred to the target VM
    pub transferred: i64,
}

/// Detailed migration status.
///
#[qapi(added = "1.0.0")]
#[qapi(removed = "11.1.0")]
pub struct MigrationStats {
    /// number of pages dirtied by second by the guest
    /// (since 1.3)
    #[qapi(name = "dirty-pages-rate")]
    #[qapi(added = "1.3.0")]
    pub dirty_pages_rate: Option<i64>,
    /// number of times that dirty ram was synchronized
    /// (since 2.1)
    #[qapi(name = "dirty-sync-count")]
    #[qapi(added = "2.1.0")]
    pub dirty_sync_count: Option<i64>,
    /// Number of times dirty RAM
    /// synchronization could not avoid copying dirty pages.  This is
    /// between 0 and @dirty-sync-count * @multifd-channels.
    /// (since 7.1)
    #[qapi(name = "dirty-sync-missed-zero-copy")]
    #[qapi(added = "7.1.0")]
    pub dirty_sync_missed_zero_copy: Option<u64>,
    /// The number of bytes sent while the guest is paused
    /// (since 7.0).
    #[qapi(name = "downtime-bytes")]
    #[qapi(added = "7.0.0")]
    pub downtime_bytes: Option<u64>,
    /// number of duplicate (zero) pages (since 1.2)
    #[qapi(added = "1.2.0")]
    pub duplicate: Option<i64>,
    /// throughput in megabits/sec.  (since 1.6)
    #[qapi(added = "1.6.0")]
    pub mbps: Option<f64>,
    /// The number of bytes sent through multifd (since 3.0)
    #[qapi(name = "multifd-bytes")]
    #[qapi(added = "3.0.0")]
    pub multifd_bytes: Option<u64>,
    /// number of normal pages (since 1.2)
    #[qapi(added = "1.2.0")]
    pub normal: Option<i64>,
    /// number of normal bytes sent (since 1.2)
    #[qapi(name = "normal-bytes")]
    #[qapi(added = "1.2.0")]
    pub normal_bytes: Option<i64>,
    /// The number of bytes per page for the various page-based
    /// statistics (since 2.10)
    #[qapi(name = "page-size")]
    #[qapi(added = "2.10.0")]
    pub page_size: Option<i64>,
    /// the number of memory pages transferred per second
    /// (Since 4.0)
    #[qapi(name = "pages-per-second")]
    #[qapi(added = "4.0.0")]
    pub pages_per_second: Option<u64>,
    /// The number of bytes sent during the post-copy phase
    /// (since 7.0).
    #[qapi(name = "postcopy-bytes")]
    #[qapi(added = "7.0.0")]
    pub postcopy_bytes: Option<u64>,
    /// The number of page requests received from the
    /// destination (since 2.7)
    #[qapi(name = "postcopy-requests")]
    #[qapi(added = "2.7.0")]
    pub postcopy_requests: Option<i64>,
    /// The number of bytes sent in the pre-copy phase
    /// (since 7.0).
    #[qapi(name = "precopy-bytes")]
    #[qapi(added = "7.0.0")]
    pub precopy_bytes: Option<u64>,
    /// amount of bytes remaining to be transferred to the
    /// target VM
    pub remaining: i64,
    /// number of skipped zero pages (since 1.5)
    #[qapi(feature = "deprecated")]
    #[qapi(added = "1.5.0")]
    #[qapi(deprecated(added = "8.1.0"))]
    #[qapi(removed = "9.1.0")]
    pub skipped: Option<i64>,
    /// total amount of bytes involved in the migration process
    pub total: i64,
    /// amount of bytes already transferred to the target VM
    pub transferred: i64,
}

/// An enumeration of migration status.
///
#[qapi(added = "2.3.0")]
pub enum MigrationStatus {
    /// in the process of doing migration.
    #[qapi(name = "active")]
    Active,
    /// cancelling migration is finished.
    #[qapi(name = "cancelled")]
    Cancelled,
    /// in the process of cancelling migration.
    #[qapi(name = "cancelling")]
    Cancelling,
    /// VM is in the process of fault tolerance, VM can not get into
    /// this state unless colo capability is enabled for migration.
    /// (since 2.8)
    #[qapi(name = "colo")]
    #[qapi(added = "2.8.0")]
    Colo,
    /// migration is finished.
    #[qapi(name = "completed")]
    Completed,
    /// During device serialisation (also known as switchover
    /// phase).  Before 9.2, this is only used when (1) in precopy, and
    /// (2) when pre-switchover capability is enabled.  After 10.0, this
    /// state will always be present for every migration procedure as
    /// the switchover phase.  (since 2.11)
    #[qapi(name = "device")]
    #[qapi(added = "2.11.0")]
    Device,
    /// error occurred during migration, clean-up done.
    #[qapi(name = "failed")]
    Failed,
    /// error occurred during migration, clean-up underway.
    /// (since 11.0)
    #[qapi(name = "failing")]
    #[qapi(added = "11.0.0")]
    Failing,
    /// no migration has ever happened.
    #[qapi(name = "none")]
    None,
    /// like active, but now in postcopy mode.
    /// (since 2.5)
    #[qapi(name = "postcopy-active")]
    #[qapi(added = "2.5.0")]
    PostcopyActive,
    /// like postcopy-active, but the destination is still
    /// loading device state and is not running yet.  If migration fails
    /// during this state, the source side will resume.  If there is no
    /// return-path from destination to source, this state is skipped.
    /// (since 10.2)
    #[qapi(name = "postcopy-device")]
    #[qapi(added = "10.2.0")]
    PostcopyDevice,
    /// during postcopy but paused.  (since 3.0)
    #[qapi(name = "postcopy-paused")]
    #[qapi(added = "3.0.0")]
    PostcopyPaused,
    /// trying to recover from a paused postcopy.
    /// (since 3.0)
    #[qapi(name = "postcopy-recover")]
    #[qapi(added = "3.0.0")]
    PostcopyRecover,
    /// setup phase for a postcopy recovery
    /// process, preparing for a recovery phase to start.  (since 9.1)
    #[qapi(name = "postcopy-recover-setup")]
    #[qapi(added = "9.1.0")]
    PostcopyRecoverSetup,
    /// Paused before device serialisation.  (since 2.11)
    #[qapi(name = "pre-switchover")]
    #[qapi(added = "2.11.0")]
    PreSwitchover,
    /// migration process has been initiated.
    #[qapi(name = "setup")]
    Setup,
    /// wait for device unplug request by guest OS to be
    /// completed.  (since 4.2)
    #[qapi(name = "wait-unplug")]
    #[qapi(added = "4.2.0")]
    WaitUnplug,
}

/// Information about migrationthreads
///
#[qapi(added = "8.0.0")]
#[qapi(removed = "11.0.0")]
pub struct MigrationThreadInfo {
    /// the name of migration thread
    pub name: String,
    /// ID of the underlying host thread
    #[qapi(name = "thread-id")]
    pub thread_id: i64,
}

/// An enumeration whose values tell the mirror block job when to
/// trigger writes to the target.
///
#[qapi(added = "3.0.0")]
pub enum MirrorCopyMode {
    /// copy data in background only.
    #[qapi(name = "background")]
    Background,
    /// when data is written to the source, write it
    /// (synchronously) to the target as well.  In addition, data is
    /// copied in background just like in @background mode.
    #[qapi(name = "write-blocking")]
    WriteBlocking,
}

/// An enumeration of possible behaviors for the initial synchronization
/// phase of storage mirroring.
///
#[qapi(added = "1.3.0")]
pub enum MirrorSyncMode {
    /// only copy data described by the dirty bitmap.  Behavior on
    /// completion is determined by the `BitmapSyncMode`.  (since: 4.2)
    #[qapi(name = "bitmap")]
    #[qapi(added = "4.2.0")]
    Bitmap,
    /// copies data from all images to the destination
    #[qapi(name = "full")]
    Full,
    /// only copy data described by the dirty bitmap.
    /// (since: 2.4)
    #[qapi(name = "incremental")]
    #[qapi(added = "2.4.0")]
    Incremental,
    /// only copy data written from now on
    #[qapi(name = "none")]
    None,
    /// copies data in the topmost image to the destination
    #[qapi(name = "top")]
    Top,
}

/// Properties for the HMP monitor
///
#[qapi(name = "MonitorHMPProperties")]
#[qapi(added = "11.1.0")]
pub struct MonitorHmpProperties {
    /// ID of the character device providing the monitor transport
    pub chardev: String,
    /// whether to enable readline for line editing
    /// (default: true)
    pub readline: Option<bool>,
}

/// An enumeration of monitor modes.
///
#[qapi(added = "5.0.0")]
pub enum MonitorMode {
    /// QMP monitor (JSON-based machine interface)
    #[qapi(name = "control")]
    Control,
    /// HMP monitor (human-oriented command line interface)
    #[qapi(name = "readline")]
    Readline,
}

/// Options to be used for adding a new monitor.
///
#[qapi(added = "5.0.0")]
pub struct MonitorOptions {
    /// Name of a character device to expose the monitor on
    pub chardev: String,
    /// Name of the monitor
    pub id: Option<String>,
    /// Selects the monitor mode (default: readline in the system
    /// emulator, control in qemu-storage-daemon)
    pub mode: Option<MonitorMode>,
    /// Enables pretty printing (QMP only)
    pub pretty: Option<bool>,
}

/// Properties for all monitors
///
#[qapi(added = "11.1.0")]
pub struct MonitorProperties {
    /// ID of the character device providing the monitor transport
    pub chardev: String,
}

/// Action to take when the character device backend is closed.
///
#[qapi(name = "MonitorQMPCloseAction")]
#[qapi(added = "11.1.0")]
pub enum MonitorQmpCloseAction {
    /// delete both the 'monitor-qmp' object and its associated
    /// character device backend object
    #[qapi(name = "delete")]
    Delete,
    /// take no action
    #[qapi(name = "none")]
    None,
}

/// Properties for the QMP monitor
///
#[qapi(name = "MonitorQMPProperties")]
#[qapi(added = "11.1.0")]
pub struct MonitorQmpProperties {
    /// ID of the character device providing the monitor transport
    pub chardev: String,
    /// action to take when the character device backend is
    /// closed (default: none)
    #[qapi(name = "close-action")]
    pub close_action: Option<MonitorQmpCloseAction>,
    /// whether to pretty print JSON responses (default: false)
    pub pretty: Option<bool>,
}

/// Information about a mouse device.
///
#[qapi(added = "1.0.0")]
pub struct MouseInfo {
    /// true if this device supports absolute coordinates as
    /// input
    pub absolute: bool,
    /// true if this device is currently receiving mouse events
    pub current: bool,
    /// the index of the mouse device
    pub index: i64,
    /// the name of the mouse device
    pub name: String,
}

/// An enumeration of multifd compression methods.
///
#[qapi(name = "MultiFDCompression")]
#[qapi(added = "5.0.0")]
pub enum MultiFdCompression {
    /// no compression.
    #[qapi(name = "none")]
    None,
    /// use qatzip compression method.  (Since 9.2)
    #[qapi(name = "qatzip")]
    #[qapi(condition = "CONFIG_QATZIP")]
    #[qapi(added = "9.2.0")]
    Qatzip,
    /// use qpl compression method.  Query Processing Library(qpl) is
    /// based on the deflate compression algorithm and use the Intel
    /// In-Memory Analytics Accelerator(IAA) accelerated compression and
    /// decompression.  (Since 9.1)
    #[qapi(name = "qpl")]
    #[qapi(condition = "CONFIG_QPL")]
    #[qapi(added = "9.1.0")]
    Qpl,
    /// use UADK library compression method.  (Since 9.1)
    #[qapi(name = "uadk")]
    #[qapi(condition = "CONFIG_UADK")]
    #[qapi(added = "9.1.0")]
    Uadk,
    /// use zlib compression method.
    #[qapi(name = "zlib")]
    Zlib,
    /// use zstd compression method.
    #[qapi(name = "zstd")]
    #[qapi(condition = "defined(CONFIG_ZSTD)")]
    Zstd,
}

/// Guest name information.
///
#[qapi(added = "1.0.0")]
pub struct NameInfo {
    /// The name of the guest
    pub name: Option<String>,
}

/// An NBD block export, per legacy `nbd-server-add` command.
///
#[qapi(added = "5.2.0")]
pub struct NbdServerAddOptions {
    /// Also export a single dirty bitmap reachable from @device,
    /// so the NBD client can use NBD_OPT_SET_META_CONTEXT with the
    /// metadata context name "qemu:dirty-bitmap:BITMAP" to inspect the
    /// bitmap (since 4.0).
    pub bitmap: Option<String>,
    /// Free-form description of the export, up to 4096 bytes.
    /// (Since 5.0)
    pub description: Option<String>,
    /// The device name or node name of the node to be exported
    pub device: String,
    /// Export name.  If unspecified, the @device parameter is used
    /// as the export name.  (Since 2.12)
    pub name: Option<String>,
    /// Whether clients should be able to write to the device via
    /// the NBD connection (default false).
    pub writable: Option<bool>,
}

/// Keep this type consistent with the nbd-server-start arguments.  The
/// only intended difference is using SocketAddress instead of
/// SocketAddressLegacy.
///
#[qapi(added = "5.0.0")]
pub struct NbdServerOptions {
    /// Address on which to listen.
    pub addr: SocketAddress,
    #[qapi(name = "handshake-max-seconds")]
    #[qapi(added = "10.0.0")]
    pub handshake_max_seconds: Option<u32>,
    /// The maximum number of connections to allow at the
    /// same time, 0 for unlimited.  Setting this to 1 also stops the
    /// server from advertising multiple client support (since 5.2;
    /// default: 100)
    #[qapi(name = "max-connections")]
    #[qapi(added = "5.2.0")]
    pub max_connections: Option<u32>,
    /// ID of the QAuthZ authorization object used to validate
    /// the client's x509 distinguished name.  This object is is only
    /// resolved at time of use, so can be deleted and recreated on the
    /// fly while the NBD server is active.  If missing, it will default
    /// to denying access (since 4.0).
    #[qapi(name = "tls-authz")]
    pub tls_authz: Option<String>,
    /// ID of the TLS credentials object (since 2.6).
    #[qapi(name = "tls-creds")]
    pub tls_creds: Option<String>,
}

#[qapi(added = "10.0.0")]
pub struct NbdServerOptionsBase {
    #[qapi(name = "handshake-max-seconds")]
    pub handshake_max_seconds: Option<u32>,
    #[qapi(name = "max-connections")]
    pub max_connections: Option<u32>,
    #[qapi(name = "tls-authz")]
    pub tls_authz: Option<String>,
    #[qapi(name = "tls-creds")]
    pub tls_creds: Option<String>,
}

#[qapi(added = "10.0.0")]
pub struct NbdServerOptionsLegacy {
    pub addr: SocketAddressLegacy,
    #[qapi(name = "handshake-max-seconds")]
    pub handshake_max_seconds: Option<u32>,
    #[qapi(name = "max-connections")]
    pub max_connections: Option<u32>,
    #[qapi(name = "tls-authz")]
    pub tls_authz: Option<String>,
    #[qapi(name = "tls-creds")]
    pub tls_creds: Option<String>,
}

/// Mode for removing an NBD export.
///
#[qapi(added = "2.12.0")]
#[qapi(removed = "5.2.0")]
pub enum NbdServerRemoveMode {
    /// Drop all connections immediately and remove export.
    ///
    /// Potential additional modes to be added in the future:
    ///
    /// hide: Just hide export from new clients, leave existing connections as is.
    /// Remove export after all clients are disconnected.
    ///
    /// soft: Hide export from new clients, answer with ESHUTDOWN for all further
    /// requests from existing clients.
    #[qapi(name = "hard")]
    Hard,
    /// Remove export if there are no existing connections, fail otherwise.
    #[qapi(name = "safe")]
    Safe,
}

/// Available netdev drivers.
///
#[qapi(added = "2.7.0")]
pub enum NetClientDriver {
    /// since 8.2
    #[qapi(name = "af-xdp")]
    #[qapi(condition = "CONFIG_AF_XDP")]
    #[qapi(added = "8.2.0")]
    AfXdp,
    #[qapi(name = "bridge")]
    Bridge,
    /// since 7.2
    #[qapi(name = "dgram")]
    #[qapi(added = "7.2.0")]
    Dgram,
    #[qapi(name = "dump")]
    #[qapi(removed = "2.12.0")]
    Dump,
    #[qapi(name = "hubport")]
    Hubport,
    /// since 2.1
    #[qapi(name = "l2tpv3")]
    L2tpv3,
    #[qapi(name = "netmap")]
    Netmap,
    #[qapi(name = "nic")]
    Nic,
    #[qapi(name = "none")]
    None,
    /// since 10.1
    #[qapi(name = "passt")]
    #[qapi(condition = "CONFIG_PASST")]
    #[qapi(added = "10.1.0")]
    Passt,
    #[qapi(name = "socket")]
    Socket,
    /// since 7.2
    #[qapi(name = "stream")]
    #[qapi(added = "7.2.0")]
    Stream,
    #[qapi(name = "tap")]
    Tap,
    #[qapi(name = "user")]
    User,
    #[qapi(name = "vde")]
    Vde,
    #[qapi(name = "vhost-user")]
    VhostUser,
    /// since 5.1
    #[qapi(name = "vhost-vdpa")]
    #[qapi(added = "5.1.0")]
    VhostVdpa,
    /// since 7.1
    #[qapi(name = "vmnet-bridged")]
    #[qapi(condition = "CONFIG_VMNET")]
    #[qapi(added = "7.1.0")]
    VmnetBridged,
    /// since 7.1
    #[qapi(name = "vmnet-host")]
    #[qapi(condition = "CONFIG_VMNET")]
    #[qapi(added = "7.1.0")]
    VmnetHost,
    /// since 7.1
    #[qapi(name = "vmnet-shared")]
    #[qapi(condition = "CONFIG_VMNET")]
    #[qapi(added = "7.1.0")]
    VmnetShared,
}

/// A discriminated record of network device traits.
///
#[qapi(added = "1.2.0")]
#[qapi(removed = "2.7.0")]
pub struct NetClientOptions {
    #[qapi(discriminator)]
    #[qapi(name = "type")]
    pub r_type: NetClientOptionsType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<NetClientOptionsBranch>,
}

#[qapi(added = "1.2.0")]
#[qapi(removed = "2.7.0")]
pub enum NetClientOptionsBranch {
    #[qapi(name = "bridge")]
    Bridge(NetdevBridgeOptions),
    #[qapi(name = "dump")]
    Dump(NetdevDumpOptions),
    #[qapi(name = "hubport")]
    Hubport(NetdevHubPortOptions),
    #[qapi(name = "l2tpv3")]
    #[qapi(added = "2.1.0")]
    L2tpv3(NetdevL2tPv3Options),
    #[qapi(name = "netmap")]
    #[qapi(added = "2.0.0")]
    Netmap(NetdevNetmapOptions),
    #[qapi(name = "nic")]
    Nic(NetLegacyNicOptions),
    #[qapi(name = "none")]
    None(NetdevNoneOptions),
    #[qapi(name = "socket")]
    Socket(NetdevSocketOptions),
    #[qapi(name = "tap")]
    Tap(NetdevTapOptions),
    #[qapi(name = "user")]
    User(NetdevUserOptions),
    #[qapi(name = "vde")]
    Vde(NetdevVdeOptions),
    #[qapi(name = "vhost-user")]
    #[qapi(added = "2.1.0")]
    VhostUser(NetdevVhostUserOptions),
}

#[qapi(added = "1.2.0")]
#[qapi(removed = "2.7.0")]
pub enum NetClientOptionsType {
    #[qapi(name = "bridge")]
    Bridge,
    #[qapi(name = "dump")]
    Dump,
    #[qapi(name = "hubport")]
    Hubport,
    #[qapi(name = "l2tpv3")]
    #[qapi(added = "2.1.0")]
    L2tpv3,
    #[qapi(name = "netmap")]
    #[qapi(added = "2.0.0")]
    Netmap,
    #[qapi(name = "nic")]
    Nic,
    #[qapi(name = "none")]
    None,
    #[qapi(name = "socket")]
    Socket,
    #[qapi(name = "tap")]
    Tap,
    #[qapi(name = "user")]
    User,
    #[qapi(name = "vde")]
    Vde,
    #[qapi(name = "vhost-user")]
    #[qapi(added = "2.1.0")]
    VhostUser,
}

/// Captures the configuration of a network device.
///
#[qapi(added = "1.2.0")]
pub struct Netdev {
    /// identifier for monitor commands.
    pub id: String,
    /// device type specific properties
    #[qapi(removed = "2.7.0")]
    pub opts: Option<NetClientOptions>,
    /// Specify the driver used for interpreting remaining arguments.
    #[qapi(discriminator)]
    #[qapi(added = "2.7.0")]
    pub r#type: Option<NetClientDriver>,
    #[qapi(union)]
    #[qapi(flatten)]
    #[qapi(added = "2.7.0")]
    pub u: Option<NetdevBranch>,
}

/// AF_XDP network backend
///
#[qapi(name = "NetdevAFXDPOptions")]
#[qapi(condition = "CONFIG_AF_XDP")]
#[qapi(added = "8.2.0")]
pub struct NetdevAfxdpOptions {
    /// Force XDP copy mode even if device supports zero-copy.
    /// (default: false)
    #[qapi(name = "force-copy")]
    pub force_copy: Option<bool>,
    /// The name of an existing network interface.
    pub ifname: String,
    /// Don't load a default XDP program, use one already loaded
    /// to the interface (default: false).  Requires @sock-fds or
    pub inhibit: Option<bool>,
    /// The path to a pinned xsk map to push file descriptors
    /// for bound AF_XDP sockets into.  @map-path and @sock-fds are
    /// mutually exclusive.  Requires @inhibit.  (Since 10.1)
    #[qapi(name = "map-path")]
    #[qapi(added = "10.1.0")]
    pub map_path: Option<String>,
    /// Use @map-path to insert xsk sockets starting from
    /// this index number (default: 0).  Requires @map-path.
    /// (Since 10.1)
    #[qapi(name = "map-start-index")]
    #[qapi(added = "10.1.0")]
    pub map_start_index: Option<i32>,
    /// Attach mode for a default XDP program.  If not specified,
    /// then 'native' will be tried first, then 'skb'.
    pub mode: Option<AfxdpMode>,
    /// number of queues to be used for multiqueue interfaces
    /// (default: 1).
    pub queues: Option<i64>,
    /// A colon (:) separated list of file descriptors for
    /// already open but not bound AF_XDP sockets in the queue order.
    /// One fd per queue.  These descriptors should already be added
    /// into XDP socket map for corresponding queues.  @sock-fds and
    #[qapi(name = "sock-fds")]
    pub sock_fds: Option<String>,
    /// Use @queues starting from this queue number
    /// (default: 0).
    #[qapi(name = "start-queue")]
    pub start_queue: Option<i64>,
}

#[qapi(added = "2.7.0")]
pub enum NetdevBranch {
    #[qapi(name = "af-xdp")]
    #[qapi(condition = "CONFIG_AF_XDP")]
    #[qapi(added = "8.2.0")]
    AfXdp(NetdevAfxdpOptions),
    #[qapi(name = "bridge")]
    Bridge(NetdevBridgeOptions),
    #[qapi(name = "dgram")]
    #[qapi(added = "7.2.0")]
    Dgram(NetdevDgramOptions),
    #[qapi(name = "dump")]
    #[qapi(removed = "2.12.0")]
    Dump(NetdevDumpOptions),
    #[qapi(name = "hubport")]
    Hubport(NetdevHubPortOptions),
    #[qapi(name = "l2tpv3")]
    L2tpv3(NetdevL2tPv3Options),
    #[qapi(name = "netmap")]
    Netmap(NetdevNetmapOptions),
    #[qapi(name = "nic")]
    Nic(NetLegacyNicOptions),
    #[qapi(name = "none")]
    None,
    #[qapi(name = "passt")]
    #[qapi(condition = "CONFIG_PASST")]
    #[qapi(added = "10.1.0")]
    Passt(NetdevPasstOptions),
    #[qapi(name = "socket")]
    Socket(NetdevSocketOptions),
    #[qapi(name = "stream")]
    #[qapi(added = "7.2.0")]
    Stream(NetdevStreamOptions),
    #[qapi(name = "tap")]
    Tap(NetdevTapOptions),
    #[qapi(name = "user")]
    User(NetdevUserOptions),
    #[qapi(name = "vde")]
    Vde(NetdevVdeOptions),
    #[qapi(name = "vhost-user")]
    VhostUser(NetdevVhostUserOptions),
    #[qapi(name = "vhost-vdpa")]
    #[qapi(added = "5.1.0")]
    VhostVdpa(NetdevVhostVdpaOptions),
    #[qapi(name = "vmnet-bridged")]
    #[qapi(condition = "CONFIG_VMNET")]
    #[qapi(added = "7.1.0")]
    VmnetBridged(NetdevVmnetBridgedOptions),
    #[qapi(name = "vmnet-host")]
    #[qapi(condition = "CONFIG_VMNET")]
    #[qapi(added = "7.1.0")]
    VmnetHost(NetdevVmnetHostOptions),
    #[qapi(name = "vmnet-shared")]
    #[qapi(condition = "CONFIG_VMNET")]
    #[qapi(added = "7.1.0")]
    VmnetShared(NetdevVmnetSharedOptions),
}

/// Connect a host TAP network interface to a host bridge device.
///
#[qapi(added = "1.2.0")]
pub struct NetdevBridgeOptions {
    /// bridge name
    pub br: Option<String>,
    /// command to execute to configure bridge
    pub helper: Option<String>,
}

/// Configuration info for datagram socket netdev.
///
#[qapi(added = "7.2.0")]
pub struct NetdevDgramOptions {
    /// local address
    ///
    /// Only `SocketAddress` types 'unix', 'inet' and 'fd' are supported.
    ///
    /// If remote address is present and it's a multicast address, local
    /// address is optional.  Otherwise local address is required and remote
    /// address is optional.
    ///
    /// .. table:: Valid parameters combination table
    /// :widths: auto
    ///
    /// =============  ========  =====
    /// remote         local     okay?
    /// =============  ========  =====
    /// absent         absent    no
    /// absent         not fd    no
    /// absent         fd        yes
    /// multicast      absent    yes
    /// multicast      present   yes
    /// not multicast  absent    no
    /// not multicast  present   yes
    /// =============  ========  =====
    pub local: Option<SocketAddress>,
    /// remote address
    pub remote: Option<SocketAddress>,
}

/// Dump VLAN network traffic to a file.
///
#[qapi(added = "1.2.0")]
#[qapi(removed = "2.12.0")]
pub struct NetdevDumpOptions {
    /// dump file path (default is qemu-vlan0.pcap)
    pub file: Option<String>,
    /// per-packet size limit (64k default). Understands [TGMKkb]
    /// suffixes.
    pub len: Option<u64>,
}

/// Connect two or more net clients through a software hub.
///
#[qapi(added = "1.2.0")]
pub struct NetdevHubPortOptions {
    /// hub identifier number
    pub hubid: i32,
    /// used to connect hub to a netdev instead of a device
    /// (since 2.12)
    #[qapi(added = "2.12.0")]
    pub netdev: Option<String>,
}

/// Configure an Ethernet over L2TPv3 tunnel.
///
#[qapi(name = "NetdevL2TPv3Options")]
#[qapi(added = "2.1.0")]
pub struct NetdevL2tPv3Options {
    /// use 64 bit cookies
    pub cookie64: Option<bool>,
    /// have sequence counter
    pub counter: Option<bool>,
    /// destination address
    pub dst: String,
    /// destination port - mandatory for udp, optional for ip
    pub dstport: Option<String>,
    /// force the use of ipv6
    pub ipv6: Option<bool>,
    /// additional offset - allows the insertion of additional
    /// application-specific data before the packet payload
    pub offset: Option<u32>,
    /// pin sequence counter to zero - workaround for buggy
    /// implementations or networks with packet reorder
    pub pincounter: Option<bool>,
    /// 32 or 64 bit receive cookie
    pub rxcookie: Option<u64>,
    /// 32 bit receive session - if not specified set to the
    /// same value as transmit
    pub rxsession: Option<u32>,
    /// source address
    pub src: String,
    /// source port - mandatory for udp, optional for ip
    pub srcport: Option<String>,
    /// 32 or 64 bit transmit cookie
    pub txcookie: Option<u64>,
    /// 32 bit transmit session
    pub txsession: u32,
    /// use the udp version of l2tpv3 encapsulation
    pub udp: Option<bool>,
}

/// Connect a client to a netmap-enabled NIC or to a VALE switch port
///
#[qapi(added = "2.0.0")]
pub struct NetdevNetmapOptions {
    /// path of the netmap device (default: '/dev/netmap').
    pub devname: Option<String>,
    /// Either the name of an existing network interface supported
    /// by netmap, or the name of a VALE port (created on the fly).  A
    /// VALE port name is in the form 'valeXXX:YYY', where XXX and YYY
    /// are non-negative integers.  XXX identifies a switch and YYY
    /// identifies a port of the switch.  VALE ports having the same XXX
    /// are therefore connected to the same switch.
    pub ifname: String,
}

/// Use it alone to have zero network devices.
///
#[qapi(added = "1.2.0")]
#[qapi(removed = "3.0.0")]
pub struct NetdevNoneOptions {}

/// Unprivileged user-mode network connectivity using passt
///
#[qapi(condition = "CONFIG_PASST")]
#[qapi(added = "10.1.0")]
#[qapi(removed = "11.1.0")]
pub struct NetDevPasstOptions {
    /// IPv4 or IPv6 address
    pub address: Option<String>,
    /// enable/disable DHCP
    pub dhcp: Option<bool>,
    /// enable/disable DNS list in DHCP/DHCPv6/NDP
    #[qapi(name = "dhcp-dns")]
    pub dhcp_dns: Option<bool>,
    /// enable/disable list in DHCP/DHCPv6/NDP
    #[qapi(name = "dhcp-search")]
    pub dhcp_search: Option<bool>,
    /// enable/disable DHCPv6
    pub dhcpv6: Option<bool>,
    /// IPv4 or IPv6 address as DNS
    pub dns: Option<String>,
    /// forward DNS queries sent to
    #[qapi(name = "dns-forward")]
    pub dns_forward: Option<String>,
    /// host nameserver to direct queries to
    #[qapi(name = "dns-host")]
    pub dns_host: Option<String>,
    /// FQDN to configure client with
    pub fqdn: Option<String>,
    /// bind to any address for forwarding
    pub freebind: Option<bool>,
    /// IPv4 or IPv6 address as gateway
    pub gateway: Option<String>,
    /// enable/disable ICMP
    pub icmp: Option<bool>,
    /// interface for addresses and routes
    pub interface: Option<String>,
    /// enable/disable IPv4
    pub ipv4: Option<bool>,
    /// enable/disable IPv6
    pub ipv6: Option<bool>,
    /// source MAC address
    pub mac: Option<String>,
    /// addr to translate to guest's address
    #[qapi(name = "map-guest-addr")]
    pub map_guest_addr: Option<String>,
    /// addresse to refer to host
    #[qapi(name = "map-host-loopback")]
    pub map_host_loopback: Option<String>,
    /// assign MTU via DHCP/NDP
    pub mtu: Option<i64>,
    /// enable/disable NDP
    pub ndp: Option<bool>,
    /// IPv4 mask
    pub netmask: Option<String>,
    /// bind to address as outbound source
    pub outbound: Option<String>,
    /// bind to outbound interface for IPv4
    #[qapi(name = "outbound-if4")]
    pub outbound_if4: Option<String>,
    /// bind to outbound interface for IPv6
    #[qapi(name = "outbound-if6")]
    pub outbound_if6: Option<String>,
    /// parameter to pass to passt command
    pub param: Option<Vec<QapiString>>,
    /// Filename of the passt program to run (by default 'passt', and
    /// use PATH)
    pub path: Option<String>,
    /// don't print informational messages (default, passed as
    /// '--quiet')
    pub quiet: Option<bool>,
    /// enable/disable route advertisements
    pub ra: Option<bool>,
    /// search domains
    pub search: Option<Vec<QapiString>>,
    /// enable/disable TCP
    pub tcp: Option<bool>,
    /// TCP ports to forward
    #[qapi(name = "tcp-ports")]
    pub tcp_ports: Option<Vec<QapiString>>,
    /// enable/disable UDP
    pub udp: Option<bool>,
    /// UDP ports to forward
    #[qapi(name = "udp-ports")]
    pub udp_ports: Option<Vec<QapiString>>,
    /// enable vhost-user
    #[qapi(name = "vhost-user")]
    pub vhost_user: Option<bool>,
}

/// Unprivileged user-mode network connectivity using passt
///
#[qapi(condition = "CONFIG_PASST")]
#[qapi(added = "11.1.0")]
pub struct NetdevPasstOptions {
    /// IPv4 or IPv6 address
    pub address: Option<String>,
    /// enable/disable DHCP
    pub dhcp: Option<bool>,
    /// enable/disable DNS list in DHCP/DHCPv6/NDP
    #[qapi(name = "dhcp-dns")]
    pub dhcp_dns: Option<bool>,
    /// enable/disable list in DHCP/DHCPv6/NDP
    #[qapi(name = "dhcp-search")]
    pub dhcp_search: Option<bool>,
    /// enable/disable DHCPv6
    pub dhcpv6: Option<bool>,
    /// IPv4 or IPv6 address as DNS
    pub dns: Option<String>,
    /// forward DNS queries sent to
    #[qapi(name = "dns-forward")]
    pub dns_forward: Option<String>,
    /// host nameserver to direct queries to
    #[qapi(name = "dns-host")]
    pub dns_host: Option<String>,
    /// FQDN to configure client with
    pub fqdn: Option<String>,
    /// bind to any address for forwarding
    pub freebind: Option<bool>,
    /// IPv4 or IPv6 address as gateway
    pub gateway: Option<String>,
    /// enable/disable ICMP
    pub icmp: Option<bool>,
    /// interface for addresses and routes
    pub interface: Option<String>,
    /// enable/disable IPv4
    pub ipv4: Option<bool>,
    /// enable/disable IPv6
    pub ipv6: Option<bool>,
    /// source MAC address
    pub mac: Option<String>,
    /// addr to translate to guest's address
    #[qapi(name = "map-guest-addr")]
    pub map_guest_addr: Option<String>,
    /// addresse to refer to host
    #[qapi(name = "map-host-loopback")]
    pub map_host_loopback: Option<String>,
    /// assign MTU via DHCP/NDP
    pub mtu: Option<i64>,
    /// enable/disable NDP
    pub ndp: Option<bool>,
    /// IPv4 mask
    pub netmask: Option<String>,
    /// bind to address as outbound source
    pub outbound: Option<String>,
    /// bind to outbound interface for IPv4
    #[qapi(name = "outbound-if4")]
    pub outbound_if4: Option<String>,
    /// bind to outbound interface for IPv6
    #[qapi(name = "outbound-if6")]
    pub outbound_if6: Option<String>,
    /// parameter to pass to passt command
    pub param: Option<Vec<PasstParameter>>,
    /// Filename of the passt program to run (by default 'passt', and
    /// use PATH)
    pub path: Option<String>,
    /// don't print informational messages (default, passed as
    /// '--quiet')
    pub quiet: Option<bool>,
    /// enable/disable route advertisements
    pub ra: Option<bool>,
    /// search domains
    pub search: Option<Vec<PasstSearch>>,
    /// enable/disable TCP
    pub tcp: Option<bool>,
    /// TCP ports to forward
    #[qapi(name = "tcp-ports")]
    pub tcp_ports: Option<Vec<PasstPortForward>>,
    /// enable/disable UDP
    pub udp: Option<bool>,
    /// UDP ports to forward
    #[qapi(name = "udp-ports")]
    pub udp_ports: Option<Vec<PasstPortForward>>,
    /// enable vhost-user
    #[qapi(name = "vhost-user")]
    pub vhost_user: Option<bool>,
}

/// Socket netdevs are used to establish a network connection to another
/// QEMU virtual machine via a TCP socket.
///
#[qapi(added = "1.2.0")]
pub struct NetdevSocketOptions {
    /// port number, and optional hostname, to connect to
    pub connect: Option<String>,
    /// file descriptor of an already opened socket
    pub fd: Option<String>,
    /// port number, and optional hostname, to listen on
    pub listen: Option<String>,
    /// source address and port for multicast and udp packets
    pub localaddr: Option<String>,
    /// UDP multicast address and port number
    pub mcast: Option<String>,
    /// UDP unicast address and port number
    pub udp: Option<String>,
}

/// Configuration info for stream socket netdev
///
#[qapi(added = "7.2.0")]
pub struct NetdevStreamOptions {
    /// socket address to listen on (server=true) or connect to
    /// (server=false)
    pub addr: SocketAddress,
    /// For a client socket, if a socket is disconnected, then
    /// attempt a reconnect after the given number of seconds.  Setting
    /// this to zero disables this function.  (default: 0) (since 8.0)
    ///
    /// Only SocketAddress types 'unix', 'inet' and 'fd' are supported.
    #[qapi(feature = "deprecated")]
    #[qapi(added = "8.0.0")]
    #[qapi(deprecated(added = "9.2.0"))]
    #[qapi(removed = "10.2.0")]
    pub reconnect: Option<i64>,
    /// For a client socket, if a socket is disconnected,
    /// then attempt a reconnect after the given number of milliseconds.
    /// Setting this to zero disables this function.
    /// (default: 0) (Since: 9.2)
    ///
    /// Only `SocketAddress` types 'unix', 'inet' and 'fd' are supported.
    #[qapi(name = "reconnect-ms")]
    #[qapi(added = "9.2.0")]
    pub reconnect_ms: Option<i64>,
    /// create server socket (default: false)
    pub server: Option<bool>,
}

/// Used to configure a host TAP network interface backend.
///
#[qapi(added = "1.2.0")]
pub struct NetdevTapOptions {
    /// bridge name (since 2.8)
    #[qapi(added = "2.8.0")]
    pub br: Option<String>,
    /// script to shut down the interface
    pub downscript: Option<String>,
    /// file descriptor of an already opened tap
    pub fd: Option<String>,
    /// multiple file descriptors of already opened multiqueue capable
    /// tap
    #[qapi(added = "1.4.0")]
    pub fds: Option<String>,
    /// command to execute to configure bridge
    pub helper: Option<String>,
    /// interface name
    pub ifname: Option<String>,
    /// maximum number of microseconds that could be spent on busy
    /// polling for tap (since 2.7)
    #[qapi(name = "poll-us")]
    #[qapi(added = "2.7.0")]
    pub poll_us: Option<u32>,
    /// number of queues to be created for multiqueue capable tap
    #[qapi(added = "1.4.0")]
    pub queues: Option<u32>,
    /// script to initialize the interface
    pub script: Option<String>,
    /// send buffer limit.  Understands [TGMKkb] suffixes.
    pub sndbuf: Option<u64>,
    /// enable vhost-net network accelerator
    pub vhost: Option<bool>,
    /// file descriptor of an already opened vhost net device
    pub vhostfd: Option<String>,
    /// file descriptors of multiple already opened vhost net
    /// devices
    #[qapi(added = "1.4.0")]
    pub vhostfds: Option<String>,
    /// vhost on for non-MSIX virtio guests
    pub vhostforce: Option<bool>,
    /// enable the IFF_VNET_HDR flag on the tap interface
    pub vnet_hdr: Option<bool>,
}

#[qapi(added = "11.1.0")]
pub struct NetdevUserDomainSuffix {
    /// DNS domain name suffix for host name lookup, similar to
    /// resolv.conf(5)
    pub str: String,
}

#[qapi(added = "11.1.0")]
pub struct NetdevUserGuestForward {
    pub str: String,
}

#[qapi(added = "11.1.0")]
pub struct NetdevUserHostForward {
    pub str: String,
}

/// Use the user mode network stack which requires no administrator
/// privilege to run.
///
#[qapi(added = "1.2.0")]
pub struct NetdevUserOptions {
    /// BOOTP filename, for use with tftp=
    pub bootfile: Option<String>,
    /// the first of the 16 IPs the built-in DHCP server can
    /// assign
    pub dhcpstart: Option<String>,
    /// guest-visible address of the virtual nameserver
    pub dns: Option<String>,
    /// list of DNS suffixes to search, passed as DHCP option to
    /// the guest
    #[qapi(added = "1.3.0")]
    pub dnssearch: Option<Vec<NetdevUserDomainSuffix>>,
    /// guest-visible domain name of the virtual nameserver
    /// (since 3.0)
    #[qapi(added = "3.0.0")]
    pub domainname: Option<String>,
    /// forward guest TCP connections
    pub guestfwd: Option<Vec<NetdevUserGuestForward>>,
    /// guest-visible address of the host
    pub host: Option<String>,
    /// redirect incoming TCP, UDP or UNIX host connections to
    /// guest endpoints
    pub hostfwd: Option<Vec<NetdevUserHostForward>>,
    /// client hostname reported by the builtin DHCP server
    pub hostname: Option<String>,
    /// legacy parameter, use net= instead
    pub ip: Option<String>,
    /// whether to support IPv4, default true for enabled (since 2.6)
    #[qapi(added = "2.6.0")]
    pub ipv4: Option<bool>,
    /// whether to support IPv6, default true for enabled (since 2.6)
    #[qapi(added = "2.6.0")]
    pub ipv6: Option<bool>,
    /// guest-visible IPv6 address of the virtual nameserver
    /// (since 2.6)
    #[qapi(name = "ipv6-dns")]
    #[qapi(added = "2.6.0")]
    pub ipv6_dns: Option<String>,
    /// guest-visible IPv6 address of the host (since 2.6)
    #[qapi(name = "ipv6-host")]
    #[qapi(added = "2.6.0")]
    pub ipv6_host: Option<String>,
    /// IPv6 network prefix (default is fec0::).  The network
    /// prefix is given in the usual hexadecimal IPv6 address notation.
    /// (since 2.6)
    #[qapi(name = "ipv6-prefix")]
    #[qapi(added = "2.6.0")]
    pub ipv6_prefix: Option<String>,
    /// IPv6 network prefix length (default is 64)
    /// (since 2.6)
    #[qapi(name = "ipv6-prefixlen")]
    #[qapi(added = "2.6.0")]
    pub ipv6_prefixlen: Option<i64>,
    /// IP network address that the guest will see, in the form
    /// addr[/netmask] The netmask is optional, and can be either in the
    /// form a.b.c.d or as a number of valid top-most bits.  Default is
    /// 10.0.2.0/24.
    pub net: Option<String>,
    /// isolate the guest from the host
    pub restrict: Option<bool>,
    /// root directory of the built-in SMB server
    pub smb: Option<String>,
    /// IP address of the built-in SMB server
    pub smbserver: Option<String>,
    /// root directory of the built-in TFTP server
    pub tftp: Option<String>,
    /// RFC2132 "TFTP server name" string (Since 3.1)
    #[qapi(name = "tftp-server-name")]
    #[qapi(added = "3.1.0")]
    pub tftp_server_name: Option<String>,
}

/// Connect to a vde switch running on the host.
///
#[qapi(added = "1.2.0")]
pub struct NetdevVdeOptions {
    /// group owner of socket
    pub group: Option<String>,
    /// permissions for socket
    pub mode: Option<u16>,
    /// port number
    pub port: Option<u16>,
    /// socket path
    pub sock: Option<String>,
}

/// Vhost-user network backend
///
#[qapi(added = "2.1.0")]
pub struct NetdevVhostUserOptions {
    /// name of a unix socket chardev
    pub chardev: String,
    /// number of queues to be created for multiqueue vhost-user
    /// (default: 1) (Since 2.5)
    #[qapi(added = "2.5.0")]
    pub queues: Option<i64>,
    /// vhost on for non-MSIX virtio guests (default: false).
    pub vhostforce: Option<bool>,
}

/// Vhost-vdpa network backend
///
/// vDPA device is a device that uses a datapath which complies with the virtio
/// specifications with a vendor specific control path.
///
#[qapi(name = "NetdevVhostVDPAOptions")]
#[qapi(added = "5.1.0")]
pub struct NetdevVhostVdpaOptions {
    /// number of queues to be created for multiqueue vhost-vdpa
    /// (default: 1)
    pub queues: Option<i64>,
    /// path of vhost-vdpa device
    /// (default:'/dev/vhost-vdpa-0')
    pub vhostdev: Option<String>,
    #[qapi(added = "7.2.0")]
    pub vhostfd: Option<String>,
    #[qapi(name = "x-svq")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "7.1.0")]
    pub x_svq: Option<bool>,
}

/// vmnet (bridged mode) network backend.
///
/// Bridges the vmnet interface with a physical network interface.
///
#[qapi(condition = "CONFIG_VMNET")]
#[qapi(added = "7.1.0")]
pub struct NetdevVmnetBridgedOptions {
    /// The name of the physical interface to be bridged.
    pub ifname: String,
    /// Enable isolation for this interface.  Interface isolation
    /// ensures that vmnet interface is not able to communicate with any
    /// other vmnet interfaces.  Only communication with host is
    /// allowed.  Requires at least macOS Big Sur 11.0.
    pub isolated: Option<bool>,
}

/// vmnet (host mode) network backend.
///
/// Allows the vmnet interface to communicate with other vmnet
/// interfaces that are in host mode and also with the host.
///
#[qapi(condition = "CONFIG_VMNET")]
#[qapi(added = "7.1.0")]
pub struct NetdevVmnetHostOptions {
    /// The DHCP IPv4 range end address to use for the
    /// interface.  Must be in the private IP range (RFC 1918).  Must be
    /// specified along with @start-address and @subnet-mask.
    #[qapi(name = "end-address")]
    pub end_address: Option<String>,
    /// Enable isolation for this interface.  Interface isolation
    /// ensures that vmnet interface is not able to communicate with any
    /// other vmnet interfaces.  Only communication with host is
    /// allowed.  Requires at least macOS Big Sur 11.0.
    pub isolated: Option<bool>,
    /// The identifier (UUID) to uniquely identify the isolated
    /// network vmnet interface should be added to.  If set, no DHCP
    /// service is provided for this interface and network communication
    /// is allowed only with other interfaces added to this network
    /// identified by the UUID.  Requires at least macOS Big Sur 11.0.
    #[qapi(name = "net-uuid")]
    pub net_uuid: Option<String>,
    /// The starting IPv4 address to use for the interface.
    /// Must be in the private IP range (RFC 1918).  Must be specified
    /// along with @end-address and @subnet-mask.  This address is used
    /// as the gateway address.  The subsequent address up to and
    /// including end-address are placed in the DHCP pool.
    #[qapi(name = "start-address")]
    pub start_address: Option<String>,
    /// The IPv4 subnet mask to use on the interface.  Must be
    /// specified along with @start-address and @subnet-mask.
    #[qapi(name = "subnet-mask")]
    pub subnet_mask: Option<String>,
}

/// vmnet (shared mode) network backend.
///
/// Allows traffic originating from the vmnet interface to reach the
/// Internet through a network address translator (NAT).  The vmnet
/// interface can communicate with the host and with other shared mode
/// interfaces on the same subnet.  If no DHCP settings, subnet mask and
/// IPv6 prefix specified, the interface can communicate with any of
/// other interfaces in shared mode.
///
#[qapi(condition = "CONFIG_VMNET")]
#[qapi(added = "7.1.0")]
pub struct NetdevVmnetSharedOptions {
    /// The DHCP IPv4 range end address to use for the
    /// interface.  Must be in the private IP range (RFC 1918).  Must be
    /// specified along with @start-address and @subnet-mask.
    #[qapi(name = "end-address")]
    pub end_address: Option<String>,
    /// Enable isolation for this interface.  Interface isolation
    /// ensures that vmnet interface is not able to communicate with any
    /// other vmnet interfaces.  Only communication with host is
    /// allowed.  Requires at least macOS Big Sur 11.0.
    pub isolated: Option<bool>,
    /// The IPv6 prefix to use into guest network.  Must be a
    /// unique local address i.e. start with fd00::/8 and have length of
    /// 64.
    #[qapi(name = "nat66-prefix")]
    pub nat66_prefix: Option<String>,
    /// The starting IPv4 address to use for the interface.
    /// Must be in the private IP range (RFC 1918).  Must be specified
    /// along with @end-address and @subnet-mask.  This address is used
    /// as the gateway address.  The subsequent address up to and
    /// including end-address are placed in the DHCP pool.
    #[qapi(name = "start-address")]
    pub start_address: Option<String>,
    /// The IPv4 subnet mask to use on the interface.  Must be
    /// specified along with @start-address and @subnet-mask.
    #[qapi(name = "subnet-mask")]
    pub subnet_mask: Option<String>,
}

/// Indicates whether a netfilter is attached to a netdev's transmit
/// queue or receive queue or both.
///
#[qapi(added = "2.5.0")]
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

/// Indicates where to insert a netfilter relative to a given other
/// filter.
///
#[qapi(added = "6.0.0")]
pub enum NetfilterInsert {
    /// insert before the specified filter
    #[qapi(name = "before")]
    Before,
    /// insert behind the specified filter
    #[qapi(name = "behind")]
    Behind,
}

/// Properties for objects of classes derived from netfilter.
///
#[qapi(added = "6.0.0")]
pub struct NetfilterProperties {
    /// where to insert the filter relative to the filter given in
    pub insert: Option<NetfilterInsert>,
    /// id of the network device backend to filter
    pub netdev: String,
    /// specifies where the filter should be inserted in the
    /// filter list.  "head" means the filter is inserted at the head of
    /// the filter list, before any existing filters.  "tail" means the
    /// filter is inserted at the tail of the filter list, behind any
    /// existing filters (default).  "id=<id>" means the filter is
    /// inserted before or behind the filter specified by <id>,
    /// depending on the @insert property.  (default: "tail")
    ///
    ///   .: Ignored if @position is "head" or "tail". (default: behind)
    pub position: Option<String>,
    /// indicates which queue(s) to filter (default: all)
    pub queue: Option<NetFilterDirection>,
    /// indicates whether the filter is enabled ("on") or disabled
    /// ("off") (default: "on")
    pub status: Option<String>,
}

/// Captures the configuration of a network device; legacy.
///
#[qapi(added = "1.2.0")]
#[qapi(removed = "5.1.0")]
pub struct NetLegacy {
    /// identifier for monitor commands
    pub id: Option<String>,
    /// identifier for monitor commands, ignored if @id is present
    pub name: Option<String>,
    /// device type specific properties (legacy)
    pub opts: NetLegacyOptions,
    /// vlan number
    #[qapi(removed = "3.0.0")]
    pub vlan: Option<i32>,
}

/// Create a new Network Interface Card.
///
#[qapi(added = "1.2.0")]
pub struct NetLegacyNicOptions {
    /// PCI device address
    pub addr: Option<String>,
    /// MAC address
    pub macaddr: Option<String>,
    /// device model (e1000, rtl8139, virtio etc.)
    pub model: Option<String>,
    /// id of -netdev to connect to
    pub netdev: Option<String>,
    /// number of MSI-x vectors, 0 to disable MSI-X
    pub vectors: Option<u32>,
}

/// Like Netdev, but for use only by the legacy command line options
///
#[qapi(added = "2.7.0")]
#[qapi(removed = "5.1.0")]
pub struct NetLegacyOptions {
    #[qapi(discriminator)]
    #[qapi(name = "type")]
    #[qapi(removed = "2.9.0")]
    pub r_type: Option<NetLegacyOptionsType>,
    #[qapi(discriminator)]
    #[qapi(added = "2.9.0")]
    pub r#type: Option<NetLegacyOptionsType>,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<NetLegacyOptionsBranch>,
}

#[qapi(added = "2.7.0")]
#[qapi(removed = "5.1.0")]
pub enum NetLegacyOptionsBranch {
    #[qapi(name = "bridge")]
    Bridge(NetdevBridgeOptions),
    #[qapi(name = "dump")]
    #[qapi(removed = "2.12.0")]
    Dump(NetdevDumpOptions),
    #[qapi(name = "l2tpv3")]
    L2tpv3(NetdevL2tPv3Options),
    #[qapi(name = "netmap")]
    Netmap(NetdevNetmapOptions),
    #[qapi(name = "nic")]
    Nic(NetLegacyNicOptions),
    #[qapi(name = "none")]
    None,
    #[qapi(name = "socket")]
    Socket(NetdevSocketOptions),
    #[qapi(name = "tap")]
    Tap(NetdevTapOptions),
    #[qapi(name = "user")]
    User(NetdevUserOptions),
    #[qapi(name = "vde")]
    Vde(NetdevVdeOptions),
    #[qapi(name = "vhost-user")]
    VhostUser(NetdevVhostUserOptions),
}

#[qapi(added = "2.7.0")]
#[qapi(removed = "5.1.0")]
pub enum NetLegacyOptionsType {
    #[qapi(name = "bridge")]
    Bridge,
    #[qapi(name = "dump")]
    #[qapi(removed = "2.12.0")]
    Dump,
    #[qapi(name = "l2tpv3")]
    L2tpv3,
    #[qapi(name = "netmap")]
    Netmap,
    #[qapi(name = "nic")]
    Nic,
    #[qapi(name = "none")]
    None,
    #[qapi(name = "socket")]
    Socket,
    #[qapi(name = "tap")]
    Tap,
    #[qapi(name = "user")]
    User,
    #[qapi(name = "vde")]
    Vde,
    #[qapi(name = "vhost-user")]
    VhostUser,
}

/// The network address family
///
#[qapi(added = "2.1.0")]
pub enum NetworkAddressFamily {
    /// IPV4 family
    #[qapi(name = "ipv4")]
    Ipv4,
    /// IPV6 family
    #[qapi(name = "ipv6")]
    Ipv6,
    /// unix socket
    #[qapi(name = "unix")]
    Unix,
    /// otherwise
    #[qapi(name = "unknown")]
    Unknown,
    /// vsock family (since 2.8)
    #[qapi(name = "vsock")]
    #[qapi(added = "2.8.0")]
    Vsock,
}

/// An enumeration that tells QEMU how to set the backing file path in a
/// new image file.
///
#[qapi(added = "1.1.0")]
pub enum NewImageMode {
    /// QEMU should create a new image with absolute paths
    /// for the backing file.  If there is no backing file available,
    /// the new image will not be backed either.
    #[qapi(name = "absolute-paths")]
    AbsolutePaths,
    /// QEMU should look for an existing image file.
    #[qapi(name = "existing")]
    Existing,
}

/// Captures the address of the socket
///
#[qapi(name = "NFSServer")]
#[qapi(added = "2.8.0")]
pub struct NfsServer {
    /// host address for NFS server
    pub host: String,
    /// transport type used for NFS (only TCP supported)
    pub r#type: NfsTransport,
}

/// An enumeration of NFS transport types
///
#[qapi(name = "NFSTransport")]
#[qapi(added = "2.8.0")]
pub enum NfsTransport {
    /// TCP transport
    #[qapi(name = "inet")]
    Inet,
}

/// An enumeration of the options specified when enabling notify VM exit
///
#[qapi(added = "7.2.0")]
pub enum NotifyVmexitOption {
    /// disable the feature.
    #[qapi(name = "disable")]
    Disable,
    /// enable the feature, raise a internal error if the
    /// notify VM exit happens.
    #[qapi(name = "internal-error")]
    InternalError,
    /// enable the feature, do nothing and continue if the notify VM
    /// exit happens.
    #[qapi(name = "run")]
    Run,
}

/// Option "-numa cpu" overrides default cpu to node mapping.  It
/// accepts the same set of cpu properties as returned by
/// `query-hotpluggable-cpus[].props <query-hotpluggable-cpus>`, where
/// node-id could be used to override default node mapping.
///
#[qapi(added = "2.10.0")]
pub struct NumaCpuOptions {
    /// book number within parent container the CPU belongs to
    /// (since 8.2)
    #[qapi(name = "book-id")]
    #[qapi(added = "8.2.0")]
    pub book_id: Option<i64>,
    /// cluster number within the parent container the CPU
    /// belongs to (since 7.1)
    #[qapi(name = "cluster-id")]
    #[qapi(added = "7.1.0")]
    pub cluster_id: Option<i64>,
    /// core number within the parent container the CPU belongs to
    #[qapi(name = "core-id")]
    pub core_id: Option<i64>,
    /// die number within the parent container the CPU belongs to
    /// (since 4.1)
    #[qapi(name = "die-id")]
    #[qapi(added = "4.1.0")]
    pub die_id: Option<i64>,
    /// drawer number within CPU topology the CPU belongs to
    /// (since 8.2)
    #[qapi(name = "drawer-id")]
    #[qapi(added = "8.2.0")]
    pub drawer_id: Option<i64>,
    /// module number within the parent container the CPU
    /// belongs to (since 9.1)
    #[qapi(name = "module-id")]
    #[qapi(added = "9.1.0")]
    pub module_id: Option<i64>,
    /// NUMA node ID the CPU belongs to
    #[qapi(name = "node-id")]
    pub node_id: Option<i64>,
    /// socket number within parent container the CPU belongs to
    #[qapi(name = "socket-id")]
    pub socket_id: Option<i64>,
    /// thread number within the core the CPU  belongs to
    #[qapi(name = "thread-id")]
    pub thread_id: Option<i64>,
}

/// Set the distance between 2 NUMA nodes.
///
#[qapi(added = "2.10.0")]
pub struct NumaDistOptions {
    /// destination NUMA node.
    pub dst: u16,
    /// source NUMA node.
    pub src: u16,
    /// NUMA distance from source node to destination node.  When a
    /// node is unreachable from another node, set the distance between
    /// them to 255.
    pub val: u8,
}

/// Set the memory side cache information for a given memory domain.
///
/// For more information of `NumaHmatCacheOptions`, see chapter
/// 5.2.27.5: Table 5-147: Field "Cache Attributes" of ACPI 6.3 spec.
///
#[qapi(added = "5.0.0")]
pub struct NumaHmatCacheOptions {
    /// the cache associativity,
    /// none/direct-mapped/complex(complex cache indexing).
    pub associativity: HmatCacheAssociativity,
    /// the cache level described in this structure.
    pub level: u8,
    /// the cache line size in bytes.
    pub line: u16,
    /// the memory proximity domain to which the memory belongs.
    #[qapi(name = "node-id")]
    pub node_id: u32,
    /// the write policy, none/write-back/write-through.
    pub policy: HmatCacheWritePolicy,
    /// the size of memory side cache in bytes.
    pub size: u64,
}

/// Set the system locality latency and bandwidth information between
/// Initiator and Target proximity Domains.
///
/// For more information about `NumaHmatLBOptions`, see chapter
/// 5.2.27.4: Table 5-146 of ACPI 6.3 spec.
///
#[qapi(name = "NumaHmatLBOptions")]
#[qapi(added = "5.0.0")]
pub struct NumaHmatLbOptions {
    /// the value of bandwidth between @initiator and @target
    /// proximity domain, the bandwidth unit is "Bytes per second".
    pub bandwidth: Option<u64>,
    /// presents the type of data, access/read/write latency or
    /// hit latency.
    #[qapi(name = "data-type")]
    pub data_type: HmatLbDataType,
    /// the Memory Hierarchy.  Indicates the performance of
    /// memory or side cache.
    pub hierarchy: HmatLbMemoryHierarchy,
    /// the Initiator Proximity Domain.
    pub initiator: u16,
    /// the value of latency from @initiator to @target proximity
    /// domain, the latency unit is "ns(nanosecond)".
    pub latency: Option<u64>,
    /// the Target Proximity Domain.
    pub target: u16,
}

/// Create a guest NUMA node.  (for OptsVisitor)
///
#[qapi(added = "2.1.0")]
pub struct NumaNodeOptions {
    /// VCPUs belonging to this node (assign VCPUS round-robin if
    /// omitted)
    pub cpus: Option<Vec<u16>>,
    /// defined in ACPI 6.3 Chapter 5.2.27.3 Table 5-145, points
    /// to the nodeid which has the memory controller responsible for
    /// this NUMA node.  This field provides additional information as
    /// to the initiator node that is closest (as in directly attached)
    /// to this node, and therefore has the best performance (since 5.0)
    #[qapi(added = "5.0.0")]
    pub initiator: Option<u16>,
    /// memory size of this node; mutually exclusive with @memdev.
    /// Equally divide total memory among nodes if both @mem and @memdev
    /// are omitted.
    pub mem: Option<u64>,
    /// memory backend object.  If specified for one node, it must
    /// be specified for all nodes.
    pub memdev: Option<String>,
    /// NUMA node ID (increase by 1 from 0 if omitted)
    pub nodeid: Option<u16>,
}

/// A discriminated record of NUMA options.  (for OptsVisitor)
///
#[qapi(added = "2.1.0")]
pub struct NumaOptions {
    #[qapi(discriminator)]
    #[qapi(name = "type")]
    #[qapi(removed = "2.9.0")]
    pub r_type: Option<NumaOptionsType>,
    /// NUMA option type
    #[qapi(discriminator)]
    #[qapi(added = "2.9.0")]
    pub r#type: Option<NumaOptionsType>,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<NumaOptionsBranch>,
}

#[qapi(added = "2.1.0")]
pub enum NumaOptionsBranch {
    #[qapi(name = "cpu")]
    #[qapi(added = "2.10.0")]
    Cpu(NumaCpuOptions),
    #[qapi(name = "dist")]
    #[qapi(added = "2.10.0")]
    Dist(NumaDistOptions),
    #[qapi(name = "hmat-cache")]
    #[qapi(added = "5.0.0")]
    HmatCache(NumaHmatCacheOptions),
    #[qapi(name = "hmat-lb")]
    #[qapi(added = "5.0.0")]
    HmatLb(NumaHmatLbOptions),
    #[qapi(name = "node")]
    Node(NumaNodeOptions),
}

#[qapi(added = "2.1.0")]
pub enum NumaOptionsType {
    /// property based CPU(s) to node mapping (Since: 2.10)
    #[qapi(name = "cpu")]
    #[qapi(added = "2.10.0")]
    Cpu,
    /// NUMA distance configuration (since 2.10)
    #[qapi(name = "dist")]
    #[qapi(added = "2.10.0")]
    Dist,
    /// memory side cache information (Since: 5.0)
    #[qapi(name = "hmat-cache")]
    #[qapi(added = "5.0.0")]
    HmatCache,
    /// memory latency and bandwidth information (Since: 5.0)
    #[qapi(name = "hmat-lb")]
    #[qapi(added = "5.0.0")]
    HmatLb,
    /// NUMA nodes configuration
    #[qapi(name = "node")]
    Node,
}

/// SMMUv3 Output Address Size configuration mode.
///
#[qapi(added = "11.0.0")]
pub enum OasMode {
    /// 32-bit output address size
    #[qapi(name = "32")]
    _32,
    /// 36-bit output address size
    #[qapi(name = "36")]
    _36,
    /// 40-bit output address size
    #[qapi(name = "40")]
    _40,
    /// 42-bit output address size
    #[qapi(name = "42")]
    _42,
    /// 44-bit output address size
    #[qapi(name = "44")]
    _44,
    /// 48-bit output address size
    #[qapi(name = "48")]
    _48,
    /// 52-bit output address size
    #[qapi(name = "52")]
    _52,
    /// 56-bit output address size
    #[qapi(name = "56")]
    _56,
    /// derive from host IOMMU capabilities
    #[qapi(name = "auto")]
    Auto,
}

/// Describes the options of a user creatable QOM object.
///
#[qapi(added = "6.0.0")]
pub struct ObjectOptions {
    /// the name of the new object
    pub id: String,
    /// the class name for the object to be created
    #[qapi(name = "qom-type")]
    #[qapi(discriminator)]
    pub qom_type: ObjectType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<ObjectOptionsBranch>,
}

#[qapi(added = "6.0.0")]
pub enum ObjectOptionsBranch {
    #[qapi(name = "acpi-generic-initiator")]
    #[qapi(added = "9.0.0")]
    AcpiGenericInitiator(AcpiGenericInitiatorProperties),
    #[qapi(name = "acpi-generic-port")]
    #[qapi(added = "9.2.0")]
    AcpiGenericPort(AcpiGenericPortProperties),
    #[qapi(name = "authz-list")]
    AuthzList(AuthZListProperties),
    #[qapi(name = "authz-listfile")]
    AuthzListfile(AuthZListFileProperties),
    #[qapi(name = "authz-pam")]
    AuthzPam(AuthZpamProperties),
    #[qapi(name = "authz-simple")]
    AuthzSimple(AuthZSimpleProperties),
    #[qapi(name = "can-bus")]
    CanBus,
    #[qapi(name = "can-host-socketcan")]
    #[qapi(condition = "CONFIG_LINUX")]
    CanHostSocketcan(CanHostSocketcanProperties),
    #[qapi(name = "colo-compare")]
    ColoCompare(ColoCompareProperties),
    #[qapi(name = "cryptodev-backend")]
    CryptodevBackend(CryptodevBackendProperties),
    #[qapi(name = "cryptodev-backend-builtin")]
    CryptodevBackendBuiltin(CryptodevBackendProperties),
    #[qapi(name = "cryptodev-backend-lkcf")]
    #[qapi(added = "7.2.0")]
    CryptodevBackendLkcf(CryptodevBackendProperties),
    #[qapi(name = "cryptodev-vhost-user")]
    #[qapi(condition = "defined(CONFIG_VHOST_CRYPTO)")]
    CryptodevVhostUser(CryptodevVhostUserProperties),
    #[qapi(name = "dbus-vmstate")]
    DbusVmstate(DBusVmStateProperties),
    #[qapi(name = "filter-buffer")]
    FilterBuffer(FilterBufferProperties),
    #[qapi(name = "filter-dump")]
    FilterDump(FilterDumpProperties),
    #[qapi(name = "filter-mirror")]
    FilterMirror(FilterMirrorProperties),
    #[qapi(name = "filter-redirector")]
    FilterRedirector(FilterRedirectorProperties),
    #[qapi(name = "filter-replay")]
    FilterReplay(NetfilterProperties),
    #[qapi(name = "filter-rewriter")]
    FilterRewriter(FilterRewriterProperties),
    #[qapi(name = "igvm-cfg")]
    #[qapi(condition = "CONFIG_IGVM")]
    #[qapi(added = "10.1.0")]
    IgvmCfg(IgvmCfgProperties),
    #[qapi(name = "input-barrier")]
    InputBarrier(InputBarrierProperties),
    #[qapi(name = "input-linux")]
    #[qapi(condition = "CONFIG_LINUX")]
    InputLinux(InputLinuxProperties),
    #[qapi(name = "iommufd")]
    #[qapi(added = "9.0.0")]
    Iommufd(IommufdProperties),
    #[qapi(name = "iothread")]
    Iothread(IothreadProperties),
    #[qapi(name = "main-loop")]
    #[qapi(added = "7.1.0")]
    MainLoop(MainLoopProperties),
    #[qapi(name = "memory-backend-epc")]
    #[qapi(condition = "CONFIG_LINUX")]
    #[qapi(added = "6.2.0")]
    MemoryBackendEpc(MemoryBackendEpcProperties),
    #[qapi(name = "memory-backend-file")]
    MemoryBackendFile(MemoryBackendFileProperties),
    #[qapi(name = "memory-backend-memfd")]
    #[qapi(condition = "defined(CONFIG_LINUX)")]
    MemoryBackendMemfd(MemoryBackendMemfdProperties),
    #[qapi(name = "memory-backend-ram")]
    MemoryBackendRam(MemoryBackendProperties),
    #[qapi(name = "memory-backend-shm")]
    #[qapi(condition = "CONFIG_POSIX")]
    #[qapi(added = "9.1.0")]
    MemoryBackendShm(MemoryBackendShmProperties),
    #[qapi(name = "monitor-hmp")]
    #[qapi(added = "11.1.0")]
    MonitorHmp(MonitorHmpProperties),
    #[qapi(name = "monitor-qmp")]
    #[qapi(added = "11.1.0")]
    MonitorQmp(MonitorQmpProperties),
    #[qapi(name = "pef-guest")]
    PefGuest,
    #[qapi(name = "pr-manager-helper")]
    #[qapi(condition = "CONFIG_LINUX")]
    PrManagerHelper(PrManagerHelperProperties),
    #[qapi(name = "qtest")]
    #[qapi(added = "6.1.0")]
    Qtest(QtestProperties),
    #[qapi(name = "rng-builtin")]
    RngBuiltin(RngProperties),
    #[qapi(name = "rng-egd")]
    RngEgd(RngEgdProperties),
    #[qapi(name = "rng-random")]
    #[qapi(condition = "CONFIG_POSIX")]
    RngRandom(RngRandomProperties),
    #[qapi(name = "s390-pv-guest")]
    S390PvGuest,
    #[qapi(name = "secret")]
    Secret(SecretProperties),
    #[qapi(name = "secret_keyring")]
    #[qapi(condition = "CONFIG_SECRET_KEYRING")]
    SecretKeyring(SecretKeyringProperties),
    #[qapi(name = "sev-guest")]
    SevGuest(SevGuestProperties),
    #[qapi(name = "sev-snp-guest")]
    #[qapi(added = "9.1.0")]
    SevSnpGuest(SevSnpGuestProperties),
    #[qapi(name = "tdx-guest")]
    #[qapi(added = "10.1.0")]
    TdxGuest(TdxGuestProperties),
    #[qapi(name = "thread-context")]
    #[qapi(added = "7.2.0")]
    ThreadContext(ThreadContextProperties),
    #[qapi(name = "throttle-group")]
    ThrottleGroup(ThrottleGroupProperties),
    #[qapi(name = "tls-cipher-suites")]
    TlsCipherSuites(TlsCredsProperties),
    #[qapi(name = "tls-creds-anon")]
    TlsCredsAnon(TlsCredsAnonProperties),
    #[qapi(name = "tls-creds-psk")]
    TlsCredsPsk(TlsCredsPskProperties),
    #[qapi(name = "tls-creds-x509")]
    TlsCredsX509(TlsCredsX509Properties),
    #[qapi(name = "x-remote-object")]
    XRemoteObject(RemoteObjectProperties),
    #[qapi(name = "x-vfio-user-server")]
    #[qapi(added = "7.1.0")]
    XVfioUserServer(VfioUserServerProperties),
}

#[qapi(added = "10.1.0")]
pub struct ObjectPropertiesValues {
    /// a list of properties.
    pub properties: Vec<ObjectPropertyValue>,
}

#[qapi(added = "1.1.0")]
pub struct ObjectPropertyInfo {
    /// the default value, if any (since 5.0)
    #[qapi(name = "default-value")]
    #[qapi(added = "5.0.0")]
    pub default_value: Option<serde_json::Value>,
    /// if specified, the description of the property.
    #[qapi(added = "2.12.0")]
    pub description: Option<String>,
    /// the name of the property
    pub name: String,
    /// the type of the property.  This will typically come in one of
    /// four forms:
    ///
    /// 1) A primitive type such as 'u8', 'u16', 'bool', 'str', or
    /// 'double'.  These types are mapped to the appropriate JSON
    /// type.
    ///
    /// 2) A child type in the form 'child<subtype>' where subtype is a
    /// qdev device type name.  Child properties create the
    /// composition tree.
    ///
    /// 3) A link type in the form 'link<subtype>' where subtype is a
    /// qdev device type name.  Link properties form the device model
    /// graph.
    pub r#type: String,
}

#[qapi(added = "10.1.0")]
pub struct ObjectPropertyValue {
    /// the name of the property.
    pub name: String,
    /// the type of the property, as described in
    /// `ObjectPropertyInfo`.
    pub r#type: String,
    /// the value of the property.  Absent when the property cannot
    /// be read.
    pub value: Option<serde_json::Value>,
}

#[qapi(added = "6.0.0")]
pub enum ObjectType {
    #[qapi(name = "acpi-generic-initiator")]
    #[qapi(added = "9.0.0")]
    AcpiGenericInitiator,
    #[qapi(name = "acpi-generic-port")]
    #[qapi(added = "9.2.0")]
    AcpiGenericPort,
    #[qapi(name = "authz-list")]
    AuthzList,
    #[qapi(name = "authz-listfile")]
    AuthzListfile,
    #[qapi(name = "authz-pam")]
    AuthzPam,
    #[qapi(name = "authz-simple")]
    AuthzSimple,
    #[qapi(name = "can-bus")]
    CanBus,
    #[qapi(name = "can-host-socketcan")]
    #[qapi(condition = "CONFIG_LINUX")]
    CanHostSocketcan,
    #[qapi(name = "colo-compare")]
    ColoCompare,
    #[qapi(name = "cryptodev-backend")]
    CryptodevBackend,
    #[qapi(name = "cryptodev-backend-builtin")]
    CryptodevBackendBuiltin,
    #[qapi(name = "cryptodev-backend-lkcf")]
    #[qapi(added = "7.2.0")]
    CryptodevBackendLkcf,
    #[qapi(name = "cryptodev-vhost-user")]
    #[qapi(condition = "defined(CONFIG_VHOST_CRYPTO)")]
    CryptodevVhostUser,
    #[qapi(name = "dbus-vmstate")]
    DbusVmstate,
    #[qapi(name = "filter-buffer")]
    FilterBuffer,
    #[qapi(name = "filter-dump")]
    FilterDump,
    #[qapi(name = "filter-mirror")]
    FilterMirror,
    #[qapi(name = "filter-redirector")]
    FilterRedirector,
    #[qapi(name = "filter-replay")]
    FilterReplay,
    #[qapi(name = "filter-rewriter")]
    FilterRewriter,
    #[qapi(name = "igvm-cfg")]
    #[qapi(condition = "CONFIG_IGVM")]
    #[qapi(added = "10.1.0")]
    IgvmCfg,
    #[qapi(name = "input-barrier")]
    InputBarrier,
    #[qapi(name = "input-linux")]
    #[qapi(condition = "CONFIG_LINUX")]
    InputLinux,
    #[qapi(name = "iommufd")]
    #[qapi(added = "9.0.0")]
    Iommufd,
    #[qapi(name = "iothread")]
    Iothread,
    #[qapi(name = "main-loop")]
    #[qapi(added = "7.1.0")]
    MainLoop,
    #[qapi(name = "memory-backend-epc")]
    #[qapi(condition = "CONFIG_LINUX")]
    #[qapi(added = "6.2.0")]
    MemoryBackendEpc,
    #[qapi(name = "memory-backend-file")]
    MemoryBackendFile,
    #[qapi(name = "memory-backend-memfd")]
    #[qapi(condition = "defined(CONFIG_LINUX)")]
    MemoryBackendMemfd,
    #[qapi(name = "memory-backend-ram")]
    MemoryBackendRam,
    #[qapi(name = "memory-backend-shm")]
    #[qapi(condition = "CONFIG_POSIX")]
    #[qapi(added = "9.1.0")]
    MemoryBackendShm,
    #[qapi(name = "monitor-hmp")]
    #[qapi(added = "11.1.0")]
    MonitorHmp,
    #[qapi(name = "monitor-qmp")]
    #[qapi(added = "11.1.0")]
    MonitorQmp,
    #[qapi(name = "pef-guest")]
    PefGuest,
    #[qapi(name = "pr-manager-helper")]
    #[qapi(condition = "CONFIG_LINUX")]
    PrManagerHelper,
    #[qapi(name = "qtest")]
    #[qapi(added = "6.1.0")]
    Qtest,
    #[qapi(name = "rng-builtin")]
    RngBuiltin,
    #[qapi(name = "rng-egd")]
    RngEgd,
    #[qapi(name = "rng-random")]
    #[qapi(condition = "CONFIG_POSIX")]
    RngRandom,
    #[qapi(name = "s390-pv-guest")]
    S390PvGuest,
    #[qapi(name = "secret")]
    Secret,
    #[qapi(name = "secret_keyring")]
    #[qapi(condition = "CONFIG_SECRET_KEYRING")]
    SecretKeyring,
    #[qapi(name = "sev-guest")]
    SevGuest,
    #[qapi(name = "sev-snp-guest")]
    #[qapi(added = "9.1.0")]
    SevSnpGuest,
    #[qapi(name = "tdx-guest")]
    #[qapi(added = "10.1.0")]
    TdxGuest,
    #[qapi(name = "thread-context")]
    #[qapi(added = "7.2.0")]
    ThreadContext,
    #[qapi(name = "throttle-group")]
    ThrottleGroup,
    #[qapi(name = "tls-cipher-suites")]
    TlsCipherSuites,
    #[qapi(name = "tls-creds-anon")]
    TlsCredsAnon,
    #[qapi(name = "tls-creds-psk")]
    TlsCredsPsk,
    #[qapi(name = "tls-creds-x509")]
    TlsCredsX509,
    #[qapi(name = "x-remote-object")]
    #[qapi(feature = "unstable")]
    XRemoteObject,
    #[qapi(name = "x-vfio-user-server")]
    #[qapi(feature = "unstable")]
    #[qapi(added = "7.1.0")]
    XVfioUserServer,
}

/// This structure describes a search result from `qom-list-types`
///
#[qapi(added = "1.1.0")]
pub struct ObjectTypeInfo {
    /// the type is abstract and can't be directly instantiated.
    /// Omitted if false.  (since 2.10)
    #[qapi(added = "2.10.0")]
    pub r#abstract: Option<bool>,
    /// the type name found in the search
    pub name: String,
    /// Name of parent type, if any (since 2.10)
    #[qapi(added = "2.10.0")]
    pub parent: Option<String>,
}

/// An enumeration of options for specifying a PCI BAR
///
#[qapi(name = "OffAutoPCIBAR")]
#[qapi(added = "2.12.0")]
pub enum OffAutoPcibar {
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
    /// The specified feature is disabled
    #[qapi(name = "off")]
    Off,
}

/// An enumeration of possible behaviors for copy-before-write operation
/// failures.
///
#[qapi(added = "7.1.0")]
pub enum OnCbwError {
    /// report the error to the guest.  This way, the
    /// guest will not be able to overwrite areas that cannot be backed
    /// up, so the backup process remains valid.
    #[qapi(name = "break-guest-write")]
    BreakGuestWrite,
    /// continue guest write.  Doing so will make the
    /// provided snapshot state invalid and any backup or export process
    /// based on it will finally fail.
    #[qapi(name = "break-snapshot")]
    BreakSnapshot,
}

/// An enumeration of three options: on, off, and auto
///
#[qapi(added = "2.2.0")]
pub enum OnOffAuto {
    /// QEMU selects the value between on and off
    #[qapi(name = "auto")]
    Auto,
    /// Disabled
    #[qapi(name = "off")]
    Off,
    /// Enabled
    #[qapi(name = "on")]
    On,
}

/// An enumeration of three values: on, off, and split
///
#[qapi(added = "2.6.0")]
pub enum OnOffSplit {
    /// Disabled
    #[qapi(name = "off")]
    Off,
    /// Enabled
    #[qapi(name = "on")]
    On,
    /// Mixed
    #[qapi(name = "split")]
    Split,
}

#[qapi(added = "6.0.0")]
pub enum PanicAction {
    /// Shutdown the VM and exit with nonzero status
    /// (since 7.1)
    #[qapi(name = "exit-failure")]
    #[qapi(added = "7.1.0")]
    ExitFailure,
    /// Continue VM execution
    #[qapi(name = "none")]
    None,
    /// Pause the VM
    #[qapi(name = "pause")]
    Pause,
    /// Shutdown the VM and exit, according to the shutdown
    /// action
    #[qapi(name = "shutdown")]
    Shutdown,
}

#[qapi(added = "11.1.0")]
pub struct PasstParameter {
    /// Additional arguments for the passt executable, see passt(1)
    pub str: String,
}

#[qapi(added = "11.1.0")]
pub struct PasstPortForward {
    /// passt port forwarding specification, see passt(1) option
    /// --tcp-ports and --udp-ports.
    pub str: String,
}

#[qapi(added = "11.1.0")]
pub struct PasstSearch {
    /// DNS domain name suffix for host name lookup, or "none".  See
    /// passt(1) option --search.
    pub str: String,
}

/// PCDIMMDevice state information
///
#[qapi(name = "PCDIMMDeviceInfo")]
#[qapi(added = "2.1.0")]
pub struct PcdimmDeviceInfo {
    /// physical address, where device is mapped
    pub addr: i64,
    /// true if device if could be added/removed while
    /// machine is running
    pub hotpluggable: bool,
    /// true if device was hotplugged
    pub hotplugged: bool,
    /// device's ID
    pub id: Option<String>,
    /// memory backend linked with device
    pub memdev: String,
    /// NUMA node number where device is plugged in
    pub node: i64,
    /// size of memory that the device provides
    pub size: i64,
    /// slot number at which device is plugged in
    pub slot: i64,
}

#[qapi(name = "PCDIMMDeviceInfoWrapper")]
#[qapi(added = "6.2.0")]
pub struct PcdimmDeviceInfoWrapper {
    /// PCDIMMDevice state information
    pub data: PcdimmDeviceInfo,
}

/// Information about a PCI Bridge device
///
#[qapi(added = "1.0.0")]
pub struct PciBridgeInfo {
    /// information about the bus the device resides on
    pub bus: PciBusInfo,
    /// a list of `PciDeviceInfo` for each device on this bridge
    pub devices: Option<Box<Vec<PciDeviceInfo>>>,
}

#[qapi(added = "1.0.0")]
#[qapi(removed = "2.4.0")]
pub struct PciBridgeInfoBus {
    pub io_range: PciMemoryRange,
    pub memory_range: PciMemoryRange,
    pub number: i64,
    pub prefetchable_range: PciMemoryRange,
    pub secondary: i64,
    pub subordinate: i64,
}

/// Information about a bus of a PCI Bridge device
///
#[qapi(added = "2.4.0")]
pub struct PciBusInfo {
    /// The PIO range for all devices on this bridge
    pub io_range: PciMemoryRange,
    /// The MMIO range for all devices on this bridge
    pub memory_range: PciMemoryRange,
    /// primary bus interface number.  This should be the number of
    /// the bus the device resides on.
    pub number: i64,
    /// The range of prefetchable MMIO for all devices
    /// on this bridge
    pub prefetchable_range: PciMemoryRange,
    /// secondary bus interface number.  This is the number of
    /// the main bus for the bridge
    pub secondary: i64,
    /// This is the highest number bus that resides below the
    /// bridge.
    pub subordinate: i64,
}

/// Information about the Class of a PCI device
///
#[qapi(added = "2.4.0")]
pub struct PciDeviceClass {
    /// the class code of the device
    pub class: i64,
    /// a string description of the device's class (not stable, and
    /// should only be treated as informational)
    pub desc: Option<String>,
}

/// Information about the Id of a PCI device
///
#[qapi(added = "2.4.0")]
pub struct PciDeviceId {
    /// the PCI device id
    pub device: i64,
    /// the PCI subsystem id (since 3.1)
    #[qapi(added = "3.1.0")]
    pub subsystem: Option<i64>,
    /// the PCI subsystem vendor id (since 3.1)
    #[qapi(name = "subsystem-vendor")]
    #[qapi(added = "3.1.0")]
    pub subsystem_vendor: Option<i64>,
    /// the PCI vendor id
    pub vendor: i64,
}

/// Information about a PCI device
///
#[qapi(added = "1.0.0")]
pub struct PciDeviceInfo {
    /// the bus number of the device
    pub bus: i64,
    /// the class of the device
    pub class_info: PciDeviceClass,
    /// the function of the slot used by the device
    pub function: i64,
    /// the PCI device id
    pub id: PciDeviceId,
    /// if an IRQ is assigned to the device, the IRQ number
    pub irq: Option<i64>,
    /// the IRQ pin, zero means no IRQ (since 5.1)
    #[qapi(added = "5.1.0")]
    pub irq_pin: Option<i64>,
    /// if the device is a PCI bridge, the bridge information
    pub pci_bridge: Option<Box<PciBridgeInfo>>,
    /// the device name of the PCI device
    pub qdev_id: String,
    /// a list of the PCI I/O regions associated with the device
    pub regions: Vec<PciMemoryRegion>,
    /// the slot the device is located in
    pub slot: i64,
}

#[qapi(added = "1.0.0")]
#[qapi(removed = "2.4.0")]
pub struct PciDeviceInfoClassInfo {
    pub class: i64,
    pub desc: Option<String>,
}

#[qapi(added = "1.0.0")]
#[qapi(removed = "2.4.0")]
pub struct PciDeviceInfoId {
    pub device: i64,
    pub vendor: i64,
}

/// An enumeration of PCIe link speeds in units of GT/s
///
#[qapi(name = "PCIELinkSpeed")]
#[qapi(added = "4.0.0")]
pub enum PcieLinkSpeed {
    /// 16.0GT/s
    #[qapi(name = "16")]
    _16,
    /// 2.5GT/s
    #[qapi(name = "2_5")]
    _25,
    /// 32.0GT/s (since 9.0)
    #[qapi(name = "32")]
    #[qapi(added = "9.0.0")]
    _32,
    /// 5.0GT/s
    #[qapi(name = "5")]
    _5,
    /// 64.0GT/s (since 9.0)
    #[qapi(name = "64")]
    #[qapi(added = "9.0.0")]
    _64,
    /// 8.0GT/s
    #[qapi(name = "8")]
    _8,
}

/// An enumeration of PCIe link width
///
#[qapi(name = "PCIELinkWidth")]
#[qapi(added = "4.0.0")]
pub enum PcieLinkWidth {
    /// x1
    #[qapi(name = "1")]
    _1,
    /// x12
    #[qapi(name = "12")]
    _12,
    /// x16
    #[qapi(name = "16")]
    _16,
    /// x2
    #[qapi(name = "2")]
    _2,
    /// x32
    #[qapi(name = "32")]
    _32,
    /// x4
    #[qapi(name = "4")]
    _4,
    /// x8
    #[qapi(name = "8")]
    _8,
}

/// Information about a PCI bus
///
#[qapi(added = "1.0.0")]
pub struct PciInfo {
    /// the bus index
    pub bus: i64,
    /// a list of devices on this bus
    pub devices: Vec<PciDeviceInfo>,
}

/// A PCI device memory region
///
#[qapi(added = "1.0.0")]
pub struct PciMemoryRange {
    /// the starting address (guest physical)
    pub base: i64,
    /// the ending address (guest physical)
    pub limit: i64,
}

/// Information about a PCI device I/O region.
///
#[qapi(added = "1.0.0")]
pub struct PciMemoryRegion {
    /// memory address
    pub address: i64,
    /// the index of the Base Address Register for this region
    pub bar: i64,
    /// if @type is 'memory', true if the BAR is 64-bit
    pub mem_type_64: Option<bool>,
    /// if @type is 'memory', true if the memory is prefetchable
    pub prefetch: Option<bool>,
    /// memory size
    pub size: i64,
    /// - 'io' if the region is a PIO region
    /// - 'memory' if the region is a MMIO region
    pub r#type: String,
}

/// Preallocation mode of QEMU image file
///
#[qapi(added = "2.2.0")]
pub enum PreallocMode {
    /// like @full preallocation but allocate disk space by
    /// posix_fallocate() rather than writing data.
    #[qapi(name = "falloc")]
    Falloc,
    /// preallocate all data by writing it to the device to ensure
    /// disk space is really available.  This data may or may not be
    /// zero, depending on the image format and storage.  @full
    /// preallocation also sets up metadata correctly.
    #[qapi(name = "full")]
    Full,
    /// preallocate only for metadata
    #[qapi(name = "metadata")]
    Metadata,
    /// no preallocation
    #[qapi(name = "off")]
    Off,
}

/// Properties for pr-manager-helper objects.
///
#[qapi(condition = "CONFIG_LINUX")]
#[qapi(added = "6.0.0")]
pub struct PrManagerHelperProperties {
    /// the path to a Unix domain socket for connecting to the
    /// external helper
    pub path: String,
}

/// Information about a persistent reservation manager
///
#[qapi(name = "PRManagerInfo")]
#[qapi(added = "3.0.0")]
pub struct PrManagerInfo {
    /// true if the persistent reservation manager is connected
    /// to the underlying storage or helper
    pub connected: bool,
    /// the identifier of the persistent reservation manager
    pub id: String,
}

/// QEMU error classes
///
#[qapi(added = "2.6.0")]
pub enum QapiErrorClass {
    /// the requested command has not been found
    CommandNotFound,
    /// the requested operation can't be fulfilled because the
    /// selected device is encrypted
    #[qapi(removed = "2.10.0")]
    DeviceEncrypted,
    /// a device has failed to be become active
    DeviceNotActive,
    /// the requested device has not been found
    DeviceNotFound,
    /// this is used for errors that don't require a specific
    /// error class.  This should be the default case for most errors
    GenericError,
    /// the requested operation can't be fulfilled because a
    /// required KVM capability is missing
    #[qapi(name = "KVMMissingCap")]
    KvmMissingCap,
}

/// A fat type wrapping 'str', to be embedded in lists.
///
#[qapi(name = "String")]
#[qapi(added = "1.2.0")]
#[qapi(removed = "11.1.0")]
pub struct QapiString {
    pub str: String,
}

/// An enumeration of the VFIO device migration states.  In addition to
/// the regular states, there are prepare states (with 'prepare' suffix)
/// which indicate that the device is just about to transition to the
/// corresponding state.  Note that seeing a prepare state for state X
/// doesn't guarantee that the next state will be X, as the state
/// transition can fail and the device may transition to a different
/// state instead.
///
#[qapi(added = "9.2.0")]
pub enum QapiVfioMigrationState {
    /// The device is running, tracking its internal state and
    /// its internal state is available for reading.
    #[qapi(name = "pre-copy")]
    PreCopy,
    /// The device is running in the P2P quiescent state,
    /// tracking its internal state and its internal state is available
    /// for reading.
    #[qapi(name = "pre-copy-p2p")]
    PreCopyP2p,
    /// The device is just about to move to
    /// pre-copy-p2p state.  (since 11.0)
    #[qapi(name = "pre-copy-p2p-prepare")]
    #[qapi(added = "11.0.0")]
    PreCopyP2pPrepare,
    /// The device is stopped and its internal state is available
    /// for writing.
    #[qapi(name = "resuming")]
    Resuming,
    /// The device is running.
    #[qapi(name = "running")]
    Running,
    /// The device is running in the P2P quiescent state.
    #[qapi(name = "running-p2p")]
    RunningP2p,
    /// The device is stopped.
    #[qapi(name = "stop")]
    Stop,
    /// The device is stopped and its internal state is
    /// available for reading.
    #[qapi(name = "stop-copy")]
    StopCopy,
}

/// The authorization policy match format
///
#[qapi(added = "4.0.0")]
pub enum QAuthZListFormat {
    /// an exact string match
    #[qapi(name = "exact")]
    Exact,
    /// string with ? and * shell wildcard support
    #[qapi(name = "glob")]
    Glob,
}

/// The authorization policy result
///
#[qapi(added = "4.0.0")]
pub enum QAuthZListPolicy {
    /// allow access
    #[qapi(name = "allow")]
    Allow,
    /// deny access
    #[qapi(name = "deny")]
    Deny,
}

/// A single authorization rule.
///
#[qapi(added = "4.0.0")]
pub struct QAuthZListRule {
    /// the format of the @match rule (default 'exact')
    pub format: Option<QAuthZListFormat>,
    /// a string or glob to match against a user identity
    pub r#match: String,
    /// the result to return if @match evaluates to true
    pub policy: QAuthZListPolicy,
}

/// Not exposed via QMP; hack to generate QAuthZListRuleList
/// for use internally by the code.
///
#[qapi(added = "4.0.0")]
#[qapi(removed = "6.0.0")]
pub struct QAuthZListRuleListHack {
    pub unused: Vec<QAuthZListRule>,
}

/// Qcow2 bitmap information.
///
#[qapi(added = "4.0.0")]
pub struct Qcow2BitmapInfo {
    /// flags of the bitmap
    pub flags: Vec<Qcow2BitmapInfoFlags>,
    /// granularity of the bitmap in bytes
    pub granularity: u32,
    /// the name of the bitmap
    pub name: String,
}

/// An enumeration of flags that a bitmap can report to the user.
///
#[qapi(added = "4.0.0")]
pub enum Qcow2BitmapInfoFlags {
    /// The bitmap must reflect all changes of the virtual disk by
    /// any application that would write to this qcow2 file.
    #[qapi(name = "auto")]
    Auto,
    /// This flag is set by any process actively modifying the
    /// qcow2 file, and cleared when the updated bitmap is flushed to
    /// the qcow2 image.  The presence of this flag in an offline image
    /// means that the bitmap was not saved correctly after its last
    /// usage, and may contain inconsistent data.
    #[qapi(name = "in-use")]
    InUse,
}

/// Compression type used in qcow2 image file
///
#[qapi(added = "5.1.0")]
pub enum Qcow2CompressionType {
    /// zlib compression, see <http://zlib.net/>
    #[qapi(name = "zlib")]
    Zlib,
    /// zstd compression, see <http://github.com/facebook/zstd>
    #[qapi(name = "zstd")]
    #[qapi(condition = "defined(CONFIG_ZSTD)")]
    Zstd,
}

/// Structure of flags for each metadata structure.  Setting a field to
/// 'true' makes QEMU guard that Qcow2 format structure against
/// unintended overwriting.  See Qcow2 format specification for detailed
/// information on these structures.  The default value is chosen
/// according to the template given.
///
#[qapi(added = "2.2.0")]
pub struct Qcow2OverlapCheckFlags {
    /// Qcow2 active L1 table
    #[qapi(name = "active-l1")]
    pub active_l1: Option<bool>,
    /// Qcow2 active L2 table
    #[qapi(name = "active-l2")]
    pub active_l2: Option<bool>,
    /// Qcow2 bitmap directory (since 3.0)
    #[qapi(name = "bitmap-directory")]
    #[qapi(added = "3.0.0")]
    pub bitmap_directory: Option<bool>,
    /// Qcow2 inactive L1 tables
    #[qapi(name = "inactive-l1")]
    pub inactive_l1: Option<bool>,
    /// Qcow2 inactive L2 tables
    #[qapi(name = "inactive-l2")]
    pub inactive_l2: Option<bool>,
    /// Qcow2 format header
    #[qapi(name = "main-header")]
    pub main_header: Option<bool>,
    /// Qcow2 refcount blocks
    #[qapi(name = "refcount-block")]
    pub refcount_block: Option<bool>,
    /// Qcow2 refcount table
    #[qapi(name = "refcount-table")]
    pub refcount_table: Option<bool>,
    /// Qcow2 snapshot table
    #[qapi(name = "snapshot-table")]
    pub snapshot_table: Option<bool>,
    /// Specifies a template mode which can be adjusted using the
    /// other flags, defaults to 'cached'
    pub template: Option<Qcow2OverlapCheckMode>,
}

/// General overlap check modes.
///
#[qapi(added = "2.2.0")]
pub enum Qcow2OverlapCheckMode {
    /// Perform all available overlap checks
    #[qapi(name = "all")]
    All,
    /// Perform only checks which can be done without reading
    /// anything from disk
    #[qapi(name = "cached")]
    Cached,
    /// Perform only checks which can be done in constant time
    /// and without reading anything from disk
    #[qapi(name = "constant")]
    Constant,
    /// Do not perform any checks
    #[qapi(name = "none")]
    None,
}

/// Specifies which metadata structures should be guarded against
/// unintended overwriting.
///
#[qapi(added = "2.4.0")]
pub enum Qcow2OverlapChecks {
    /// set of flags for separate specification of each metadata
    /// structure type
    #[qapi(name = "flags")]
    Flags(Qcow2OverlapCheckFlags),
    /// named mode which chooses a specific set of flags
    #[qapi(name = "mode")]
    Mode(Qcow2OverlapCheckMode),
}

#[qapi(added = "2.2.0")]
#[qapi(removed = "2.4.0")]
pub enum Qcow2OverlapChecksBranch {
    #[qapi(name = "flags")]
    Flags(Qcow2OverlapCheckFlags),
    #[qapi(name = "mode")]
    Mode(Qcow2OverlapCheckMode),
}

#[qapi(added = "2.2.0")]
#[qapi(removed = "2.4.0")]
pub enum Qcow2OverlapChecksType {
    #[qapi(name = "flags")]
    Flags,
    #[qapi(name = "mode")]
    Mode,
}

/// The supported algorithms for asymmetric encryption ciphers
///
#[qapi(added = "9.2.0")]
pub enum QCryptoAkCipherAlgo {
    /// RSA algorithm
    #[qapi(name = "rsa")]
    Rsa,
}

/// The supported algorithms for asymmetric encryption ciphers
///
#[qapi(added = "7.1.0")]
#[qapi(removed = "9.2.0")]
pub enum QCryptoAkCipherAlgorithm {
    /// RSA algorithm
    #[qapi(name = "rsa")]
    Rsa,
}

/// The type of asymmetric keys.
///
#[qapi(added = "7.1.0")]
pub enum QCryptoAkCipherKeyType {
    /// private key
    #[qapi(name = "private")]
    Private,
    /// public key
    #[qapi(name = "public")]
    Public,
}

/// The options that are available for all asymmetric key algorithms
/// when creating a new QCryptoAkCipher.
///
#[qapi(added = "7.1.0")]
pub struct QCryptoAkCipherOptions {
    /// encryption cipher algorithm
    #[qapi(discriminator)]
    pub alg: QCryptoAkCipherAlgo,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<QCryptoAkCipherOptionsBranch>,
}

#[qapi(added = "7.1.0")]
pub enum QCryptoAkCipherOptionsBranch {
    #[qapi(name = "rsa")]
    Rsa(QCryptoAkCipherOptionsRsa),
}

/// Specific parameters for RSA algorithm.
///
#[qapi(name = "QCryptoAkCipherOptionsRSA")]
#[qapi(added = "7.1.0")]
pub struct QCryptoAkCipherOptionsRsa {
    /// `QCryptoHashAlgo`
    #[qapi(name = "hash-alg")]
    pub hash_alg: QCryptoHashAlgo,
    /// `QCryptoRSAPaddingAlgo`
    #[qapi(name = "padding-alg")]
    pub padding_alg: QCryptoRsaPaddingAlgo,
}

/// The options that are available for all encryption formats when
/// amending encryption settings
///
#[qapi(added = "5.1.0")]
pub struct QCryptoBlockAmendOptions {
    /// the encryption format
    #[qapi(discriminator)]
    pub format: QCryptoBlockFormat,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<QCryptoBlockAmendOptionsBranch>,
}

#[qapi(added = "5.1.0")]
pub enum QCryptoBlockAmendOptionsBranch {
    #[qapi(name = "luks")]
    Luks(QCryptoBlockAmendOptionsLuks),
    /// QCow/QCow2 built-in AES-CBC encryption.  Use only for
    /// liberating data from old images.
    #[qapi(name = "qcow")]
    Qcow,
}

/// This struct defines the update parameters that activate/de-activate
/// set of keyslots
///
#[qapi(name = "QCryptoBlockAmendOptionsLUKS")]
#[qapi(added = "5.1.0")]
pub struct QCryptoBlockAmendOptionsLuks {
    /// Optional (for activation only).  Number of milliseconds
    /// to spend in PBKDF passphrase processing for the newly activated
    /// keyslot.  Currently defaults to 2000.
    #[qapi(name = "iter-time")]
    pub iter_time: Option<i64>,
    /// Optional.  ID of the keyslot to activate/deactivate.  For
    /// keyslot activation, keyslot should not be active already (this
    /// is unsafe to update an active keyslot), but possible if 'force'
    /// parameter is given.  If keyslot is not given, first free keyslot
    /// will be written.
    ///
    /// For keyslot deactivation, this parameter specifies the exact
    /// keyslot to deactivate
    pub keyslot: Option<i64>,
    /// The ID of a QCryptoSecret object providing the password
    /// to be written into added active keyslots
    #[qapi(name = "new-secret")]
    pub new_secret: Option<String>,
    /// Optional (for deactivation only).  If given will
    /// deactivate all keyslots that match password located in
    /// QCryptoSecret with this ID
    #[qapi(name = "old-secret")]
    pub old_secret: Option<String>,
    /// Optional.  The ID of a QCryptoSecret object providing the
    /// password to use to retrieve current master key.  Defaults to the
    /// same secret that was used to open the image
    pub secret: Option<String>,
    /// the desired state of the keyslots
    pub state: QCryptoBlockLuksKeyslotState,
}

/// The options that are available for all encryption formats when
/// initializing a new volume
///
#[qapi(added = "2.6.0")]
pub struct QCryptoBlockCreateOptions {
    /// the encryption format
    #[qapi(discriminator)]
    pub format: QCryptoBlockFormat,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<QCryptoBlockCreateOptionsBranch>,
}

#[qapi(added = "2.6.0")]
pub enum QCryptoBlockCreateOptionsBranch {
    #[qapi(name = "luks")]
    Luks(QCryptoBlockCreateOptionsLuks),
    #[qapi(name = "qcow")]
    Qcow(QCryptoBlockOptionsQCow),
}

/// The options that apply to LUKS encryption format initialization
///
#[qapi(name = "QCryptoBlockCreateOptionsLUKS")]
#[qapi(added = "2.6.0")]
pub struct QCryptoBlockCreateOptionsLuks {
    /// the cipher algorithm for data encryption.  Currently
    /// defaults to 'aes-256'.
    #[qapi(name = "cipher-alg")]
    pub cipher_alg: Option<QCryptoCipherAlgo>,
    /// the cipher mode for data encryption.  Currently
    /// defaults to 'xts'
    #[qapi(name = "cipher-mode")]
    pub cipher_mode: Option<QCryptoCipherMode>,
    /// create a detached LUKS header.  (since 9.0)
    #[qapi(name = "detached-header")]
    #[qapi(added = "9.0.0")]
    #[qapi(removed = "9.1.0")]
    pub detached_header: Option<bool>,
    /// the master key hash algorithm.  Currently defaults to
    /// 'sha256'
    #[qapi(name = "hash-alg")]
    pub hash_alg: Option<QCryptoHashAlgo>,
    /// number of milliseconds to spend in PBKDF passphrase
    /// processing.  Currently defaults to 2000.  (since 2.8)
    #[qapi(name = "iter-time")]
    #[qapi(added = "2.8.0")]
    pub iter_time: Option<i64>,
    /// the initialization vector generator.  Currently defaults
    /// to 'plain64'
    #[qapi(name = "ivgen-alg")]
    pub ivgen_alg: Option<QCryptoIvGenAlgo>,
    /// the initialization vector generator hash.
    /// Currently defaults to 'sha256'
    #[qapi(name = "ivgen-hash-alg")]
    pub ivgen_hash_alg: Option<QCryptoHashAlgo>,
    /// the ID of a QCryptoSecret object providing the
    /// decryption key.  Mandatory except when probing image for
    /// metadata only.
    #[qapi(name = "key-secret")]
    pub key_secret: Option<String>,
}

/// The supported full disk encryption formats
///
#[qapi(added = "2.6.0")]
pub enum QCryptoBlockFormat {
    /// LUKS encryption format.  Recommended for new images
    #[qapi(name = "luks")]
    Luks,
    /// QCow/QCow2 built-in AES-CBC encryption.  Use only for
    /// liberating data from old images.
    #[qapi(name = "qcow")]
    Qcow,
}

/// Information about the block encryption options
///
#[qapi(added = "2.7.0")]
pub struct QCryptoBlockInfo {
    /// the encryption format
    #[qapi(discriminator)]
    pub format: QCryptoBlockFormat,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<QCryptoBlockInfoBranch>,
}

/// The common information that applies to all full disk encryption
/// formats
///
#[qapi(added = "2.7.0")]
pub struct QCryptoBlockInfoBase {
    /// the encryption format
    pub format: QCryptoBlockFormat,
}

#[qapi(added = "2.7.0")]
pub enum QCryptoBlockInfoBranch {
    #[qapi(name = "luks")]
    Luks(QCryptoBlockInfoLuks),
    /// QCow/QCow2 built-in AES-CBC encryption.  Use only for
    /// liberating data from old images.
    #[qapi(name = "qcow")]
    Qcow,
}

/// Information about the LUKS block encryption options
///
#[qapi(name = "QCryptoBlockInfoLUKS")]
#[qapi(added = "2.7.0")]
pub struct QCryptoBlockInfoLuks {
    /// the cipher algorithm for data encryption
    #[qapi(name = "cipher-alg")]
    pub cipher_alg: QCryptoCipherAlgo,
    /// the cipher mode for data encryption
    #[qapi(name = "cipher-mode")]
    pub cipher_mode: QCryptoCipherMode,
    /// whether the LUKS header is detached (Since 9.0)
    #[qapi(name = "detached-header")]
    #[qapi(added = "9.0.0")]
    pub detached_header: Option<bool>,
    /// the master key hash algorithm
    #[qapi(name = "hash-alg")]
    pub hash_alg: QCryptoHashAlgo,
    /// the initialization vector generator
    #[qapi(name = "ivgen-alg")]
    pub ivgen_alg: QCryptoIvGenAlgo,
    /// the initialization vector generator hash
    #[qapi(name = "ivgen-hash-alg")]
    pub ivgen_hash_alg: Option<QCryptoHashAlgo>,
    /// number of PBKDF2 iterations for key material
    #[qapi(name = "master-key-iters")]
    pub master_key_iters: i64,
    /// offset to the payload data in bytes
    #[qapi(name = "payload-offset")]
    pub payload_offset: i64,
    /// information about each key slot
    pub slots: Vec<QCryptoBlockInfoLuksSlot>,
    /// unique identifier for the volume
    pub uuid: String,
}

/// Information about the LUKS block encryption key slot options
///
#[qapi(name = "QCryptoBlockInfoLUKSSlot")]
#[qapi(added = "2.7.0")]
pub struct QCryptoBlockInfoLuksSlot {
    /// whether the key slot is currently in use
    pub active: bool,
    /// number of PBKDF2 iterations for key material
    pub iters: Option<i64>,
    /// offset to the key material in bytes
    #[qapi(name = "key-offset")]
    pub key_offset: i64,
    /// number of stripes for splitting key material
    pub stripes: Option<i64>,
}

/// Information about the QCow block encryption options
///
#[qapi(added = "2.7.0")]
#[qapi(removed = "3.0.0")]
pub struct QCryptoBlockInfoQCow {}

/// Defines state of keyslots that are affected by the update
///
#[qapi(name = "QCryptoBlockLUKSKeyslotState")]
#[qapi(added = "5.1.0")]
pub enum QCryptoBlockLuksKeyslotState {
    /// The slots contain the given password and marked as active
    #[qapi(name = "active")]
    Active,
    /// The slots are erased (contain garbage) and marked as
    /// inactive
    #[qapi(name = "inactive")]
    Inactive,
}

/// The options that are available for all encryption formats when
/// opening an existing volume
///
#[qapi(added = "2.6.0")]
pub struct QCryptoBlockOpenOptions {
    /// the encryption format
    #[qapi(discriminator)]
    pub format: QCryptoBlockFormat,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<QCryptoBlockOpenOptionsBranch>,
}

#[qapi(added = "2.6.0")]
pub enum QCryptoBlockOpenOptionsBranch {
    #[qapi(name = "luks")]
    Luks(QCryptoBlockOptionsLuks),
    #[qapi(name = "qcow")]
    Qcow(QCryptoBlockOptionsQCow),
}

/// The common options that apply to all full disk encryption formats
///
#[qapi(added = "2.6.0")]
pub struct QCryptoBlockOptionsBase {
    /// the encryption format
    pub format: QCryptoBlockFormat,
}

/// The options that apply to LUKS encryption format
///
#[qapi(name = "QCryptoBlockOptionsLUKS")]
#[qapi(added = "2.6.0")]
pub struct QCryptoBlockOptionsLuks {
    /// the ID of a QCryptoSecret object providing the
    /// decryption key.  Mandatory except when probing image for
    /// metadata only.
    #[qapi(name = "key-secret")]
    pub key_secret: Option<String>,
}

/// The options that apply to QCow/QCow2 AES-CBC encryption format
///
#[qapi(added = "2.6.0")]
pub struct QCryptoBlockOptionsQCow {
    /// the ID of a QCryptoSecret object providing the
    /// decryption key.  Mandatory except when probing image for
    /// metadata only.
    #[qapi(name = "key-secret")]
    pub key_secret: Option<String>,
}

/// The supported algorithms for content encryption ciphers
///
#[qapi(added = "9.2.0")]
pub enum QCryptoCipherAlgo {
    /// 3DES(EDE) with 192 bit / 24 byte keys (since 2.9)
    #[qapi(name = "3des")]
    _3des,
    /// AES with 128 bit / 16 byte keys
    #[qapi(name = "aes-128")]
    Aes128,
    /// AES with 192 bit / 24 byte keys
    #[qapi(name = "aes-192")]
    Aes192,
    /// AES with 256 bit / 32 byte keys
    #[qapi(name = "aes-256")]
    Aes256,
    /// Cast5 with 128 bit / 16 byte keys
    #[qapi(name = "cast5-128")]
    Cast5128,
    /// DES with 56 bit / 8 byte keys.  Do not use except in VNC.
    /// (since 6.1)
    #[qapi(name = "des")]
    Des,
    /// Serpent with 128 bit / 16 byte keys
    #[qapi(name = "serpent-128")]
    Serpent128,
    /// Serpent with 192 bit / 24 byte keys
    #[qapi(name = "serpent-192")]
    Serpent192,
    /// Serpent with 256 bit / 32 byte keys
    #[qapi(name = "serpent-256")]
    Serpent256,
    /// SM4 with 128 bit / 16 byte keys (since 9.0)
    #[qapi(name = "sm4")]
    Sm4,
    /// Twofish with 128 bit / 16 byte keys
    #[qapi(name = "twofish-128")]
    Twofish128,
    /// Twofish with 192 bit / 24 byte keys
    #[qapi(name = "twofish-192")]
    Twofish192,
    /// Twofish with 256 bit / 32 byte keys
    #[qapi(name = "twofish-256")]
    Twofish256,
}

/// The supported algorithms for content encryption ciphers
///
#[qapi(added = "2.6.0")]
#[qapi(removed = "9.2.0")]
pub enum QCryptoCipherAlgorithm {
    /// 3DES(EDE) with 192 bit / 24 byte keys (since 2.9)
    #[qapi(name = "3des")]
    #[qapi(added = "2.9.0")]
    _3des,
    /// AES with 128 bit / 16 byte keys
    #[qapi(name = "aes-128")]
    Aes128,
    /// AES with 192 bit / 24 byte keys
    #[qapi(name = "aes-192")]
    Aes192,
    /// AES with 256 bit / 32 byte keys
    #[qapi(name = "aes-256")]
    Aes256,
    /// Cast5 with 128 bit / 16 byte keys
    #[qapi(name = "cast5-128")]
    Cast5128,
    /// DES with 56 bit / 8 byte keys.  Do not use except in VNC.
    /// (since 6.1)
    #[qapi(name = "des")]
    #[qapi(added = "6.1.0")]
    Des,
    /// RFB specific variant of single DES. Do not use except in VNC.
    #[qapi(name = "des-rfb")]
    #[qapi(removed = "6.1.0")]
    DesRfb,
    /// Serpent with 128 bit / 16 byte keys
    #[qapi(name = "serpent-128")]
    Serpent128,
    /// Serpent with 192 bit / 24 byte keys
    #[qapi(name = "serpent-192")]
    Serpent192,
    /// Serpent with 256 bit / 32 byte keys
    #[qapi(name = "serpent-256")]
    Serpent256,
    /// SM4 with 128 bit / 16 byte keys (since 9.0)
    #[qapi(name = "sm4")]
    #[qapi(added = "9.0.0")]
    Sm4,
    /// Twofish with 128 bit / 16 byte keys
    #[qapi(name = "twofish-128")]
    Twofish128,
    /// Twofish with 192 bit / 24 byte keys
    #[qapi(name = "twofish-192")]
    Twofish192,
    /// Twofish with 256 bit / 32 byte keys
    #[qapi(name = "twofish-256")]
    Twofish256,
}

/// The supported modes for content encryption ciphers
///
#[qapi(added = "2.6.0")]
pub enum QCryptoCipherMode {
    /// Cipher Block Chaining
    #[qapi(name = "cbc")]
    Cbc,
    /// Counter (Since 2.8)
    #[qapi(name = "ctr")]
    #[qapi(added = "2.8.0")]
    Ctr,
    /// Electronic Code Book
    #[qapi(name = "ecb")]
    Ecb,
    /// XEX with tweaked code book and ciphertext stealing
    #[qapi(name = "xts")]
    Xts,
}

/// The supported algorithm types of a crypto device.
///
#[qapi(added = "9.2.0")]
pub enum QCryptodevBackendAlgoType {
    /// asymmetric encryption
    #[qapi(name = "asym")]
    Asym,
    /// symmetric encryption
    #[qapi(name = "sym")]
    Sym,
}

/// The supported algorithm types of a crypto device.
///
#[qapi(added = "8.0.0")]
#[qapi(removed = "9.2.0")]
pub enum QCryptodevBackendAlgType {
    /// asymmetric Encryption
    #[qapi(name = "asym")]
    Asym,
    /// symmetric encryption
    #[qapi(name = "sym")]
    Sym,
}

/// Information about a queue of crypto device.
///
#[qapi(added = "8.0.0")]
pub struct QCryptodevBackendClient {
    /// the queue index of the crypto device
    pub queue: u32,
    /// the type of the crypto device
    pub r#type: QCryptodevBackendType,
}

/// The supported service types of a crypto device.
///
#[qapi(added = "8.0.0")]
pub enum QCryptodevBackendServiceType {
    /// Authenticated Encryption with Associated Data service
    #[qapi(name = "aead")]
    Aead,
    /// Asymmetric Key Cipher service
    #[qapi(name = "akcipher")]
    Akcipher,
    /// Symmetric Key Cipher service
    #[qapi(name = "cipher")]
    Cipher,
    /// Hash service
    #[qapi(name = "hash")]
    Hash,
    /// Message Authentication Codes service
    #[qapi(name = "mac")]
    Mac,
}

/// The crypto device backend type
///
#[qapi(added = "8.0.0")]
pub enum QCryptodevBackendType {
    /// the QEMU builtin support
    #[qapi(name = "builtin")]
    Builtin,
    /// Linux kernel cryptographic framework
    #[qapi(name = "lkcf")]
    Lkcf,
    /// vhost-user
    #[qapi(name = "vhost-user")]
    VhostUser,
}

/// Information about a crypto device.
///
#[qapi(added = "8.0.0")]
pub struct QCryptodevInfo {
    /// the additional information of the crypto device
    pub client: Vec<QCryptodevBackendClient>,
    /// the id of the crypto device
    pub id: String,
    /// supported service types of a crypto device
    pub service: Vec<QCryptodevBackendServiceType>,
}

/// The supported algorithms for computing content digests
///
#[qapi(added = "9.2.0")]
pub enum QCryptoHashAlgo {
    /// MD5.  Should not be used in any new code, legacy compat only
    #[qapi(name = "md5")]
    Md5,
    /// RIPEMD-160.  (since 2.7)
    #[qapi(name = "ripemd160")]
    Ripemd160,
    /// SHA-1.  Should not be used in any new code, legacy compat
    /// only
    #[qapi(name = "sha1")]
    Sha1,
    /// SHA-224.  (since 2.7)
    #[qapi(name = "sha224")]
    Sha224,
    /// SHA-256.  Current recommended strong hash.
    #[qapi(name = "sha256")]
    Sha256,
    /// SHA-384.  (since 2.7)
    #[qapi(name = "sha384")]
    Sha384,
    /// SHA-512.  (since 2.7)
    #[qapi(name = "sha512")]
    Sha512,
    /// SM3.  (since 9.2.0)
    #[qapi(name = "sm3")]
    Sm3,
}

/// The supported algorithms for computing content digests
///
#[qapi(added = "2.6.0")]
#[qapi(removed = "9.2.0")]
pub enum QCryptoHashAlgorithm {
    /// MD5.  Should not be used in any new code, legacy compat only
    #[qapi(name = "md5")]
    Md5,
    /// RIPEMD-160.  (since 2.7)
    #[qapi(name = "ripemd160")]
    #[qapi(added = "2.7.0")]
    Ripemd160,
    /// SHA-1.  Should not be used in any new code, legacy compat only
    #[qapi(name = "sha1")]
    Sha1,
    /// SHA-224.  (since 2.7)
    #[qapi(name = "sha224")]
    #[qapi(added = "2.7.0")]
    Sha224,
    /// SHA-256.  Current recommended strong hash.
    #[qapi(name = "sha256")]
    Sha256,
    /// SHA-384.  (since 2.7)
    #[qapi(name = "sha384")]
    #[qapi(added = "2.7.0")]
    Sha384,
    /// SHA-512.  (since 2.7)
    #[qapi(name = "sha512")]
    #[qapi(added = "2.7.0")]
    Sha512,
}

/// The supported algorithms for generating initialization vectors for
/// full disk encryption.  The 'plain' generator should not be used for
/// disks with sector numbers larger than 2^32, except where
/// compatibility with pre-existing Linux dm-crypt volumes is required.
///
#[qapi(name = "QCryptoIVGenAlgo")]
#[qapi(added = "9.2.0")]
pub enum QCryptoIvGenAlgo {
    /// 64-bit sector number encrypted with a hash of the encryption
    /// key
    #[qapi(name = "essiv")]
    Essiv,
    /// 64-bit sector number truncated to 32-bits
    #[qapi(name = "plain")]
    Plain,
    /// 64-bit sector number
    #[qapi(name = "plain64")]
    Plain64,
}

/// The supported algorithms for generating initialization vectors for
/// full disk encryption.  The 'plain' generator should not be used for
/// disks with sector numbers larger than 2^32, except where
/// compatibility with pre-existing Linux dm-crypt volumes is required.
///
#[qapi(name = "QCryptoIVGenAlgorithm")]
#[qapi(added = "2.6.0")]
#[qapi(removed = "9.2.0")]
pub enum QCryptoIvGenAlgorithm {
    /// 64-bit sector number encrypted with a hash of the encryption
    /// key
    #[qapi(name = "essiv")]
    Essiv,
    /// 64-bit sector number truncated to 32-bits
    #[qapi(name = "plain")]
    Plain,
    /// 64-bit sector number
    #[qapi(name = "plain64")]
    Plain64,
}

/// The padding algorithm for RSA.
///
#[qapi(name = "QCryptoRSAPaddingAlgo")]
#[qapi(added = "9.2.0")]
pub enum QCryptoRsaPaddingAlgo {
    /// pkcs1#v1.5
    #[qapi(name = "pkcs1")]
    Pkcs1,
    /// no padding used
    #[qapi(name = "raw")]
    Raw,
}

/// The padding algorithm for RSA.
///
#[qapi(name = "QCryptoRSAPaddingAlgorithm")]
#[qapi(added = "7.1.0")]
#[qapi(removed = "9.2.0")]
pub enum QCryptoRsaPaddingAlgorithm {
    /// pkcs1#v1.5
    #[qapi(name = "pkcs1")]
    Pkcs1,
    /// no padding used
    #[qapi(name = "raw")]
    Raw,
}

/// The data format that the secret is provided in
///
#[qapi(added = "2.6.0")]
pub enum QCryptoSecretFormat {
    /// arbitrary base64 encoded binary data
    #[qapi(name = "base64")]
    Base64,
    /// raw bytes.  When encoded in JSON only valid UTF-8 sequences
    /// can be used
    #[qapi(name = "raw")]
    Raw,
}

/// The type of network endpoint that will be using the credentials.
/// Most types of credential require different setup / structures
/// depending on whether they will be used in a server versus a client.
///
#[qapi(name = "QCryptoTLSCredsEndpoint")]
#[qapi(added = "2.5.0")]
pub enum QCryptoTlsCredsEndpoint {
    /// the network endpoint is acting as the client
    #[qapi(name = "client")]
    Client,
    /// the network endpoint is acting as the server
    #[qapi(name = "server")]
    Server,
}

/// Information on the target configuration built into the QEMU binary.
///
#[qapi(added = "10.1.0")]
pub struct QemuTargetInfo {
    /// the target architecture
    pub arch: SysEmuTarget,
}

/// An enumeration of key name.
///
/// This is used by the `send-key` command.
///
#[qapi(added = "1.3.0")]
pub enum QKeyCode {
    #[qapi(name = "0")]
    _0,
    #[qapi(name = "1")]
    _1,
    #[qapi(name = "2")]
    _2,
    #[qapi(name = "3")]
    _3,
    #[qapi(name = "4")]
    _4,
    #[qapi(name = "5")]
    _5,
    #[qapi(name = "6")]
    _6,
    #[qapi(name = "7")]
    _7,
    #[qapi(name = "8")]
    _8,
    #[qapi(name = "9")]
    _9,
    #[qapi(name = "a")]
    A,
    /// since 2.10
    #[qapi(name = "ac_back")]
    #[qapi(added = "2.10.0")]
    AcBack,
    /// since 2.10
    #[qapi(name = "ac_bookmarks")]
    #[qapi(added = "2.10.0")]
    AcBookmarks,
    /// since 2.10
    #[qapi(name = "ac_forward")]
    #[qapi(added = "2.10.0")]
    AcForward,
    /// since 2.10
    #[qapi(name = "ac_home")]
    #[qapi(added = "2.10.0")]
    AcHome,
    /// since 2.10
    #[qapi(name = "ac_refresh")]
    #[qapi(added = "2.10.0")]
    AcRefresh,
    #[qapi(name = "again")]
    Again,
    #[qapi(name = "alt")]
    Alt,
    #[qapi(name = "alt_r")]
    AltR,
    #[qapi(name = "altgr")]
    #[qapi(removed = "2.10.0")]
    Altgr,
    #[qapi(name = "altgr_r")]
    #[qapi(removed = "2.10.0")]
    AltgrR,
    #[qapi(name = "apostrophe")]
    Apostrophe,
    #[qapi(name = "asterisk")]
    Asterisk,
    /// since 2.10
    #[qapi(name = "audiomute")]
    #[qapi(added = "2.10.0")]
    Audiomute,
    /// since 2.10
    #[qapi(name = "audionext")]
    #[qapi(added = "2.10.0")]
    Audionext,
    /// since 2.10
    #[qapi(name = "audioplay")]
    #[qapi(added = "2.10.0")]
    Audioplay,
    /// since 2.10
    #[qapi(name = "audioprev")]
    #[qapi(added = "2.10.0")]
    Audioprev,
    /// since 2.10
    #[qapi(name = "audiostop")]
    #[qapi(added = "2.10.0")]
    Audiostop,
    #[qapi(name = "b")]
    B,
    #[qapi(name = "backslash")]
    Backslash,
    #[qapi(name = "backspace")]
    Backspace,
    #[qapi(name = "bracket_left")]
    BracketLeft,
    #[qapi(name = "bracket_right")]
    BracketRight,
    #[qapi(name = "c")]
    C,
    /// since 2.10
    #[qapi(name = "calculator")]
    #[qapi(added = "2.10.0")]
    Calculator,
    #[qapi(name = "caps_lock")]
    CapsLock,
    #[qapi(name = "comma")]
    Comma,
    #[qapi(name = "compose")]
    Compose,
    /// since 2.10
    #[qapi(name = "computer")]
    #[qapi(added = "2.10.0")]
    Computer,
    #[qapi(name = "copy")]
    Copy,
    #[qapi(name = "ctrl")]
    Ctrl,
    #[qapi(name = "ctrl_r")]
    CtrlR,
    #[qapi(name = "cut")]
    Cut,
    #[qapi(name = "d")]
    D,
    #[qapi(name = "delete")]
    Delete,
    #[qapi(name = "dot")]
    Dot,
    #[qapi(name = "down")]
    Down,
    #[qapi(name = "e")]
    E,
    #[qapi(name = "end")]
    End,
    #[qapi(name = "equal")]
    Equal,
    #[qapi(name = "esc")]
    Esc,
    #[qapi(name = "f")]
    F,
    #[qapi(name = "f1")]
    F1,
    #[qapi(name = "f10")]
    F10,
    #[qapi(name = "f11")]
    F11,
    #[qapi(name = "f12")]
    F12,
    /// since 8.0
    #[qapi(name = "f13")]
    #[qapi(added = "8.0.0")]
    F13,
    /// since 8.0
    #[qapi(name = "f14")]
    #[qapi(added = "8.0.0")]
    F14,
    /// since 8.0
    #[qapi(name = "f15")]
    #[qapi(added = "8.0.0")]
    F15,
    /// since 8.0
    #[qapi(name = "f16")]
    #[qapi(added = "8.0.0")]
    F16,
    /// since 8.0
    #[qapi(name = "f17")]
    #[qapi(added = "8.0.0")]
    F17,
    /// since 8.0
    #[qapi(name = "f18")]
    #[qapi(added = "8.0.0")]
    F18,
    /// since 8.0
    #[qapi(name = "f19")]
    #[qapi(added = "8.0.0")]
    F19,
    #[qapi(name = "f2")]
    F2,
    /// since 8.0
    #[qapi(name = "f20")]
    #[qapi(added = "8.0.0")]
    F20,
    /// since 8.0
    #[qapi(name = "f21")]
    #[qapi(added = "8.0.0")]
    F21,
    /// since 8.0
    #[qapi(name = "f22")]
    #[qapi(added = "8.0.0")]
    F22,
    /// since 8.0
    #[qapi(name = "f23")]
    #[qapi(added = "8.0.0")]
    F23,
    /// since 8.0
    ///
    /// 'sysrq' was mistakenly added to hack around the fact that the ps2
    /// driver was not generating correct scancodes sequences when
    /// 'alt+print' was pressed.  This flaw is now fixed and the 'sysrq' key
    /// serves no further purpose.  Any further use of 'sysrq' will be
    /// transparently changed to 'print', so they are effectively synonyms.
    #[qapi(name = "f24")]
    #[qapi(added = "8.0.0")]
    F24,
    #[qapi(name = "f3")]
    F3,
    #[qapi(name = "f4")]
    F4,
    #[qapi(name = "f5")]
    F5,
    #[qapi(name = "f6")]
    F6,
    #[qapi(name = "f7")]
    F7,
    #[qapi(name = "f8")]
    F8,
    #[qapi(name = "f9")]
    F9,
    #[qapi(name = "find")]
    Find,
    #[qapi(name = "front")]
    Front,
    #[qapi(name = "g")]
    G,
    #[qapi(name = "grave_accent")]
    GraveAccent,
    #[qapi(name = "h")]
    H,
    #[qapi(name = "help")]
    Help,
    /// since 2.9
    #[qapi(name = "henkan")]
    #[qapi(added = "2.9.0")]
    Henkan,
    /// since 2.9
    #[qapi(name = "hiragana")]
    #[qapi(added = "2.9.0")]
    Hiragana,
    #[qapi(name = "home")]
    Home,
    #[qapi(name = "i")]
    I,
    #[qapi(name = "insert")]
    Insert,
    #[qapi(name = "j")]
    J,
    #[qapi(name = "k")]
    K,
    /// since 2.12
    #[qapi(name = "katakanahiragana")]
    #[qapi(added = "2.12.0")]
    Katakanahiragana,
    #[qapi(name = "kp_0")]
    Kp0,
    #[qapi(name = "kp_1")]
    Kp1,
    #[qapi(name = "kp_2")]
    Kp2,
    #[qapi(name = "kp_3")]
    Kp3,
    #[qapi(name = "kp_4")]
    Kp4,
    #[qapi(name = "kp_5")]
    Kp5,
    #[qapi(name = "kp_6")]
    Kp6,
    #[qapi(name = "kp_7")]
    Kp7,
    #[qapi(name = "kp_8")]
    Kp8,
    #[qapi(name = "kp_9")]
    Kp9,
    #[qapi(name = "kp_add")]
    KpAdd,
    /// since 2.4
    #[qapi(name = "kp_comma")]
    #[qapi(added = "2.3.1")]
    KpComma,
    #[qapi(name = "kp_decimal")]
    KpDecimal,
    #[qapi(name = "kp_divide")]
    KpDivide,
    #[qapi(name = "kp_enter")]
    KpEnter,
    /// since 2.6
    #[qapi(name = "kp_equals")]
    #[qapi(added = "2.6.0")]
    KpEquals,
    #[qapi(name = "kp_multiply")]
    KpMultiply,
    #[qapi(name = "kp_subtract")]
    KpSubtract,
    #[qapi(name = "l")]
    L,
    /// since 6.1
    #[qapi(name = "lang1")]
    #[qapi(added = "6.1.0")]
    Lang1,
    /// since 6.1
    #[qapi(name = "lang2")]
    #[qapi(added = "6.1.0")]
    Lang2,
    #[qapi(name = "left")]
    Left,
    #[qapi(name = "less")]
    Less,
    #[qapi(name = "lf")]
    Lf,
    #[qapi(name = "m")]
    M,
    /// since 2.10
    #[qapi(name = "mail")]
    #[qapi(added = "2.10.0")]
    Mail,
    /// since 2.10
    #[qapi(name = "mediaselect")]
    #[qapi(added = "2.10.0")]
    Mediaselect,
    #[qapi(name = "menu")]
    Menu,
    #[qapi(name = "meta_l")]
    MetaL,
    #[qapi(name = "meta_r")]
    MetaR,
    #[qapi(name = "minus")]
    Minus,
    /// since 2.12
    #[qapi(name = "muhenkan")]
    #[qapi(added = "2.12.0")]
    Muhenkan,
    #[qapi(name = "n")]
    N,
    #[qapi(name = "num_lock")]
    NumLock,
    #[qapi(name = "o")]
    O,
    #[qapi(name = "open")]
    Open,
    #[qapi(name = "p")]
    P,
    #[qapi(name = "paste")]
    Paste,
    /// since 2.0
    #[qapi(name = "pause")]
    #[qapi(added = "2.0.0")]
    Pause,
    #[qapi(name = "pgdn")]
    Pgdn,
    #[qapi(name = "pgup")]
    Pgup,
    /// since 2.6
    #[qapi(name = "power")]
    #[qapi(added = "2.6.0")]
    Power,
    #[qapi(name = "print")]
    Print,
    #[qapi(name = "props")]
    Props,
    #[qapi(name = "q")]
    Q,
    #[qapi(name = "r")]
    R,
    #[qapi(name = "ret")]
    Ret,
    #[qapi(name = "right")]
    Right,
    /// since 2.4
    #[qapi(name = "ro")]
    #[qapi(added = "2.3.1")]
    Ro,
    #[qapi(name = "s")]
    S,
    #[qapi(name = "scroll_lock")]
    ScrollLock,
    #[qapi(name = "semicolon")]
    Semicolon,
    #[qapi(name = "shift")]
    Shift,
    #[qapi(name = "shift_r")]
    ShiftR,
    #[qapi(name = "slash")]
    Slash,
    /// since 2.10
    #[qapi(name = "sleep")]
    #[qapi(added = "2.10.0")]
    Sleep,
    #[qapi(name = "spc")]
    Spc,
    #[qapi(name = "stop")]
    Stop,
    #[qapi(name = "sysrq")]
    Sysrq,
    #[qapi(name = "t")]
    T,
    #[qapi(name = "tab")]
    Tab,
    #[qapi(name = "u")]
    U,
    #[qapi(name = "undo")]
    Undo,
    /// since 2.0
    #[qapi(name = "unmapped")]
    #[qapi(added = "2.0.0")]
    Unmapped,
    #[qapi(name = "up")]
    Up,
    #[qapi(name = "v")]
    V,
    /// since 2.10
    #[qapi(name = "volumedown")]
    #[qapi(added = "2.10.0")]
    Volumedown,
    /// since 2.10
    #[qapi(name = "volumeup")]
    #[qapi(added = "2.10.0")]
    Volumeup,
    #[qapi(name = "w")]
    W,
    /// since 2.10
    #[qapi(name = "wake")]
    #[qapi(added = "2.10.0")]
    Wake,
    #[qapi(name = "x")]
    X,
    #[qapi(name = "y")]
    Y,
    /// since 2.9
    #[qapi(name = "yen")]
    #[qapi(added = "2.9.0")]
    Yen,
    #[qapi(name = "z")]
    Z,
}

#[qapi(added = "6.2.0")]
pub struct QKeyCodeWrapper {
    /// An enumeration of key name
    pub data: QKeyCode,
}

/// Enumeration of capabilities to be advertised during initial client
/// connection, used for agreeing on particular QMP extension behaviors.
///
#[qapi(name = "QMPCapability")]
#[qapi(added = "2.12.0")]
pub enum QmpCapability {
    /// QMP ability to support out-of-band requests.  (Please refer to
    /// qmp-spec.rst for more information on OOB)
    #[qapi(name = "oob")]
    Oob,
}

/// Properties for qtest objects.
///
#[qapi(added = "6.1.0")]
pub struct QtestProperties {
    /// the chardev to be used to receive qtest commands on.
    pub chardev: String,
    /// the path to a log file
    pub log: Option<String>,
}

/// An enumeration of the quorum operation types
///
#[qapi(added = "2.6.0")]
pub enum QuorumOpType {
    /// flush operation
    #[qapi(name = "flush")]
    Flush,
    /// read operation
    #[qapi(name = "read")]
    Read,
    /// write operation
    #[qapi(name = "write")]
    Write,
}

/// An enumeration of quorum read patterns.
///
#[qapi(added = "2.2.0")]
pub enum QuorumReadPattern {
    /// read only from the first child that has not failed
    #[qapi(name = "fifo")]
    Fifo,
    /// read all the children and do a quorum vote on reads
    #[qapi(name = "quorum")]
    Quorum,
}

#[qapi(added = "3.0.0")]
pub enum RbdAuthMode {
    #[qapi(name = "cephx")]
    Cephx,
    #[qapi(name = "none")]
    None,
}

#[qapi(added = "6.1.0")]
pub struct RbdEncryptionCreateOptions {
    /// Encryption format.
    #[qapi(discriminator)]
    pub format: RbdImageEncryptionFormat,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<RbdEncryptionCreateOptionsBranch>,
}

#[qapi(added = "6.1.0")]
pub enum RbdEncryptionCreateOptionsBranch {
    #[qapi(name = "luks")]
    Luks(RbdEncryptionCreateOptionsLuks),
    /// Used for opening either luks or luks2 (Since 8.0)
    #[qapi(name = "luks-any")]
    #[qapi(added = "8.0.0")]
    LuksAny,
    #[qapi(name = "luks2")]
    Luks2(RbdEncryptionCreateOptionsLuks2),
}

#[qapi(name = "RbdEncryptionCreateOptionsLUKS")]
#[qapi(added = "6.1.0")]
pub struct RbdEncryptionCreateOptionsLuks {
    /// The encryption algorithm
    #[qapi(name = "cipher-alg")]
    pub cipher_alg: Option<QCryptoCipherAlgo>,
    /// ID of a QCryptoSecret object providing a passphrase for
    /// unlocking the encryption
    #[qapi(name = "key-secret")]
    pub key_secret: String,
}

#[qapi(name = "RbdEncryptionCreateOptionsLUKS2")]
#[qapi(added = "6.1.0")]
pub struct RbdEncryptionCreateOptionsLuks2 {
    /// The encryption algorithm
    #[qapi(name = "cipher-alg")]
    pub cipher_alg: Option<QCryptoCipherAlgo>,
    /// ID of a QCryptoSecret object providing a passphrase for
    /// unlocking the encryption
    #[qapi(name = "key-secret")]
    pub key_secret: String,
}

#[qapi(name = "RbdEncryptionCreateOptionsLUKSBase")]
#[qapi(added = "6.1.0")]
pub struct RbdEncryptionCreateOptionsLuksBase {
    /// The encryption algorithm
    #[qapi(name = "cipher-alg")]
    pub cipher_alg: Option<QCryptoCipherAlgo>,
    /// ID of a QCryptoSecret object providing a passphrase for
    /// unlocking the encryption
    #[qapi(name = "key-secret")]
    pub key_secret: String,
}

#[qapi(added = "6.1.0")]
pub struct RbdEncryptionOptions {
    /// Encryption format.
    #[qapi(discriminator)]
    pub format: RbdImageEncryptionFormat,
    /// Parent image encryption options (for cloned images).  Can
    /// be left unspecified if this cloned image is encrypted using the
    /// same format and secret as its parent image (i.e. not explicitly
    /// formatted) or if its parent image is not encrypted.  (Since 8.0)
    #[qapi(added = "8.0.0")]
    pub parent: Option<Box<RbdEncryptionOptions>>,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<RbdEncryptionOptionsBranch>,
}

#[qapi(added = "6.1.0")]
pub enum RbdEncryptionOptionsBranch {
    #[qapi(name = "luks")]
    Luks(RbdEncryptionOptionsLuks),
    #[qapi(name = "luks-any")]
    #[qapi(added = "8.0.0")]
    LuksAny(RbdEncryptionOptionsLuksAny),
    #[qapi(name = "luks2")]
    Luks2(RbdEncryptionOptionsLuks2),
}

#[qapi(name = "RbdEncryptionOptionsLUKS")]
#[qapi(added = "6.1.0")]
pub struct RbdEncryptionOptionsLuks {
    /// ID of a QCryptoSecret object providing a passphrase for
    /// unlocking the encryption
    #[qapi(name = "key-secret")]
    pub key_secret: String,
}

#[qapi(name = "RbdEncryptionOptionsLUKS2")]
#[qapi(added = "6.1.0")]
pub struct RbdEncryptionOptionsLuks2 {
    /// ID of a QCryptoSecret object providing a passphrase for
    /// unlocking the encryption
    #[qapi(name = "key-secret")]
    pub key_secret: String,
}

#[qapi(name = "RbdEncryptionOptionsLUKSAny")]
#[qapi(added = "8.0.0")]
pub struct RbdEncryptionOptionsLuksAny {
    /// ID of a QCryptoSecret object providing a passphrase for
    /// unlocking the encryption
    #[qapi(name = "key-secret")]
    pub key_secret: String,
}

#[qapi(name = "RbdEncryptionOptionsLUKSBase")]
#[qapi(added = "6.1.0")]
pub struct RbdEncryptionOptionsLuksBase {
    /// ID of a QCryptoSecret object providing a passphrase for
    /// unlocking the encryption
    #[qapi(name = "key-secret")]
    pub key_secret: String,
}

#[qapi(added = "6.1.0")]
pub enum RbdImageEncryptionFormat {
    #[qapi(name = "luks")]
    Luks,
    /// Used for opening either luks or luks2 (Since 8.0)
    #[qapi(name = "luks-any")]
    #[qapi(added = "8.0.0")]
    LuksAny,
    #[qapi(name = "luks2")]
    Luks2,
}

/// Possible QEMU actions upon guest reboot
///
#[qapi(added = "6.0.0")]
pub enum RebootAction {
    /// Reset the VM
    #[qapi(name = "reset")]
    Reset,
    /// Shutdown the VM and exit, according to the shutdown
    /// action
    #[qapi(name = "shutdown")]
    Shutdown,
}

/// Properties for x-remote-object objects.
///
#[qapi(added = "6.0.0")]
pub struct RemoteObjectProperties {
    /// the id of the device to be associated with the file
    /// descriptor
    pub devid: String,
    /// file descriptor name previously passed via `getfd` command
    pub fd: String,
}

/// Record/replay information.
///
#[qapi(added = "5.2.0")]
pub struct ReplayInfo {
    /// name of the record/replay log file.  It is present only
    /// in record or replay modes, when the log is recorded or replayed.
    pub filename: Option<String>,
    /// current number of executed instructions.
    pub icount: i64,
    /// current mode.
    pub mode: ReplayMode,
}

/// Mode of the replay subsystem.
///
#[qapi(added = "2.5.0")]
pub enum ReplayMode {
    /// normal execution mode.  Replay or record are not enabled.
    #[qapi(name = "none")]
    None,
    /// replay mode.  Non-deterministic data required for system
    /// execution is read from the log.
    #[qapi(name = "play")]
    Play,
    /// record mode.  All non-deterministic data is written into
    /// the replay log.
    #[qapi(name = "record")]
    Record,
}

/// An enumeration of replication modes.
///
#[qapi(condition = "defined(CONFIG_REPLICATION)")]
#[qapi(added = "2.8.0")]
pub enum ReplicationMode {
    /// Primary mode, the vm's state will be sent to secondary
    /// QEMU.
    #[qapi(name = "primary")]
    Primary,
    /// Secondary mode, receive the vm's state from primary
    /// QEMU.
    #[qapi(name = "secondary")]
    Secondary,
}

/// The result format for `query-xen-replication-status`.
///
#[qapi(condition = "defined(CONFIG_REPLICATION)")]
#[qapi(added = "2.9.0")]
pub struct ReplicationStatus {
    /// the human readable error description string, when @error is
    /// 'true'.
    pub desc: Option<String>,
    /// true if an error happened, false if replication is normal.
    pub error: bool,
}

/// Properties for rng-egd objects.
///
#[qapi(added = "6.0.0")]
pub struct RngEgdProperties {
    /// the name of a character device backend that provides the
    /// connection to the RNG daemon
    pub chardev: String,
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "6.1.0"))]
    pub opened: Option<bool>,
}

#[qapi(added = "6.0.0")]
pub struct RngProperties {
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "6.1.0"))]
    pub opened: Option<bool>,
}

/// Properties for rng-random objects.
///
#[qapi(condition = "CONFIG_POSIX")]
#[qapi(added = "6.0.0")]
pub struct RngRandomProperties {
    /// the filename of the device on the host to obtain entropy
    /// from (default: "/dev/urandom")
    pub filename: Option<String>,
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "6.1.0"))]
    pub opened: Option<bool>,
}

/// Rocker switch OF-DPA flow
///
#[qapi(added = "2.4.0")]
pub struct RockerOfDpaFlow {
    /// flow action
    pub action: RockerOfDpaFlowAction,
    /// flow unique cookie ID
    pub cookie: u64,
    /// count of matches (hits) on flow
    pub hits: u64,
    /// flow key
    pub key: RockerOfDpaFlowKey,
    /// flow mask
    pub mask: RockerOfDpaFlowMask,
}

/// Rocker switch OF-DPA flow action
///
#[qapi(added = "2.4.0")]
pub struct RockerOfDpaFlowAction {
    /// next table ID
    #[qapi(name = "goto-tbl")]
    pub goto_tbl: Option<u32>,
    /// group ID
    #[qapi(name = "group-id")]
    pub group_id: Option<u32>,
    /// new VLAN ID
    #[qapi(name = "new-vlan-id")]
    pub new_vlan_id: Option<u16>,
    /// physical output port
    ///
    /// .. note:: Optional members may or may not appear in the flow action
    /// depending if they're relevant to the flow action.
    #[qapi(name = "out-pport")]
    pub out_pport: Option<u32>,
    /// tunnel logical port ID
    #[qapi(name = "tunnel-lport")]
    pub tunnel_lport: Option<u32>,
    /// VLAN ID
    #[qapi(name = "vlan-id")]
    pub vlan_id: Option<u16>,
}

/// Rocker switch OF-DPA flow key
///
#[qapi(added = "2.4.0")]
pub struct RockerOfDpaFlowKey {
    /// Ethernet header destination MAC address
    #[qapi(name = "eth-dst")]
    pub eth_dst: Option<String>,
    /// Ethernet header source MAC address
    #[qapi(name = "eth-src")]
    pub eth_src: Option<String>,
    /// Ethernet header type
    #[qapi(name = "eth-type")]
    pub eth_type: Option<u16>,
    /// physical input port
    #[qapi(name = "in-pport")]
    pub in_pport: Option<u32>,
    /// IP header destination address
    ///
    /// .. note:: Optional members may or may not appear in the flow key
    /// depending if they're relevant to the flow key.
    #[qapi(name = "ip-dst")]
    pub ip_dst: Option<String>,
    /// IP Header protocol field
    #[qapi(name = "ip-proto")]
    pub ip_proto: Option<u8>,
    /// IP header TOS field
    #[qapi(name = "ip-tos")]
    pub ip_tos: Option<u8>,
    /// key priority, 0 being lowest priority
    pub priority: u32,
    /// flow table ID
    #[qapi(name = "tbl-id")]
    pub tbl_id: u32,
    /// tunnel ID
    #[qapi(name = "tunnel-id")]
    pub tunnel_id: Option<u32>,
    /// VLAN ID
    #[qapi(name = "vlan-id")]
    pub vlan_id: Option<u16>,
}

/// Rocker switch OF-DPA flow mask
///
#[qapi(added = "2.4.0")]
pub struct RockerOfDpaFlowMask {
    /// Ethernet header destination MAC address
    #[qapi(name = "eth-dst")]
    pub eth_dst: Option<String>,
    /// Ethernet header source MAC address
    #[qapi(name = "eth-src")]
    pub eth_src: Option<String>,
    /// physical input port
    #[qapi(name = "in-pport")]
    pub in_pport: Option<u32>,
    /// IP Header protocol field
    #[qapi(name = "ip-proto")]
    pub ip_proto: Option<u8>,
    /// IP header TOS field
    ///
    /// .. note:: Optional members may or may not appear in the flow mask
    /// depending if they're relevant to the flow mask.
    #[qapi(name = "ip-tos")]
    pub ip_tos: Option<u8>,
    /// tunnel ID
    #[qapi(name = "tunnel-id")]
    pub tunnel_id: Option<u32>,
    /// VLAN ID
    #[qapi(name = "vlan-id")]
    pub vlan_id: Option<u16>,
}

/// Rocker switch OF-DPA group
///
#[qapi(added = "2.4.0")]
pub struct RockerOfDpaGroup {
    /// next group ID
    #[qapi(name = "group-id")]
    pub group_id: Option<u32>,
    /// list of next group IDs
    #[qapi(name = "group-ids")]
    pub group_ids: Option<Vec<u32>>,
    /// group unique ID
    pub id: u32,
    /// group index, unique with group type
    pub index: Option<u32>,
    /// output physical port number
    #[qapi(name = "out-pport")]
    pub out_pport: Option<u32>,
    /// pop VLAN headr from packet
    #[qapi(name = "pop-vlan")]
    pub pop_vlan: Option<u8>,
    /// physical port number
    pub pport: Option<u32>,
    /// set destination MAC address in Ethernet header
    #[qapi(name = "set-eth-dst")]
    pub set_eth_dst: Option<String>,
    /// set source MAC address in Ethernet header
    #[qapi(name = "set-eth-src")]
    pub set_eth_src: Option<String>,
    /// VLAN ID to set
    #[qapi(name = "set-vlan-id")]
    pub set_vlan_id: Option<u16>,
    /// perform TTL check
    ///
    /// .. note:: Optional members may or may not appear in the group
    /// depending if they're relevant to the group type.
    #[qapi(name = "ttl-check")]
    pub ttl_check: Option<u8>,
    /// group type
    pub r#type: u8,
    /// VLAN ID
    #[qapi(name = "vlan-id")]
    pub vlan_id: Option<u16>,
}

/// Rocker switch port information.
///
#[qapi(added = "2.4.0")]
pub struct RockerPort {
    /// port link autoneg
    pub autoneg: RockerPortAutoneg,
    /// port link duplex
    pub duplex: RockerPortDuplex,
    /// port is enabled for I/O
    pub enabled: bool,
    /// physical link is UP on port
    #[qapi(name = "link-up")]
    pub link_up: bool,
    /// port name
    pub name: String,
    /// port link speed in Mbps
    pub speed: u32,
}

/// An enumeration of port autoneg states.
///
#[qapi(added = "2.4.0")]
pub enum RockerPortAutoneg {
    /// autoneg is off
    #[qapi(name = "off")]
    Off,
    /// autoneg is on
    #[qapi(name = "on")]
    On,
}

/// An enumeration of port duplex states.
///
#[qapi(added = "2.4.0")]
pub enum RockerPortDuplex {
    /// full duplex
    #[qapi(name = "full")]
    Full,
    /// half duplex
    #[qapi(name = "half")]
    Half,
}

/// Rocker switch information.
///
#[qapi(added = "2.4.0")]
pub struct RockerSwitch {
    /// switch ID
    pub id: u64,
    /// switch name
    pub name: String,
    /// number of front-panel ports
    pub ports: u32,
}

/// An enumeration of VM run states.
///
#[qapi(added = "1.0.0")]
pub enum RunState {
    #[qapi(name = "colo")]
    #[qapi(added = "2.8.0")]
    Colo,
    /// QEMU is running on a debugger
    #[qapi(name = "debug")]
    Debug,
    /// guest is paused to finish the migration process
    #[qapi(name = "finish-migrate")]
    FinishMigrate,
    /// guest has been panicked as a result of guest OS panic
    ///
    #[qapi(name = "guest-panicked")]
    #[qapi(added = "1.5.0")]
    GuestPanicked,
    /// guest is paused waiting for an incoming migration.  Note
    /// that this state does not tell whether the machine will start at the
    /// end of the migration.  This depends on the command-line -S option and
    /// any invocation of 'stop' or 'cont' that has happened since QEMU was
    /// started.
    #[qapi(name = "inmigrate")]
    Inmigrate,
    /// An internal error that prevents further guest execution
    /// has occurred
    #[qapi(name = "internal-error")]
    InternalError,
    /// the last IOP has failed and the device is configured to pause
    /// on I/O errors
    #[qapi(name = "io-error")]
    IoError,
    /// guest has been paused via the 'stop' command
    #[qapi(name = "paused")]
    Paused,
    /// guest is paused following a successful 'migrate'
    #[qapi(name = "postmigrate")]
    Postmigrate,
    #[qapi(name = "preconfig")]
    #[qapi(added = "3.0.0")]
    #[qapi(removed = "6.0.0")]
    Preconfig,
    /// QEMU was started with -S and guest has not started
    #[qapi(name = "prelaunch")]
    Prelaunch,
    /// guest is paused to restore VM state
    #[qapi(name = "restore-vm")]
    RestoreVm,
    /// guest is actively running
    #[qapi(name = "running")]
    Running,
    /// guest is paused to save the VM state
    #[qapi(name = "save-vm")]
    SaveVm,
    /// guest is shut down (and -no-shutdown is in use)
    #[qapi(name = "shutdown")]
    Shutdown,
    /// guest is suspended (ACPI S3)
    #[qapi(name = "suspended")]
    #[qapi(added = "1.1.0")]
    Suspended,
    /// the watchdog action is configured to pause and has been triggered
    #[qapi(name = "watchdog")]
    Watchdog,
}

/// Rx-filter information for a NIC.
///
#[qapi(added = "1.6.0")]
pub struct RxFilterInfo {
    /// whether to receive broadcast
    #[qapi(name = "broadcast-allowed")]
    pub broadcast_allowed: bool,
    /// the main macaddr string
    #[qapi(name = "main-mac")]
    pub main_mac: String,
    /// multicast receive state
    pub multicast: RxState,
    /// multicast table is overflowed or not
    #[qapi(name = "multicast-overflow")]
    pub multicast_overflow: bool,
    /// a list of multicast macaddr string
    #[qapi(name = "multicast-table")]
    pub multicast_table: Vec<String>,
    /// net client name
    pub name: String,
    /// whether promiscuous mode is enabled
    pub promiscuous: bool,
    /// unicast receive state
    pub unicast: RxState,
    /// unicast table is overflowed or not
    #[qapi(name = "unicast-overflow")]
    pub unicast_overflow: bool,
    /// a list of unicast macaddr string
    #[qapi(name = "unicast-table")]
    pub unicast_table: Vec<String>,
    /// vlan receive state (Since 2.0)
    #[qapi(added = "2.0.0")]
    pub vlan: Option<RxState>,
    /// a list of active vlan id
    #[qapi(name = "vlan-table")]
    pub vlan_table: Vec<i64>,
}

/// Packets receiving state
///
#[qapi(added = "1.6.0")]
pub enum RxState {
    /// receive all assigned packets
    #[qapi(name = "all")]
    All,
    /// don't receive any assigned packet
    #[qapi(name = "none")]
    None,
    /// filter assigned packets according to the mac-table
    #[qapi(name = "normal")]
    Normal,
}

/// An enumeration of CPU entitlements that can be assumed by a virtual
/// S390 CPU
///
#[qapi(added = "9.2.0")]
pub enum S390CpuEntitlement {
    #[qapi(name = "auto")]
    Auto,
    #[qapi(name = "high")]
    High,
    #[qapi(name = "low")]
    Low,
    #[qapi(name = "medium")]
    Medium,
}

/// An enumeration of CPU polarization that can be assumed by a virtual
/// S390 CPU
///
#[qapi(condition = "TARGET_S390X")]
#[qapi(added = "9.2.0")]
pub enum S390CpuPolarization {
    #[qapi(name = "horizontal")]
    Horizontal,
    #[qapi(name = "vertical")]
    Vertical,
}

/// An enumeration of cpu states that can be assumed by a virtual S390
/// CPU
///
#[qapi(added = "9.2.0")]
pub enum S390CpuState {
    #[qapi(name = "check-stop")]
    CheckStop,
    #[qapi(name = "load")]
    Load,
    #[qapi(name = "operating")]
    Operating,
    #[qapi(name = "stopped")]
    Stopped,
    #[qapi(name = "uninitialized")]
    Uninitialized,
}

/// Reason why the CPU is in a crashed state.
///
#[qapi(added = "2.12.0")]
pub enum S390CrashReason {
    /// the CPU has entered a disabled wait state
    #[qapi(name = "disabled-wait")]
    DisabledWait,
    /// clock comparator or cpu timer interrupt with new PSW
    /// enabled for external interrupts
    #[qapi(name = "extint-loop")]
    ExtintLoop,
    /// operation exception interrupt with invalid code at the
    /// program interrupt new PSW
    #[qapi(name = "opint-loop")]
    OpintLoop,
    /// program interrupt with BAD new PSW
    #[qapi(name = "pgmint-loop")]
    PgmintLoop,
    /// no crash reason was set
    #[qapi(name = "unknown")]
    Unknown,
}

#[qapi(added = "2.5.0")]
pub struct SchemaInfo {
    /// names of features associated with the entity, in no
    /// particular order.  (since 4.1 for object types, 4.2 for
    /// commands, 5.0 for the rest)
    #[qapi(added = "5.0.0")]
    pub features: Option<Vec<String>>,
    /// the entity's meta type, inherited from @base.
    #[qapi(name = "meta-type")]
    #[qapi(discriminator)]
    pub meta_type: SchemaMetaType,
    /// the entity's name, inherited from @base.  The `SchemaInfo` is
    /// always referenced by this name.  Commands and events have the
    /// name defined in the QAPI schema.  Unlike command and event
    /// names, type names are not part of the wire ABI.  Consequently,
    /// type names are meaningless strings here, although they are still
    /// guaranteed unique regardless of @meta-type.
    pub name: String,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<SchemaInfoBranch>,
}

/// Additional `SchemaInfo` members for meta-type 'alternate'.
///
#[qapi(added = "2.5.0")]
pub struct SchemaInfoAlternate {
    /// the alternate type's members, in no particular order.  The
    /// members' wire encoding is distinct, see
    /// :doc:`/devel/qapi-code-gen` section Alternate types.
    ///
    /// On the wire, this can be any of the members.
    pub members: Vec<SchemaInfoAlternateMember>,
}

/// An alternate member.
///
#[qapi(added = "2.5.0")]
pub struct SchemaInfoAlternateMember {
    /// the name of the member's type.
    pub r#type: String,
}

/// Additional `SchemaInfo` members for meta-type 'array'.
///
#[qapi(added = "2.5.0")]
pub struct SchemaInfoArray {
    /// the array type's element type.
    ///
    /// Values of this type are JSON array on the wire.
    #[qapi(name = "element-type")]
    pub element_type: String,
}

/// Members common to any @SchemaInfo.
///
#[qapi(added = "2.5.0")]
#[qapi(removed = "2.6.0")]
pub struct SchemaInfoBase {
    #[qapi(name = "meta-type")]
    pub meta_type: SchemaMetaType,
    pub name: String,
}

#[qapi(added = "2.5.0")]
pub enum SchemaInfoBranch {
    #[qapi(name = "alternate")]
    Alternate(SchemaInfoAlternate),
    #[qapi(name = "array")]
    Array(SchemaInfoArray),
    #[qapi(name = "builtin")]
    Builtin(SchemaInfoBuiltin),
    #[qapi(name = "command")]
    Command(SchemaInfoCommand),
    #[qapi(name = "enum")]
    Enum(SchemaInfoEnum),
    #[qapi(name = "event")]
    Event(SchemaInfoEvent),
    #[qapi(name = "object")]
    Object(SchemaInfoObject),
}

/// Additional `SchemaInfo` members for meta-type 'builtin'.
///
#[qapi(added = "2.5.0")]
pub struct SchemaInfoBuiltin {
    /// the JSON type used for this type on the wire.
    #[qapi(name = "json-type")]
    pub json_type: JsonType,
}

#[qapi(added = "2.5.0")]
pub struct SchemaInfoCommand {
    #[qapi(name = "allow-oob")]
    #[qapi(added = "2.12.0")]
    pub allow_oob: Option<bool>,
    #[qapi(name = "arg-type")]
    pub arg_type: String,
    #[qapi(added = "4.2.0")]
    #[qapi(removed = "5.0.0")]
    pub features: Option<Vec<String>>,
    #[qapi(name = "ret-type")]
    pub ret_type: String,
}

/// Additional SchemaInfo members for meta-type 'enum'.
///
#[qapi(added = "2.5.0")]
pub struct SchemaInfoEnum {
    #[qapi(added = "6.2.0")]
    pub members: Option<Vec<SchemaInfoEnumMember>>,
    /// the enumeration type's values, in no particular order.
    ///
    /// Values of this type are JSON string on the wire.
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "6.2.0"))]
    pub values: Vec<String>,
}

/// An object member.
///
#[qapi(added = "6.2.0")]
pub struct SchemaInfoEnumMember {
    /// names of features associated with the member, in no
    /// particular order.
    pub features: Option<Vec<String>>,
    /// the member's name, as defined in the QAPI schema.
    pub name: String,
}

/// Additional `SchemaInfo` members for meta-type 'event'.
///
#[qapi(added = "2.5.0")]
pub struct SchemaInfoEvent {
    /// the name of the object type that provides the event's
    /// parameters.
    #[qapi(name = "arg-type")]
    pub arg_type: String,
}

/// Additional `SchemaInfo` members for meta-type 'object'.
///
#[qapi(added = "2.5.0")]
pub struct SchemaInfoObject {
    /// names of features associated with the type, in no particular
    /// order. (since: 4.1)
    ///
    /// Values of this type are JSON object on the wire.
    #[qapi(added = "4.1.0")]
    #[qapi(removed = "5.0.0")]
    pub features: Option<Vec<String>>,
    /// with this name must exist.
    pub members: Vec<SchemaInfoObjectMember>,
    /// the name of the member serving as type tag.  An element of
    pub tag: Option<String>,
    /// variant members, i.e. additional members that depend on
    /// the type tag's value.  Present exactly when @tag is present.
    /// The variants are in no particular order, and may even differ
    /// from the order of the values of the enum type of the @tag.
    ///
    /// Values of this type are JSON object on the wire.
    pub variants: Option<Vec<SchemaInfoObjectVariant>>,
}

/// An object member.
///
#[qapi(added = "2.5.0")]
pub struct SchemaInfoObjectMember {
    /// default when used as command parameter.  If absent, the
    /// parameter is mandatory.  If present, the value must be null.
    /// The parameter is optional, and behavior when it's missing is not
    /// specified here.  Future extension: if present and non-null, the
    /// parameter is optional, and defaults to this value.
    pub default: Option<serde_json::Value>,
    /// names of features associated with the member, in no
    /// particular order.  (since 5.0)
    #[qapi(added = "5.0.0")]
    pub features: Option<Vec<String>>,
    /// the member's name, as defined in the QAPI schema.
    pub name: String,
    /// the name of the member's type.
    pub r#type: String,
}

/// The variant members for a value of the type tag.
///
#[qapi(added = "2.5.0")]
pub struct SchemaInfoObjectVariant {
    /// a value of the type tag.
    pub case: String,
    /// the name of the object type that provides the variant members
    /// when the type tag has value @case.
    pub r#type: String,
}

/// This is a `SchemaInfo`'s meta type, i.e. the kind of entity it
/// describes.
///
#[qapi(added = "2.5.0")]
pub enum SchemaMetaType {
    /// an alternate type
    #[qapi(name = "alternate")]
    Alternate,
    /// an array type
    #[qapi(name = "array")]
    Array,
    /// a predefined type such as 'int' or 'bool'.
    #[qapi(name = "builtin")]
    Builtin,
    /// a QMP command
    #[qapi(name = "command")]
    Command,
    /// an enumeration type
    #[qapi(name = "enum")]
    Enum,
    /// a QMP event
    #[qapi(name = "event")]
    Event,
    /// an object type (struct or union)
    #[qapi(name = "object")]
    Object,
}

/// Properties for objects of classes derived from secret-common.
///
#[qapi(added = "6.0.0")]
pub struct SecretCommonProperties {
    /// the data format that the secret is provided in
    /// (default: raw)
    pub format: Option<QCryptoSecretFormat>,
    /// the random initialization vector used for encryption of this
    /// particular secret.  Should be a base64 encrypted string of the
    /// 16-byte IV.  Mandatory if @keyid is given.  Ignored if @keyid is
    /// absent.
    pub iv: Option<String>,
    /// the name of another secret that should be used to decrypt
    /// the provided data.  If not present, the data is assumed to be
    /// unencrypted.
    pub keyid: Option<String>,
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "6.1.0"))]
    #[qapi(removed = "9.2.0")]
    pub loaded: Option<bool>,
}

/// Properties for secret_keyring objects.
///
#[qapi(condition = "CONFIG_SECRET_KEYRING")]
#[qapi(added = "6.0.0")]
pub struct SecretKeyringProperties {
    /// the data format that the secret is provided in
    /// (default: raw)
    pub format: Option<QCryptoSecretFormat>,
    /// the random initialization vector used for encryption of this
    /// particular secret.  Should be a base64 encrypted string of the
    /// 16-byte IV.  Mandatory if @keyid is given.  Ignored if @keyid is
    /// absent.
    pub iv: Option<String>,
    /// the name of another secret that should be used to decrypt
    /// the provided data.  If not present, the data is assumed to be
    /// unencrypted.
    pub keyid: Option<String>,
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "6.1.0"))]
    #[qapi(removed = "9.2.0")]
    pub loaded: Option<bool>,
    /// serial number that identifies a key to get from the kernel
    pub serial: i32,
}

/// Properties for secret objects.
///
/// Either @data or @file must be provided, but not both.
///
#[qapi(added = "6.0.0")]
pub struct SecretProperties {
    /// the associated with the secret from
    pub data: Option<String>,
    /// the filename to load the data associated with the secret from
    pub file: Option<String>,
    /// the data format that the secret is provided in
    /// (default: raw)
    pub format: Option<QCryptoSecretFormat>,
    /// the random initialization vector used for encryption of this
    /// particular secret.  Should be a base64 encrypted string of the
    /// 16-byte IV.  Mandatory if @keyid is given.  Ignored if @keyid is
    /// absent.
    pub iv: Option<String>,
    /// the name of another secret that should be used to decrypt
    /// the provided data.  If not present, the data is assumed to be
    /// unencrypted.
    pub keyid: Option<String>,
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "6.1.0"))]
    #[qapi(removed = "9.2.0")]
    pub loaded: Option<bool>,
}

/// An action to take on changing a password on a connection with active
/// clients.
///
#[qapi(added = "7.0.0")]
pub enum SetPasswordAction {
    /// disconnect existing clients
    #[qapi(name = "disconnect")]
    Disconnect,
    /// fail the command if clients are connected
    #[qapi(name = "fail")]
    Fail,
    /// maintain existing clients
    #[qapi(name = "keep")]
    Keep,
}

/// Options for `set_password`.
///
#[qapi(added = "7.0.0")]
pub struct SetPasswordOptions {
    /// How to handle existing clients when changing the
    /// password.  If nothing is specified, defaults to 'keep'.  For
    /// VNC, only 'keep' is currently implemented.
    pub connected: Option<SetPasswordAction>,
    /// the new password
    pub password: String,
    /// - 'vnc' to modify the VNC server password
    /// - 'spice' to modify the Spice server password
    #[qapi(discriminator)]
    pub protocol: DisplayProtocol,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<SetPasswordOptionsBranch>,
}

#[qapi(added = "7.0.0")]
pub enum SetPasswordOptionsBranch {
    #[qapi(name = "spice")]
    Spice,
    #[qapi(name = "vnc")]
    Vnc(SetPasswordOptionsVnc),
}

/// Options for `set_password` specific to the VNC protocol.
///
#[qapi(added = "7.0.0")]
pub struct SetPasswordOptionsVnc {
    /// The id of the display where the password should be
    /// changed.  Defaults to the first.
    pub display: Option<String>,
}

/// The struct describes attestation report for a Secure Encrypted
/// Virtualization feature.
///
#[qapi(condition = "defined(TARGET_I386)")]
#[qapi(added = "6.1.0")]
pub struct SevAttestationReport {
    /// guest attestation report (base64 encoded)
    pub data: String,
}

/// The struct describes capability for a Secure Encrypted
/// Virtualization feature.
///
#[qapi(condition = "defined(TARGET_I386)")]
#[qapi(added = "2.12.0")]
pub struct SevCapability {
    /// C-bit location in page table entry
    pub cbitpos: i64,
    /// PDH certificate chain (base64 encoded)
    #[qapi(name = "cert-chain")]
    pub cert_chain: String,
    /// Unique ID of CPU0 (base64 encoded) (since 7.1)
    #[qapi(name = "cpu0-id")]
    #[qapi(added = "7.1.0")]
    pub cpu0_id: Option<String>,
    /// Platform Diffie-Hellman key (base64 encoded)
    pub pdh: String,
    /// Number of physical address bit reduction when
    /// SEV is enabled
    #[qapi(name = "reduced-phys-bits")]
    pub reduced_phys_bits: i64,
}

/// Properties common to objects that are derivatives of sev-common.
///
#[qapi(feature = "confidential-guest-reset")]
#[qapi(added = "9.1.0")]
pub struct SevCommonProperties {
    /// C-bit location in page table entry (default: 0)
    pub cbitpos: Option<u32>,
    /// if true, add hashes of kernel/initrd/cmdline to a
    /// designated guest firmware page for measured boot with -kernel
    /// (default: false) (since 6.2)
    #[qapi(name = "kernel-hashes")]
    pub kernel_hashes: Option<bool>,
    /// number of bits in physical addresses that become
    /// unavailable when SEV is enabled
    #[qapi(name = "reduced-phys-bits")]
    pub reduced_phys_bits: u32,
    /// SEV device to use (default: "/dev/sev")
    #[qapi(name = "sev-device")]
    pub sev_device: Option<String>,
}

/// Information specific to legacy SEV/SEV-ES guests.
///
#[qapi(condition = "TARGET_I386")]
#[qapi(added = "9.1.0")]
pub struct SevGuestInfo {
    /// SEV firmware handle
    pub handle: u32,
    /// SEV policy value
    pub policy: u32,
}

/// Properties for sev-guest objects.
///
#[qapi(added = "6.0.0")]
pub struct SevGuestProperties {
    /// C-bit location in page table entry (default: 0)
    pub cbitpos: Option<u32>,
    /// guest owners DH certificate (encoded with base64)
    #[qapi(name = "dh-cert-file")]
    pub dh_cert_file: Option<String>,
    /// SEV firmware handle (default: 0)
    pub handle: Option<u32>,
    /// if true, add hashes of kernel/initrd/cmdline to a
    /// designated guest firmware page for measured boot with -kernel
    /// (default: false) (since 6.2)
    #[qapi(name = "kernel-hashes")]
    #[qapi(added = "6.2.0")]
    pub kernel_hashes: Option<bool>,
    /// Use legacy KVM_SEV_INIT KVM interface for creating
    /// the VM.  The newer KVM_SEV_INIT2 interface, from Linux >= 6.10,
    /// syncs additional vCPU state when initializing the VMSA
    /// structures, which will result in a different guest measurement.
    /// Set this to 'on' to force compatibility with older QEMU or kernel
    /// versions that rely on legacy KVM_SEV_INIT behavior.  'auto' will
    /// behave identically to 'on', but will automatically switch to
    /// using KVM_SEV_INIT2 if the user specifies any additional options
    /// that require it.  If set to 'off', QEMU will require
    /// KVM_SEV_INIT2 unconditionally.  (default: off) (since 9.1)
    #[qapi(name = "legacy-vm-type")]
    #[qapi(added = "9.1.0")]
    pub legacy_vm_type: Option<OnOffAuto>,
    /// SEV policy value (default: 0x1)
    pub policy: Option<u32>,
    /// number of bits in physical addresses that become
    /// unavailable when SEV is enabled
    #[qapi(name = "reduced-phys-bits")]
    pub reduced_phys_bits: u32,
    /// guest owners session parameters (encoded with base64)
    #[qapi(name = "session-file")]
    pub session_file: Option<String>,
    /// SEV device to use (default: "/dev/sev")
    #[qapi(name = "sev-device")]
    pub sev_device: Option<String>,
}

/// An enumeration indicating the type of SEV guest being run.
///
#[qapi(condition = "TARGET_I386")]
#[qapi(added = "9.1.0")]
pub enum SevGuestType {
    /// The guest is a legacy SEV or SEV-ES guest.
    #[qapi(name = "sev")]
    Sev,
    /// The guest is an SEV-SNP guest.
    #[qapi(name = "sev-snp")]
    SevSnp,
}

/// Information about Secure Encrypted Virtualization (SEV) support
///
#[qapi(condition = "defined(TARGET_I386)")]
#[qapi(added = "2.12.0")]
pub struct SevInfo {
    /// SEV API major version
    #[qapi(name = "api-major")]
    pub api_major: u8,
    /// SEV API minor version
    #[qapi(name = "api-minor")]
    pub api_minor: u8,
    /// SEV FW build id
    #[qapi(name = "build-id")]
    pub build_id: u8,
    /// true if SEV is active
    pub enabled: bool,
    /// SEV firmware handle
    #[qapi(removed = "9.1.0")]
    pub handle: Option<u32>,
    /// SEV policy value
    #[qapi(removed = "9.1.0")]
    pub policy: Option<u32>,
    /// Type of SEV guest being run
    #[qapi(name = "sev-type")]
    #[qapi(discriminator)]
    #[qapi(added = "9.1.0")]
    pub sev_type: Option<SevGuestType>,
    /// SEV guest state
    pub state: SevState,
    #[qapi(union)]
    #[qapi(flatten)]
    #[qapi(added = "9.1.0")]
    pub u: Option<SevInfoBranch>,
}

#[qapi(condition = "TARGET_I386")]
#[qapi(added = "9.1.0")]
pub enum SevInfoBranch {
    #[qapi(name = "sev")]
    Sev(SevGuestInfo),
    #[qapi(name = "sev-snp")]
    SevSnp(SevSnpGuestInfo),
}

/// SEV Guest Launch measurement information
///
#[qapi(condition = "defined(TARGET_I386)")]
#[qapi(added = "2.12.0")]
pub struct SevLaunchMeasureInfo {
    /// the measurement value encoded in base64
    pub data: String,
}

/// Information specific to SEV-SNP guests.
///
#[qapi(condition = "TARGET_I386")]
#[qapi(added = "9.1.0")]
pub struct SevSnpGuestInfo {
    /// SEV-SNP policy value
    #[qapi(name = "snp-policy")]
    pub snp_policy: u64,
}

/// Properties for sev-snp-guest objects.  Most of these are direct
/// arguments for the KVM_SNP_* interfaces documented in the Linux
/// kernel source under
/// Documentation/arch/x86/amd-memory-encryption.rst, which are in turn
/// closely coupled with the SNP_INIT/SNP_LAUNCH_* firmware commands
/// documented in the SEV-SNP Firmware ABI Specification (Rev 0.9).
///
/// More usage information is also available in the QEMU source tree
/// under docs/amd-memory-encryption.
///
#[qapi(added = "9.1.0")]
pub struct SevSnpGuestProperties {
    /// true if 'id-auth' blob contains the
    /// 'AUTHOR_KEY' field defined SEV-SNP firmware ABI (default: false)
    #[qapi(name = "author-key-enabled")]
    pub author_key_enabled: Option<bool>,
    /// C-bit location in page table entry (default: 0)
    pub cbitpos: Option<u32>,
    /// 16-byte, base64-encoded blob to report
    /// hypervisor-defined workarounds, corresponding to the 'GOSVW'
    /// parameter of the SNP_LAUNCH_START command defined in the SEV-SNP
    /// firmware ABI (default: all-zero)
    #[qapi(name = "guest-visible-workarounds")]
    pub guest_visible_workarounds: Option<String>,
    /// 32-byte, base64-encoded, user-defined blob to provide to
    /// the guest, as documented for the 'HOST_DATA' parameter of the
    /// SNP_LAUNCH_FINISH command in the SEV-SNP firmware ABI (default:
    /// all-zero)
    #[qapi(name = "host-data")]
    pub host_data: Option<String>,
    /// 4096-byte, base64-encoded blob to provide the 'ID
    /// Authentication Information Structure' for the SNP_LAUNCH_FINISH
    /// command defined in the SEV-SNP firmware ABI (default: all-zero)
    #[qapi(name = "id-auth")]
    pub id_auth: Option<String>,
    /// 96-byte, base64-encoded blob to provide the 'ID Block'
    /// structure for the SNP_LAUNCH_FINISH command defined in the
    /// SEV-SNP firmware ABI (default: all-zero)
    #[qapi(name = "id-block")]
    pub id_block: Option<String>,
    /// if true, add hashes of kernel/initrd/cmdline to a
    /// designated guest firmware page for measured boot with -kernel
    /// (default: false) (since 6.2)
    #[qapi(name = "kernel-hashes")]
    pub kernel_hashes: Option<bool>,
    /// the 'POLICY' parameter to the SNP_LAUNCH_START command, as
    /// defined in the SEV-SNP firmware ABI (default: 0x30000)
    pub policy: Option<u64>,
    /// number of bits in physical addresses that become
    /// unavailable when SEV is enabled
    #[qapi(name = "reduced-phys-bits")]
    pub reduced_phys_bits: u32,
    /// SEV device to use (default: "/dev/sev")
    #[qapi(name = "sev-device")]
    pub sev_device: Option<String>,
    /// Guests are by default allowed to choose between VLEK
    /// (Versioned Loaded Endorsement Key) or VCEK (Versioned Chip
    /// Endorsement Key) when requesting attestation reports from
    /// firmware.  Set this to true to disable the use of VCEK.
    /// (default: false) (since: 9.1)
    #[qapi(name = "vcek-disabled")]
    pub vcek_disabled: Option<bool>,
}

/// An enumeration of SEV state information used during `query-sev`.
///
#[qapi(condition = "defined(TARGET_I386)")]
#[qapi(added = "2.12.0")]
pub enum SevState {
    /// The guest is currently being launched; ciphertext
    /// data is being imported.
    #[qapi(name = "launch-secret")]
    LaunchSecret,
    /// The guest is currently being launched; plaintext
    /// data and register state is being imported.
    #[qapi(name = "launch-update")]
    LaunchUpdate,
    /// The guest is currently being migrated from another
    /// machine.
    #[qapi(name = "receive-update")]
    ReceiveUpdate,
    /// The guest is fully launched or migrated in.
    #[qapi(name = "running")]
    Running,
    /// The guest is currently being migrated out to another
    /// machine.
    #[qapi(name = "send-update")]
    SendUpdate,
    /// The guest is uninitialized.
    #[qapi(name = "uninit")]
    Uninit,
}

/// Sgx EPC cmdline information
///
#[qapi(name = "SgxEPC")]
#[qapi(added = "6.2.0")]
pub struct SgxEpc {
    /// memory backend linked with device
    pub memdev: String,
    /// the numa node (Since: 7.0)
    #[qapi(added = "7.0.0")]
    pub node: Option<i64>,
}

/// Sgx EPC state information
///
#[qapi(name = "SgxEPCDeviceInfo")]
#[qapi(added = "6.2.0")]
pub struct SgxEpcDeviceInfo {
    /// device's ID
    pub id: Option<String>,
    /// physical address in memory, where device is mapped
    pub memaddr: u64,
    /// memory backend linked with device
    pub memdev: String,
    /// the numa node (Since: 7.0)
    #[qapi(added = "7.0.0")]
    pub node: Option<i64>,
    /// size of memory that the device provides
    pub size: u64,
}

#[qapi(name = "SgxEPCDeviceInfoWrapper")]
#[qapi(added = "6.2.0")]
pub struct SgxEpcDeviceInfoWrapper {
    /// Sgx EPC state information
    pub data: SgxEpcDeviceInfo,
}

/// SGX properties of machine types.
///
#[qapi(name = "SgxEPCProperties")]
#[qapi(added = "6.2.0")]
pub struct SgxEpcProperties {
    /// list of ids of memory-backend-epc objects.
    #[qapi(name = "sgx-epc")]
    pub sgx_epc: Vec<SgxEpc>,
}

/// Information about intel SGX EPC section
///
#[qapi(added = "10.1.0")]
pub struct SgxEpcSection {
    /// the numa node
    pub node: i64,
    /// the size of EPC section
    pub size: u64,
}

/// Information about intel SGX EPC section info
///
#[qapi(name = "SGXEPCSection")]
#[qapi(added = "7.0.0")]
#[qapi(removed = "10.1.0")]
pub struct SgxepcSection {
    /// the numa node
    pub node: i64,
    /// the size of EPC section
    pub size: u64,
}

/// Information about intel Safe Guard eXtension (SGX) support
///
#[qapi(name = "SGXInfo")]
#[qapi(condition = "TARGET_I386")]
#[qapi(added = "6.2.0")]
pub struct SgxInfo {
    /// true if FLC is supported
    pub flc: bool,
    /// The EPC section size for guest
    #[qapi(name = "section-size")]
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "7.0.0"))]
    #[qapi(removed = "8.0.0")]
    pub section_size: Option<u64>,
    /// The EPC sections information (Since: 7.0)
    #[qapi(added = "7.0.0")]
    pub sections: Option<Vec<SgxEpcSection>>,
    /// true if SGX is supported
    pub sgx: bool,
    /// true if SGX1 is supported
    pub sgx1: bool,
    /// true if SGX2 is supported
    pub sgx2: bool,
}

#[qapi(added = "2.12.0")]
#[qapi(removed = "6.1.0")]
pub struct SheepdogRedundancy {
    #[qapi(discriminator)]
    pub r#type: SheepdogRedundancyType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<SheepdogRedundancyBranch>,
}

#[qapi(added = "2.12.0")]
#[qapi(removed = "6.1.0")]
pub enum SheepdogRedundancyBranch {
    #[qapi(name = "erasure-coded")]
    ErasureCoded(SheepdogRedundancyErasureCoded),
    #[qapi(name = "full")]
    Full(SheepdogRedundancyFull),
}

#[qapi(added = "2.12.0")]
#[qapi(removed = "6.1.0")]
pub struct SheepdogRedundancyErasureCoded {
    /// Number of data strips to use (one of {2,4,8,16})
    #[qapi(name = "data-strips")]
    pub data_strips: i64,
    /// Number of parity strips to use (between 1 and 15)
    #[qapi(name = "parity-strips")]
    pub parity_strips: i64,
}

#[qapi(added = "2.12.0")]
#[qapi(removed = "6.1.0")]
pub struct SheepdogRedundancyFull {
    /// Number of copies to use (between 1 and 31)
    pub copies: i64,
}

#[qapi(added = "2.12.0")]
#[qapi(removed = "6.1.0")]
pub enum SheepdogRedundancyType {
    /// Create an erasure coded vdi with x data strips and
    /// y parity strips
    #[qapi(name = "erasure-coded")]
    ErasureCoded,
    /// Create a fully replicated vdi with x copies
    #[qapi(name = "full")]
    Full,
}

/// Possible QEMU actions upon guest shutdown
///
#[qapi(added = "6.0.0")]
pub enum ShutdownAction {
    /// pause the VM
    #[qapi(name = "pause")]
    Pause,
    /// Shutdown the VM and exit
    #[qapi(name = "poweroff")]
    Poweroff,
}

#[qapi(added = "4.0.0")]
pub enum ShutdownCause {
    #[qapi(name = "guest-panic")]
    GuestPanic,
    #[qapi(name = "guest-reset")]
    GuestReset,
    #[qapi(name = "guest-shutdown")]
    GuestShutdown,
    #[qapi(name = "host-error")]
    HostError,
    #[qapi(name = "host-qmp-quit")]
    HostQmpQuit,
    #[qapi(name = "host-qmp-system-reset")]
    HostQmpSystemReset,
    #[qapi(name = "host-signal")]
    HostSignal,
    #[qapi(name = "host-ui")]
    HostUi,
    #[qapi(name = "none")]
    None,
    #[qapi(name = "snapshot-load")]
    #[qapi(added = "7.2.0")]
    SnapshotLoad,
    #[qapi(name = "subsystem-reset")]
    SubsystemReset,
}

#[qapi(added = "7.0.0")]
pub enum SmbiosEntryPointType {
    /// SMBIOS version 2.1 (32-bit) Entry Point
    #[qapi(name = "32")]
    _32,
    /// SMBIOS version 3.0 (64-bit) Entry Point
    #[qapi(name = "64")]
    _64,
    /// Either 2.x or 3.x SMBIOS version, 2.x if configuration can be
    /// described by it and 3.x otherwise (since: 9.0)
    #[qapi(name = "auto")]
    #[qapi(added = "9.0.0")]
    Auto,
}

/// Cache information for SMP system.
///
#[qapi(added = "9.2.0")]
pub struct SmpCacheProperties {
    /// Cache name, which is the combination of cache level and
    /// cache type.
    pub cache: CacheLevelAndType,
    /// Cache topology level.  It accepts the CPU topology
    /// enumeration as the parameter, i.e., CPUs in the same
    /// topology container share the same cache.
    pub topology: CpuTopologyLevel,
}

/// List wrapper of `SmpCacheProperties`.
///
#[qapi(added = "9.2.0")]
pub struct SmpCachePropertiesWrapper {
    /// the list of `SmpCacheProperties`.
    pub caches: Vec<SmpCacheProperties>,
}

/// Schema for CPU topology configuration.  A missing value lets QEMU
/// figure out a suitable value based on the ones that are provided.
///
/// The members other than @cpus and @maxcpus define a topology of
/// containers.
///
/// The ordering from highest/coarsest to lowest/finest is: @drawers,
#[qapi(name = "SMPConfiguration")]
#[qapi(added = "6.1.0")]
pub struct SmpConfiguration {
    /// number of books in the CPU topology (since 8.2)
    #[qapi(added = "8.2.0")]
    pub books: Option<i64>,
    /// number of clusters per parent container (since 7.0)
    #[qapi(added = "7.0.0")]
    pub clusters: Option<i64>,
    /// number of cores per parent container
    pub cores: Option<i64>,
    /// number of virtual CPUs in the virtual machine
    pub cpus: Option<i64>,
    /// number of dies per parent container
    pub dies: Option<i64>,
    /// number of drawers in the CPU topology (since 8.2)
    #[qapi(added = "8.2.0")]
    pub drawers: Option<i64>,
    /// maximum number of hotpluggable virtual CPUs in the virtual
    /// machine
    pub maxcpus: Option<i64>,
    /// number of modules per parent container (since 9.1)
    #[qapi(added = "9.1.0")]
    pub modules: Option<i64>,
    /// number of sockets per parent container
    pub sockets: Option<i64>,
    /// number of threads per core
    pub threads: Option<i64>,
}

#[qapi(added = "1.3.0")]
pub struct SnapshotInfo {
    /// fractional part in nano seconds to be used with date-sec
    #[qapi(name = "date-nsec")]
    pub date_nsec: i64,
    /// UTC date of the snapshot in seconds
    #[qapi(name = "date-sec")]
    pub date_sec: i64,
    /// Current instruction count.  Appears when execution
    /// record/replay is enabled.  Used for "time-traveling" to match
    /// the moment in the recorded execution with the snapshots.  This
    /// counter may be obtained through `query-replay` command
    /// (since 5.2)
    #[qapi(added = "5.2.0")]
    pub icount: Option<i64>,
    /// unique snapshot id
    pub id: String,
    /// user chosen name
    pub name: String,
    /// fractional part in nano seconds to be used with
    /// vm-clock-sec
    #[qapi(name = "vm-clock-nsec")]
    pub vm_clock_nsec: i64,
    /// VM clock relative to boot in seconds
    #[qapi(name = "vm-clock-sec")]
    pub vm_clock_sec: i64,
    /// size of the VM state
    #[qapi(name = "vm-state-size")]
    pub vm_state_size: i64,
}

/// Captures the address of a socket, which could also be a socket file
/// descriptor
///
#[qapi(added = "1.3.0")]
pub struct SocketAddress {
    #[qapi(discriminator)]
    #[qapi(name = "type")]
    #[qapi(removed = "2.10.0")]
    pub r_type: Option<SocketAddressType>,
    /// Transport type
    #[qapi(discriminator)]
    #[qapi(added = "2.10.0")]
    pub r#type: Option<SocketAddressType>,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<SocketAddressBranch>,
}

#[qapi(added = "1.3.0")]
pub enum SocketAddressBranch {
    #[qapi(name = "fd")]
    Fd(FdSocketAddress),
    #[qapi(name = "inet")]
    Inet(InetSocketAddress),
    #[qapi(name = "unix")]
    Unix(UnixSocketAddress),
    #[qapi(name = "vsock")]
    #[qapi(added = "2.8.0")]
    Vsock(VsockSocketAddress),
}

/// Captures the address of a socket
///
#[qapi(added = "2.9.0")]
#[qapi(removed = "2.10.0")]
pub struct SocketAddressFlat {
    /// Transport type
    ///
    /// This is just like SocketAddress, except it's a flat union rather
    /// than a simple union.  Nicer because it avoids nesting on the wire,
    /// i.e. this form has fewer {}.
    #[qapi(discriminator)]
    pub r#type: SocketAddressFlatType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<SocketAddressFlatBranch>,
}

#[qapi(added = "2.9.0")]
#[qapi(removed = "2.10.0")]
pub enum SocketAddressFlatBranch {
    #[qapi(name = "fd")]
    Fd(QapiString),
    #[qapi(name = "inet")]
    Inet(InetSocketAddress),
    #[qapi(name = "unix")]
    Unix(UnixSocketAddress),
    #[qapi(name = "vsock")]
    Vsock(VsockSocketAddress),
}

/// Available SocketAddressFlat types
///
#[qapi(added = "2.9.0")]
#[qapi(removed = "2.10.0")]
pub enum SocketAddressFlatType {
    #[qapi(name = "fd")]
    Fd,
    /// Internet address
    #[qapi(name = "inet")]
    Inet,
    /// Unix domain socket
    #[qapi(name = "unix")]
    Unix,
    #[qapi(name = "vsock")]
    Vsock,
}

/// Captures the address of a socket, which could also be a named file
/// descriptor
///
#[qapi(added = "2.10.0")]
pub struct SocketAddressLegacy {
    #[qapi(discriminator)]
    #[qapi(name = "type")]
    #[qapi(removed = "6.2.0")]
    pub r_type: Option<SocketAddressLegacyType>,
    /// Transport type
    #[qapi(discriminator)]
    #[qapi(added = "6.2.0")]
    pub r#type: Option<SocketAddressType>,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<SocketAddressLegacyBranch>,
}

#[qapi(added = "2.10.0")]
pub enum SocketAddressLegacyBranch {
    #[qapi(name = "fd")]
    Fd(FdSocketAddressWrapper),
    #[qapi(name = "inet")]
    Inet(InetSocketAddressWrapper),
    #[qapi(name = "unix")]
    Unix(UnixSocketAddressWrapper),
    #[qapi(name = "vsock")]
    Vsock(VsockSocketAddressWrapper),
}

#[qapi(added = "2.10.0")]
#[qapi(removed = "6.2.0")]
pub enum SocketAddressLegacyType {
    #[qapi(name = "fd")]
    Fd,
    #[qapi(name = "inet")]
    Inet,
    #[qapi(name = "unix")]
    Unix,
    #[qapi(name = "vsock")]
    Vsock,
}

/// Available `SocketAddress` types
///
#[qapi(added = "1.3.0")]
pub enum SocketAddressType {
    /// Socket file descriptor
    #[qapi(name = "fd")]
    Fd,
    /// Internet address
    #[qapi(name = "inet")]
    Inet,
    /// Unix domain socket
    #[qapi(name = "unix")]
    Unix,
    /// VMCI address
    #[qapi(name = "vsock")]
    #[qapi(added = "2.8.0")]
    Vsock,
}

/// The basic information for SPICE network connection
///
#[qapi(condition = "defined(CONFIG_SPICE)")]
#[qapi(added = "2.1.0")]
pub struct SpiceBasicInfo {
    /// address family
    pub family: NetworkAddressFamily,
    /// IP address
    pub host: String,
    /// port number
    pub port: String,
}

/// Information about a SPICE client channel.
///
#[qapi(condition = "defined(CONFIG_SPICE)")]
#[qapi(added = "1.0.0")]
pub struct SpiceChannel {
    /// SPICE channel ID number.  Usually "0", might be
    /// different when multiple channels of the same type exist, such as
    /// multiple display channels in a multihead setup
    #[qapi(name = "channel-id")]
    pub channel_id: i64,
    /// SPICE channel type number.  "1" is the main control
    /// channel, filter for this one if you want to track spice sessions
    /// only
    #[qapi(name = "channel-type")]
    pub channel_type: i64,
    /// SPICE connection id number.  All channels with the
    /// same id belong to the same SPICE session.
    #[qapi(name = "connection-id")]
    pub connection_id: i64,
    /// address family
    pub family: NetworkAddressFamily,
    /// IP address
    pub host: String,
    /// port number
    pub port: String,
    /// true if the channel is encrypted, false otherwise.
    pub tls: bool,
}

/// Information about the SPICE session.
///
#[qapi(condition = "defined(CONFIG_SPICE)")]
#[qapi(added = "1.0.0")]
pub struct SpiceInfo {
    /// the current authentication type used by the server
    ///
    /// - 'none' if no authentication is being used
    /// - 'spice' uses SASL or direct TLS authentication, depending on
    /// command line options
    pub auth: Option<String>,
    /// a list of `SpiceChannel` for each active spice channel
    pub channels: Option<Vec<SpiceChannel>>,
    /// SPICE server version.
    #[qapi(name = "compiled-version")]
    pub compiled_version: Option<String>,
    /// true if the SPICE server is enabled, false otherwise
    pub enabled: bool,
    /// The hostname the SPICE server is bound to.  This depends on
    /// the name resolution on the host and may be an IP address.
    pub host: Option<String>,
    /// true if the last guest migration completed and spice
    /// migration had completed as well, false otherwise (since 1.4)
    #[qapi(added = "1.2.1")]
    pub migrated: Option<bool>,
    /// The mode in which the mouse cursor is displayed
    /// currently.  Can be determined by the client or the server, or
    /// unknown if spice server doesn't provide this information.
    /// (since: 1.1)
    #[qapi(name = "mouse-mode")]
    #[qapi(added = "1.1.0")]
    pub mouse_mode: Option<SpiceQueryMouseMode>,
    /// The SPICE server's port number.
    pub port: Option<i64>,
    /// The SPICE server's TLS port number.
    #[qapi(name = "tls-port")]
    pub tls_port: Option<i64>,
}

/// An enumeration of Spice mouse states.
///
#[qapi(condition = "defined(CONFIG_SPICE)")]
#[qapi(added = "1.1.0")]
pub enum SpiceQueryMouseMode {
    /// Mouse cursor position is determined by the client.
    #[qapi(name = "client")]
    Client,
    /// Mouse cursor position is determined by the server.
    #[qapi(name = "server")]
    Server,
    /// No information is available about mouse mode used by the
    /// spice server.
    #[qapi(name = "unknown")]
    Unknown,
}

/// Information about a SPICE server
///
#[qapi(condition = "defined(CONFIG_SPICE)")]
#[qapi(added = "2.1.0")]
pub struct SpiceServerInfo {
    /// authentication method
    pub auth: Option<String>,
    /// address family
    pub family: NetworkAddressFamily,
    /// IP address
    pub host: String,
    /// port number
    pub port: String,
}

/// sp-mem device state information
///
#[qapi(added = "11.1.0")]
pub struct SpMemDeviceInfo {
    /// physical address, where device is mapped
    pub addr: u64,
    /// device's ID
    pub id: Option<String>,
    /// memory backend linked with device
    pub memdev: String,
    /// NUMA proximity domain to which the device is assigned
    pub node: i64,
    /// size of memory that the device provides, in bytes
    pub size: u64,
}

#[qapi(added = "11.1.0")]
pub struct SpMemDeviceInfoWrapper {
    /// sp-mem device state information
    pub data: SpMemDeviceInfo,
}

#[qapi(added = "2.12.0")]
pub struct SshHostKeyCheck {
    /// How to check the host key
    #[qapi(discriminator)]
    pub mode: SshHostKeyCheckMode,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<SshHostKeyCheckBranch>,
}

#[qapi(added = "2.12.0")]
pub enum SshHostKeyCheckBranch {
    #[qapi(name = "hash")]
    Hash(SshHostKeyHash),
    /// Check the host key against the known_hosts file
    #[qapi(name = "known_hosts")]
    KnownHosts,
    /// Don't check the host key at all
    #[qapi(name = "none")]
    None,
}

#[qapi(added = "2.12.0")]
pub enum SshHostKeyCheckHashType {
    /// The given hash is an md5 hash
    #[qapi(name = "md5")]
    Md5,
    /// The given hash is an sha1 hash
    #[qapi(name = "sha1")]
    Sha1,
    /// The given hash is an sha256 hash
    #[qapi(name = "sha256")]
    #[qapi(added = "6.1.0")]
    Sha256,
}

#[qapi(added = "2.12.0")]
pub enum SshHostKeyCheckMode {
    /// Compare the host key with a given hash
    #[qapi(name = "hash")]
    Hash,
    /// Check the host key against the known_hosts file
    #[qapi(name = "known_hosts")]
    KnownHosts,
    /// Don't check the host key at all
    #[qapi(name = "none")]
    None,
}

/// For those union branches that don't need additional fields.
///
#[qapi(added = "2.12.0")]
#[qapi(removed = "3.0.0")]
pub struct SshHostKeyDummy {}

#[qapi(added = "2.12.0")]
pub struct SshHostKeyHash {
    /// The expected hash value
    pub hash: String,
    /// The hash algorithm used for the hash
    pub r#type: SshHostKeyCheckHashType,
}

/// SMMUv3 SubstreamID size configuration mode.
///
#[qapi(added = "11.0.0")]
pub enum SsidSizeMode {
    #[qapi(name = "0")]
    _0,
    #[qapi(name = "1")]
    _1,
    #[qapi(name = "10")]
    _10,
    #[qapi(name = "11")]
    _11,
    #[qapi(name = "12")]
    _12,
    #[qapi(name = "13")]
    _13,
    #[qapi(name = "14")]
    _14,
    #[qapi(name = "15")]
    _15,
    #[qapi(name = "16")]
    _16,
    #[qapi(name = "17")]
    _17,
    #[qapi(name = "18")]
    _18,
    #[qapi(name = "19")]
    _19,
    #[qapi(name = "2")]
    _2,
    #[qapi(name = "20")]
    _20,
    #[qapi(name = "3")]
    _3,
    #[qapi(name = "4")]
    _4,
    #[qapi(name = "5")]
    _5,
    #[qapi(name = "6")]
    _6,
    #[qapi(name = "7")]
    _7,
    #[qapi(name = "8")]
    _8,
    #[qapi(name = "9")]
    _9,
    /// derive from host IOMMU capabilities
    ///
    /// Values 0-20: SSIDSIZE value in bits.  0 disables SubstreamID.
    #[qapi(name = "auto")]
    Auto,
}

#[qapi(added = "7.1.0")]
pub struct Stats {
    /// name of stat.
    pub name: String,
    /// stat value.
    pub value: StatsValue,
}

/// The arguments to the `query-stats` command; specifies a target for
/// which to request statistics and optionally the required subset of
/// information for that target.
///
#[qapi(added = "7.1.0")]
pub struct StatsFilter {
    /// which providers to request statistics from, and
    /// optionally which named values to return within each provider
    pub providers: Option<Vec<StatsRequest>>,
    /// the kind of objects to query.  Note that each possible
    /// target may enable additional filtering options
    #[qapi(discriminator)]
    pub target: StatsTarget,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<StatsFilterBranch>,
}

#[qapi(added = "7.1.0")]
pub enum StatsFilterBranch {
    /// statistics that apply to a crypto device (since 8.0)
    #[qapi(name = "cryptodev")]
    #[qapi(added = "8.0.0")]
    Cryptodev,
    #[qapi(name = "vcpu")]
    Vcpu(StatsVcpuFilter),
    /// statistics that apply to the entire virtual machine or the
    /// entire QEMU process.
    #[qapi(name = "vm")]
    Vm,
}

/// Enumeration of statistics providers.
///
#[qapi(added = "7.1.0")]
pub enum StatsProvider {
    /// since 8.0
    #[qapi(name = "cryptodev")]
    #[qapi(added = "8.0.0")]
    Cryptodev,
    /// since 7.1
    #[qapi(name = "kvm")]
    Kvm,
}

/// Indicates a set of statistics that should be returned by
/// `query-stats`.
///
#[qapi(added = "7.1.0")]
pub struct StatsRequest {
    /// statistics to be returned (all if omitted).
    pub names: Option<Vec<String>>,
    /// provider for which to return statistics.
    pub provider: StatsProvider,
}

#[qapi(added = "7.1.0")]
pub struct StatsResult {
    /// provider for this set of statistics.
    pub provider: StatsProvider,
    /// Path to the object for which the statistics are returned,
    /// if the object is exposed in the QOM tree
    #[qapi(name = "qom-path")]
    pub qom_path: Option<String>,
    /// list of statistics.
    pub stats: Vec<Stats>,
}

/// Schema for all available statistics for a provider and target.
///
#[qapi(added = "7.1.0")]
pub struct StatsSchema {
    /// provider for this set of statistics.
    pub provider: StatsProvider,
    /// list of statistics.
    pub stats: Vec<StatsSchemaValue>,
    /// the kind of object that can be queried through the
    /// provider.
    pub target: StatsTarget,
}

/// Schema for a single statistic.
///
#[qapi(added = "7.1.0")]
pub struct StatsSchemaValue {
    /// base for the multiple of @unit in which the statistic is
    /// measured.  Only present if @exponent is non-zero; @base and
    pub base: Option<i8>,
    /// Present when @type is "linear-histogram", contains the
    /// width of each bucket of the histogram.
    #[qapi(name = "bucket-size")]
    pub bucket_size: Option<u32>,
    /// exponent for the multiple of @unit in which the statistic
    /// is expressed, or 0 for the basic unit
    pub exponent: i16,
    /// name of the statistic; each element of the schema is uniquely
    /// identified by a target, a provider (both available in
    /// `StatsSchema`) and the name.
    pub name: String,
    /// kind of statistic.
    pub r#type: StatsType,
    /// basic unit of measure for the statistic; if missing, the
    /// statistic is a simple number or counter.
    pub unit: Option<StatsUnit>,
}

/// The kinds of objects on which one can request statistics.
///
#[qapi(added = "7.1.0")]
pub enum StatsTarget {
    /// statistics that apply to a crypto device (since 8.0)
    #[qapi(name = "cryptodev")]
    #[qapi(added = "8.0.0")]
    Cryptodev,
    /// statistics that apply to a single virtual CPU.
    #[qapi(name = "vcpu")]
    Vcpu,
    /// statistics that apply to the entire virtual machine or the
    /// entire QEMU process.
    #[qapi(name = "vm")]
    Vm,
}

/// Enumeration of statistics types
///
#[qapi(added = "7.1.0")]
pub enum StatsType {
    /// stat is cumulative; value can only increase.
    #[qapi(name = "cumulative")]
    Cumulative,
    /// stat is instantaneous; value can increase or decrease.
    #[qapi(name = "instant")]
    Instant,
    /// stat is a linear histogram.
    #[qapi(name = "linear-histogram")]
    LinearHistogram,
    /// stat is a logarithmic histogram, with one bucket
    /// for each power of two.
    #[qapi(name = "log2-histogram")]
    Log2Histogram,
    /// stat is the peak value; value can only increase.
    #[qapi(name = "peak")]
    Peak,
}

/// Enumeration of unit of measurement for statistics
///
#[qapi(added = "7.1.0")]
pub enum StatsUnit {
    /// stat is a boolean value.
    #[qapi(name = "boolean")]
    Boolean,
    /// stat reported in bytes.
    #[qapi(name = "bytes")]
    Bytes,
    /// stat reported in clock cycles.
    #[qapi(name = "cycles")]
    Cycles,
    /// stat reported in seconds.
    #[qapi(name = "seconds")]
    Seconds,
}

#[qapi(added = "7.1.0")]
pub enum StatsValue {
    /// single boolean value.
    #[qapi(name = "boolean")]
    Boolean(bool),
    /// list of unsigned 64-bit integers (used for histograms).
    #[qapi(name = "list")]
    List(Vec<u64>),
    /// single unsigned 64-bit integers.
    #[qapi(name = "scalar")]
    Scalar(u64),
}

#[qapi(name = "StatsVCPUFilter")]
#[qapi(added = "7.1.0")]
pub struct StatsVcpuFilter {
    /// list of QOM paths for the desired vCPU objects.
    pub vcpus: Option<Vec<String>>,
}

/// Information about VM run state
///
#[qapi(added = "1.0.0")]
pub struct StatusInfo {
    /// true if all VCPUs are runnable, false if not runnable
    pub running: bool,
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "8.1.0"))]
    #[qapi(removed = "9.0.0")]
    pub singlestep: Option<bool>,
    /// the virtual machine `RunState`
    pub status: RunState,
}

#[qapi(added = "6.2.0")]
#[qapi(removed = "9.0.0")]
pub struct StringWrapper {
    pub data: QapiString,
}

/// This is a string value or the explicit lack of a string (null
/// pointer in C).  Intended for cases when 'optional absent' already
/// has a different meaning.
///
#[qapi(added = "2.10.0")]
pub enum StrOrNull {
    /// no string value
    #[qapi(name = "n")]
    N(()),
    /// the string value
    #[qapi(name = "s")]
    S(String),
}

/// The comprehensive enumeration of QEMU system emulation ("softmmu")
/// targets.  Run "./configure --help" in the project root directory,
/// and look for the \*-softmmu targets near the "--target-list" option.
/// The individual target constants are not documented here, for the
/// time being.
///
#[qapi(added = "3.0.0")]
pub enum SysEmuTarget {
    #[qapi(name = "aarch64")]
    Aarch64,
    #[qapi(name = "alpha")]
    Alpha,
    #[qapi(name = "arm")]
    Arm,
    /// since 5.1
    #[qapi(name = "avr")]
    #[qapi(added = "5.1.0")]
    Avr,
    #[qapi(name = "cris")]
    #[qapi(removed = "9.2.0")]
    Cris,
    /// since 11.0
    ///
    /// .. note:: The resulting QMP strings can be appended to the
    /// "qemu-system-" prefix to produce the corresponding QEMU
    /// executable name.  This is true even for "qemu-system-x86_64".
    #[qapi(name = "hexagon")]
    #[qapi(added = "11.0.0")]
    Hexagon,
    #[qapi(name = "hppa")]
    Hppa,
    #[qapi(name = "i386")]
    I386,
    #[qapi(name = "lm32")]
    #[qapi(removed = "6.1.0")]
    Lm32,
    /// since 7.1
    #[qapi(name = "loongarch64")]
    #[qapi(added = "7.1.0")]
    Loongarch64,
    #[qapi(name = "m68k")]
    M68k,
    #[qapi(name = "microblaze")]
    Microblaze,
    #[qapi(name = "microblazeel")]
    #[qapi(removed = "11.0.0")]
    Microblazeel,
    #[qapi(name = "mips")]
    Mips,
    #[qapi(name = "mips64")]
    Mips64,
    #[qapi(name = "mips64el")]
    Mips64el,
    #[qapi(name = "mipsel")]
    Mipsel,
    #[qapi(name = "moxie")]
    #[qapi(removed = "6.1.0")]
    Moxie,
    #[qapi(name = "nios2")]
    #[qapi(removed = "9.1.0")]
    Nios2,
    #[qapi(name = "or1k")]
    Or1k,
    #[qapi(name = "ppc")]
    Ppc,
    #[qapi(name = "ppc64")]
    Ppc64,
    #[qapi(name = "ppcemb")]
    #[qapi(removed = "3.1.0")]
    Ppcemb,
    #[qapi(name = "riscv32")]
    Riscv32,
    #[qapi(name = "riscv64")]
    Riscv64,
    /// since 5.0
    #[qapi(name = "rx")]
    #[qapi(added = "5.0.0")]
    Rx,
    #[qapi(name = "s390x")]
    S390x,
    #[qapi(name = "sh4")]
    Sh4,
    #[qapi(name = "sh4eb")]
    Sh4eb,
    #[qapi(name = "sparc")]
    Sparc,
    #[qapi(name = "sparc64")]
    Sparc64,
    #[qapi(name = "tricore")]
    Tricore,
    #[qapi(name = "unicore32")]
    #[qapi(removed = "6.1.0")]
    Unicore32,
    #[qapi(name = "x86_64")]
    X8664,
    #[qapi(name = "xtensa")]
    Xtensa,
    #[qapi(name = "xtensaeb")]
    Xtensaeb,
}

/// Information describing the QEMU target.
///
#[qapi(added = "1.2.0")]
#[qapi(removed = "10.1.0")]
pub struct TargetInfo {
    /// the target architecture
    pub arch: SysEmuTarget,
}

/// Target CPU emulation type
///
/// These parameters correspond to the softmmu binary CPU name that is currently
/// running.
///
#[qapi(added = "1.2.0")]
#[qapi(removed = "1.6.0")]
pub enum TargetType {
    #[qapi(name = "alpha")]
    Alpha,
    #[qapi(name = "arm")]
    Arm,
    #[qapi(name = "cris")]
    Cris,
    #[qapi(name = "i386")]
    I386,
    #[qapi(name = "lm32")]
    Lm32,
    #[qapi(name = "m68k")]
    M68k,
    #[qapi(name = "microblaze")]
    Microblaze,
    #[qapi(name = "microblazeel")]
    Microblazeel,
    #[qapi(name = "mips")]
    Mips,
    #[qapi(name = "mips64")]
    Mips64,
    #[qapi(name = "mips64el")]
    Mips64el,
    #[qapi(name = "mipsel")]
    Mipsel,
    #[qapi(name = "moxie")]
    #[qapi(added = "1.5.0")]
    Moxie,
    #[qapi(name = "or32")]
    Or32,
    #[qapi(name = "ppc")]
    Ppc,
    #[qapi(name = "ppc64")]
    Ppc64,
    #[qapi(name = "ppcemb")]
    Ppcemb,
    #[qapi(name = "s390x")]
    S390x,
    #[qapi(name = "sh4")]
    Sh4,
    #[qapi(name = "sh4eb")]
    Sh4eb,
    #[qapi(name = "sparc")]
    Sparc,
    #[qapi(name = "sparc64")]
    Sparc64,
    #[qapi(name = "unicore32")]
    Unicore32,
    #[qapi(name = "x86_64")]
    X8664,
    #[qapi(name = "xtensa")]
    Xtensa,
    #[qapi(name = "xtensaeb")]
    Xtensaeb,
}

/// Properties for tdx-guest objects.
///
#[qapi(feature = "confidential-guest-reset")]
#[qapi(added = "10.1.0")]
pub struct TdxGuestProperties {
    /// The 'attributes' of a TD guest that is passed to
    /// KVM_TDX_INIT_VM
    pub attributes: Option<u64>,
    /// ID for non-owner-defined configuration of the guest TD,
    /// e.g., run-time or OS configuration (base64 encoded SHA384
    /// digest).  Defaults to all zeros.
    pub mrconfigid: Option<String>,
    /// ID for the guest TD’s owner (base64 encoded SHA384
    /// digest).  Defaults to all zeros.
    pub mrowner: Option<String>,
    /// ID for owner-defined configuration of the guest TD,
    /// e.g., specific to the workload rather than the run-time or OS
    /// (base64 encoded SHA384 digest).  Defaults to all zeros.
    pub mrownerconfig: Option<String>,
    /// socket address for Quote Generation
    /// Service (QGS).  QGS is a daemon running on the host.  Without
    /// it, the guest will not be able to get a TD quote for
    /// attestation.
    #[qapi(name = "quote-generation-socket")]
    pub quote_generation_socket: Option<SocketAddress>,
    /// toggle bit 28 of TD attributes to control
    /// disabling of EPT violation conversion to #VE on guest TD access
    /// of PENDING pages.  Some guest OS (e.g., Linux TD guest) may
    /// require this to be set, otherwise they refuse to boot.
    #[qapi(name = "sept-ve-disable")]
    pub sept_ve_disable: Option<bool>,
}

/// Properties for thread context objects.
///
#[qapi(added = "7.2.0")]
pub struct ThreadContextProperties {
    /// the list of host CPU numbers used as CPU affinity for
    /// all threads created in the thread context (default: QEMU main
    /// thread CPU affinity)
    #[qapi(name = "cpu-affinity")]
    pub cpu_affinity: Option<Vec<u16>>,
    /// the list of host node numbers that will be resolved
    /// to a list of host CPU numbers used as CPU affinity.  This is a
    /// shortcut for specifying the list of host CPU numbers belonging
    /// to the host nodes manually by setting @cpu-affinity.
    /// (default: QEMU main thread affinity)
    #[qapi(name = "node-affinity")]
    pub node_affinity: Option<Vec<u16>>,
}

/// Properties for throttle-group objects.
///
/// The options starting with x- are aliases for the same key without x- in
/// the @limits object. As indicated by the x- prefix, this is not a stable
/// interface and may be removed or changed incompatibly in the future. Use
#[qapi(added = "6.0.0")]
pub struct ThrottleGroupProperties {
    /// limits to apply for this throttle group
    pub limits: Option<ThrottleLimits>,
    #[qapi(name = "x-bps-read")]
    #[qapi(feature = "unstable")]
    pub x_bps_read: Option<i64>,
    #[qapi(name = "x-bps-read-max")]
    #[qapi(feature = "unstable")]
    pub x_bps_read_max: Option<i64>,
    #[qapi(name = "x-bps-read-max-length")]
    #[qapi(feature = "unstable")]
    pub x_bps_read_max_length: Option<i64>,
    #[qapi(name = "x-bps-total")]
    #[qapi(feature = "unstable")]
    pub x_bps_total: Option<i64>,
    #[qapi(name = "x-bps-total-max")]
    #[qapi(feature = "unstable")]
    pub x_bps_total_max: Option<i64>,
    #[qapi(name = "x-bps-total-max-length")]
    #[qapi(feature = "unstable")]
    pub x_bps_total_max_length: Option<i64>,
    #[qapi(name = "x-bps-write")]
    #[qapi(feature = "unstable")]
    pub x_bps_write: Option<i64>,
    #[qapi(name = "x-bps-write-max")]
    #[qapi(feature = "unstable")]
    pub x_bps_write_max: Option<i64>,
    #[qapi(name = "x-bps-write-max-length")]
    #[qapi(feature = "unstable")]
    pub x_bps_write_max_length: Option<i64>,
    #[qapi(name = "x-iops-read")]
    #[qapi(feature = "unstable")]
    pub x_iops_read: Option<i64>,
    #[qapi(name = "x-iops-read-max")]
    #[qapi(feature = "unstable")]
    pub x_iops_read_max: Option<i64>,
    #[qapi(name = "x-iops-read-max-length")]
    #[qapi(feature = "unstable")]
    pub x_iops_read_max_length: Option<i64>,
    #[qapi(name = "x-iops-size")]
    #[qapi(feature = "unstable")]
    pub x_iops_size: Option<i64>,
    #[qapi(name = "x-iops-total")]
    #[qapi(feature = "unstable")]
    pub x_iops_total: Option<i64>,
    #[qapi(name = "x-iops-total-max")]
    #[qapi(feature = "unstable")]
    pub x_iops_total_max: Option<i64>,
    #[qapi(name = "x-iops-total-max-length")]
    #[qapi(feature = "unstable")]
    pub x_iops_total_max_length: Option<i64>,
    #[qapi(name = "x-iops-write")]
    #[qapi(feature = "unstable")]
    pub x_iops_write: Option<i64>,
    #[qapi(name = "x-iops-write-max")]
    #[qapi(feature = "unstable")]
    pub x_iops_write_max: Option<i64>,
    #[qapi(name = "x-iops-write-max-length")]
    #[qapi(feature = "unstable")]
    pub x_iops_write_max_length: Option<i64>,
}

/// Limit parameters for throttling.  Since some limit combinations are
/// illegal, limits should always be set in one transaction.  All fields
/// are optional.  When setting limits, if a field is missing the
/// current value is not changed.
///
#[qapi(added = "2.11.0")]
pub struct ThrottleLimits {
    /// limit read bytes per second
    #[qapi(name = "bps-read")]
    pub bps_read: Option<i64>,
    /// total bytes read burst
    #[qapi(name = "bps-read-max")]
    pub bps_read_max: Option<i64>,
    /// length of the bps-read-max burst period, in
    /// seconds.  It must only be set if @bps-read-max is set as well.
    #[qapi(name = "bps-read-max-length")]
    pub bps_read_max_length: Option<i64>,
    /// limit total bytes per second
    #[qapi(name = "bps-total")]
    pub bps_total: Option<i64>,
    /// total bytes burst
    #[qapi(name = "bps-total-max")]
    pub bps_total_max: Option<i64>,
    /// length of the bps-total-max burst period, in
    /// seconds.  It must only be set if @bps-total-max is set as well.
    #[qapi(name = "bps-total-max-length")]
    pub bps_total_max_length: Option<i64>,
    /// limit write bytes per second
    #[qapi(name = "bps-write")]
    pub bps_write: Option<i64>,
    /// total bytes write burst
    #[qapi(name = "bps-write-max")]
    pub bps_write_max: Option<i64>,
    /// length of the bps-write-max burst period, in
    /// seconds.  It must only be set if @bps-write-max is set as well.
    #[qapi(name = "bps-write-max-length")]
    pub bps_write_max_length: Option<i64>,
    /// limit read operations per second
    #[qapi(name = "iops-read")]
    pub iops_read: Option<i64>,
    /// I/O operations read burst
    #[qapi(name = "iops-read-max")]
    pub iops_read_max: Option<i64>,
    /// length of the iops-read-max burst period, in
    /// seconds.  It must only be set if @iops-read-max is set as well.
    #[qapi(name = "iops-read-max-length")]
    pub iops_read_max_length: Option<i64>,
    /// when limiting by iops max size of an I/O in bytes
    #[qapi(name = "iops-size")]
    pub iops_size: Option<i64>,
    /// limit total I/O operations per second
    #[qapi(name = "iops-total")]
    pub iops_total: Option<i64>,
    /// I/O operations burst
    #[qapi(name = "iops-total-max")]
    pub iops_total_max: Option<i64>,
    /// length of the iops-total-max burst period,
    /// in seconds.  It must only be set if @iops-total-max is set as
    /// well.
    #[qapi(name = "iops-total-max-length")]
    pub iops_total_max_length: Option<i64>,
    /// limit write operations per second
    #[qapi(name = "iops-write")]
    pub iops_write: Option<i64>,
    /// I/O operations write burst
    #[qapi(name = "iops-write-max")]
    pub iops_write_max: Option<i64>,
    /// length of the iops-write-max burst period,
    /// in seconds.  It must only be set if @iops-write-max is set as
    /// well.
    #[qapi(name = "iops-write-max-length")]
    pub iops_write_max_length: Option<i64>,
}

/// Specifies unit in which time-related value is specified.
///
#[qapi(added = "8.2.0")]
pub enum TimeUnit {
    /// value is in milliseconds
    #[qapi(name = "millisecond")]
    Millisecond,
    /// value is in seconds
    #[qapi(name = "second")]
    Second,
}

/// Properties for tls-creds-anon objects.
///
#[qapi(added = "6.0.0")]
pub struct TlsCredsAnonProperties {
    /// the path of the directory that contains the credential files
    pub dir: Option<String>,
    /// whether the QEMU network backend that uses the
    /// credentials will be acting as a client or as a server
    /// (default: client)
    pub endpoint: Option<QCryptoTlsCredsEndpoint>,
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "6.1.0"))]
    #[qapi(removed = "9.2.0")]
    pub loaded: Option<bool>,
    /// a gnutls priority string as described at
    /// https://gnutls.org/manual/html_node/Priority-Strings.html
    pub priority: Option<String>,
    /// if true the peer credentials will be verified once the
    /// handshake is completed.  This is a no-op for anonymous
    /// credentials.  (default: true)
    #[qapi(name = "verify-peer")]
    pub verify_peer: Option<bool>,
}

/// Properties for objects of classes derived from tls-creds.
///
#[qapi(added = "6.0.0")]
pub struct TlsCredsProperties {
    /// the path of the directory that contains the credential files
    pub dir: Option<String>,
    /// whether the QEMU network backend that uses the
    /// credentials will be acting as a client or as a server
    /// (default: client)
    pub endpoint: Option<QCryptoTlsCredsEndpoint>,
    /// a gnutls priority string as described at
    /// https://gnutls.org/manual/html_node/Priority-Strings.html
    pub priority: Option<String>,
    /// if true the peer credentials will be verified once the
    /// handshake is completed.  This is a no-op for anonymous
    /// credentials.  (default: true)
    #[qapi(name = "verify-peer")]
    pub verify_peer: Option<bool>,
}

/// Properties for tls-creds-psk objects.
///
#[qapi(added = "6.0.0")]
pub struct TlsCredsPskProperties {
    /// the path of the directory that contains the credential files
    pub dir: Option<String>,
    /// whether the QEMU network backend that uses the
    /// credentials will be acting as a client or as a server
    /// (default: client)
    pub endpoint: Option<QCryptoTlsCredsEndpoint>,
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "6.1.0"))]
    #[qapi(removed = "9.2.0")]
    pub loaded: Option<bool>,
    /// a gnutls priority string as described at
    /// https://gnutls.org/manual/html_node/Priority-Strings.html
    pub priority: Option<String>,
    /// the username which will be sent to the server.  For
    /// clients only.  If absent, "qemu" is sent and the property will
    /// read back as an empty string.
    pub username: Option<String>,
    /// if true the peer credentials will be verified once the
    /// handshake is completed.  This is a no-op for anonymous
    /// credentials.  (default: true)
    #[qapi(name = "verify-peer")]
    pub verify_peer: Option<bool>,
}

/// Properties for tls-creds-x509 objects.
///
#[qapi(added = "6.0.0")]
pub struct TlsCredsX509Properties {
    /// the path of the directory that contains the credential files
    pub dir: Option<String>,
    /// whether the QEMU network backend that uses the
    /// credentials will be acting as a client or as a server
    /// (default: client)
    pub endpoint: Option<QCryptoTlsCredsEndpoint>,
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "6.1.0"))]
    #[qapi(removed = "9.2.0")]
    pub loaded: Option<bool>,
    /// For the server-key.pem and client-key.pem files which
    /// contain sensitive private keys, it is possible to use an
    /// encrypted version by providing the @passwordid parameter.  This
    /// provides the ID of a previously created secret object containing
    /// the password for decryption.
    pub passwordid: Option<String>,
    /// a gnutls priority string as described at
    /// https://gnutls.org/manual/html_node/Priority-Strings.html
    pub priority: Option<String>,
    /// if true, perform some sanity checks before using the
    /// credentials (default: true)
    #[qapi(name = "sanity-check")]
    pub sanity_check: Option<bool>,
    /// if true the peer credentials will be verified once the
    /// handshake is completed.  This is a no-op for anonymous
    /// credentials.  (default: true)
    #[qapi(name = "verify-peer")]
    pub verify_peer: Option<bool>,
}

/// Information about the TPM emulator type
///
#[qapi(name = "TPMEmulatorOptions")]
#[qapi(condition = "defined(CONFIG_TPM)")]
#[qapi(added = "2.11.0")]
pub struct TpmEmulatorOptions {
    /// Name of a unix socket chardev
    pub chardev: String,
}

#[qapi(name = "TPMEmulatorOptionsWrapper")]
#[qapi(condition = "CONFIG_TPM")]
#[qapi(added = "6.2.0")]
pub struct TpmEmulatorOptionsWrapper {
    /// Information about the TPM emulator type
    pub data: TpmEmulatorOptions,
}

/// Information about the TPM
///
#[qapi(name = "TPMInfo")]
#[qapi(condition = "defined(CONFIG_TPM)")]
#[qapi(added = "1.5.0")]
pub struct TpmInfo {
    /// The Id of the TPM
    pub id: String,
    /// The TPM frontend model
    pub model: TpmModel,
    /// The TPM (backend) type configuration options
    pub options: TpmTypeOptions,
}

/// An enumeration of TPM models
///
#[qapi(condition = "defined(CONFIG_TPM)")]
#[qapi(added = "1.5.0")]
pub enum TpmModel {
    /// TPM CRB model (since 2.12)
    #[qapi(name = "tpm-crb")]
    #[qapi(added = "2.12.0")]
    TpmCrb,
    /// TPM SPAPR model (since 5.0)
    #[qapi(name = "tpm-spapr")]
    #[qapi(added = "5.0.0")]
    TpmSpapr,
    /// TPM TIS model
    #[qapi(name = "tpm-tis")]
    TpmTis,
}

/// Information about the TPM passthrough type
///
#[qapi(name = "TPMPassthroughOptions")]
#[qapi(condition = "defined(CONFIG_TPM)")]
#[qapi(added = "1.5.0")]
pub struct TpmPassthroughOptions {
    /// string showing the TPM's sysfs cancel file for
    /// cancellation of TPM commands while they are executing
    #[qapi(name = "cancel-path")]
    pub cancel_path: Option<String>,
    /// string describing the path used for accessing the TPM device
    pub path: Option<String>,
}

#[qapi(name = "TPMPassthroughOptionsWrapper")]
#[qapi(condition = "CONFIG_TPM")]
#[qapi(added = "6.2.0")]
pub struct TpmPassthroughOptionsWrapper {
    /// Information about the TPM passthrough type
    pub data: TpmPassthroughOptions,
}

/// An enumeration of TPM types
///
#[qapi(condition = "defined(CONFIG_TPM)")]
#[qapi(added = "1.5.0")]
pub enum TpmType {
    /// Software Emulator TPM type (since 2.11)
    #[qapi(name = "emulator")]
    #[qapi(added = "2.11.0")]
    Emulator,
    /// TPM passthrough type
    #[qapi(name = "passthrough")]
    Passthrough,
}

/// A union referencing different TPM backend types' configuration
/// options
///
#[qapi(condition = "defined(CONFIG_TPM)")]
#[qapi(added = "1.5.0")]
pub struct TpmTypeOptions {
    #[qapi(discriminator)]
    #[qapi(name = "type")]
    #[qapi(removed = "6.2.0")]
    pub r_type: Option<TpmTypeOptionsType>,
    /// - 'passthrough' The configuration options for the TPM
    /// passthrough type
    /// - 'emulator' The configuration options for TPM emulator backend
    /// type
    #[qapi(discriminator)]
    #[qapi(added = "6.2.0")]
    pub r#type: Option<TpmType>,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<TpmTypeOptionsBranch>,
}

#[qapi(condition = "defined(CONFIG_TPM)")]
#[qapi(added = "1.5.0")]
pub enum TpmTypeOptionsBranch {
    #[qapi(name = "emulator")]
    #[qapi(added = "2.11.0")]
    Emulator(TpmEmulatorOptionsWrapper),
    #[qapi(name = "passthrough")]
    Passthrough(TpmPassthroughOptionsWrapper),
}

#[qapi(condition = "defined(CONFIG_TPM)")]
#[qapi(added = "1.5.0")]
#[qapi(removed = "6.2.0")]
pub enum TpmTypeOptionsType {
    #[qapi(name = "emulator")]
    #[qapi(added = "2.11.0")]
    Emulator,
    #[qapi(name = "passthrough")]
    Passthrough,
}

/// Information of a tracing event.
///
#[qapi(added = "2.2.0")]
pub struct TraceEventInfo {
    /// Event name.
    pub name: String,
    /// Tracing state.
    pub state: TraceEventState,
    /// Whether this is a per-vCPU event (since 2.7).
    ///
    /// An event is per-vCPU if it has the "vcpu" property in the "trace-events"
    /// files.
    #[qapi(feature = "deprecated")]
    #[qapi(added = "2.7.0")]
    #[qapi(deprecated(added = "8.1.0"))]
    #[qapi(removed = "9.1.0")]
    pub vcpu: Option<bool>,
}

/// State of a tracing event.
///
#[qapi(added = "2.2.0")]
pub enum TraceEventState {
    /// The event is dynamically disabled.
    #[qapi(name = "disabled")]
    Disabled,
    /// The event is dynamically enabled.
    #[qapi(name = "enabled")]
    Enabled,
    /// The event is statically disabled.
    #[qapi(name = "unavailable")]
    Unavailable,
}

/// A discriminated record of operations that can be performed with
/// `transaction`.
///
#[qapi(added = "1.6.0")]
pub struct TransactionAction {
    #[qapi(discriminator)]
    #[qapi(name = "type")]
    #[qapi(removed = "6.2.0")]
    pub r_type: Option<TransactionActionType>,
    /// the operation to be performed
    #[qapi(discriminator)]
    #[qapi(added = "6.2.0")]
    pub r#type: Option<TransactionActionKind>,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<TransactionActionBranch>,
}

#[qapi(added = "1.6.0")]
pub enum TransactionActionBranch {
    #[qapi(name = "abort")]
    Abort(AbortWrapper),
    #[qapi(name = "block-dirty-bitmap-add")]
    #[qapi(added = "2.5.0")]
    BlockDirtyBitmapAdd(BlockDirtyBitmapAddWrapper),
    #[qapi(name = "block-dirty-bitmap-clear")]
    #[qapi(added = "2.5.0")]
    BlockDirtyBitmapClear(BlockDirtyBitmapWrapper),
    #[qapi(name = "block-dirty-bitmap-disable")]
    #[qapi(added = "4.0.0")]
    BlockDirtyBitmapDisable(BlockDirtyBitmapWrapper),
    #[qapi(name = "block-dirty-bitmap-enable")]
    #[qapi(added = "4.0.0")]
    BlockDirtyBitmapEnable(BlockDirtyBitmapWrapper),
    #[qapi(name = "block-dirty-bitmap-merge")]
    #[qapi(added = "4.0.0")]
    BlockDirtyBitmapMerge(BlockDirtyBitmapMergeWrapper),
    #[qapi(name = "block-dirty-bitmap-remove")]
    #[qapi(added = "4.2.0")]
    BlockDirtyBitmapRemove(BlockDirtyBitmapWrapper),
    #[qapi(name = "blockdev-backup")]
    #[qapi(added = "2.3.0")]
    BlockdevBackup(BlockdevBackupWrapper),
    #[qapi(name = "blockdev-snapshot")]
    #[qapi(added = "2.5.0")]
    BlockdevSnapshot(BlockdevSnapshotWrapper),
    #[qapi(name = "blockdev-snapshot-internal-sync")]
    #[qapi(added = "1.7.0")]
    BlockdevSnapshotInternalSync(BlockdevSnapshotInternalWrapper),
    #[qapi(name = "blockdev-snapshot-sync")]
    BlockdevSnapshotSync(BlockdevSnapshotSyncWrapper),
    #[qapi(name = "drive-backup")]
    DriveBackup(DriveBackupWrapper),
    #[qapi(name = "x-block-dirty-bitmap-disable")]
    #[qapi(added = "3.0.0")]
    #[qapi(removed = "4.0.0")]
    XBlockDirtyBitmapDisable(BlockDirtyBitmap),
    #[qapi(name = "x-block-dirty-bitmap-enable")]
    #[qapi(added = "3.0.0")]
    #[qapi(removed = "4.0.0")]
    XBlockDirtyBitmapEnable(BlockDirtyBitmap),
    #[qapi(name = "x-block-dirty-bitmap-merge")]
    #[qapi(added = "3.1.0")]
    #[qapi(removed = "4.0.0")]
    XBlockDirtyBitmapMerge(BlockDirtyBitmapMerge),
}

#[qapi(added = "6.2.0")]
pub enum TransactionActionKind {
    #[qapi(name = "abort")]
    Abort,
    #[qapi(name = "block-dirty-bitmap-add")]
    BlockDirtyBitmapAdd,
    #[qapi(name = "block-dirty-bitmap-clear")]
    BlockDirtyBitmapClear,
    #[qapi(name = "block-dirty-bitmap-disable")]
    BlockDirtyBitmapDisable,
    #[qapi(name = "block-dirty-bitmap-enable")]
    BlockDirtyBitmapEnable,
    #[qapi(name = "block-dirty-bitmap-merge")]
    BlockDirtyBitmapMerge,
    #[qapi(name = "block-dirty-bitmap-remove")]
    BlockDirtyBitmapRemove,
    #[qapi(name = "blockdev-backup")]
    BlockdevBackup,
    #[qapi(name = "blockdev-snapshot")]
    BlockdevSnapshot,
    #[qapi(name = "blockdev-snapshot-internal-sync")]
    BlockdevSnapshotInternalSync,
    #[qapi(name = "blockdev-snapshot-sync")]
    BlockdevSnapshotSync,
    #[qapi(name = "drive-backup")]
    #[qapi(feature = "deprecated")]
    #[qapi(deprecated(added = "7.0.0"))]
    DriveBackup,
}

#[qapi(added = "1.6.0")]
#[qapi(removed = "6.2.0")]
pub enum TransactionActionType {
    #[qapi(name = "abort")]
    Abort,
    #[qapi(name = "block-dirty-bitmap-add")]
    #[qapi(added = "2.5.0")]
    BlockDirtyBitmapAdd,
    #[qapi(name = "block-dirty-bitmap-clear")]
    #[qapi(added = "2.5.0")]
    BlockDirtyBitmapClear,
    #[qapi(name = "block-dirty-bitmap-disable")]
    #[qapi(added = "4.0.0")]
    BlockDirtyBitmapDisable,
    #[qapi(name = "block-dirty-bitmap-enable")]
    #[qapi(added = "4.0.0")]
    BlockDirtyBitmapEnable,
    #[qapi(name = "block-dirty-bitmap-merge")]
    #[qapi(added = "4.0.0")]
    BlockDirtyBitmapMerge,
    #[qapi(name = "block-dirty-bitmap-remove")]
    #[qapi(added = "4.2.0")]
    BlockDirtyBitmapRemove,
    #[qapi(name = "blockdev-backup")]
    #[qapi(added = "2.3.0")]
    BlockdevBackup,
    #[qapi(name = "blockdev-snapshot")]
    #[qapi(added = "2.5.0")]
    BlockdevSnapshot,
    #[qapi(name = "blockdev-snapshot-internal-sync")]
    #[qapi(added = "1.7.0")]
    BlockdevSnapshotInternalSync,
    #[qapi(name = "blockdev-snapshot-sync")]
    BlockdevSnapshotSync,
    #[qapi(name = "drive-backup")]
    DriveBackup,
    #[qapi(name = "x-block-dirty-bitmap-disable")]
    #[qapi(added = "3.0.0")]
    #[qapi(removed = "4.0.0")]
    XBlockDirtyBitmapDisable,
    #[qapi(name = "x-block-dirty-bitmap-enable")]
    #[qapi(added = "3.0.0")]
    #[qapi(removed = "4.0.0")]
    XBlockDirtyBitmapEnable,
    #[qapi(name = "x-block-dirty-bitmap-merge")]
    #[qapi(added = "3.1.0")]
    #[qapi(removed = "4.0.0")]
    XBlockDirtyBitmapMerge,
}

/// Optional arguments to modify the behavior of a Transaction.
///
#[qapi(added = "2.5.0")]
pub struct TransactionProperties {
    /// Controls how jobs launched asynchronously by
    /// Actions will complete or fail as a group.  See
    /// `ActionCompletionMode` for details.
    #[qapi(name = "completion-mode")]
    pub completion_mode: Option<ActionCompletionMode>,
}

/// UEFI Variable.  Check the UEFI specification for more detailed
/// information on the fields.
///
#[qapi(added = "10.0.0")]
pub struct UefiVariable {
    /// variable attributes.
    pub attr: i64,
    /// variable value, encoded as hex string.
    pub data: String,
    /// variable certificate digest.  Used to verify the signature
    /// of updates for authenticated variables.  UEFI has two kinds of
    /// authenticated variables.  The secure boot variables ('PK',
    /// 'KEK', 'db' and 'dbx') have hard coded signature checking rules.
    /// For other authenticated variables the firmware stores a digest
    /// of the signing certificate at variable creation time, and any
    /// updates must be signed with the same certificate.
    pub digest: Option<String>,
    /// variable namespace GUID
    pub guid: String,
    /// variable name, in UTF-8 encoding.
    pub name: String,
    /// variable modification time.  EFI_TIME struct, encoded as hex
    /// string.  Used only for authenticated variables, where the
    /// EFI_VARIABLE_TIME_BASED_AUTHENTICATED_WRITE_ACCESS attribute bit
    /// is set.
    pub time: Option<String>,
}

#[qapi(added = "10.0.0")]
pub struct UefiVarStore {
    /// list of UEFI variables
    pub variables: Vec<UefiVariable>,
    /// currently always 2
    pub version: i64,
}

/// Captures a socket address in the local ("Unix socket") namespace.
///
#[qapi(added = "1.3.0")]
pub struct UnixSocketAddress {
    /// if true, this is a Linux abstract socket address.  @path
    /// will be prefixed by a null byte, and optionally padded with null
    /// bytes.  Defaults to false.  (Since 5.1)
    #[qapi(condition = "defined(CONFIG_LINUX)")]
    #[qapi(added = "5.1.0")]
    pub r#abstract: Option<bool>,
    /// filesystem path to use
    pub path: String,
    /// if false, pad an abstract socket address with enough null
    /// bytes to make it fill struct sockaddr_un member sun_path.
    /// Defaults to true.  (Since 5.1)
    #[qapi(condition = "defined(CONFIG_LINUX)")]
    #[qapi(added = "5.1.0")]
    pub tight: Option<bool>,
}

#[qapi(added = "6.2.0")]
pub struct UnixSocketAddressWrapper {
    /// UNIX domain socket address
    pub data: UnixSocketAddress,
}

#[qapi(added = "1.0.0")]
pub struct UuidInfo {
    #[qapi(name = "UUID")]
    pub uuid: String,
}

/// A description of QEMU's version.
///
#[qapi(added = "1.0.0")]
pub struct VersionInfo {
    /// QEMU will always set this field to an empty string.
    /// Downstream versions of QEMU should set this to a non-empty
    /// string.  The exact format depends on the downstream however it
    /// highly recommended that a unique name is used.
    pub package: String,
    /// The version of QEMU.  By current convention, a micro version
    /// of 50 signifies a development branch.  A micro version greater
    /// than or equal to 90 signifies a release candidate for the next
    /// minor version.  A micro version of less than 50 signifies a
    /// stable release.
    pub qemu: VersionTriple,
}

#[qapi(added = "1.0.0")]
#[qapi(removed = "2.4.0")]
pub struct VersionInfoQemu {
    pub major: i64,
    pub micro: i64,
    pub minor: i64,
}

/// A three-part version number.
///
#[qapi(added = "2.4.0")]
pub struct VersionTriple {
    /// The major version number.
    pub major: i64,
    /// The micro version number.
    pub micro: i64,
    /// The minor version number.
    pub minor: i64,
}

/// An enumeration of the VFIO device migration states.
///
#[qapi(added = "9.1.0")]
#[qapi(removed = "9.2.0")]
pub enum VfioMigrationState {
    /// The device is running, tracking its internal state and
    /// its internal state is available for reading.
    #[qapi(name = "pre-copy")]
    PreCopy,
    /// The device is running in the P2P quiescent state,
    /// tracking its internal state and its internal state is available
    /// for reading.
    #[qapi(name = "pre-copy-p2p")]
    PreCopyP2p,
    /// The device is stopped and its internal state is available
    /// for writing.
    #[qapi(name = "resuming")]
    Resuming,
    /// The device is running.
    #[qapi(name = "running")]
    Running,
    /// The device is running in the P2P quiescent state.
    #[qapi(name = "running-p2p")]
    RunningP2p,
    /// The device is stopped.
    #[qapi(name = "stop")]
    Stop,
    /// The device is stopped and its internal state is
    /// available for reading.
    #[qapi(name = "stop-copy")]
    StopCopy,
}

/// Detailed VFIO devices migration statistics
///
#[qapi(added = "5.2.0")]
pub struct VfioStats {
    /// amount of bytes transferred to the target VM by VFIO
    /// devices
    pub transferred: i64,
}

/// Properties for x-vfio-user-server objects.
///
#[qapi(added = "7.1.0")]
pub struct VfioUserServerProperties {
    /// the ID of the device to be emulated at the server
    pub device: String,
    /// socket to be used by the libvfio-user library
    pub socket: SocketAddress,
}

/// A structure defined to list the vhost user protocol features of a
/// Vhost User device
///
#[qapi(added = "7.2.0")]
pub struct VhostDeviceProtocols {
    /// List of decoded vhost user protocol features of a vhost
    /// user device
    pub protocols: Vec<String>,
    /// Vhost user device protocol features bitmap that
    /// have not been decoded
    #[qapi(name = "unknown-protocols")]
    pub unknown_protocols: Option<u64>,
}

/// Information about a vhost device.  This information will only be
/// displayed if the vhost device is active.
///
#[qapi(added = "7.2.0")]
pub struct VhostStatus {
    /// vhost_dev acked_features
    #[qapi(name = "acked-features")]
    pub acked_features: VirtioDeviceFeatures,
    /// vhost_dev backend_cap
    #[qapi(name = "backend-cap")]
    pub backend_cap: u64,
    /// vhost_dev backend_features
    #[qapi(name = "backend-features")]
    #[qapi(removed = "11.1.0")]
    pub backend_features: Option<VirtioDeviceFeatures>,
    /// vhost_dev features
    pub features: VirtioDeviceFeatures,
    /// vhost_dev log_enabled flag
    #[qapi(name = "log-enabled")]
    pub log_enabled: bool,
    /// vhost_dev log_size
    #[qapi(name = "log-size")]
    pub log_size: u64,
    /// vhost_dev max_queues
    #[qapi(name = "max-queues")]
    pub max_queues: u64,
    /// vhost_dev n_mem_sections
    #[qapi(name = "n-mem-sections")]
    pub n_mem_sections: i64,
    /// vhost_dev n_tmp_sections
    #[qapi(name = "n-tmp-sections")]
    pub n_tmp_sections: i64,
    /// vhost_dev nvqs (number of virtqueues being used)
    pub nvqs: u32,
    /// vhost_dev protocol_features
    #[qapi(name = "protocol-features")]
    pub protocol_features: VhostDeviceProtocols,
    /// vhost_dev vq_index
    #[qapi(name = "vq-index")]
    pub vq_index: i64,
}

/// The common fields that apply to most Virtio devices.  Some devices
/// may not have their own device-specific features (e.g. virtio-rng).
///
#[qapi(added = "7.2.0")]
pub struct VirtioDeviceFeatures {
    /// List of device-specific features (if the device has
    /// unique features)
    #[qapi(name = "dev-features")]
    pub dev_features: Option<Vec<String>>,
    /// List of transport features of the virtio device
    pub transports: Vec<String>,
    /// Virtio device features bitmap that have not
    /// been decoded (bits 0-63)
    #[qapi(name = "unknown-dev-features")]
    pub unknown_dev_features: Option<u64>,
    /// Virtio device features bitmap that have not
    /// been decoded (bits 64-127) (since 10.2)
    #[qapi(name = "unknown-dev-features2")]
    #[qapi(added = "10.2.0")]
    pub unknown_dev_features2: Option<u64>,
}

/// A structure defined to list the configuration statuses of a virtio
/// device
///
#[qapi(added = "7.2.0")]
pub struct VirtioDeviceStatus {
    /// List of decoded configuration statuses of the virtio
    /// device
    pub statuses: Vec<String>,
    /// Virtio device statuses bitmap that have not been
    /// decoded
    #[qapi(name = "unknown-statuses")]
    pub unknown_statuses: Option<u8>,
}

/// Describes configuration of a VirtIO GPU output.  If both @xres and
#[qapi(name = "VirtIOGPUOutput")]
#[qapi(added = "10.1.0")]
pub struct VirtIogpuOutput {
    /// the name of the output
    pub name: String,
    /// horizontal resolution of the output in pixels (since 11.0)
    #[qapi(added = "11.0.0")]
    pub xres: Option<u16>,
    /// vertical resolution of the output in pixels (since 11.0)
    #[qapi(added = "11.0.0")]
    pub yres: Option<u16>,
}

/// Basic information about a given VirtIODevice
///
#[qapi(added = "7.2.0")]
pub struct VirtioInfo {
    /// Name of the VirtIODevice
    pub name: String,
    /// The VirtIODevice's canonical QOM path
    pub path: String,
}

/// VirtioMEMDevice state information
///
#[qapi(name = "VirtioMEMDeviceInfo")]
#[qapi(added = "5.1.0")]
pub struct VirtioMemDeviceInfo {
    /// the block size of memory that the device provides
    #[qapi(name = "block-size")]
    pub block_size: u64,
    /// device's ID
    pub id: Option<String>,
    /// the maximum size of memory that the device can provide
    #[qapi(name = "max-size")]
    pub max_size: u64,
    /// physical address in memory, where device is mapped
    pub memaddr: u64,
    /// memory backend linked with the region
    pub memdev: String,
    /// NUMA node number where device is assigned to
    pub node: i64,
    /// the user requested size of the device
    #[qapi(name = "requested-size")]
    pub requested_size: u64,
    /// the (current) size of memory that the device provides
    pub size: u64,
}

#[qapi(name = "VirtioMEMDeviceInfoWrapper")]
#[qapi(added = "6.2.0")]
pub struct VirtioMemDeviceInfoWrapper {
    /// VirtioMEMDevice state information
    pub data: VirtioMemDeviceInfo,
}

/// VirtioPMEM state information
///
#[qapi(name = "VirtioPMEMDeviceInfo")]
#[qapi(added = "4.1.0")]
pub struct VirtioPmemDeviceInfo {
    /// device's ID
    pub id: Option<String>,
    /// physical address in memory, where device is mapped
    pub memaddr: u64,
    /// memory backend linked with device
    pub memdev: String,
    /// size of memory that the device provides
    pub size: u64,
}

#[qapi(name = "VirtioPMEMDeviceInfoWrapper")]
#[qapi(added = "6.2.0")]
pub struct VirtioPmemDeviceInfoWrapper {
    /// VirtioPMEM state information
    pub data: VirtioPmemDeviceInfo,
}

/// Information regarding a VirtQueue's VirtQueueElement including
/// descriptor, driver, and device areas
///
#[qapi(added = "7.2.0")]
pub struct VirtioQueueElement {
    /// VRingAvail info
    pub avail: VirtioRingAvail,
    /// List of descriptors (`VirtioRingDesc`)
    pub descs: Vec<VirtioRingDesc>,
    /// Index of the element in the queue
    pub index: u32,
    /// Name of the VirtIODevice that uses this VirtQueue
    pub name: String,
    /// VRingUsed info
    pub used: VirtioRingUsed,
}

/// Information regarding the avail vring (a.k.a. driver area)
///
#[qapi(added = "7.2.0")]
pub struct VirtioRingAvail {
    /// VRingAvail flags
    pub flags: u16,
    /// VRingAvail index
    pub idx: u16,
    /// VRingAvail ring[] entry at provided index
    pub ring: u16,
}

/// Information regarding the vring descriptor area
///
#[qapi(added = "7.2.0")]
pub struct VirtioRingDesc {
    /// Guest physical address of the descriptor area
    pub addr: u64,
    /// List of descriptor flags
    pub flags: Vec<String>,
    /// Length of the descriptor area
    pub len: u32,
}

/// Information regarding the used vring (a.k.a. device area)
///
#[qapi(added = "7.2.0")]
pub struct VirtioRingUsed {
    /// VRingUsed flags
    pub flags: u16,
    /// VRingUsed index
    pub idx: u16,
}

/// Full status of the virtio device with most VirtIODevice members.
/// Also includes the full status of the corresponding vhost device if
/// the vhost device is active.
///
#[qapi(added = "7.2.0")]
pub struct VirtioStatus {
    /// VirtIODevice backend_features
    #[qapi(name = "backend-features")]
    pub backend_features: VirtioDeviceFeatures,
    /// VirtIODevice broken flag
    pub broken: bool,
    /// VirtIODevice bus_name
    #[qapi(name = "bus-name")]
    pub bus_name: String,
    /// VirtIODevice device_endian
    #[qapi(name = "device-endian")]
    pub device_endian: String,
    /// VirtIODevice ID
    #[qapi(name = "device-id")]
    pub device_id: u16,
    /// VirtIODevice disabled_legacy_check flag
    #[qapi(name = "disable-legacy-check")]
    pub disable_legacy_check: bool,
    /// VirtIODevice disabled flag
    pub disabled: bool,
    /// VirtIODevice guest_features
    #[qapi(name = "guest-features")]
    pub guest_features: VirtioDeviceFeatures,
    /// VirtIODevice host_features
    #[qapi(name = "host-features")]
    pub host_features: VirtioDeviceFeatures,
    /// VirtIODevice ISR
    pub isr: u8,
    /// VirtIODevice name
    pub name: String,
    /// VirtIODevice virtqueue count.  This is the number of
    /// active virtqueues being used by the VirtIODevice.
    #[qapi(name = "num-vqs")]
    pub num_vqs: i64,
    /// VirtIODevice queue_sel
    #[qapi(name = "queue-sel")]
    pub queue_sel: u16,
    /// VirtIODevice start_on_kick flag
    #[qapi(name = "start-on-kick")]
    pub start_on_kick: bool,
    /// VirtIODevice started flag
    pub started: bool,
    /// VirtIODevice configuration status (`VirtioDeviceStatus`)
    pub status: VirtioDeviceStatus,
    /// VirtIODevice use_guest_notifier_mask flag
    #[qapi(name = "use-guest-notifier-mask")]
    pub use_guest_notifier_mask: bool,
    /// VirtIODevice use_started flag
    #[qapi(name = "use-started")]
    pub use_started: bool,
    /// Corresponding vhost device info for a given
    /// VirtIODevice.  Present if the given VirtIODevice has an active
    /// vhost device.
    #[qapi(name = "vhost-dev")]
    pub vhost_dev: Option<VhostStatus>,
    /// VirtIODevice vhost_started flag
    #[qapi(name = "vhost-started")]
    pub vhost_started: bool,
    /// VirtIODevice vm_running flag
    #[qapi(name = "vm-running")]
    pub vm_running: bool,
}

/// Information of a VirtIODevice VirtQueue, including most members of
/// the VirtQueue data structure.
///
#[qapi(added = "7.2.0")]
pub struct VirtQueueStatus {
    /// VirtQueue inuse
    pub inuse: u32,
    /// VirtQueue last_avail_idx or return of vhost_dev
    /// vhost_get_vring_base (if vhost active)
    #[qapi(name = "last-avail-idx")]
    pub last_avail_idx: Option<u16>,
    /// Name of the VirtIODevice that uses this VirtQueue
    pub name: String,
    /// VirtQueue queue_index
    #[qapi(name = "queue-index")]
    pub queue_index: u16,
    /// VirtQueue shadow_avail_idx
    #[qapi(name = "shadow-avail-idx")]
    pub shadow_avail_idx: Option<u16>,
    /// VirtQueue signalled_used
    #[qapi(name = "signalled-used")]
    pub signalled_used: u16,
    /// VirtQueue signalled_used_valid flag
    #[qapi(name = "signalled-used-valid")]
    pub signalled_used_valid: bool,
    /// VirtQueue used_idx
    #[qapi(name = "used-idx")]
    pub used_idx: u16,
    /// VirtQueue vring.align
    #[qapi(name = "vring-align")]
    pub vring_align: u32,
    /// VirtQueue vring.avail (driver area)
    #[qapi(name = "vring-avail")]
    pub vring_avail: u64,
    /// VirtQueue vring.desc (descriptor area)
    #[qapi(name = "vring-desc")]
    pub vring_desc: u64,
    /// VirtQueue vring.num
    #[qapi(name = "vring-num")]
    pub vring_num: u32,
    /// VirtQueue vring.num_default
    #[qapi(name = "vring-num-default")]
    pub vring_num_default: u32,
    /// VirtQueue vring.used (device area)
    #[qapi(name = "vring-used")]
    pub vring_used: u64,
}

/// Information of a vhost device's vhost_virtqueue, including most
/// members of the vhost_dev vhost_virtqueue data structure.
///
#[qapi(added = "7.2.0")]
pub struct VirtVhostQueueStatus {
    /// vhost_virtqueue avail
    #[qapi(removed = "11.1.0")]
    pub avail: Option<u64>,
    /// vhost_virtqueue avail_phys (driver area physical
    /// address)
    #[qapi(name = "avail-phys")]
    pub avail_phys: u64,
    /// vhost_virtqueue avail_size
    #[qapi(name = "avail-size")]
    pub avail_size: u32,
    /// vhost_virtqueue call
    pub call: i64,
    /// vhost_virtqueue desc
    #[qapi(removed = "11.1.0")]
    pub desc: Option<u64>,
    /// vhost_virtqueue desc_phys (descriptor area physical
    /// address)
    #[qapi(name = "desc-phys")]
    pub desc_phys: u64,
    /// vhost_virtqueue desc_size
    #[qapi(name = "desc-size")]
    pub desc_size: u32,
    /// vhost_virtqueue kick
    pub kick: i64,
    /// Name of the VirtIODevice that uses this vhost_virtqueue
    pub name: String,
    /// vhost_virtqueue num
    pub num: i64,
    /// vhost_virtqueue used
    #[qapi(removed = "11.1.0")]
    pub used: Option<u64>,
    /// vhost_virtqueue used_phys (device area physical address)
    #[qapi(name = "used-phys")]
    pub used_phys: u64,
    /// vhost_virtqueue used_size
    #[qapi(name = "used-size")]
    pub used_size: u32,
}

#[qapi(name = "VMAppleVirtioBlkVariant")]
#[qapi(added = "10.0.0")]
pub enum VmAppleVirtioBlkVariant {
    /// Block device holding auxiliary data required for boot
    #[qapi(name = "aux")]
    Aux,
    /// Block device holding the root volume
    #[qapi(name = "root")]
    Root,
    /// The default, not a valid setting.
    #[qapi(name = "unspecified")]
    Unspecified,
}

/// Information about a VMDK extent file
///
#[qapi(added = "8.0.0")]
pub struct VmdkExtentInfo {
    /// Cluster size in bytes (for non-flat extents)
    #[qapi(name = "cluster-size")]
    pub cluster_size: Option<i64>,
    /// Whether this extent contains compressed data
    pub compressed: Option<bool>,
    /// Name of the extent file
    pub filename: String,
    /// Extent type (e.g. FLAT or SPARSE)
    pub format: String,
    /// Number of bytes covered by this extent
    #[qapi(name = "virtual-size")]
    pub virtual_size: i64,
}

/// The basic information for vnc network connection
///
#[qapi(condition = "defined(CONFIG_VNC)")]
#[qapi(added = "2.1.0")]
pub struct VncBasicInfo {
    /// address family
    pub family: NetworkAddressFamily,
    /// IP address
    pub host: String,
    /// The service name of the vnc port.  This may depend on the
    /// host system's service database so symbolic names should not be
    /// relied on.
    pub service: String,
    /// true in case the socket is a websocket (since 2.3).
    #[qapi(added = "2.3.0")]
    pub websocket: Option<bool>,
}

/// Information about a connected VNC client.
///
#[qapi(condition = "defined(CONFIG_VNC)")]
#[qapi(added = "1.0.0")]
pub struct VncClientInfo {
    /// address family
    pub family: NetworkAddressFamily,
    /// IP address
    pub host: String,
    /// If SASL authentication is in use, the SASL username
    /// used for authentication.
    pub sasl_username: Option<String>,
    /// The service name of the vnc port.  This may depend on the
    /// host system's service database so symbolic names should not be
    /// relied on.
    pub service: String,
    /// true in case the socket is a websocket (since 2.3).
    #[qapi(added = "2.3.0")]
    pub websocket: Option<bool>,
    /// If x509 authentication is in use, the Distinguished
    /// Name of the client.
    pub x509_dname: Option<String>,
}

/// Information about the VNC session.
///
#[qapi(condition = "defined(CONFIG_VNC)")]
#[qapi(added = "1.0.0")]
pub struct VncInfo {
    /// the current authentication type used by the server
    ///
    /// - 'none' if no authentication is being used
    /// - 'vnc' if VNC authentication is being used
    /// - 'vencrypt+plain' if VEncrypt is used with plain text
    /// authentication
    /// - 'vencrypt+tls+none' if VEncrypt is used with TLS and no
    /// authentication
    /// - 'vencrypt+tls+vnc' if VEncrypt is used with TLS and VNC
    /// authentication
    /// - 'vencrypt+tls+plain' if VEncrypt is used with TLS and plain
    /// text auth
    /// - 'vencrypt+x509+none' if VEncrypt is used with x509 and no auth
    /// - 'vencrypt+x509+vnc' if VEncrypt is used with x509 and VNC auth
    /// - 'vencrypt+x509+plain' if VEncrypt is used with x509 and plain
    /// text auth
    /// - 'vencrypt+tls+sasl' if VEncrypt is used with TLS and SASL auth
    /// - 'vencrypt+x509+sasl' if VEncrypt is used with x509 and SASL
    /// auth
    pub auth: Option<String>,
    /// a list of `VncClientInfo` of all currently connected
    /// clients
    pub clients: Option<Vec<VncClientInfo>>,
    /// true if the VNC server is enabled, false otherwise
    pub enabled: bool,
    /// - 'ipv6' if the host is listening for IPv6 connections
    /// - 'ipv4' if the host is listening for IPv4 connections
    /// - 'unix' if the host is listening on a unix domain socket
    /// - 'unknown' otherwise
    pub family: Option<NetworkAddressFamily>,
    /// The hostname the VNC server is bound to.  This depends on the
    /// name resolution on the host and may be an IP address.
    pub host: Option<String>,
    /// The service name of the server's port.  This may depends
    /// on the host system's service database so symbolic names should
    /// not be relied on.
    pub service: Option<String>,
}

/// Information about a vnc server
///
#[qapi(condition = "defined(CONFIG_VNC)")]
#[qapi(added = "2.3.0")]
pub struct VncInfo2 {
    /// The current authentication type used by the non-websockets
    /// servers
    pub auth: VncPrimaryAuth,
    /// A list of `VncClientInfo` of all currently connected
    /// clients.  The list can be empty, for obvious reasons.
    pub clients: Vec<VncClientInfo>,
    /// The display device the vnc server is linked to.
    pub display: Option<String>,
    /// vnc server name.
    pub id: String,
    /// A list of `VncBasicInfo` describing all listening sockets.
    /// The list can be empty (in case the vnc server is disabled).  It
    /// also may have multiple entries: normal + websocket, possibly
    /// also ipv4 + ipv6 in the future.
    pub server: Vec<VncServerInfo2>,
    /// The vencrypt authentication type used by the servers,
    /// only specified in case auth == vencrypt.
    pub vencrypt: Option<VncVencryptSubAuth>,
}

/// vnc primary authentication method.
///
#[qapi(condition = "defined(CONFIG_VNC)")]
#[qapi(added = "2.3.0")]
pub enum VncPrimaryAuth {
    #[qapi(name = "none")]
    None,
    #[qapi(name = "ra2")]
    Ra2,
    #[qapi(name = "ra2ne")]
    Ra2ne,
    #[qapi(name = "sasl")]
    Sasl,
    #[qapi(name = "tight")]
    Tight,
    #[qapi(name = "tls")]
    Tls,
    #[qapi(name = "ultra")]
    Ultra,
    #[qapi(name = "vencrypt")]
    Vencrypt,
    #[qapi(name = "vnc")]
    Vnc,
}

/// The network connection information for server
///
#[qapi(condition = "defined(CONFIG_VNC)")]
#[qapi(added = "2.1.0")]
pub struct VncServerInfo {
    /// authentication method used for the plain (non-websocket) VNC
    /// server
    pub auth: Option<String>,
    /// address family
    pub family: NetworkAddressFamily,
    /// IP address
    pub host: String,
    /// The service name of the vnc port.  This may depend on the
    /// host system's service database so symbolic names should not be
    /// relied on.
    pub service: String,
    /// true in case the socket is a websocket (since 2.3).
    #[qapi(added = "2.3.0")]
    pub websocket: Option<bool>,
}

/// The network connection information for server
///
#[qapi(condition = "defined(CONFIG_VNC)")]
#[qapi(added = "2.9.0")]
pub struct VncServerInfo2 {
    /// The current authentication type used by the servers
    pub auth: VncPrimaryAuth,
    /// address family
    pub family: NetworkAddressFamily,
    /// IP address
    pub host: String,
    /// The service name of the vnc port.  This may depend on the
    /// host system's service database so symbolic names should not be
    /// relied on.
    pub service: String,
    /// The vencrypt sub authentication type used by the servers,
    /// only specified in case auth == vencrypt.
    pub vencrypt: Option<VncVencryptSubAuth>,
    /// true in case the socket is a websocket (since 2.3).
    pub websocket: bool,
}

/// vnc sub authentication method with vencrypt.
///
#[qapi(condition = "defined(CONFIG_VNC)")]
#[qapi(added = "2.3.0")]
pub enum VncVencryptSubAuth {
    #[qapi(name = "plain")]
    Plain,
    #[qapi(name = "tls-none")]
    TlsNone,
    #[qapi(name = "tls-plain")]
    TlsPlain,
    #[qapi(name = "tls-sasl")]
    TlsSasl,
    #[qapi(name = "tls-vnc")]
    TlsVnc,
    #[qapi(name = "x509-none")]
    X509None,
    #[qapi(name = "x509-plain")]
    X509Plain,
    #[qapi(name = "x509-sasl")]
    X509Sasl,
    #[qapi(name = "x509-vnc")]
    X509Vnc,
}

/// Captures a socket address in the vsock namespace.
///
#[qapi(added = "2.8.0")]
pub struct VsockSocketAddress {
    /// unique host identifier
    pub cid: String,
    /// port
    ///
    /// .. note:: String types are used to allow for possible future
    /// hostname or service resolution support.
    pub port: String,
}

#[qapi(added = "6.2.0")]
pub struct VsockSocketAddressWrapper {
    /// VSOCK domain socket address
    pub data: VsockSocketAddress,
}

/// An enumeration of the actions taken when the watchdog device's timer
/// is expired
///
#[qapi(added = "2.11.0")]
pub enum WatchdogAction {
    /// system enters debug state
    #[qapi(name = "debug")]
    Debug,
    /// a non-maskable interrupt is injected into the first
    /// VCPU (all VCPUS on x86) (since 2.4)
    #[qapi(name = "inject-nmi")]
    InjectNmi,
    /// nothing is done
    #[qapi(name = "none")]
    None,
    /// system pauses, similar to @stop
    #[qapi(name = "pause")]
    Pause,
    /// system poweroff, the emulator program exits
    #[qapi(name = "poweroff")]
    Poweroff,
    /// system resets
    #[qapi(name = "reset")]
    Reset,
    /// system shutdown, note that it is similar to @powerdown,
    /// which tries to set to system status and notify guest
    #[qapi(name = "shutdown")]
    Shutdown,
}

/// An enumeration of the actions taken when the watchdog device's timer is
/// expired
///
#[qapi(added = "2.1.0")]
#[qapi(removed = "2.11.0")]
pub enum WatchdogExpirationAction {
    /// system enters debug state
    #[qapi(name = "debug")]
    Debug,
    /// a non-maskable interrupt is injected into the first VCPU (all
    /// VCPUS on x86) (since 2.4)
    #[qapi(name = "inject-nmi")]
    #[qapi(added = "2.4.0")]
    InjectNmi,
    /// nothing is done
    #[qapi(name = "none")]
    None,
    /// system pauses, similar to @stop
    #[qapi(name = "pause")]
    Pause,
    /// system poweroff, the emulator program exits
    #[qapi(name = "poweroff")]
    Poweroff,
    /// system resets
    #[qapi(name = "reset")]
    Reset,
    /// system shutdown, note that it is similar to @powerdown, which
    /// tries to set to system status and notify guest
    #[qapi(name = "shutdown")]
    Shutdown,
}

/// Information about a X86 CPU feature word
///
#[qapi(name = "X86CPUFeatureWordInfo")]
#[qapi(added = "1.5.0")]
pub struct X86cpuFeatureWordInfo {
    /// Input EAX value for CPUID instruction for that
    /// feature word
    #[qapi(name = "cpuid-input-eax")]
    pub cpuid_input_eax: i64,
    /// Input ECX value for CPUID instruction for that
    /// feature word
    #[qapi(name = "cpuid-input-ecx")]
    pub cpuid_input_ecx: Option<i64>,
    /// Output register containing the feature bits
    #[qapi(name = "cpuid-register")]
    pub cpuid_register: X86cpuRegister32,
    /// value of output register, containing the feature bits
    pub features: i64,
}

/// A X86 32-bit register
///
#[qapi(name = "X86CPURegister32")]
#[qapi(added = "1.5.0")]
pub enum X86cpuRegister32 {
    #[qapi(name = "EAX")]
    Eax,
    #[qapi(name = "EBP")]
    Ebp,
    #[qapi(name = "EBX")]
    Ebx,
    #[qapi(name = "ECX")]
    Ecx,
    #[qapi(name = "EDI")]
    Edi,
    #[qapi(name = "EDX")]
    Edx,
    #[qapi(name = "ESI")]
    Esi,
    #[qapi(name = "ESP")]
    Esp,
}

/// Detailed XBZRLE migration cache statistics
///
#[qapi(name = "XBZRLECacheStats")]
#[qapi(added = "1.2.0")]
pub struct XbzrleCacheStats {
    /// amount of bytes already transferred to the target VM
    pub bytes: i64,
    /// number of cache miss
    #[qapi(name = "cache-miss")]
    pub cache_miss: i64,
    /// rate of cache miss (since 2.1)
    #[qapi(name = "cache-miss-rate")]
    #[qapi(added = "2.1.0")]
    pub cache_miss_rate: Option<f64>,
    /// XBZRLE cache size
    #[qapi(name = "cache-size")]
    pub cache_size: u64,
    /// rate of encoded bytes (since 5.1)
    #[qapi(name = "encoding-rate")]
    #[qapi(added = "5.1.0")]
    pub encoding_rate: Option<f64>,
    /// number of overflows
    pub overflow: i64,
    /// amount of pages transferred to the target VM
    pub pages: i64,
}

/// Block Graph - list of nodes and list of edges.
///
#[qapi(added = "4.0.0")]
pub struct XDbgBlockGraph {
    pub edges: Vec<XDbgBlockGraphEdge>,
    pub nodes: Vec<XDbgBlockGraphNode>,
}

/// Block Graph edge description for `x-debug-query-block-graph`.
///
#[qapi(added = "4.0.0")]
pub struct XDbgBlockGraphEdge {
    /// child id
    pub child: u64,
    /// name of the relation (examples are 'file' and 'backing')
    pub name: String,
    /// parent id
    pub parent: u64,
    /// granted permissions for the parent operating on the child
    pub perm: Vec<BlockPermission>,
    /// permissions that can still be granted to other users
    /// of the child while it is still attached to this parent
    #[qapi(name = "shared-perm")]
    pub shared_perm: Vec<BlockPermission>,
}

#[qapi(added = "4.0.0")]
pub struct XDbgBlockGraphNode {
    /// Block graph node identifier.  This @id is generated only for
    /// `x-debug-query-block-graph` and does not relate to any other
    /// identifiers in QEMU.
    pub id: u64,
    /// Human readable name of the node.  Corresponds to node-name
    /// for block-driver-state nodes; is not guaranteed to be unique in
    /// the whole graph (with block-jobs and block-backends).
    pub name: String,
    /// Type of graph node.  Can be one of block-backend, block-job
    /// or block-driver-state.
    pub r#type: XDbgBlockGraphNodeType,
}

#[qapi(added = "4.0.0")]
pub enum XDbgBlockGraphNodeType {
    /// corresponds to BlockBackend
    #[qapi(name = "block-backend")]
    BlockBackend,
    /// corresponds to BlockDriverState
    #[qapi(name = "block-driver")]
    BlockDriver,
    /// corresponds to BlockJob
    #[qapi(name = "block-job")]
    BlockJob,
}

/// A yank instance can be yanked with the `yank` qmp command to recover
/// from a hanging QEMU.
///
#[qapi(added = "6.0.0")]
pub struct YankInstance {
    /// yank instance type
    ///
    /// Currently implemented yank instances:
    ///
    /// - nbd block device: Yanking it will shut down the connection to the
    /// nbd server without attempting to reconnect.
    /// - socket chardev: Yanking it will shut down the connected socket.
    /// - migration: Yanking it will shut down all migration connections.
    /// Unlike `migrate_cancel`, it will not notify the migration process,
    /// so migration will go into @failed state, instead of @cancelled
    /// state.  `yank` should be used to recover from hangs.
    #[qapi(discriminator)]
    pub r#type: YankInstanceType,
    #[qapi(union)]
    #[qapi(flatten)]
    pub u: Option<YankInstanceBranch>,
}

/// Specifies which block graph node to yank.  See `YankInstance` for
/// more information.
///
#[qapi(added = "6.0.0")]
pub struct YankInstanceBlockNode {
    /// the name of the block graph node
    #[qapi(name = "node-name")]
    pub node_name: String,
}

#[qapi(added = "6.0.0")]
pub enum YankInstanceBranch {
    #[qapi(name = "block-node")]
    BlockNode(YankInstanceBlockNode),
    #[qapi(name = "chardev")]
    Chardev(YankInstanceChardev),
    #[qapi(name = "migration")]
    Migration,
}

/// Specifies which character device to yank.  See `YankInstance` for
/// more information.
///
#[qapi(added = "6.0.0")]
pub struct YankInstanceChardev {
    /// the chardev's ID
    pub id: String,
}

/// An enumeration of yank instance types.  See `YankInstance` for more
/// information.
///
#[qapi(added = "6.0.0")]
pub enum YankInstanceType {
    #[qapi(name = "block-node")]
    BlockNode,
    #[qapi(name = "chardev")]
    Chardev,
    #[qapi(name = "migration")]
    Migration,
}

#[qapi(added = "9.0.0")]
pub enum ZeroPageDetection {
    /// Perform zero page checking in main migration thread.
    #[qapi(name = "legacy")]
    Legacy,
    /// Perform zero page checking in multifd sender thread if
    /// multifd migration is enabled, else in the main migration thread
    /// as for @legacy.
    #[qapi(name = "multifd")]
    Multifd,
    /// Do not perform zero page checking.
    #[qapi(name = "none")]
    None,
}

pub mod commands;
pub mod events;
pub mod metadata;

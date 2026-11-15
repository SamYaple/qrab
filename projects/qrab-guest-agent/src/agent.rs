//! Command dispatch and the filesystem freeze state machine.
use crate::{
    boot::{QrabBootNext, QrabBootState},
    framing::{Frame, Framer},
    proto::{
        self, QrabFirmwareState, QrabNetworkGetInterfaces, QrabUpdateStatus, QrabUpdateTrigger,
        SCHEMA_VERSION,
    },
    secure_boot::{QrabSecureBootDatabases, QrabSecureBootUpdate},
    sys::{Mount, Platform, ShutdownMode},
    update::{HookPolicy, Updater},
};
use qapi::{
    qga::{
        commands::{
            GuestFsfreezeFreeze, GuestFsfreezeFreezeList, GuestFsfreezeStatusCommand,
            GuestFsfreezeThaw, GuestFstrim, GuestGetHostName, GuestGetOsinfo, GuestGetTime,
            GuestInfo, GuestNetworkGetInterfaces, GuestPing, GuestShutdown, GuestSync,
            GuestSyncDelimited,
        },
        GuestAgentCommandInfo, GuestAgentInfo, GuestFilesystemTrimResponse,
        GuestFilesystemTrimResult, GuestFsfreezeStatus, GuestHostName, GuestOsInfo,
    },
    QapiCommand,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Map, Value};
use std::{
    io::{self, Read, Write},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

/// Requests are small; anything larger is hostile or corrupt.
pub const MAX_REQUEST_BYTES: usize = 384 * 1024;
const MAX_FREEZE_MOUNTPOINTS: usize = 256;
/// FIFREEZE nests; repeat FITHAW a bounded number of times per filesystem.
const MAX_THAW_REPEATS: usize = 16;

#[derive(Debug, Clone)]
pub struct Config {
    /// Filesystems are thawed automatically if the host does not thaw them
    /// within this bound, so a lost host connection cannot wedge the guest.
    pub freeze_timeout: Duration,
    pub update: HookPolicy,
    pub os_release: Vec<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            freeze_timeout: Duration::from_secs(120),
            update: HookPolicy {
                path: proto::UPDATE_HOOK.into(),
                require_root: true,
                timeout: Duration::from_secs(3600),
            },
            os_release: vec!["/etc/os-release".into(), "/usr/lib/os-release".into()],
        }
    }
}

#[derive(Debug)]
pub struct QgaError {
    pub class: &'static str,
    pub desc: String,
}

impl QgaError {
    fn generic(desc: impl Into<String>) -> Self {
        Self {
            class: "GenericError",
            desc: desc.into(),
        }
    }
}

type Handler = fn(&Agent, Value) -> Result<Value, QgaError>;

struct Spec {
    name: &'static str,
    success_response: bool,
    args: &'static [&'static str],
    /// QEMU's freeze allowlist plus the read-only update status.
    while_frozen: bool,
    handler: Handler,
}

macro_rules! spec {
    ($command:ty, [$($arg:literal),*], $frozen:expr, $handler:expr) => {
        Spec {
            name: <$command as QapiCommand>::NAME,
            success_response: <$command as QapiCommand>::SUCCESS_RESPONSE,
            args: &[$($arg),*],
            while_frozen: $frozen,
            handler: $handler,
        }
    };
}

/// Everything served. Arbitrary execution, file access, user and network
/// management commands are deliberately absent.
static COMMANDS: &[Spec] = &[
    spec!(GuestSync, ["id"], true, |_, a| typed(a, |c: GuestSync| Ok(
        c.id
    ))),
    spec!(GuestSyncDelimited, ["id"], true, |_, a| {
        typed(a, |c: GuestSyncDelimited| Ok(c.id))
    }),
    spec!(GuestPing, [], true, |_, _| Ok(json!({}))),
    spec!(
        QrabSecureBootUpdate,
        ["database-id", "payload-hex", "append"],
        false,
        |agent, a| {
            typed(a, |request: QrabSecureBootUpdate| {
                crate::secure_boot::validate_update(&request)
                    .map_err(|e| QgaError::generic(e.to_string()))?;
                agent
                    .platform
                    .secure_boot_update(request)
                    .map_err(|e| QgaError::generic(e.to_string()))
            })
        }
    ),
    spec!(QrabSecureBootDatabases, [], false, |agent, _| {
        encode(
            &agent
                .platform
                .secure_boot_databases()
                .map_err(|e| QgaError::generic(e.to_string()))?,
        )
    }),
    spec!(QrabBootState, [], false, |agent, _| {
        encode(
            &agent
                .platform
                .boot_state()
                .map_err(|e| QgaError::generic(e.to_string()))?,
        )
    }),
    spec!(QrabBootNext, ["reference"], false, |agent, a| {
        typed(a, |c: QrabBootNext| {
            agent
                .platform
                .boot_next(c.reference)
                .map_err(|e| QgaError::generic(e.to_string()))
        })
    }),
    spec!(QrabFirmwareState, [], false, |agent, _| {
        encode(
            &agent
                .platform
                .firmware_state()
                .map_err(|e| QgaError::generic(e.to_string()))?,
        )
    }),
    spec!(GuestNetworkGetInterfaces, [], false, |agent, _| {
        let items = agent
            .platform
            .network_interfaces()
            .map_err(|e| QgaError::generic(e.to_string()))?;
        let items: Vec<_> = items
            .into_iter()
            .take(proto::MAX_NETWORK_INTERFACES)
            .map(|mut i| {
                i.ip_addresses.truncate(proto::MAX_NETWORK_ADDRESSES);
                i.qga()
            })
            .collect();
        encode(&items)
    }),
    spec!(QrabNetworkGetInterfaces, [], false, |agent, _| {
        let mut items = agent
            .platform
            .network_interfaces()
            .map_err(|e| QgaError::generic(e.to_string()))?;
        items.truncate(proto::MAX_NETWORK_INTERFACES);
        for item in &mut items {
            item.ip_addresses.truncate(proto::MAX_NETWORK_ADDRESSES);
        }
        encode(&items)
    }),
    spec!(GuestInfo, [], true, |agent, _| encode(&agent.info())),
    spec!(GuestGetOsinfo, [], false, |agent, _| encode(
        &agent.osinfo()
    )),
    spec!(GuestGetHostName, [], false, |_, _| encode(&host_name())),
    spec!(GuestGetTime, [], false, |_, _| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| QgaError::generic(e.to_string()))?;
        Ok(json!(now.as_nanos() as i64))
    }),
    spec!(GuestFsfreezeStatusCommand, [], true, |agent, _| {
        encode(&agent.freeze_status())
    }),
    spec!(GuestFsfreezeFreeze, [], false, |agent, _| Ok(json!(
        agent.freeze(None)?
    ))),
    spec!(
        GuestFsfreezeFreezeList,
        ["mountpoints"],
        false,
        |agent, a| { typed(a, |c: GuestFsfreezeFreezeList| agent.freeze(c.mountpoints)) }
    ),
    spec!(GuestFsfreezeThaw, [], true, |agent, _| Ok(json!(
        agent.thaw()?
    ))),
    spec!(GuestFstrim, ["minimum"], false, |agent, a| {
        typed(a, |c: GuestFstrim| agent.fstrim(c.minimum))
    }),
    spec!(GuestShutdown, ["mode"], false, |agent, a| {
        typed(a, |c: GuestShutdown| agent.shutdown(c.mode))
    }),
    spec!(QrabUpdateTrigger, ["args"], false, |agent, a| {
        typed(a, |c: QrabUpdateTrigger| {
            agent.updater.trigger(c.args).map_err(QgaError::generic)
        })
    }),
    spec!(QrabUpdateStatus, [], true, |agent, _| encode(
        &agent.updater.status()
    )),
];

fn encode<T: Serialize>(value: &T) -> Result<Value, QgaError> {
    qapi::with_version(SCHEMA_VERSION, || serde_json::to_value(value))
        .map_err(|e| QgaError::generic(format!("cannot encode reply: {e}")))
}

/// Replies are typed by the command's generated `Returns`, so the agent
/// cannot drift from the schema clients decode with.
fn typed<C: QapiCommand + DeserializeOwned>(
    args: Value,
    run: impl FnOnce(C) -> Result<C::Returns, QgaError>,
) -> Result<Value, QgaError>
where
    C::Returns: Serialize,
{
    let command = qapi::with_version(SCHEMA_VERSION, || serde_json::from_value(args))
        .map_err(|e| QgaError::generic(format!("Invalid parameter: {e}")))?;
    encode(&run(command)?)
}

fn host_name() -> GuestHostName {
    let host_name = std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_owned())
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| crate::sys::uname().map(|u| u.0))
        .unwrap_or_else(|| "localhost".into());
    GuestHostName { host_name }
}

#[derive(Default)]
struct FreezeState {
    frozen: bool,
    generation: u64,
    deadline: Option<Instant>,
}

struct Shared {
    freeze: Mutex<FreezeState>,
    wake: Condvar,
}

pub struct Agent {
    config: Config,
    platform: Arc<dyn Platform>,
    shared: Arc<Shared>,
    updater: Updater,
}

impl Agent {
    /// Starts the freeze watchdog thread. Any filesystem left frozen by a
    /// previous agent instance is thawed first: no watchdog would cover it.
    pub fn new(config: Config, platform: Arc<dyn Platform>) -> Arc<Self> {
        let agent = Arc::new(Self {
            updater: Updater::new(config.update.clone()),
            config,
            platform,
            shared: Arc::new(Shared {
                freeze: Mutex::new(FreezeState::default()),
                wake: Condvar::new(),
            }),
        });
        let thawed = thaw_all(agent.platform.as_ref());
        if thawed > 0 {
            eprintln!("qrab-guest-agent: thawed {thawed} filesystem(s) left frozen at startup");
        }
        let weak = Arc::downgrade(&agent);
        let shared = agent.shared.clone();
        std::thread::spawn(move || watchdog(&shared, &weak));
        agent
    }

    fn info(&self) -> GuestAgentInfo {
        let frozen = self.shared.freeze.lock().unwrap().frozen;
        let hook = self.updater.hook_ready().is_ok();
        GuestAgentInfo {
            version: proto::AGENT_VERSION.into(),
            supported_commands: Some(
                COMMANDS
                    .iter()
                    .map(|spec| GuestAgentCommandInfo {
                        name: spec.name.into(),
                        enabled: self.disabled_reason(spec, frozen, hook).is_none(),
                        success_response: Some(spec.success_response),
                    })
                    .collect(),
            ),
        }
    }

    fn disabled_reason(&self, spec: &Spec, frozen: bool, hook: bool) -> Option<&'static str> {
        if frozen && !spec.while_frozen {
            Some("the agent is in frozen state")
        } else if spec.name == QrabUpdateTrigger::NAME && !hook {
            Some("no valid update hook is installed")
        } else {
            None
        }
    }

    fn osinfo(&self) -> GuestOsInfo {
        let release = self
            .config
            .os_release
            .iter()
            .find_map(|p| std::fs::read_to_string(p).ok())
            .map(|t| crate::sys::parse_os_release(&t))
            .unwrap_or_default();
        let get = |key: &str| release.get(key).cloned();
        let uname = crate::sys::uname();
        GuestOsInfo {
            id: get("ID"),
            name: get("NAME"),
            pretty_name: get("PRETTY_NAME"),
            version: get("VERSION"),
            version_id: get("VERSION_ID"),
            variant: get("VARIANT"),
            variant_id: get("VARIANT_ID"),
            kernel_release: uname.as_ref().map(|u| u.1.clone()),
            kernel_version: uname.as_ref().map(|u| u.2.clone()),
            machine: uname.map(|u| u.3),
        }
    }

    fn freeze_status(&self) -> GuestFsfreezeStatus {
        if self.shared.freeze.lock().unwrap().frozen {
            GuestFsfreezeStatus::Frozen
        } else {
            GuestFsfreezeStatus::Thawed
        }
    }

    fn freeze(&self, only: Option<Vec<String>>) -> Result<i64, QgaError> {
        if self.updater.running() {
            return Err(QgaError::generic(
                "an update hook run is active; refusing to freeze filesystems",
            ));
        }
        if only
            .as_ref()
            .is_some_and(|m| m.len() > MAX_FREEZE_MOUNTPOINTS)
        {
            return Err(QgaError::generic(format!(
                "at most {MAX_FREEZE_MOUNTPOINTS} mountpoints"
            )));
        }
        let mut state = self.shared.freeze.lock().unwrap();
        let mut mounts = self
            .platform
            .mounts()
            .map_err(|e| QgaError::generic(format!("cannot list mounts: {e}")))?;
        if let Some(only) = &only {
            for wanted in only {
                if !mounts.iter().any(|m| m.dir.as_os_str() == wanted.as_str()) {
                    return Err(QgaError::generic(format!(
                        "{wanted} is not a freezable mounted filesystem"
                    )));
                }
            }
            mounts.retain(|m| only.iter().any(|w| m.dir.as_os_str() == w.as_str()));
        }
        // Like qemu-ga, enter frozen state first and freeze children before
        // their parents. Any failure thaws what was frozen.
        state.frozen = true;
        let mut frozen: Vec<&Mount> = Vec::new();
        for mount in mounts.iter().rev() {
            match self.platform.freeze(mount) {
                Ok(()) => frozen.push(mount),
                Err(e) if e.kind() == io::ErrorKind::Unsupported => {}
                Err(e) => {
                    for mount in &frozen {
                        thaw_one(self.platform.as_ref(), mount);
                    }
                    state.frozen = false;
                    return Err(QgaError::generic(format!(
                        "failed to freeze {}: {e}",
                        mount.dir.display()
                    )));
                }
            }
        }
        state.generation += 1;
        state.deadline = Some(Instant::now() + self.config.freeze_timeout);
        self.shared.wake.notify_all();
        Ok(frozen.len() as i64)
    }

    fn thaw(&self) -> Result<i64, QgaError> {
        let mut state = self.shared.freeze.lock().unwrap();
        // Thaw every mounted filesystem, not only those recorded: state is
        // not persisted, and the host must always be able to recover.
        let count = thaw_all(self.platform.as_ref());
        state.frozen = false;
        state.deadline = None;
        self.shared.wake.notify_all();
        Ok(count as i64)
    }

    fn fstrim(&self, minimum: Option<i64>) -> Result<GuestFilesystemTrimResponse, QgaError> {
        let minimum = u64::try_from(minimum.unwrap_or(0))
            .map_err(|_| QgaError::generic("minimum must not be negative"))?;
        let mounts = self
            .platform
            .mounts()
            .map_err(|e| QgaError::generic(format!("cannot list mounts: {e}")))?;
        let paths = mounts
            .iter()
            .map(|mount| {
                let path = mount.dir.to_string_lossy().into_owned();
                match self.platform.trim(mount, minimum) {
                    Ok((trimmed, minimum)) => GuestFilesystemTrimResult {
                        path,
                        trimmed: Some(trimmed as i64),
                        minimum: Some(minimum as i64),
                        error: None,
                    },
                    Err(e) => GuestFilesystemTrimResult {
                        path,
                        trimmed: None,
                        minimum: None,
                        error: Some(e.to_string()),
                    },
                }
            })
            .collect();
        Ok(GuestFilesystemTrimResponse { paths })
    }

    fn shutdown(&self, mode: Option<String>) -> Result<(), QgaError> {
        let mode = match mode.as_deref().unwrap_or("powerdown") {
            "powerdown" => ShutdownMode::Powerdown,
            "halt" => ShutdownMode::Halt,
            "reboot" => ShutdownMode::Reboot,
            _ => {
                return Err(QgaError::generic(
                    "mode is invalid (valid values are: halt|powerdown|reboot)",
                ))
            }
        };
        let platform = self.platform.clone();
        std::thread::spawn(move || {
            eprintln!("qrab-guest-agent: guest-shutdown {mode:?}");
            if let Err(e) = platform.shutdown(mode) {
                eprintln!("qrab-guest-agent: shutdown failed: {e}");
            }
        });
        Ok(())
    }

    /// Execute one framed request. `None` means no reply is sent.
    pub fn handle(&self, bytes: &[u8]) -> Option<Reply> {
        let value: Value = match serde_json::from_slice(bytes) {
            Ok(value) => value,
            Err(e) => {
                return Some(Reply::error(
                    None,
                    QgaError::generic(format!("JSON parse error: {e}")),
                ))
            }
        };
        let Value::Object(mut request) = value else {
            return Some(Reply::error(
                None,
                QgaError::generic("QMP input must be a JSON object"),
            ));
        };
        let id = request.remove("id");
        let result = self.dispatch(request);
        match result {
            Ok((spec, _)) if !spec.success_response => None,
            Ok((spec, value)) => Some(Reply {
                delimited: spec.name == GuestSyncDelimited::NAME,
                value: with_id(json!({"return": value}), id),
            }),
            Err(error) => Some(Reply::error(id, error)),
        }
    }

    fn dispatch(
        &self,
        mut request: Map<String, Value>,
    ) -> Result<(&'static Spec, Value), QgaError> {
        if request.contains_key("exec-oob") {
            return Err(QgaError::generic("guest agent does not support exec-oob"));
        }
        let name = match request.remove("execute") {
            Some(Value::String(name)) => name,
            Some(_) => {
                return Err(QgaError::generic(
                    "QMP input member 'execute' must be a string",
                ))
            }
            None => return Err(QgaError::generic("QMP input lacks member 'execute'")),
        };
        let args = match request.remove("arguments") {
            None => Map::new(),
            Some(Value::Object(args)) => args,
            Some(_) => {
                return Err(QgaError::generic(
                    "QMP input member 'arguments' must be an object",
                ))
            }
        };
        if let Some(key) = request.keys().next() {
            return Err(QgaError::generic(format!(
                "QMP input member '{key}' is unexpected"
            )));
        }
        let Some(spec) = COMMANDS.iter().find(|s| s.name == name) else {
            return Err(QgaError {
                class: "CommandNotFound",
                desc: format!("The command {name} has not been found"),
            });
        };
        let frozen = self.shared.freeze.lock().unwrap().frozen;
        let hook = spec.name != QrabUpdateTrigger::NAME || self.updater.hook_ready().is_ok();
        if let Some(reason) = self.disabled_reason(spec, frozen, hook) {
            return Err(QgaError {
                class: "CommandNotFound",
                desc: format!("Command {name} has been disabled: {reason}"),
            });
        }
        if let Some(key) = args.keys().find(|k| !spec.args.contains(&k.as_str())) {
            return Err(QgaError::generic(format!(
                "Parameter '{key}' is unexpected"
            )));
        }
        Ok((spec, (spec.handler)(self, Value::Object(args))?))
    }

    /// Serve one byte stream until EOF or an I/O error. Replies are written
    /// in request order; each ends with a line feed, and the
    /// guest-sync-delimited reply is preceded by `0xFF`.
    pub fn serve(&self, mut reader: impl Read, mut writer: impl Write) -> io::Result<()> {
        let mut framer = Framer::new(MAX_REQUEST_BYTES);
        let mut buf = [0u8; 4096];
        loop {
            let n = match reader.read(&mut buf) {
                Ok(0) => return Ok(()),
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            for byte in &buf[..n] {
                let reply = match framer.push(*byte) {
                    None => continue,
                    Some(Frame::Overflow) => Some(Reply::error(
                        None,
                        QgaError::generic(format!(
                            "JSON request exceeds {MAX_REQUEST_BYTES} bytes"
                        )),
                    )),
                    Some(Frame::Message(message)) => self.handle(&message),
                };
                if let Some(reply) = reply {
                    writer.write_all(&reply.encode())?;
                    writer.flush()?;
                }
            }
        }
    }
}

fn with_id(mut value: Value, id: Option<Value>) -> Value {
    if let Some(id) = id {
        value["id"] = id;
    }
    value
}

pub struct Reply {
    pub delimited: bool,
    pub value: Value,
}

impl Reply {
    fn error(id: Option<Value>, error: QgaError) -> Self {
        Self {
            delimited: false,
            value: with_id(
                json!({"error": {"class": error.class, "desc": error.desc}}),
                id,
            ),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        if self.delimited {
            bytes.push(0xff);
        }
        bytes.extend(serde_json::to_vec(&self.value).expect("JSON value encodes"));
        bytes.push(b'\n');
        bytes
    }
}

fn thaw_one(platform: &dyn Platform, mount: &Mount) -> bool {
    let mut thawed = false;
    for _ in 0..MAX_THAW_REPEATS {
        if platform.thaw(mount).is_err() {
            break;
        }
        thawed = true;
    }
    thawed
}

fn thaw_all(platform: &dyn Platform) -> usize {
    match platform.mounts() {
        Ok(mounts) => mounts.iter().filter(|m| thaw_one(platform, m)).count(),
        Err(e) => {
            eprintln!("qrab-guest-agent: cannot list mounts to thaw: {e}");
            0
        }
    }
}

fn watchdog(shared: &Shared, agent: &std::sync::Weak<Agent>) {
    let mut state = shared.freeze.lock().unwrap();
    loop {
        match state.deadline {
            None => {
                let (next, _) = shared
                    .wake
                    .wait_timeout(state, Duration::from_secs(60))
                    .unwrap();
                state = next;
            }
            Some(deadline) if Instant::now() >= deadline => {
                let Some(agent) = agent.upgrade() else { return };
                let count = thaw_all(agent.platform.as_ref());
                state.frozen = false;
                state.deadline = None;
                eprintln!(
                    "qrab-guest-agent: freeze generation {} exceeded {:?}; auto-thawed {count} filesystem(s)",
                    state.generation, agent.config.freeze_timeout
                );
            }
            Some(deadline) => {
                let (next, _) = shared
                    .wake
                    .wait_timeout(state, deadline.saturating_duration_since(Instant::now()))
                    .unwrap();
                state = next;
            }
        }
        if agent.strong_count() == 0 {
            return;
        }
    }
}

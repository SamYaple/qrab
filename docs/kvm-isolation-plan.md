# KVM + Namespace Isolation — Implementation Plan v3

Supersedes v2. Revised based on 6 rounds of automated audit findings (see `.opencode/plans/audit-*.md`).

## Decisions

| Decision | Choice |
|---|---|
| User namespace (`CLONE_NEWUSER`) | ✅ Port from `main` branch |
| Mount namespace (`CLONE_NEWNS`) | ✅ Minimal `/dev` in pre\_exec, private `/proc` in qrab-init child |
| Network namespace (`CLONE_NEWNET`) | ✅ Isolated loopback only |
| UTS namespace (`CLONE_NEWUTS`) | ✅ Include |
| PID namespace (`CLONE_NEWPID`) | ✅ Custom `qrab-init` binary as init process |
| KVM fd passing | ✅ `-add-fd fd=N,set=M` + `-accel kvm,device=/dev/fdset/M` |
| KVM group membership | ✅ Hard requirement, no fallback (auto-detect: try KVM first, fall back to TCG) |
| CI KVM access | ✅ Available — real integration tests |
| QEMU min version | ✅ Bump to 7.2.0 (`-accel kvm,device=` support) |
| Namespace isolation | ✅ Always-on — no opt-out |
| Signal forwarding | ✅ `qrab-init` forwards SIGTERM/SIGINT to QEMU |
| Config transport | ✅ Pipe fd + serde\_json |
| Socket paths | ✅ Private tmpdir per VM (`mkdtemp`) — fixes audit Security #3 |
| Socket permissions | ✅ chmod 0o600 on QMP/VNC sockets — audit Security #2 (already implemented) |
| QMP connect timeout | ✅ 10s timeout on greeting/caps — audit BUG #36 (already fixed) |
| Crash propagation | ✅ qrab-init exit code → synthetic event on events channel — audit BUG #7/#29 |
| KVM auto-detect | ✅ Try `/dev/kvm` open, fall back to `-accel tcg` — audit Industry #49 |
| SMP config | ✅ Add `.smp()` builder method as companion change — audit Industry #38 |
| Graceful shutdown | ✅ ACPI powerdown → timeout → SIGTERM → SIGKILL chain — audit Industry #14 |
| Temp file cleanup | ✅ Cleanup on all failure paths including Qmp::new() — audit BUG #37 (already fixed) |
| Version error msg | ✅ Update to reference `-accel kvm,device=` (7.2.0) — audit BUG #32 |

---

## Architecture

### Process Tree

```
manager (parent)
  └─ qrab-init (child, in user/mount/net/UTS ns; PID ns parent)
       └─ qemu-system-* (pid 1 in new PID ns)
            └─ vCPU threads, I/O threads, etc.
```

### Spawn Sequence

```
manager:
  1. open("/dev/kvm") → kvm_fd (no O_CLOEXEC)
  2. create sync pipes (sync_r, sync_w) for uid_map handshake
  3. create config pipe (cfg_r, cfg_w) for qrab-init config
  4. build QEMU args including:
       -add-fd fd={kvm_fd},set=1
       -accel kvm,device=/dev/fdset/1
  5. serialize InitConfig { qemu_binary, qemu_args, kvm_fd } → JSON
  6. Command::new("qrab-init")
       .arg(cfg_r.as_raw_fd().to_string())    // config pipe fd as argv[1]
       .pre_exec(namespace_setup_closure)
       .kill_on_drop(true)
       .spawn()
  7. write JSON config to cfg_w, close cfg_w
  8. run NsHelper async task (read sync pipe, write uid/gid map, ack)
  9. close parent's copy of kvm_fd
  10. create private tmpdir via mkdtemp (fixes Security #3 — predictable paths)
  11. place QMP/VNC/serial sockets inside private tmpdir
  12. chmod 0o600 on sockets after creation (Security #2 — already implemented)
  13. wait for QMP socket with tokio::time::timeout (BUG #36 — already fixed, 10s)
  14. connect QMP, negotiate caps with timeout
  15. on ANY failure in steps 10-14: cleanup tmpdir + sockets + kill child (BUG #37)
  16. spawn_monitor watches qrab-init exit → send synthetic crash event (BUG #7/#29)

pre_exec (runs in forked child before exec of qrab-init):
  1. setsid()
  2. unshare(CLONE_NEWUSER | CLONE_NEWNS | CLONE_NEWNET | CLONE_NEWUTS | CLONE_NEWPID)
  3. write sync pipe: pid bytes → parent
  4. read sync pipe: wait for parent ack (uid/gid map done)
  5. setuid(0), setgid(0)
  6. setup_dev()           // mount minimal /dev
  7. clear CLOEXEC on kvm_fd, cfg_r

qrab-init:
  1. read config pipe fd from argv[1]
  2. read + deserialize InitConfig from pipe fd
  3. close config pipe fd
  4. fork()
     child (pid 1 in PID ns):
       mount("proc", "/proc", "proc", MS_NOSUID|MS_NODEV|MS_NOEXEC)
       execve(qemu_binary, qemu_args)
     parent:
       close kvm_fd (not needed in init parent)
       install SIGTERM/SIGINT handlers → forward to child
       loop: waitpid(-1, 0)
         when child (QEMU) exits → exit with QEMU's exit code
```

### KVM fd Passing

Parent opens `/dev/kvm` before fork. fd inherited through:
parent fork → pre\_exec child → exec qrab-init → qrab-init fork → child → exec QEMU.

QEMU receives fd via `-add-fd fd=N,set=1` and accesses KVM through `/dev/fdset/1`.

**Auto-detection (audit Industry #49):**

```
try open("/dev/kvm") → Ok(fd):
    use KVM via fd passing
    args += -add-fd fd=N,set=1 -accel kvm,device=/dev/fdset/1
try open("/dev/kvm") → Err(ENOENT | EACCES):
    log warning, fall back to TCG
    args += -accel tcg
```

No hard failure on missing KVM. `.kvm(true)` forces KVM (error if unavailable).
`.kvm(false)` forces TCG. Default: auto-detect.

**Security properties:**
- Permission check happens once in parent (kvm group required)
- Child namespace never touches `/dev/kvm` by path
- No `/dev/kvm` in mount namespace — QEMU cannot reopen it
- fd-only access: KVM ioctls use opener's real credentials

### Mount Namespace Setup (split across two stages)

**Stage 1 — pre\_exec (before exec qrab-init):**

After `setuid(0)` in user namespace. `CAP_SYS_ADMIN` available.

```
mount("none", "/", NULL, MS_REC|MS_PRIVATE, NULL)    // prevent propagation
mount("tmpfs", "/dev", "tmpfs", MS_NOSUID|MS_STRICTATIME, "mode=755")
bind_mount("/dev/null")
bind_mount("/dev/zero")
bind_mount("/dev/urandom")
```

No `/dev/kvm` — KVM accessed via inherited fd only.

**Stage 2 — qrab-init child (after fork, pid 1 in PID ns):**

```
mount("proc", "/proc", "proc", MS_NOSUID|MS_NODEV|MS_NOEXEC, NULL)
```

Mounted from within PID namespace → `/proc` shows only namespace-local processes.
This is why `/proc` mount cannot happen in pre\_exec (pre\_exec process is not in PID ns).

### Signal Forwarding in qrab-init

```
qrab-init parent process:
  sigaction(SIGTERM) → kill(child_pid, SIGTERM)
  sigaction(SIGINT)  → kill(child_pid, SIGINT)
  loop:
    waitpid(-1, 0)
    if child exited → exit(child_exit_code)
    else → continue reaping zombies
```

Manager sends SIGTERM to qrab-init → qrab-init forwards to QEMU → graceful shutdown.
Manager sends SIGKILL to qrab-init → qrab-init dies → kernel kills pid 1 → all PID ns processes die.

### Crash Propagation (audit BUG #7, #29, Industry #18)

v2 gap: QEMU crash logged by `spawn_monitor` but never propagated to event channel
or state machine. User discovers crash only when next QMP command fails.

v3 fix: `spawn_monitor` detects qrab-init exit → sends synthetic event:

```
spawn_monitor:
  child.wait() → exit_status
  if exit_status != expected_shutdown:
      event_tx.send(SyntheticEvent::ProcessDeath { exit_status })
      // Qmp read_loop detects EOF → tasks shut down
      // VmProcess::Drop cleans up tmpdir
```

This addresses:
- BUG #7: No early-exit on QEMU crash — monitor now signals immediately
- BUG #29: Crash not propagated to events — synthetic event sent
- Industry #18: Process death → state transition — event triggers state update

### Graceful Shutdown Escalation (audit Industry #14)

v2 gap: Only SIGTERM forwarding. No timeout-based escalation.

v3 fix: `Vm::shutdown()` implements escalation chain:

```
1. system_powerdown (ACPI) via QMP
2. wait for SHUTDOWN event (timeout: caller-specified, default 30s)
3. if timeout: send quit QMP command
4. wait 5s for qrab-init exit
5. if still alive: SIGKILL to qrab-init (kills entire PID namespace)
```

### Socket Path Security (audit Security #3)

v2 gap: Sockets in `/tmp` with predictable `qrab-qmp-{pid}-{counter}.sock` paths.
TOCTOU symlink attack possible.

v3 fix: Private tmpdir per VM:

```rust
let tmpdir = tempfile::TempDir::new_in(std::env::temp_dir())?;
// tmpdir has random name, 0o700 permissions
let qmp_sock = tmpdir.path().join("qmp.sock");
let vnc_sock = tmpdir.path().join("vnc.sock");
let serial_log = tmpdir.path().join("serial.log");
// chmod 0o600 on sockets after QEMU creates them (Security #2)
```

Sockets live OUTSIDE namespace (manager connects from host). tmpdir cleaned up
by `VmProcess::Drop` (already handles cleanup on all paths including Qmp::new()
failure — BUG #37 fixed).

---

## Background

### Current State (post-audit fixes)

Manager crate (`projects/manager/src/vm.rs`) spawns QEMU via
`tokio::process::Command` with no isolation. `nix` crate was removed
(audit Security NEW-8 — unused dependency). Will be re-added with
actual usage for namespace operations.

`VmProcess` owns a `tokio::process::Child` via `spawn_monitor` task +
oneshot shutdown channel. Recent fixes (audit rounds 3-7):
- `impl Drop for Qmp` aborts background tasks (BUG #2 — r4)
- `spawn_monitor` with `kill_on_drop(true)` eliminates zombies (BUG #5 — r4)
- VmProcess::Drop uses runtime detection for async/sync cleanup (BUG #30 — r5)
- Temp files cleaned on all failure paths (BUG #33, #37 — r6, r7)
- Socket permissions chmod 0o600 (Security #2 — r6)
- stderr piped to `tracing::warn` (Security NEW-5 — r5)
- QMP greeting/caps has 10s timeout (BUG #36 — r7)

This architecture preserved — `qrab-init` replaces direct QEMU spawn as child process.

### Prior Art

`main` branch implements:
- `spawn_namespaced_process()` — `pre_exec` with `unshare(CLONE_NEWUSER)`,
  pipe sync, uid\_map
- `spawn_preexec_nshelper()` — parent-side async uid/gid map writer

Port logic, not code. Libraries have diverged.

### KVM fd Cleanup

KVM resources are fd-ref-counted in kernel. Process death closes all fds.
VM fd (from `KVM_CREATE_VM`) → kernel destroys VM, memory slots, vCPU fds,
IRQ routing. Zero explicit cleanup needed from manager.

---

## Workspace Changes

### New crate: `projects/qrab-init/`

```
projects/qrab-init/
  Cargo.toml              — nix (mount, process, signal), serde, serde_json
  src/
    main.rs               — entry point: read config, fork, mount /proc, exec QEMU
    config.rs             — InitConfig struct (shared type definition)
```

Minimal binary. No tokio, no async. Pure sync `nix` + `serde_json`.

### Modified: `projects/manager/`

```
projects/manager/
  src/
    namespace.rs          NEW  — pre_exec closure, NsHelper, KvmFd, setup_dev()
    vm.rs                 MOD  — always-on namespace spawn, KVM fd passing, bump min version
    lib.rs                MOD  — pub mod namespace, re-exports
  Cargo.toml              MOD  — add nix "mount" feature, add serde + serde_json
  tests/
    namespace.rs          NEW  — isolation verification tests
    kvm.rs                NEW  — KVM integration tests
```

### Config type (shared between manager and qrab-init)

```rust
/// Serialized over pipe fd from manager to qrab-init.
#[derive(Serialize, Deserialize)]
pub struct InitConfig {
    /// Absolute path to QEMU binary.
    pub qemu_binary: PathBuf,
    /// Complete QEMU argument list.
    pub qemu_args: Vec<String>,
    /// Raw fd number for pre-opened /dev/kvm. None if KVM disabled.
    pub kvm_fd: Option<i32>,
}
```

Defined in qrab-init crate. Manager depends on qrab-init for the type,
or define in a shared location. Simplest: manager depends on qrab-init
as a build dependency for the type, or duplicate the small struct.

---

## Implementation Steps

### Step 1 — `qrab-init` crate

Create `projects/qrab-init/` with:

**`main.rs`:**
1. Parse argv[1] as config pipe fd number
2. Read pipe to EOF, deserialize `InitConfig`
3. Close config pipe fd
4. `fork()`
5. **Child (pid 1 in PID ns):**
   - `mount("proc", "/proc", "proc", MS_NOSUID|MS_NODEV|MS_NOEXEC, NULL)`
   - Build argv from `InitConfig`
   - `execve(qemu_binary, args, environ)`
6. **Parent (init):**
   - Close KVM fd (only child/QEMU needs it)
   - Install SIGTERM/SIGINT handlers forwarding to child pid
   - `loop { waitpid(-1, 0) }` — exit when QEMU exits

**`Cargo.toml`:**
```toml
[package]
name = "qrab-init"
version = "0.1.0"
edition = "2021"

[dependencies]
nix = { version = "0.29", features = ["mount", "process", "signal"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### Step 2 — `namespace.rs` in manager

**`KvmFd`:**
```rust
pub struct KvmFd(OwnedFd);

impl KvmFd {
    /// Opens /dev/kvm without O_CLOEXEC.
    pub fn open() -> Result<Self> {
        let fd = nix::fcntl::open(
            "/dev/kvm",
            nix::fcntl::OFlag::O_RDWR,
            nix::sys::stat::Mode::empty(),
        )?;
        Ok(KvmFd(unsafe { OwnedFd::from_raw_fd(fd) }))
    }

    pub fn raw_fd(&self) -> RawFd { self.0.as_raw_fd() }

    /// Clear CLOEXEC. Call in pre_exec.
    pub fn clear_cloexec(&self) -> nix::Result<()> {
        let flags = nix::fcntl::fcntl(self.raw_fd(), FcntlArg::F_GETFD)?;
        let mut fd_flags = FdFlag::from_bits_truncate(flags);
        fd_flags.remove(FdFlag::FD_CLOEXEC);
        nix::fcntl::fcntl(self.raw_fd(), FcntlArg::F_SETFD(fd_flags))?;
        Ok(())
    }
}
```

**`NsHelper`** (parent-side async task):
- Read child PID from sync pipe
- Write `/proc/{pid}/setgroups` → `"deny"`
- Write `/proc/{pid}/uid_map` → `"0 {real_uid} 1"`
- Write `/proc/{pid}/gid_map` → `"0 {real_gid} 1"`
- Ack child via sync pipe

**`make_pre_exec()`** closure factory:
- Captures: sync pipe fds, kvm\_fd, config pipe fd
- Body:
  1. `setsid()`
  2. `unshare(CLONE_NEWUSER | CLONE_NEWNS | CLONE_NEWNET | CLONE_NEWUTS | CLONE_NEWPID)`
  3. Write PID to sync pipe, wait for ack
  4. `setuid(0)`, `setgid(0)`
  5. `setup_dev()`
  6. Clear CLOEXEC on kvm\_fd and config pipe fd

**`setup_dev()`:**
```
mount("none", "/", NULL, MS_REC|MS_PRIVATE, NULL)
mount("tmpfs", "/dev", "tmpfs", MS_NOSUID|MS_STRICTATIME, "mode=755")
bind_mount("/dev/null")
bind_mount("/dev/zero")
bind_mount("/dev/urandom")
```

### Step 3 — `vm.rs` modifications

**Bump version (audit BUG #32 — misleading error message):**
```rust
pub const PRECONFIG_MIN_VERSION: (u32, u32, u32) = (7, 2, 0);
// Error message must reference: "QEMU >= 7.2.0 required for KVM fd passing
// via -accel kvm,device= and blockdev-add preconfig support"
```

**Add `KvmMode` and `kvm` field to `VmBuilder`:**
```rust
pub enum KvmMode { Auto, Force, Disable }

pub fn kvm(mut self, mode: KvmMode) -> Self {
    self.kvm = mode;
    self
}
// Default: KvmMode::Auto
```

**Add `smp` field to `VmBuilder` (audit Industry #38):**
```rust
pub fn smp(mut self, cpus: u32) -> Self {
    self.smp = cpus;
    self
}
// Default: 1. Emits -smp N when N > 1.
```

**Rework `spawn()`:**

Replace direct `Command::new(self.arch.binary())` with namespaced spawn:

```rust
// 0. Create private tmpdir (Security #3 — predictable socket paths)
let tmpdir = tempfile::TempDir::new_in(std::env::temp_dir())?;
let qmp_sock = tmpdir.path().join("qmp.sock");
let vnc_sock = tmpdir.path().join("vnc.sock");
let serial_log = tmpdir.path().join("serial.log");

// 1. Open KVM fd — auto-detect by default (Industry #49)
let kvm_fd = match self.kvm {
    KvmMode::Force => Some(KvmFd::open()
        .context("/dev/kvm open failed — is process in kvm group?")?),
    KvmMode::Disable => None,
    KvmMode::Auto => match KvmFd::open() {
        Ok(fd) => {
            tracing::info!("KVM available, using hardware acceleration");
            Some(fd)
        }
        Err(e) => {
            tracing::warn!("KVM unavailable ({e}), falling back to TCG");
            None
        }
    },
};

// 2. Add accel args — KVM fd passing or TCG fallback
if let Some(ref fd) = kvm_fd {
    args.extend([
        "-add-fd".into(),
        format!("fd={},set=1", fd.raw_fd()),
        "-accel".into(),
        "kvm,device=/dev/fdset/1".into(),
    ]);
} else {
    args.extend(["-accel".into(), "tcg".into()]);
}

// 2b. SMP (Industry #38)
if self.smp > 1 {
    args.extend(["-smp".into(), self.smp.to_string()]);
}

// 3. Create pipes
let (sync_r, sync_w) = nix::unistd::pipe()?;
let (cfg_r, cfg_w) = nix::unistd::pipe()?;

// 4. Serialize config
let config = InitConfig {
    qemu_binary: self.arch.binary(),
    qemu_args: args.clone(),
    kvm_fd: kvm_fd.as_ref().map(|f| f.raw_fd()),
};

// 5. Build pre_exec
let pre_exec = make_pre_exec(sync_w, kvm_fd.as_ref(), cfg_r);

// 6. Spawn qrab-init
let mut child = Command::new("qrab-init")
    .arg(cfg_r.to_string())
    .stdout(Stdio::null())
    .stderr(Stdio::piped())
    .kill_on_drop(true)
    .pre_exec(pre_exec)
    .spawn()?;

// 7. Write config to pipe, close write end
let config_json = serde_json::to_vec(&config)?;
nix::unistd::write(cfg_w, &config_json)?;
nix::unistd::close(cfg_w)?;

// 8. NsHelper — uid/gid map setup
NsHelper::run(sync_r).await?;

// 9. Drop parent's KVM fd
drop(kvm_fd);

// 10. QMP socket wait + connect (existing logic)
```

**Stderr forwarding stays** — pipe qrab-init's stderr to `tracing::warn`
(QEMU's stderr flows through qrab-init to manager). Already implemented
(audit Security NEW-5 — r5).

**Temp file cleanup on failure (audit BUG #37):**
All paths between tmpdir creation and VmProcess construction must clean up
on error. Use `scopeguard::defer!` or manual cleanup in each `?` path.
`VmProcess::Drop` handles cleanup after construction. Already fixed in
current code — preserve behavior when adding namespace spawn.

### Step 4 — `Cargo.toml` changes

**`projects/manager/Cargo.toml`:**
```toml
[dependencies.nix]
version = "0.29"
features = ["fs", "mount", "process", "sched", "signal", "user"]

[dependencies.serde]
version = "1"
features = ["derive"]

[dependencies.serde_json]
version = "1"

[dependencies.tempfile]
version = "3"
```

Note: `nix` was previously removed as unused dependency (audit Security NEW-8).
Re-adding with actual namespace usage resolves that finding.

**Root `Cargo.toml` workspace members:** add `projects/qrab-init`.

### Step 5 — Tests

#### `tests/namespace.rs`

```rust
#[tokio::test]
async fn process_runs_as_uid_0_in_namespace() {
    // Spawn `id -u` via namespaced path (using qrab-init)
    // Verify output is "0"
}

#[tokio::test]
async fn network_namespace_isolates_loopback() {
    // Spawn `ip link` in namespace
    // Only loopback interface visible
}

#[tokio::test]
async fn mount_namespace_has_minimal_dev() {
    // Spawn `ls /dev` in namespace
    // Contains only: null, zero, urandom (no kvm)
}

#[tokio::test]
async fn pid_namespace_shows_only_local_processes() {
    // Spawn `ls /proc` in namespace
    // Only pid 1 visible (and /proc/self)
}

#[tokio::test]
async fn proc_mounted_in_pid_namespace() {
    // Spawn `cat /proc/1/status` in namespace
    // Pid 1 is the spawned process itself
}

#[tokio::test]
async fn uts_namespace_isolation() {
    // Hostname inside namespace can be set independently
}
```

#### `tests/kvm.rs`

```rust
fn require_kvm() {
    if std::fs::metadata("/dev/kvm").is_err() {
        eprintln!("SKIP: /dev/kvm not available");
        return;
    }
}

#[tokio::test]
async fn kvm_via_fd_passing() {
    require_kvm();
    let vm = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .kvm(true)
        .spawn().await.unwrap()
        .start().await.unwrap();
    let kvm_info: qapi::KvmInfo = vm.execute(&qapi::commands::QueryKvm {}).await.unwrap();
    assert!(kvm_info.enabled);
    vm.shutdown().await;
}

#[tokio::test]
async fn kvm_fd_no_dev_kvm_in_namespace() {
    require_kvm();
    // Spawn with kvm(true)
    // Verify /dev/kvm does NOT exist inside namespace
    // Verify KVM still works via fd passing
}

#[tokio::test]
async fn kvm_cleanup_on_process_death() {
    require_kvm();
    // Spawn VM with kvm, kill it
    // Verify /dev/kvm still openable (no leaked state)
}

#[tokio::test]
async fn kvm_disabled_uses_tcg() {
    // Spawn VM without .kvm(true)
    // QMP query-kvm → enabled == false
}

#[tokio::test]
async fn kvm_open_failure_clear_error() {
    // Test with user not in kvm group (if possible)
    // Verify error message references /dev/kvm and kvm group
}
```

---

## Ordering Constraints

1. Open `/dev/kvm` in parent BEFORE fork — parent has kvm group, child won't after namespace
2. `CLONE_NEWUSER` in unshare call — kernel requirement for unprivileged namespaces
3. `setgroups → deny` before `gid_map` write — kernel security requirement
4. `setuid(0)` + `setgid(0)` after parent completes uid/gid map — otherwise EPERM
5. `/dev` mount after `setuid(0)` — needs `CAP_SYS_ADMIN` in user namespace
6. `CLONE_NEWPID` in same unshare call — qrab-init's fork creates pid 1 in new ns
7. `/proc` mount in qrab-init child AFTER fork — must be inside PID namespace
8. Clear CLOEXEC on KVM fd + config pipe fd in pre\_exec — ensures fds survive exec chain

## Security Notes

- User namespace uid 0 is NOT host root — 1:1 mapping to unprivileged real uid
- KVM access gated by parent's group membership at fd open time
- KVM fd passing: QEMU never opens `/dev/kvm` by path — no `/dev/kvm` in namespace
- `setgroups deny` prevents supplementary group manipulation inside namespace
- Mount namespace prevents child from affecting host mounts
- `/dev` contains only null, zero, urandom — minimal attack surface
- `/proc` mounted inside PID namespace — shows only namespace-local processes
- Network namespace gives only isolated loopback — no host network access
- qrab-init handles zombie reaping and signal forwarding
- PID namespace kill-all-on-init-death: killing qrab-init kills entire process tree
- gVisor NOT needed — user namespace + seccomp + IOMMU sufficient for threat model
- IOMMU required for safe VFIO device passthrough (future work)
- Seccomp filter (future work) should restrict to `KVM_*`, `VFIO_*`, standard I/O ioctls
- **Socket paths use private tmpdir** with random name + 0o700 perms (audit Security #3)
- **Sockets chmod 0o600** after QEMU creates them (audit Security #2)
- **`QEMU_TEST_BIN_*` env vars** control binary path — dev/test only, do NOT trust in production (audit Security #7). `InitConfig.qemu_binary` must be validated by caller
- **`VmBuilder::arg()` allows arbitrary QEMU args** — same trust boundary as direct QEMU invocation, namespace doesn't change this (audit Security #8)
- **`kill_on_drop(true)` race** — between spawn and monitor task ownership, early `?` drops child and kills it. Acceptable: fail-closed behavior (audit NEW-15)
- **OnceLock version** — `set_qemu_version()` is global, second call silently dropped. Namespace isolation doesn't fix this. Architectural debt tracked separately (audit BUG #4, Industry #60)

## Host Requirements

- Linux kernel >= 3.8 (user namespaces)
- QEMU >= 7.2.0
- `qrab-init` binary in PATH (built from workspace)
- Process owner in `kvm` group (for KVM acceleration)
- `/proc/sys/kernel/unprivileged_userns_clone = 1` (on Debian/Ubuntu)

## Changes from v2

| Area | v2 | v3 |
|---|---|---|
| KVM mode | `.kvm(true/false)` opt-in | `KvmMode::Auto/Force/Disable` — auto-detect by default (Industry #49) |
| SMP | Not addressed | `.smp(N)` builder method (Industry #38) |
| Socket paths | Predictable `/tmp/qrab-qmp-{pid}-{counter}.sock` | Private `mkdtemp` tmpdir per VM (Security #3) |
| Socket perms | Not addressed | chmod 0o600 (Security #2 — already implemented) |
| Crash detection | QEMU crash logged, not propagated | Synthetic event on crash via spawn\_monitor (BUG #7/#29, Industry #18) |
| Shutdown | SIGTERM forwarding only | ACPI → quit → SIGTERM → SIGKILL escalation chain (Industry #14) |
| QMP connect | No timeout | 10s timeout on greeting/caps (BUG #36 — already fixed) |
| Temp cleanup | Leak on Qmp::new() failure | All failure paths clean up (BUG #37 — already fixed) |
| Stderr | Piped to null | Piped to `tracing::warn` (Security NEW-5 — already fixed) |
| nix dep | Listed but unused (Security NEW-8) | Re-added with actual namespace usage |
| Version error | "does not support --preconfig" | References `-accel kvm,device=` and 7.2.0 (BUG #32) |
| Security notes | Basic namespace properties | Audit-informed: env var trust, arg injection, OnceLock, kill\_on\_drop race |

## Changes from v1

| Area | v1 | v2 |
|---|---|---|
| KVM access | `/dev/kvm` bind-mount + fd pre-open | fd passing only (`-add-fd` + `-accel kvm,device=`) |
| PID ns init | Undecided (Option A/B) | Custom `qrab-init` binary |
| QEMU min version | 7.1.0 | 7.2.0 |
| Isolation toggle | Undecided | Always-on |
| `/proc` | Mounted in pre\_exec (wrong PID ns) | Mounted in qrab-init child (correct PID ns) |
| `/dev/kvm` in namespace | Bind-mounted | Not present — fd-only access |
| VmProcess | Possible refactor | Unchanged — `tokio::process::Child` wrapping qrab-init |
| Init code | Custom init.rs or re-exec | Separate `qrab-init` crate |
| Config transport | Env vars (fragile) | Pipe fd + serde\_json |
| New deps | nix "mount" only | nix "mount", serde + serde\_json in manager; new qrab-init crate |

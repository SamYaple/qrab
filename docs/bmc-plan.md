# Dioxus Virtual BMC Frontend — Implementation Plan

## Context

The qrab workspace provides typed Rust bindings for QEMU's QMP/QGA protocols and a high-level `manager` crate with a typestate VM lifecycle (VmBuilder -> PreconfigVm -> Vm). There is currently no web UI. This plan adds a Dioxus fullstack web frontend at `projects/bmc/` that acts as a virtual Baseboard Management Controller — allowing browser-based VM lifecycle management, status monitoring, event streaming, and QAPI command execution.

The plan is split into two phases, each with its own testing framework and commit points. The completed plan document will be committed to the repo before implementation begins.

---

## Phase 1: Dioxus Scaffold and Server-Side VM Management

### Goal
A working Dioxus fullstack app that can spawn/manage a QEMU VM and display its state in the browser.

### Step 1.1 — Add Clone + Serialize to QmpEvent

**File**: `projects/qapi-protocol/src/lib.rs`

Add `Clone, serde::Serialize` to the derive lists of `QmpEvent` and `Timestamp`. This is backward-compatible and enables the BMC crate to re-broadcast events.

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QmpEvent { ... }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Timestamp { ... }
```

**Commit point**: "Add Clone and Serialize derives to QmpEvent and Timestamp"

### Step 1.2 — Create bmc crate skeleton

**New directory**: `projects/bmc/`

Add `"projects/bmc"` to the workspace members in the root `Cargo.toml`.

**Files to create**:
```
projects/bmc/
  Cargo.toml
  Dioxus.toml
  src/
    main.rs
    state.rs
    server/
      mod.rs
      vm_actions.rs
      vm_status.rs
    components/
      mod.rs
      dashboard.rs
    types.rs
  assets/
    main.css
  tests/
    mock_bmc.rs
    integration.rs
```

**Cargo.toml dependencies**:
- `dioxus` 0.6 with `fullstack` and `router` features
- `axum` 0.7
- `tokio` 1 (full)
- `serde` 1 + `serde_json` 1
- `anyhow` 1
- `tracing` 0.1
- `manager` (path), `qapi` (path), `qapi-protocol` (path)
- Dev: `paste`, `which`, `tokio` (full)

**Dioxus.toml**: Configure fullstack platform, app title "qrab BMC", watch paths.

### Step 1.3 — Core state management (`src/state.rs`)

Key types:

```rust
/// VM lifecycle phases visible to the frontend
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VmPhase {
    Off,
    Preconfig,
    Running,
    Paused,
    ShuttingDown,
}

/// Internal VM state (not serializable — holds owned QEMU handles)
pub enum VmState {
    Off,
    Preconfig(PreconfigVm),
    Running(Vm),
    ShuttingDown,
}

/// VM configuration for spawning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BmcConfig {
    pub arch: String,
    pub machine: String,
    pub memory: String,
}

/// Central BMC state shared across server functions
pub struct BmcState {
    pub vm: Mutex<VmState>,                       // tokio::sync::Mutex
    pub event_tx: broadcast::Sender<QmpEvent>,
    pub event_history: Mutex<VecDeque<QmpEvent>>,  // capped ring buffer
    pub config: Mutex<BmcConfig>,
}
```

**Event forwarding**: When `power_on()` spawns a VM, extract the `mpsc::Receiver<QmpEvent>` from `PreconfigVm.events` using `std::mem::replace` (the field is `pub`), replace with a dummy receiver, then spawn a background tokio task that drains the real receiver and re-broadcasts via `broadcast::Sender<QmpEvent>` + appends to the ring buffer history.

**State transition methods on BmcState**:
| Method | Transition | QMP Command |
|---|---|---|
| `power_on()` | Off -> Preconfig | spawns QEMU via VmBuilder |
| `start_vm()` | Preconfig -> Running | x-exit-preconfig (via PreconfigVm::start) |
| `pause()` | Running -> Running(paused) | stop |
| `resume()` | Running(paused) -> Running | cont |
| `reset()` | Running -> Running | system_reset |
| `power_off()` | Running -> ShuttingDown -> Off | system_powerdown |
| `force_off()` | Any -> Off | quit + cleanup |
| `shutdown()` | Any -> Off | graceful shutdown + cleanup |
| `phase()` | (read-only) | query-status to distinguish paused vs running |

Each method locks `vm`, validates the current state, performs the operation, and updates state. Invalid transitions return an error.

### Step 1.4 — Server functions

**`src/server/vm_actions.rs`**: `#[server]` functions wrapping each BmcState method. Access BmcState via Dioxus server context / Axum state extraction.

**`src/server/vm_status.rs`**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmStatusResponse {
    pub phase: VmPhase,
    pub running: Option<bool>,
    pub status: Option<String>,
    pub qemu_version: Option<String>,
}

#[server]
pub async fn get_status() -> Result<VmStatusResponse, ServerFnError> { ... }
```

Queries `query-status` and `query-version` on the locked VM handle.

### Step 1.5 — Basic Dioxus app (`src/main.rs`, `src/components/dashboard.rs`)

- Router with single `/` route -> Dashboard component
- Dashboard calls `get_status()` server function and displays VM phase
- Minimal power on / start / shutdown buttons to prove the round-trip works

### Step 1.6 — Phase 1 tests

**`tests/mock_bmc.rs`** (unit tests using mock QMP server):
- `test_bmc_state_lifecycle` — Off -> Preconfig -> Running -> shutdown
- `test_event_forwarding` — mock server sends event, verify it appears on broadcast channel and in history
- `test_invalid_transitions` — pause when Off, power_on when Running, etc. return errors
- `test_power_on_config` — verify BmcConfig correctly maps to VmBuilder args

**`tests/integration.rs`** (real QEMU, auto-skip):
- `test_full_lifecycle_real_qemu` — spawn, start, query-status, shutdown with real QEMU
- Uses `Arch::X86_64.is_available()` guard pattern from existing tests

**Commit point**: "Add bmc crate with Dioxus scaffold, VM state management, and Phase 1 tests"

---

## Phase 2: BMC Dashboard — Status, Events, CPU Info, Power Controls

### Goal
Full BMC dashboard with real-time event log, VM status panel, CPU/thread info, and comprehensive power controls.

### Step 2.1 — SSE event endpoint (`src/server/events.rs`)

Add a custom Axum route `/api/events` that streams `QmpEvent` as Server-Sent Events:

```rust
async fn sse_handler(State(state): State<Arc<BmcState>>)
    -> Sse<impl Stream<Item = Result<Event, Infallible>>>
{
    let mut rx = state.event_tx.subscribe();
    let stream = async_stream::stream! {
        while let Ok(event) = rx.recv().await {
            let json = serde_json::to_string(&event).unwrap_or_default();
            yield Ok(Event::default().data(json));
        }
    };
    Sse::new(stream)
}
```

Also add a `#[server]` function `get_event_history()` that returns the current ring buffer contents for initial page load.

**Additional dep**: `async-stream` 0.3, `futures` 0.3 (for Stream trait).

### Step 2.2 — Additional server functions

| Server Function | QMP Command | Available States |
|---|---|---|
| `get_status()` | query-status | Preconfig, Running |
| `get_version()` | query-version | Preconfig, Running |
| `get_name()` | query-name | Preconfig, Running |
| `get_uuid()` | query-uuid | Running |
| `get_cpu_info()` | query-cpus-fast | Running |
| `get_kvm_info()` | query-kvm | Running |
| `get_host_info()` | (std::thread, /proc/cpuinfo) | Always |
| `power_on()` | (VmBuilder::spawn) | Off |
| `start_vm()` | x-exit-preconfig | Preconfig |
| `pause()` | stop | Running |
| `resume()` | cont | Running (paused) |
| `reset()` | system_reset | Running |
| `power_off()` | system_powerdown | Running |
| `force_off()` | quit | Preconfig, Running |

Response DTOs:
```rust
pub struct CpuInfoResponse {
    pub guest_cpus: Vec<GuestCpu>,
    pub host_threads: usize,
    pub host_cores: usize,
}

pub struct GuestCpu {
    pub index: u32,
    pub thread_id: Option<i64>,
    pub halted: bool,
    pub qom_path: Option<String>,
}

pub struct KvmInfoResponse {
    pub enabled: bool,
    pub present: bool,
}
```

### Step 2.3 — UI components

**`src/components/status_panel.rs`** — StatusPanel:
- Displays VmPhase, RunState, QEMU version, VM name/UUID
- Auto-refreshes via `use_resource` + coroutine polling every 2s
- Color-coded status indicator (green=running, yellow=paused, red=off, blue=preconfig)

**`src/components/power_controls.rs`** — PowerControls:
- Buttons: Power On, Start VM, Pause, Resume, Reset, ACPI Powerdown, Force Off
- Each button enabled/disabled based on current VmPhase
- Onclick handlers call corresponding server functions and trigger status refresh
- Confirmation dialog for destructive actions (Force Off, Reset)

**`src/components/event_log.rs`** — EventLog:
- On mount: fetch event history via `get_event_history()` server function
- Subscribe to `/api/events` SSE via `web_sys::EventSource` (client-side JS interop)
- Display scrolling list of events: name, timestamp, data (collapsible JSON)
- Most recent events at top

**`src/components/cpu_info.rs`** — CpuInfo:
- Guest vCPUs table: index, thread ID, halted state, QOM path
- Host info: total cores, total threads
- Auto-refreshes alongside status panel

**`src/components/layout.rs`** — Dashboard layout:
- Header with "qrab Virtual BMC" title
- CSS grid layout: status + controls on top row, CPU info + event log on bottom
- Responsive (single column on narrow screens)

### Step 2.4 — Styling (`assets/main.css`)

Minimal, functional CSS. No framework dependency. Grid layout, status colors, button states, scrollable event log.

### Step 2.5 — Phase 2 tests

**Mock-based tests** (extend `tests/mock_bmc.rs`):
- `test_get_status_preconfig` — mock server responds to query-status, verify VmStatusResponse
- `test_get_status_running` — same for running state
- `test_get_cpu_info` — mock server responds to query-cpus-fast, verify CpuInfoResponse
- `test_pause_resume_cycle` — pause -> verify stop sent + STOP event -> resume -> verify cont sent
- `test_event_history_ring_buffer` — push >1000 events, verify oldest are dropped
- `test_sse_endpoint` — start Axum app with mock state, connect to /api/events, inject events into broadcast channel, verify SSE delivery

**Integration tests** (extend `tests/integration.rs`):
- `test_query_status_real_qemu` — spawn, start, query-status, verify running=true
- `test_pause_resume_real_qemu` — full pause/resume cycle with event verification
- `test_cpu_info_real_qemu` — query-cpus-fast against real QEMU

**Commit point**: "Add BMC dashboard with status, events, CPU info, and power controls"

---

## Commit Sequence Summary

1. Add Clone + Serialize derives to QmpEvent/Timestamp in qapi-protocol
2. Add bmc crate skeleton + Dioxus scaffold + state management + Phase 1 tests
3. Add SSE event streaming endpoint
4. Add status panel + power controls components
5. Add event log component with SSE subscription
6. Add CPU/thread info component + host info
7. Add Phase 2 tests
8. Add CSS styling and dashboard layout polish

---

## Verification

### Phase 1
1. `cargo build -p bmc` compiles without errors
2. `cargo test -p bmc` passes mock and integration tests
3. `dx serve` (or `cargo run -p bmc`) launches web server, browser shows VM phase
4. Clicking "Power On" spawns QEMU, status updates to Preconfig, then Running after "Start"

### Phase 2
1. All Phase 1 verification still passes
2. Event log populates in real-time when VM state changes
3. Power controls correctly enable/disable based on VM phase
4. CPU info table populates when VM is running
5. `cargo test -p bmc` passes all new mock and integration tests
6. SSE endpoint delivers events to browser without polling

### General
- `cargo test --workspace` passes (no regressions in existing crates)
- `cargo clippy --workspace` clean

---

## Key Files to Modify

| File | Change |
|---|---|
| `Cargo.toml` (workspace root) | Add `"projects/bmc"` to members |
| `projects/qapi-protocol/src/lib.rs` | Add Clone + Serialize to QmpEvent, Timestamp |

## Key Files to Create

| File | Purpose |
|---|---|
| `projects/bmc/Cargo.toml` | Crate manifest with dioxus + manager deps |
| `projects/bmc/Dioxus.toml` | Dioxus build config |
| `projects/bmc/src/main.rs` | Entry point, router, app shell |
| `projects/bmc/src/state.rs` | BmcState, VmState, event forwarder |
| `projects/bmc/src/types.rs` | Shared serializable DTOs |
| `projects/bmc/src/server/mod.rs` | Server module |
| `projects/bmc/src/server/vm_actions.rs` | Power/lifecycle server functions |
| `projects/bmc/src/server/vm_status.rs` | Query server functions |
| `projects/bmc/src/server/events.rs` | SSE endpoint |
| `projects/bmc/src/components/mod.rs` | Component module |
| `projects/bmc/src/components/dashboard.rs` | Main dashboard layout |
| `projects/bmc/src/components/status_panel.rs` | VM status display |
| `projects/bmc/src/components/power_controls.rs` | BMC power buttons |
| `projects/bmc/src/components/event_log.rs` | Scrolling QMP event log |
| `projects/bmc/src/components/cpu_info.rs` | CPU/thread info table |
| `projects/bmc/assets/main.css` | Styling |
| `projects/bmc/tests/mock_bmc.rs` | Mock QMP server tests |
| `projects/bmc/tests/integration.rs` | Real QEMU integration tests |

## Architectural Notes

- **Mutex strategy**: `tokio::sync::Mutex` for VM state since all QMP ops are async. Held only during individual command execution (short-lived). Acceptable for single-VM BMC.
- **Event receiver ownership**: Extract `mpsc::Receiver` from `PreconfigVm.events` via `std::mem::replace` before storing in VmState. The real receiver goes to the forwarder task; PreconfigVm retains a dummy. This avoids modifying the manager crate's API.
- **WASM boundary**: `manager`, `qapi`, and `tokio` types must not leak into client-side WASM code. Server functions are `#[cfg(feature = "server")]` only. Shared DTOs in `types.rs` use only serde-compatible types.
- **Auto-skip pattern**: Integration tests check `Arch::X86_64.is_available()` and return early (not fail) if QEMU is absent, matching the existing project convention.

# Video Console for BMC

## Context

The BMC currently runs QEMU headless (`-nographic -nodefaults`). We need to add a video console accessible from the web UI, with keyboard and mouse input. QEMU's VNC server will provide the display, proxied through a WebSocket to a noVNC JavaScript client in the browser.

**Preconfig feasibility:** VNC and VGA are configured via QEMU CLI args, which are processed before QEMU enters the preconfig pause. This means the VNC server is active during preconfig -- the user can see BIOS/UEFI screens before guest execution starts. `chardev-add` and `device_add` are NOT preconfig-safe, so dynamic addition via QMP is not an option -- CLI args are the correct approach.

## Implementation

### 1. VmBuilder: VNC socket management

**File:** `projects/manager/src/vm.rs`

- Add `vnc_sock: Option<PathBuf>` to `VmProcess` (line 25-29). Update `cleanup()` and `Drop` to remove it.
- Add `display: bool` field to `VmBuilder` (line 68-78) with a `.display()` builder method.
- In `spawn()` (line 186), conditionally replace `-nographic -nodefaults` with:
  - When `display: true`: `-nodefaults`, `-device VGA`, `-vnc unix:/tmp/qrab-vnc-{pid}-{id}.sock`
  - When `display: false`: `-nographic -nodefaults` (unchanged)
- Add `vnc_sock()` accessor to both `PreconfigVm` and `Vm`, matching the `serial_log()` pattern.

### 2. BmcConfig + BmcState: wire display flag

**File:** `projects/bmc/src/types.rs`
- Add `display: bool` to `BmcConfig`, default `false`.

**File:** `projects/bmc/src/state.rs`
- Add `vnc_sock: Mutex<Option<PathBuf>>` to `BmcState`.
- In `power_on()`: conditionally call `.display()` on builder when `config.display` is true. After spawn, store `preconfig.vnc_sock().cloned()` into `self.vnc_sock`.
- In `force_off()`: clear `vnc_sock` to `None`.

### 3. WebSocket-to-VNC proxy

**New file:** `projects/bmc/src/server/console.rs`

A raw bidirectional byte proxy: WebSocket <-> VNC Unix socket.

- Axum `WebSocketUpgrade` handler at `/api/console`
- Reads `vnc_sock` path from `BmcState`
- Connects to VNC Unix socket via `tokio::net::UnixStream`
- Two spawned tasks: WS->VNC (binary messages to socket writes) and VNC->WS (socket reads to binary messages)
- `tokio::select!` to tear down when either side disconnects

**File:** `projects/bmc/src/server/mod.rs` -- add `pub mod console;`

**File:** `projects/bmc/Cargo.toml`:
- Add `features = ["ws"]` to axum dependency
- Add `"tokio/net"`, `"tokio/io-util"` to server feature

### 4. Wire custom routes into Dioxus server

**File:** `projects/bmc/src/main.rs`

The current `dioxus::launch(App)` doesn't support custom Axum routes. Switch to `LaunchBuilder` with server config, or manually construct the Axum router and serve Dioxus as a nested service. This also wires up the existing unused `events_router`.

Need to verify the exact Dioxus 0.7 API for this -- check `dioxus::LaunchBuilder` and `ServeConfigBuilder` for route merging support.

### 5. noVNC assets

**Directory:** `projects/bmc/assets/novnc/`

Vendor the noVNC ES module build. The key file is `rfb.js` (the RFB client) plus its supporting modules from `@novnc/novnc` core. These are served as static assets via Dioxus's `asset_dir = "assets"`.

### 6. Console component

**New file:** `projects/bmc/src/components/console.rs`

- Renders a `<div id="vnc-container">`
- Uses Dioxus `eval()` JS interop to import and initialize noVNC's `RFB` class
- Connects to `ws://host/api/console` with `scaleViewport: true`
- noVNC natively handles keyboard capture and mouse events through the VNC protocol -- no additional input code needed
- Shows "Power on the VM to access the console" when VM is off
- Disconnects noVNC when VM shuts down

**File:** `projects/bmc/src/components/mod.rs` -- add `pub mod console;`

**File:** `projects/bmc/src/components/dashboard.rs` -- add `Console` between top-section and EventLog

**File:** `projects/bmc/assets/main.css` -- add `.console-panel` and `.vnc-container` styles (black background, min-height, scale canvas to fit)

### 7. Console availability server function

**File:** `projects/bmc/src/server/vm_status.rs`

Add `get_console_available()` server function that checks `state.vnc_sock` -- lets the frontend conditionally show the console panel.

## Key constraints

- `machine=none` has no PCI bus and cannot host VGA. Display only works with `bios` or `uefi` machine types.
- `-nographic` must NOT be passed when display is enabled (it disables graphical output).
- VNC Unix socket avoids TCP port allocation and external exposure.
- noVNC handles all keyboard/mouse input natively through the VNC protocol.

## Verification

1. **Unit tests:** Spawn VM with `.display()`, verify `vnc_sock()` returns `Some`, verify socket file exists, verify cleanup removes it.
2. **VNC handshake test:** Connect to VNC Unix socket directly, verify first bytes are `RFB xxx.yyy\n`.
3. **Backward compat:** Spawn VM without `.display()`, verify `vnc_sock()` is `None`, behavior unchanged.
4. **Manual:** Launch BMC with `display: true` and `machine: bios`, open web UI, verify console renders, test keyboard/mouse input.

# Console protocol spike: audio, USB and data streams

Spike for `work/console-protocol-spike` (container/bmc-experience-2026-11), 2026-11-14.
Question: which upstream QEMU protocol carries admin-bucket guest audio and browser USB
redirection, and is browser USB worth doing at all. Candidates:

- **A**: keep VNC; audio through QEMU's VNC audio extension; USB through a separate
  `usb-redir` chardev.
- **B**: SPICE (spice-server inside QEMU, plus a browser SPICE client).
- **C**: any other upstream QEMU option (D-Bus display, `qemu-vnc`, other audiodevs).

Everything here is an experiment. No main-path code, image or cluster change was made.

## Recommendation

1. **Audio: A (VNC audio extension).** It works on the **currently pinned qrab QEMU image,
   unmodified and without a rebuild** (experiment A). The work is a JavaScript patch to our
   vendored noVNC (upstream noVNC PR #1952 shows the pattern, ~300 lines), a client-to-server
   RFB filter in the BMC, and three extra QEMU arguments. VNC stays the only console stream.
2. **SPICE is not worth it for audio.** Opus brings bandwidth down to about 100 kbit/s from
   about 1.4 Mbit/s. Against that it needs a QEMU rebuild with libspice-server (GStreamer,
   SASL, JPEG and Opus come with it), a second browser client (spice-html5: LGPLv3, Opus only
   through MediaSource, no usbredir), and only one SPICE client per VM. It also exposes
   display, inputs and cursor channels next to VNC, so each one must be gated. The experiment
   also crashed QEMU with SIGSEGV from a short client link sequence (B4/B5).
3. **Browser USB redirection: do not build it now (cancel `work/admin-webusb-redirect`).**
   Chromium's WebUSB refuses the audio, HID, mass-storage, hub, smart-card, video, AV and
   wireless-controller interface classes, so it refuses almost every device an operator would
   attach: keyboards, mice, security keys, smart cards, USB sticks, webcams, headsets. What
   is left is vendor-specific devices, and on Linux and Windows the host's own driver often
   still holds those. Storage already has Redfish VirtualMedia; keyboard and mouse have VNC;
   serial has the serial console. WebHID is no fallback, because keyboards, mice and security
   keys are protected there too.
4. **If browser USB is ever wanted for a named vendor-class device**, use A's transport: a
   plain `usb-redir` chardev bridged by the manager like `console`. SPICE's usbredir channel
   carries the same usbredir protocol inside spicevmc, so it saves no work. Both transports
   still need a JavaScript usbredir host over WebUSB, and no such library exists.
5. **C is dismissed.** The D-Bus display is local IPC for out-of-process UIs. Using it means
   writing our own remote protocol, and its audio and chardevs offer nothing over A.
6. **No recommended path requires patching QEMU.** Audio needs nothing new. The optional USB
   path needs only configure flags (`--enable-usb-redir` plus libusbredirparser), and the
   pinned 11.1.3 source builds it unchanged (experiment B0). Because no QEMU patch is
   proposed, no awaiting-human patch decision is needed.

## Comparison

| | A: VNC + audio ext + usb-redir chardev | B: SPICE | C: D-Bus display / qemu-vnc |
|---|---|---|---|
| Browser client | noVNC (vendored, MPL-2.0, maintained). Audio is an open upstream PR (#1952, 308 added lines, open since 2025-04), so we carry a JS patch. No JS usbredir host exists (we would write it) | spice-html5 (LGPLv3+, ~400 KB of source, active again in 2026 with 55 commits but no release since 0.3.0 in 2020). It covers display, inputs, cursor, playback (Opus only), port and file transfer, but has no usbredir and no record. eyeos spice-web-client (MIT, 2015-16, dormant). We would write the BMC multi-socket bridge, a playback-only page, and a usbredir host if USB were wanted | No browser client. The D-Bus API is local; we would design a wire protocol and write both ends. `qemu-vnc` (new in 11.x) just re-serves VNC out of process |
| Audio playback | Raw PCM (U8..S32, 1 or 2 ch, at most 48 kHz). Measured 176,966 B/s at S16 stereo 44.1 kHz (1.42 Mbit/s), in ~10 ms chunks. Latency is our jitter buffer plus the network; no A/V timestamps | Opus at 48 kHz stereo when the client advertises it. Measured 12.9 KB/s (~103 kbit/s), 100 frames/s with timestamps. spice-html5 plays through MediaSource `<audio>` (higher, browser-controlled latency); WebCodecs would be our own code | `-audiodev dbus` passes PCM to a local listener; we would need our own streaming |
| Audio record | None: the extension is playback-only. That suits the posture (no microphone) | Record channel only exists when a guest input voice uses the spice audiodev. With `hda-output` it does not exist (measured) | Local only |
| USB | `usb-redir` on a unix chardev. Measured: QEMU speaks the usbredir hello with 8 caps and waits for device_connect. Needs `--enable-usb-redir` | usbredir channel (type 9) over spicevmc. Same usbredir payload, same missing JS host | D-Bus chardev export, local only |
| Data streams | Separate chardevs (serial is already in place) | `port` channel (spiceport). Linked fine, but it is in the crash path (B4/B5) | D-Bus chardevs, local |
| QEMU build | **Audio: none.** The pinned image already has `none`/`wav` audiodevs, intel-hda/hda-output and VNC audio. USB: `--enable-usb-redir`, `libusbredirparser-dev`, runtime `libusbredirparser1t64` (0.15.0) | `--enable-spice --enable-spice-protocol` (+usb-redir). Build: `libspice-server-dev libspice-protocol-dev`. Runtime `libspice-server1` 0.15.2 pulls 24 packages (libopus0, GStreamer, libjpeg, libsasl2, libdrm, ...; 11.8 MB of archives) | `--enable-dbus-display` (gio, gdbus-codegen) |
| Bridge via manager | Audio is in-band on the existing `console` stream; no new operation. USB: a new `usb_redirect` operation hands over one connected chardev stream, like `console` | One unix connection per channel per session: a new `spice_channel {type}` operation per connection, with the manager checking the declared type against the link message | n/a |
| Admin-bucket enforcement point | Audio: a BMC client-to-server RFB filter that strips -259 from SetEncodings and drops 255/1 messages. QEMU gates per connection (measured). USB: admission on the `/api/usb` WebSocket upgrade | The first 22 bytes of each connection: SpiceLinkHeader `REDQ` 2.2, then SpiceLinkMess `channel_type` at **byte 20**. Measured and exact | n/a |
| Concurrency | `share=force-shared`; many viewers, audio opt-in per connection | **One client per VM**: a second main link drops the first (measured) | n/a |
| Robustness | No issues seen | **QEMU SIGSEGV (exit 139)**: playback with a wrong connection_id, then port link+close. 5 of 7 runs | n/a |
| Effect on noVNC / dashboard / bmc-browser.py | Patch both identical noVNC copies; add a speaker toggle; byte proxies unchanged; `bmc-browser.py` unchanged, plus one audio test later | New page, client, WebSocket route per channel in BMC and dashboard; new browser tests | n/a |
| Patch QEMU? | No | No | No |

## Findings

### A. VNC audio extension (QEMU `ui/vnc.c`, pinned 11.1.3 at 005c61c0)

- Wire format: the client advertises pseudo-encoding `VNC_ENCODING_AUDIO 0xFFFFFEFD (-259)`
  (`ui/vnc.h`:419). QEMU answers with a FramebufferUpdate holding one rectangle of encoding
  -259 and no payload (`send_ext_audio_ack`). Client messages are `255, 1, u16 op`:
  0 enable, 1 disable, 2 set-format `u8 fmt (0 U8 .. 5 S32), u8 channels (1|2), u32 freq
  (<= 48000)`. Server messages are `255, 1, u16 op`: 0 end, 1 begin, 2 data `u32 len, PCM`.
  Audio data is throttled with the framebuffer output (`vs->throttle_output_offset`), so a
  slow client drops audio rather than stalling the VM.
- With `-nodefaults` and no `-audiodev` there is no default audio backend
  (`system/vl.c`: `default_audio = 0`), and QEMU ignores -259. **Today's qrab console
  cannot carry audio**, even from a patched client (experiment A1). That matches QEMU's 2023
  change "ui/vnc: don't accept VNC_ENCODING_AUDIO without audiodev".
- A client that sends 255/1 without negotiating -259 is disconnected
  (`"Audio message %d with audio disabled"`, `vnc_client_error`). Only its own connection
  drops; a fresh client on the same VM negotiates normally (A3/A4). Audio capture is per
  `VncState`, so other viewers on `share=force-shared` never receive audio they did not ask
  for.
- noVNC: our vendored copy (`projects/bmc/assets/novnc`, byte-identical to the dashboard
  copy) has no -259 or 255 handling; an unknown rectangle encoding makes it fail with
  "Unsupported encoding". Upstream PR novnc/noVNC#1952 ("Qemu audio support v2", Dietmar
  Maurer, opened 2025-04-06, still open on 2026-10-23) adds `core/audio.js` (173 lines) and 110
  lines in `rfb.js`. It requests U16 stereo 44.1 kHz, schedules `AudioBufferSourceNode`s with a
  20 ms jitter target, resamples above 100 ms of lead and skips above 0.5 s. Review found a
  typo'd identifier in the default branch and a zero-length-payload stall. **This is a JS
  patch, not a QEMU patch**; we would adapt it, request S16 (fmt 3), and play through an
  AudioWorklet with a bounded buffer.
- Bandwidth: raw S16 stereo at 44.1 kHz is 1.41 Mbit/s per listening console (measured
  176,966 B/s). Mono 22.05 kHz would be 0.35 Mbit/s. The 64 KiB WebSocket frame limits in
  the BMC and dashboard are not a problem: one audio message is about 1.7 KB.

### B. SPICE (spice-server 0.15.2, spice-protocol 0.14.3, built into the pinned source)

- Link wire format (spice-protocol `spice/protocol.h`, little-endian, packed):
  `SpiceLinkHeader {u32 magic "REDQ", u32 major 2, u32 minor 2, u32 size}` then
  `SpiceLinkMess {u32 connection_id, u8 channel_type, u8 channel_id, u32 num_common_caps,
  u32 num_channel_caps, u32 caps_offset}` and caps words. **`channel_type` is byte 20 of
  the connection.** Channel types (`spice/enums.h`): 1 main, 2 display, 3 inputs, 4 cursor,
  5 playback, 6 record, 7 tunnel, 8 smartcard, 9 usbredir, 10 port, 11 webdav. The server
  sends a `SpiceLinkReply` (error, 162-byte RSA public key, caps). The client then sends the
  auth mechanism (u32 1) and a 128-byte RSA-OAEP ticket. With `disable-ticketing=on` any
  ticket passes, so **the BMC is the only authentication**.
- Measured on `-spice unix=on,addr=…,disable-ticketing=on,disable-copy-paste=on,
  disable-agent-file-xfer=on` with `-audiodev spice`, usb-redir and spiceport configured:
  the main channel lists `display, cursor, usbredir, playback, port, inputs`. SPICE
  therefore offers a full second display and input path next to VNC, and a gate must
  refuse 2, 3 and 4 when only audio is wanted. A channel the server lacks (record without an
  input voice, smartcard, webdav) is closed without a reply. A non-main link with a wrong
  `connection_id` passes the link reply and fails after auth with `BAD_CONNECTION_ID`.
- Playback: `PLAYBACK_MODE` gives Opus when the client advertises `SPICE_PLAYBACK_CAP_OPUS`.
  `PLAYBACK_START` is 2 channels, S16, 48000 Hz. Measured 100 data messages/s,
  12,880 B/s.
- Single client: a second main-channel link drops the first (measured). Multi-client exists
  only behind `SPICE_DEBUG_ALLOW_MC`, which upstream calls experimental and crash-prone.
- **Crash**: with main linked, a playback link with a wrong connection_id followed by a
  port-channel link and close kills QEMU with SIGSEGV (exit 139). It crashed in 4 of 5 B4
  runs and in the minimal B5 sequence; usbredir alone or port alone did not crash (B2/B3).
  The last log line before the crash is `port:0 … client … have already been removed from
  char dev`. Anything that can reach `spice.sock` can kill the VM, which is another reason
  to keep SPICE out and never to configure spiceport.
- spice-html5 (`gitlab.freedesktop.org/spice/spice-html5`, cloned): its sources say LGPLv3+.
  The last tag is 0.3.0 (2020-09-04). Commits resumed in 2026 (55 commits, latest
  2026-09-09). Playback accepts only `SPICE_AUDIO_DATA_MODE_OPUS` through MediaSource
  `audio/webm; codecs="opus"`. There is no usbredir or record channel code. The SPICE site
  still calls it a prototype that needs a WebSocket proxy. oVirt dropped its integration in
  4.2.

### C. Other upstream QEMU options

- `-display dbus` (`ui/dbus*.c`, `docs/interop/dbus-display.rst`) exports display, input,
  clipboard, audio (`org.qemu.Display1.Audio`, PCM to a listener) and chardevs on a D-Bus
  bus or a peer-to-peer fd. It is designed for local out-of-process UIs. Reaching a browser
  means writing a D-Bus-to-WebSocket protocol server and a matching client, which is a new
  protocol. It also needs `--enable-dbus-display` (gio, gdbus-codegen). Dismissed.
- `qemu-vnc` (new in 11.x, `tools/qemu-vnc`) is a standalone VNC server over that D-Bus
  interface, including audio. It is still the VNC protocol, so it does not change the A/B
  decision. It could later move the VNC server out of the QEMU process.
- Other audiodevs (`wav`, `pa`, `pipewire`, `jack`, `oss`, `sndio`, `alsa`) are local
  sinks, and none streams to a remote client. Dismissed.

### USB in the browser

- WebUSB protected interface classes (WICG spec, enforced by Chromium):
  0x01 Audio, 0x03 HID, 0x08 Mass Storage, 0x09 Hub, 0x0B Smart Card, 0x0E Video,
  0x10 Audio/Video, 0xE0 Wireless Controller. The only exception is the
  `usb-unrestricted` permission, which is limited to Isolated Web Apps that declare it in a
  manifest. A BMC page cannot use it. A separate vendor/product blocklist
  (`blocklist.txt`) also applies.
- Transfer types: the spec does define `isochronousTransferIn/Out`, contrary to the brief's
  premise. In practice isochronous devices (audio, video) are protected classes anyway, and
  isochronous timing over a WebSocket and usbredir is poor.
- Only Chromium-based browsers ship WebUSB; Firefox and Safari do not. On Linux,
  `claimInterface` fails while a kernel driver (cdc_acm, usblp, usb-storage, …) holds the
  interface, and no page API detaches it. The admin would need udev rules, and on Windows
  WinUSB.
- WebHID protects keyboard, keypad, mouse and pointer collections and security keys. Only
  niche HID devices remain (gamepads, macro pads, UPS), and QEMU has no "replay this HID
  descriptor" device, so WebHID cannot feed the guest a generic device.
- Storage: `/Systems/1/VirtualMedia` (Redfish InsertMedia of local .iso/.img) already
  covers installation media. Browser-uploaded media would be an extension of that, not USB.
- What browser USB redirection would cost: a JS usbredir host (usbredirproto: hello and
  caps, device_connect, interface/ep info, control/bulk/interrupt packets, config and
  alt-setting, stream start/stop, filters, 64-bit ids), about 1.5-2.5k lines plus tests. On
  top of that come a new manager operation, a BMC route, a QEMU rebuild, and a device class
  the browser will release.

### Bridging through the manager control socket

- Today `Operation::Console` (`projects/manager/src/pod/socket.rs`) connects to `vnc.sock`
  and `copy_bidirectional`s it with the BMC's control connection. `MAX_CONSOLES = 4`. The BMC
  (`projects/bmc/src/console.rs`) admits admins only, checks Origin for cookie sessions, and
  bridges to the WebSocket. The dashboard proxies the BMC WebSocket byte for byte.
- A audio: no new stream. The BMC's bridge gains the RFB filter on the browser-to-VNC
  direction, applied after the 12-byte version, the security exchange and ClientInit. It must
  parse every client message noVNC sends: 0 SetPixelFormat, 2 SetEncodings, 3 FBUpdateRequest,
  4 Key, 5 Pointer, 6 ClientCutText (and extended clipboard), 150 EnableContinuousUpdates,
  248 Fence, 250 xvp, 251 SetDesktopSize, 255/0 QEMU extended key, 255/1 audio. Unknown
  types close the connection. On admission loss the simplest correct move is to close the
  console connection; injecting 255/1/1 (disable) would also need audio data already in
  flight dropped, which means parsing the server-to-client stream.
- B SPICE would need a typed operation per channel connection
  (`spice_channel { channel_type }`). The manager opens a new `spice.sock` connection,
  rejects the stream unless byte 20 of the first link message matches the declared type, and
  checks it against the VM's enabled set. The BMC holds one WebSocket per channel and admits
  it per type: main+playback for audio, usbredir for USB, never display, inputs or cursor
  while VNC is the console.
- A USB would mirror `console`: a `usb_redirect` operation, a chardev at
  `<session>/usbredir0.sock`, one stream per VM.

### Effect on existing pieces

- noVNC console: A changes only the vendored noVNC, through a recorded patch applied to both
  copies, plus the BMC filter. Unpatched clients behave as today because they never send
  -259. B adds a second client stack and leaves noVNC as is.
- Dashboard proxy (`projects/dashboard/src/console.rs`): byte-transparent, so A needs no
  change beyond the UI toggle and asking the BMC for audio admission. B would need
  per-channel WebSocket routes.
- `contrib/tests/bmc-browser.py`: unchanged by A. One audio test can be added later
  (toggle, then assert that the page's AudioWorklet received frames).

## Experiments

All ran on this host with rootless podman, `--network=none`, TCG, no root, and no cluster
access. The guest-side audio source is `beep-disk.py`, an MBR that drives a 1 kHz PC-speaker
tone (`-machine q35,pcspk-audiodev=snd0`), so no guest OS is needed. Raw outputs are in
`results/`.

| # | Script | Image | Result |
|---|---|---|---|
| A0 | `podman run … -audiodev help`, `-device help` | pinned `qrab/qemu@sha256:771ce49b…` | audiodevs `none wav`; `intel-hda`, `ich9-intel-hda`, `hda-output/duplex/micro`, `AC97`, `sb16`, `usb-audio`, `virtio-sound-pci` are present. No `usb-redir`; `-spice` is an invalid option |
| A1 | `run-vnc-audio.sh` step 1 | pinned | Current qrab arguments: no -259 ack, and QEMU drops the client that sends audio enable |
| A2 | step 2 | pinned | `-audiodev none,id=snd0 -device intel-hda -device hda-output,audiodev=snd0 -vnc …,audiodev=snd0`: ack rectangle, then `audio-begin` and 306 data messages, 176,966 B/s, every sample non-zero (the tone) |
| A3/A4 | steps 3-4 | pinned | Unnegotiated audio enable drops that client only; a fresh negotiating client on the same VM gets audio |
| A-USB | `run-usbredir.sh` + `usbredir_hello_probe.py` | experiment | `usb-redir` on a plain unix chardev: QEMU sends the usbredir hello `qemu usb-redir guest 11.1.3` with 8 caps (including 64-bit ids and bulk streams) and keeps the link open waiting for device_connect |
| B0 | `prepare-context.sh` + `Containerfile.qemu-spice` | experiment | The pinned source with the existing locked subprojects and the qrab patch list (no new patch) configures and builds with `--enable-spice --enable-spice-protocol --enable-usb-redir` (meson: spice protocol 0.14.3, spice server 0.15.2, usb_redir enabled). Binary 28.3 MB against 28.1 MB pinned |
| B1 | `run-spice.sh` + `spice_link_probe.py` | experiment | Main link OK; channel list as above; Opus playback 48 kHz/2ch, 12,880 B/s; wrong connection_id gives BAD_CONNECTION_ID; record, smartcard and webdav closed without reply; a second main link drops the first; VNC still serves the same VM |
| B2/B3 | `spice_channel_churn.py` | experiment | Linking and closing record, usbredir or port, alone or paired, leaves the server alive |
| B4/B5 | `spice_crash_repro.py` | experiment | QEMU SIGSEGV (exit 139) in 4/5 runs of the full sequence and in the minimal wrong-id playback + port sequence |

Not done: an in-browser listening test with a real guest sound driver. It needs the noVNC
audio patch, which is the implementation task itself. The QEMU side (capture through an
HDA codec on the same audiodev) uses the same mixing path as the PC-speaker tone measured
here.

Reproduce (paths short enough for AF_UNIX's 108-byte limit):

```
contrib/experiments/console-protocol/run-vnc-audio.sh /tmp/qa
git init /tmp/q && git -C /tmp/q fetch --depth 1 https://gitea.yapplesauce.com/mirrors/qemu.git \
  005c61c0ef6f6ae615283440b4bb7be51b2c6ddd && git -C /tmp/q checkout FETCH_HEAD
contrib/experiments/console-protocol/prepare-context.sh /tmp/q /tmp/qctx
podman build -f contrib/experiments/console-protocol/Containerfile.qemu-spice \
  -t localhost/qrab-experiment/qemu-spice:11.1.3 /tmp/qctx
contrib/experiments/console-protocol/run-spice.sh /tmp/qb      # ~5 min, many fresh VMs
contrib/experiments/console-protocol/run-usbredir.sh /tmp/qu
```

The SPICE probes need python3 `cryptography` (RSA-OAEP ticket).

## Sources

- QEMU 11.1.3 source at 005c61c0 (`ui/vnc.c`, `ui/vnc.h`, `audio/audio.c`,
  `system/vl.c`, `hw/i386/pc.c`, `meson.build`, `meson_options.txt`,
  `docs/tools/qemu-vnc.rst`, `docs/interop/dbus-display.rst`), from
  https://gitea.yapplesauce.com/mirrors/qemu.git
- spice-protocol 0.14.3 headers (Debian trixie `libspice-protocol-dev`): `spice/protocol.h`,
  `spice/enums.h`
- https://github.com/novnc/noVNC/pull/1952 (noVNC QEMU audio PR: status, size, design)
- https://listman.redhat.com/archives/libvir-list/2023-January/237051.html (QEMU: no -259
  without audiodev)
- https://gitlab.freedesktop.org/spice/spice-html5 (cloned: licence, tags, playback code)
- https://www.spice-space.org/spice-html5.html (prototype status, WebSocket proxy)
- https://bugzilla.redhat.com/show_bug.cgi?id=1443989 (oVirt dropped spice-html5 in 4.2)
- https://github.com/eyeos/spice-web-client (alternative SPICE web client)
- https://lists.freedesktop.org/archives/spice-devel/2012-February/007328.html
  (`SPICE_DEBUG_ALLOW_MC` multi-client is experimental)
- https://wicg.github.io/webusb/ and https://raw.githubusercontent.com/WICG/webusb/main/index.bs
  (protected interface classes, isochronous methods, `usb-unrestricted`)
- https://groups.google.com/a/chromium.org/g/blink-dev/c/LZXocaeCwDw (Chromium WebUSB
  interface class filtering)
- https://developer.mozilla.org/en-US/docs/Web/API/USBDevice/isochronousTransferIn (not
  Baseline; Chromium only)
- https://kiwix.lema.org/stackoverflow.com_en_all_2023-11/questions/47695160/failed-to-claim-interface-0-device-or-resource-busy
  (Linux kernel driver blocks claimInterface)
- https://developer.chrome.com/articles/hid (WebHID protected keyboard/mouse collections)

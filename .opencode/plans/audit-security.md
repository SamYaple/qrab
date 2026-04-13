# Security Audit Report

Generated: 2026-04-13 | Re-audited: 2026-04-13

## Prior Findings Status

| # | Finding | Severity | Status |
|---|---------|----------|--------|
| 1 | Unbounded `read_line` — DoS | HIGH (CWE-400) | STILL VULNERABLE |
| 2 | No socket permission control | HIGH (CWE-732) | STILL VULNERABLE |
| 3 | Predictable socket paths — TOCTOU | MEDIUM (CWE-377/367) | STILL VULNERABLE |
| 4 | No auth on BMC web interface | HIGH (CWE-306) | STILL VULNERABLE |
| 5 | Passwords not zeroized | MEDIUM (CWE-316) | STILL VULNERABLE |
| 6 | QGA sync ID truncation | LOW (CWE-330) | STILL VULNERABLE |
| 7 | Env var controls binary path | MEDIUM (CWE-426) | STILL VULNERABLE |
| 8 | Arbitrary QEMU args | MEDIUM (CWE-78) | STILL VULNERABLE |
| 9 | Firmware path not validated | LOW (CWE-22) | STILL VULNERABLE |
| 10 | Advisory deps | LOW (CWE-1395) | INDETERMINATE |
| 11 | No `unsafe` | POSITIVE | CONFIRMED STILL POSITIVE |
| 12 | Silent error swallowing | LOW (CWE-754) | STILL VULNERABLE |

**Zero mitigated.**

## NEW Vulnerabilities

### NEW-1: VNC WebSocket — No Origin Validation (HIGH, CWE-346)

`bmc/src/server/console.rs:20-24` — No `Origin` header check on WebSocket upgrade. Cross-site WebSocket hijacking gives any browser tab full VNC access. Combined with #4 (no auth) + `0.0.0.0` default bind = network-accessible unauthenticated VM control from any website.

### NEW-2: Unrestricted Static File Serving (MEDIUM, CWE-22)

`bmc/src/main.rs:47-53` — `ServeDir` uses relative paths. Wrong working directory = unexpected content served. `tower_http::ServeDir` prevents `../` but relative base fragile.

### NEW-3: Disk Path Injection via BmcConfig (HIGH, CWE-22/73)

`bmc/src/types.rs:79`, `state.rs:366` — `DiskConfig.file` is `PathBuf`, no validation. `BmcConfig` is `Deserialize`. If ever user-settable, attacker controls file path to `blockdev-add` → arbitrary file read/write through QEMU block layer.

### NEW-4: curl Subprocess — No TLS Pinning (MEDIUM, CWE-78)

`bmc/src/state.rs:487-489` — `curl` invoked without `--proto =https`. MITM could serve malicious disk image. No checksum verification.

### NEW-5: QEMU stderr Suppressed (LOW, CWE-223)

`manager/src/vm.rs:258-259` — `Stdio::null()` for stdout+stderr. Security warnings invisible.

### NEW-6: No Rate Limiting on VM Control (MEDIUM, CWE-799)

`bmc/src/server/vm_actions.rs` — No throttling. Rapid power_on/force_off causes resource exhaustion.

### NEW-7: Event History Unbounded Size (LOW, CWE-400)

`bmc/src/state.rs:19,315-318` — 1000 cap but each entry size unchecked. Large event payloads grow memory.

### NEW-8: `nix` Crate Unused but in Deps (LOW, CWE-1395)

`manager/Cargo.toml:17-19` — `nix` pulled with 5 features but no `use nix` in source. Unnecessary attack surface.

## Summary

| Severity | Prior (still vulnerable) | New |
|----------|------------------------|-----|
| **HIGH** | #1 unbounded read, #2 socket perms, #4 no BMC auth | NEW-1 CSWSH, NEW-3 disk path injection |
| **MEDIUM** | #3 TOCTOU, #5 secrets, #7 env binary, #8 arg injection | NEW-2 static files, NEW-4 curl no TLS, NEW-6 no rate limit |
| **LOW** | #6 weak nonce, #9 path traversal, #12 silent errors | NEW-5 stderr, NEW-7 event memory, NEW-8 unused dep |

**Critical combo:** #4 + NEW-1 = unauthenticated remote VM control from any browser on network.

# Security Audit Report

Generated: 2026-04-13 | Round 5

## Prior Findings

| # | Finding | Severity | Status |
|---|---------|----------|--------|
| 1 | Unbounded read_line — DoS | HIGH (CWE-400) | STILL VULNERABLE |
| 2 | No socket permission control | HIGH (CWE-732) | STILL VULNERABLE |
| 3 | Predictable socket paths — TOCTOU | MEDIUM (CWE-377/367) | STILL VULNERABLE |
| 4 | No auth on BMC web interface | HIGH (CWE-306) | STILL VULNERABLE |
| 5 | Passwords not zeroized | INFORMATIONAL | N/A |
| 6 | QGA sync ID truncation | LOW (CWE-330) | STILL VULNERABLE (theoretical) |
| 7 | Env var controls binary path | MEDIUM (CWE-426) | STILL VULNERABLE |
| 8 | Arbitrary QEMU args | MEDIUM (CWE-78) | STILL VULNERABLE |
| 9 | Firmware path not validated | LOW (CWE-22) | STILL VULNERABLE |
| 11 | No unsafe | POSITIVE | CONFIRMED |
| 12 | Silent error swallowing | LOW (CWE-754) | PARTIALLY MITIGATED (stderr→tracing) |
| NEW-1 | VNC WebSocket no Origin (CSWSH) | HIGH (CWE-346) | STILL VULNERABLE |
| NEW-3 | Disk path injection via BmcConfig | MEDIUM (CWE-22/73) | STILL VULNERABLE |
| NEW-4 | curl no TLS pinning | MEDIUM (CWE-295) | STILL VULNERABLE |
| NEW-5 | QEMU stderr suppressed | LOW (CWE-223) | **MITIGATED** (piped to tracing) |
| NEW-6 | No rate limiting on VM control | MEDIUM (CWE-799) | STILL VULNERABLE |
| NEW-7 | Event history unbounded entry size | INFORMATIONAL | DOWNGRADED (count bounded) |
| NEW-8 | nix crate unused | LOW (CWE-1395) | STILL PRESENT |
| NEW-9 | SSE event stream no auth | MEDIUM (CWE-306) | STILL VULNERABLE |
| NEW-12 | Error messages leak internal state | LOW (CWE-209) | STILL VULNERABLE |

## NEW Findings (Round 5)

| # | Finding | Severity | CWE |
|---|---------|----------|-----|
| NEW-13 | BmcConfig pub Mutex — one fn from unauth mutation | HIGH | CWE-306 |
| NEW-14 | No timeout on QMP connect (greeting/caps) | MEDIUM | CWE-400 |
| NEW-15 | kill_on_drop race in monitor handoff | LOW | CWE-367 |
| NEW-16 | No TLS on BMC HTTP server — cleartext VNC/QMP | HIGH | CWE-319 |
| NEW-17 | WebSocket binary frame size unbounded | MEDIUM | CWE-400 |

## Summary

| Severity | Count |
|----------|-------|
| HIGH | 5 (#1, #2, #4, NEW-1, NEW-16) |
| MEDIUM | 8 (#3, #7, #8, NEW-3, NEW-4, NEW-6, NEW-9, NEW-14, NEW-17) |
| LOW | 5 |
| POSITIVE | 1 |

**Critical chain:** #4 + NEW-1 + NEW-9 + NEW-16 = unauthenticated remote VM takeover over cleartext HTTP.
**Top 3:** BMC auth middleware, TLS on HTTP, bound read_line + connect timeout.

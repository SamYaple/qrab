# Security Audit Report

Generated: 2026-04-13 | Re-audited: 2026-04-13 (x3)

## Prior Findings Status

| # | Finding | Severity | Status |
|---|---------|----------|--------|
| 1 | Unbounded `read_line` — DoS | HIGH (CWE-400) | STILL VULNERABLE |
| 2 | No socket permission control | HIGH (CWE-732) | STILL VULNERABLE |
| 3 | Predictable socket paths — TOCTOU | MEDIUM (CWE-377/367) | STILL VULNERABLE |
| 4 | No auth on BMC web interface | HIGH (CWE-306) | STILL VULNERABLE |
| 5 | Passwords not zeroized | — | **DOWNGRADED to INFORMATIONAL** — no password fields in current source |
| 6 | QGA sync ID truncation | LOW (CWE-330) | STILL VULNERABLE (theoretical) |
| 7 | Env var controls binary path | MEDIUM (CWE-426) | STILL VULNERABLE |
| 8 | Arbitrary QEMU args | MEDIUM (CWE-78) | STILL VULNERABLE |
| 9 | Firmware path not validated | LOW (CWE-22) | STILL VULNERABLE |
| 10 | Advisory deps | LOW (CWE-1395) | INDETERMINATE |
| 11 | No `unsafe` | POSITIVE | CONFIRMED POSITIVE |
| 12 | Silent error swallowing | LOW (CWE-754) | STILL VULNERABLE |

## NEW Vulnerabilities

| # | Finding | Severity |
|---|---------|----------|
| NEW-1 | VNC WebSocket — no Origin validation (CSWSH) | HIGH (CWE-346) |
| NEW-2 | Static file serving relative paths | LOW-MEDIUM (CWE-22) |
| NEW-3 | Disk path injection via BmcConfig | HIGH (CWE-22/73) — currently MEDIUM exposure (config not API-settable yet) |
| NEW-4 | curl subprocess no TLS pinning | MEDIUM (CWE-295) |
| NEW-5 | QEMU stderr suppressed | LOW (CWE-223) |
| NEW-6 | No rate limiting on VM control | MEDIUM (CWE-799) |
| NEW-7 | Event history unbounded entry size | LOW (CWE-400) |
| NEW-8 | nix crate unused but in deps | LOW (CWE-1395) |
| NEW-9 | SSE event stream no auth | MEDIUM (CWE-306) |
| NEW-10 | Broadcast channel backpressure | LOW (CWE-400) |
| NEW-11 | Serial log path TOCTOU | LOW (CWE-367) |

## Summary

| Severity | Count |
|----------|-------|
| HIGH | 4 (#1, #2, #4, NEW-1) |
| MEDIUM | 6 (#3, #7, #8, NEW-3*, NEW-4, NEW-6, NEW-9) |
| LOW | 8 (#6, #9, #12, NEW-2, NEW-5, NEW-7, NEW-8, NEW-10, NEW-11) |
| POSITIVE | 1 (#11 no unsafe) |

**Critical combo:** #4 + NEW-1 = unauthenticated remote VM control from any browser on network.

**Top 3 fixes:** Add BMC auth middleware, bound read_line, set socket permissions 0o600.

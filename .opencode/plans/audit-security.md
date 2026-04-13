# Security Audit Report

Generated: 2026-04-13 | Round 4

## Prior Findings

| # | Finding | Severity | Status |
|---|---------|----------|--------|
| 1 | Unbounded read_line — DoS | HIGH (CWE-400) | STILL VULNERABLE |
| 2 | No socket permission control | HIGH (CWE-732) | STILL VULNERABLE |
| 3 | Predictable socket paths — TOCTOU | MEDIUM (CWE-377/367) | STILL VULNERABLE |
| 4 | No auth on BMC web interface | HIGH (CWE-306) | STILL VULNERABLE |
| 5 | Passwords not zeroized | INFORMATIONAL | No password fields currently |
| 6 | QGA sync ID truncation | LOW (CWE-330) | STILL VULNERABLE (theoretical) |
| 7 | Env var controls binary path | MEDIUM (CWE-426) | STILL VULNERABLE |
| 8 | Arbitrary QEMU args | MEDIUM (CWE-78) | STILL VULNERABLE |
| 9 | Firmware path not validated | LOW (CWE-22) | STILL VULNERABLE |
| 10 | Advisory deps | LOW (CWE-1395) | INDETERMINATE |
| 11 | No unsafe | POSITIVE | CONFIRMED |
| 12 | Silent error swallowing | LOW (CWE-754) | STILL VULNERABLE (partial remediation in monitor) |

## NEW + Prior NEW Findings

| # | Finding | Severity |
|---|---------|----------|
| NEW-1 | VNC WebSocket no Origin validation (CSWSH) | HIGH (CWE-346) |
| NEW-2 | Static file serving relative paths | LOW (CWE-22) |
| NEW-3 | Disk path injection via BmcConfig | MEDIUM (CWE-22/73) |
| NEW-4 | curl no TLS pinning | MEDIUM (CWE-295) |
| NEW-5 | QEMU stderr suppressed | LOW (CWE-223) |
| NEW-6 | No rate limiting on VM control | MEDIUM (CWE-799) |
| NEW-7 | Event history unbounded entry size | LOW (CWE-400) |
| NEW-8 | nix crate unused | LOW (CWE-1395) |
| NEW-9 | SSE event stream no auth | MEDIUM (CWE-306) |
| NEW-10 | Broadcast backpressure | LOW (CWE-400) |
| NEW-11 | cirros download TOCTOU | LOW (CWE-367) |
| NEW-12 | Server fn error messages leak internal state | LOW (CWE-209) |

## Summary

| Severity | Count |
|----------|-------|
| HIGH | 4 (#1, #2, #4, NEW-1) |
| MEDIUM | 7 (#3, #7, #8, NEW-3, NEW-4, NEW-6, NEW-9) |
| LOW | 9 |
| POSITIVE | 1 |

**Critical chain:** #4 + NEW-1 + NEW-9 = unauthenticated remote VM takeover from any browser.
**Top 3 fixes:** BMC auth middleware, bound read_line, socket permissions.

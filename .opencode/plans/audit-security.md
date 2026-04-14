# Security Audit Report

Generated: 2026-04-13 | Round 8

## Changes This Round
- **#1 (unbounded read_line) → MITIGATED** — `read_line_bounded` with 1 MiB cap in handshake AND read_loop
- **NEW-25 (read_loop unbounded) → RESOLVED** — same fix

## Findings

| # | Finding | Severity | Status |
|---|---------|----------|--------|
| 1 | Unbounded read_line | HIGH (CWE-400) | **MITIGATED** (1 MiB cap) |
| 2 | Socket permissions | HIGH (CWE-732) | MITIGATED (chmod 0o600) |
| 4 | No auth on BMC web | HIGH (CWE-306) | STILL VULNERABLE |
| 11 | No unsafe | POSITIVE | CONFIRMED |
| 12 | Silent error swallowing | LOW (CWE-754) | MOSTLY MITIGATED |
| NEW-1 | VNC WebSocket no Origin (CSWSH) | HIGH (CWE-346) | STILL VULNERABLE |
| NEW-3 | Disk path injection | MEDIUM (CWE-22/73) | STILL VULNERABLE |
| NEW-4 | curl no TLS pinning | MEDIUM (CWE-295) | STILL VULNERABLE |
| NEW-5 | QEMU stderr suppressed | LOW | MITIGATED |
| NEW-6 | No rate limiting | MEDIUM (CWE-799) | STILL VULNERABLE |
| NEW-9 | SSE no auth | MEDIUM (CWE-306) | STILL VULNERABLE |
| NEW-12 | Error messages leak state | LOW (CWE-209) | STILL VULNERABLE |
| NEW-16 | No TLS on BMC HTTP | HIGH (CWE-319) | STILL VULNERABLE |
| NEW-17/22 | WebSocket frame unbounded | MEDIUM (CWE-400) | STILL VULNERABLE |
| NEW-19 | No CSRF on server fns | MEDIUM (CWE-352) | STILL VULNERABLE |
| NEW-20 | Cirros no checksum | MEDIUM (CWE-494) | STILL VULNERABLE |
| NEW-24 | No CSP headers | LOW (CWE-16) | STILL VULNERABLE |
| NEW-26 | Config mutation chain | HIGH (CWE-306) | STILL VULNERABLE |
| NEW-27 | Mock server unbounded reads | LOW (CWE-400) | NEW |
| NEW-28 | Static asset integrity | LOW (CWE-367) | NEW |

## Summary

| Severity | Count |
|----------|-------|
| HIGH | 3 (#4, NEW-1, NEW-16) |
| MEDIUM | 7 |
| LOW | 4 |
| MITIGATED | 6 (#1, #2, #12, NEW-5, NEW-14, NEW-15) |
| RESOLVED | 2 (NEW-8, NEW-25) |

**Critical chain:** #4 + NEW-1 + NEW-16 + NEW-19 + NEW-26 = unauthenticated CSRF VM takeover over cleartext.
**Top 3:** BMC auth + CSRF, TLS, WebSocket frame limits.

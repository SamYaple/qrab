# Security Audit Report

Generated: 2026-04-13 | Round 7

## Changes This Round
- **#1 (unbounded read_line) → PARTIALLY MITIGATED** — handshake has 10s timeout. read_loop still unbounded per-line.
- **NEW-14 (QMP connect timeout) → MITIGATED** — 10s handshake timeout covers greeting/caps.
- **NEW-8 (nix unused) → RESOLVED** — dependency removed.

## Findings

| # | Finding | Severity | Status |
|---|---------|----------|--------|
| 1 | Unbounded read_line — DoS | HIGH (CWE-400) | PARTIALLY MITIGATED (read_loop still unbounded) |
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
| NEW-25 | read_loop unbounded line length | MEDIUM (CWE-400) | NEW |
| NEW-26 | Config mutation chain (auth+path) | HIGH (CWE-306) | NEW |

## Summary

| Severity | Count |
|----------|-------|
| HIGH | 4 (#1 partial, #4, NEW-1, NEW-16) |
| MEDIUM | 8 |
| LOW | 2 |
| MITIGATED | 5 (#2, #12, NEW-5, NEW-14, NEW-15) |
| RESOLVED | 1 (NEW-8) |

**Critical chain:** #4 + NEW-1 + NEW-16 + NEW-19 + NEW-26 = unauthenticated CSRF VM takeover + host file read over cleartext.

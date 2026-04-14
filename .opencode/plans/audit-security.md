# Security Audit Report

Generated: 2026-04-13 | Round 6

## Findings

| # | Finding | Severity | Status |
|---|---------|----------|--------|
| 1 | Unbounded read_line — DoS | HIGH (CWE-400) | STILL VULNERABLE |
| 2 | No socket permission control | HIGH (CWE-732) | **MITIGATED** — chmod 0o600 on QMP+VNC sockets |
| 3 | Predictable socket paths — TOCTOU | MEDIUM (CWE-377/367) | STILL VULNERABLE (reduced — chmod helps) |
| 4 | No auth on BMC web interface | HIGH (CWE-306) | STILL VULNERABLE |
| 11 | No unsafe | POSITIVE | CONFIRMED |
| 12 | Silent error swallowing | LOW (CWE-754) | MOSTLY MITIGATED (stderr→tracing, parse errors logged) |
| NEW-1 | VNC WebSocket no Origin (CSWSH) | HIGH (CWE-346) | STILL VULNERABLE |
| NEW-3 | Disk path injection via BmcConfig | MEDIUM (CWE-22/73) | STILL VULNERABLE |
| NEW-4 | curl no TLS pinning | MEDIUM (CWE-295) | STILL VULNERABLE |
| NEW-5 | QEMU stderr suppressed | LOW (CWE-223) | MITIGATED |
| NEW-6 | No rate limiting on VM control | MEDIUM (CWE-799) | STILL VULNERABLE |
| NEW-8 | nix crate unused | LOW (CWE-1395) | STILL PRESENT |
| NEW-9 | SSE event stream no auth | MEDIUM (CWE-306) | STILL VULNERABLE |
| NEW-12 | Error messages leak internal state | LOW (CWE-209) | STILL VULNERABLE |
| NEW-14 | No timeout on QMP connect | MEDIUM (CWE-400) | STILL VULNERABLE |
| NEW-15 | kill_on_drop race | LOW (CWE-367) | MITIGATED |
| NEW-16 | No TLS on BMC HTTP | HIGH (CWE-319) | STILL VULNERABLE |
| NEW-17 | WebSocket frame size unbounded | MEDIUM (CWE-400) | STILL VULNERABLE |
| NEW-19 | No CSRF protection on server fns | MEDIUM (CWE-352) | NEW |
| NEW-20 | Cirros download no checksum | MEDIUM (CWE-494) | NEW |
| NEW-22 | WebSocket frame size confirmed 64MB | MEDIUM (CWE-400) | NEW (refinement of NEW-17) |
| NEW-23 | Pub config mutation without auth | HIGH (CWE-306) | NEW (refinement of NEW-13) |
| NEW-24 | No CSP headers on noVNC | LOW (CWE-22) | NEW |

## Summary

| Severity | Count |
|----------|-------|
| HIGH | 4 (#1, #4, NEW-1, NEW-16) |
| MEDIUM | 9 |
| LOW | 4 |
| MITIGATED | 4 (#2, #12, NEW-5, NEW-15) |
| POSITIVE | 1 |

**Critical chain:** #4 + NEW-1 + NEW-9 + NEW-16 + NEW-19 = unauthenticated CSRF-exploitable VM takeover over cleartext.
**Top 3:** BMC auth + CSRF tokens, TLS on HTTP, bound read_line.

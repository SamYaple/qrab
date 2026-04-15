# Security Audit Report

Generated: 2026-04-13 | Round 11

## Changes: 4 new findings from namespace code. No mitigations this round.

## Summary: 3 HIGH, 8 MEDIUM, 9 LOW. 7 MITIGATED, 2 RESOLVED.

| # | Finding | Severity | Status |
|---|---------|----------|--------|
| 4 | No auth on BMC web | HIGH (CWE-306) | STILL VULNERABLE |
| NEW-1 | VNC WebSocket no Origin | HIGH (CWE-346) | STILL VULNERABLE |
| NEW-16 | No TLS on BMC HTTP | HIGH (CWE-319) | STILL VULNERABLE |
| NEW-26 | Config mutation chain | HIGH (CWE-306) | STILL VULNERABLE |
| NEW-3 | Disk path injection | MEDIUM (CWE-22/73) | STILL VULNERABLE |
| NEW-4 | curl no TLS pinning | MEDIUM (CWE-295) | STILL VULNERABLE |
| NEW-6 | No rate limiting | MEDIUM (CWE-799) | STILL VULNERABLE |
| NEW-9 | SSE no auth | MEDIUM (CWE-306) | STILL VULNERABLE |
| NEW-17/22 | WebSocket frame unbounded | MEDIUM (CWE-400) | STILL VULNERABLE |
| NEW-19 | No CSRF on server fns | MEDIUM (CWE-352) | STILL VULNERABLE |
| NEW-20 | Cirros no checksum | MEDIUM (CWE-494) | STILL VULNERABLE |
| NEW-32 | Incomplete namespace — host /dev accessible | MEDIUM (CWE-269) | NEW |
| NEW-33 | Env leak to QEMU via qrab-init | LOW (CWE-526) | NEW |
| NEW-34 | Config pipe unbounded read | LOW (CWE-400) | NEW |
| NEW-35 | Temp overlay predictable name | LOW (CWE-377) | NEW |

**Critical chain:** #4 + NEW-1 + NEW-16 + NEW-19 + NEW-26. NEW-32 weakens sandbox if chain exploited.

# Security Audit Report

Generated: 2026-04-13 | Round 10

## Changes: New mitigations confirmed (id correlation, memory validation, early-exit). 2 new LOW.

## Summary: 3 HIGH open, 7 MEDIUM, 7 LOW. 7 MITIGATED, 2 RESOLVED.

| # | Finding | Severity | Status |
|---|---------|----------|--------|
| 1 | Unbounded read_line | HIGH | MITIGATED (1 MiB cap) |
| 2 | Socket permissions | HIGH | MITIGATED (chmod 0o600) |
| 4 | No auth on BMC web | HIGH (CWE-306) | STILL VULNERABLE |
| 11 | No unsafe | POSITIVE | CONFIRMED |
| NEW-1 | VNC WebSocket no Origin | HIGH (CWE-346) | STILL VULNERABLE |
| NEW-3 | Disk path injection | MEDIUM (CWE-22/73) | STILL VULNERABLE |
| NEW-4 | curl no TLS pinning | MEDIUM (CWE-295) | STILL VULNERABLE |
| NEW-6 | No rate limiting | MEDIUM (CWE-799) | STILL VULNERABLE |
| NEW-9 | SSE no auth | MEDIUM (CWE-306) | STILL VULNERABLE |
| NEW-12 | Error messages leak state | LOW (CWE-209) | STILL VULNERABLE |
| NEW-16 | No TLS on BMC HTTP | HIGH (CWE-319) | STILL VULNERABLE |
| NEW-17/22 | WebSocket frame unbounded | MEDIUM (CWE-400) | STILL VULNERABLE |
| NEW-19 | No CSRF on server fns | MEDIUM (CWE-352) | STILL VULNERABLE |
| NEW-20 | Cirros no checksum | MEDIUM (CWE-494) | STILL VULNERABLE |
| NEW-24 | No CSP headers | LOW (CWE-16) | STILL VULNERABLE |
| NEW-26 | Config mutation chain | HIGH (CWE-306) | STILL VULNERABLE |
| NEW-27 | Mock server unbounded reads | LOW (CWE-400) | CONFIRMED |
| NEW-28 | Static asset integrity | LOW (CWE-367) | STILL VULNERABLE |
| NEW-29 | execute_raw arbitrary commands | LOW (CWE-20) | STILL PRESENT |
| NEW-30 | QEMU stderr unbounded line | LOW (CWE-400) | NEW |
| NEW-31 | VmBuilder::arg injection surface | LOW (CWE-88) | NEW |

**Critical chain:** #4 + NEW-1 + NEW-16 + NEW-19 + NEW-26.

Already maximally compressed. Table data, CWE refs, severity tags, statuses — all technical substance, nothing to cut.
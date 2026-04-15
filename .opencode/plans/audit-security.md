# Security Audit Report

Generated: 2026-04-13 | Round 6

## Changes This Round

| # | Finding | Change |
|---|---------|--------|
| NEW-17/22 | WebSocket frame + read_line unbounded | **MITIGATED** — MAX_LINE_BYTES=1MiB + 8192-byte WS buffer |
| NEW-34 | Config pipe unbounded read | DOWNGRADED to INFORMATIONAL (parent controls pipe) |

## NEW Findings (Round 6)

| # | Finding | Severity | CWE |
|---|---------|----------|-----|
| NEW-36 | Disk path traversal via server fn — arbitrary host file as VM disk | HIGH | CWE-22 |
| NEW-37 | Config mutation via unauth server fns (set_kvm etc) | MEDIUM | CWE-306 |
| NEW-38 | Namespace incomplete — no pivot_root, no /dev isolation, host fs visible | MEDIUM | CWE-269 |
| NEW-39 | ensure_cirros_image curl download no checksum | MEDIUM | CWE-494 |
| NEW-40 | collect_environ passes all env vars (incl secrets) to QEMU | LOW | CWE-526 |
| NEW-41 | Broadcast channel unbounded subscriber growth (SSE DoS) | LOW | CWE-400 |

## Summary

| Severity | Count |
|----------|-------|
| HIGH | 5 (#1, #2, #4, NEW-1, NEW-36) |
| MEDIUM | 9 |
| LOW | 6 |
| MITIGATED | 2 (NEW-5, NEW-17/22) |
| POSITIVE | 1 (#11 no unsafe in runtime crates) |

**Critical chain:** #4 (no auth) + NEW-36 (disk path traversal) + NEW-38 (no namespace isolation) = unauthenticated arbitrary host file read via QEMU block layer.

**Top 3:** BMC auth, disk path validation, pivot_root + /dev isolation.

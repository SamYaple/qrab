# Security Audit Report

Generated: 2026-04-13

## FINDING 1: Unbounded `read_line` — DoS (HIGH, CWE-400)

No size limit on `BufReader::read_line()`. OOM via endless line without `\n`.

Locations: `lib.rs:53,74,190`, `qga.rs:70,114`, `server.rs:57,72,158,179`

**Mitigation:** `take()` to enforce max 16MB line length.

## FINDING 2: No Socket Permission Control (HIGH, CWE-732)

QMP Unix sockets in `/tmp` with default umask. Any local user can connect.

`projects/manager/src/vm.rs:152`

**Mitigation:** Restrictive umask or private 0700 directory.

## FINDING 3: Predictable Socket Paths — TOCTOU (MEDIUM, CWE-377/367)

`vm.rs:152-159` — PID + counter path. Symlink attack possible.

**Mitigation:** `tempfile::TempDir` with random names.

## FINDING 4: No Auth on BMC Web Interface (HIGH, CWE-306)

`projects/bmc/src/server/vm_actions.rs` — Zero auth on VM control + VNC proxy.

**Mitigation:** Auth middleware. At minimum bind 127.0.0.1 only.

## FINDING 5: Passwords Not Zeroized (MEDIUM, CWE-316)

Generated types have `pub password: String`. Memory dumps leak secrets.

**Mitigation:** `secrecy::SecretString` or `zeroize::Zeroizing<String>` in codegen.

## FINDING 6: QGA Sync ID Truncation (LOW, CWE-330)

`qga.rs:43-46` — `as_nanos() as i64` truncates.

**Mitigation:** `rand::random::<i64>()`.

## FINDING 7: Env Var Controls Binary Path (MEDIUM, CWE-426)

`arch.rs:26-28` — `QEMU_TEST_BIN_*` env vars control spawned binary.

**Mitigation:** Validate path. Document as dev/test only.

## FINDING 8: Arbitrary QEMU Args (MEDIUM, CWE-78 indirect)

`vm.rs:139-141` — `VmBuilder::arg()` accepts anything. QEMU arg injection possible.

**Mitigation:** Typed builder methods instead of raw `arg()`.

## FINDING 9: Firmware Path Not Validated (LOW, CWE-22)

`vm.rs:133-135` — Firmware path passed to QEMU unchecked.

## FINDING 10: Advisory Deps (LOW, CWE-1395)

`paste 1.0.15` unmaintained. `rand 0.8.5` unsound with custom logger.

## FINDING 11: No `unsafe` (Positive)

Zero unsafe blocks in runtime crates.

## FINDING 12: Silent Error Swallowing (LOW, CWE-754)

`lib.rs:193` — Malformed JSON silently dropped. Can desync command/response.

## Summary

| Severity | Findings |
|----------|----------|
| **HIGH** | #1 unbounded read, #2 socket perms, #4 no BMC auth |
| **MEDIUM** | #3 TOCTOU, #5 secrets, #7 env binary, #8 arg injection |
| **LOW** | #6 weak nonce, #9 path traversal, #10 deps, #12 silent errors |

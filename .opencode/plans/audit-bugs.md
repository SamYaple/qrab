# Bug Audit Report

Generated: 2026-04-13

## BUG 1 — Race condition: QMP event interleaving with command responses

**Severity: CRITICAL**

`projects/qapi-protocol/src/lib.rs:182-230`

`read_loop` classifies messages as events (has `"event"` key) or responses (everything else). If unsolicited non-event message arrives, it gets routed as response, pairing with wrong command's reply_tx. Response ordering can break with interleaved events.

**Fix:** Tag requests with IDs (`"id"` field in QMP). Match responses to requests by ID.

---

## BUG 2 — `Qmp::Drop` missing — background tasks leak

**Severity: HIGH**

`projects/qapi-protocol/src/lib.rs:30-38`

`Qmp` stores `JoinHandle`s but has no `Drop` impl. `tokio::select!` is random by default — task could be stuck in `lines.next_line()` indefinitely.

**Fix:** Impl `Drop` for `Qmp` that aborts handles on drop.

---

## BUG 3 — `as i64` truncation in QGA sync ID

**Severity: HIGH**

`projects/qapi-protocol/src/qga.rs:43-46`

`as_nanos()` returns `u128`, cast to `i64` truncates. `unwrap()` panics if clock before epoch.

**Fix:** Use `as_millis() as i64` or `as_secs() as i64`.

---

## BUG 4 — `OnceLock` silently drops second `set_qemu_version` call

**Severity: HIGH**

`projects/qapi/src/lib.rs:42-44`

`.ok()` silently discards failure. Multi-VM same process = second version silently wrong.

**Fix:** Use `AtomicU32` or `Mutex`. Or store version per-`Qmp` instance.

---

## BUG 5 — `VmProcess::Drop` doesn't wait for child — zombie processes

**Severity: MEDIUM**

`projects/manager/src/vm.rs:53-66`

`Drop` calls `child.start_kill()` but never `child.wait()`. Zombie accumulation.

**Fix:** Add `child.try_wait()` loop or blocking `child.wait()` in Drop.

---

## BUG 6 — `.expect()` panic in `MachineConfig::Bios` on ARM

**Severity: MEDIUM** | `projects/manager/src/vm.rs:182`

**Fix:** Return `Result` from `spawn()`.

---

## BUG 7 — Socket poll loop: no early exit on QEMU crash

**Severity: MEDIUM** | `projects/manager/src/vm.rs:262-267`

**Fix:** Poll `child.try_wait()` in loop. Bail if child exited.

---

## BUG 8 — QEMU orphaned on version check failure

**Severity: MEDIUM** | `projects/manager/src/vm.rs:275-288`

**Fix:** Kill child before bailing or wrap in VmProcess earlier.

---

## BUG 9 — Empty tag list panics

**Severity: LOW** | `projects/qapi-codegen/src/main.rs:33-34`

**Fix:** Check `tags.is_empty()` and bail.

---

## BUG 10 — `schema_file.parent().unwrap()` panic

**Severity: LOW** | `projects/qapi-codegen/src/lib.rs:100`

---

## BUG 11 — `unreachable!`/`panic!` in IR processing

**Severity: LOW** | `projects/qapi-codegen/src/qapi_ir/process_qapi.rs` (multiple lines)

**Fix:** Return `Result` with context.

---

## BUG 12 — Thread join masks panics

**Severity: LOW** | `projects/qapi-codegen/src/unified.rs:383, 1428`

**Fix:** `h.join().unwrap_or_else(|e| std::panic::resume_unwind(e))`

---

## BUG 13 — Alternate enum deser order-dependent

**Severity: MEDIUM** | `projects/qapi-macros/src/lib.rs:716-754`

**Fix:** Match on JSON type (`is_string()`, `is_number()`) before trying variants.

---

## BUG 14 — `rust_deprecated_by_bounds` always returns true

**Severity: LOW** | `projects/qapi-macros/src/lib.rs:109-120`

`_dep_ver` computed but unused. Always returns `true`.

**Fix:** `return dep_ver <= min;`

---

## Summary

| # | Bug | Severity | Crate |
|---|-----|----------|-------|
| 1 | QMP response/event interleave race | Critical | qapi-protocol |
| 2 | No Drop on Qmp — task leak | High | qapi-protocol |
| 3 | `as i64` nanos truncation | High | qapi-protocol |
| 4 | OnceLock silently drops multi-VM version | High | qapi |
| 5 | Zombie processes | Medium | manager |
| 6 | `.expect()` panic on ARM+BIOS | Medium | manager |
| 7 | No early-exit on QEMU crash | Medium | manager |
| 8 | QEMU orphaned on version mismatch | Medium | manager |
| 9 | Empty tags unwrap | Low | qapi-codegen |
| 10 | parent().unwrap() | Low | qapi-codegen |
| 11 | panic! in IR processing | Low | qapi-codegen |
| 12 | thread join masks panics | Low | qapi-codegen |
| 13 | Alternate deser order-dependent | Medium | qapi-macros |
| 14 | deprecated_by_bounds broken | Low | qapi-macros |

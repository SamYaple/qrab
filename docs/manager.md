# Manager Crate

High-level typestate VM lifecycle management built on top of `qapi-protocol`.

## Typestate Pattern

```
VmBuilder::new(arch, config)
    .kernel(path)
    .serial(mode)
    .firmware(path)
    .spawn().await?
    -> PreconfigVm                  QEMU running, machine NOT initialized

PreconfigVm
    .execute(&cmd).await?           compile-time PreconfigSafe enforcement
    .start().await?
    -> Vm                           machine initialized, guest running

Vm
    .execute(&cmd).await?           any QapiCommand
    .stop() / .cont() / .reset()   high-level convenience methods
    .shutdown().await               graceful cleanup
```

## Key Files

| File | Content |
|---|---|
| `src/lib.rs` | Re-exports: `Arch`, `MachineConfig`, `VmBuilder`, `PreconfigVm`, `Vm`, `PRECONFIG_MIN_VERSION` |
| `src/vm.rs` | Core typestate implementation (VmBuilder, PreconfigVm, Vm, VmProcess) |
| `src/arch.rs` | `Arch` enum, `MachineConfig`, `detect_version()` |

## VmBuilder

Configures a QEMU instance. Always spawns with `--preconfig`. Options include:

- Architecture and machine type
- Kernel, initrd, kernel command line
- Serial output mode
- Firmware (UEFI/BIOS) path
- Extra raw QEMU arguments

## PreconfigVm

QEMU is running but the machine is not yet initialized. The `execute()` method
has a trait bound requiring `C: QapiCommand + PreconfigSafe` — only commands
that QEMU accepts in preconfig state can be called. This is enforced at
**compile time**.

Calling `start()` sends `x-exit-preconfig`, consumes `self`, and returns `Vm`.

## Vm

Full QMP access with no trait bound restrictions. Current convenience methods:

- `stop()`, `cont()`, `reset()`, `powerdown()`, `quit()`, `shutdown()`

Planned semantic methods that orchestrate multiple QMP calls:

- `add_cpu()`, `snapshot()`, and similar high-level operations

Low-level access remains available: the underlying QMP client is exposed for
direct `execute()` calls with any `QapiCommand`.

## Version Requirements

- `--preconfig` flag requires QEMU >= 3.0.0 (`PRECONFIG_MIN_VERSION`)
- `x-exit-preconfig`: added in 3.0.0, still present in 10.2.0+
- Version is checked after QMP connect; error if below minimum

## Versioning

The `manager` crate will carry its own semantic version independent of the
`qapi` crate, tracking when high-level interfaces change.

## Internal Design

- `VmInner` holds shared state (Qmp client, events, child process, paths)
- `PreconfigVm` and `Vm` wrap `VmInner` in `ManuallyDrop` for ownership
  transfer during state transitions
- `unsafe ManuallyDrop::take` + `mem::forget` avoids Drop during `start()`
- Drop impls provide safety-net cleanup (kill child, remove socket)

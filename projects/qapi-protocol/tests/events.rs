//! Event capture tests: trigger QMP events and validate typed deserialization.

#[allow(dead_code, unused_imports)]
mod common;

#[allow(unused_imports)]
use common::*;
use qapi::*;
use qapi::commands::*;
use qapi::events::*;

// ══════════════════════════════════════════════════════════════════════════════
// Lifecycle events — work with -machine none on all architectures
// ══════════════════════════════════════════════════════════════════════════════

arch_tests!(
    event_stop,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let mut t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        t.execute(&Stop {}).await.unwrap();
        let evt = wait_for_event(&mut t.events, "STOP", event_timeout()).await;
        assert!(evt.is_some(), "Expected STOP event");
        t.shutdown().await;
    }
);

arch_tests!(
    event_resume,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let mut t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        t.execute(&Stop {}).await.unwrap();
        wait_for_event(&mut t.events, "STOP", event_timeout()).await;
        t.execute(&Cont {}).await.unwrap();
        let evt = wait_for_event(&mut t.events, "RESUME", event_timeout()).await;
        assert!(evt.is_some(), "Expected RESUME event");
        t.shutdown().await;
    }
);

arch_tests!(
    event_reset,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let mut t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        t.execute(&SystemReset {}).await.unwrap();
        let evt = wait_for_event(&mut t.events, "RESET", event_timeout()).await;
        assert!(evt.is_some(), "Expected RESET event");
        t.shutdown().await;
    }
);

arch_tests!(
    event_shutdown_via_quit,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let mut t = VmBuilder::new(arch, MachineConfig::None)
            .no_shutdown()
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();

        // quit triggers SHUTDOWN event. With -no-shutdown, QEMU doesn't exit immediately.
        t.execute(&Quit {}).await.unwrap();
        let evt = wait_for_typed_event::<ShutdownEvent>(&mut t.events, event_timeout()).await;
        assert!(evt.is_some(), "Expected SHUTDOWN event");
        let shutdown = evt.unwrap();
        // Host-initiated shutdown (via quit)
        assert_eq!(shutdown.guest, Some(false));
    }
);

// ══════════════════════════════════════════════════════════════════════════════
// POWERDOWN — requires a machine with ACPI (SeaBIOS x86)
// ══════════════════════════════════════════════════════════════════════════════

arch_tests!(event_powerdown, [X86_64], |arch: Arch| async move {
    let mut t = VmBuilder::new(
        arch,
        MachineConfig::Bios {
            memory: "64M".into(),
        },
    )
    .no_shutdown()
    .spawn()
    .await
    .unwrap()
    .start()
    .await
    .unwrap();

    t.execute(&SystemPowerdown {}).await.unwrap();
    let evt = wait_for_event(&mut t.events, "POWERDOWN", event_timeout()).await;
    assert!(evt.is_some(), "Expected POWERDOWN event");
    t.shutdown().await;
});

// ══════════════════════════════════════════════════════════════════════════════
// Stop + Resume typed deserialization
// ══════════════════════════════════════════════════════════════════════════════

arch_tests!(
    event_stop_typed,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let mut t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        t.execute(&Stop {}).await.unwrap();
        let evt = wait_for_typed_event::<StopEvent>(&mut t.events, event_timeout()).await;
        assert!(evt.is_some(), "Expected typed STOP event");
        t.shutdown().await;
    }
);

arch_tests!(
    event_resume_typed,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let mut t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        t.execute(&Stop {}).await.unwrap();
        wait_for_event(&mut t.events, "STOP", event_timeout()).await;
        t.execute(&Cont {}).await.unwrap();
        let evt = wait_for_typed_event::<ResumeEvent>(&mut t.events, event_timeout()).await;
        assert!(evt.is_some(), "Expected typed RESUME event");
        t.shutdown().await;
    }
);

arch_tests!(
    event_reset_typed,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let mut t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        t.execute(&SystemReset {}).await.unwrap();
        let evt = wait_for_typed_event::<ResetEvent>(&mut t.events, event_timeout()).await;
        assert!(evt.is_some(), "Expected typed RESET event");
        let reset = evt.unwrap();
        // Reset should indicate not guest-initiated
        assert_eq!(reset.guest, Some(false));
        t.shutdown().await;
    }
);

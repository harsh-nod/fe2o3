use super::*;
use crate::queue::submit::{
    GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1, GFX942_CWSR_TOTAL_BYTES_V1, GFX942_CWSR_XCC_COUNT_V1,
};
use std::os::fd::AsFd;

type DiagnosticShadowFixture = (
    LinuxCwsrShadowPagesV1,
    Vec<Box<[u8; 4096]>>,
    Box<[u8; 4096]>,
    LinuxQueueExceptionEventV1,
    std::fs::File,
);

fn diagnostic_shadow_fixture() -> DiagnosticShadowFixture {
    let file = std::fs::File::open("/dev/null").unwrap();
    let binding = QueueExceptionBindingV1 {
        event_id: KfdSignalEventIdV1::new(7).unwrap(),
        opener_pid: std::process::id(),
        raw_fd: file.as_fd().as_raw_fd(),
    };
    let mut storage: Vec<Box<[u8; 4096]>> = (0..GFX942_CWSR_SHADOW_PAGES_V1)
        .map(|_| Box::new([0_u8; 4096]))
        .collect();
    let mut payload_storage = Box::new([0_u8; 4096]);
    let payload_page = NonNull::new(payload_storage.as_mut_ptr().cast::<c_void>()).unwrap();
    let payload = NonNull::new(payload_storage.as_mut_ptr().cast::<u64>()).unwrap();
    let payload_address =
        KfdQueueExceptionPayloadAddressV1::new(payload.as_ptr() as usize as u64).unwrap();
    for xcc in 0..GFX942_CWSR_XCC_COUNT_V1 {
        let page = &mut storage[xcc * CWSR_CONTROL_STACK_PAGES_PER_XCC_V1];
        let header =
            crate::queue::submit::gfx942_cwsr_header_bytes(xcc, payload_address, binding.event_id)
                .unwrap();
        page[..header.len()].copy_from_slice(&header);
    }
    let pages: [NonNull<c_void>; GFX942_CWSR_SHADOW_PAGES_V1] = storage
        .iter_mut()
        .map(|page| NonNull::new(page.as_mut_ptr().cast::<c_void>()).unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let shadows = LinuxCwsrShadowPagesV1 {
        pages,
        payload_page,
        payload,
        binding,
        page_bytes: 4096,
        payload_page_active: true,
        active: true,
    };
    let event = LinuxQueueExceptionEventV1 {
        binding,
        active: true,
        poisoned: false,
        observation_used: false,
    };
    (shadows, storage, payload_storage, event, file)
}

type MappedDiagnosticShadowFixture = (
    LinuxCwsrShadowPagesV1,
    Vec<Box<[u8; 4096]>>,
    LinuxQueueExceptionEventV1,
    std::fs::File,
);

pub(super) fn mapped_diagnostic_shadow_fixture() -> MappedDiagnosticShadowFixture {
    let file = std::fs::File::open("/dev/null").unwrap();
    let binding = QueueExceptionBindingV1 {
        event_id: KfdSignalEventIdV1::new(7).unwrap(),
        opener_pid: std::process::id(),
        raw_fd: file.as_fd().as_raw_fd(),
    };
    let mut storage: Vec<Box<[u8; 4096]>> = (0..GFX942_CWSR_SHADOW_PAGES_V1)
        .map(|_| Box::new([0_u8; 4096]))
        .collect();
    let payload_page = map_cwsr_payload_page(4096).unwrap();
    let payload = NonNull::new(payload_page.as_ptr().cast::<u64>()).unwrap();
    let payload_address =
        KfdQueueExceptionPayloadAddressV1::new(payload.as_ptr() as usize as u64).unwrap();
    for xcc in 0..GFX942_CWSR_XCC_COUNT_V1 {
        let page = &mut storage[xcc * CWSR_CONTROL_STACK_PAGES_PER_XCC_V1];
        let header =
            crate::queue::submit::gfx942_cwsr_header_bytes(xcc, payload_address, binding.event_id)
                .unwrap();
        page[..header.len()].copy_from_slice(&header);
    }
    let pages: [NonNull<c_void>; GFX942_CWSR_SHADOW_PAGES_V1] = storage
        .iter_mut()
        .map(|page| NonNull::new(page.as_mut_ptr().cast::<c_void>()).unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let shadows = LinuxCwsrShadowPagesV1 {
        pages,
        payload_page,
        payload,
        binding,
        page_bytes: 4096,
        payload_page_active: true,
        active: true,
    };
    let event = LinuxQueueExceptionEventV1 {
        binding,
        active: true,
        poisoned: false,
        observation_used: false,
    };
    (shadows, storage, event, file)
}

#[test]
fn shadow_plan_is_exact_and_hostile_geometry_fails_closed() {
    let plan =
        CwsrShadowPlanV1::from_owned_reservation(0x1_0000_0000, GFX942_CWSR_TOTAL_BYTES_V1, 4096)
            .unwrap();
    assert_eq!(plan.bytes, 0xb16_7000);
    assert_eq!(plan.page_bytes, 4096);
    let mut addresses = [0_u64; GFX942_CWSR_SHADOW_PAGES_V1];
    for xcc in 0..GFX942_CWSR_XCC_COUNT_V1 {
        for page in 0..CWSR_CONTROL_STACK_PAGES_PER_XCC_V1 {
            let ordinal = xcc * CWSR_CONTROL_STACK_PAGES_PER_XCC_V1 + page;
            addresses[ordinal] =
                plan.base + (xcc * GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1 + page * 4096) as u64;
            assert!(addresses[ordinal].is_multiple_of(4096));
        }
    }
    assert!(addresses.windows(2).all(|pair| pair[0] < pair[1]));

    for result in [
        CwsrShadowPlanV1::from_owned_reservation(plan.base + 1, GFX942_CWSR_TOTAL_BYTES_V1, 4096),
        CwsrShadowPlanV1::from_owned_reservation(
            plan.base,
            GFX942_CWSR_TOTAL_BYTES_V1 - 4096,
            4096,
        ),
        CwsrShadowPlanV1::from_owned_reservation(plan.base, GFX942_CWSR_TOTAL_BYTES_V1, 8192),
        CwsrShadowPlanV1::from_owned_reservation(u64::MAX - 4095, GFX942_CWSR_TOTAL_BYTES_V1, 4096),
    ] {
        assert!(result.is_err());
    }
}

#[test]
fn unpublished_payload_is_unmapped_when_final_admission_fails() {
    let (shadows, mut storage, _event, _file) = mapped_diagnostic_shadow_fixture();
    storage[0][0] ^= 1;
    // Admission returns only after release succeeds; a release failure is
    // process-terminal because this call consumes the final page owner.
    assert!(admit_installed_cwsr_shadows(shadows).is_err());
}

#[test]
fn payload_is_unmapped_at_event_destroy_boundary_before_later_cleanup() {
    let (shadows, _storage, _event, file) = mapped_diagnostic_shadow_fixture();
    let binding = shadows.binding;
    let after_event = shadows
        .after_event_destroy(LinuxDestroyedQueueExceptionEventV1 { binding })
        .unwrap();
    // A successful transition means zero/protect/munmap completed. Check
    // the retained state instead of racing another thread's address reuse.
    assert!(!after_event.shadows.payload_page_active);

    let ready = after_event
        .after_runtime_destroy(LinuxKfdRuntimeDisabledV1 {
            binding: KfdRuntimeBindingV1 {
                opener_pid: std::process::id(),
                raw_fd: file.as_fd().as_raw_fd(),
            },
            completion_pending: true,
        })
        .unwrap();
    ready.complete().unwrap();
}

#[test]
fn terminal_unpublished_cleanup_retains_metadata_and_rejects_publication() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let (shadows, _storage, _event, _file) = mapped_diagnostic_shadow_fixture();
    let identity = (
        shadows.pages,
        shadows.payload_page,
        shadows.payload,
        shadows.binding,
        shadows.page_bytes,
    );
    let mut owner = LinuxUnpublishedCwsrShadowPagesV1 {
        shadows: Some(shadows),
        terminal_payload_released: false,
    };
    owner.cleanup_payload_for_terminal_retention();
    assert!(owner.terminal_payload_released);
    let shadows = owner.shadows.as_ref().unwrap();
    assert_eq!(
        identity,
        (
            shadows.pages,
            shadows.payload_page,
            shadows.payload,
            shadows.binding,
            shadows.page_bytes
        )
    );
    assert!(!shadows.active && !shadows.payload_page_active);
    assert!(shadows.observe_reason().is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| owner.shadows())).is_err());
    owner.cleanup_payload_for_terminal_retention();
    // The rejected consuming publication also drops the disposed wrapper;
    // a second payload release would abort this process.
    assert!(
        catch_unwind(AssertUnwindSafe(
            || owner.publish_for_native_queue_creation()
        ))
        .is_err()
    );
}

#[test]
fn terminal_unpublished_cleanup_failure_aborts_instead_of_losing_custody() {
    const CHILD_ENV: &str = "FE2O3_TEST_TERMINAL_UNPUBLISHED_PAYLOAD_ABORT";
    if std::env::var_os(CHILD_ENV).is_some() {
        let (mut shadows, _storage, _event, _file) = mapped_diagnostic_shadow_fixture();
        shadows.page_bytes = 0;
        let mut owner = LinuxUnpublishedCwsrShadowPagesV1 {
            shadows: Some(shadows),
            terminal_payload_released: false,
        };
        owner.cleanup_payload_for_terminal_retention();
        panic!("terminal payload cleanup unexpectedly returned");
    }
    use std::os::unix::process::ExitStatusExt;
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("queue_linux::tests::terminal_unpublished_cleanup_failure_aborts_instead_of_losing_custody")
        .env(CHILD_ENV, "1").status().unwrap();
    assert_eq!(status.signal(), Some(libc::SIGABRT));
}

#[test]
fn unpublished_custody_unmaps_payload_on_early_return() {
    let (shadows, _storage, _event, _file) = mapped_diagnostic_shadow_fixture();
    // The unpublished owner aborts if release fails. Returning from this
    // drop therefore proves the page completed its release path.
    drop(LinuxUnpublishedCwsrShadowPagesV1 {
        shadows: Some(shadows),
        terminal_payload_released: false,
    });
}

#[test]
fn unpublished_custody_cleanup_failure_is_process_terminal() {
    const CHILD_ENV: &str = "FE2O3_TEST_UNPUBLISHED_CWSR_PAYLOAD_RELEASE_ABORT";
    if std::env::var_os(CHILD_ENV).is_some() {
        let (mut shadows, _storage, _event, _file) = mapped_diagnostic_shadow_fixture();
        shadows.page_bytes = 0;
        drop(LinuxUnpublishedCwsrShadowPagesV1 {
            shadows: Some(shadows),
            terminal_payload_released: false,
        });
        panic!("unpublished payload cleanup failure returned instead of terminating");
    }

    use std::os::unix::process::ExitStatusExt;
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("queue_linux::tests::unpublished_custody_cleanup_failure_is_process_terminal")
        .arg("--nocapture")
        .env(CHILD_ENV, "1")
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(libc::SIGABRT));
}

#[test]
fn payload_release_failure_after_event_destroy_is_process_terminal() {
    const CHILD_ENV: &str = "FE2O3_TEST_CWSR_PAYLOAD_RELEASE_ABORT";
    if std::env::var_os(CHILD_ENV).is_some() {
        let (mut shadows, _storage, _event, _file) = mapped_diagnostic_shadow_fixture();
        let binding = shadows.binding;
        // A zero-length mprotect/munmap request cannot complete release of
        // the retained mapping. The production transition must abort
        // rather than return after consuming its only owner.
        shadows.page_bytes = 0;
        let _ = shadows.after_event_destroy(LinuxDestroyedQueueExceptionEventV1 { binding });
        panic!("payload cleanup failure returned instead of terminating");
    }

    use std::os::unix::process::ExitStatusExt;
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("queue_linux::tests::payload_release_failure_after_event_destroy_is_process_terminal")
        .arg("--nocapture")
        .env(CHILD_ENV, "1")
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(libc::SIGABRT));
}

#[test]
fn wait_and_payload_must_agree_and_unknown_reasons_are_rejected() {
    let empty = KfdQueueExceptionReasonV1::from_untrusted_wire(0).unwrap();
    let fault = KfdQueueExceptionReasonV1::from_untrusted_wire(1).unwrap();
    assert_eq!(
        admit_queue_exception_wait(KfdWaitResultV1::Timeout, empty).unwrap(),
        QueueExceptionWaitObservationV1::NoExceptionAtObservation
    );
    assert_eq!(
        admit_queue_exception_wait(KfdWaitResultV1::Complete, fault).unwrap(),
        QueueExceptionWaitObservationV1::Exception(fault)
    );
    assert!(admit_queue_exception_wait(KfdWaitResultV1::Complete, empty).is_err());
    assert!(admit_queue_exception_wait(KfdWaitResultV1::Timeout, fault).is_err());
    assert!(KfdQueueExceptionReasonV1::from_untrusted_wire(1 << 63).is_none());
}

#[test]
fn timeout_diagnostic_admits_reason_but_rejects_malformed_shadow_state() {
    let (shadows, mut storage, _payload_storage, event, file) = diagnostic_shadow_fixture();
    // SAFETY: the fixture retains the aligned writable payload word.
    unsafe { core::ptr::write_volatile(shadows.payload.as_ptr(), 1_u64.to_le()) };
    assert!(
        event
            .validate_live_with_shadows_for_diagnostic(file.as_fd(), std::process::id(), &shadows,)
            .is_ok()
    );
    assert!(
        event
            .validate_live_with_shadows(file.as_fd(), std::process::id(), &shadows)
            .is_err()
    );
    assert_eq!(shadows.observe_reason().unwrap().get(), 1);

    // SAFETY: the fixture retains the aligned writable payload word.
    unsafe { core::ptr::write_volatile(shadows.payload.as_ptr(), (1_u64 << 63).to_le()) };
    assert!(shadows.observe_reason().is_err());

    storage[CWSR_CONTROL_STACK_PAGES_PER_XCC_V1][0] ^= 1;
    assert!(
        event
            .validate_live_with_shadows_for_diagnostic(file.as_fd(), std::process::id(), &shadows,)
            .is_err()
    );
}

#[test]
fn runtime_queue_event_order_is_linear_and_hostile_reordering_fails() {
    use KfdRuntimeLifecyclePhaseV1 as P;
    let mut phase = P::EnabledBeforeQueue;
    phase = admit_runtime_transition(phase, P::EnabledBeforeQueue, P::QueueLive).unwrap();
    phase = admit_runtime_transition(phase, P::QueueLive, P::QueueDestroyed).unwrap();
    phase = admit_runtime_transition(phase, P::QueueDestroyed, P::EventDestroyed).unwrap();
    phase = admit_runtime_transition(phase, P::EventDestroyed, P::Disabled).unwrap();
    assert_eq!(phase, P::Disabled);

    assert!(
        admit_runtime_transition(P::EnabledBeforeQueue, P::QueueLive, P::QueueDestroyed).is_err()
    );
    assert!(admit_runtime_transition(P::QueueLive, P::EventDestroyed, P::Disabled).is_err());
    assert!(admit_runtime_transition(P::QueueDestroyed, P::EventDestroyed, P::Disabled).is_err());
    assert!(admit_runtime_transition(P::Disabled, P::EnabledBeforeQueue, P::QueueLive).is_err());
}

#[test]
fn queue_exception_observation_cannot_be_reused() {
    let mut used = false;
    assert!(begin_one_shot_observation(&mut used).is_ok());
    assert!(used);
    assert!(begin_one_shot_observation(&mut used).is_err());
}

#[test]
fn terminal_teardown_arm_clears_only_after_confirmed_success() {
    let gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
    let first = arm_runtime_gate_for_terminal_teardown(&gate);
    let second = arm_runtime_gate_for_terminal_teardown(&gate);
    assert_eq!(lock_runtime_gate_v1(&gate).teardown_arms, 2);
    first.confirm_destroyed();
    assert_eq!(lock_runtime_gate_v1(&gate).teardown_arms, 1);
    assert!(!lock_runtime_gate_v1(&gate).permanently_poisoned);
    second.confirm_destroyed();
    assert_eq!(lock_runtime_gate_v1(&gate).teardown_arms, 0);
    assert!(!lock_runtime_gate_v1(&gate).permanently_poisoned);

    let arm = arm_runtime_gate_for_terminal_teardown(&gate);
    assert_eq!(lock_runtime_gate_v1(&gate).teardown_arms, 1);
    drop(arm);
    assert_eq!(lock_runtime_gate_v1(&gate).teardown_arms, 0);
    assert!(lock_runtime_gate_v1(&gate).permanently_poisoned);

    let panic_gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
    let result = std::panic::catch_unwind(|| {
        let _arm = arm_runtime_gate_for_terminal_teardown(&panic_gate);
        panic!("simulated teardown panic");
    });
    assert!(result.is_err());
    assert_eq!(lock_runtime_gate_v1(&panic_gate).teardown_arms, 0);
    assert!(lock_runtime_gate_v1(&panic_gate).permanently_poisoned);
}

#[test]
fn checked_creation_finalization_requires_healthy_admitted_runtime() {
    for fault in 0..7 {
        let gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
        {
            let mut state = lock_runtime_gate_v1(&gate);
            assert!(state.admit_runtime(41).unwrap());
            state.runtime.commit_first_enabled(41);
        }
        let mut arm = arm_runtime_gate_for_terminal_creation(&gate).unwrap();
        {
            let mut state = lock_runtime_gate_v1(&gate);
            assert!(state.admit_runtime(41).is_err());
            match fault {
                1 => state.poison(),
                2 => {
                    state.arm_teardown();
                }
                3 => state.runtime = ProcessKfdRuntimeStateV1::Disabled,
                4 => {
                    state.runtime = ProcessKfdRuntimeStateV1::Enabled {
                        opener_pid: 41,
                        leases: 0,
                    }
                }
                5 => state.creation_in_flight = false,
                _ => (),
            }
        }
        let result = arm.finish_checked(if fault == 6 { 42 } else { 41 });
        if fault == 0 {
            result.unwrap();
            assert!(!lock_runtime_gate_v1(&gate).is_blocked());
            assert!(arm.finish_checked(41).is_err());
        } else {
            assert!(result.is_err());
        }
        assert!(lock_runtime_gate_v1(&gate).permanently_poisoned);
        drop(arm);
        assert!(lock_runtime_gate_v1(&gate).permanently_poisoned);
    }
}

#[test]
fn checked_creation_finish_disarms_once_after_runtime_admission() {
    let gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
    {
        let mut state = lock_runtime_gate_v1(&gate);
        assert!(state.admit_runtime(41).unwrap());
        state.runtime.commit_first_enabled(41);
    }
    let mut arm = arm_runtime_gate_for_terminal_creation(&gate).unwrap();
    arm.finish_checked(41).unwrap();
    drop(arm);
    let mut state = lock_runtime_gate_v1(&gate);
    assert!(!state.is_blocked());
    assert!(!state.admit_runtime(41).unwrap());
    assert_eq!(
        state.runtime,
        ProcessKfdRuntimeStateV1::Enabled {
            opener_pid: 41,
            leases: 2
        }
    );
}

#[test]
fn terminal_creation_arm_poisons_on_drop_or_unwind_and_disarms_only_on_success() {
    let successful_gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
    let successful_arm = arm_runtime_gate_for_terminal_creation(&successful_gate).unwrap();
    {
        let mut gate = lock_runtime_gate_v1(&successful_gate);
        assert!(gate.creation_in_flight);
        assert!(gate.admit_runtime(41).is_err());
        assert!(gate.arm_creation().is_err());
    }
    successful_arm.disarm();
    {
        let mut gate = lock_runtime_gate_v1(&successful_gate);
        assert!(!gate.creation_in_flight);
        assert!(!gate.permanently_poisoned);
        assert!(gate.admit_runtime(41).unwrap());
    }

    let dropped_gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
    drop(arm_runtime_gate_for_terminal_creation(&dropped_gate).unwrap());
    {
        let gate = lock_runtime_gate_v1(&dropped_gate);
        assert!(!gate.creation_in_flight);
        assert!(gate.permanently_poisoned);
    }

    let panic_gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
    let result = std::panic::catch_unwind(|| {
        let _arm = arm_runtime_gate_for_terminal_creation(&panic_gate).unwrap();
        panic!("simulated creation panic");
    });
    assert!(result.is_err());
    {
        let gate = lock_runtime_gate_v1(&panic_gate);
        assert!(!gate.creation_in_flight);
        assert!(gate.permanently_poisoned);
    }

    let poisoned_gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
    lock_runtime_gate_v1(&poisoned_gate).poison();
    assert!(arm_runtime_gate_for_terminal_creation(&poisoned_gate).is_err());
    assert!(lock_runtime_gate_v1(&poisoned_gate).permanently_poisoned);

    let poisoned_while_armed_gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
    let arm = arm_runtime_gate_for_terminal_creation(&poisoned_while_armed_gate).unwrap();
    lock_runtime_gate_v1(&poisoned_while_armed_gate).poison();
    arm.disarm();
    let gate = lock_runtime_gate_v1(&poisoned_while_armed_gate);
    assert!(!gate.creation_in_flight);
    assert!(gate.permanently_poisoned);
}

#[test]
fn teardown_arm_attempt_after_admission_check_linearizes_after_lease() {
    use std::sync::{Arc, Barrier, mpsc};

    let pid = 41;
    let gate = Arc::new(Mutex::new(ProcessGlobalKfdRuntimeGateV1::new()));
    let start_arm = Arc::new(Barrier::new(2));
    let (attempted_tx, attempted_rx) = mpsc::sync_channel(0);
    let (armed_tx, armed_rx) = mpsc::sync_channel(0);
    let (release_tx, release_rx) = mpsc::sync_channel(0);

    let mut admission = lock_runtime_gate_v1(&gate);
    assert!(!admission.is_blocked());
    let worker_gate = Arc::clone(&gate);
    let worker_barrier = Arc::clone(&start_arm);
    let worker = std::thread::spawn(move || {
        worker_barrier.wait();
        attempted_tx.send(()).unwrap();
        let arm = arm_runtime_gate_for_terminal_teardown(&worker_gate);
        armed_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        arm.confirm_destroyed();
    });

    start_arm.wait();
    attempted_rx.recv().unwrap();
    // This models the former gap between the final arm check and the lease
    // join. The arm thread has started, but the shared gate lock keeps its
    // transition ordered after this admission.
    assert_eq!(admission.teardown_arms, 0);
    assert!(admission.runtime.join_enabled(pid).unwrap());
    admission.runtime.commit_first_enabled(pid);
    drop(admission);

    armed_rx.recv().unwrap();
    let mut blocked_admission = lock_runtime_gate_v1(&gate);
    assert!(blocked_admission.is_blocked());
    assert!(matches!(
        blocked_admission.admit_runtime(pid),
        Err(LinuxDoorbellErrorV1::Runtime("process-global gate blocked"))
    ));
    drop(blocked_admission);
    release_tx.send(()).unwrap();
    worker.join().unwrap();

    let gate = lock_runtime_gate_v1(&gate);
    assert_eq!(gate.teardown_arms, 0);
    assert!(!gate.permanently_poisoned);
    assert_eq!(
        gate.runtime,
        ProcessKfdRuntimeStateV1::Enabled {
            opener_pid: pid,
            leases: 1,
        }
    );
}

#[test]
fn process_runtime_context_multiplexes_independent_queue_leases() {
    let pid = 41;
    let mut state = ProcessKfdRuntimeStateV1::Disabled;
    assert!(state.join_enabled(pid).unwrap());
    state.commit_first_enabled(pid);
    assert!(!state.join_enabled(pid).unwrap());
    assert!(!state.join_enabled(pid).unwrap());
    assert_eq!(
        state,
        ProcessKfdRuntimeStateV1::Enabled {
            opener_pid: pid,
            leases: 3,
        }
    );

    assert!(!state.release_plan(pid).unwrap());
    assert!(!state.release_plan(pid).unwrap());
    assert!(state.release_plan(pid).unwrap());
    state.commit_last_disabled();
    assert_eq!(state, ProcessKfdRuntimeStateV1::Disabled);
}

#[test]
fn process_runtime_context_rejects_cross_process_and_poisoned_joins() {
    let mut state = ProcessKfdRuntimeStateV1::Enabled {
        opener_pid: 17,
        leases: 1,
    };
    assert!(matches!(
        state.join_enabled(18),
        Err(LinuxDoorbellErrorV1::ProcessChanged)
    ));
    assert!(matches!(
        state.release_plan(18),
        Err(LinuxDoorbellErrorV1::ProcessChanged)
    ));
    state.poison();
    assert!(matches!(
        state.join_enabled(17),
        Err(LinuxDoorbellErrorV1::Runtime(
            "process runtime context poisoned"
        ))
    ));
}

#[test]
fn destroy_event_setter_owns_the_wire_record_not_reference_bytes() {
    let source = include_str!("../queue_linux.rs");
    let production = source.split("\n#[cfg(test)]\nmod tests").next().unwrap();
    assert!(production.contains("Setter::<DESTROY_EVENT_OPCODE, _>::new(args)"));
    assert!(!production.contains("Setter::<DESTROY_EVENT_OPCODE, _>::new(&args)"));
    assert!(production.contains("Setter::<UPDATE_QUEUE_OPCODE, _>::new(*args)"));
    assert!(!production.contains("Setter::<UPDATE_QUEUE_OPCODE, _>::new(args)"));
}

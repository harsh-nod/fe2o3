//! Pure state/process-classifier controls. No KFD fd, ioctl, native memory,
//! runtime enable, trap installation, queue, fork or GPU execution occurs.

use super::super::arm_runtime_gate_for_terminal_teardown;
use super::*;

fn pid() -> u32 {
    std::process::id()
}

fn assert_runtime_error<T>(result: Result<T, LinuxDoorbellErrorV1>, expected: &str) {
    match result {
        Err(LinuxDoorbellErrorV1::Runtime(actual)) => assert_eq!(actual, expected),
        _ => panic!("expected the designated runtime refusal"),
    }
}

#[test]
fn ordinary_plain_refcounts_and_disable_plan_are_unchanged() {
    let mut gate = ProcessGlobalKfdRuntimeGateV1::new();
    assert!(gate.admit_runtime(pid()).unwrap());
    gate.runtime.commit_first_enabled(pid());
    assert!(!gate.admit_runtime(pid()).unwrap());
    assert!(!gate.admit_runtime(pid()).unwrap());
    assert_eq!(
        gate.runtime,
        ProcessKfdRuntimeStateV1::Enabled {
            opener_pid: pid(),
            leases: 3
        }
    );
    assert!(!gate.runtime.release_plan(pid()).unwrap());
    assert!(!gate.runtime.release_plan(pid()).unwrap());
    assert!(gate.runtime.release_plan(pid()).unwrap());
    gate.runtime.commit_last_disabled();
    assert_eq!(gate.runtime, ProcessKfdRuntimeStateV1::Disabled);
    assert_eq!(gate.next_debug_reservation, 1);
}

#[test]
fn plain_first_refuses_debug_without_changing_lease_or_serial() {
    let mut gate = ProcessGlobalKfdRuntimeGateV1::new();
    assert!(gate.admit_runtime(pid()).unwrap());
    gate.runtime.commit_first_enabled(pid());
    let before = gate.runtime;
    assert_runtime_error(
        gate.reserve_debug_profile_v1(pid()),
        "plain/debug runtime profile conflict",
    );
    assert_eq!(gate.runtime, before);
    assert_eq!(gate.next_debug_reservation, 1);
    assert!(!gate.permanently_poisoned);
}

#[test]
fn debug_first_refuses_plain_enable_and_plain_release_in_both_phases() {
    for expose in [false, true] {
        let mut gate = ProcessGlobalKfdRuntimeGateV1::new();
        let id = gate.reserve_debug_profile_v1(pid()).unwrap();
        if expose {
            gate.begin_debug_external_transition_v1(pid(), id).unwrap();
        }
        let before = gate.runtime;
        assert_runtime_error(
            gate.admit_runtime(pid()),
            "plain/debug runtime profile conflict",
        );
        assert_runtime_error(
            gate.runtime.release_plan(pid()),
            "plain/debug runtime profile conflict",
        );
        assert_eq!(gate.runtime, before);
        assert!(!gate.permanently_poisoned);
    }
}

#[test]
fn a_second_debug_reservation_refuses_without_consuming_an_identity() {
    let mut gate = ProcessGlobalKfdRuntimeGateV1::new();
    let id = gate.reserve_debug_profile_v1(pid()).unwrap();
    let before = gate.runtime;
    assert_runtime_error(
        gate.reserve_debug_profile_v1(pid()),
        "debug runtime reservation already held",
    );
    assert_eq!(gate.runtime, before);
    assert_eq!(gate.next_debug_reservation, id + 1);
}

#[test]
fn unexposed_owner_drop_rolls_back_only_its_claim_and_identity_is_not_reused() {
    let gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
    let binding = AtomicU32::new(0);
    let first = DebugRuntimeReservationV1::reserve(&gate, &binding, pid()).unwrap();
    assert_eq!(first.reservation_id, 1);
    drop(first);
    assert_eq!(
        lock_runtime_gate_v1(&gate).runtime,
        ProcessKfdRuntimeStateV1::Disabled
    );
    let second = DebugRuntimeReservationV1::reserve(&gate, &binding, pid()).unwrap();
    assert_eq!(second.reservation_id, 2);
    drop(second);
    assert_eq!(binding.load(Ordering::Acquire), pid()); // sticky after rollback
    assert!(lock_runtime_gate_v1(&gate).admit_runtime(pid()).unwrap());
}

#[test]
fn moving_the_owner_preserves_exact_reservation_identity() {
    let gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
    let binding = AtomicU32::new(0);
    let owner = DebugRuntimeReservationV1::reserve(&gate, &binding, pid()).unwrap();
    let id = owner.reservation_id;
    let mut moved = Box::new(owner);
    moved.begin_external_transition().unwrap();
    assert_eq!(
        lock_runtime_gate_v1(&gate).runtime,
        ProcessKfdRuntimeStateV1::DebugReserved {
            opener_pid: pid(),
            reservation_id: id,
            exposed: true,
        }
    );
    drop(moved);
    assert!(lock_runtime_gate_v1(&gate).permanently_poisoned);
}

#[test]
fn exposed_drop_poison_blocks_every_later_profile_without_native_calls() {
    let gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
    let binding = AtomicU32::new(0);
    let mut owner = DebugRuntimeReservationV1::reserve(&gate, &binding, pid()).unwrap();
    owner.begin_external_transition().unwrap();
    drop(owner);
    let mut state = lock_runtime_gate_v1(&gate);
    assert_eq!(state.runtime, ProcessKfdRuntimeStateV1::Poisoned);
    assert!(state.permanently_poisoned);
    assert_runtime_error(state.admit_runtime(pid()), "process-global gate poisoned");
    assert_runtime_error(
        state.reserve_debug_profile_v1(pid()),
        "process-global gate poisoned",
    );
}

#[test]
fn repeated_exposure_or_wrong_identity_poison_instead_of_rebinding() {
    for wrong_identity in [false, true] {
        let mut gate = ProcessGlobalKfdRuntimeGateV1::new();
        let id = gate.reserve_debug_profile_v1(pid()).unwrap();
        if !wrong_identity {
            gate.begin_debug_external_transition_v1(pid(), id).unwrap();
        }
        assert_runtime_error(
            gate.begin_debug_external_transition_v1(
                pid(),
                if wrong_identity { id + 1 } else { id },
            ),
            "debug runtime reservation identity or phase",
        );
        assert!(gate.permanently_poisoned);
        assert_eq!(gate.runtime, ProcessKfdRuntimeStateV1::Poisoned);
    }
}

#[test]
fn mismatched_drop_never_releases_some_other_claim() {
    let mut gate = ProcessGlobalKfdRuntimeGateV1::new();
    let id = gate.reserve_debug_profile_v1(pid()).unwrap();
    gate.drop_debug_reservation_v1(pid(), id + 1);
    assert!(gate.permanently_poisoned);
    assert_eq!(gate.runtime, ProcessKfdRuntimeStateV1::Poisoned);
}

#[test]
fn identity_zero_and_exhaustion_refuse_before_state_or_counter_change() {
    for next in [0, u64::MAX] {
        let mut gate = ProcessGlobalKfdRuntimeGateV1::new();
        gate.next_debug_reservation = next;
        assert_runtime_error(
            gate.reserve_debug_profile_v1(pid()),
            "debug runtime reservation capacity",
        );
        assert_eq!(gate.runtime, ProcessKfdRuntimeStateV1::Disabled);
        assert_eq!(gate.next_debug_reservation, next);
        assert!(!gate.permanently_poisoned);
    }
}

#[test]
fn teardown_arms_exclude_reservation_and_survive_clean_reservation_rollback() {
    let gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
    let binding = AtomicU32::new(0);
    let arm = arm_runtime_gate_for_terminal_teardown(&gate);
    assert_runtime_error(
        DebugRuntimeReservationV1::reserve(&gate, &binding, pid()),
        "process-global gate poisoned",
    );
    assert_eq!(lock_runtime_gate_v1(&gate).next_debug_reservation, 1);
    arm.confirm_destroyed();
    let owner = DebugRuntimeReservationV1::reserve(&gate, &binding, pid()).unwrap();
    let arm = arm_runtime_gate_for_terminal_teardown(&gate);
    drop(owner);
    let state = lock_runtime_gate_v1(&gate);
    assert_eq!(state.runtime, ProcessKfdRuntimeStateV1::Disabled);
    assert_eq!(state.teardown_arms, 1);
    assert!(state.is_blocked());
    drop(state);
    arm.confirm_destroyed();
    assert!(!lock_runtime_gate_v1(&gate).is_blocked());
}

#[test]
fn exposure_is_refused_while_teardown_is_in_progress() {
    let gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
    let binding = AtomicU32::new(0);
    let mut owner = DebugRuntimeReservationV1::reserve(&gate, &binding, pid()).unwrap();
    let arm = arm_runtime_gate_for_terminal_teardown(&gate);
    assert_runtime_error(
        owner.begin_external_transition(),
        "process-global gate poisoned",
    );
    assert!(matches!(
        lock_runtime_gate_v1(&gate).runtime,
        ProcessKfdRuntimeStateV1::DebugReserved { exposed: false, .. }
    ));
    drop(owner); // still unexposed; safe rollback does not remove the arm
    arm.confirm_destroyed();
    assert!(!lock_runtime_gate_v1(&gate).is_blocked());
}

#[test]
fn unconfirmed_teardown_cannot_be_undone_by_reservation_drop() {
    let gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
    let binding = AtomicU32::new(0);
    let owner = DebugRuntimeReservationV1::reserve(&gate, &binding, pid()).unwrap();
    drop(arm_runtime_gate_for_terminal_teardown(&gate));
    drop(owner);
    assert!(lock_runtime_gate_v1(&gate).permanently_poisoned);
    assert_eq!(
        lock_runtime_gate_v1(&gate).runtime,
        ProcessKfdRuntimeStateV1::Poisoned
    );
}

#[test]
fn mutex_poison_refuses_reservation_using_existing_gate_poison_handler() {
    let gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
    let binding = AtomicU32::new(0);
    let panic = std::panic::catch_unwind(|| {
        let _held = gate.lock().unwrap();
        panic!("synthetic gate-lock unwind");
    });
    assert!(panic.is_err());
    assert_runtime_error(
        DebugRuntimeReservationV1::reserve(&gate, &binding, pid()),
        "process-global gate poisoned",
    );
    assert!(lock_runtime_gate_v1(&gate).permanently_poisoned);
}

#[test]
fn unwind_rolls_back_only_before_external_exposure() {
    for exposed in [false, true] {
        let gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
        let binding = AtomicU32::new(0);
        let panic = std::panic::catch_unwind(|| {
            let mut owner = DebugRuntimeReservationV1::reserve(&gate, &binding, pid()).unwrap();
            if exposed {
                owner.begin_external_transition().unwrap();
            }
            panic!("synthetic owner unwind");
        });
        assert!(panic.is_err());
        let state = lock_runtime_gate_v1(&gate);
        assert_eq!(state.permanently_poisoned, exposed);
        assert_eq!(
            state.runtime,
            if exposed {
                ProcessKfdRuntimeStateV1::Poisoned
            } else {
                ProcessKfdRuntimeStateV1::Disabled
            }
        );
    }
}

#[test]
fn process_classifier_is_sticky_and_rejects_zero_or_foreign_pid() {
    let binding = AtomicU32::new(0);
    assert!(matches!(
        bind_gate_process_v1(&binding, 0),
        Err(LinuxDoorbellErrorV1::ProcessChanged)
    ));
    assert_eq!(binding.load(Ordering::Acquire), 0);
    bind_gate_process_v1(&binding, 17).unwrap();
    bind_gate_process_v1(&binding, 17).unwrap();
    assert!(matches!(
        bind_gate_process_v1(&binding, 18),
        Err(LinuxDoorbellErrorV1::ProcessChanged)
    ));
    assert_eq!(binding.load(Ordering::Acquire), 17);
}

#[test]
fn foreign_process_refusal_precedes_even_an_already_held_mutex() {
    let gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
    let binding = AtomicU32::new(pid());
    let held = gate.lock().unwrap();
    let foreign = if pid() == u32::MAX { 1 } else { pid() + 1 };
    assert!(matches!(
        DebugRuntimeReservationV1::reserve(&gate, &binding, foreign),
        Err(LinuxDoorbellErrorV1::ProcessChanged),
    ));
    assert_eq!(held.runtime, ProcessKfdRuntimeStateV1::Disabled);
}

#[test]
fn synthetic_owner_pid_drift_never_locks_or_rolls_back_inherited_claim() {
    let gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
    let binding = AtomicU32::new(0);
    let mut owner = DebugRuntimeReservationV1::reserve(&gate, &binding, pid()).unwrap();
    owner.opener_pid = 0; // inject mismatch; no actual fork or resource owner
    let held = gate.lock().unwrap();
    let before = held.runtime;
    drop(owner); // must not try to acquire this held mutex
    assert_eq!(held.runtime, before);
}

#[test]
fn cross_process_state_methods_cannot_join_or_release_any_profile() {
    for debug in [false, true] {
        let mut gate = ProcessGlobalKfdRuntimeGateV1::new();
        if debug {
            gate.reserve_debug_profile_v1(17).unwrap();
        } else {
            gate.runtime.commit_first_enabled(17);
        }
        let before = gate.runtime;
        assert!(matches!(
            gate.admit_runtime(18),
            Err(LinuxDoorbellErrorV1::ProcessChanged)
        ));
        assert!(matches!(
            gate.runtime.release_plan(18),
            Err(LinuxDoorbellErrorV1::ProcessChanged)
        ));
        assert!(matches!(
            gate.reserve_debug_profile_v1(18),
            Err(LinuxDoorbellErrorV1::ProcessChanged)
        ));
        assert_eq!(gate.runtime, before);
    }
}

#[test]
fn test_only_acknowledgment_releases_matching_exposed_claim() {
    let gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
    let binding = AtomicU32::new(0);
    let mut owner = DebugRuntimeReservationV1::reserve(&gate, &binding, pid()).unwrap();
    owner.begin_external_transition().unwrap();
    owner.acknowledge_synthetic_teardown().unwrap();
    let mut state = lock_runtime_gate_v1(&gate);
    assert_eq!(state.runtime, ProcessKfdRuntimeStateV1::Disabled);
    assert!(!state.permanently_poisoned);
    assert!(state.admit_runtime(pid()).unwrap());
}

#[test]
fn test_only_acknowledgment_refuses_wrong_phase_or_identity() {
    for wrong_identity in [false, true] {
        let mut gate = ProcessGlobalKfdRuntimeGateV1::new();
        let id = gate.reserve_debug_profile_v1(pid()).unwrap();
        if wrong_identity {
            gate.begin_debug_external_transition_v1(pid(), id).unwrap();
        }
        assert_runtime_error(
            gate.acknowledge_synthetic_debug_teardown_v1(
                pid(),
                if wrong_identity { id + 1 } else { id },
            ),
            "synthetic debug teardown identity or phase",
        );
        assert!(gate.permanently_poisoned);
    }
}

#[test]
fn production_source_has_no_native_transition_or_exposed_release_escape() {
    let source = include_str!("runtime_debug_profile_gate_v1.rs");
    assert!(!source.contains("rustix::ioctl::ioctl"));
    assert!(!source.contains("fn acknowledge_teardown("));
    assert!(!source.contains("unsafe"));
    assert!(source.contains("#[cfg(test)]\n    fn acknowledge_synthetic_debug_teardown_v1"));
    assert!(source.contains("#[cfg(test)]\n    fn acknowledge_synthetic_teardown"));
}

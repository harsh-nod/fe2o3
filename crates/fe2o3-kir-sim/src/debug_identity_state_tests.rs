//! Pure bookkeeping tests only. No simulator, capture or protocol capability
//! is exercised or claimed by this unintegrated module.

use std::fmt::Debug;
use std::mem::{needs_drop, size_of};

use fe2o3_kernel_ir::BlockId;

use super::*;

fn invocation(global: u64) -> SimulationInvocationV1 {
    SimulationInvocationV1 {
        global: [global, 0, 0],
        workgroup: [global / 2, 0, 0],
        local: [(global % 2) as u32, 0, 0],
        workgroup_size: [2, 1, 1],
        workgroup_count: [2, 1, 1],
        launch_extent: [4, 1, 1],
    }
}

fn limits(steps: u64) -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_steps: steps,
        ..SimulationLimitsV1::default()
    }
}

fn site(function: usize, block: u32, operation: u32) -> SimulationDebugSiteV1 {
    SimulationDebugSiteV1 {
        function_ordinal: function,
        block: BlockId(block),
        operation,
    }
}

fn states(steps: u64) -> (InvocationIdentityState, FrameIdentityState) {
    InvocationIdentityState::new(invocation(0), 7, limits(steps)).unwrap()
}

fn frame_snapshot(state: &FrameIdentityState) -> (FrameIdentity, usize, u64, u64, FramePhase) {
    (
        state.identity,
        state.function_ordinal,
        state.attempt_limit,
        state.last_attempt,
        state.phase,
    )
}

fn invocation_snapshot(
    state: &InvocationIdentityState,
) -> (SimulationInvocationV1, FrameActivationId, u64, u64) {
    (
        state.invocation,
        state.last_activation,
        state.activation_limit,
        state.attempt_limit,
    )
}

fn rejects_unchanged<T: Debug>(
    frame: &mut FrameIdentityState,
    expected: IdentityStateError,
    action: impl FnOnce(&mut FrameIdentityState) -> Result<T, IdentityStateError>,
) {
    let before = frame_snapshot(frame);
    assert_eq!(action(frame).unwrap_err(), expected);
    assert_eq!(frame_snapshot(frame), before);
}

fn rejects_reset_unchanged(
    state: &mut InvocationIdentityState,
    frame: &mut FrameIdentityState,
    expected: IdentityStateError,
) {
    let before_state = invocation_snapshot(state);
    let before_frame = frame_snapshot(frame);
    assert_eq!(state.reset_frame(frame, 9).unwrap_err(), expected);
    assert_eq!(invocation_snapshot(state), before_state);
    assert_eq!(frame_snapshot(frame), before_frame);
}

#[test]
fn root_and_first_attempt_are_nonzero_and_keys_retain_exact_existing_scope_and_site() {
    let (owner, mut root) = states(16);
    assert_eq!(owner.last_activation().get(), 1);
    assert_eq!(root.identity().activation().get(), 1);
    assert_eq!(root.identity().invocation(), invocation(0));
    assert_eq!(root.last_attempt(), 0);
    assert_eq!(root.pending(), None);
    assert!(!root.is_suspended());
    assert!(!root.is_retired());
    let location = site(7, 3, 11);
    let token = root.begin(location).unwrap();
    assert_eq!(token.frame(), root.identity());
    assert_eq!(token.attempt().get(), 1);
    assert_eq!(token.site(), location);
    assert_eq!(root.pending(), Some(token));
    assert_eq!(root.last_attempt(), 1);
}

#[test]
fn repeated_static_site_and_block_changes_increment_attempt_without_resetting_activation() {
    let (_, mut root) = states(16);
    let activation = root.identity();
    let repeated = site(7, 3, 11);
    let mut previous = None;
    for expected in 1..=5 {
        let token = root.begin(repeated).unwrap();
        assert_eq!(token.attempt().get(), expected);
        assert_eq!(token.frame(), activation);
        assert_eq!(token.site(), repeated);
        assert_ne!(Some(token), previous);
        previous = Some(token);
        root.complete(token).unwrap();
        assert_eq!(root.pending(), None);
    }
    let token = root.begin(site(7, 91, 0)).unwrap();
    assert_eq!(token.attempt().get(), 6);
    root.complete(token).unwrap();
    let token = root.begin(repeated).unwrap();
    assert_eq!(token.attempt().get(), 7);
}

#[test]
fn repeated_observation_and_suspend_resume_reuse_one_pending_attempt() {
    let (_, mut root) = states(16);
    let token = root.begin(site(7, 0, 0)).unwrap();
    for _ in 0..4 {
        assert_eq!(root.pending(), Some(token));
        root.suspend(token).unwrap();
        assert!(root.is_suspended());
        assert_eq!(root.pending(), Some(token));
        assert_eq!(root.resume(token).unwrap(), token);
        assert!(!root.is_suspended());
        assert_eq!(root.last_attempt(), 1);
    }
    root.complete(token).unwrap();
    assert_eq!(root.begin(site(7, 0, 0)).unwrap().attempt().get(), 2);
}

#[test]
fn nested_children_leave_each_suspended_caller_operation_unchanged() {
    let (mut owner, mut root) = states(32);
    let root_call = root.begin(site(7, 0, 3)).unwrap();
    root.suspend(root_call).unwrap();
    let root_before = frame_snapshot(&root);

    let mut child = owner.enter_frame(8).unwrap();
    let child_call = child.begin(site(8, 0, 4)).unwrap();
    child.suspend(child_call).unwrap();
    let child_before = frame_snapshot(&child);
    let mut grandchild = owner.enter_frame(9).unwrap();
    let work = grandchild.begin(site(9, 0, 1)).unwrap();
    assert_eq!(root.identity().activation().get(), 1);
    assert_eq!(child.identity().activation().get(), 2);
    assert_eq!(grandchild.identity().activation().get(), 3);
    assert_eq!(work.attempt().get(), 1);
    grandchild.complete(work).unwrap();
    grandchild.retire().unwrap();

    assert_eq!(frame_snapshot(&root), root_before);
    assert_eq!(frame_snapshot(&child), child_before);
    assert_eq!(child.resume(child_call).unwrap(), child_call);
    child.complete(child_call).unwrap();
    child.retire().unwrap();
    assert_eq!(frame_snapshot(&root), root_before);
    assert_eq!(root.resume(root_call).unwrap(), root_call);
    assert_eq!(root.pending(), Some(root_call));
    root.complete(root_call).unwrap();
}

#[test]
fn retired_slot_gets_a_fresh_activation_even_at_the_same_function_and_static_site() {
    let (mut owner, _) = states(16);
    let mut slot = owner.enter_frame(8).unwrap();
    let old = slot.begin(site(8, 0, 3)).unwrap();
    slot.complete(old).unwrap();
    slot.retire().unwrap();
    assert!(slot.is_retired());
    let old_identity = slot.identity();
    owner.reset_frame(&mut slot, 8).unwrap();
    assert!(!slot.is_retired());
    assert_eq!(slot.last_attempt(), 0);
    assert_eq!(slot.pending(), None);
    assert_eq!(slot.identity().activation().get(), 3);
    assert_ne!(slot.identity(), old_identity);
    let new = slot.begin(site(8, 0, 3)).unwrap();
    assert_eq!(new.attempt(), old.attempt());
    assert_eq!(new.site(), old.site());
    assert_ne!(new, old);
    rejects_unchanged(&mut slot, IdentityStateError::WrongActivation, |slot| {
        slot.complete(old)
    });
    slot.complete(new).unwrap();
    slot.retire().unwrap();
    owner.reset_frame(&mut slot, 91).unwrap();
    assert_eq!(slot.identity().activation().get(), 4);
    assert_eq!(slot.begin(site(91, 0, 0)).unwrap().attempt().get(), 1);
}

#[test]
fn live_running_and_suspended_slots_cannot_be_reset_and_do_not_consume_activation() {
    let (mut owner, _) = states(16);
    let mut slot = owner.enter_frame(8).unwrap();
    rejects_reset_unchanged(&mut owner, &mut slot, IdentityStateError::FrameNotRetired);
    let token = slot.begin(site(8, 0, 0)).unwrap();
    rejects_reset_unchanged(&mut owner, &mut slot, IdentityStateError::FrameNotRetired);
    slot.suspend(token).unwrap();
    rejects_reset_unchanged(&mut owner, &mut slot, IdentityStateError::FrameNotRetired);
    assert_eq!(owner.last_activation().get(), 2);
}

#[test]
fn root_cannot_be_reset_or_reissued_through_a_retired_slot() {
    let (mut owner, mut root) = states(16);
    rejects_reset_unchanged(&mut owner, &mut root, IdentityStateError::CannotResetRoot);
    root.retire().unwrap();
    rejects_reset_unchanged(&mut owner, &mut root, IdentityStateError::CannotResetRoot);
    assert_eq!(owner.last_activation().get(), 1);
}

#[test]
fn keys_distinguish_invocations_even_with_equal_numeric_tokens_and_static_sites() {
    let (_, mut first) = InvocationIdentityState::new(invocation(0), 7, limits(8)).unwrap();
    let (_, mut second) = InvocationIdentityState::new(invocation(1), 7, limits(8)).unwrap();
    let a = first.begin(site(7, 0, 0)).unwrap();
    let b = second.begin(site(7, 0, 0)).unwrap();
    assert_eq!(a.frame().activation(), b.frame().activation());
    assert_eq!(a.attempt(), b.attempt());
    assert_eq!(a.site(), b.site());
    assert_ne!(a.frame(), b.frame());
    assert_ne!(a, b);
    rejects_unchanged(&mut first, IdentityStateError::WrongInvocation, |state| {
        state.complete(b)
    });
    rejects_unchanged(&mut second, IdentityStateError::WrongInvocation, |state| {
        state.suspend(a)
    });
}

#[test]
fn full_invocation_scope_not_just_global_coordinates_participates_in_key_equality() {
    let first_scope = invocation(0);
    let mut second_scope = first_scope;
    second_scope.workgroup_count = [3, 1, 1];
    second_scope.launch_extent = [6, 1, 1];
    let (_, mut first) = InvocationIdentityState::new(first_scope, 7, limits(8)).unwrap();
    let (_, mut second) = InvocationIdentityState::new(second_scope, 7, limits(8)).unwrap();
    assert_ne!(
        first.begin(site(7, 0, 0)).unwrap(),
        second.begin(site(7, 0, 0)).unwrap(),
    );
}

#[test]
fn foreign_invocation_slot_cannot_consume_an_activation_or_be_reset() {
    let (mut owner, _) = states(8);
    let (mut other, _) = InvocationIdentityState::new(invocation(1), 7, limits(8)).unwrap();
    let mut foreign = other.enter_frame(8).unwrap();
    foreign.retire().unwrap();
    rejects_reset_unchanged(
        &mut owner,
        &mut foreign,
        IdentityStateError::WrongInvocation,
    );
}

#[test]
fn wrong_function_and_stale_site_or_attempt_tokens_leave_frame_unchanged() {
    let (_, mut root) = states(16);
    rejects_unchanged(&mut root, IdentityStateError::WrongFunction, |state| {
        state.begin(site(8, 0, 0))
    });
    let token = root.begin(site(7, 0, 0)).unwrap();
    let wrong_site = OperationIdentity {
        site: site(7, 0, 1),
        ..token
    };
    let wrong_attempt = OperationIdentity {
        attempt: OperationAttemptId(NonZeroU64::new(2).unwrap()),
        ..token
    };
    for wrong in [wrong_site, wrong_attempt] {
        rejects_unchanged(&mut root, IdentityStateError::OperationMismatch, |state| {
            state.suspend(wrong)
        });
        rejects_unchanged(&mut root, IdentityStateError::OperationMismatch, |state| {
            state.complete(wrong)
        });
    }
    root.suspend(token).unwrap();
    rejects_unchanged(&mut root, IdentityStateError::OperationMismatch, |state| {
        state.resume(wrong_site)
    });
    root.resume(token).unwrap();
    root.complete(token).unwrap();
    let next = root.begin(site(7, 0, 0)).unwrap();
    rejects_unchanged(&mut root, IdentityStateError::OperationMismatch, |state| {
        state.complete(token)
    });
    assert_eq!(root.pending(), Some(next));
}

#[test]
fn invalid_phase_transitions_preserve_pending_context_and_counters() {
    let (_, mut root) = states(16);
    let token = root.begin(site(7, 0, 0)).unwrap();
    rejects_unchanged(
        &mut root,
        IdentityStateError::OperationAlreadyPending,
        |state| state.begin(site(7, 0, 1)),
    );
    rejects_unchanged(
        &mut root,
        IdentityStateError::OperationAlreadyPending,
        FrameIdentityState::retire,
    );
    rejects_unchanged(
        &mut root,
        IdentityStateError::OperationNotSuspended,
        |state| state.resume(token),
    );
    root.suspend(token).unwrap();
    rejects_unchanged(&mut root, IdentityStateError::AlreadySuspended, |state| {
        state.suspend(token)
    });
    rejects_unchanged(&mut root, IdentityStateError::OperationSuspended, |state| {
        state.complete(token)
    });
    rejects_unchanged(
        &mut root,
        IdentityStateError::OperationAlreadyPending,
        |state| state.begin(site(7, 0, 1)),
    );
    root.resume(token).unwrap();
    root.complete(token).unwrap();
    rejects_unchanged(&mut root, IdentityStateError::NoPendingOperation, |state| {
        state.complete(token)
    });
    rejects_unchanged(&mut root, IdentityStateError::NoPendingOperation, |state| {
        state.suspend(token)
    });
    rejects_unchanged(&mut root, IdentityStateError::NoPendingOperation, |state| {
        state.resume(token)
    });
    root.retire().unwrap();
    rejects_unchanged(
        &mut root,
        IdentityStateError::FrameRetired,
        FrameIdentityState::retire,
    );
    rejects_unchanged(&mut root, IdentityStateError::FrameRetired, |state| {
        state.begin(site(7, 0, 0))
    });
    rejects_unchanged(&mut root, IdentityStateError::FrameRetired, |state| {
        state.complete(token)
    });
}

#[test]
fn attempt_ceiling_comes_from_existing_steps_plus_one_final_failed_attempt() {
    let (_, mut root) = states(1);
    for expected in 1..=2 {
        let token = root.begin(site(7, 0, 0)).unwrap();
        assert_eq!(token.attempt().get(), expected);
        root.complete(token).unwrap();
    }
    rejects_unchanged(
        &mut root,
        IdentityStateError::AttemptLimit { limit: 2 },
        |state| state.begin(site(7, 0, 0)),
    );
    assert_eq!(root.last_attempt(), 2);
}

#[test]
fn activation_ceiling_and_rejected_reset_preserve_both_owner_and_slot() {
    let (mut owner, _) = states(2);
    let mut slot = owner.enter_frame(8).unwrap();
    slot.retire().unwrap();
    owner.reset_frame(&mut slot, 8).unwrap();
    assert_eq!(slot.identity().activation().get(), 3);
    slot.retire().unwrap();
    rejects_reset_unchanged(
        &mut owner,
        &mut slot,
        IdentityStateError::ActivationLimit { limit: 3 },
    );
    let before = invocation_snapshot(&owner);
    assert_eq!(
        owner.enter_frame(8).unwrap_err(),
        IdentityStateError::ActivationLimit { limit: 3 }
    );
    assert_eq!(invocation_snapshot(&owner), before);
}

#[test]
fn attempt_overflow_never_wraps_to_zero_or_mutates_state() {
    let (_, mut root) = states(8);
    // Child tests inject the otherwise unreachable arithmetic boundary. The
    // constructor always derives a far smaller ceiling from validated limits.
    root.last_attempt = u64::MAX;
    root.attempt_limit = u64::MAX;
    rejects_unchanged(&mut root, IdentityStateError::CounterOverflow, |state| {
        state.begin(site(7, 0, 0))
    });
}

#[test]
fn activation_overflow_leaves_allocator_and_reusable_slot_unchanged() {
    let (mut owner, _) = states(8);
    let mut slot = owner.enter_frame(8).unwrap();
    slot.retire().unwrap();
    owner.last_activation = FrameActivationId(NonZeroU64::new(u64::MAX).unwrap());
    owner.activation_limit = u64::MAX;
    rejects_reset_unchanged(&mut owner, &mut slot, IdentityStateError::CounterOverflow);
    let before = invocation_snapshot(&owner);
    assert_eq!(
        owner.enter_frame(8).unwrap_err(),
        IdentityStateError::CounterOverflow
    );
    assert_eq!(invocation_snapshot(&owner), before);
}

#[test]
fn existing_simulation_limit_validation_precedes_root_creation() {
    for (steps, expected) in [
        (0, SimulationLimitsErrorV1::Zero("max_steps")),
        (u64::MAX, SimulationLimitsErrorV1::AboveHardCap("max_steps")),
    ] {
        assert_eq!(
            InvocationIdentityState::new(invocation(0), 7, limits(steps)).unwrap_err(),
            IdentityStateError::InvalidSimulationLimits(expected),
        );
    }
    let mut invalid = limits(8);
    invalid.max_call_depth = 0;
    assert_eq!(
        InvocationIdentityState::new(invocation(0), 7, invalid).unwrap_err(),
        IdentityStateError::InvalidSimulationLimits(SimulationLimitsErrorV1::Zero(
            "max_call_depth"
        )),
    );
}

#[test]
fn a_fresh_owner_resets_local_tokens_without_claiming_cross_run_uniqueness() {
    let (_, mut first_run) = states(8);
    let first = first_run.begin(site(7, 0, 0)).unwrap();
    first_run.complete(first).unwrap();
    let later = first_run.begin(site(7, 0, 0)).unwrap();
    let (_, mut new_run) = states(8);
    let first_again = new_run.begin(site(7, 0, 0)).unwrap();
    assert_eq!(
        first, first_again,
        "keys are scoped to their live run owner"
    );
    assert_ne!(later, first_again);
    assert_eq!(
        first.attempt().get(),
        1,
        "copied observations are immutable"
    );
}

#[test]
fn identity_state_has_only_fixed_size_storage_and_no_drop_owned_payloads() {
    assert!(!needs_drop::<InvocationIdentityState>());
    assert!(!needs_drop::<FrameIdentityState>());
    assert!(!needs_drop::<OperationIdentity>());
    assert!(!needs_drop::<FramePhase>());
    assert_eq!(size_of::<FrameActivationId>(), size_of::<u64>());
    assert_eq!(size_of::<OperationAttemptId>(), size_of::<u64>());
    assert!(
        size_of::<InvocationIdentityState>()
            <= size_of::<SimulationInvocationV1>() + 4 * size_of::<u64>()
    );
    assert!(
        size_of::<FrameIdentityState>()
            <= size_of::<SimulationInvocationV1>()
                + 12 * size_of::<u64>()
                + size_of::<Option<ParentCallIdentity>>()
    );
    assert!(
        size_of::<OperationIdentity>()
            <= size_of::<SimulationInvocationV1>() + 5 * size_of::<u64>()
    );
}

#[test]
fn parent_call_identity_is_compact_and_rebound_only_on_actual_fresh_activation() {
    let (mut owner, mut root) = states(32);
    let first = root.begin(site(7, 0, 0)).unwrap();
    root.suspend(first).unwrap();
    let mut child = owner.enter_frame(9).unwrap();
    child.set_parent(first).unwrap();
    let parent = child.parent().unwrap();
    assert_eq!(parent.activation(), first.frame().activation());
    assert_eq!(parent.attempt(), first.attempt());
    assert_eq!(parent.site(), first.site());
    assert!(!needs_drop::<ParentCallIdentity>());
    assert!(size_of::<ParentCallIdentity>() <= 4 * size_of::<u64>());
    assert!(size_of::<Option<ParentCallIdentity>>() <= 4 * size_of::<u64>());
    assert!(
        child.set_parent(first).is_err(),
        "parent cannot be replaced in an activation"
    );
    assert!(
        root.set_parent(first).is_err(),
        "root cannot acquire a caller"
    );
    let old_activation = child.identity().activation();
    child.retire().unwrap();
    owner.reset_frame(&mut child, 9).unwrap();
    assert!(child.parent().is_none());
    assert_ne!(child.identity().activation(), old_activation);
    root.resume(first).unwrap();
    root.complete(first).unwrap();
    let second = root.begin(site(7, 0, 0)).unwrap();
    root.suspend(second).unwrap();
    child.set_parent(second).unwrap();
    assert_eq!(child.parent().unwrap().attempt(), second.attempt());
    assert_ne!(child.parent().unwrap().attempt(), parent.attempt());
}

#[test]
fn wrong_invocation_parent_is_rejected_without_changing_child_custody() {
    let (mut owner, _) = states(32);
    let mut child = owner.enter_frame(9).unwrap();
    let (_, mut other) = InvocationIdentityState::new(invocation(1), 7, limits(32)).unwrap();
    let call = other.begin(site(7, 0, 0)).unwrap();
    other.suspend(call).unwrap();
    assert_eq!(
        child.set_parent(call),
        Err(IdentityStateError::WrongInvocation)
    );
    assert!(child.parent().is_none());
}
